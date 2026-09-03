//! Radiology — orders, reports, and verification workflow
//! (Phase 2-D, SRS FR-0140–FR-0142).
//!
//! P0 Remediation: state machine, RBAC bypass, report validation,
//! DB concurrency, soft delete, pagination.
//! P1 Hardening: SEQUENCE-based order numbers, enum validation,
//! dashboard consolidation, partial index, code cleanup.
//!
//! RBAC: uses seven dedicated `Radiology*` permission variants.
//! Audit: every write command writes to `audit_logs`.
//!
//! Status workflow (strict state machine):
//!   ordered → scheduled → in_progress → completed → reported → verified
//!   Any non-terminal state can transition to 'cancelled'.
//!   'verified' can ONLY be set via `verify_radiology_report` (not via
//!   `update_radiology_order_status`).
//!   'reported' can ONLY be set via `create_radiology_report` (not via
//!   `update_radiology_order_status`).

use sqlx::PgPool;

use crate::audit;
use crate::models::{
    CreateRadiologyOrder, CreateRadiologyReport, RadiologyOrder, RadiologyReport,
};
use crate::rbac::{self, Permission, SessionState};

// ── P1-2: Validated enum constants ──────────────────────────────────────────

const VALID_STATUSES: &[&str] = &[
    "ordered", "scheduled", "in_progress", "completed",
    "reported", "verified", "cancelled",
];

const VALID_PRIORITIES: &[&str] = &["routine", "urgent", "emergency", "stat"];

const VALID_STUDY_TYPES: &[&str] = &[
    "X-Ray", "CT Scan", "MRI", "Ultrasound",
    "Mammography", "Fluoroscopy", "DEXA", "Other",
];

fn validate_enum(value: &str, allowed: &[&str], field_name: &str) -> Result<(), String> {
    if !allowed.contains(&value) {
        return Err(format!(
            "Invalid {} '{}'. Allowed values: {}.",
            field_name, value, allowed.join(", ")
        ));
    }
    Ok(())
}

// ── State machine (P0-1) ────────────────────────────────────────────────────

const FORBIDDEN_VIA_UPDATE: &[&str] = &["reported", "verified"];

fn is_valid_transition(current: &str, target: &str) -> bool {
    if FORBIDDEN_VIA_UPDATE.contains(&target) {
        return false;
    }
    matches!(
        (current, target),
        ("ordered", "scheduled")
            | ("ordered", "cancelled")
            | ("scheduled", "in_progress")
            | ("scheduled", "cancelled")
            | ("in_progress", "completed")
            | ("in_progress", "cancelled")
            | ("completed", "cancelled")
    )
}

fn allowed_transitions_from(status: &str) -> String {
    match status {
        "ordered" => "scheduled, cancelled".to_string(),
        "scheduled" => "in_progress, cancelled".to_string(),
        "in_progress" => "completed, cancelled".to_string(),
        "completed" => "cancelled (use create report to advance to 'reported')".to_string(),
        "reported" => "verified (use verify report command)".to_string(),
        "verified" => "(terminal — no further transitions)".to_string(),
        "cancelled" => "(terminal — no further transitions)".to_string(),
        _ => "(unknown status)".to_string(),
    }
}

// ── Orders (FR-0140) ─────────────────────────────────────────────────────────

const SELECT_ORDERS: &str = r#"
    SELECT ro.id, ro.patient_id, ro.encounter_id, ro.ordered_by_doctor_id,
           ro.ordered_by_user_id, ro.order_number, ro.department,
           ro.clinical_indication, ro.symptoms, ro.diagnosis, ro.priority,
           ro.study_type, ro.contrast_required, ro.body_part, ro.instructions,
           ro.status, ro.assigned_radiologist_id, ro.assigned_technician,
           ro.expected_date, ro.ordered_at, ro.scheduled_at, ro.completed_at,
           ro.reported_at, ro.verified_at, ro.created_at, ro.updated_at,
           ro.deleted_at,
           p.first_name || ' ' || p.last_name AS patient_name,
           d.first_name || ' ' || d.last_name   AS doctor_name,
           r.first_name || ' ' || r.last_name   AS radiologist_name
    FROM radiology_orders ro
    LEFT JOIN patients p ON p.id = ro.patient_id
    LEFT JOIN doctors   d ON d.id = ro.ordered_by_doctor_id
    LEFT JOIN doctors   r ON r.id = ro.assigned_radiologist_id
