//! Blood Bank — donor registry, donations, inventory, cross-matching,
//! reservations, issue, transfusion, discard, and full traceability
//! (Phase 2-E, SRS FR-0145–FR-0149).
//!
//! Reuses the architectural patterns established by the Radiology module
//! (RAD-BASELINE-1.0):
//!   - Server-side enum validation before any DB access (`validate_enum`).
//!   - Strict state machine for blood-unit lifecycle (`is_valid_unit_transition`).
//!   - RBAC on every command via `rbac::require`.
//!   - Audit logging on every write command via `audit::for_session`.
//!   - Soft-delete on donors + units (HIPAA §164.530(j) retention).
//!   - Server-side pagination (LIMIT/OFFSET) with count metadata.
//!   - SEQUENCE-based concurrency-safe number generation.
//!   - `sanitize_db_error` on every `.map_err()`.
//!
//! Clinical workflow (FR-0145–FR-0149):
//!   Donor Registration → Donation → Lab Screening → Component Separation →
//!   Storage → Inventory → Cross-Match → Reservation → Issue → Transfusion →
//!   Completion → Traceability → Archiving
//!
//! Blood-unit status state machine:
//!   available → reserved → issued → transfused
//!   available/reserved/issued → discarded
//!   available/reserved/issued → expired (auto)
//!   available/reserved/issued → quarantine
//!   'transfused'/'discarded'/'expired' are terminal.

use sqlx::PgPool;

use crate::audit;
use crate::models::{
    BloodCrossmatch, BloodDiscard, BloodDonation, BloodDonor, BloodIssue, BloodMovement,
    BloodTransfusion, BloodUnit, BloodUnitHistory, CreateBloodCrossmatch,
    CreateBloodDiscard, CreateBloodDonation, CreateBloodDonor, CreateBloodIssue,
    CreateBloodReservation, CreateBloodTransfusion, CreateBloodUnit,
};
use crate::rbac::{self, Permission, SessionState};

// ── Validated enum constants ─────────────────────────────────────────────────

const VALID_BLOOD_GROUPS: &[&str] = &["A", "B", "AB", "O"];
const VALID_RH_FACTORS: &[&str] = &["+", "-"];
const VALID_COMPONENT_TYPES: &[&str] = &[
    "whole_blood", "prbc", "ffp", "platelets", "cryoprecipitate", "plasma", "granulocytes",
];
const VALID_UNIT_STATUSES: &[&str] = &[
    "available", "reserved", "issued", "transfused", "discarded", "expired", "quarantine",
];
/// Referenced by the unit-test suite as the Rust-side contract; production
/// donor-status validation is not yet wired (tracked in verification report).
#[allow(dead_code)]
const VALID_DONOR_STATUSES: &[&str] = &["active", "deferred", "blacklisted"];
const VALID_CROSSMATCH_RESULTS: &[&str] =
    &["pending", "compatible", "incompatible", "weak", "indeterminate"];
const VALID_CROSSMATCH_METHODS: &[&str] =
    &["saline_37c", "ahg", "gel_card", "tube_ahg", "electronic"];
/// Referenced by the unit-test suite as the Rust-side contract; production
/// reservation-status validation is not yet wired (tracked in verification report).
#[allow(dead_code)]
const VALID_RESERVATION_STATUSES: &[&str] = &["active", "fulfilled", "expired", "cancelled"];
const VALID_PRIORITIES: &[&str] = &["routine", "urgent", "emergency", "stat"];
const VALID_ISSUE_TYPES: &[&str] = &["routine", "emergency", "uncrossmatched", "autologous"];
const VALID_DISCARD_REASONS: &[&str] = &[
    "expired", "contaminated", "hemolysed", "broken", "positive_screen", "insufficient_volume",
    "other",
];
const VALID_TRANSFUSION_OUTCOMES: &[&str] =
    &["completed", "reaction", "incomplete", "cancelled"];
const VALID_SCREENING_STATUSES: &[&str] = &["pending", "passed", "failed", "quarantine"];

fn validate_enum(value: &str, allowed: &[&str], field_name: &str) -> Result<(), String> {
    if !allowed.contains(&value) {
        return Err(format!(
            "Invalid {} '{}'. Allowed values: {}.",
            field_name,
            value,
            allowed.join(", ")
        ));
    }
    Ok(())
}

fn sanitize_db_error(e: &sqlx::Error) -> String {
    crate::db::sanitize_db_error(e)
}

// ── Blood-unit state machine ─────────────────────────────────────────────────
//
// available → reserved → issued → transfused  (happy path)
// available/reserved/issued → discarded / quarantine (interventional)
// any non-terminal → expired (auto, but allowed manually for correction)
// 'transfused', 'discarded', 'expired' are terminal.

const TERMINAL_UNIT_STATUSES: &[&str] = &["transfused", "discarded", "expired"];

fn is_valid_unit_transition(current: &str, target: &str) -> bool {
    if TERMINAL_UNIT_STATUSES.contains(&current) {
        return false; // terminal — no transitions out
    }
    // BE-08: The generic update_blood_unit_status command no longer permits
    // 'reserved → available' or 'issued → available'. Those "release"/"return"
    // transitions MUST go through the dedicated commands (cancel_blood_reservation,
    // return_blood_unit) which properly clear stale fields (reserved_for_patient_id,
    // reservation_id, issued_to_patient_id, issued_at). The generic command
    // left these fields populated, producing an "available" unit that falsely
    // showed as reserved/issued. The dedicated commands are the only path back
    // to 'available' from 'reserved' or 'issued'.
    matches!(
        (current, target),
        ("available", "reserved")
            | ("available", "issued")
            | ("available", "discarded")
            | ("available", "expired")
            | ("available", "quarantine")
            | ("reserved", "issued")
            | ("reserved", "discarded")
            | ("reserved", "expired")
            | ("reserved", "quarantine")
            | ("issued", "transfused")
            | ("issued", "discarded")
            | ("issued", "expired")
            | ("quarantine", "available")
            | ("quarantine", "discarded")
            | ("quarantine", "expired")
    )
}

fn allowed_unit_transitions_from(status: &str) -> String {
    match status {
        "available" => "reserved, issued, discarded, expired, quarantine".to_string(),
        "reserved" => "issued, discarded, expired, quarantine (use cancel_blood_reservation to release back to available)".to_string(),
        "issued" => "transfused, discarded, expired (use return_blood_unit to return to available)".to_string(),
        "quarantine" => "available, discarded, expired".to_string(),
        "transfused" => "(terminal — no further transitions)".to_string(),
        "discarded" => "(terminal — no further transitions)".to_string(),
        "expired" => "(terminal — no further transitions)".to_string(),
        _ => "(unknown status)".to_string(),
    }
}

// ── Shared SELECT fragments (joined reads) ───────────────────────────────────

const SELECT_UNITS: &str = r#"
    SELECT bu.id, bu.unit_number, bu.donation_id, bu.donor_id, bu.component_type,
           bu.blood_group, bu.rh_factor, bu.volume_ml, bu.collection_date,
           bu.expiry_date, bu.storage_temperature, bu.storage_location, bu.status,
           bu.reserved_for_patient_id, bu.reservation_id, bu.issued_to_patient_id,
           bu.issued_at, bu.transfused_at, bu.transfused_to_patient_id,
           bu.discarded_at, bu.discard_reason, bu.created_by_user_id,
           bu.created_at, bu.updated_at, bu.deleted_at,
           bd.first_name || ' ' || bd.last_name AS donor_name,
           p.first_name || ' ' || p.last_name AS patient_name,
           (DATE(bu.expiry_date) - DATE(NOW())) AS days_to_expiry
    FROM blood_units bu
    LEFT JOIN blood_donors bd ON bd.id = bu.donor_id
    LEFT JOIN patients p ON p.id = bu.reserved_for_patient_id
"#;

const SELECT_DONORS: &str = r#"
    SELECT bd.id, bd.donor_number, bd.patient_id, bd.first_name, bd.last_name,
           bd.date_of_birth, bd.gender, bd.blood_group, bd.rh_factor, bd.phone,
           bd.email, bd.address, bd.weight_kg, bd.height_cm, bd.last_donation_date,
           bd.total_donations, bd.status, bd.medically_deferred_until, bd.defer_reason,
           bd.notes, bd.created_by_user_id, bd.created_at, bd.updated_at, bd.deleted_at,
           p.first_name || ' ' || p.last_name AS patient_name
    FROM blood_donors bd
    LEFT JOIN patients p ON p.id = bd.patient_id
"#;

const SELECT_DONATIONS: &str = r#"
    SELECT bd.id, bd.donation_number, bd.donor_id, bd.donation_date, bd.collection_site,
           bd.collected_by_user_id, bd.volume_ml, bd.blood_group, bd.rh_factor,
           bd.bag_type, bd.status, bd.screening_status, bd.screening_notes,
           bd.screened_by_user_id, bd.screened_at, bd.hemoglobin_level,
           bd.blood_pressure, bd.pulse, bd.temperature_c, bd.notes,
           bd.created_at, bd.updated_at,
           d.first_name || ' ' || d.last_name AS donor_name
    FROM blood_donations bd
    LEFT JOIN blood_donors d ON d.id = bd.donor_id
"#;

const SELECT_CROSSMATCHES: &str = r#"
    SELECT bc.id, bc.unit_id, bc.patient_id, bc.doctor_id, bc.requested_by_user_id,
           bc.crossmatch_date, bc.method, bc.result, bc.reaction_grade,
           bc.incubation_time_min, bc.ahg_phase, bc.notes, bc.performed_by_user_id,
           bc.verified_by_user_id, bc.verified_at, bc.created_at, bc.updated_at,
           bu.unit_number AS unit_number,
           p.first_name || ' ' || p.last_name AS patient_name,
           d.first_name || ' ' || d.last_name AS doctor_name
    FROM blood_crossmatch_results bc
    LEFT JOIN blood_units bu ON bu.id = bc.unit_id
    LEFT JOIN patients p ON p.id = bc.patient_id
    LEFT JOIN doctors d ON d.id = bc.doctor_id
"#;

const SELECT_ISSUES: &str = r#"
    SELECT bi.id, bi.issue_number, bi.unit_id, bi.patient_id, bi.reservation_id,
           bi.crossmatch_id, bi.doctor_id, bi.issued_by_user_id, bi.issued_at,
           bi.issued_to_location, bi.issue_type, bi.clinical_indication,
           bi.special_instructions, bi.returned_at, bi.return_reason,
           bi.received_by_user_id, bi.created_at, bi.updated_at,
           bu.unit_number AS unit_number,
           p.first_name || ' ' || p.last_name AS patient_name,
           d.first_name || ' ' || d.last_name AS doctor_name,
           u.username AS issued_by_name
    FROM blood_issues bi
    LEFT JOIN blood_units bu ON bu.id = bi.unit_id
    LEFT JOIN patients p ON p.id = bi.patient_id
    LEFT JOIN doctors d ON d.id = bi.doctor_id
    LEFT JOIN users u ON u.id = bi.issued_by_user_id
"#;

const SELECT_TRANSFUSIONS: &str = r#"
    SELECT bt.id, bt.transfusion_number, bt.issue_id, bt.unit_id, bt.patient_id,
           bt.doctor_id, bt.nurse_id, bt.started_at, bt.completed_at,
           bt.volume_transfused_ml, bt.pre_transfusion_bp, bt.post_transfusion_bp,
           bt.pre_transfusion_temp, bt.post_transfusion_temp,
           bt.pre_transfusion_pulse, bt.post_transfusion_pulse,
           bt.reaction_observed, bt.reaction_type, bt.reaction_severity,
           bt.reaction_notes, bt.outcome, bt.notes, bt.created_at, bt.updated_at,
           bu.unit_number AS unit_number,
           p.first_name || ' ' || p.last_name AS patient_name,
           d.first_name || ' ' || d.last_name AS doctor_name
    FROM blood_transfusions bt
    LEFT JOIN blood_units bu ON bu.id = bt.unit_id
    LEFT JOIN patients p ON p.id = bt.patient_id
    LEFT JOIN doctors d ON d.id = bt.doctor_id
"#;

const SELECT_DISCARDS: &str = r#"
    SELECT bdi.id, bdi.unit_id, bdi.discard_number, bdi.discarded_at, bdi.discard_reason,
           bdi.discard_notes, bdi.discarded_by_user_id, bdi.authorized_by_user_id,
           bdi.disposal_method, bdi.created_at, bdi.updated_at,
           bu.unit_number AS unit_number,
           u.username AS discarded_by_name
    FROM blood_discards bdi
    LEFT JOIN blood_units bu ON bu.id = bdi.unit_id
    LEFT JOIN users u ON u.id = bdi.discarded_by_user_id
"#;

// ── Helper: insert a status-history row + inventory movement ─────────────────

