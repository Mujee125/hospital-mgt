//! Laboratory — test catalog, orders, results. RBAC-guarded + audited.

use sqlx::PgPool;

use crate::audit;
use crate::models::{CreateLabOrder, LabOrder, LabOrderTest, LabTestCatalog, UpdateLabResult};
use crate::rbac::{self, Permission, SessionState};

// ── Test catalog ──────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_lab_catalog(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
) -> Result<Vec<LabTestCatalog>, String> {
    let _ = rbac::require(&session, Permission::LabView)?;
    sqlx::query_as("SELECT * FROM lab_test_catalog WHERE is_active = TRUE ORDER BY name")
        .fetch_all(pool.inner())
        .await
        .map_err(|e| format!("Get lab catalog: {}", e))
}

// Tauri command — `#[tauri::command]` requires flat parameters (one per JS
// arg) and does not support grouping into a struct, so we allow the extra
// arguments rather than refactor the public command signature.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn create_lab_test(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    name: String,
    code: String,
    category: Option<String>,
    sample_type: Option<String>,
    normal_range: Option<String>,
    unit: Option<String>,
    price: Option<f64>,
) -> Result<i32, String> {
    let s = rbac::require(&session, Permission::LabCatalogManage)?;
    let row: (i32,) = sqlx::query_as(
        r#"INSERT INTO lab_test_catalog (name, code, category, sample_type, normal_range, unit, price)
           VALUES ($1,$2,$3,$4,$5,$6,$7) RETURNING id"#,
    )
    .bind(&name).bind(&code).bind(&category).bind(&sample_type)
    .bind(&normal_range).bind(&unit)
    .bind(rust_decimal::Decimal::from_f64_retain(price.unwrap_or(0.0)).unwrap_or_default())
    .fetch_one(pool.inner())
    .await
    .map_err(|e| format!("Create lab test: {}", e))?;
    audit::for_session(pool.inner(), &s, "lab_test_create", "lab_test_catalog",
        Some(&row.0.to_string()), Some(serde_json::json!({"code": code}))).await;
    Ok(row.0)
}

// ── Orders ────────────────────────────────────────────────────────────────────

const SELECT_ORDERS: &str = r#"
    SELECT lo.id, lo.patient_id, lo.encounter_id, lo.ordered_by_doctor_id,
           lo.ordered_by_user_id, lo.status, lo.ordered_at, lo.created_at,
           lo.sample_barcode, lo.sampled_at, lo.sampled_by_user_id,
           lo.approved_at, lo.approved_by_user_id,
           p.first_name || ' ' || p.last_name AS patient_name,
           d.first_name || ' ' || d.last_name AS doctor_name
    FROM lab_orders lo
    LEFT JOIN patients p ON p.id = lo.patient_id
    LEFT JOIN doctors d ON d.id = lo.ordered_by_doctor_id
"#;

#[tauri::command]
pub async fn get_lab_orders(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    status_filter: Option<String>,
) -> Result<Vec<LabOrder>, String> {
    let _ = rbac::require(&session, Permission::LabView)?;
    let q = match status_filter.as_deref() {
        Some(s) if !s.is_empty() => format!("{} WHERE lo.status = $1 ORDER BY lo.ordered_at DESC", SELECT_ORDERS),
        _ => format!("{} ORDER BY lo.ordered_at DESC", SELECT_ORDERS),
    };
    let mut query = sqlx::query_as::<_, LabOrder>(&q);
    if let Some(s) = status_filter.filter(|s| !s.is_empty()) {
        query = query.bind(s);
    }
    query.fetch_all(pool.inner())
        .await
        .map_err(|e| format!("Get lab orders: {}", e))
}

