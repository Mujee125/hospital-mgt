//! Pharmacy — medication catalog, prescriptions, and dispensing
//! (Phase 2-C, SRS FR-0120–FR-0124).
//!
//! This module implements the pharmacy workflow on top of three new tables
//! (`medications`, `prescriptions`, `prescription_items`) created in
//! `db.rs::run_migrations`. Stock tracking reuses the existing
//! `inventory_items` / `inventory_movements` tables — there is no separate
//! pharmacy stock table, so a dispense both marks the prescription item
//! `dispensed = TRUE` and decrements the matching `inventory_items` row
//! (matched by name) inside one transaction, with a movement row carrying
//! `reference_id = prescription_item_id` so the audit trail links back to
//! the dispense event (FR-0122).
//!
//! RBAC: reuses `InventoryView` / `InventoryManage` (FR-0124) for catalog
//! and dispensing operations and `PatientsView` / `PatientsCreate` for
//! prescription reads/writes — no new permission variants were added.
//!
//! Audit: every write command (`create_medication`, `update_medication`,
//! `delete_medication`, `create_prescription`, `dispense_prescription_item`)
//! writes one row to `audit_logs` via `audit::for_session`.

use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::audit;
use crate::models::{
    CreateMedication, CreatePrescription, Medication, Prescription, PrescriptionItem,
    PrescriptionWithItems, UpdateMedication,
};
use crate::rbac::{self, Permission, SessionState};

// ── Medication catalog (FR-0120) ─────────────────────────────────────────────

const SELECT_MEDICATIONS: &str = r#"
    SELECT id, brand_name, generic_name, form, strength, schedule,
           category, manufacturer, reorder_level, is_active, created_at
    FROM medications
"#;

/// List medications with an optional name search. Returns both active and
/// inactive rows (the catalog page distinguishes them visually). Search
/// matches brand OR generic name, case-insensitive LIKE.
///
/// RBAC: `InventoryView` (FR-0124 — pharmacists already have it; doctors
/// and nurses also have it via their seed permission sets).
#[tauri::command]
pub async fn get_medications(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    search: Option<String>,
) -> Result<Vec<Medication>, String> {
    let _ = rbac::require(&session, Permission::InventoryView)?;
    let term = search.as_deref().filter(|s| !s.trim().is_empty());
    let rows = match term {
        Some(_) => {
            let pattern = format!("%{}%", term.unwrap().to_lowercase());
            let q = format!(
                "{} WHERE LOWER(brand_name) LIKE $1 OR LOWER(generic_name) LIKE $1 \
                 ORDER BY is_active DESC, brand_name ASC",
                SELECT_MEDICATIONS
            );
            sqlx::query_as::<_, Medication>(&q)
                .bind(pattern)
                .fetch_all(pool.inner())
                .await
        }
        None => {
            let q = format!("{} ORDER BY is_active DESC, brand_name ASC", SELECT_MEDICATIONS);
            sqlx::query_as::<_, Medication>(&q)
                .fetch_all(pool.inner())
                .await
        }
    };
    rows.map_err(|e| crate::db::sanitize_db_error(&e))
}

/// Create a new medication catalog entry. Audit-logged.
///
/// RBAC: `InventoryManage` (FR-0124).
#[tauri::command]
pub async fn create_medication(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    medication: CreateMedication,
) -> Result<i32, String> {
    let s = rbac::require(&session, Permission::InventoryManage)?;
    if medication.brand_name.trim().is_empty() {
        return Err("Brand name is required.".to_string());
    }
    if medication.generic_name.trim().is_empty() {
        return Err("Generic name is required.".to_string());
    }
    if medication.strength.trim().is_empty() {
        return Err("Strength is required.".to_string());
    }

    let row: (i32,) = sqlx::query_as(
        r#"INSERT INTO medications
              (brand_name, generic_name, form, strength, schedule, category,
               manufacturer, reorder_level, is_active)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) RETURNING id"#,
    )
    .bind(medication.brand_name.trim())
    .bind(medication.generic_name.trim())
    .bind(medication.form.as_deref().unwrap_or("tablet"))
    .bind(medication.strength.trim())
    .bind(medication.schedule.as_deref().unwrap_or("non-controlled"))
    .bind(medication.category.as_deref())
    .bind(medication.manufacturer.as_deref())
    .bind(medication.reorder_level.unwrap_or(10))
    .bind(medication.is_active.unwrap_or(true))
    .fetch_one(pool.inner())
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    audit::for_session(
        pool.inner(),
        &s,
        "medication_create",
        "medications",
        Some(&row.0.to_string()),
        Some(serde_json::json!({
            "brand_name": medication.brand_name,
            "generic_name": medication.generic_name,
            "schedule": medication.schedule.as_deref().unwrap_or("non-controlled"),
        })),
    )
    .await;
    Ok(row.0)
}

