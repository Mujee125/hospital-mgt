//! Global search — the titlebar's cross-module lookup (Phase 10).
//!
//! One command, RBAC-scoped section by section: a hit is returned from a
//! table only when the signed-in user holds that section's view permission
//! (the same guard the section's own list command uses). The server is the
//! source of truth; the titlebar merely renders what comes back.
//!
//! Contract:
//!   - Any authenticated user may call it (`require_session`); a user with
//!     no searchable permission simply gets an empty result set.
//!   - < 2 non-space characters returns an empty vec (no error) — the
//!     frontend won't even invoke it below that, the backend enforces it too.
//!   - Every query is parameter-bound. The user's term is LIKE-escaped
//!     (`%`/`_`/`\` neutralized with an explicit ESCAPE '\' clause) so a
//!     crafted wildcard cannot widen the match — the bind prevents SQL
//!     injection, the escape prevents wildcard injection.
//!   - Soft-deleted patients are excluded (CR-11 / HIPAA retention posture).
//!   - Sections run sequentially and are capped at 5 hits each — this is a
//!     navigation aid, not a report.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::rbac::{self, Permission, SessionState};

const MAX_HITS_PER_SECTION: i64 = 5;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GlobalSearchHit {
    /// "patient" | "doctor" | "appointment" | "invoice" | "lab_order" | "inventory_item"
    pub entity_type: String,
    pub id: i32,
    pub title: String,
    pub subtitle: Option<String>,
}

#[tauri::command]
pub async fn global_search(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    query: String,
) -> Result<Vec<GlobalSearchHit>, String> {
    global_search_core(pool.inner(), &session, query).await
}

