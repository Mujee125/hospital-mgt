//! Accounts — expense ledger + income-vs-expense summary
//! (SRS §2.15, Phase 8). The revenue side lives in `billing.rs`; this
//! module completes the finance picture with the expense side.
//!
//! Permission mapping (reuses the existing billing grants — no new
//! variants, least privilege preserved):
//!   • view expenses / summary  → BillingView (billing staff, doctors, admin)
//!   • record an expense        → BillingManage (billing manager / admin;
//!     a receptionist — who holds only BillingCreate — cannot)
//!   • VOID an expense          → BillingApprove (super admin only — money
//!     leaves the ledger only through the same gate as refunds/cancellations)
//!
//! Void is a soft delete: the row (and who voided it, why) stays in the
//! table forever; lists and summaries exclude it. Financial rows are never
//! hard-deleted.
//!
//! All writes are audited; every write command is *_core-extracted
//! (AERP Part G) for command-level testing.

use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::audit;
use crate::models::{AccountsSummary, CreateExpense, Expense, ExpenseCategoryTotal, VoidExpense};
use crate::rbac::{self, Permission, SessionState};

fn dec(f: f64) -> Decimal {
    Decimal::from_f64(f).unwrap_or_default()
}

const SELECT_EXPENSES: &str = r#"
    SELECT e.id, e.category, e.description, e.amount, e.expense_date,
           e.paid_to, e.payment_method, e.reference_number,
           e.recorded_by_user_id, e.created_at,
           e.voided_at, e.voided_by_user_id, e.void_reason,
           u.username AS recorded_by_name
    FROM expenses e
    LEFT JOIN users u ON u.id = e.recorded_by_user_id
"#;

#[tauri::command]
pub async fn get_expenses(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    from_date: Option<String>,
    to_date: Option<String>,
    include_voided: Option<bool>,
) -> Result<Vec<Expense>, String> {
    let _ = rbac::require(&session, Permission::BillingView)?;
    // Defaults: current month, active (non-voided) rows.
    let from = from_date
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{}-01", chrono::Utc::now().format("%Y-%m")));
    let to = to_date
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());

    let mut q = String::from(SELECT_EXPENSES);
    q.push_str(" WHERE e.expense_date >= $1::date AND e.expense_date <= $2::date");
    if !include_voided.unwrap_or(false) {
        q.push_str(" AND e.voided_at IS NULL");
    }
    q.push_str(" ORDER BY e.expense_date DESC, e.id DESC LIMIT 500");

    sqlx::query_as::<_, Expense>(&q)
        .bind(&from)
        .bind(&to)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| format!("Get expenses: {}", e))
}

#[tauri::command]
pub async fn create_expense(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    expense: CreateExpense,
) -> Result<i32, String> {
    create_expense_core(pool.inner(), &session, expense).await
}