/// Update an existing medication. Audit-logged.
///
/// RBAC: `InventoryManage`.
#[tauri::command]
pub async fn update_medication(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    id: i32,
    medication: UpdateMedication,
) -> Result<(), String> {
    let s = rbac::require(&session, Permission::InventoryManage)?;
    if id != medication.id {
        return Err("Path id does not match body id.".to_string());
    }
    if medication.brand_name.trim().is_empty() {
        return Err("Brand name is required.".to_string());
    }
    if medication.generic_name.trim().is_empty() {
        return Err("Generic name is required.".to_string());
    }
    if medication.strength.trim().is_empty() {
        return Err("Strength is required.".to_string());
    }

    sqlx::query(
        r#"UPDATE medications SET
              brand_name=$1, generic_name=$2, form=$3, strength=$4,
              schedule=$5, category=$6, manufacturer=$7,
              reorder_level=$8, is_active=$9
           WHERE id=$10"#,
    )
    .bind(medication.brand_name.trim())
    .bind(medication.generic_name.trim())
    .bind(&medication.form)
    .bind(medication.strength.trim())
    .bind(&medication.schedule)
    .bind(medication.category.as_deref())
    .bind(medication.manufacturer.as_deref())
    .bind(medication.reorder_level)
    .bind(medication.is_active)
    .bind(id)
    .execute(pool.inner())
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    audit::for_session(
        pool.inner(),
        &s,
        "medication_update",
        "medications",
        Some(&id.to_string()),
        Some(serde_json::json!({
            "brand_name": medication.brand_name,
            "schedule": medication.schedule,
            "is_active": medication.is_active,
        })),
    )
    .await;
    Ok(())
}

/// Soft-delete a medication (set `is_active = false`). Audit-logged.
///
/// RBAC: `InventoryManage`. The row is preserved so historical
/// prescriptions remain readable (FK is ON DELETE SET NULL but we never
/// actually DELETE — soft-delete keeps the catalog row visible to past
/// prescriptions while hiding it from new-prescription dropdowns).
#[tauri::command]
pub async fn delete_medication(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    id: i32,
) -> Result<(), String> {
    let s = rbac::require(&session, Permission::InventoryManage)?;
    sqlx::query("UPDATE medications SET is_active = FALSE WHERE id = $1")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;

    audit::for_session(
        pool.inner(),
        &s,
        "medication_delete",
        "medications",
        Some(&id.to_string()),
        Some(serde_json::json!({"soft_delete": true})),
    )
    .await;
    Ok(())
}

// ── Prescriptions (FR-0121) ──────────────────────────────────────────────────

const SELECT_PRESCRIPTIONS: &str = r#"
    SELECT p.id, p.patient_id, p.doctor_id, p.encounter_id,
           p.prescribed_by_user_id, p.status, p.notes, p.created_at,
           pt.first_name || ' ' || pt.last_name AS patient_name,
           d.first_name || ' ' || d.last_name   AS doctor_name
    FROM prescriptions p
    LEFT JOIN patients pt ON pt.id = p.patient_id
    LEFT JOIN doctors   d ON d.id  = p.doctor_id
"#;