/// Search logic core (AERP Part G extraction pattern).
pub async fn global_search_core(
    pool: &PgPool,
    session_state: &SessionState,
    query: String,
) -> Result<Vec<GlobalSearchHit>, String> {
    let s = rbac::require_session(session_state)?;

    let term = query.trim().to_lowercase();
    if term.chars().count() < 2 {
        return Ok(Vec::new());
    }
    // Escape LIKE wildcards so the term matches literally. Backslash first,
    // then the wildcards themselves.
    let escaped = term
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    let pattern = format!("%{}%", escaped);

    let mut hits: Vec<GlobalSearchHit> = Vec::new();

    // ── Patients (patients.view) ─────────────────────────────────────────
    if s.has(Permission::PatientsView) {
        let rows = sqlx::query_as::<_, (i32, String, String, Option<String>, String)>(
            r#"
            SELECT id, first_name || ' ' || last_name,
                   COALESCE(mrn, ''), phone, gender
            FROM patients
            WHERE deleted_at IS NULL
              AND (LOWER(first_name) LIKE $1 ESCAPE '\'
                   OR LOWER(last_name) LIKE $1 ESCAPE '\'
                   OR LOWER(COALESCE(mrn, '')) LIKE $1 ESCAPE '\'
                   OR phone LIKE $1 ESCAPE '\')
            ORDER BY id
            LIMIT $2
            "#,
        )
        .bind(&pattern)
        .bind(MAX_HITS_PER_SECTION)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Search patients: {}", e))?;
        for (id, name, mrn, phone, gender) in rows {
            let mut sub = mrn.clone();
            if let Some(p) = &phone {
                if !p.is_empty() {
                    sub = if sub.is_empty() { p.clone() } else { format!("{} · {}", sub, p) };
                }
            }
            if !gender.is_empty() {
                sub = if sub.is_empty() {
                    gender.clone()
                } else {
                    format!("{} · {}", sub, gender)
                };
            }
            hits.push(GlobalSearchHit {
                entity_type: "patient".into(),
                id,
                title: name,
                subtitle: if sub.is_empty() { None } else { Some(sub) },
            });
        }
    }

    // ── Doctors (doctors.view) ───────────────────────────────────────────
    if s.has(Permission::DoctorsView) {
        let rows = sqlx::query_as::<_, (i32, String, String, Option<bool>)>(
            r#"
            SELECT id, first_name || ' ' || last_name, specialization, is_active
            FROM doctors
            WHERE LOWER(first_name) LIKE $1 ESCAPE '\'
               OR LOWER(last_name) LIKE $1 ESCAPE '\'
               OR LOWER(specialization) LIKE $1 ESCAPE '\'
            ORDER BY id
            LIMIT $2
            "#,
        )
        .bind(&pattern)
        .bind(MAX_HITS_PER_SECTION)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Search doctors: {}", e))?;
        for (id, name, spec, active) in rows {
            let sub = match active {
                Some(false) => format!("{} · inactive", spec),
                _ => spec,
            };
            hits.push(GlobalSearchHit {
                entity_type: "doctor".into(),
                id,
                title: name,
                subtitle: Some(sub),
            });
        }
    }

    // ── Appointments (appointments.view) ────────────────────────────────
    // Matched on the patient's or doctor's name (the reception flow: "when
    // is Mr X coming in?").
    if s.has(Permission::AppointmentsView) {
        let rows = sqlx::query_as::<_, (i32, String, Option<chrono::NaiveDate>)>(
            r#"
            SELECT a.id,
                   p.first_name || ' ' || p.last_name
                   || ' — ' || a.appointment_date::text
                   || ' ' || a.appointment_time::text,
                   a.appointment_date
            FROM appointments a
            JOIN patients p ON p.id = a.patient_id
            LEFT JOIN doctors d2 ON d2.id = a.doctor_id
            WHERE (LOWER(p.first_name) LIKE $1 ESCAPE '\'
                   OR LOWER(p.last_name) LIKE $1 ESCAPE '\'
                   OR LOWER(d2.last_name) LIKE $1 ESCAPE '\')
            ORDER BY a.appointment_date DESC, a.appointment_time DESC
            LIMIT $2
            "#,
        )
        .bind(&pattern)
        .bind(MAX_HITS_PER_SECTION)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Search appointments: {}", e))?;
        for (id, title, date) in rows {
            hits.push(GlobalSearchHit {
                entity_type: "appointment".into(),
                id,
                title,
                subtitle: date.map(|d| d.to_string()),
            });
        }
    }

    // ── Invoices (billing.view) ─────────────────────────────────────────
    if s.has(Permission::BillingView) {
        let rows = sqlx::query_as::<_, (i32, String, Option<String>, Option<String>)>(
            r#"
            SELECT b.id, b.bill_number,
                   p.first_name || ' ' || p.last_name,
                   b.status
            FROM bills b
            JOIN patients p ON p.id = b.patient_id
            WHERE b.bill_number LIKE $1 ESCAPE '\'
               OR LOWER(p.first_name) LIKE $1 ESCAPE '\'
               OR LOWER(p.last_name) LIKE $1 ESCAPE '\'
            ORDER BY b.id DESC
            LIMIT $2
            "#,
        )
        .bind(&pattern)
        .bind(MAX_HITS_PER_SECTION)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Search bills: {}", e))?;
        for (id, number, patient, status) in rows {
            let sub = match (patient, status) {
                (Some(p), Some(st)) => format!("{} · {}", p, st),
                (Some(p), None) => p,
                (None, Some(st)) => st,
                (None, None) => String::new(),
            };
            hits.push(GlobalSearchHit {
                entity_type: "invoice".into(),
                id,
                title: number,
                subtitle: if sub.is_empty() { None } else { Some(sub) },
            });
        }
    }

    // ── Lab orders (lab.view) ───────────────────────────────────────────
    if s.has(Permission::LabView) {
        let rows = sqlx::query_as::<_, (i32, String, Option<String>)>(
            r#"
            SELECT lo.id, p.first_name || ' ' || p.last_name, lo.status
            FROM lab_orders lo
            JOIN patients p ON p.id = lo.patient_id
            WHERE LOWER(p.first_name) LIKE $1 ESCAPE '\'
               OR LOWER(p.last_name) LIKE $1 ESCAPE '\'
            ORDER BY lo.id DESC
            LIMIT $2
            "#,
        )
        .bind(&pattern)
        .bind(MAX_HITS_PER_SECTION)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Search lab orders: {}", e))?;
        for (id, patient, status) in rows {
            hits.push(GlobalSearchHit {
                entity_type: "lab_order".into(),
                id,
                title: format!("Lab order #{} — {}", id, patient),
                subtitle: status,
            });
        }
    }

    // ── Inventory items (inventory.view) ────────────────────────────────
    if s.has(Permission::InventoryView) {
        let rows = sqlx::query_as::<_, (i32, String, Option<String>)>(
            r#"
            SELECT id, name, category
            FROM inventory_items
            WHERE LOWER(name) LIKE $1 ESCAPE '\'
               OR LOWER(COALESCE(sku, '')) LIKE $1 ESCAPE '\'
            ORDER BY name
            LIMIT $2
            "#,
        )
        .bind(&pattern)
        .bind(MAX_HITS_PER_SECTION)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Search inventory: {}", e))?;
        for (id, name, category) in rows {
            hits.push(GlobalSearchHit {
                entity_type: "inventory_item".into(),
                id,
                title: name,
                subtitle: category,
            });
        }
    }

    Ok(hits)
}