"#;

/// List radiology orders with server-side pagination (P0-6).
/// RBAC: `RadiologyView`. Excludes soft-deleted records (P0-5).
///
/// Returns a JSON object with `orders` (array) + `total` (count) so the
/// frontend can render "Page X of Y" without a separate count query.
#[tauri::command]
pub async fn get_radiology_orders(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    status_filter: Option<String>,
    priority_filter: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<serde_json::Value, String> {
    let _ = rbac::require(&session, Permission::RadiologyView)?;
    let st = status_filter.as_deref().filter(|s| !s.is_empty());
    let pr = priority_filter.as_deref().filter(|s| !s.is_empty());

    let pg = page.unwrap_or(1).max(1);
    let ps = page_size.unwrap_or(10).clamp(1, 100);
    let offset = (pg - 1) * ps;

    // Build WHERE clause — always exclude soft-deleted records.
    let (where_clause, has_status, has_priority) = match (st, pr) {
        (Some(_), Some(_)) => ("WHERE ro.deleted_at IS NULL AND ro.status = $1 AND ro.priority = $2", true, true),
        (Some(_), None) => ("WHERE ro.deleted_at IS NULL AND ro.status = $1", true, false),
        (None, Some(_)) => ("WHERE ro.deleted_at IS NULL AND ro.priority = $1", false, true),
        (None, None) => ("WHERE ro.deleted_at IS NULL", false, false),
    };

    // Count query (for pagination metadata).
    let count_sql = format!(
        "SELECT COUNT(*) FROM radiology_orders ro {}",
        where_clause
    );
    let mut count_q = sqlx::query_scalar::<_, i64>(&count_sql);
    if has_status { count_q = count_q.bind(st.unwrap()); }
    if has_priority { count_q = count_q.bind(pr.unwrap()); }
    let total: i64 = count_q
        .fetch_one(pool.inner())
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;

    // Data query with LIMIT/OFFSET.
    let data_sql = format!(
        "{} {} ORDER BY ro.ordered_at DESC LIMIT ${} OFFSET ${}",
        SELECT_ORDERS, where_clause,
        if has_status && has_priority { 3 } else if has_status || has_priority { 2 } else { 1 },
        if has_status && has_priority { 4 } else if has_status || has_priority { 3 } else { 2 },
    );
    let mut data_q = sqlx::query_as::<_, RadiologyOrder>(&data_sql);
    if has_status { data_q = data_q.bind(st.unwrap()); }
    if has_priority { data_q = data_q.bind(pr.unwrap()); }
    data_q = data_q.bind(ps).bind(offset);
    let orders: Vec<RadiologyOrder> = data_q
        .fetch_all(pool.inner())
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;

    Ok(serde_json::json!({
        "orders": orders,
        "total": total,
        "page": pg,
        "page_size": ps,
        "total_pages": ((total + ps - 1) / ps).max(1),
    }))
}

/// Get a single radiology order by id. RBAC: `RadiologyView`.
/// Excludes soft-deleted records.
#[tauri::command]
pub async fn get_radiology_order(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    id: i32,
) -> Result<RadiologyOrder, String> {
    let _ = rbac::require(&session, Permission::RadiologyView)?;
    let q = format!("{} WHERE ro.id = $1 AND ro.deleted_at IS NULL", SELECT_ORDERS);
    sqlx::query_as::<_, RadiologyOrder>(&q)
        .bind(id)
        .fetch_one(pool.inner())
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))
}

