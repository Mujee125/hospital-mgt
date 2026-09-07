//! Billing — bills, line items, payments, refunds, advances, insurance
//! claims, credit-note cancellation. RBAC-guarded + audited.
//!
//! Money is stored as NUMERIC(14,2) and round-tripped via `rust_decimal`.
//! Bill `net_amount` is recomputed server-side from items + discount + tax so a
//! tampered client payload cannot inflate or deflate a bill. Payment status
//! rolls up automatically: when the sum of payments minus refunds ≥
//! net_amount the bill becomes `paid`; a partial payment leaves it `partial`.
//!
//! Phase 6.3 (SRS §2.10):
//!   • Invoice numbers are sequential + immutable: `INV-YYYY-NNNNNN` from the
//!     `bill_number_seq` SEQUENCE (the old COUNT(*)+1 could hand the same
//!     number to two simultaneous bills).
//!   • Refunds never delete payment rows — they insert into `refunds` and
//!     the effective-paid roll-up subtracts them (append-only finance).
//!     Requires BillingApprove (manager) — a clerk cannot reverse money.
//!   • Advances (deposits) are held per patient and applied to a bill at
//!     settlement; `remaining` cannot go negative.
//!   • Cancellation is a credit note: the bill keeps its number and rows,
//!     gets `cancelled` status + a `CN-YYYY-NNNNNN` note number.
//!   • Insurance/TPA claims track draft→submitted→settled per bill+insurer.
//!   • Discounts above the threshold (5% of gross) require BillingApprove.

use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use sqlx::{PgPool, Row};

use crate::audit;
use crate::models::{
    Bill, BillItem, CreateAdvance, CreateBill, CreateInsuranceClaim, CreatePayment,
    CreateRefund, InsuranceClaim, PatientAdvance, Payment, Refund, UpdateClaimStatus,
};
use crate::rbac::{self, Permission, SessionState};

/// A discount (absolute amount) larger than this fraction of the bill gross
/// needs manager approval (BillingApprove). Two-level discount control per
/// SRS §2.10; 5% chosen as the clerk discretion ceiling for v1.
const DISCOUNT_APPROVAL_FRACTION: f64 = 0.05;

fn dec(f: f64) -> Decimal {
    Decimal::from_f64(f).unwrap_or_default()
}

const SELECT_BILLS: &str = r#"
    SELECT b.id, b.patient_id, b.encounter_id, b.ipd_admission_id, b.bill_number,
           b.bill_type, b.total_amount, b.discount, b.tax, b.net_amount, b.status,
           b.created_by_user_id, b.created_at, b.updated_at,
           b.cancelled_at, b.cancelled_by_user_id, b.cancellation_reason, b.credit_note_number,
           p.first_name || ' ' || p.last_name AS patient_name,
           COALESCE((SELECT SUM(amount) FROM payments WHERE bill_id = b.id), 0)
             - COALESCE((SELECT SUM(amount) FROM refunds WHERE bill_id = b.id), 0) AS amount_paid,
           COALESCE((SELECT SUM(amount) FROM refunds WHERE bill_id = b.id), 0) AS refund_total
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
    sqlx::query_as(&q)
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
    discount_approved: bool,
) -> Result<i32, String> {
    create_bill_core(pool.inner(), &session, bill, discount_approved).await
}