async fn record_unit_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    unit_id: i32,
    status: &str,
    user_id: i32,
    notes: Option<&str>,
    related_record_type: Option<&str>,
    related_record_id: Option<i32>,
) -> Result<(), String> {
    sqlx::query(
        r#"INSERT INTO blood_unit_status_history
              (unit_id, status, changed_by_user_id, notes, related_record_type, related_record_id)
           VALUES ($1, $2, $3, $4, $5, $6)"#,
    )
    .bind(unit_id)
    .bind(status)
    .bind(user_id)
    .bind(notes)
    .bind(related_record_type)
    .bind(related_record_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;
    Ok(())
}

// Audit helper: mirrors the inventory_movement table columns — one arg per column.
#[allow(clippy::too_many_arguments)]
async fn record_movement(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    unit_id: i32,
    movement_type: &str,
    from_location: Option<&str>,
    to_location: Option<&str>,
    user_id: i32,
    reason: Option<&str>,
    related_record_type: Option<&str>,
    related_record_id: Option<i32>,
) -> Result<(), String> {
    sqlx::query(
        r#"INSERT INTO blood_inventory_movements
              (unit_id, movement_type, from_location, to_location, moved_by_user_id,
               reason, related_record_type, related_record_id)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
    )
    .bind(unit_id)
    .bind(movement_type)
    .bind(from_location)
    .bind(to_location)
    .bind(user_id)
    .bind(reason)
    .bind(related_record_type)
    .bind(related_record_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;
    Ok(())
}

// ════════════════════════════════════════════════════════════════════════════
// FR-0146 — DONOR REGISTRY
// ════════════════════════════════════════════════════════════════════════════

/// List donors with server-side pagination. RBAC: `BloodBankView`.
/// Excludes soft-deleted records.
#[tauri::command]
#[allow(unused_assignments)] // bind_idx increments after the last condition are harmless
pub async fn get_blood_donors(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    search: Option<String>,
    blood_group_filter: Option<String>,
    status_filter: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<serde_json::Value, String> {
    let _ = rbac::require(&session, Permission::BloodBankView)?;

    let pg = page.unwrap_or(1).max(1);
    let ps = page_size.unwrap_or(10).clamp(1, 100);
    let offset = (pg - 1) * ps;

    let search_term = search.as_deref().filter(|s| !s.trim().is_empty()).map(|s| s.trim());
    let bg = blood_group_filter.as_deref().filter(|s| !s.is_empty());
    let st = status_filter.as_deref().filter(|s| !s.is_empty());

    // Build dynamic WHERE with parameterised binds.
    let mut conditions = vec!["bd.deleted_at IS NULL".to_string()];
    let mut bind_idx = 1;
    if search_term.is_some() {
        conditions.push(format!(
            "(bd.donor_number ILIKE ${0} OR bd.first_name ILIKE ${0} OR bd.last_name ILIKE ${0} OR bd.phone ILIKE ${0})",
            bind_idx
        ));
        bind_idx += 1;
    }
    if bg.is_some() {
        conditions.push(format!("bd.blood_group = ${}", bind_idx));
        bind_idx += 1;
    }
    if st.is_some() {
        conditions.push(format!("bd.status = ${}", bind_idx));
        bind_idx += 1;
    }
    let where_clause = format!("WHERE {}", conditions.join(" AND "));

    let count_sql = format!("SELECT COUNT(*) FROM blood_donors bd {}", where_clause);
    let mut count_q = sqlx::query_scalar::<_, i64>(&count_sql);
    if let Some(s) = search_term {
        count_q = count_q.bind(format!("%{}%", s));
    }
    if let Some(b) = bg {
        count_q = count_q.bind(b);
    }
    if let Some(s) = st {
        count_q = count_q.bind(s);
    }
    let total: i64 = count_q.fetch_one(pool.inner()).await.map_err(|e| sanitize_db_error(&e))?;

    let data_sql = format!(
        "{} {} ORDER BY bd.created_at DESC LIMIT ${} OFFSET ${}",
        SELECT_DONORS, where_clause, bind_idx, bind_idx + 1
    );
    let mut data_q = sqlx::query_as::<_, BloodDonor>(&data_sql);
    if let Some(s) = search_term {
        data_q = data_q.bind(format!("%{}%", s));
    }
    if let Some(b) = bg {
        data_q = data_q.bind(b);
    }
    if let Some(s) = st {
        data_q = data_q.bind(s);
    }
    data_q = data_q.bind(ps).bind(offset);
    let donors: Vec<BloodDonor> = data_q
        .fetch_all(pool.inner())
        .await
        .map_err(|e| sanitize_db_error(&e))?;

    Ok(serde_json::json!({
        "donors": donors,
        "total": total,
        "page": pg,
        "page_size": ps,
        "total_pages": ((total + ps - 1) / ps).max(1),
    }))
}

/// Get a single donor by id. RBAC: `BloodBankView`.
#[tauri::command]
pub async fn get_blood_donor(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    id: i32,
) -> Result<BloodDonor, String> {
    let _ = rbac::require(&session, Permission::BloodBankView)?;
    let q = format!("{} WHERE bd.id = $1 AND bd.deleted_at IS NULL", SELECT_DONORS);
    sqlx::query_as::<_, BloodDonor>(&q)
        .bind(id)
        .fetch_one(pool.inner())
        .await
        .map_err(|e| sanitize_db_error(&e))
}

/// Register a new blood donor. RBAC: `BloodBankDonorManage`. Audit-logged.
/// Validates blood_group, rh_factor, gender, and status before DB access.
#[tauri::command]
pub async fn create_blood_donor(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    donor: CreateBloodDonor,
) -> Result<i32, String> {
    let s = rbac::require(&session, Permission::BloodBankDonorManage)?;

    let blood_group = donor.blood_group.trim();
    let rh_factor = donor.rh_factor.trim();
    validate_enum(blood_group, VALID_BLOOD_GROUPS, "blood_group")?;
    validate_enum(rh_factor, VALID_RH_FACTORS, "rh_factor")?;
    if let Some(g) = donor.gender.as_deref() {
        if !g.is_empty() {
            let valid_genders = ["male", "female", "other"];
            if !valid_genders.contains(&g) {
                return Err(format!(
                    "Invalid gender '{}'. Allowed values: male, female, other.",
                    g
                ));
            }
        }
    }

    let dob: Option<chrono::NaiveDate> = match donor.date_of_birth.as_deref() {
        Some(s) if !s.trim().is_empty() => Some(
            chrono::NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d")
                .map_err(|e| format!("Invalid date_of_birth '{}': {}", s, e))?,
        ),
        _ => None,
    };

    let mut tx = pool.begin().await.map_err(|e| sanitize_db_error(&e))?;

    let donor_number_row: (String,) = sqlx::query_as(
        "SELECT 'DON-' || TO_CHAR(NOW(),'YYYYMMDD') || '-' || \
         LPAD(nextval('blood_donor_seq')::text, 6, '0')",
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;
    let donor_number = donor_number_row.0;

    let row: (i32,) = sqlx::query_as(
        r#"INSERT INTO blood_donors
              (donor_number, patient_id, first_name, last_name, date_of_birth, gender,
               blood_group, rh_factor, phone, email, address, weight_kg, height_cm,
               status, notes, created_by_user_id)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,'active',$14,$15)
           RETURNING id"#,
    )
    .bind(&donor_number)
    .bind(donor.patient_id)
    .bind(donor.first_name.trim())
    .bind(donor.last_name.trim())
    .bind(dob)
    .bind(donor.gender.as_deref().filter(|s| !s.is_empty()))
    .bind(blood_group)
    .bind(rh_factor)
    .bind(donor.phone.as_deref())
    .bind(donor.email.as_deref())
    .bind(donor.address.as_deref())
    .bind(donor.weight_kg.map(rust_decimal::Decimal::from_f64_retain))
    .bind(donor.height_cm.map(rust_decimal::Decimal::from_f64_retain))
    .bind(donor.notes.as_deref())
    .bind(s.user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    tx.commit().await.map_err(|e| sanitize_db_error(&e))?;

    audit::for_session(
        pool.inner(),
        &s,
        "blood_donor_create",
        "blood_donors",
        Some(&row.0.to_string()),
        Some(serde_json::json!({
            "donor_number": donor_number,
            "blood_group": blood_group,
            "rh_factor": rh_factor,
        })),
    )
    .await;
    Ok(row.0)
}

/// Soft-delete a donor. RBAC: `BloodBankDonorManage`. Audit-logged.
/// Refuses if the donor has active blood units in inventory (RESTRICT-equivalent).
#[tauri::command]
pub async fn delete_blood_donor(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    id: i32,
    reason: Option<String>,
) -> Result<(), String> {
    let s = rbac::require(&session, Permission::BloodBankDonorManage)?;

    let mut tx = pool.begin().await.map_err(|e| sanitize_db_error(&e))?;

    // Safety check: refuse deletion if donor has non-terminal units in inventory.
    let active_units: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM blood_units WHERE donor_id = $1 \
         AND status IN ('available','reserved','issued','quarantine') AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    if active_units > 0 {
        return Err(format!(
            "Cannot delete donor {}: {} active blood unit(s) still in inventory. \
             Discard or transfuse them first.",
            id, active_units
        ));
    }

    let result = sqlx::query(
        r#"UPDATE blood_donors
           SET deleted_at = NOW(), deleted_by_user_id = $1, deleted_reason = $2,
               updated_at = NOW()
           WHERE id = $3 AND deleted_at IS NULL"#,
    )
    .bind(s.user_id)
    .bind(reason.as_deref())
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    if result.rows_affected() == 0 {
        return Err("Blood donor not found or already deleted.".to_string());
    }

    tx.commit().await.map_err(|e| sanitize_db_error(&e))?;

    audit::for_session(
        pool.inner(),
        &s,
        "blood_donor_delete",
        "blood_donors",
        Some(&id.to_string()),
        Some(serde_json::json!({ "soft_delete": true, "reason": reason })),
    )
    .await;
    Ok(())
}

// ════════════════════════════════════════════════════════════════════════════
// FR-0146 — DONATIONS (collection + screening)
// ════════════════════════════════════════════════════════════════════════════

/// List donations with pagination. RBAC: `BloodBankView`.
#[tauri::command]
pub async fn get_blood_donations(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    donor_id: Option<i32>,
    screening_status_filter: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<serde_json::Value, String> {
    let _ = rbac::require(&session, Permission::BloodBankView)?;

    let pg = page.unwrap_or(1).max(1);
    let ps = page_size.unwrap_or(10).clamp(1, 100);
    let offset = (pg - 1) * ps;
    let ss = screening_status_filter.as_deref().filter(|s| !s.is_empty());

    let mut conditions: Vec<String> = vec![];
    let mut bind_idx = 1;
    if donor_id.is_some() {
        conditions.push(format!("bd.donor_id = ${}", bind_idx));
        bind_idx += 1;
    }
    if ss.is_some() {
        conditions.push(format!("bd.screening_status = ${}", bind_idx));
        bind_idx += 1;
    }
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let count_sql = format!("SELECT COUNT(*) FROM blood_donations bd {}", where_clause);
    let mut count_q = sqlx::query_scalar::<_, i64>(&count_sql);
    if let Some(d) = donor_id {
        count_q = count_q.bind(d);
    }
    if let Some(s) = ss {
        count_q = count_q.bind(s);
    }
    let total: i64 = count_q.fetch_one(pool.inner()).await.map_err(|e| sanitize_db_error(&e))?;

    let data_sql = format!(
        "{} {} ORDER BY bd.donation_date DESC LIMIT ${} OFFSET ${}",
        SELECT_DONATIONS, where_clause, bind_idx, bind_idx + 1
    );
    let mut data_q = sqlx::query_as::<_, BloodDonation>(&data_sql);
    if let Some(d) = donor_id {
        data_q = data_q.bind(d);
    }
    if let Some(s) = ss {
        data_q = data_q.bind(s);
    }
    data_q = data_q.bind(ps).bind(offset);
    let donations: Vec<BloodDonation> = data_q
        .fetch_all(pool.inner())
        .await
        .map_err(|e| sanitize_db_error(&e))?;

    Ok(serde_json::json!({
        "donations": donations,
        "total": total,
        "page": pg,
        "page_size": ps,
        "total_pages": ((total + ps - 1) / ps).max(1),
    }))
}

/// Record a new blood donation. RBAC: `BloodBankDonorManage`. Audit-logged.
/// Creates the donation record AND a corresponding blood_unit (whole_blood)
/// with default 35-day expiry for whole blood.
#[tauri::command]
pub async fn create_blood_donation(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    donation: CreateBloodDonation,
) -> Result<i32, String> {
    let s = rbac::require(&session, Permission::BloodBankDonorManage)?;

    let blood_group = donation.blood_group.trim();
    let rh_factor = donation.rh_factor.trim();
    validate_enum(blood_group, VALID_BLOOD_GROUPS, "blood_group")?;
    validate_enum(rh_factor, VALID_RH_FACTORS, "rh_factor")?;

    if donation.volume_ml <= 0 || donation.volume_ml > 600 {
        return Err(format!(
            "Invalid volume_ml {}: must be between 1 and 600.",
            donation.volume_ml
        ));
    }

    let mut tx = pool.begin().await.map_err(|e| sanitize_db_error(&e))?;

    // Verify the donor exists, is not deleted, and is not blacklisted/deferred.
    let donor_row: Option<(String, String)> = sqlx::query_as(
        "SELECT status, blood_group || rh_factor FROM blood_donors \
         WHERE id = $1 AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(donation.donor_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    let (donor_status, _donor_blood_type) = donor_row
        .ok_or_else(|| format!("Blood donor {} not found or deleted.", donation.donor_id))?;

    if donor_status == "blacklisted" {
        return Err(format!(
            "Donor {} is blacklisted and cannot donate.",
            donation.donor_id
        ));
    }
    if donor_status == "deferred" {
        return Err(format!(
            "Donor {} is medically deferred. Clear the deferral before collecting.",
            donation.donor_id
        ));
    }

    let donation_number_row: (String,) = sqlx::query_as(
        "SELECT 'BDN-' || TO_CHAR(NOW(),'YYYYMMDD') || '-' || \
         LPAD(nextval('blood_donation_seq')::text, 6, '0')",
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;
    let donation_number = donation_number_row.0;

    let row: (i32,) = sqlx::query_as(
        r#"INSERT INTO blood_donations
              (donation_number, donor_id, collection_site, collected_by_user_id,
               volume_ml, blood_group, rh_factor, bag_type, status, screening_status,
               hemoglobin_level, blood_pressure, pulse, temperature_c, notes)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'collected','pending',$9,$10,$11,$12,$13)
           RETURNING id"#,
    )
    .bind(&donation_number)
    .bind(donation.donor_id)
    .bind(donation.collection_site.as_deref())
    .bind(s.user_id)
    .bind(donation.volume_ml)
    .bind(blood_group)
    .bind(rh_factor)
    .bind(donation.bag_type.as_deref().unwrap_or("single"))
    .bind(donation.hemoglobin_level.map(rust_decimal::Decimal::from_f64_retain))
    .bind(donation.blood_pressure.as_deref())
    .bind(donation.pulse)
    .bind(donation.temperature_c.map(rust_decimal::Decimal::from_f64_retain))
    .bind(donation.notes.as_deref())
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;
    let donation_id = row.0;

    // Auto-create the corresponding whole-blood unit with 35-day expiry.
    let unit_number_row: (String,) = sqlx::query_as(
        "SELECT 'BU-' || TO_CHAR(NOW(),'YYYYMMDD') || '-' || \
         LPAD(nextval('blood_unit_seq')::text, 6, '0')",
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;
    let unit_number = unit_number_row.0;

    // BE-06: Unit is created in 'quarantine' status — NOT 'available'. The
    // unit only becomes available inventory after the linked donation passes
    // infectious-disease screening (see update_blood_donation_screening). This
    // prevents unscreened, potentially infectious blood from being reserved,
    // cross-matched, or issued. The state machine allows quarantine → available
    // (verified in is_valid_unit_transition).
    let unit_row: (i32,) = sqlx::query_as(
        r#"INSERT INTO blood_units
              (unit_number, donation_id, donor_id, component_type, blood_group, rh_factor,
               volume_ml, collection_date, expiry_date, storage_temperature,
               storage_location, status, created_by_user_id)
           VALUES ($1,$2,$3,'whole_blood',$4,$5,$6,NOW(),
                   NOW() + INTERVAL '35 days','2-6°C','Blood Bank Storage',
                   'quarantine',$7) RETURNING id"#,
    )
    .bind(&unit_number)
    .bind(donation_id)
    .bind(donation.donor_id)
    .bind(blood_group)
    .bind(rh_factor)
    .bind(donation.volume_ml)
    .bind(s.user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;
    let unit_id = unit_row.0;

    // Record initial unit status + movement.
    record_unit_event(
        &mut tx,
        unit_id,
        "quarantine",
        s.user_id,
        Some("Unit created from donation — pending screening"),
        Some("donation"),
        Some(donation_id),
    )
    .await?;
    record_movement(
        &mut tx,
        unit_id,
        "received",
        None,
        Some("Blood Bank Storage (Quarantine)"),
        s.user_id,
        Some("Received from donation — quarantined pending screening"),
        Some("donation"),
        Some(donation_id),
    )
    .await?;

    // Update donor's last_donation_date + total_donations.
    sqlx::query(
        "UPDATE blood_donors SET last_donation_date = CURRENT_DATE, \
         total_donations = total_donations + 1, updated_at = NOW() WHERE id = $1",
    )
    .bind(donation.donor_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    tx.commit().await.map_err(|e| sanitize_db_error(&e))?;

    audit::for_session(
        pool.inner(),
        &s,
        "blood_donation_create",
        "blood_donations",
        Some(&donation_id.to_string()),
        Some(serde_json::json!({
            "donation_number": donation_number,
            "donor_id": donation.donor_id,
            "unit_id": unit_id,
            "unit_number": unit_number,
            "volume_ml": donation.volume_ml,
        })),
    )
    .await;
    Ok(donation_id)
}

/// Update a donation's screening status (lab screening step).
/// RBAC: `BloodBankDonorManage` (lab techs). Audit-logged.
/// When screening passes, the linked blood unit remains available; when it
/// fails, the unit is moved to quarantine.
#[tauri::command]
pub async fn update_blood_donation_screening(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    donation_id: i32,
    screening_status: String,
    notes: Option<String>,
) -> Result<(), String> {
    let s = rbac::require(&session, Permission::BloodBankDonorManage)?;

    let ss = screening_status.trim();
    validate_enum(ss, VALID_SCREENING_STATUSES, "screening_status")?;

    let mut tx = pool.begin().await.map_err(|e| sanitize_db_error(&e))?;

    let current: Option<(String,)> = sqlx::query_as(
        "SELECT screening_status FROM blood_donations WHERE id = $1 FOR UPDATE",
    )
    .bind(donation_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    let _old = current
        .ok_or_else(|| format!("Donation {} not found.", donation_id))?
        .0;

    sqlx::query(
        r#"UPDATE blood_donations
           SET screening_status = $1, screening_notes = $2,
               screened_by_user_id = $3, screened_at = NOW(),
               status = CASE WHEN $1 = 'passed' THEN 'screened'
                             WHEN $1 = 'failed' THEN 'rejected'
                             ELSE status END,
               updated_at = NOW()
           WHERE id = $4"#,
    )
    .bind(ss)
    .bind(notes.as_deref())
    .bind(s.user_id)
    .bind(donation_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    // BE-06: Transition the linked unit based on screening result.
    //   passed  → quarantine → available  (unit enters circulating inventory)
    //   failed  → any non-terminal → quarantine (unit removed from inventory)
    //   quarantine/pending → no status change (unit stays in current state)
    //
    // After BE-06, units are created in 'quarantine', so the 'passed' branch
    // is the normal path that releases a unit into available inventory. The
    // 'failed' branch handles both the post-BE-06 case (unit already in
    // quarantine — stays there) and the defensive case (a unit that was
    // somehow already 'available' — pulled back to quarantine).
    let unit_row: Option<(i32,)> = sqlx::query_as(
        "SELECT id FROM blood_units WHERE donation_id = $1 AND deleted_at IS NULL",
    )
    .bind(donation_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    if let Some((unit_id,)) = unit_row {
        let unit_status: Option<(String,)> = sqlx::query_as(
            "SELECT status FROM blood_units WHERE id = $1 FOR UPDATE",
        )
        .bind(unit_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| sanitize_db_error(&e))?;

        if let Some((cur,)) = unit_status {
            // Only transition non-terminal units (transfused/discarded/expired
            // are terminal — a late screening result on an already-transfused
            // unit is recorded on the donation but cannot change unit status).
            if !TERMINAL_UNIT_STATUSES.contains(&cur.as_str()) {
                let (new_status, event_note) = match ss {
                    "passed" if cur == "quarantine" => (
                        "available",
                        "Screening passed — released to available inventory",
                    ),
                    "failed" if cur != "quarantine" => (
                        "quarantine",
                        "Quarantined — screening failed",
                    ),
                    // 'quarantine' or 'pending' screening result: no status change
                    _ => ("", ""),
                };

                if !new_status.is_empty() {
                    sqlx::query(
                        "UPDATE blood_units SET status = $1, updated_at = NOW() WHERE id = $2",
                    )
                    .bind(new_status)
                    .bind(unit_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| sanitize_db_error(&e))?;

                    record_unit_event(
                        &mut tx, unit_id, new_status, s.user_id,
                        Some(event_note),
                        Some("donation"), Some(donation_id),
                    ).await?;
                }
            }
        }
    }

    tx.commit().await.map_err(|e| sanitize_db_error(&e))?;

    audit::for_session(
        pool.inner(),
        &s,
        "blood_donation_screening",
        "blood_donations",
        Some(&donation_id.to_string()),
        Some(serde_json::json!({
            "screening_status": ss,
            "notes": notes,
        })),
    )
    .await;
    Ok(())
}

// ════════════════════════════════════════════════════════════════════════════
// FR-0145 — BLOOD INVENTORY
// ════════════════════════════════════════════════════════════════════════════

/// List blood units (inventory) with server-side pagination + filters.
/// RBAC: `BloodBankView`. Excludes soft-deleted units.
#[tauri::command]
#[allow(unused_assignments)] // bind_idx increments after the last condition are harmless
#[allow(clippy::too_many_arguments)] // IPC filter surface — stable command contract
pub async fn get_blood_units(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    status_filter: Option<String>,
    blood_group_filter: Option<String>,
    rh_filter: Option<String>,
    component_filter: Option<String>,
    expiring_days: Option<i32>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<serde_json::Value, String> {
    let _ = rbac::require(&session, Permission::BloodBankView)?;

    let pg = page.unwrap_or(1).max(1);
    let ps = page_size.unwrap_or(10).clamp(1, 100);
    let offset = (pg - 1) * ps;

    let st = status_filter.as_deref().filter(|s| !s.is_empty());
    let bg = blood_group_filter.as_deref().filter(|s| !s.is_empty());
    let rh = rh_filter.as_deref().filter(|s| !s.is_empty());
    let ct = component_filter.as_deref().filter(|s| !s.is_empty());

    let mut conditions = vec!["bu.deleted_at IS NULL".to_string()];
    let mut bind_idx = 1;
    if st.is_some() {
        conditions.push(format!("bu.status = ${}", bind_idx));
        bind_idx += 1;
    }
    if bg.is_some() {
        conditions.push(format!("bu.blood_group = ${}", bind_idx));
        bind_idx += 1;
    }
    if rh.is_some() {
        conditions.push(format!("bu.rh_factor = ${}", bind_idx));
        bind_idx += 1;
    }
    if ct.is_some() {
        conditions.push(format!("bu.component_type = ${}", bind_idx));
        bind_idx += 1;
    }
    if let Some(days) = expiring_days {
        if days > 0 {
            conditions.push(format!("bu.expiry_date <= NOW() + INTERVAL '1 day' * ${}", bind_idx));
            bind_idx += 1;
        }
    }
    let where_clause = format!("WHERE {}", conditions.join(" AND "));

    let count_sql = format!("SELECT COUNT(*) FROM blood_units bu {}", where_clause);
    let mut count_q = sqlx::query_scalar::<_, i64>(&count_sql);
    if let Some(s) = st { count_q = count_q.bind(s); }
    if let Some(b) = bg { count_q = count_q.bind(b); }
    if let Some(r) = rh { count_q = count_q.bind(r); }
    if let Some(c) = ct { count_q = count_q.bind(c); }
    if let Some(d) = expiring_days { if d > 0 { count_q = count_q.bind(d); } }
    let total: i64 = count_q.fetch_one(pool.inner()).await.map_err(|e| sanitize_db_error(&e))?;

    let data_sql = format!(
        "{} {} ORDER BY bu.expiry_date ASC LIMIT ${} OFFSET ${}",
        SELECT_UNITS, where_clause, bind_idx, bind_idx + 1
    );
    let mut data_q = sqlx::query_as::<_, BloodUnit>(&data_sql);
    if let Some(s) = st { data_q = data_q.bind(s); }
    if let Some(b) = bg { data_q = data_q.bind(b); }
    if let Some(r) = rh { data_q = data_q.bind(r); }
    if let Some(c) = ct { data_q = data_q.bind(c); }
    if let Some(d) = expiring_days { if d > 0 { data_q = data_q.bind(d); } }
    data_q = data_q.bind(ps).bind(offset);
    let units: Vec<BloodUnit> = data_q
        .fetch_all(pool.inner())
        .await
        .map_err(|e| sanitize_db_error(&e))?;

    Ok(serde_json::json!({
        "units": units,
        "total": total,
        "page": pg,
        "page_size": ps,
        "total_pages": ((total + ps - 1) / ps).max(1),
    }))
}

/// Search blood inventory by blood group + component type (the canonical
/// "do we have compatible stock?" query). RBAC: `BloodBankView`.
/// Returns available (non-deleted) units only.
#[tauri::command]
#[allow(unused_assignments)] // bind_idx increments after the last condition are harmless
pub async fn search_blood_inventory(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    blood_group: Option<String>,
    rh_factor: Option<String>,
    component_type: Option<String>,
) -> Result<Vec<BloodUnit>, String> {
    let _ = rbac::require(&session, Permission::BloodBankView)?;

    let bg = blood_group.as_deref().filter(|s| !s.is_empty());
    let rh = rh_factor.as_deref().filter(|s| !s.is_empty());
    let ct = component_type.as_deref().filter(|s| !s.is_empty());

    if let Some(b) = bg { validate_enum(b, VALID_BLOOD_GROUPS, "blood_group")?; }
    if let Some(r) = rh { validate_enum(r, VALID_RH_FACTORS, "rh_factor")?; }
    if let Some(c) = ct { validate_enum(c, VALID_COMPONENT_TYPES, "component_type")?; }

    let mut conditions = vec![
        "bu.deleted_at IS NULL".to_string(),
        "bu.status = 'available'".to_string(),
    ];
    let mut bind_idx = 1;
    if bg.is_some() { conditions.push(format!("bu.blood_group = ${}", bind_idx)); bind_idx += 1; }
    if rh.is_some() { conditions.push(format!("bu.rh_factor = ${}", bind_idx)); bind_idx += 1; }
    if ct.is_some() { conditions.push(format!("bu.component_type = ${}", bind_idx)); bind_idx += 1; }
    let where_clause = format!("WHERE {}", conditions.join(" AND "));

    let data_sql = format!("{} {} ORDER BY bu.expiry_date ASC LIMIT 100", SELECT_UNITS, where_clause);
    let mut data_q = sqlx::query_as::<_, BloodUnit>(&data_sql);
    if let Some(b) = bg { data_q = data_q.bind(b); }
    if let Some(r) = rh { data_q = data_q.bind(r); }
    if let Some(c) = ct { data_q = data_q.bind(c); }
    let units: Vec<BloodUnit> = data_q
        .fetch_all(pool.inner())
        .await
        .map_err(|e| sanitize_db_error(&e))?;
    Ok(units)
}

/// Get a single blood unit by id. RBAC: `BloodBankView`.
#[tauri::command]
pub async fn get_blood_unit(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    id: i32,
) -> Result<BloodUnit, String> {
    let _ = rbac::require(&session, Permission::BloodBankView)?;
    let q = format!("{} WHERE bu.id = $1 AND bu.deleted_at IS NULL", SELECT_UNITS);
    sqlx::query_as::<_, BloodUnit>(&q)
        .bind(id)
        .fetch_one(pool.inner())
        .await
        .map_err(|e| sanitize_db_error(&e))
}

/// Manually create a blood unit (for component separation from an existing
/// donation, or receipt from an external source). RBAC: `BloodBankManage`.
/// Audit-logged.
#[tauri::command]
pub async fn create_blood_unit(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    unit: CreateBloodUnit,
) -> Result<i32, String> {
    let s = rbac::require(&session, Permission::BloodBankManage)?;

    let blood_group = unit.blood_group.trim();
    let rh_factor = unit.rh_factor.trim();
    let component_type = unit.component_type.trim();
    validate_enum(blood_group, VALID_BLOOD_GROUPS, "blood_group")?;
    validate_enum(rh_factor, VALID_RH_FACTORS, "rh_factor")?;
    validate_enum(component_type, VALID_COMPONENT_TYPES, "component_type")?;

    if unit.volume_ml <= 0 {
        return Err("volume_ml must be greater than 0.".to_string());
    }

    let expiry: chrono::DateTime<chrono::Utc> = chrono::DateTime::parse_from_rfc3339(unit.expiry_date.trim())
        .map_err(|e| format!("Invalid expiry_date '{}': {}. Use ISO 8601 / RFC 3339.", unit.expiry_date, e))?
        .with_timezone(&chrono::Utc);

    let mut tx = pool.begin().await.map_err(|e| sanitize_db_error(&e))?;

    // Verify donor exists.
    let donor_exists: Option<(i32,)> = sqlx::query_as(
        "SELECT id FROM blood_donors WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(unit.donor_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    if donor_exists.is_none() {
        return Err(format!("Blood donor {} not found or deleted.", unit.donor_id));
    }

    let unit_number_row: (String,) = sqlx::query_as(
        "SELECT 'BU-' || TO_CHAR(NOW(),'YYYYMMDD') || '-' || \
         LPAD(nextval('blood_unit_seq')::text, 6, '0')",
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;
    let unit_number = unit_number_row.0;

    let row: (i32,) = sqlx::query_as(
        r#"INSERT INTO blood_units
              (unit_number, donation_id, donor_id, component_type, blood_group, rh_factor,
               volume_ml, collection_date, expiry_date, storage_temperature,
               storage_location, status, created_by_user_id)
           VALUES ($1,$2,$3,$4,$5,$6,NOW(),$7,$8,$9,'Blood Bank Storage','available',$10)
           RETURNING id"#,
    )
    .bind(&unit_number)
    .bind(unit.donation_id)
    .bind(unit.donor_id)
    .bind(component_type)
    .bind(blood_group)
    .bind(rh_factor)
    .bind(unit.volume_ml)
    .bind(expiry)
    .bind(unit.storage_temperature.as_deref().unwrap_or("2-6°C"))
    .bind(s.user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;
    let unit_id = row.0;

    record_unit_event(
        &mut tx, unit_id, "available", s.user_id,
        Some("Unit created manually"), None, None,
    ).await?;
    record_movement(
        &mut tx, unit_id, "received", None, Some("Blood Bank Storage"),
        s.user_id, Some("Manual unit creation"), None, None,
    ).await?;

    tx.commit().await.map_err(|e| sanitize_db_error(&e))?;

    audit::for_session(
        pool.inner(),
        &s,
        "blood_unit_create",
        "blood_units",
        Some(&unit_id.to_string()),
        Some(serde_json::json!({
            "unit_number": unit_number,
            "component_type": component_type,
            "blood_group": blood_group,
            "rh_factor": rh_factor,
        })),
    )
    .await;
    Ok(unit_id)
}

/// Update a blood unit's status with state-machine validation.
/// RBAC: `BloodBankManage`. Audit-logged.
#[tauri::command]
pub async fn update_blood_unit_status(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    id: i32,
    status: String,
    notes: Option<String>,
) -> Result<(), String> {
    let s = rbac::require(&session, Permission::BloodBankManage)?;
    let new_status = status.trim();
    validate_enum(new_status, VALID_UNIT_STATUSES, "status")?;

    let mut tx = pool.begin().await.map_err(|e| sanitize_db_error(&e))?;

    let current_row: Option<(String,)> = sqlx::query_as(
        "SELECT status FROM blood_units WHERE id = $1 AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    let current_status = current_row
        .ok_or_else(|| format!("Blood unit {} not found or deleted.", id))?
        .0;

    if !is_valid_unit_transition(&current_status, new_status) {
        return Err(format!(
            "Invalid status transition: '{}' → '{}'. \
             Allowed transitions from '{}' are: {}.",
            current_status, new_status, current_status,
            allowed_unit_transitions_from(&current_status)
        ));
    }

    let stamp_clause = match new_status {
        "transfused" => ", transfused_at = COALESCE(transfused_at, NOW())",
        "discarded" => ", discarded_at = COALESCE(discarded_at, NOW())",
        _ => "",
    };
    let update_sql = format!(
        "UPDATE blood_units SET status = $1{}, updated_at = NOW() WHERE id = $2",
        stamp_clause
    );
    sqlx::query(&update_sql)
        .bind(new_status)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| sanitize_db_error(&e))?;

    record_unit_event(&mut tx, id, new_status, s.user_id, notes.as_deref(), None, None).await?;

    tx.commit().await.map_err(|e| sanitize_db_error(&e))?;

    audit::for_session(
        pool.inner(),
        &s,
        "blood_unit_status_update",
        "blood_units",
        Some(&id.to_string()),
        Some(serde_json::json!({
            "old_status": current_status,
            "new_status": new_status,
            "notes": notes,
        })),
    )
    .await;
    Ok(())
}

/// Soft-delete a blood unit. RBAC: `BloodBankManage`. Audit-logged.
/// Refuses if the unit is in a non-terminal active state (issued/reserved).
#[tauri::command]
pub async fn delete_blood_unit(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    id: i32,
    reason: Option<String>,
) -> Result<(), String> {
    let s = rbac::require(&session, Permission::BloodBankManage)?;

    let mut tx = pool.begin().await.map_err(|e| sanitize_db_error(&e))?;

    let current: Option<(String,)> = sqlx::query_as(
        "SELECT status FROM blood_units WHERE id = $1 AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    let status = current
        .ok_or_else(|| format!("Blood unit {} not found or already deleted.", id))?
        .0;

    if status == "reserved" || status == "issued" {
        return Err(format!(
            "Cannot delete unit {} in '{}' status. Release the reservation or \
             return the issue first.",
            id, status
        ));
    }

    sqlx::query(
        r#"UPDATE blood_units
           SET deleted_at = NOW(), deleted_by_user_id = $1, deleted_reason = $2,
               updated_at = NOW()
           WHERE id = $3 AND deleted_at IS NULL"#,
    )
    .bind(s.user_id)
    .bind(reason.as_deref())
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    tx.commit().await.map_err(|e| sanitize_db_error(&e))?;

    audit::for_session(
        pool.inner(),
        &s,
        "blood_unit_delete",
        "blood_units",
        Some(&id.to_string()),
        Some(serde_json::json!({ "soft_delete": true, "reason": reason })),
    )
    .await;
    Ok(())
}

// ════════════════════════════════════════════════════════════════════════════
// FR-0147 — CROSS-MATCHING & RESERVATIONS
// ════════════════════════════════════════════════════════════════════════════

/// List cross-match results. RBAC: `BloodBankView`.
#[tauri::command]
#[allow(unused_assignments)] // bind_idx increments after the last condition are harmless
pub async fn get_blood_crossmatches(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    patient_id: Option<i32>,
    unit_id: Option<i32>,
    result_filter: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<serde_json::Value, String> {
    let _ = rbac::require(&session, Permission::BloodBankView)?;

    let pg = page.unwrap_or(1).max(1);
    let ps = page_size.unwrap_or(10).clamp(1, 100);
    let offset = (pg - 1) * ps;
    let rf = result_filter.as_deref().filter(|s| !s.is_empty());

    let mut conditions: Vec<String> = vec![];
    let mut bind_idx = 1;
    if patient_id.is_some() {
        conditions.push(format!("bc.patient_id = ${}", bind_idx));
        bind_idx += 1;
    }
    if unit_id.is_some() {
        conditions.push(format!("bc.unit_id = ${}", bind_idx));
        bind_idx += 1;
    }
    if rf.is_some() {
        conditions.push(format!("bc.result = ${}", bind_idx));
        bind_idx += 1;
    }
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let count_sql = format!("SELECT COUNT(*) FROM blood_crossmatch_results bc {}", where_clause);
    let mut count_q = sqlx::query_scalar::<_, i64>(&count_sql);
    if let Some(p) = patient_id { count_q = count_q.bind(p); }
    if let Some(u) = unit_id { count_q = count_q.bind(u); }
    if let Some(r) = rf { count_q = count_q.bind(r); }
    let total: i64 = count_q.fetch_one(pool.inner()).await.map_err(|e| sanitize_db_error(&e))?;

    let data_sql = format!(
        "{} {} ORDER BY bc.crossmatch_date DESC LIMIT ${} OFFSET ${}",
        SELECT_CROSSMATCHES, where_clause, bind_idx, bind_idx + 1
    );
    let mut data_q = sqlx::query_as::<_, BloodCrossmatch>(&data_sql);
    if let Some(p) = patient_id { data_q = data_q.bind(p); }
    if let Some(u) = unit_id { data_q = data_q.bind(u); }
    if let Some(r) = rf { data_q = data_q.bind(r); }
    data_q = data_q.bind(ps).bind(offset);
    let rows: Vec<BloodCrossmatch> = data_q
        .fetch_all(pool.inner())
        .await
        .map_err(|e| sanitize_db_error(&e))?;

    Ok(serde_json::json!({
        "crossmatches": rows,
        "total": total,
        "page": pg,
        "page_size": ps,
        "total_pages": ((total + ps - 1) / ps).max(1),
    }))
}

/// Check ABO/Rh compatibility between a donor unit and a recipient patient
/// using the seeded compatibility matrix. Returns a boolean.
/// RBAC: `BloodBankView`.
#[tauri::command]
pub async fn check_blood_compatibility(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    unit_id: i32,
    patient_id: i32,
) -> Result<serde_json::Value, String> {
    let _ = rbac::require(&session, Permission::BloodBankView)?;

    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT bu.blood_group, bu.rh_factor FROM blood_units bu \
         WHERE bu.id = $1 AND bu.deleted_at IS NULL",
    )
    .bind(unit_id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    let (donor_group, donor_rh) = row
        .ok_or_else(|| format!("Blood unit {} not found or deleted.", unit_id))?;

    // Patient blood group — stored in patients.blood_group (if present).
    let patient_row: Option<(Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT blood_group, rh_factor FROM patients WHERE id = $1",
    )
    .bind(patient_id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    let (patient_group_opt, patient_rh_opt) = patient_row
        .ok_or_else(|| format!("Patient {} not found.", patient_id))?;

    let patient_group = patient_group_opt.unwrap_or_else(|| "".to_string());
    let patient_rh = patient_rh_opt.unwrap_or_else(|| "".to_string());

    // BE-01: fail-closed if patient ABO or Rh is not recorded. Previously only
    // blood_group was checked; a missing rh_factor would silently fall through
    // to the matrix lookup, which returns no row → `false`. Now we surface a
    // clear clinical message so the operator knows exactly what to record.
    if patient_group.is_empty() || patient_rh.is_empty() {
        return Ok(serde_json::json!({
            "compatible": false,
            "reason": "Patient blood type is not fully recorded (ABO and Rh both required). Record it before cross-matching.",
            "donor_group": donor_group,
            "donor_rh": donor_rh,
            "patient_group": patient_group,
            "patient_rh": patient_rh,
        }));
    }

    let compatible: bool = sqlx::query_scalar(
        "SELECT compatible FROM blood_compatibility_matrix \
         WHERE recipient_group = $1 AND recipient_rh = $2 \
         AND donor_group = $3 AND donor_rh = $4",
    )
    .bind(&patient_group)
    .bind(&patient_rh)
    .bind(&donor_group)
    .bind(&donor_rh)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| sanitize_db_error(&e))?
    .unwrap_or(false);

    Ok(serde_json::json!({
        "compatible": compatible,
        "donor_group": donor_group,
        "donor_rh": donor_rh,
        "patient_group": patient_group,
        "patient_rh": patient_rh,
        "reason": if compatible {
            "ABO/Rh compatible".to_string()
        } else {
            "ABO/Rh INCOMPATIBLE — do not transfuse".to_string()
        },
    }))
}

/// Record a cross-match test result. RBAC: `BloodBankCrossmatch`. Audit-logged.
/// Validates the result + method enums and checks that the unit is available.
#[tauri::command]
pub async fn create_blood_crossmatch(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    crossmatch: CreateBloodCrossmatch,
) -> Result<i32, String> {
    let s = rbac::require(&session, Permission::BloodBankCrossmatch)?;

    let result = crossmatch.result.trim();
    validate_enum(result, VALID_CROSSMATCH_RESULTS, "result")?;
    if let Some(m) = crossmatch.method.as_deref() {
        if !m.is_empty() {
            validate_enum(m, VALID_CROSSMATCH_METHODS, "method")?;
        }
    }

    let mut tx = pool.begin().await.map_err(|e| sanitize_db_error(&e))?;

    // Verify the unit exists, is not deleted, and is in a crossmatchable state.
    let unit_row: Option<(String,)> = sqlx::query_as(
        "SELECT status FROM blood_units WHERE id = $1 AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(crossmatch.unit_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    let unit_status = unit_row
        .ok_or_else(|| format!("Blood unit {} not found or deleted.", crossmatch.unit_id))?
        .0;

    if unit_status == "transfused" || unit_status == "discarded" || unit_status == "expired" {
        return Err(format!(
            "Cannot cross-match unit {}: status is '{}' (terminal).",
            crossmatch.unit_id, unit_status
        ));
    }

    let row: (i32,) = sqlx::query_as(
        r#"INSERT INTO blood_crossmatch_results
              (unit_id, patient_id, doctor_id, requested_by_user_id, method, result,
               reaction_grade, incubation_time_min, ahg_phase, notes, performed_by_user_id)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) RETURNING id"#,
    )
    .bind(crossmatch.unit_id)
    .bind(crossmatch.patient_id)
    .bind(crossmatch.doctor_id)
    .bind(s.user_id)
    .bind(crossmatch.method.as_deref().unwrap_or("saline_37c"))
    .bind(result)
    .bind(crossmatch.reaction_grade)
    .bind(crossmatch.incubation_time_min)
    .bind(crossmatch.ahg_phase.as_deref())
    .bind(crossmatch.notes.as_deref())
    .bind(s.user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    tx.commit().await.map_err(|e| sanitize_db_error(&e))?;

    audit::for_session(
        pool.inner(),
        &s,
        "blood_crossmatch_create",
        "blood_crossmatch_results",
        Some(&row.0.to_string()),
        Some(serde_json::json!({
            "unit_id": crossmatch.unit_id,
            "patient_id": crossmatch.patient_id,
            "result": result,
        })),
    )
    .await;
    Ok(row.0)
}

/// Verify a cross-match result (second technologist confirmation).
/// RBAC: `BloodBankVerify`. Audit-logged.
#[tauri::command]
pub async fn verify_blood_crossmatch(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    crossmatch_id: i32,
) -> Result<(), String> {
    let s = rbac::require(&session, Permission::BloodBankVerify)?;

    let result = sqlx::query(
        r#"UPDATE blood_crossmatch_results
           SET verified_at = COALESCE(verified_at, NOW()),
               verified_by_user_id = $1,
               updated_at = NOW()
           WHERE id = $2"#,
    )
    .bind(s.user_id)
    .bind(crossmatch_id)
    .execute(pool.inner())
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    if result.rows_affected() == 0 {
        return Err(format!("Cross-match {} not found.", crossmatch_id));
    }

    audit::for_session(
        pool.inner(),
        &s,
        "blood_crossmatch_verify",
        "blood_crossmatch_results",
        Some(&crossmatch_id.to_string()),
        Some(serde_json::json!({ "verifier_user_id": s.user_id })),
    )
    .await;
    Ok(())
}

/// Create a reservation (holds a unit for a patient). RBAC: `BloodBankCrossmatch`.
/// Audit-logged. Moves the unit to 'reserved' status and stamps the
/// reserved_for_patient_id + reservation_id.
#[tauri::command]
pub async fn create_blood_reservation(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    reservation: CreateBloodReservation,
) -> Result<i32, String> {
    let s = rbac::require(&session, Permission::BloodBankCrossmatch)?;

    let priority = reservation.priority.trim();
    validate_enum(priority, VALID_PRIORITIES, "priority")?;

    if reservation.expires_in_hours <= 0 || reservation.expires_in_hours > 168 {
        return Err(format!(
            "expires_in_hours must be between 1 and 168 (7 days). Got {}.",
            reservation.expires_in_hours
        ));
    }

    let mut tx = pool.begin().await.map_err(|e| sanitize_db_error(&e))?;

    // Atomically claim the unit: only available units can be reserved.
    let claim: Option<(i32,)> = sqlx::query_as(
        r#"UPDATE blood_units
           SET status = 'reserved',
               reserved_for_patient_id = $1,
               updated_at = NOW()
           WHERE id = $2 AND status = 'available' AND deleted_at IS NULL
           RETURNING id"#,
    )
    .bind(reservation.patient_id)
    .bind(reservation.unit_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    if claim.is_none() {
        let cur: Option<(String,)> = sqlx::query_as(
            "SELECT status FROM blood_units WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(reservation.unit_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| sanitize_db_error(&e))?;
        return Err(match cur {
            None => format!("Blood unit {} not found or deleted.", reservation.unit_id),
            Some((status,)) => format!(
                "Blood unit {} is not available (status: '{}'). Only 'available' units can be reserved.",
                reservation.unit_id, status
            ),
        });
    }

    let res_number_row: (String,) = sqlx::query_as(
        "SELECT 'BRS-' || TO_CHAR(NOW(),'YYYYMMDD') || '-' || \
         LPAD(nextval('blood_reservation_seq')::text, 6, '0')",
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;
    let reservation_number = res_number_row.0;

    let row: (i32,) = sqlx::query_as(
        r#"INSERT INTO blood_reservations
              (reservation_number, unit_id, patient_id, doctor_id, requested_by_user_id,
               crossmatch_id, expires_at, status, priority, clinical_indication, notes)
           VALUES ($1,$2,$3,$4,$5,$6,NOW() + INTERVAL '1 hour' * $7,'active',$8,$9,$10)
           RETURNING id"#,
    )
    .bind(&reservation_number)
    .bind(reservation.unit_id)
    .bind(reservation.patient_id)
    .bind(reservation.doctor_id)
    .bind(s.user_id)
    .bind(reservation.crossmatch_id)
    .bind(reservation.expires_in_hours)
    .bind(priority)
    .bind(reservation.clinical_indication.as_deref())
    .bind(reservation.notes.as_deref())
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;
    let reservation_id = row.0;

    // Link the reservation_id back onto the unit.
    sqlx::query("UPDATE blood_units SET reservation_id = $1 WHERE id = $2")
        .bind(reservation_id)
        .bind(reservation.unit_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| sanitize_db_error(&e))?;

    record_unit_event(
        &mut tx, reservation.unit_id, "reserved", s.user_id,
        Some(&format!("Reserved for patient {}", reservation.patient_id)),
        Some("reservation"), Some(reservation_id),
    ).await?;

    tx.commit().await.map_err(|e| sanitize_db_error(&e))?;

    audit::for_session(
        pool.inner(),
        &s,
        "blood_reservation_create",
        "blood_reservations",
        Some(&reservation_id.to_string()),
        Some(serde_json::json!({
            "reservation_number": reservation_number,
            "unit_id": reservation.unit_id,
            "patient_id": reservation.patient_id,
            "priority": priority,
        })),
    )
    .await;
    Ok(reservation_id)
}

/// Cancel a reservation (releases the unit back to 'available').
/// RBAC: `BloodBankCrossmatch`. Audit-logged.
#[tauri::command]
pub async fn cancel_blood_reservation(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    reservation_id: i32,
    reason: Option<String>,
) -> Result<(), String> {
    let s = rbac::require(&session, Permission::BloodBankCrossmatch)?;

    let mut tx = pool.begin().await.map_err(|e| sanitize_db_error(&e))?;

    let res_row: Option<(i32, String)> = sqlx::query_as(
        "SELECT unit_id, status FROM blood_reservations WHERE id = $1 FOR UPDATE",
    )
    .bind(reservation_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    let (unit_id, res_status) = res_row
        .ok_or_else(|| format!("Reservation {} not found.", reservation_id))?;

    if res_status != "active" {
        return Err(format!(
            "Reservation {} is not active (status: '{}'). Only active reservations can be cancelled.",
            reservation_id, res_status
        ));
    }

    sqlx::query(
        r#"UPDATE blood_reservations
           SET status = 'cancelled', cancelled_at = NOW(), notes = COALESCE($1, notes),
               updated_at = NOW()
           WHERE id = $2"#,
    )
    .bind(reason.as_deref())
    .bind(reservation_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    // Release the unit back to available.
    sqlx::query(
        r#"UPDATE blood_units
           SET status = 'available', reserved_for_patient_id = NULL, reservation_id = NULL,
               updated_at = NOW()
           WHERE id = $1"#,
    )
    .bind(unit_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    record_unit_event(
        &mut tx, unit_id, "available", s.user_id,
        Some(&format!("Reservation {} cancelled", reservation_id)),
        Some("reservation"), Some(reservation_id),
    ).await?;

    tx.commit().await.map_err(|e| sanitize_db_error(&e))?;

    audit::for_session(
        pool.inner(),
        &s,
        "blood_reservation_cancel",
        "blood_reservations",
        Some(&reservation_id.to_string()),
        Some(serde_json::json!({ "reason": reason, "unit_id": unit_id })),
    )
    .await;
    Ok(())
}

// ════════════════════════════════════════════════════════════════════════════
// FR-0148 — BLOOD ISSUE & TRANSFUSION
// ════════════════════════════════════════════════════════════════════════════

/// List blood issues with pagination. RBAC: `BloodBankView`.
#[tauri::command]
#[allow(unused_assignments)] // bind_idx increments after the last condition are harmless
pub async fn get_blood_issues(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    patient_id: Option<i32>,
    issue_type_filter: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<serde_json::Value, String> {
    let _ = rbac::require(&session, Permission::BloodBankView)?;

    let pg = page.unwrap_or(1).max(1);
    let ps = page_size.unwrap_or(10).clamp(1, 100);
    let offset = (pg - 1) * ps;
    let it = issue_type_filter.as_deref().filter(|s| !s.is_empty());

    let mut conditions: Vec<String> = vec![];
    let mut bind_idx = 1;
    if patient_id.is_some() {
        conditions.push(format!("bi.patient_id = ${}", bind_idx));
        bind_idx += 1;
    }
    if it.is_some() {
        conditions.push(format!("bi.issue_type = ${}", bind_idx));
        bind_idx += 1;
    }
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let count_sql = format!("SELECT COUNT(*) FROM blood_issues bi {}", where_clause);
    let mut count_q = sqlx::query_scalar::<_, i64>(&count_sql);
    if let Some(p) = patient_id { count_q = count_q.bind(p); }
    if let Some(i) = it { count_q = count_q.bind(i); }
    let total: i64 = count_q.fetch_one(pool.inner()).await.map_err(|e| sanitize_db_error(&e))?;

    let data_sql = format!(
        "{} {} ORDER BY bi.issued_at DESC LIMIT ${} OFFSET ${}",
        SELECT_ISSUES, where_clause, bind_idx, bind_idx + 1
    );
    let mut data_q = sqlx::query_as::<_, BloodIssue>(&data_sql);
    if let Some(p) = patient_id { data_q = data_q.bind(p); }
    if let Some(i) = it { data_q = data_q.bind(i); }
    data_q = data_q.bind(ps).bind(offset);
    let rows: Vec<BloodIssue> = data_q
        .fetch_all(pool.inner())
        .await
        .map_err(|e| sanitize_db_error(&e))?;

    Ok(serde_json::json!({
        "issues": rows,
        "total": total,
        "page": pg,
        "page_size": ps,
        "total_pages": ((total + ps - 1) / ps).max(1),
    }))
}

/// Issue blood from the bank to a patient/ward. RBAC: `BloodBankIssue`.
/// Audit-logged. Moves the unit to 'issued' status, fulfils the reservation
/// (if linked), and records the inventory movement.
#[tauri::command]
pub async fn issue_blood(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    issue: CreateBloodIssue,
) -> Result<i32, String> {
    let s = rbac::require(&session, Permission::BloodBankIssue)?;

    let issue_type = issue.issue_type.trim();
    validate_enum(issue_type, VALID_ISSUE_TYPES, "issue_type")?;

    let mut tx = pool.begin().await.map_err(|e| sanitize_db_error(&e))?;

    // BE-02 + BE-03: Pre-condition checks before claiming the unit.
    //
    // We verify expiry + screening in a single SELECT...FOR UPDATE so the
    // clinical error is specific (the atomic UPDATE...RETURNING below would
    // only return a generic "cannot be issued" if we folded these into the
    // claim predicate). Doing it as a pre-check gives the operator a clear
    // reason ("expired", "screening pending") rather than a bare status error.
    let precheck: Option<(String, Option<chrono::DateTime<chrono::Utc>>, Option<i32>)> = sqlx::query_as(
        r#"SELECT bu.status, bu.expiry_date, bu.donation_id
           FROM blood_units bu
           WHERE bu.id = $1 AND bu.deleted_at IS NULL
           FOR UPDATE"#,
    )
    .bind(issue.unit_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    let (unit_status, unit_expiry, unit_donation_id) = precheck
        .ok_or_else(|| format!("Blood unit {} not found or deleted.", issue.unit_id))?;

    // BE-02: Reject expired units.
    if unit_expiry.map(|e| e <= chrono::Utc::now()).unwrap_or(true) {
        return Err(format!(
            "Blood unit {} cannot be issued: it has expired (expiry date: {}). \
             Expired blood must be discarded, never transfused.",
            issue.unit_id,
            unit_expiry.map(|e| e.format("%Y-%m-%d %H:%M").to_string()).unwrap_or_else(|| "unknown".to_string())
        ));
    }

    // Status check (available or reserved for this patient).
    if unit_status != "available" && unit_status != "reserved" {
        return Err(format!(
            "Blood unit {} cannot be issued: status is '{}' (must be 'available' or 'reserved').",
            issue.unit_id, unit_status
        ));
    }

    // BE-03: Verify the linked donation passed screening. Units without a
    // donation_id (manually created) are exempt — they are assumed to have
    // been screened by the blood-bank technician at creation time, and the
    // BloodBankManage permission required to create them is the control.
    if let Some(did) = unit_donation_id {
        let screening: Option<(String,)> = sqlx::query_as(
            "SELECT screening_status FROM blood_donations WHERE id = $1",
        )
        .bind(did)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| sanitize_db_error(&e))?;

        let ss = screening
            .ok_or_else(|| format!("Blood unit {} references donation {} which does not exist.", issue.unit_id, did))?
            .0;

        if ss != "passed" {
            return Err(format!(
                "Blood unit {} cannot be issued: the linked donation {} has screening status \
                 '{}' (must be 'passed'). Unscreened, pending, failed, or quarantined blood \
                 must not be issued.",
                issue.unit_id, did, ss
            ));
        }
    }

    // BE-11: If the unit is reserved, verify the linked reservation has not
    // expired. A reservation's expires_at is typically much sooner than the
    // unit's expiry_date (e.g. 24h hold vs 35-day unit). An expired reservation
    // should be cancelled (releasing the unit) rather than fulfilled. We check
    // the reservation's expires_at AND its status (must be 'active').
    //
    // This check is defensive: the claim query below only succeeds for units
    // with status 'available' or 'reserved', and an expired reservation's unit
    // is still 'reserved' (reservations don't auto-expire in the current
    // scheduler — that's a future enhancement). Without this check, an operator
    // could fulfil a reservation that expired hours ago, defeating the hold's
    // time-limited purpose.
    let reservation_row: Option<(chrono::DateTime<chrono::Utc>, String)> = sqlx::query_as(
        "SELECT expires_at, status FROM blood_reservations \
         WHERE unit_id = $1 AND status = 'active' \
         ORDER BY reserved_at DESC LIMIT 1",
    )
    .bind(issue.unit_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    if let Some((expires_at, res_status)) = reservation_row {
        if expires_at <= chrono::Utc::now() {
            return Err(format!(
                "Blood unit {} cannot be issued: the active reservation expired at {}. \
                 An expired reservation must be cancelled (releasing the unit back to \
                 available inventory) before the unit can be issued. Call \
                 cancel_blood_reservation first, then issue_blood.",
                issue.unit_id,
                expires_at.format("%Y-%m-%d %H:%M")
            ));
        }
        let _ = res_status; // status is 'active' (query filtered)
    }

    // BE-04: Server-side ABO/Rh compatibility enforcement.
    //
    // For 'routine' issues: the patient's recorded ABO/Rh MUST be compatible
    //   with the unit's ABO/Rh per the ISBT matrix. If the patient's blood
    //   type is not recorded, reject (fail-closed).
    // For 'emergency' / 'uncrossmatched' issues: ABO/Rh check is bypassed
    //   ONLY if a non-empty clinical_indication is provided as documented
    //   override. This is the clinical reality — in a massive hemorrhage the
    //   doctor may issue O- uncrossmatched before the lab finishes typing.
    //   The override is audit-logged.
    // For 'autologous' issues: the donor is the patient themselves, so ABO
    //   is inherently compatible — skip the matrix check.
    if issue_type != "autologous" {
        let unit_bt: (String, String) = sqlx::query_as(
            "SELECT blood_group, rh_factor FROM blood_units WHERE id = $1",
        )
        .bind(issue.unit_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| sanitize_db_error(&e))?;

        let patient_bt: Option<(Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT blood_group, rh_factor FROM patients WHERE id = $1",
        )
        .bind(issue.patient_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| sanitize_db_error(&e))?;

        let (p_group_opt, p_rh_opt) = patient_bt
            .ok_or_else(|| format!("Patient {} not found.", issue.patient_id))?;
        let p_group = p_group_opt.unwrap_or_default();
        let p_rh = p_rh_opt.unwrap_or_default();

        let compatible = if p_group.is_empty() || p_rh.is_empty() {
            false // cannot verify — fail-closed
        } else {
            sqlx::query_scalar::<_, bool>(
                "SELECT compatible FROM blood_compatibility_matrix \
                 WHERE recipient_group = $1 AND recipient_rh = $2 \
                 AND donor_group = $3 AND donor_rh = $4",
            )
            .bind(&p_group)
            .bind(&p_rh)
            .bind(&unit_bt.0)
            .bind(&unit_bt.1)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| sanitize_db_error(&e))?
            .unwrap_or(false)
        };

        if !compatible {
            // Determine if an emergency override is permitted.
            let is_emergency_override = issue_type == "emergency" || issue_type == "uncrossmatched";
            let has_indication = issue.clinical_indication.as_deref().map(|s| !s.trim().is_empty()).unwrap_or(false);

            if !is_emergency_override {
                let reason = if p_group.is_empty() || p_rh.is_empty() {
                    format!(
                        "Patient {} blood type is not fully recorded (ABO and Rh both required). \
                         Record it before issuing blood.",
                        issue.patient_id
                    )
                } else {
                    format!(
                        "ABO/Rh INCOMPATIBLE: unit {} is {}{} but patient {} is {}{}. \
                         Incompatible transfusion can cause acute hemolytic reaction (fatal). \
                         Use issue_type='emergency' with a clinical_indication only for \
                         life-threatening massive hemorrhage where typing is not yet available.",
                        issue.unit_id, unit_bt.0, unit_bt.1,
                        issue.patient_id, p_group, p_rh
                    )
                };
                return Err(reason);
            }

            if !has_indication {
                return Err(format!(
                    "Emergency issue of potentially incompatible blood requires a non-empty \
                     clinical_indication documenting the life-threatening reason (e.g. 'massive \
                     hemorrhage, O- not available'). Unit {} is {}{}, patient {} is {}{}.",
                    issue.unit_id, unit_bt.0, unit_bt.1,
                    issue.patient_id, if p_group.is_empty() { "untyped".to_string() } else { p_group.clone() },
                    if p_rh.is_empty() { "".to_string() } else { p_rh.clone() }
                ));
            }
            // Emergency override with documented indication — proceed.
            // The audit log (below) records the override.
        }
    }

    // Atomically claim the unit for issue: must be 'available' or 'reserved'
    // for this patient. By this point expiry + screening + compatibility are
    // already verified, so the claim is a pure state transition.
    let claim: Option<(Option<i32>,)> = sqlx::query_as(
        r#"UPDATE blood_units
           SET status = 'issued',
               issued_to_patient_id = $1,
               issued_at = NOW(),
               updated_at = NOW()
           WHERE id = $2
             AND status IN ('available', 'reserved')
             AND deleted_at IS NULL
             AND (reserved_for_patient_id IS NULL OR reserved_for_patient_id = $1)
           RETURNING reservation_id"#,
    )
    .bind(issue.patient_id)
    .bind(issue.unit_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    let linked_reservation_id = match claim {
        None => {
            // The pre-checks passed but the claim failed — this means the
            // unit's status changed between the pre-check and the claim (a
            // concurrent issue). Return a clear concurrency error.
            return Err(format!(
                "Blood unit {} could not be claimed — it may have been issued or reserved by \
                 another user concurrently. Please refresh and try again.",
                issue.unit_id
            ));
        }
        Some((res_id,)) => res_id.or(issue.reservation_id),
    };

    let issue_number_row: (String,) = sqlx::query_as(
        "SELECT 'BIS-' || TO_CHAR(NOW(),'YYYYMMDD') || '-' || \
         LPAD(nextval('blood_issue_seq')::text, 6, '0')",
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;
    let issue_number = issue_number_row.0;

    let row: (i32,) = sqlx::query_as(
        r#"INSERT INTO blood_issues
              (issue_number, unit_id, patient_id, reservation_id, crossmatch_id, doctor_id,
               issued_by_user_id, issued_to_location, issue_type, clinical_indication,
               special_instructions)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) RETURNING id"#,
    )
    .bind(&issue_number)
    .bind(issue.unit_id)
    .bind(issue.patient_id)
    .bind(linked_reservation_id)
    .bind(issue.crossmatch_id)
    .bind(issue.doctor_id)
    .bind(s.user_id)
    .bind(issue.issued_to_location.as_deref())
    .bind(issue_type)
    .bind(issue.clinical_indication.as_deref())
    .bind(issue.special_instructions.as_deref())
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;
    let issue_id = row.0;

    // Fulfil the linked reservation (if any).
    if let Some(rid) = linked_reservation_id {
        sqlx::query(
            "UPDATE blood_reservations SET status = 'fulfilled', fulfilled_at = NOW(), \
             updated_at = NOW() WHERE id = $1",
        )
        .bind(rid)
        .execute(&mut *tx)
        .await
        .map_err(|e| sanitize_db_error(&e))?;
    }

    record_unit_event(
        &mut tx, issue.unit_id, "issued", s.user_id,
        Some(&format!("Issued to patient {} ({})", issue.patient_id, issue_type)),
        Some("issue"), Some(issue_id),
    ).await?;
    record_movement(
        &mut tx, issue.unit_id, "issued",
        Some("Blood Bank Storage"),
        issue.issued_to_location.as_deref(),
        s.user_id,
        Some(&format!("Issued ({})", issue_type)),
        Some("issue"), Some(issue_id),
    ).await?;

    tx.commit().await.map_err(|e| sanitize_db_error(&e))?;

    audit::for_session(
        pool.inner(),
        &s,
        "blood_issue",
        "blood_issues",
        Some(&issue_id.to_string()),
        Some(serde_json::json!({
            "issue_number": issue_number,
            "unit_id": issue.unit_id,
            "patient_id": issue.patient_id,
            "issue_type": issue_type,
        })),
    )
    .await;
    Ok(issue_id)
}

/// Receive/return blood back to the bank (unused return).
/// RBAC: `BloodBankIssue`. Audit-logged. Moves the unit back to 'available'.
#[tauri::command]
pub async fn return_blood_unit(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    issue_id: i32,
    reason: Option<String>,
) -> Result<(), String> {
    let s = rbac::require(&session, Permission::BloodBankIssue)?;

    let mut tx = pool.begin().await.map_err(|e| sanitize_db_error(&e))?;

    let issue_row: Option<(i32,)> = sqlx::query_as(
        "SELECT unit_id FROM blood_issues WHERE id = $1 FOR UPDATE",
    )
    .bind(issue_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    let unit_id = issue_row
        .ok_or_else(|| format!("Blood issue {} not found.", issue_id))?
        .0;

    // Verify the unit is still in 'issued' status.
    let unit_row: Option<(String,)> = sqlx::query_as(
        "SELECT status FROM blood_units WHERE id = $1 AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(unit_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    let status = unit_row
        .ok_or_else(|| format!("Blood unit {} not found or deleted.", unit_id))?
        .0;

    if status != "issued" {
        return Err(format!(
            "Cannot return unit {}: status is '{}' (expected 'issued').",
            unit_id, status
        ));
    }

    sqlx::query(
        r#"UPDATE blood_issues
           SET returned_at = NOW(), return_reason = $1, updated_at = NOW()
           WHERE id = $2"#,
    )
    .bind(reason.as_deref())
    .bind(issue_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    sqlx::query(
        r#"UPDATE blood_units
           SET status = 'available',
               issued_to_patient_id = NULL,
               issued_at = NULL,
               reserved_for_patient_id = NULL,
               reservation_id = NULL,
               updated_at = NOW()
           WHERE id = $1"#,
    )
    .bind(unit_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    record_unit_event(
        &mut tx, unit_id, "available", s.user_id,
        Some(&format!("Returned from issue {} ({})", issue_id, reason.as_deref().unwrap_or("no reason"))),
        Some("issue"), Some(issue_id),
    ).await?;
    record_movement(
        &mut tx, unit_id, "returned",
        Some("Ward"), Some("Blood Bank Storage"),
        s.user_id, reason.as_deref(),
        Some("issue"), Some(issue_id),
    ).await?;

    tx.commit().await.map_err(|e| sanitize_db_error(&e))?;

    audit::for_session(
        pool.inner(),
        &s,
        "blood_return",
        "blood_issues",
        Some(&issue_id.to_string()),
        Some(serde_json::json!({ "unit_id": unit_id, "reason": reason })),
    )
    .await;
    Ok(())
}

/// List transfusions. RBAC: `BloodBankView`.
#[tauri::command]
pub async fn get_blood_transfusions(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    patient_id: Option<i32>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<serde_json::Value, String> {
    let _ = rbac::require(&session, Permission::BloodBankView)?;

    let pg = page.unwrap_or(1).max(1);
    let ps = page_size.unwrap_or(10).clamp(1, 100);
    let offset = (pg - 1) * ps;

    let (where_clause, has_patient) = match patient_id {
        Some(_) => ("WHERE bt.patient_id = $1", true),
        None => ("", false),
    };

    let count_sql = format!("SELECT COUNT(*) FROM blood_transfusions bt {}", where_clause);
    let mut count_q = sqlx::query_scalar::<_, i64>(&count_sql);
    if has_patient { count_q = count_q.bind(patient_id.unwrap()); }
    let total: i64 = count_q.fetch_one(pool.inner()).await.map_err(|e| sanitize_db_error(&e))?;

    let bind_idx = if has_patient { 2 } else { 1 };
    let data_sql = format!(
        "{} {} ORDER BY bt.started_at DESC LIMIT ${} OFFSET ${}",
        SELECT_TRANSFUSIONS, where_clause, bind_idx, bind_idx + 1
    );
    let mut data_q = sqlx::query_as::<_, BloodTransfusion>(&data_sql);
    if has_patient { data_q = data_q.bind(patient_id.unwrap()); }
    data_q = data_q.bind(ps).bind(offset);
    let rows: Vec<BloodTransfusion> = data_q
        .fetch_all(pool.inner())
        .await
        .map_err(|e| sanitize_db_error(&e))?;

    Ok(serde_json::json!({
        "transfusions": rows,
        "total": total,
        "page": pg,
        "page_size": ps,
        "total_pages": ((total + ps - 1) / ps).max(1),
    }))
}

/// Record a transfusion event (the actual administration of blood).
/// RBAC: `BloodBankTransfuse`. Audit-logged.
/// Moves the unit to 'transfused' (terminal) and records the transfusion.
#[tauri::command]
pub async fn create_blood_transfusion(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    transfusion: CreateBloodTransfusion,
) -> Result<i32, String> {
    let s = rbac::require(&session, Permission::BloodBankTransfuse)?;

    if let Some(o) = transfusion.outcome.as_deref() {
        if !o.is_empty() {
            validate_enum(o, VALID_TRANSFUSION_OUTCOMES, "outcome")?;
        }
    }

    let outcome = if transfusion.outcome.as_deref().unwrap_or("completed").is_empty() {
        "completed"
    } else {
        transfusion.outcome.as_deref().unwrap_or("completed")
    };

    let mut tx = pool.begin().await.map_err(|e| sanitize_db_error(&e))?;

    // Verify the issue exists and the unit is in 'issued' status.
    // BE-07: Also fetch patient_id to verify the transfusion is being recorded
    // for the SAME patient the blood was issued to. Without this check, a
    // transfusion could be recorded against patient B under an issue record
    // for patient A — a wrong-patient transfusion never-event.
    let issue_row: Option<(i32, i32)> = sqlx::query_as(
        "SELECT unit_id, patient_id FROM blood_issues WHERE id = $1 FOR UPDATE",
    )
    .bind(transfusion.issue_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    let (issue_unit_id, issue_patient_id) = issue_row
        .ok_or_else(|| format!("Blood issue {} not found.", transfusion.issue_id))?;

    if issue_unit_id != transfusion.unit_id {
        return Err(format!(
            "Unit mismatch: issue {} is for unit {}, but transfusion references unit {}.",
            transfusion.issue_id, issue_unit_id, transfusion.unit_id
        ));
    }

    // BE-07: Patient identity check — the transfusion patient MUST match the
    // issue patient. This is a critical wrong-patient safety barrier.
    if issue_patient_id != transfusion.patient_id {
        return Err(format!(
            "Patient mismatch: blood issue {} was for patient {}, but the transfusion is being \
             recorded for patient {}. A transfusion cannot be recorded against a different \
             patient than the one the blood was issued to.",
            transfusion.issue_id, issue_patient_id, transfusion.patient_id
        ));
    }

    let unit_row: Option<(String,)> = sqlx::query_as(
        "SELECT status FROM blood_units WHERE id = $1 AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(transfusion.unit_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    let unit_status = unit_row
        .ok_or_else(|| format!("Blood unit {} not found or deleted.", transfusion.unit_id))?
        .0;

    // The unit must be 'issued' to be transfused. If outcome is 'cancelled'
    // (transfusion never started), we don't move the unit to transfused.
    if unit_status != "issued" && outcome != "cancelled" {
        return Err(format!(
            "Cannot transfuse unit {}: status is '{}' (expected 'issued').",
            transfusion.unit_id, unit_status
        ));
    }

    let transfusion_number_row: (String,) = sqlx::query_as(
        "SELECT 'BTR-' || TO_CHAR(NOW(),'YYYYMMDD') || '-' || \
         LPAD(nextval('blood_transfusion_seq')::text, 6, '0')",
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;
    let transfusion_number = transfusion_number_row.0;

    let row: (i32,) = sqlx::query_as(
        r#"INSERT INTO blood_transfusions
              (transfusion_number, issue_id, unit_id, patient_id, doctor_id, nurse_id,
               volume_transfused_ml, pre_transfusion_bp, post_transfusion_bp,
               pre_transfusion_temp, post_transfusion_temp, pre_transfusion_pulse,
               post_transfusion_pulse, reaction_observed, reaction_type, reaction_severity,
               reaction_notes, outcome, notes)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19)
           RETURNING id"#,
    )
    .bind(&transfusion_number)
    .bind(transfusion.issue_id)
    .bind(transfusion.unit_id)
    .bind(transfusion.patient_id)
    .bind(transfusion.doctor_id)
    .bind(transfusion.nurse_id)
    .bind(transfusion.volume_transfused_ml)
    .bind(transfusion.pre_transfusion_bp.as_deref())
    .bind(transfusion.post_transfusion_bp.as_deref())
    .bind(transfusion.pre_transfusion_temp.map(rust_decimal::Decimal::from_f64_retain))
    .bind(transfusion.post_transfusion_temp.map(rust_decimal::Decimal::from_f64_retain))
    .bind(transfusion.pre_transfusion_pulse)
    .bind(transfusion.post_transfusion_pulse)
    .bind(transfusion.reaction_observed)
    .bind(transfusion.reaction_type.as_deref())
    .bind(transfusion.reaction_severity.as_deref())
    .bind(transfusion.reaction_notes.as_deref())
    .bind(outcome)
    .bind(transfusion.notes.as_deref())
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;
    let transfusion_id = row.0;

    // If completed (not cancelled), move the unit to terminal 'transfused' status.
    if outcome == "completed" || outcome == "reaction" {
        sqlx::query(
            r#"UPDATE blood_units
               SET status = 'transfused',
                   transfused_at = NOW(),
                   transfused_to_patient_id = $1,
                   updated_at = NOW()
               WHERE id = $2"#,
        )
        .bind(transfusion.patient_id)
        .bind(transfusion.unit_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| sanitize_db_error(&e))?;

        record_unit_event(
            &mut tx, transfusion.unit_id, "transfused", s.user_id,
            Some(&format!("Transfused to patient {}", transfusion.patient_id)),
            Some("transfusion"), Some(transfusion_id),
        ).await?;
        record_movement(
            &mut tx, transfusion.unit_id, "transfused",
            Some("Blood Bank Storage"), Some("Patient"),
            s.user_id, Some("Transfused"),
            Some("transfusion"), Some(transfusion_id),
        ).await?;
    }

    tx.commit().await.map_err(|e| sanitize_db_error(&e))?;

    audit::for_session(
        pool.inner(),
        &s,
        "blood_transfusion",
        "blood_transfusions",
        Some(&transfusion_id.to_string()),
        Some(serde_json::json!({
            "transfusion_number": transfusion_number,
            "unit_id": transfusion.unit_id,
            "patient_id": transfusion.patient_id,
            "outcome": outcome,
            "reaction_observed": transfusion.reaction_observed,
        })),
    )
    .await;
    Ok(transfusion_id)
}

// ════════════════════════════════════════════════════════════════════════════
// FR-0149 — DISCARD & TRACEABILITY
// ════════════════════════════════════════════════════════════════════════════

/// Discard a blood unit. RBAC: `BloodBankDiscard`. Audit-logged.
/// Moves the unit to 'discarded' (terminal) and records the discard reason.
#[tauri::command]
pub async fn discard_blood_unit(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    discard: CreateBloodDiscard,
) -> Result<i32, String> {
    let s = rbac::require(&session, Permission::BloodBankDiscard)?;

    let reason = discard.discard_reason.trim();
    validate_enum(reason, VALID_DISCARD_REASONS, "discard_reason")?;

    let mut tx = pool.begin().await.map_err(|e| sanitize_db_error(&e))?;

    let unit_row: Option<(String,)> = sqlx::query_as(
        "SELECT status FROM blood_units WHERE id = $1 AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(discard.unit_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    let status = unit_row
        .ok_or_else(|| format!("Blood unit {} not found or deleted.", discard.unit_id))?
        .0;

    if status == "transfused" || status == "discarded" || status == "expired" {
        return Err(format!(
            "Cannot discard unit {}: status is '{}' (terminal).",
            discard.unit_id, status
        ));
    }

    let discard_number_row: (String,) = sqlx::query_as(
        "SELECT 'BDC-' || TO_CHAR(NOW(),'YYYYMMDD') || '-' || \
         LPAD(nextval('blood_discard_seq')::text, 6, '0')",
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;
    let discard_number = discard_number_row.0;

    let row: (i32,) = sqlx::query_as(
        r#"INSERT INTO blood_discards
              (unit_id, discard_number, discard_reason, discard_notes,
               discarded_by_user_id, disposal_method)
           VALUES ($1,$2,$3,$4,$5,$6) RETURNING id"#,
    )
    .bind(discard.unit_id)
    .bind(&discard_number)
    .bind(reason)
    .bind(discard.discard_notes.as_deref())
    .bind(s.user_id)
    .bind(discard.disposal_method.as_deref())
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;
    let discard_id = row.0;

    // Move unit to 'discarded' terminal status.
    sqlx::query(
        r#"UPDATE blood_units
           SET status = 'discarded', discarded_at = NOW(), discard_reason = $1,
               reserved_for_patient_id = NULL, reservation_id = NULL,
               updated_at = NOW()
           WHERE id = $2"#,
    )
    .bind(reason)
    .bind(discard.unit_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    record_unit_event(
        &mut tx, discard.unit_id, "discarded", s.user_id,
        Some(&format!("Discarded: {}", reason)),
        Some("discard"), Some(discard_id),
    ).await?;
    record_movement(
        &mut tx, discard.unit_id, "discarded",
        Some("Blood Bank Storage"), Some("Biohazard Disposal"),
        s.user_id, Some(&format!("Discarded: {}", reason)),
        Some("discard"), Some(discard_id),
    ).await?;

    tx.commit().await.map_err(|e| sanitize_db_error(&e))?;

    audit::for_session(
        pool.inner(),
        &s,
        "blood_discard",
        "blood_discards",
        Some(&discard_id.to_string()),
        Some(serde_json::json!({
            "discard_number": discard_number,
            "unit_id": discard.unit_id,
            "reason": reason,
        })),
    )
    .await;
    Ok(discard_id)
}

/// List discards. RBAC: `BloodBankView`.
#[tauri::command]
pub async fn get_blood_discards(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<serde_json::Value, String> {
    let _ = rbac::require(&session, Permission::BloodBankView)?;

    let pg = page.unwrap_or(1).max(1);
    let ps = page_size.unwrap_or(10).clamp(1, 100);
    let offset = (pg - 1) * ps;

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM blood_discards")
        .fetch_one(pool.inner())
        .await
        .map_err(|e| sanitize_db_error(&e))?;

    let data_sql = format!(
        "{} ORDER BY bdi.discarded_at DESC LIMIT $1 OFFSET $2",
        SELECT_DISCARDS
    );
    let rows: Vec<BloodDiscard> = sqlx::query_as::<_, BloodDiscard>(&data_sql)
        .bind(ps)
        .bind(offset)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| sanitize_db_error(&e))?;

    Ok(serde_json::json!({
        "discards": rows,
        "total": total,
        "page": pg,
        "page_size": ps,
        "total_pages": ((total + ps - 1) / ps).max(1),
    }))
}

/// Get the full status history for a blood unit (traceability — FR-0149).
/// RBAC: `BloodBankView`.
#[tauri::command]
pub async fn get_blood_unit_history(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    unit_id: i32,
) -> Result<Vec<BloodUnitHistory>, String> {
    let _ = rbac::require(&session, Permission::BloodBankView)?;

    let q = r#"SELECT h.id, h.unit_id, h.status, h.changed_by_user_id, h.changed_at,
                      h.notes, h.related_record_type, h.related_record_id,
                      u.username AS changed_by_name
               FROM blood_unit_status_history h
               LEFT JOIN users u ON u.id = h.changed_by_user_id
               WHERE h.unit_id = $1
               ORDER BY h.changed_at ASC"#;
    sqlx::query_as::<_, BloodUnitHistory>(q)
        .bind(unit_id)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| sanitize_db_error(&e))
}

/// Get the full inventory movement log for a blood unit (chain-of-custody).
/// RBAC: `BloodBankView`.
#[tauri::command]
pub async fn get_blood_unit_movements(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    unit_id: i32,
) -> Result<Vec<BloodMovement>, String> {
    let _ = rbac::require(&session, Permission::BloodBankView)?;

    let q = r#"SELECT m.id, m.unit_id, m.movement_type, m.from_location, m.to_location,
                      m.moved_by_user_id, m.moved_at, m.reason, m.related_record_type,
                      m.related_record_id, m.created_at,
                      bu.unit_number AS unit_number,
                      u.username AS moved_by_name
               FROM blood_inventory_movements m
               LEFT JOIN blood_units bu ON bu.id = m.unit_id
               LEFT JOIN users u ON u.id = m.moved_by_user_id
               WHERE m.unit_id = $1
               ORDER BY m.moved_at ASC"#;
    sqlx::query_as::<_, BloodMovement>(q)
        .bind(unit_id)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| sanitize_db_error(&e))
}

/// Get the full traceability trail for a unit — combines status history +
/// movements + crossmatches + issues + transfusion into a single timeline.
/// RBAC: `BloodBankView`. (FR-0149)
#[tauri::command]
pub async fn get_blood_unit_traceability(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    unit_id: i32,
) -> Result<serde_json::Value, String> {
    let _ = rbac::require(&session, Permission::BloodBankView)?;

    // Inline the history + movements queries (cannot call the
    // get_blood_unit_history / get_blood_unit_movements commands directly
    // because they expect tauri::State params, not &PgPool / &State).
    let history: Vec<BloodUnitHistory> = sqlx::query_as(
        r#"SELECT h.id, h.unit_id, h.status, h.changed_by_user_id, h.changed_at,
                  h.notes, h.related_record_type, h.related_record_id,
                  u.username AS changed_by_name
           FROM blood_unit_status_history h
           LEFT JOIN users u ON u.id = h.changed_by_user_id
           WHERE h.unit_id = $1
           ORDER BY h.changed_at ASC"#,
    )
    .bind(unit_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    let movements: Vec<BloodMovement> = sqlx::query_as(
        r#"SELECT m.id, m.unit_id, m.movement_type, m.from_location, m.to_location,
                  m.moved_by_user_id, m.moved_at, m.reason, m.related_record_type,
                  m.related_record_id, m.created_at,
                  bu.unit_number AS unit_number,
                  u.username AS moved_by_name
           FROM blood_inventory_movements m
           LEFT JOIN blood_units bu ON bu.id = m.unit_id
           LEFT JOIN users u ON u.id = m.moved_by_user_id
           WHERE m.unit_id = $1
           ORDER BY m.moved_at ASC"#,
    )
    .bind(unit_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    // Crossmatches for this unit.
    let crossmatches: Vec<BloodCrossmatch> = sqlx::query_as(
        r#"SELECT bc.id, bc.unit_id, bc.patient_id, bc.doctor_id, bc.requested_by_user_id,
                  bc.crossmatch_date, bc.method, bc.result, bc.reaction_grade,
                  bc.incubation_time_min, bc.ahg_phase, bc.notes, bc.performed_by_user_id,
                  bc.verified_by_user_id, bc.verified_at, bc.created_at, bc.updated_at,
                  bu.unit_number AS unit_number,
                  p.first_name || ' ' || p.last_name AS patient_name,
                  d.first_name || ' ' || d.last_name AS doctor_name
           FROM blood_crossmatch_results bc
           LEFT JOIN blood_units bu ON bu.id = bc.unit_id
           LEFT JOIN patients p ON p.id = bc.patient_id
           LEFT JOIN doctors d ON d.id = bc.doctor_id
           WHERE bc.unit_id = $1 ORDER BY bc.crossmatch_date ASC"#,
    )
    .bind(unit_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    // Issues for this unit.
    let issues: Vec<BloodIssue> = sqlx::query_as(
        r#"SELECT bi.id, bi.issue_number, bi.unit_id, bi.patient_id, bi.reservation_id,
                  bi.crossmatch_id, bi.doctor_id, bi.issued_by_user_id, bi.issued_at,
                  bi.issued_to_location, bi.issue_type, bi.clinical_indication,
                  bi.special_instructions, bi.returned_at, bi.return_reason,
                  bi.received_by_user_id, bi.created_at, bi.updated_at,
                  bu.unit_number AS unit_number,
                  p.first_name || ' ' || p.last_name AS patient_name,
                  d.first_name || ' ' || d.last_name AS doctor_name,
                  u.username AS issued_by_name
           FROM blood_issues bi
           LEFT JOIN blood_units bu ON bu.id = bi.unit_id
           LEFT JOIN patients p ON p.id = bi.patient_id
           LEFT JOIN doctors d ON d.id = bi.doctor_id
           LEFT JOIN users u ON u.id = bi.issued_by_user_id
           WHERE bi.unit_id = $1 ORDER BY bi.issued_at ASC"#,
    )
    .bind(unit_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    // Transfusions for this unit.
    let transfusions: Vec<BloodTransfusion> = sqlx::query_as(
        r#"SELECT bt.id, bt.transfusion_number, bt.issue_id, bt.unit_id, bt.patient_id,
                  bt.doctor_id, bt.nurse_id, bt.started_at, bt.completed_at,
                  bt.volume_transfused_ml, bt.pre_transfusion_bp, bt.post_transfusion_bp,
                  bt.pre_transfusion_temp, bt.post_transfusion_temp,
                  bt.pre_transfusion_pulse, bt.post_transfusion_pulse,
                  bt.reaction_observed, bt.reaction_type, bt.reaction_severity,
                  bt.reaction_notes, bt.outcome, bt.notes, bt.created_at, bt.updated_at,
                  bu.unit_number AS unit_number,
                  p.first_name || ' ' || p.last_name AS patient_name,
                  d.first_name || ' ' || d.last_name AS doctor_name
           FROM blood_transfusions bt
           LEFT JOIN blood_units bu ON bu.id = bt.unit_id
           LEFT JOIN patients p ON p.id = bt.patient_id
           LEFT JOIN doctors d ON d.id = bt.doctor_id
           WHERE bt.unit_id = $1 ORDER BY bt.started_at ASC"#,
    )
    .bind(unit_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    // Discards for this unit.
    let discards: Vec<BloodDiscard> = sqlx::query_as(
        r#"SELECT bdi.id, bdi.unit_id, bdi.discard_number, bdi.discarded_at, bdi.discard_reason,
                  bdi.discard_notes, bdi.discarded_by_user_id, bdi.authorized_by_user_id,
                  bdi.disposal_method, bdi.created_at, bdi.updated_at,
                  bu.unit_number AS unit_number,
                  u.username AS discarded_by_name
           FROM blood_discards bdi
           LEFT JOIN blood_units bu ON bu.id = bdi.unit_id
           LEFT JOIN users u ON u.id = bdi.discarded_by_user_id
           WHERE bdi.unit_id = $1 ORDER BY bdi.discarded_at ASC"#,
    )
    .bind(unit_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    Ok(serde_json::json!({
        "unit_id": unit_id,
        "status_history": history,
        "movements": movements,
        "crossmatches": crossmatches,
        "issues": issues,
        "transfusions": transfusions,
        "discards": discards,
    }))
}

// ════════════════════════════════════════════════════════════════════════════
// DASHBOARD & STATISTICS
// ════════════════════════════════════════════════════════════════════════════

/// Blood bank dashboard KPIs. RBAC: `BloodBankView`.
/// Uses conditional aggregation (COUNT(*) FILTER) for efficiency — a single
/// table scan computes all inventory KPIs.
#[tauri::command]
pub async fn get_blood_bank_dashboard(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
) -> Result<serde_json::Value, String> {
    let _ = rbac::require(&session, Permission::BloodBankView)?;

    // Inventory KPIs — single conditional-aggregation scan.
    let inv: (i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        r#"SELECT
            COUNT(*) FILTER (WHERE status = 'available') AS available,
            COUNT(*) FILTER (WHERE status = 'reserved') AS reserved,
            COUNT(*) FILTER (WHERE status = 'issued') AS issued,
            COUNT(*) FILTER (WHERE status = 'quarantine') AS quarantine,
            COUNT(*) FILTER (WHERE status = 'discarded') AS discarded_all,
            COUNT(*) FILTER (WHERE expiry_date <= NOW() + INTERVAL '7 days' AND status = 'available') AS expiring_soon
           FROM blood_units
           WHERE deleted_at IS NULL"#,
    )
    .fetch_one(pool.inner())
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    // Donors + donations KPIs (separate table).
    let donor_kpis: (i64, i64, i64) = sqlx::query_as(
        r#"SELECT
            COUNT(*) FILTER (WHERE deleted_at IS NULL) AS total_donors,
            COUNT(*) FILTER (WHERE deleted_at IS NULL AND status = 'active') AS active_donors,
            COUNT(*) FILTER (WHERE deleted_at IS NULL AND status = 'deferred') AS deferred_donors
           FROM blood_donors"#,
    )
    .fetch_one(pool.inner())
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    // Today's transfusions.
    let transfusions_today: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM blood_transfusions WHERE started_at >= CURRENT_DATE AND started_at < CURRENT_DATE + INTERVAL '1 day'",
    )
    .fetch_one(pool.inner())
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    // Active reservations.
    let active_reservations: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM blood_reservations WHERE status = 'active' AND expires_at > NOW()",
    )
    .fetch_one(pool.inner())
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    // Inventory by blood group + component (for the stock grid).
    let stock_by_type: Vec<(String, String, String, i64)> = sqlx::query_as(
        r#"SELECT blood_group, rh_factor, component_type, COUNT(*) AS count
           FROM blood_units
           WHERE status = 'available' AND deleted_at IS NULL
           GROUP BY blood_group, rh_factor, component_type
           ORDER BY blood_group, rh_factor, component_type"#,
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    let stock_grid: serde_json::Value = stock_by_type
        .into_iter()
        .map(|(bg, rh, ct, count)| {
            serde_json::json!({
                "blood_group": bg,
                "rh_factor": rh,
                "component_type": ct,
                "count": count,
            })
        })
        .collect::<Vec<_>>()
        .into();

    Ok(serde_json::json!({
        "available_units": inv.0,
        "reserved_units": inv.1,
        "issued_units": inv.2,
        "quarantine_units": inv.3,
        "discarded_all_time": inv.4,
        "expiring_soon": inv.5,
        "total_donors": donor_kpis.0,
        "active_donors": donor_kpis.1,
        "deferred_donors": donor_kpis.2,
        "transfusions_today": transfusions_today,
        "active_reservations": active_reservations,
        "stock_by_type": stock_grid,
    }))
}

/// Blood bank statistics (monthly aggregates for reporting).
/// RBAC: `BloodBankView`.
#[tauri::command]
pub async fn get_blood_bank_statistics(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    months: Option<i32>,
) -> Result<serde_json::Value, String> {
    let _ = rbac::require(&session, Permission::BloodBankView)?;

    let m = months.unwrap_or(12).clamp(1, 60);

    // Donations per month.
    let donations: Vec<(String, i64)> = sqlx::query_as(
        r#"SELECT TO_CHAR(donation_date, 'YYYY-MM') AS month, COUNT(*) AS count
           FROM blood_donations
           WHERE donation_date >= NOW() - INTERVAL '1 month' * $1
           GROUP BY month ORDER BY month"#,
    )
    .bind(m)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    // Transfusions per month.
    let transfusions: Vec<(String, i64)> = sqlx::query_as(
        r#"SELECT TO_CHAR(started_at, 'YYYY-MM') AS month, COUNT(*) AS count
           FROM blood_transfusions
           WHERE started_at >= NOW() - INTERVAL '1 month' * $1
           GROUP BY month ORDER BY month"#,
    )
    .bind(m)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    // Discards per month.
    let discards: Vec<(String, i64)> = sqlx::query_as(
        r#"SELECT TO_CHAR(discarded_at, 'YYYY-MM') AS month, COUNT(*) AS count
           FROM blood_discards
           WHERE discarded_at >= NOW() - INTERVAL '1 month' * $1
           GROUP BY month ORDER BY month"#,
    )
    .bind(m)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    // Transfusion reactions per month.
    let reactions: Vec<(String, i64)> = sqlx::query_as(
        r#"SELECT TO_CHAR(started_at, 'YYYY-MM') AS month, COUNT(*) AS count
           FROM blood_transfusions
           WHERE reaction_observed = TRUE
             AND started_at >= NOW() - INTERVAL '1 month' * $1
           GROUP BY month ORDER BY month"#,
    )
    .bind(m)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    Ok(serde_json::json!({
        "months": m,
        "donations_per_month": donations,
        "transfusions_per_month": transfusions,
        "discards_per_month": discards,
        "reactions_per_month": reactions,
    }))
}

// ── BE-05: Auto-expiry (called by scheduler every 5 minutes) ─────────────────
//
// Architecture decision: scheduler-integrated periodic task.
//
// Alternatives considered:
//   1. Startup-only reconciliation — insufficient: a desktop app runs for
//      days; units can expire mid-session and would remain 'available' until
//      the next app restart, re-exposing BE-02.
//   2. Independent tokio::spawn task — duplicates the shutdown-flag plumbing
//      already in start_scheduler(); adds maintenance burden for no benefit.
//   3. Scheduler-integrated task (CHOSEN) — reuses the existing 5-minute tick
//      + ShutdownFlags + graceful-exit semantics. One indexed UPDATE per tick
//      is negligible load. Matches the Radiology baseline's pattern of
//      centralized background work in scheduler.rs.
//
// The function is `pub` so scheduler.rs can call it, but it is NOT a
// `#[tauri::command]` — it is not IPC-exposed. It can only be invoked from
// trusted Rust code (the scheduler). This prevents direct IPC invocation
// bypassing the scheduler's rate-limiting.
//
// Transitions: available / reserved / issued / quarantine → expired.
// (Transfused/discarded/expired are terminal and excluded by the WHERE clause.)
// For each expired unit, records a status_history entry + inventory_movement.
// Returns the count of expired units for scheduler logging.

// Intended to be called by the background scheduler (see doc comment above);
// the scheduler wiring is pending — tracked as a finding in the verification
// report. Not IPC-exposed by design.
#[allow(dead_code)]
pub async fn expire_blood_units(pool: &PgPool) -> Result<u64, String> {
    // Single transaction: SELECT the candidates FOR UPDATE, then UPDATE them,
    // then record history + movement for each. The FOR UPDATE prevents a race
    // where another transaction issues/reserves a unit between our SELECT and
    // UPDATE — if that happens, the unit's status changes and our UPDATE
    // WHERE status IN (...) simply skips it (0 rows affected for that unit).
    let mut tx = pool.begin().await.map_err(|e| sanitize_db_error(&e))?;

    // Select units that have passed their expiry date and are not already in a
    // terminal state. FOR UPDATE locks them so concurrent issue/reserve
    // commands block until we commit (or they see the new 'expired' status).
    let expired: Vec<(i32, String)> = sqlx::query_as(
        r#"SELECT id, status FROM blood_units
           WHERE expiry_date <= NOW()
             AND deleted_at IS NULL
             AND status IN ('available', 'reserved', 'issued', 'quarantine')
           FOR UPDATE"#,
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    if expired.is_empty() {
        tx.commit().await.map_err(|e| sanitize_db_error(&e))?;
        return Ok(0);
    }

    let count = expired.len() as u64;

    // Bulk-update all expired units to 'expired' status in one statement.
    // The per-unit old_status is already captured in `expired` for the
    // history records below.
    sqlx::query(
        r#"UPDATE blood_units
           SET status = 'expired', updated_at = NOW()
           WHERE expiry_date <= NOW()
             AND deleted_at IS NULL
             AND status IN ('available', 'reserved', 'issued', 'quarantine')"#,
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| sanitize_db_error(&e))?;

    // Record a status_history + movement for each expired unit. The user_id
    // is NULL (system action) — recorded in the changed_by_user_id column
    // which is nullable, and in the notes as "Automatic expiry".
    for (unit_id, old_status) in &expired {
        sqlx::query(
            r#"INSERT INTO blood_unit_status_history
                  (unit_id, status, changed_by_user_id, notes, related_record_type, related_record_id)
               VALUES ($1, 'expired', NULL, $2, 'scheduler', NULL)"#,
        )
        .bind(unit_id)
        .bind(format!("Automatic expiry (was '{}')", old_status))
        .execute(&mut *tx)
        .await
        .map_err(|e| sanitize_db_error(&e))?;

        sqlx::query(
            r#"INSERT INTO blood_inventory_movements
                  (unit_id, movement_type, from_location, to_location, moved_by_user_id,
                   reason, related_record_type, related_record_id)
               VALUES ($1, 'quarantined', NULL, NULL, NULL,
                       'Automatic expiry — unit removed from inventory',
                       'scheduler', NULL)"#,
        )
        .bind(unit_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| sanitize_db_error(&e))?;
    }

    tx.commit().await.map_err(|e| sanitize_db_error(&e))?;

    // Audit-log the bulk expiry as a single system event. We do NOT use
    // audit::for_session because there is no user session — this is a system
    // action. Write directly via audit::record with user_id=NULL.
    let _ = crate::audit::record(
        pool,
        None,
        Some("system"),
        "blood_unit_auto_expire",
        "blood_units",
        None,
        Some(serde_json::json!({
            "expired_count": count,
            "unit_ids": expired.iter().map(|(id, _)| id).collect::<Vec<_>>(),
        })),
    )
    .await;

    Ok(count)
}

// ── BB-007: Pure-function extraction for testability ─────────────────────────
//
// The ABO/Rh compatibility logic was previously embedded inside
// `check_blood_compatibility` and `issue_blood`, both of which require a live
// DB connection (the matrix is a DB table). To make the ISBT rules unit-testable
// without a DB, this pure function encodes the same rules. The DB-backed
// `check_blood_compatibility` command is the production path; this function is
// used by the `#[cfg(test)]` module below to verify the rules themselves.
//
// The rules match the `blood_compatibility_matrix` seed in db.rs exactly:
//   - O- is the universal donor (compatible with all 8 recipient types)
//   - AB+ is the universal recipient (compatible with all 8 donor types)
//   - Rh- recipients can only receive Rh- blood
//   - ABO plasma/PRBC rules follow standard ISBT 128
//
// This is a behaviour-preserving extraction — no production logic changed.

// Tested reference implementation of the ABO/Rh rules; production enforcement
// uses the seeded `blood_compatibility_matrix` table (BE-04) so this fn has no
// production call sites. Kept as the executable specification for the matrix.
#[allow(dead_code)]
fn is_abo_rh_compatible(
    recipient_group: &str,
    recipient_rh: &str,
    donor_group: &str,
    donor_rh: &str,
) -> bool {
    // Fail-closed on missing/invalid input (matches issue_blood BE-04 logic).
    let valid_groups = ["A", "B", "AB", "O"];
    let valid_rh = ["+", "-"];
    if !valid_groups.contains(&recipient_group)
        || !valid_groups.contains(&donor_group)
        || !valid_rh.contains(&recipient_rh)
        || !valid_rh.contains(&donor_rh)
    {
        return false;
    }

    // Universal donor: O- can go to anyone.
    if donor_group == "O" && donor_rh == "-" {
        return true;
    }

    // Rh rule: Rh- recipients cannot receive Rh+ blood.
    if recipient_rh == "-" && donor_rh == "+" {
        return false;
    }

    // ABO rule (recipient plasma antibodies vs donor RBC antigens):
    //   - O recipient: anti-A + anti-B → only O donors
    //   - A recipient: anti-B → A or O donors
    //   - B recipient: anti-A → B or O donors
    //   - AB recipient: neither → A, B, AB, or O donors (universal recipient)
    match recipient_group {
        "O" => donor_group == "O",
        "A" => donor_group == "A" || donor_group == "O",
        "B" => donor_group == "B" || donor_group == "O",
        "AB" => true, // universal recipient
        _ => false,
    }
}

// ════════════════════════════════════════════════════════════════════════════
// BB-007: UNIT TESTS
// ════════════════════════════════════════════════════════════════════════════
//
// These tests cover the pure functions in this module: the state machine,
// enum validation, and ABO/Rh compatibility logic. They require no database
// and no Tauri runtime — they are standard Rust unit tests runnable via
// `cargo test --lib blood_bank`.
//
// Tests NOT included here (require DB + Tauri runtime):
//   - Integration tests (donation/screening/issue/transfusion/return/discard)
//   - Concurrency tests
//   - Scheduler tests
//   - IPC/RBAC tests
// These are defined in the BB-006 package and require a test DB to execute.

#[cfg(test)]
mod tests {
    use super::*;

    // ── State Machine Tests (UT-SM-001 through UT-SM-024) ───────────────────

    #[test]
    fn test_sm_available_to_reserved() {
        assert!(is_valid_unit_transition("available", "reserved"));
    }

    #[test]
    fn test_sm_available_to_issued() {
        assert!(is_valid_unit_transition("available", "issued"));
    }

    #[test]
    fn test_sm_available_to_discarded() {
        assert!(is_valid_unit_transition("available", "discarded"));
    }

    #[test]
    fn test_sm_available_to_expired() {
        assert!(is_valid_unit_transition("available", "expired"));
    }

    #[test]
    fn test_sm_available_to_quarantine() {
        assert!(is_valid_unit_transition("available", "quarantine"));
    }

    #[test]
    fn test_sm_reserved_to_issued() {
        assert!(is_valid_unit_transition("reserved", "issued"));
    }

    #[test]
    fn test_sm_reserved_to_discarded() {
        assert!(is_valid_unit_transition("reserved", "discarded"));
    }

    #[test]
    fn test_sm_reserved_to_expired() {
        assert!(is_valid_unit_transition("reserved", "expired"));
    }

    #[test]
    fn test_sm_reserved_to_quarantine() {
        assert!(is_valid_unit_transition("reserved", "quarantine"));
    }

    #[test]
    fn test_sm_issued_to_transfused() {
        assert!(is_valid_unit_transition("issued", "transfused"));
    }

    #[test]
    fn test_sm_issued_to_discarded() {
        assert!(is_valid_unit_transition("issued", "discarded"));
    }

    #[test]
    fn test_sm_issued_to_expired() {
        assert!(is_valid_unit_transition("issued", "expired"));
    }

    #[test]
    fn test_sm_quarantine_to_available() {
        assert!(is_valid_unit_transition("quarantine", "available"));
    }

    #[test]
    fn test_sm_quarantine_to_discarded() {
        assert!(is_valid_unit_transition("quarantine", "discarded"));
    }

    #[test]
    fn test_sm_quarantine_to_expired() {
        assert!(is_valid_unit_transition("quarantine", "expired"));
    }

    // BE-08: generic update must NOT allow reserved→available or issued→available
    #[test]
    fn test_sm_reserved_to_available_blocked_be08() {
        assert!(!is_valid_unit_transition("reserved", "available"));
    }

    #[test]
    fn test_sm_issued_to_available_blocked_be08() {
        assert!(!is_valid_unit_transition("issued", "available"));
    }

    // Terminal states — no transitions out
    #[test]
    fn test_sm_transfused_to_available_blocked_terminal() {
        assert!(!is_valid_unit_transition("transfused", "available"));
    }

    #[test]
    fn test_sm_transfused_to_discarded_blocked_terminal() {
        assert!(!is_valid_unit_transition("transfused", "discarded"));
    }

    #[test]
    fn test_sm_discarded_to_available_blocked_terminal() {
        assert!(!is_valid_unit_transition("discarded", "available"));
    }

    #[test]
    fn test_sm_expired_to_available_blocked_terminal() {
        assert!(!is_valid_unit_transition("expired", "available"));
    }

    // Invalid transitions
    #[test]
    fn test_sm_available_to_transfused_blocked_invalid() {
        assert!(!is_valid_unit_transition("available", "transfused"));
    }

    #[test]
    fn test_sm_invalid_target_value() {
        assert!(!is_valid_unit_transition("available", "banana"));
    }

    #[test]
    fn test_sm_empty_current_value() {
        assert!(!is_valid_unit_transition("", "available"));
    }

    // ── Enum Validation Tests (UT-EV-001 through UT-EV-036) ─────────────────

    #[test]
    fn test_ev_blood_group_valid_a() {
        assert!(validate_enum("A", VALID_BLOOD_GROUPS, "blood_group").is_ok());
    }

    #[test]
    fn test_ev_blood_group_valid_ab() {
        assert!(validate_enum("AB", VALID_BLOOD_GROUPS, "blood_group").is_ok());
    }

    #[test]
    fn test_ev_blood_group_invalid_c() {
        assert!(validate_enum("C", VALID_BLOOD_GROUPS, "blood_group").is_err());
    }

    #[test]
    fn test_ev_rh_valid_plus() {
        assert!(validate_enum("+", VALID_RH_FACTORS, "rh_factor").is_ok());
    }

    #[test]
    fn test_ev_rh_valid_minus() {
        assert!(validate_enum("-", VALID_RH_FACTORS, "rh_factor").is_ok());
    }

    #[test]
    fn test_ev_rh_invalid_pos() {
        assert!(validate_enum("pos", VALID_RH_FACTORS, "rh_factor").is_err());
    }

    #[test]
    fn test_ev_component_valid_whole_blood() {
        assert!(validate_enum("whole_blood", VALID_COMPONENT_TYPES, "component_type").is_ok());
    }

    #[test]
    fn test_ev_component_valid_plasma() {
        assert!(validate_enum("plasma", VALID_COMPONENT_TYPES, "component_type").is_ok());
    }

    #[test]
    fn test_ev_component_invalid_water() {
        assert!(validate_enum("water", VALID_COMPONENT_TYPES, "component_type").is_err());
    }

    #[test]
    fn test_ev_unit_status_valid_available() {
        assert!(validate_enum("available", VALID_UNIT_STATUSES, "status").is_ok());
    }

    #[test]
    fn test_ev_unit_status_valid_quarantine() {
        assert!(validate_enum("quarantine", VALID_UNIT_STATUSES, "status").is_ok());
    }

    #[test]
    fn test_ev_unit_status_invalid_zombie() {
        assert!(validate_enum("zombie", VALID_UNIT_STATUSES, "status").is_err());
    }

    #[test]
    fn test_ev_donor_status_valid_active() {
        assert!(validate_enum("active", VALID_DONOR_STATUSES, "donor_status").is_ok());
    }

    #[test]
    fn test_ev_donor_status_invalid() {
        assert!(validate_enum("inactive", VALID_DONOR_STATUSES, "donor_status").is_err());
    }

    #[test]
    fn test_ev_crossmatch_result_valid_compatible() {
        assert!(validate_enum("compatible", VALID_CROSSMATCH_RESULTS, "result").is_ok());
    }

    #[test]
    fn test_ev_crossmatch_result_invalid() {
        assert!(validate_enum("maybe", VALID_CROSSMATCH_RESULTS, "result").is_err());
    }

    #[test]
    fn test_ev_crossmatch_method_valid() {
        assert!(validate_enum("saline_37c", VALID_CROSSMATCH_METHODS, "method").is_ok());
    }

    #[test]
    fn test_ev_crossmatch_method_invalid() {
        assert!(validate_enum("guess", VALID_CROSSMATCH_METHODS, "method").is_err());
    }

    #[test]
    fn test_ev_reservation_status_valid() {
        assert!(validate_enum("active", VALID_RESERVATION_STATUSES, "reservation_status").is_ok());
    }

    #[test]
    fn test_ev_reservation_status_invalid() {
        assert!(validate_enum("pending", VALID_RESERVATION_STATUSES, "reservation_status").is_err());
    }

    #[test]
    fn test_ev_priority_valid_routine() {
        assert!(validate_enum("routine", VALID_PRIORITIES, "priority").is_ok());
    }

    #[test]
    fn test_ev_priority_invalid() {
        assert!(validate_enum("low", VALID_PRIORITIES, "priority").is_err());
    }

    #[test]
    fn test_ev_issue_type_valid_routine() {
        assert!(validate_enum("routine", VALID_ISSUE_TYPES, "issue_type").is_ok());
    }

    #[test]
    fn test_ev_issue_type_valid_emergency() {
        assert!(validate_enum("emergency", VALID_ISSUE_TYPES, "issue_type").is_ok());
    }

    #[test]
    fn test_ev_issue_type_valid_autologous() {
        assert!(validate_enum("autologous", VALID_ISSUE_TYPES, "issue_type").is_ok());
    }

    #[test]
    fn test_ev_issue_type_invalid() {
        assert!(validate_enum("urgent", VALID_ISSUE_TYPES, "issue_type").is_err());
    }

    #[test]
    fn test_ev_discard_reason_valid_expired() {
        assert!(validate_enum("expired", VALID_DISCARD_REASONS, "discard_reason").is_ok());
    }

    #[test]
    fn test_ev_discard_reason_invalid() {
        assert!(validate_enum("lost", VALID_DISCARD_REASONS, "discard_reason").is_err());
    }

    #[test]
    fn test_ev_transfusion_outcome_valid_completed() {
        assert!(validate_enum("completed", VALID_TRANSFUSION_OUTCOMES, "outcome").is_ok());
    }

    #[test]
    fn test_ev_transfusion_outcome_invalid() {
        assert!(validate_enum("done", VALID_TRANSFUSION_OUTCOMES, "outcome").is_err());
    }

    #[test]
    fn test_ev_screening_status_valid_passed() {
        assert!(validate_enum("passed", VALID_SCREENING_STATUSES, "screening_status").is_ok());
    }

    #[test]
    fn test_ev_screening_status_valid_pending() {
        assert!(validate_enum("pending", VALID_SCREENING_STATUSES, "screening_status").is_ok());
    }

    #[test]
    fn test_ev_screening_status_invalid() {
        assert!(validate_enum("unknown", VALID_SCREENING_STATUSES, "screening_status").is_err());
    }

    #[test]
    fn test_ev_empty_string_rejected() {
        assert!(validate_enum("", VALID_UNIT_STATUSES, "status").is_err());
    }

    // ── ABO/Rh Compatibility Tests (UT-ABO + UT-RH) ─────────────────────────

    #[test]
    fn test_abo_o_neg_to_o_neg_compatible() {
        assert!(is_abo_rh_compatible("O", "-", "O", "-"));
    }

    #[test]
    fn test_abo_o_neg_to_o_pos_compatible() {
        assert!(is_abo_rh_compatible("O", "+", "O", "-"));
    }

    #[test]
    fn test_abo_o_neg_to_a_pos_compatible() {
        assert!(is_abo_rh_compatible("A", "+", "O", "-"));
    }

    #[test]
    fn test_abo_o_neg_universal_donor() {
        // O- is compatible with all 8 recipient types
        for rg in &["A", "B", "AB", "O"] {
            for rr in &["+", "-"] {
                assert!(
                    is_abo_rh_compatible(rg, rr, "O", "-"),
                    "O- should be compatible with {}{}",
                    rg,
                    rr
                );
            }
        }
    }

    #[test]
    fn test_abo_a_pos_to_o_neg_incompatible() {
        assert!(!is_abo_rh_compatible("O", "-", "A", "+"));
    }

    #[test]
    fn test_abo_a_pos_to_a_pos_compatible() {
        assert!(is_abo_rh_compatible("A", "+", "A", "+"));
    }

    #[test]
    fn test_abo_b_pos_to_a_pos_incompatible() {
        assert!(!is_abo_rh_compatible("A", "+", "B", "+"));
    }

    #[test]
    fn test_abo_ab_pos_to_o_pos_incompatible() {
        assert!(!is_abo_rh_compatible("O", "+", "AB", "+"));
    }

    #[test]
    fn test_abo_ab_pos_universal_recipient() {
        // AB+ is compatible with all 8 donor types
        for dg in &["A", "B", "AB", "O"] {
            for dr in &["+", "-"] {
                assert!(
                    is_abo_rh_compatible("AB", "+", dg, dr),
                    "AB+ should accept {}{}",
                    dg,
                    dr
                );
            }
        }
    }

    #[test]
    fn test_rh_a_pos_to_a_neg_incompatible() {
        // Rh- recipient cannot receive Rh+ blood
        assert!(!is_abo_rh_compatible("A", "-", "A", "+"));
    }

    #[test]
    fn test_rh_a_neg_to_a_pos_compatible() {
        // Rh+ recipient CAN receive Rh- blood
        assert!(is_abo_rh_compatible("A", "+", "A", "-"));
    }

    #[test]
    fn test_rh_o_neg_universal_compatible() {
        assert!(is_abo_rh_compatible("O", "-", "O", "-"));
    }

    #[test]
    fn test_abo_missing_recipient_group_fail_closed() {
        assert!(!is_abo_rh_compatible("", "+", "O", "-"));
    }

    #[test]
    fn test_abo_missing_recipient_rh_fail_closed() {
        assert!(!is_abo_rh_compatible("A", "", "O", "-"));
    }

    #[test]
    fn test_abo_missing_donor_group_fail_closed() {
        assert!(!is_abo_rh_compatible("A", "+", "", "-"));
    }

    #[test]
    fn test_abo_missing_donor_rh_fail_closed() {
        assert!(!is_abo_rh_compatible("A", "+", "O", ""));
    }

    #[test]
    fn test_abo_invalid_recipient_group_fail_closed() {
        assert!(!is_abo_rh_compatible("X", "+", "O", "-"));
    }

    #[test]
    fn test_abo_invalid_rh_value_fail_closed() {
        assert!(!is_abo_rh_compatible("A", "positive", "O", "-"));
    }

    // ── Terminal Status Tests ───────────────────────────────────────────────

    #[test]
    fn test_terminal_statuses_contains_transfused() {
        assert!(TERMINAL_UNIT_STATUSES.contains(&"transfused"));
    }

    #[test]
    fn test_terminal_statuses_contains_discarded() {
        assert!(TERMINAL_UNIT_STATUSES.contains(&"discarded"));
    }

    #[test]
    fn test_terminal_statuses_contains_expired() {
        assert!(TERMINAL_UNIT_STATUSES.contains(&"expired"));
    }

    #[test]
    fn test_terminal_statuses_excludes_available() {
        assert!(!TERMINAL_UNIT_STATUSES.contains(&"available"));
    }

    #[test]
    fn test_terminal_statuses_excludes_quarantine() {
        assert!(!TERMINAL_UNIT_STATUSES.contains(&"quarantine"));
    }

    // ── allowed_unit_transitions_from Tests ─────────────────────────────────

    #[test]
    fn test_transitions_from_available_lists_reserved() {
        let s = allowed_unit_transitions_from("available");
        assert!(s.contains("reserved"));
    }

    #[test]
    fn test_transitions_from_reserved_directs_to_cancel() {
        let s = allowed_unit_transitions_from("reserved");
        assert!(s.contains("cancel_blood_reservation"));
    }

    #[test]
    fn test_transitions_from_issued_directs_to_return() {
        let s = allowed_unit_transitions_from("issued");
        assert!(s.contains("return_blood_unit"));
    }

    #[test]
    fn test_transitions_from_transfused_is_terminal() {
        let s = allowed_unit_transitions_from("transfused");
        assert!(s.contains("terminal"));
    }
}