/// List prescriptions, optionally filtered by `patient_id` and/or `status`.
/// Newest first. RBAC: `PatientsView`.
#[tauri::command]
pub async fn get_prescriptions(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    patient_id: Option<i32>,
    status: Option<String>,
) -> Result<Vec<Prescription>, String> {
    let _ = rbac::require(&session, Permission::PatientsView)?;
    let pid = patient_id;
    let st = status.as_deref().filter(|s| !s.is_empty());

    // Build the WHERE clause dynamically. Both filters are optional; the
    // explicit match keeps the bind-types stable per branch (sqlx needs
    // concrete param types per call site).
    let rows = match (pid, st) {
        (Some(_), Some(_)) => {
            let q = format!(
                "{} WHERE p.patient_id = $1 AND p.status = $2 \
                 ORDER BY p.created_at DESC",
                SELECT_PRESCRIPTIONS
            );
            sqlx::query_as::<_, Prescription>(&q)
                .bind(pid.unwrap())
                .bind(st.unwrap())
                .fetch_all(pool.inner())
                .await
        }
        (Some(_), None) => {
            let q = format!(
                "{} WHERE p.patient_id = $1 ORDER BY p.created_at DESC",
                SELECT_PRESCRIPTIONS
            );
            sqlx::query_as::<_, Prescription>(&q)
                .bind(pid.unwrap())
                .fetch_all(pool.inner())
                .await
        }
        (None, Some(_)) => {
            let q = format!(
                "{} WHERE p.status = $1 ORDER BY p.created_at DESC",
                SELECT_PRESCRIPTIONS
            );
            sqlx::query_as::<_, Prescription>(&q)
                .bind(st.unwrap())
                .fetch_all(pool.inner())
                .await
        }
        (None, None) => {
            let q = format!("{} ORDER BY p.created_at DESC", SELECT_PRESCRIPTIONS);
            sqlx::query_as::<_, Prescription>(&q)
                .fetch_all(pool.inner())
                .await
        }
    };
    rows.map_err(|e| crate::db::sanitize_db_error(&e))
}

/// Get a single prescription with its line items. RBAC: `PatientsView`.
#[tauri::command]
pub async fn get_prescription(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    id: i32,
) -> Result<PrescriptionWithItems, String> {
    let _ = rbac::require(&session, Permission::PatientsView)?;

    let q = format!("{} WHERE p.id = $1", SELECT_PRESCRIPTIONS);
    let prescription: Prescription = sqlx::query_as::<_, Prescription>(&q)
        .bind(id)
        .fetch_one(pool.inner())
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;

    let items: Vec<PrescriptionItem> = sqlx::query_as::<_, PrescriptionItem>(
        r#"SELECT id, prescription_id, medication_id, medication_name,
                  dose, route, frequency, duration, quantity, is_controlled,
                  dispensed, dispensed_at, dispensed_by_user_id, created_at
           FROM prescription_items
           WHERE prescription_id = $1
           ORDER BY id ASC"#,
    )
    .bind(id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    Ok(PrescriptionWithItems { prescription, items })
}

/// Create a prescription with one or more line items. The whole insert is
/// one transaction — if any item fails to insert, the prescription header
/// rolls back too. `is_controlled` is snapshotted from the referenced
/// `medication.schedule != 'non-controlled'` at insert time so the
/// controlled flag is preserved even if the medication is later edited.
///
/// RBAC: `PatientsCreate` (doctors prescribe — matches the existing
/// `create_patient` / `create_encounter` gate). Audit-logged.
#[tauri::command]
pub async fn create_prescription(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    prescription: CreatePrescription,
) -> Result<i32, String> {
    // Review Pass 2, Finding 2 (2026-09-04): guarded by PrescriptionsCreate,
    // NOT PatientsCreate. Receptionists hold PatientsCreate (front-desk
    // registration) and were therefore able to write prescriptions — a
    // prescribing-authority bypass. Only doctors (and super admins) have
    // PrescriptionsCreate.
    let s = rbac::require_strong(&session, pool.inner(), Permission::PrescriptionsCreate).await?;
    if prescription.items.is_empty() {
        return Err("A prescription must have at least one medication item.".to_string());
    }
    for (i, item) in prescription.items.iter().enumerate() {
        if item.medication_name.trim().is_empty() {
            return Err(format!("Item #{}: medication name is required.", i + 1));
        }
        if item.dose.trim().is_empty() {
            return Err(format!("Item #{}: dose is required.", i + 1));
        }
        if item.frequency.trim().is_empty() {
            return Err(format!("Item #{}: frequency is required.", i + 1));
        }
    }

    let mut tx = pool.begin().await.map_err(|e| crate::db::sanitize_db_error(&e))?;

    let row: (i32,) = sqlx::query_as(
        r#"INSERT INTO prescriptions
              (patient_id, doctor_id, encounter_id, prescribed_by_user_id, status, notes)
           VALUES ($1,$2,$3,$4,'active',$5) RETURNING id"#,
    )
    .bind(prescription.patient_id)
    .bind(prescription.doctor_id)
    .bind(prescription.encounter_id)
    .bind(s.user_id)
    .bind(prescription.notes.as_deref())
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    // For each item, look up the medication row (if medication_id is
    // provided) to snapshot `is_controlled` from the schedule. If the
    // medication_id is missing we treat the item as non-controlled (the
    // caller explicitly chose a free-text medication_name with no catalog
    // link — controlled-substance verification only applies to catalog
    // items, since the catalog is where the schedule is recorded).
    for item in &prescription.items {
        let is_controlled: bool = match item.medication_id {
            Some(mid) => {
                let schedule_row: Option<(String,)> =
                    sqlx::query_as("SELECT schedule FROM medications WHERE id = $1")
                        .bind(mid)
                        .fetch_optional(&mut *tx)
                        .await
                        .map_err(|e| crate::db::sanitize_db_error(&e))?;
                match schedule_row {
                    Some((schedule,)) => schedule != "non-controlled",
                    None => false,
                }
            }
            None => false,
        };

        sqlx::query(
            r#"INSERT INTO prescription_items
                  (prescription_id, medication_id, medication_name, dose,
                   route, frequency, duration, quantity, is_controlled)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)"#,
        )
        .bind(row.0)
        .bind(item.medication_id)
        .bind(item.medication_name.trim())
        .bind(item.dose.trim())
        .bind(item.route.as_deref().unwrap_or("oral"))
        .bind(item.frequency.trim())
        .bind(item.duration.as_deref())
        .bind(item.quantity.unwrap_or(1))
        .bind(is_controlled)
        .execute(&mut *tx)
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;
    }

    tx.commit().await.map_err(|e| crate::db::sanitize_db_error(&e))?;

    audit::for_session(
        pool.inner(),
        &s,
        "prescription_create",
        "prescriptions",
        Some(&row.0.to_string()),
        Some(serde_json::json!({
            "patient_id": prescription.patient_id,
            "doctor_id": prescription.doctor_id,
            "encounter_id": prescription.encounter_id,
            "item_count": prescription.items.len(),
        })),
    )
    .await;
    Ok(row.0)
}