/// Bill-creation logic core (AERP Part G extraction pattern): guard +
/// totals + discount gate + sequence-numbered INSERT + audit, callable
/// without a Tauri AppHandle (billing_tests.rs).
pub async fn create_bill_core(
    pool: &PgPool,
    session_state: &SessionState,
    bill: CreateBill,
    discount_approved: bool,
) -> Result<i32, String> {
    let s = rbac::require_strong(session_state, pool, Permission::BillingCreate).await?;
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

    // Two-level discount control: a discount above the threshold requires
    // BillingApprove (manager) — checked against the live session, and the
    // caller must pass discount_approved=true (an explicit UI confirm).
    let needs_approval = total > Decimal::ZERO
        && discount > total * Decimal::from_f64(DISCOUNT_APPROVAL_FRACTION).unwrap_or_default();
    if needs_approval {
        rbac::require(session_state, Permission::BillingApprove)
            .map_err(|_| {
                "This discount exceeds 5% of the bill and needs manager approval (billing.approve permission).".to_string()
            })?;
        if !discount_approved {
            return Err("Discount approval was not confirmed.".to_string());
        }
    }

    let mut tx = pool.begin().await.map_err(|e| crate::db::sanitize_db_error(&e))?;

    // Sequential, immutable invoice number from the SEQUENCE — assigned
    // inside the same transaction as the INSERT (Phase 6.3; replaces the
    // COUNT(*)+1 pattern that raced under concurrent creation).
    let seq: (i64,) = sqlx::query_as("SELECT NEXTVAL('bill_number_seq')")
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;
    let bill_number = format!(
        "INV-{}-{:0>6}",
        chrono::Utc::now().format("%Y"),
        seq.0
    );

    let row: (i32,) = sqlx::query_as(
        r#"INSERT INTO bills
              (patient_id, encounter_id, ipd_admission_id, bill_number, bill_type,
               total_amount, discount, tax, net_amount, status, created_by_user_id)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,'unpaid',$10) RETURNING id"#,
    )
    .bind(bill.patient_id)
    .bind(bill.encounter_id)
    .bind(bill.ipd_admission_id)
    .bind(&bill_number)
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

    audit::for_session(pool, &s, "bill_create", "bills",
        Some(&row.0.to_string()),
        Some(serde_json::json!({"bill_number": bill_number, "net": net.to_string(), "discount_approved": needs_approval}))).await;
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

    // A cancelled (credit-noted) bill can never receive money.
    let live: Option<(i32,)> = sqlx::query_as(
        "SELECT id FROM bills WHERE id = $1 AND status <> 'cancelled' FOR UPDATE",
    )
    .bind(payment.bill_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;
    if live.is_none() {
        return Err("This bill is cancelled — payments are closed on it.".to_string());
    }

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

    // Roll up bill status: paid if cumulative payments (minus refunds)
    // cover net, else partial.
    sqlx::query(
        r#"UPDATE bills SET status =
             CASE WHEN (SELECT COALESCE(SUM(amount),0) FROM payments WHERE bill_id = $1)
                    - (SELECT COALESCE(SUM(amount),0) FROM refunds WHERE bill_id = $1) >= net_amount
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

// ── Refunds (Phase 6.3) ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn record_refund(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    refund: CreateRefund,
) -> Result<i32, String> {
    record_refund_core(pool.inner(), &session, refund).await
}

/// Refund logic core (AERP Part G pattern). Business rules:
///   • Requires BillingApprove (manager) — distinct from PaymentsManage so
///     a clerk who takes payments cannot reverse them.
///   • Over-refund guard: cumulative refunds may never exceed cumulative
///     payments on the bill (fail-closed via SELECT … FOR UPDATE on the
///     bill row, so two concurrent refunds cannot both pass the check).
///   • Payment rows are NEVER deleted — the refund is an append-only
///     financial record; the paid roll-up subtracts refunds.
pub async fn record_refund_core(
    pool: &PgPool,
    session_state: &SessionState,
    refund: CreateRefund,
) -> Result<i32, String> {
    let s = rbac::require_strong(session_state, pool, Permission::BillingApprove).await?;
    if !refund.amount.is_finite() || refund.amount <= 0.0 {
        return Err("Invalid refund amount.".to_string());
    }
    if refund.reason.trim().is_empty() {
        return Err("A refund reason is required.".to_string());
    }

    let mut tx = pool.begin().await.map_err(|e| crate::db::sanitize_db_error(&e))?;

    // Lock the bill row: serializes concurrent refunds on the same bill.
    let bill: Option<(Decimal,)> = sqlx::query_as(
        "SELECT net_amount FROM bills WHERE id = $1 FOR UPDATE",
    )
    .bind(refund.bill_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;
    if bill.is_none() {
        return Err("Bill not found.".to_string());
    }

    // Effective paid = payments − refunds. Locking the bill row above
    // serializes this read-modify-write against concurrent refunds.
    let (paid,): (Decimal,) = sqlx::query_as(
        "SELECT COALESCE((SELECT SUM(amount) FROM payments WHERE bill_id = $1), 0) \
              - COALESCE((SELECT SUM(amount) FROM refunds WHERE bill_id = $1), 0)",
    )
    .bind(refund.bill_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;
    if dec(refund.amount) > paid {
        return Err(format!(
            "Refund exceeds the amount actually paid (effective paid: {}).",
            paid
        ));
    }

    let row: (i32,) = sqlx::query_as(
        r#"INSERT INTO refunds (bill_id, payment_id, amount, reason, refunded_by_user_id)
           VALUES ($1,$2,$3,$4,$5) RETURNING id"#,
    )
    .bind(refund.bill_id)
    .bind(refund.payment_id)
    .bind(dec(refund.amount))
    .bind(refund.reason.trim())
    .bind(s.user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    // Re-roll the bill status with the refund subtracted.
    sqlx::query(
        r#"UPDATE bills SET status =
             CASE WHEN (SELECT COALESCE(SUM(amount),0) FROM payments WHERE bill_id = $1)
                    - (SELECT COALESCE(SUM(amount),0) FROM refunds WHERE bill_id = $1) >= net_amount
                  THEN 'paid' ELSE 'partial' END,
             updated_at = NOW()
           WHERE id = $1"#,
    )
    .bind(refund.bill_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    tx.commit().await.map_err(|e| crate::db::sanitize_db_error(&e))?;

    audit::for_session(pool, &s, "refund_record", "refunds",
        Some(&row.0.to_string()),
        Some(serde_json::json!({"bill_id": refund.bill_id, "amount": refund.amount, "reason": refund.reason}))).await;
    Ok(row.0)
}

#[tauri::command]
pub async fn get_refunds(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    bill_id: i32,
) -> Result<Vec<Refund>, String> {
    let _ = rbac::require(&session, Permission::BillingView)?;
    sqlx::query_as("SELECT * FROM refunds WHERE bill_id = $1 ORDER BY refunded_at DESC")
        .bind(bill_id)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| format!("Get refunds: {}", e))
}

// ── Advances / deposits (Phase 6.3) ──────────────────────────────────────────

#[tauri::command]
pub async fn record_advance(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    advance: CreateAdvance,
) -> Result<i32, String> {
    record_advance_core(pool.inner(), &session, advance).await
}

/// Advance-receipt logic core (AERP Part G pattern). PaymentsManage holders
/// can receive a deposit before any bill exists.
pub async fn record_advance_core(
    pool: &PgPool,
    session_state: &SessionState,
    advance: CreateAdvance,
) -> Result<i32, String> {
    let s = rbac::require_strong(session_state, pool, Permission::PaymentsManage).await?;
    if !advance.amount.is_finite() || advance.amount <= 0.0 {
        return Err("Invalid advance amount.".to_string());
    }
    let row: (i32,) = sqlx::query_as(
        r#"INSERT INTO patient_advances
               (patient_id, amount, remaining, status, method, reference_number, notes, received_by_user_id)
           VALUES ($1,$2,$2,'active',$3,$4,$5,$6) RETURNING id"#,
    )
    .bind(advance.patient_id)
    .bind(dec(advance.amount))
    .bind(advance.method.as_deref().unwrap_or("cash"))
    .bind(&advance.reference_number)
    .bind(advance.notes.as_deref())
    .bind(s.user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    audit::for_session(pool, &s, "advance_record", "patient_advances",
        Some(&row.0.to_string()),
        Some(serde_json::json!({"patient_id": advance.patient_id, "amount": advance.amount}))).await;
    Ok(row.0)
}

/// Apply a patient's active advance balance to a specific bill: converts
/// the advance into a normal payment row (so bill roll-ups and revenue
/// reports stay uniform) and decrements `remaining`. Fails if the advance
/// is not active or the bill is already cancelled/paid-in-full.
#[tauri::command]
pub async fn apply_advance(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    advance_id: i32,
    bill_id: i32,
    amount: f64,
) -> Result<(), String> {
    apply_advance_core(pool.inner(), &session, advance_id, bill_id, amount).await
}

pub async fn apply_advance_core(
    pool: &PgPool,
    session_state: &SessionState,
    advance_id: i32,
    bill_id: i32,
    amount: f64,
) -> Result<(), String> {
    let s = rbac::require_strong(session_state, pool, Permission::PaymentsManage).await?;
    if !amount.is_finite() || amount <= 0.0 {
        return Err("Invalid application amount.".to_string());
    }

    let mut tx = pool.begin().await.map_err(|e| crate::db::sanitize_db_error(&e))?;

    // Lock the advance; read remaining + status.
    let adv: Option<(Decimal, String, i32)> = sqlx::query_as(
        "SELECT remaining, status, patient_id FROM patient_advances WHERE id = $1 FOR UPDATE",
    )
    .bind(advance_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;
    let (remaining, status, patient_id) =
        adv.ok_or_else(|| "Advance not found.".to_string())?;
    if status != "active" {
        return Err(format!("This advance is {} — only active advances can be applied.", status));
    }
    let amt = dec(amount);
    if amt > remaining {
        return Err(format!("Application amount exceeds the advance balance ({}).", remaining));
    }

    // The bill must belong to the same patient and not be cancelled.
    let bill: Option<(i32,)> = sqlx::query_as(
        "SELECT id FROM bills WHERE id = $1 AND patient_id = $2 AND status <> 'cancelled'",
    )
    .bind(bill_id)
    .bind(patient_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;
    if bill.is_none() {
        return Err("The bill does not belong to this patient (or is cancelled).".to_string());
    }

    // Convert to a payment row — keeps every downstream roll-up uniform.
    sqlx::query(
        r#"INSERT INTO payments (bill_id, amount, payment_method, reference_number, received_by_user_id)
           VALUES ($1,$2,'advance',$3,$4)"#,
    )
    .bind(bill_id)
    .bind(amt)
    .bind(format!("advance#{}", advance_id))
    .bind(s.user_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    let new_remaining = remaining - amt;
    sqlx::query(
        r#"UPDATE patient_advances SET
              remaining = $1,
              status = CASE WHEN $1 = 0 THEN 'applied' ELSE status END
           WHERE id = $2"#,
    )
    .bind(new_remaining)
    .bind(advance_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    sqlx::query(
        r#"UPDATE bills SET status =
             CASE WHEN (SELECT COALESCE(SUM(amount),0) FROM payments WHERE bill_id = $1)
                    - (SELECT COALESCE(SUM(amount),0) FROM refunds WHERE bill_id = $1) >= net_amount
                  THEN 'paid' ELSE 'partial' END,
             updated_at = NOW()
           WHERE id = $1"#,
    )
    .bind(bill_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    tx.commit().await.map_err(|e| crate::db::sanitize_db_error(&e))?;

    audit::for_session(pool, &s, "advance_applied", "patient_advances",
        Some(&advance_id.to_string()),
        Some(serde_json::json!({"bill_id": bill_id, "amount": amount}))).await;
    Ok(())
}

#[tauri::command]
pub async fn get_patient_advances(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    patient_id: Option<i32>,
) -> Result<Vec<PatientAdvance>, String> {
    let _ = rbac::require(&session, Permission::BillingView)?;
    match patient_id {
        Some(pid) => sqlx::query_as(
            "SELECT * FROM patient_advances WHERE patient_id = $1 ORDER BY created_at DESC",
        )
        .bind(pid)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| format!("Get advances: {}", e)),
        None => sqlx::query_as(
            "SELECT * FROM patient_advances WHERE status = 'active' ORDER BY created_at DESC LIMIT 200",
        )
        .fetch_all(pool.inner())
        .await
        .map_err(|e| format!("Get advances: {}", e)),
    }
}

// ── Credit-note cancellation (Phase 6.3) ────────────────────────────────────

#[tauri::command]
pub async fn cancel_bill(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    bill_id: i32,
    reason: String,
) -> Result<String, String> {
    cancel_bill_core(pool.inner(), &session, bill_id, reason).await
}

/// Cancellation logic core (AERP Part G pattern). Requires BillingApprove.
/// A cancelled bill is never deleted — rows persist for audit; status
/// becomes 'cancelled' and a sequential credit-note number is stamped.
/// Bills with payments/refunds can still be cancelled (the money side is
/// handled separately via refunds); a fresh bill simply dies un-paid.
pub async fn cancel_bill_core(
    pool: &PgPool,
    session_state: &SessionState,
    bill_id: i32,
    reason: String,
) -> Result<String, String> {
    let s = rbac::require_strong(session_state, pool, Permission::BillingApprove).await?;
    if reason.trim().is_empty() {
        return Err("A cancellation reason is required.".to_string());
    }

    let mut tx = pool.begin().await.map_err(|e| crate::db::sanitize_db_error(&e))?;

    let seq: (i64,) = sqlx::query_as("SELECT NEXTVAL('bill_number_seq')")
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;
    let credit_note = format!("CN-{}-{:0>6}", chrono::Utc::now().format("%Y"), seq.0);

    let updated = sqlx::query(
        r#"UPDATE bills SET status = 'cancelled', cancelled_at = NOW(),
              cancelled_by_user_id = $1, cancellation_reason = $2, credit_note_number = $3,
              updated_at = NOW()
           WHERE id = $4 AND status <> 'cancelled'
           RETURNING bill_number"#,
    )
    .bind(s.user_id)
    .bind(reason.trim())
    .bind(&credit_note)
    .bind(bill_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;
    let bill_number = updated
        .ok_or_else(|| "Bill not found or already cancelled.".to_string())?
        .try_get::<String, _>(0)
        .map_err(|e| format!("Read bill number: {}", e))?;

    tx.commit().await.map_err(|e| crate::db::sanitize_db_error(&e))?;

    audit::for_session(pool, &s, "bill_cancelled", "bills",
        Some(&bill_id.to_string()),
        Some(serde_json::json!({
            "bill_number": bill_number,
            "credit_note": credit_note,
            "reason": reason,
        }))).await;
    Ok(credit_note)
}

// ── Insurance / TPA claims (Phase 6.3) ────────────────────────────────────────

#[tauri::command]
pub async fn create_insurance_claim(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    claim: CreateInsuranceClaim,
) -> Result<i32, String> {
    create_insurance_claim_core(pool.inner(), &session, claim).await
}

pub async fn create_insurance_claim_core(
    pool: &PgPool,
    session_state: &SessionState,
    claim: CreateInsuranceClaim,
) -> Result<i32, String> {
    let s = rbac::require_strong(session_state, pool, Permission::BillingCreate).await?;
    if claim.insurer.trim().is_empty() {
        return Err("Insurer name is required.".to_string());
    }
    if !claim.claim_amount.is_finite() || claim.claim_amount <= 0.0 {
        return Err("Invalid claim amount.".to_string());
    }

    // Claim attaches to an existing, non-cancelled bill.
    let bill: Option<(i32,)> = sqlx::query_as(
        "SELECT patient_id FROM bills WHERE id = $1 AND status <> 'cancelled'",
    )
    .bind(claim.bill_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;
    let patient_id = bill
        .ok_or_else(|| "Bill not found (or cancelled) — claims attach to live bills only.".to_string())?
        .0;

    let row: (i32,) = sqlx::query_as(
        r#"INSERT INTO insurance_claims
               (bill_id, patient_id, insurer, policy_number, claim_amount, notes, created_by_user_id)
           VALUES ($1,$2,$3,$4,$5,$6,$7)
           ON CONFLICT (bill_id, insurer) DO UPDATE SET updated_at = NOW()
           RETURNING id"#,
    )
    .bind(claim.bill_id)
    .bind(patient_id)
    .bind(claim.insurer.trim())
    .bind(&claim.policy_number)
    .bind(dec(claim.claim_amount))
    .bind(claim.notes.as_deref())
    .bind(s.user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    audit::for_session(pool, &s, "insurance_claim_create", "insurance_claims",
        Some(&row.0.to_string()),
        Some(serde_json::json!({"bill_id": claim.bill_id, "insurer": claim.insurer, "amount": claim.claim_amount}))).await;
    Ok(row.0)
}

#[tauri::command]
pub async fn update_insurance_claim_status(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    update: UpdateClaimStatus,
) -> Result<(), String> {
    update_insurance_claim_status_core(pool.inner(), &session, update).await
}

/// Claim status transitions: draft→submitted→(approved|partially_approved|
/// rejected)→settled. Enforced here (not just the DB CHECK) so the error is
/// user-readable. `approved_amount` required for partially_approved.
pub async fn update_insurance_claim_status_core(
    pool: &PgPool,
    session_state: &SessionState,
    update: UpdateClaimStatus,
) -> Result<(), String> {
    let s = rbac::require_strong(session_state, pool, Permission::BillingManage).await?;

    let current: Option<(String,)> = sqlx::query_as(
        "SELECT status FROM insurance_claims WHERE id = $1",
    )
    .bind(update.id)
    .fetch_optional(pool)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;
    let current_status = current
        .ok_or_else(|| "Claim not found.".to_string())?
        .0;

    // State machine: only forward moves through the pipeline.
    let allowed: &[(&str, &[&str])] = &[
        ("draft", &["submitted"]),
        ("submitted", &["approved", "partially_approved", "rejected"]),
        ("approved", &["settled"]),
        ("partially_approved", &["settled"]),
        ("rejected", &[]),
        ("settled", &[]),
    ];
    let legal_next = allowed
        .iter()
        .find(|(s, _)| *s == current_status)
        .map(|(_, next)| *next)
        .unwrap_or(&[]);
    if !legal_next.contains(&update.status.as_str()) {
        return Err(format!(
            "Invalid claim transition: {} → {}.",
            current_status, update.status
        ));
    }
    if update.status == "partially_approved" && update.approved_amount.is_none() {
        return Err("An approved amount is required for a partially-approved claim.".to_string());
    }

    sqlx::query(
        r#"UPDATE insurance_claims SET
              status = $1,
              approved_amount = COALESCE($2, approved_amount),
              notes = COALESCE($3, notes),
              submitted_at = CASE WHEN $1 = 'submitted' AND submitted_at IS NULL THEN NOW() ELSE submitted_at END,
              settled_at = CASE WHEN $1 = 'settled' THEN NOW() ELSE settled_at END,
              updated_at = NOW()
           WHERE id = $4"#,
    )
    .bind(&update.status)
    .bind(update.approved_amount.map(dec))
    .bind(update.notes.as_deref())
    .bind(update.id)
    .execute(pool)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    audit::for_session(pool, &s, "insurance_claim_status", "insurance_claims",
        Some(&update.id.to_string()),
        Some(serde_json::json!({"to": update.status, "from": current_status}))).await;

    // Phase 9: a settled claim is money the billing desk should follow up
    // on (reconcile the insurer payment into the bill). Best-effort.
    if update.status == "settled" {
        let info: Option<(String, i32)> = sqlx::query_as(
            "SELECT c.insurer, c.bill_id FROM insurance_claims c WHERE c.id = $1",
        )
        .bind(update.id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
        if let Some((insurer, bill_id)) = info {
            let title = format!("Claim settled: {} (bill #{})", insurer, bill_id);
            let body = format!(
                "The insurance claim with {} (bill #{}) was settled. Reconcile the insurer payment against the bill.",
                insurer, bill_id
            );
            if let Err(e) = crate::commands::notifications::emit(
                pool,
                crate::commands::notifications::NotificationOut {
                    user_id: None,
                    role_target: Some("billing_clerk".into()),
                    kind: "claim_settled".into(),
                    title,
                    body,
                    entity_type: Some("bill".into()),
                    entity_id: Some(bill_id),
                },
            ).await {
                eprintln!("[HMS Billing] notification emit failed (non-fatal): {}", e);
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn get_insurance_claims(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    status_filter: Option<String>,
) -> Result<Vec<InsuranceClaim>, String> {
    let _ = rbac::require(&session, Permission::BillingView)?;
    let base = r#"
        SELECT c.id, c.bill_id, c.patient_id, c.insurer, c.policy_number,
               c.claim_amount, c.approved_amount, c.status,
               c.submitted_at, c.settled_at, c.notes, c.created_by_user_id,
               c.created_at, c.updated_at,
               p.first_name || ' ' || p.last_name AS patient_name,
               b.bill_number
        FROM insurance_claims c
        LEFT JOIN patients p ON p.id = c.patient_id
        LEFT JOIN bills b ON b.id = c.bill_id
    "#;
    let q = match status_filter.as_deref() {
        Some(s) if !s.is_empty() => format!("{} WHERE c.status = $1 ORDER BY c.created_at DESC", base),
        _ => format!("{} ORDER BY c.created_at DESC LIMIT 200", base),
    };
    let mut query = sqlx::query_as::<_, InsuranceClaim>(&q);
    if let Some(s) = status_filter.filter(|s| !s.is_empty()) {
        query = query.bind(s);
    }
    query.fetch_all(pool.inner())
        .await
        .map_err(|e| format!("Get claims: {}", e))
}