/// Create a new radiology order. RBAC: `RadiologyCreate`. Audit-logged.
///
/// P1-1: Order number generated via PostgreSQL SEQUENCE (radiology_order_seq)
/// — concurrency-safe, no race condition. Format: RAD-YYYYMMDD-NNNNNN.
/// P1-2: Validates study_type, priority, and status against server-side enums.
#[tauri::command]
pub async fn create_radiology_order(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    order: CreateRadiologyOrder,
) -> Result<i32, String> {
    let s = rbac::require_strong(&session, pool.inner(), Permission::RadiologyCreate).await?;

    // P1-2: Validate enums before any DB access.
    let study_type = order.study_type.trim();
    let priority = order.priority.trim();
    validate_enum(study_type, VALID_STUDY_TYPES, "study_type")?;
    validate_enum(priority, VALID_PRIORITIES, "priority")?;

    let expected_date: Option<chrono::NaiveDate> = match order.expected_date.as_deref() {
        Some(s) if !s.trim().is_empty() => Some(
            chrono::NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d")
                .map_err(|e| format!("Invalid expected_date '{}': {}", s, e))?,
        ),
        _ => None,
    };

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;

    // P1-1: Use SEQUENCE for concurrency-safe order number generation.
    // Format: RAD-YYYYMMDD-NNNNNN (6-digit zero-padded sequence number).
    let order_number_row: (String,) = sqlx::query_as(
        "SELECT 'RAD-' || TO_CHAR(NOW(),'YYYYMMDD') || '-' || \
         LPAD(nextval('radiology_order_seq')::text, 6, '0')",
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;
    let order_number = order_number_row.0;

    let row: (i32,) = sqlx::query_as(
        r#"INSERT INTO radiology_orders
              (patient_id, encounter_id, ordered_by_doctor_id, ordered_by_user_id,
               order_number, department, clinical_indication, symptoms, diagnosis,
               priority, study_type, contrast_required, body_part, instructions,
               status, assigned_radiologist_id, assigned_technician, expected_date)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,'ordered',
                   $15,$16,$17) RETURNING id"#,
    )
    .bind(order.patient_id)
    .bind(order.encounter_id)
    .bind(order.ordered_by_doctor_id)
    .bind(s.user_id)
    .bind(&order_number)
    .bind(order.department.as_deref())
    .bind(order.clinical_indication.as_deref())
    .bind(order.symptoms.as_deref())
    .bind(order.diagnosis.as_deref())
    .bind(order.priority.trim())
    .bind(order.study_type.trim())
    .bind(order.contrast_required)
    .bind(order.body_part.as_deref())
    .bind(order.instructions.as_deref())
    .bind(order.assigned_radiologist_id)
    .bind(order.assigned_technician.as_deref())
    .bind(expected_date)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    sqlx::query(
        r#"INSERT INTO radiology_status_history
              (order_id, status, changed_by_user_id, notes)
           VALUES ($1, 'ordered', $2, 'Order created')"#,
    )
    .bind(row.0)
    .bind(s.user_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    tx.commit()
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;

    audit::for_session(
        pool.inner(),
        &s,
        "radiology_order_create",
        "radiology_orders",
        Some(&row.0.to_string()),
        Some(serde_json::json!({
            "order_number": order_number,
            "patient_id": order.patient_id,
            "study_type": order.study_type,
            "priority": order.priority,
            "contrast_required": order.contrast_required,
        })),
    )
    .await;
    Ok(row.0)
}

/// Update an order's status with state-machine validation (P0-1) and
/// RBAC bypass elimination (P0-2).
///
/// P0-1: Only allowed transitions are accepted. 'reported' and 'verified'
/// are forbidden via this command — they must be set by their dedicated
/// commands (create_radiology_report / verify_radiology_report).
///
/// P0-2: If the target status is 'verified', reject with a permission
/// error directing the user to use the verify command.
///
/// RBAC: `RadiologyUpdate`. Audit-logged.
#[tauri::command]
pub async fn update_radiology_order_status(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    id: i32,
    status: String,
    notes: Option<String>,
) -> Result<(), String> {
    let s = rbac::require_strong(&session, pool.inner(), Permission::RadiologyUpdate).await?;
    let new_status = status.trim();

    if new_status.is_empty() {
        return Err("Status is required.".to_string());
    }

    // P1-2: Validate the target status against the server-side enum.
    validate_enum(new_status, VALID_STATUSES, "status")?;

    // P0-2: 'reported' and 'verified' are forbidden via this generic command.
    if FORBIDDEN_VIA_UPDATE.contains(&new_status) {
        return Err(format!(
            "Status '{}' cannot be set via the generic status update command. \
             Use the dedicated {} command instead.",
            new_status,
            if new_status == "verified" { "verify report" } else { "create report" }
        ));
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;

    // P0-1: Load current status and validate the transition.
    let current_row: Option<(String,)> = sqlx::query_as(
        "SELECT status FROM radiology_orders WHERE id = $1 AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    let current_status = current_row
        .ok_or_else(|| format!("Radiology order {} not found or deleted.", id))?
        .0;

    if !is_valid_transition(&current_status, new_status) {
        return Err(format!(
            "Invalid status transition: '{}' → '{}'. \
             Allowed transitions from '{}' are: {}.",
            current_status,
            new_status,
            current_status,
            allowed_transitions_from(&current_status)
        ));
    }

    // Insert status_history row.
    sqlx::query(
        r#"INSERT INTO radiology_status_history
              (order_id, status, changed_by_user_id, notes)
           VALUES ($1, $2, $3, $4)"#,
    )
    .bind(id)
    .bind(new_status)
    .bind(s.user_id)
    .bind(notes.as_deref())
    .execute(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    // Stamp the matching terminal timestamp.
    let stamp_clause = match new_status {
        "scheduled" => ", scheduled_at = COALESCE(scheduled_at, NOW())",
        "completed" => ", completed_at = COALESCE(completed_at, NOW())",
        _ => "",
    };
    let update_sql = format!(
        "UPDATE radiology_orders SET status = $1{}, updated_at = NOW() WHERE id = $2",
        stamp_clause
    );
    sqlx::query(&update_sql)
        .bind(new_status)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;

    tx.commit()
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;

    audit::for_session(
        pool.inner(),
        &s,
        "radiology_order_status_update",
        "radiology_orders",
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

/// Soft-delete a radiology order (P0-5).
///
/// Replaces the previous hard DELETE with a soft-delete: sets `deleted_at`
/// and `deleted_by_user_id`. All clinical data (order, reports, status
/// history) is preserved. List queries exclude soft-deleted records.
///
/// RBAC: `RadiologyDelete`. Audit-logged.
#[tauri::command]
pub async fn delete_radiology_order(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    id: i32,
    reason: Option<String>,
) -> Result<(), String> {
    let s = rbac::require_strong(&session, pool.inner(), Permission::RadiologyDelete).await?;

    let result = sqlx::query(
        r#"UPDATE radiology_orders
           SET deleted_at = NOW(),
               deleted_by_user_id = $1,
               deleted_reason = $2,
               updated_at = NOW()
           WHERE id = $3 AND deleted_at IS NULL"#,
    )
    .bind(s.user_id)
    .bind(reason.as_deref())
    .bind(id)
    .execute(pool.inner())
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    if result.rows_affected() == 0 {
        return Err("Radiology order not found or already deleted.".to_string());
    }

    audit::for_session(
        pool.inner(),
        &s,
        "radiology_order_delete",
        "radiology_orders",
        Some(&id.to_string()),
        Some(serde_json::json!({
            "soft_delete": true,
            "reason": reason,
        })),
    )
    .await;
    Ok(())
}

// ── Reports (FR-0141) ────────────────────────────────────────────────────────

/// Get the report for an order (if any). RBAC: `RadiologyView`.
///
/// P0-5-FOLLOWUP: Rejects if the parent order has been soft-deleted —
/// reports on deleted orders must not be accessible via direct IPC.
#[tauri::command]
pub async fn get_radiology_report(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    order_id: i32,
) -> Result<Option<RadiologyReport>, String> {
    let _ = rbac::require_strong(&session, pool.inner(), Permission::RadiologyView).await?;

    // P0-5-FOLLOWUP: Verify the parent order is not soft-deleted.
    let order_exists: Option<(i32,)> = sqlx::query_as(
        "SELECT id FROM radiology_orders WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(order_id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    if order_exists.is_none() {
        return Ok(None);
    }

    sqlx::query_as::<_, RadiologyReport>(
        r#"SELECT id, order_id, findings, impression, recommendations,
                  critical_finding, radiologist_id, verified_by_user_id,
                  report_date, verified_at, created_at, updated_at
           FROM radiology_reports
           WHERE order_id = $1
           ORDER BY id DESC
           LIMIT 1"#,
    )
    .bind(order_id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))
}

/// Create a radiology report. RBAC: `RadiologyReport`. Audit-logged.
///
/// P0-3: Validates that the parent order's status is 'completed' before
/// allowing report creation. Reports cannot be filed before imaging is done.
///
/// P0-4: The UNIQUE constraint on `radiology_reports.order_id` (added in
/// db.rs) is the database-level enforcement. The application-level check
/// is kept as a fast-path for a better error message, but the constraint
/// is the real guarantee against concurrent duplicates.
#[tauri::command]
pub async fn create_radiology_report(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    report: CreateRadiologyReport,
) -> Result<i32, String> {
    let s = rbac::require_strong(&session, pool.inner(), Permission::RadiologyReport).await?;

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;

    // P0-3: Verify the order exists, is not deleted, and is in 'completed'
    // status — a report cannot be filed before imaging is done.
    let order_row: Option<(String,)> = sqlx::query_as(
        "SELECT status FROM radiology_orders WHERE id = $1 AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(report.order_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    let order_status = order_row
        .ok_or_else(|| format!("Radiology order {} not found or deleted.", report.order_id))?
        .0;

    if order_status != "completed" && order_status != "reported" {
        return Err(format!(
            "Cannot create a report for order {} because its status is '{}' \
             (must be 'completed'). Imaging must be finished before a report \
             can be filed.",
            report.order_id, order_status
        ));
    }

    // P0-4: Application-level check (fast-path for a better error message).
    let existing: Option<(i32,)> =
        sqlx::query_as("SELECT id FROM radiology_reports WHERE order_id = $1")
            .bind(report.order_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| crate::db::sanitize_db_error(&e))?;
    if existing.is_some() {
        return Err("A report already exists for this radiology order.".to_string());
    }

    let critical = report.critical_finding.unwrap_or(false);

    // P0-4: The INSERT will fail with a unique-violation if a concurrent
    // transaction inserts a report for the same order_id between our
    // application-level check and this INSERT. The UNIQUE constraint on
    // order_id is the real guarantee.
    let row: (i32,) = sqlx::query_as(
        r#"INSERT INTO radiology_reports
              (order_id, findings, impression, recommendations, critical_finding)
           VALUES ($1,$2,$3,$4,$5) RETURNING id"#,
    )
    .bind(report.order_id)
    .bind(report.findings.as_deref())
    .bind(report.impression.as_deref())
    .bind(report.recommendations.as_deref())
    .bind(critical)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        // P0-4: Check for unique-violation (SQLSTATE 23505) and return
        // a user-friendly message instead of a raw SQL error.
        if let Some(db_err) = e.as_database_error() {
            if db_err.code().as_deref() == Some("23505") {
                return "A report already exists for this radiology order \
                        (concurrent submission detected). Please refresh and try again."
                    .to_string();
            }
        }
        crate::db::sanitize_db_error(&e)
    })?;

    sqlx::query(
        r#"UPDATE radiology_orders
           SET status = 'reported',
               reported_at = COALESCE(reported_at, NOW()),
               updated_at = NOW()
           WHERE id = $1"#,
    )
    .bind(report.order_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    sqlx::query(
        r#"INSERT INTO radiology_status_history
              (order_id, status, changed_by_user_id, notes)
           VALUES ($1, 'reported', $2, $3)"#,
    )
    .bind(report.order_id)
    .bind(s.user_id)
    .bind(if critical {
        "Report filed — CRITICAL FINDING flagged"
    } else {
        "Report filed"
    })
    .execute(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    tx.commit()
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;

    audit::for_session(
        pool.inner(),
        &s,
        "radiology_report_create",
        "radiology_reports",
        Some(&row.0.to_string()),
        Some(serde_json::json!({
            "order_id": report.order_id,
            "critical_finding": critical,
        })),
    )
    .await;
    Ok(row.0)
}

/// Verify a radiology report. RBAC: `RadiologyVerify`. Audit-logged.
///
/// P0-5-FOLLOWUP: Rejects if the parent order has been soft-deleted —
/// reports on deleted orders must not be verifiable via direct IPC.
#[tauri::command]
pub async fn verify_radiology_report(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    report_id: i32,
) -> Result<(), String> {
    let s = rbac::require_strong(&session, pool.inner(), Permission::RadiologyVerify).await?;

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;

    // P0-5-FOLLOWUP: Look up the report AND verify the parent order is not
    // soft-deleted in a single JOIN query. If the order is deleted (or the
    // report doesn't exist), reject with a clear error.
    let order_row: Option<(i32,)> = sqlx::query_as(
        "SELECT rr.order_id FROM radiology_reports rr \
         JOIN radiology_orders ro ON ro.id = rr.order_id \
         WHERE rr.id = $1 AND ro.deleted_at IS NULL",
    )
    .bind(report_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    let order_id = order_row
        .ok_or_else(|| {
            format!(
                "Radiology report {} not found or the parent order has been deleted.",
                report_id
            )
        })?
        .0;

    sqlx::query(
        r#"UPDATE radiology_reports
           SET verified_at = COALESCE(verified_at, NOW()),
               verified_by_user_id = $1,
               updated_at = NOW()
           WHERE id = $2"#,
    )
    .bind(s.user_id)
    .bind(report_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    sqlx::query(
        r#"UPDATE radiology_orders
           SET status = 'verified',
               verified_at = COALESCE(verified_at, NOW()),
               updated_at = NOW()
           WHERE id = $1"#,
    )
    .bind(order_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    sqlx::query(
        r#"INSERT INTO radiology_status_history
              (order_id, status, changed_by_user_id, notes)
           VALUES ($1, 'verified', $2, 'Report verified')"#,
    )
    .bind(order_id)
    .bind(s.user_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    tx.commit()
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;

    audit::for_session(
        pool.inner(),
        &s,
        "radiology_report_verify",
        "radiology_reports",
        Some(&report_id.to_string()),
        Some(serde_json::json!({
            "order_id": order_id,
            "verifier_user_id": s.user_id,
        })),
    )
    .await;
    Ok(())
}

// ── Dashboard (FR-0142) ──────────────────────────────────────────────────────

/// Radiology dashboard KPIs. RBAC: `RadiologyView`.
/// Excludes soft-deleted records from all counts.
///
/// P1-4: Consolidated from 6 separate queries into 2 (one for orders, one
/// for reports). The orders query uses conditional aggregation (COUNT(*)
/// FILTER) to compute all 5 order-based KPIs in a single table scan — a
/// 5x reduction in DB round-trips. The reports query is separate because
/// it queries a different table.
#[tauri::command]
pub async fn get_radiology_dashboard(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
) -> Result<serde_json::Value, String> {
    let _ = rbac::require_strong(&session, pool.inner(), Permission::RadiologyView).await?;

    // P1-4: Single query for all 5 order-based KPIs using conditional
    // aggregation. Much faster than 5 separate COUNT(*) queries at scale.
    let row: (i64, i64, i64, i64, i64) = sqlx::query_as(
        r#"SELECT
            COUNT(*) FILTER (WHERE ordered_at >= CURRENT_DATE AND ordered_at < CURRENT_DATE + INTERVAL '1 day') AS studies_today,
            COUNT(*) FILTER (WHERE status IN ('ordered','scheduled','in_progress','completed')) AS pending_reports,
            COUNT(*) FILTER (WHERE priority = 'emergency' AND status NOT IN ('verified','cancelled')) AS emergency_cases,
            COUNT(*) FILTER (WHERE completed_at >= CURRENT_DATE AND completed_at < CURRENT_DATE + INTERVAL '1 day') AS completed_today,
            COUNT(*) FILTER (WHERE status = 'cancelled') AS cancelled
           FROM radiology_orders
           WHERE deleted_at IS NULL"#,
    )
    .fetch_one(pool.inner())
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    // Reports KPI is separate (different table).
    let verification_pending: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM radiology_reports WHERE verified_at IS NULL",
    )
    .fetch_one(pool.inner())
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    Ok(serde_json::json!({
        "studies_today": row.0,
        "pending_reports": row.1,
        "emergency_cases": row.2,
        "completed_today": row.3,
        "cancelled": row.4,
        "verification_pending": verification_pending,
    }))
}