#[tauri::command]
pub async fn create_lab_order(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    order: CreateLabOrder,
) -> Result<i32, String> {
    let s = rbac::require_strong(&session, pool.inner(), Permission::LabOrder).await?;
    if order.test_catalog_ids.is_empty() {
        return Err("At least one test must be selected.".to_string());
    }

    let mut tx = pool.begin().await.map_err(|e| crate::db::sanitize_db_error(&e))?;

    let row: (i32,) = sqlx::query_as(
        r#"INSERT INTO lab_orders (patient_id, encounter_id, ordered_by_doctor_id, ordered_by_user_id, status)
           VALUES ($1,$2,$3,$4,'ordered') RETURNING id"#,
    )
    .bind(order.patient_id)
    .bind(order.encounter_id)
    .bind(order.ordered_by_doctor_id)
    .bind(s.user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    for tc_id in &order.test_catalog_ids {
        sqlx::query("INSERT INTO lab_order_tests (lab_order_id, test_catalog_id) VALUES ($1,$2)")
            .bind(row.0).bind(tc_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| crate::db::sanitize_db_error(&e))?;
    }

    tx.commit().await.map_err(|e| crate::db::sanitize_db_error(&e))?;

    audit::for_session(pool.inner(), &s, "lab_order_create", "lab_orders",
        Some(&row.0.to_string()),
        Some(serde_json::json!({"patient_id": order.patient_id, "tests": order.test_catalog_ids}))).await;
    Ok(row.0)
}

// ── Results ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_lab_order_tests(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    lab_order_id: i32,
) -> Result<Vec<LabOrderTest>, String> {
    let _ = rbac::require_strong(&session, pool.inner(), Permission::LabView).await?;
    sqlx::query_as(
        r#"SELECT lot.id, lot.lab_order_id, lot.test_catalog_id, lot.result_value,
                  lot.result_unit, lot.result_abnormal_flag, lot.result_notes,
                  lot.completed_at, lot.completed_by_user_id,
                  lot.approval_status, lot.critical_acknowledged_at, lot.critical_acknowledged_by_user_id,
                  tc.name AS test_name, tc.code AS test_code, tc.normal_range
           FROM lab_order_tests lot
           JOIN lab_test_catalog tc ON tc.id = lot.test_catalog_id
           WHERE lot.lab_order_id = $1
           ORDER BY tc.name"#,
    )
    .bind(lab_order_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| format!("Get lab order tests: {}", e))
}

#[tauri::command]
pub async fn update_lab_result(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    result: UpdateLabResult,
) -> Result<(), String> {
    update_lab_result_core(pool.inner(), &session, result).await
}