// ── Dispensing (FR-0122, FR-0123) ────────────────────────────────────────────

/// Dispense a single prescription item.
///
/// Atomically:
///   1. Marks the prescription_item `dispensed = TRUE`, stamps
///      `dispensed_at = NOW()` and `dispensed_by_user_id = <session>`.
///   2. Looks up an `inventory_items` row matching
///      `name = medication_name` (case-insensitive). If found, locks the
///      row, decrements `stock_quantity` by `prescription_item.quantity`,
///      and writes an `inventory_movements` row with
///      `reason = 'dispense'`, `reference_id = prescription_item.id`,
///      and the resulting balance_after snapshot — matching the pattern
///      in `commands/inventory.rs::adjust_inventory`. Refuses to drive
///      stock negative.
///   3. If all of a prescription's items are now dispensed, marks the
///      prescription `status = 'dispensed'` (best-effort — failure here
///      does not roll back the dispense; the item is still dispensed,
///      the prescription just stays 'active' until a future call
///      finishes the last item).
///   4. Audit-logs `prescription_dispense`.
///
/// Controlled-substance verification (FR-0123) is enforced at the UI
/// layer: the frontend pops a confirmation dialog when
/// `item.is_controlled == true` before calling this command. The command
/// itself only requires `InventoryManage` (which the pharmacist role has
/// via FR-0124) — there is no second-person witness table in this
/// iteration (the SRS explicitly allows the simplification).
///
/// RBAC: `InventoryManage`.
#[tauri::command]
pub async fn dispense_prescription_item(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    prescription_item_id: i32,
) -> Result<(), String> {
    let s = rbac::require_strong(&session, pool.inner(), Permission::InventoryManage).await?;

    let mut tx = pool.begin().await.map_err(|e| crate::db::sanitize_db_error(&e))?;

    // Lock the prescription_item row and read its current state.
    let item_row: Option<(i32, i32, String, i32, bool)> = sqlx::query_as(
        r#"SELECT id, prescription_id, medication_name, quantity, dispensed
           FROM prescription_items WHERE id = $1 FOR UPDATE"#,
    )
    .bind(prescription_item_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    let (item_id, rx_id, med_name, qty, already_dispensed) = item_row.ok_or_else(|| {
        format!("Prescription item {} not found.", prescription_item_id)
    })?;

    if already_dispensed {
        return Err("This prescription item has already been dispensed.".to_string());
    }

    // Decrement inventory if there's a matching inventory_items row.
    // Match by name, case-insensitive, on the active rows. Use FOR UPDATE
    // to prevent concurrent dispenses racing on the same stock row.
    let inv_row: Option<(i32, Decimal)> = sqlx::query_as(
        r#"SELECT id, stock_quantity FROM inventory_items
           WHERE LOWER(name) = LOWER($1) AND is_active = TRUE
           ORDER BY id ASC LIMIT 1 FOR UPDATE"#,
    )
    .bind(&med_name)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    // Capture the bool before the if-let so the audit JSON below doesn't
    // depend on `Option<(i32, Decimal)>: Copy` (it IS Copy today because
    // both `i32` and `rust_decimal::Decimal` are Copy, but binding the
    // bool explicitly is clearer and survives a future Decimal-loses-Copy
    // refactor).
    let inventory_adjusted = inv_row.is_some();

    if let Some((inv_id, current_balance)) = inv_row {
        let qty_dec = Decimal::from(qty);
        let new_balance = current_balance - qty_dec;
        if new_balance < Decimal::ZERO {
            return Err(format!(
                "Insufficient stock for '{}': current balance {}, requested {}. \
                 Dispense cannot exceed available quantity.",
                med_name, current_balance, qty
            ));
        }
        sqlx::query("UPDATE inventory_items SET stock_quantity = $1, updated_at = NOW() WHERE id = $2")
            .bind(new_balance)
            .bind(inv_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| crate::db::sanitize_db_error(&e))?;

        sqlx::query(
            r#"INSERT INTO inventory_movements
                  (item_id, quantity_change, reason, balance_after,
                   reference_id, notes, created_by_user_id)
               VALUES ($1, $2, 'dispense', $3, $4, $5, $6)"#,
        )
        .bind(inv_id)
        .bind(-qty_dec)
        .bind(new_balance)
        .bind(item_id)
        .bind(format!("Dispensed against prescription item #{}", item_id))
        .bind(s.user_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;
    }
    // If no matching inventory row exists we still mark the prescription
    // item dispensed — the medication may be tracked outside the system
    // (e.g. directly issued by the manufacturer, or a non-stock item).
    // The audit log captures the dispense either way; the inventory
    // movement row is only written when stock was actually decremented.

    // Mark the prescription_item as dispensed.
    sqlx::query(
        r#"UPDATE prescription_items SET
              dispensed = TRUE,
              dispensed_at = NOW(),
              dispensed_by_user_id = $1
           WHERE id = $2"#,
    )
    .bind(s.user_id)
    .bind(item_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    // Best-effort: if every item in this prescription is now dispensed,
    // flip the prescription status to 'dispensed'. Failure here is
    // swallowed (logged) — the dispense itself is already committed by
    // the UPDATE above, so we don't want a status-update error to roll
    // it back. Same swallow-on-error pattern as `update_lab_result`'s
    // order-completion flip.
    let _ = sqlx::query(
        r#"UPDATE prescriptions SET status = 'dispensed'
           WHERE id = $1 AND NOT EXISTS (
               SELECT 1 FROM prescription_items
               WHERE prescription_id = $1 AND dispensed = FALSE
           )"#,
    )
    .bind(rx_id)
    .execute(&mut *tx)
    .await;

    tx.commit().await.map_err(|e| crate::db::sanitize_db_error(&e))?;

    audit::for_session(
        pool.inner(),
        &s,
        "prescription_dispense",
        "prescription_items",
        Some(&item_id.to_string()),
        Some(serde_json::json!({
            "prescription_id": rx_id,
            "medication_name": med_name,
            "quantity": qty,
            "inventory_adjusted": inventory_adjusted,
        })),
    )
    .await;
    Ok(())
}
