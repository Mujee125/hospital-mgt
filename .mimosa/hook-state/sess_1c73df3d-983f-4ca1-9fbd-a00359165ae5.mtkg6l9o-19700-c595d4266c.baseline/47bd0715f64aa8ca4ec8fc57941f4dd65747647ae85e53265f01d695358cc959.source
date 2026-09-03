//! Billing — bills, line items, payments. RBAC-guarded + audited.
//!
//! Money is stored as NUMERIC(14,2) and round-tripped via `rust_decimal`.
//! Bill `net_amount` is recomputed server-side from items + discount + tax so a
//! tampered client payload cannot inflate or deflate a bill. Payment status
//! rolls up automatically: when the sum of payments ≥ net_amount the bill
//! becomes `paid`; a partial payment leaves it `partial`.

use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use sqlx::PgPool;

use crate::audit;
use crate::models::{Bill, BillItem, CreateBill, CreatePayment, Payment};
use crate::rbac::{self, Permission, SessionState};

fn dec(f: f64) -> Decimal {
    Decimal::from_f64(f).unwrap_or_default()
}

const SELECT_BILLS: &str = r#"
    SELECT b.id, b.patient_id, b.encounter_id, b.ipd_admission_id, b.bill_number,
           b.bill_type, b.total_amount, b.discount, b.tax, b.net_amount, b.status,
           b.created_by_user_id, b.created_at, b.updated_at,
           p.first_name || ' ' || p.last_name AS patient_name,
           COALESCE((SELECT SUM(amount) FROM payments WHERE bill_id = b.id), 0) AS amount_paid
    FROM bills b
    LEFT JOIN patients p ON p.id = b.patient_id
"#;

#[tauri::command]
pub async fn get_bills(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    status_filter: Option<String>,
) -> Result<Vec<Bill>, String> {
    let _ = rbac::require(&session, Permission::BillingView)?;
    let q = match status_filter.as_deref() {
        Some(s) if !s.is_empty() => format!("{} WHERE b.status = $1 ORDER BY b.created_at DESC", SELECT_BILLS),
        _ => format!("{} ORDER BY b.created_at DESC", SELECT_BILLS),
    };
    let mut query = sqlx::query_as::<_, Bill>(&q);
    if let Some(s) = status_filter.filter(|s| !s.is_empty()) {
        query = query.bind(s);
    }
    query.fetch_all(pool.inner())
        .await
        .map_err(|e| format!("Get bills: {}", e))
}

#[tauri::command]
pub async fn get_bill(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    id: i32,
) -> Result<Bill, String> {
    let _ = rbac::require(&session, Permission::BillingView)?;
    let q = format!("{} WHERE b.id = $1", SELECT_BILLS);
    sqlx::query_as::<_, Bill>(&q)
        .bind(id)
        .fetch_one(pool.inner())
        .await
        .map_err(|e| format!("Bill not found: {}", e))
}

#[tauri::command]
pub async fn get_bill_items(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    bill_id: i32,
) -> Result<Vec<BillItem>, String> {
    let _ = rbac::require(&session, Permission::BillingView)?;
    sqlx::query_as("SELECT * FROM bill_items WHERE bill_id = $1 ORDER BY id")
        .bind(bill_id)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| format!("Get bill items: {}", e))
}

