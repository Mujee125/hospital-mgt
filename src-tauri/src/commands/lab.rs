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
    let s = rbac::require(&session, Permission::LabOrder)?;
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
    let _ = rbac::require(&session, Permission::LabView)?;
    sqlx::query_as(
        r#"SELECT lot.id, lot.lab_order_id, lot.test_catalog_id, lot.result_value,
                  lot.result_unit, lot.result_abnormal_flag, lot.result_notes,
                  lot.completed_at, lot.completed_by_user_id,
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
    let s = rbac::require(&session, Permission::LabResultManage)?;

    // Update the single test row and stamp completion.
    sqlx::query(
        r#"UPDATE lab_order_tests SET
              result_value=$1, result_unit=$2, result_abnormal_flag=$3,
              result_notes=$4, completed_at=NOW(), completed_by_user_id=$5
           WHERE id=$6"#,
    )
    .bind(&result.result_value)
    .bind(&result.result_unit)
    .bind(&result.result_abnormal_flag)
    .bind(&result.result_notes)
    .bind(s.user_id)
    .bind(result.id)
    .execute(pool.inner())
    .await
    .map_err(|e| format!("Update lab result: {}", e))?;

    // If every test in the order is now completed, mark the order completed.
    sqlx::query(
        r#"UPDATE lab_orders SET status = 'completed'
           WHERE id = (SELECT lab_order_id FROM lab_order_tests WHERE id = $1)
             AND NOT EXISTS (
               SELECT 1 FROM lab_order_tests
               WHERE lab_order_id = (SELECT lab_order_id FROM lab_order_tests WHERE id = $1)
                 AND completed_at IS NULL
             )"#,
    )
    .bind(result.id)
    .execute(pool.inner())
    .await
    .ok();

    audit::for_session(pool.inner(), &s, "lab_result_update", "lab_order_tests",
        Some(&result.id.to_string()), None).await;
    Ok(())
}