/// Expense-creation logic core (AERP Part G extraction pattern).
pub async fn create_expense_core(
    pool: &PgPool,
    session_state: &SessionState,
    expense: CreateExpense,
) -> Result<i32, String> {
    let s = rbac::require_strong(session_state, pool, Permission::BillingManage).await?;

    if expense.category.trim().is_empty() {
        return Err("Expense category is required.".to_string());
    }
    if expense.description.trim().is_empty() {
        return Err("Expense description is required.".to_string());
    }
    // Same finite-positive discipline as record_payment (IPC-07): NaN and
    // Infinity must not become Decimal::ZERO and silently record 0.
    if !expense.amount.is_finite() || expense.amount <= 0.0 {
        return Err("Invalid expense amount.".to_string());
    }

    let row: (i32,) = sqlx::query_as(
        r#"INSERT INTO expenses
               (category, description, amount, expense_date, paid_to,
                payment_method, reference_number, recorded_by_user_id)
           VALUES ($1, $2, $3,
                   COALESCE($4::date, CURRENT_DATE),
                   $5, $6, $7, $8) RETURNING id"#,
    )
    .bind(expense.category.trim())
    .bind(expense.description.trim())
    .bind(dec(expense.amount))
    .bind(expense.expense_date.as_deref().filter(|s| !s.is_empty()))
    .bind(
        expense
            .paid_to
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    )
    .bind(expense.payment_method.as_deref().unwrap_or("cash"))
    .bind(
        expense
            .reference_number
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    )
    .bind(s.user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    audit::for_session(
        pool,
        &s,
        "expense_create",
        "expenses",
        Some(&row.0.to_string()),
        Some(serde_json::json!({
            "category": expense.category.trim(),
            "amount": expense.amount,
        })),
    )
    .await;
    Ok(row.0)
}

#[tauri::command]
pub async fn void_expense(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    void: VoidExpense,
) -> Result<(), String> {
    void_expense_core(pool.inner(), &session, void).await
}

/// Void logic core: manager-only, reason required, idempotent-rejected on
/// already-voided rows, soft delete only.
pub async fn void_expense_core(
    pool: &PgPool,
    session_state: &SessionState,
    void: VoidExpense,
) -> Result<(), String> {
    let s = rbac::require_strong(session_state, pool, Permission::BillingApprove).await?;
    if void.reason.trim().is_empty() {
        return Err("A void reason is required.".to_string());
    }

    let updated = sqlx::query(
        r#"UPDATE expenses SET voided_at = NOW(), voided_by_user_id = $1, void_reason = $2
           WHERE id = $3 AND voided_at IS NULL"#,
    )
    .bind(s.user_id)
    .bind(void.reason.trim())
    .bind(void.id)
    .execute(pool)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;
    if updated.rows_affected() == 0 {
        return Err("Expense not found (or already voided).".to_string());
    }

    audit::for_session(
        pool,
        &s,
        "expense_void",
        "expenses",
        Some(&void.id.to_string()),
        Some(serde_json::json!({"reason": void.reason.trim()})),
    )
    .await;
    Ok(())
}

/// Income-vs-expense summary over a range (SRS §2.15). Revenue matches the
/// daily-collection roll-up: payments by paid_at minus refunds by
/// refunded_at. Expenses: non-voided rows by expense_date. Exposed `pub`
/// for the CSV exporter and integration tests.
pub async fn fetch_accounts_summary(
    pool: &PgPool,
    from_date: String,
    to_date: String,
) -> Result<AccountsSummary, String> {
    const F8: &str = "::float8";

    let (total_collected,): (f64,) = sqlx::query_as(&format!(
        "SELECT COALESCE(SUM(amount), 0){F8} FROM payments \
         WHERE paid_at::date >= $1::date AND paid_at::date <= $2::date",
    ))
    .bind(&from_date)
    .bind(&to_date)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Accounts summary (payments): {e}"))?;

    let (total_refunded,): (f64,) = sqlx::query_as(&format!(
        "SELECT COALESCE(SUM(amount), 0){F8} FROM refunds \
         WHERE refunded_at::date >= $1::date AND refunded_at::date <= $2::date",
    ))
    .bind(&from_date)
    .bind(&to_date)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Accounts summary (refunds): {e}"))?;

    let (total_expenses, expense_count): (f64, i64) = sqlx::query_as(&format!(
        "SELECT COALESCE(SUM(amount), 0){F8}, COUNT(*)::bigint FROM expenses \
         WHERE voided_at IS NULL \
           AND expense_date >= $1::date AND expense_date <= $2::date",
    ))
    .bind(&from_date)
    .bind(&to_date)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Accounts summary (expenses): {e}"))?;

    let cat_rows: Vec<(String, f64, i64)> = sqlx::query_as(&format!(
        "SELECT category, COALESCE(SUM(amount), 0){F8} AS total, COUNT(*)::bigint AS count \
         FROM expenses \
         WHERE voided_at IS NULL \
           AND expense_date >= $1::date AND expense_date <= $2::date \
         GROUP BY category ORDER BY SUM(amount) DESC",
    ))
    .bind(&from_date)
    .bind(&to_date)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Accounts summary (by category): {e}"))?;

    let total_revenue = total_collected - total_refunded;
    let by_category = cat_rows
        .into_iter()
        .map(|(category, total, count)| ExpenseCategoryTotal {
            category,
            total,
            count,
        })
        .collect();

    Ok(AccountsSummary {
        from_date,
        to_date,
        total_revenue,
        total_refunded,
        total_expenses,
        net_position: total_revenue - total_expenses,
        expense_count,
        by_category,
    })
}

#[tauri::command]
pub async fn get_accounts_summary(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    from_date: String,
    to_date: String,
) -> Result<AccountsSummary, String> {
    let _ = rbac::require(&session, Permission::BillingView)?;
    fetch_accounts_summary(pool.inner(), from_date, to_date).await
}