#[tauri::command]
pub async fn create_bill(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    bill: CreateBill,
) -> Result<i32, String> {
    let s = rbac::require_strong(&session, pool.inner(), Permission::BillingCreate).await?;
    if bill.items.is_empty() {
        return Err("A bill must contain at least one line item.".to_string());
    }

    // Server-side totals (client cannot dictate the net amount).
    let mut total = Decimal::ZERO;
    for it in &bill.items {
        let line_total = dec(it.quantity) * dec(it.unit_price);
        total += line_total;
    }
    let discount = dec(bill.discount.unwrap_or(0.0));
    let tax = dec(bill.tax.unwrap_or(0.0));
    let net = (total - discount) + tax;
    if net < Decimal::ZERO {
        return Err("Bill net amount cannot be negative.".to_string());
    }

    let mut tx = pool.begin().await.map_err(|e| crate::db::sanitize_db_error(&e))?;

    // Generate a human-readable, collision-resistant bill number.
    let bill_number: (String,) = sqlx::query_as(
        "SELECT TO_CHAR(NOW(),'YYYYMMDD') || '-' || LPAD((COALESCE((SELECT COUNT(*) FROM bills WHERE created_at::date = CURRENT_DATE),0)+1)::text, 4, '0')",
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    let row: (i32,) = sqlx::query_as(
        r#"INSERT INTO bills
              (patient_id, encounter_id, ipd_admission_id, bill_number, bill_type,
               total_amount, discount, tax, net_amount, status, created_by_user_id)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,'unpaid',$10) RETURNING id"#,
    )
    .bind(bill.patient_id)
    .bind(bill.encounter_id)
    .bind(bill.ipd_admission_id)
    .bind(&bill_number.0)
    .bind(bill.bill_type.as_deref().unwrap_or("opd"))
    .bind(total)
    .bind(discount)
    .bind(tax)
    .bind(net)
    .bind(s.user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    for it in &bill.items {
        let line_total = dec(it.quantity) * dec(it.unit_price);
        sqlx::query(
            r#"INSERT INTO bill_items (bill_id, item_type, description, quantity, unit_price, total, reference_id)
               VALUES ($1,$2,$3,$4,$5,$6,$7)"#,
        )
        .bind(row.0)
        .bind(&it.item_type)
        .bind(&it.description)
        .bind(dec(it.quantity))
        .bind(dec(it.unit_price))
        .bind(line_total)
        .bind(it.reference_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;
    }

    tx.commit().await.map_err(|e| crate::db::sanitize_db_error(&e))?;

    audit::for_session(pool.inner(), &s, "bill_create", "bills",
        Some(&row.0.to_string()),
        Some(serde_json::json!({"bill_number": bill_number.0, "net": net.to_string()}))).await;
    Ok(row.0)
}

#[tauri::command]
pub async fn record_payment(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    payment: CreatePayment,
) -> Result<i32, String> {
    let s = rbac::require_strong(&session, pool.inner(), Permission::PaymentsManage).await?;
    // IPC-07: validate the payment amount BEFORE any DB work. The previous
    // check `payment.amount <= 0.0` rejected negatives and zero but let
    // `NaN` and `Infinity` through:
    //   - `f64::NaN <= 0.0` is `false` (NaN compares unordered to everything),
    //     so NaN passed the gate and reached `Decimal::from_f64(NaN)`,
    //     which returns `None` → `unwrap_or_default()` → `Decimal::ZERO`.
    //     The payment was then recorded with amount = 0, silently losing
    //     the operator's intended payment.
    //   - `f64::INFINITY <= 0.0` is `false`, so Infinity passed too, then
    //     `Decimal::from_f64(Infinity)` returns `None` → also ZERO.
    //
    // `is_finite()` rejects NaN, +Infinity, and -Infinity in one call;
    // combined with `> 0.0` this fully constrains the input to a
    // positive, finite amount before it reaches the DB layer.
    if !payment.amount.is_finite() || payment.amount <= 0.0 {
        return Err("Invalid payment amount.".to_string());
    }

    let mut tx = pool.begin().await.map_err(|e| crate::db::sanitize_db_error(&e))?;

    let row: (i32,) = sqlx::query_as(
        r#"INSERT INTO payments (bill_id, amount, payment_method, reference_number, received_by_user_id)
           VALUES ($1,$2,$3,$4,$5) RETURNING id"#,
    )
    .bind(payment.bill_id)
    .bind(dec(payment.amount))
    .bind(payment.payment_method.as_deref().unwrap_or("cash"))
    .bind(&payment.reference_number)
    .bind(s.user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    // Roll up bill status: paid if cumulative payments cover net, else partial.
    sqlx::query(
        r#"UPDATE bills SET status =
             CASE WHEN (SELECT COALESCE(SUM(amount),0) FROM payments WHERE bill_id = $1) >= net_amount
                  THEN 'paid' ELSE 'partial' END,
             updated_at = NOW()
           WHERE id = $1"#,
    )
    .bind(payment.bill_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    tx.commit().await.map_err(|e| crate::db::sanitize_db_error(&e))?;

    audit::for_session(pool.inner(), &s, "payment_record", "payments",
        Some(&row.0.to_string()),
        Some(serde_json::json!({"bill_id": payment.bill_id, "amount": payment.amount}))).await;
    Ok(row.0)
}

#[tauri::command]
pub async fn get_payments(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    bill_id: i32,
) -> Result<Vec<Payment>, String> {
    let _ = rbac::require(&session, Permission::BillingView)?;
    sqlx::query_as("SELECT * FROM payments WHERE bill_id = $1 ORDER BY paid_at")
        .bind(bill_id)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| format!("Get payments: {}", e))
}