/// Result-entry logic core (AERP Part G extraction pattern — Phase 9 moved
/// the body out from behind tauri::State so the notification emitters can
/// be integration-tested at command level).
pub async fn update_lab_result_core(
    pool: &PgPool,
    session_state: &SessionState,
    result: UpdateLabResult,
) -> Result<(), String> {
    let s = rbac::require_strong(session_state, pool, Permission::LabResultManage).await?;

    // Phase 6.2: result entry now goes through the approval workflow —
    // approval_status 'entered' (awaiting in-charge approval). Overwriting
    // an already-APPROVED result is a correction: it demotes the row back
    // to 'entered' with an 'amended' marker preserved in notes by the
    // approver, never silently (see approve_lab_result for the release path).
    let updated = sqlx::query(
        r#"UPDATE lab_order_tests SET
              result_value=$1, result_unit=$2, result_abnormal_flag=$3,
              result_notes=$4, completed_at=NOW(), completed_by_user_id=$5,
              approval_status = CASE WHEN approval_status = 'approved' THEN 'amended' ELSE 'entered' END
           WHERE id=$6"#,
    )
    .bind(&result.result_value)
    .bind(&result.result_unit)
    .bind(&result.result_abnormal_flag)
    .bind(&result.result_notes)
    .bind(s.user_id)
    .bind(result.id)
    .execute(pool)
    .await
    .map_err(|e| format!("Update lab result: {}", e))?;
    if updated.rows_affected() == 0 {
        return Err("Lab result row not found.".to_string());
    }

    // Order status: any completed test moves the order to 'resulted'
    // (awaiting approval). Full approval to 'approved' happens in
    // approve_lab_result once EVERY test is approved.
    sqlx::query(
        r#"UPDATE lab_orders SET status = 'resulted'
           WHERE id = (SELECT lab_order_id FROM lab_order_tests WHERE id = $1)
             AND status IN ('ordered', 'sampled')"#,
    )
    .bind(result.id)
    .execute(pool)
    .await
    .map_err(|e| format!("Update lab order status: {}", e))?;

    // Critical-value protocol step 1: a critical flag cannot be "approved"
    // until acknowledged (chk_lot_critical_release enforces at release),
    // and the alert is surfaced in-app immediately via the result row the
    // worklist polls. Audited with the flag so the escalation is traceable.
    if result.result_abnormal_flag.as_deref() == Some("critical") {
        let order: Option<(i32,)> = sqlx::query_as(
            "SELECT lab_order_id FROM lab_order_tests WHERE id = $1",
        )
        .bind(result.id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Lookup lab order: {}", e))?;
        audit::for_session(
            pool, &s, "lab_critical_value_entered", "lab_order_tests",
            Some(&result.id.to_string()),
            Some(serde_json::json!({
                "lab_order_id": order.map(|o| o.0),
                "flag": "critical",
            })),
        ).await;

        // Phase 9: push to the in-app notification center so every doctor
        // sees the escalation without opening the lab worklist. Broadcast
        // to the doctor role (no doctor→user mapping exists) + admins.
        // Best-effort — never fails the result entry.
        if let Some(order_id) = order.map(|o| o.0) {
            let info: Option<(String, String)> = sqlx::query_as(
                "SELECT COALESCE(p.first_name || ' ' || p.last_name, 'Patient'), \
                        COALESCE(tc.name, 'test') \
                 FROM lab_orders lo \
                 JOIN lab_order_tests lot ON lot.id = $1 \
                 JOIN lab_test_catalog tc ON tc.id = lot.test_catalog_id \
                 LEFT JOIN patients p ON p.id = lo.patient_id \
                 WHERE lo.id = $2",
            )
            .bind(result.id)
            .bind(order_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
            if let Some((patient, test)) = info {
                let title = format!("CRITICAL lab value: {} — {}", patient, test);
                let body = format!(
                    "A CRITICAL result was entered for {} (order #{}). Contact the ordering doctor immediately.",
                    patient, order_id
                );
            if let Err(e) = crate::commands::notifications::emit(
                pool,
                crate::commands::notifications::NotificationOut {
                    user_id: None,
                    role_target: Some("doctor".into()),
                    kind: "lab_critical".into(),
                    title,
                    body,
                    entity_type: Some("lab_order".into()),
                    entity_id: Some(order_id),
                },
            ).await {
                eprintln!("[HMS Lab] notification emit failed (non-fatal): {}", e);
            }
            }
        }
    }

    audit::for_session(pool, &s, "lab_result_update", "lab_order_tests",
        Some(&result.id.to_string()), None).await;
    Ok(())
}

/// Collect (or recollect) the sample for a lab order. Stamps the order
/// 'sampled' + the collecting user + a printable barcode
/// ("<order_id>-<first_test_row_id>" — unique per order, stable).
/// Phase 6.2 (SRS §2.4 sample collection tracking).
/// RBAC: LabResultManage (tech-level action). Audited.
#[tauri::command]
pub async fn collect_lab_sample(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    lab_order_id: i32,
) -> Result<String, String> {
    collect_lab_sample_core(pool.inner(), &session, lab_order_id).await
}

/// Sample-collection logic core (AERP Part G extraction pattern): guard +
/// state-rule UPDATE + audit, callable without a Tauri AppHandle for
/// command-level workflow tests (lab_tests.rs).
pub async fn collect_lab_sample_core(
    pool: &PgPool,
    session_state: &SessionState,
    lab_order_id: i32,
) -> Result<String, String> {
    let s = rbac::require_strong(session_state, pool, Permission::LabResultManage).await?;

    // Only an order still awaiting sampling can be collected; a recollect
    // after a lost/damaged sample is expressed by resetting to 'ordered'
    // (audited) and collecting again, so this guard is a hard state rule.
    let row: Option<(String,)> = sqlx::query_as(
        r#"UPDATE lab_orders SET
              status = 'sampled', sampled_at = NOW(), sampled_by_user_id = $1,
              sample_barcode = $2 || '-' || (
                SELECT MIN(id)::TEXT FROM lab_order_tests WHERE lab_order_id = $3
              )
           WHERE id = $3 AND status = 'ordered'
           RETURNING sample_barcode"#,
    )
    .bind(s.user_id)
    .bind(lab_order_id.to_string())
    .bind(lab_order_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;
    let barcode: String = row
        .ok_or_else(|| "No lab order awaiting sample collection (status must be 'ordered').".to_string())?
        .0;

    audit::for_session(pool, &s, "lab_sample_collected", "lab_orders",
        Some(&lab_order_id.to_string()),
        Some(serde_json::json!({"barcode": barcode}))).await;
    Ok(barcode)
}

/// Approve (release) a lab result row. Requires LabApprove — held by the
/// lab in-charge, doctors, and super admin, but NOT by plain
/// LabResultManage holders, so a tech cannot self-approve their own
/// entries. If the result is flagged critical, `critical_acknowledged`
/// must be true — the approver confirms the critical-value call was made
/// (phone call to the ordering doctor per protocol) before release.
/// Phase 6.2 (SRS §2.4 result approval + critical alerting).
/// RBAC: LabApprove. Audited.
#[tauri::command]
pub async fn approve_lab_result(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    lab_order_test_id: i32,
    critical_acknowledged: bool,
) -> Result<(), String> {
    approve_lab_result_core(pool.inner(), &session, lab_order_test_id, critical_acknowledged).await
}

/// Result-approval logic core (AERP Part G extraction pattern) — see
/// `collect_lab_sample_core` for the rationale.
pub async fn approve_lab_result_core(
    pool: &PgPool,
    session_state: &SessionState,
    lab_order_test_id: i32,
    critical_acknowledged: bool,
) -> Result<(), String> {
    let s = rbac::require_strong(session_state, pool, Permission::LabApprove).await?;

    let row: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT approval_status, result_abnormal_flag FROM lab_order_tests WHERE id = $1",
    )
    .bind(lab_order_test_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;
    let (status, flag) = row
        .ok_or_else(|| "Lab result row not found.".to_string())?;

    if status != "entered" && status != "amended" {
        return Err(format!("This result is not awaiting approval (status: {}). Only entered results can be approved.", status));
    }
    if flag.as_deref() == Some("critical") && !critical_acknowledged {
        return Err("This result is flagged CRITICAL. You must acknowledge that the ordering doctor has been contacted before releasing it.".to_string());
    }

    // Stamp release (and the critical acknowledgment when applicable).
    sqlx::query(
        r#"UPDATE lab_order_tests SET
              approval_status = 'approved',
              critical_acknowledged_at = CASE
                  WHEN result_abnormal_flag = 'critical' AND critical_acknowledged_at IS NULL
                  THEN NOW() ELSE critical_acknowledged_at END,
              critical_acknowledged_by_user_id = CASE
                  WHEN result_abnormal_flag = 'critical' AND critical_acknowledged_by_user_id IS NULL
                  THEN $1 ELSE critical_acknowledged_by_user_id END
           WHERE id = $2"#,
    )
    .bind(s.user_id)
    .bind(lab_order_test_id)
    .execute(pool)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    // Order is fully approved once EVERY test row is approved.
    sqlx::query(
        r#"UPDATE lab_orders SET status = 'approved', approved_at = NOW(), approved_by_user_id = $1
           WHERE id = (SELECT lab_order_id FROM lab_order_tests WHERE id = $2)
             AND NOT EXISTS (
               SELECT 1 FROM lab_order_tests
               WHERE lab_order_id = (SELECT lab_order_id FROM lab_order_tests WHERE id = $2)
                 AND (approval_status IS NULL OR approval_status NOT IN ('approved','amended'))
             )"#,
    )
    .bind(s.user_id)
    .bind(lab_order_test_id)
    .execute(pool)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    // Phase 9: notify the ordering side that the result is released
    // (best-effort — never fails the approval).
    {
        let info: Option<(i32, String, String)> = sqlx::query_as(
            "SELECT lo.id, COALESCE(p.first_name || ' ' || p.last_name, 'Patient'), \
                    COALESCE(tc.name, 'test') \
             FROM lab_order_tests lot \
             JOIN lab_orders lo ON lo.id = lot.lab_order_id \
             JOIN lab_test_catalog tc ON tc.id = lot.test_catalog_id \
             LEFT JOIN patients p ON p.id = lo.patient_id \
             WHERE lot.id = $1",
        )
        .bind(lab_order_test_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
        if let Some((order_id, patient, test)) = info {
            let title = format!("Lab result released: {} — {}", patient, test);
            let body = format!(
                "{}'s {} result on order #{} has been approved and released.",
                patient, test, order_id
            );
            if let Err(e) = crate::commands::notifications::emit(
                pool,
                crate::commands::notifications::NotificationOut {
                    user_id: None,
                    role_target: Some("doctor".into()),
                    kind: "lab_released".into(),
                    title,
                    body,
                    entity_type: Some("lab_order".into()),
                    entity_id: Some(order_id),
                },
            ).await {
                eprintln!("[HMS Lab] notification emit failed (non-fatal): {}", e);
            }
        }
    }

    audit::for_session(pool, &s, "lab_result_approved", "lab_order_tests",
        Some(&lab_order_test_id.to_string()),
        Some(serde_json::json!({
            "critical": flag.as_deref() == Some("critical"),
            "acknowledged": critical_acknowledged,
        }))).await;
    Ok(())
}
