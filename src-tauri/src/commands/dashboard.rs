//! Dashboard KPIs — role-aware. Each role sees the metrics relevant to its
//! function; the backend enforces "minimum necessary" by gating which KPIs are
//! returned based on the caller's permissions.

use sqlx::PgPool;

use crate::models::DashboardKpis;
use crate::rbac::{self, Permission, SessionState};

/// Runs a parameterless `SELECT COUNT(*) ...` and returns the count, or 0 on
/// any error (a missing table / failed query should never break the whole
/// dashboard — it just yields a zero for that KPI).
///
/// This replaces the earlier `scalar!` macro with a plain async function so
/// there is no macro arm-matching complexity at all.
async fn count(pool: &PgPool, sql: &str) -> i64 {
    let r: Result<(i64,), sqlx::Error> = sqlx::query_as::<_, (i64,)>(sql)
        .fetch_one(pool)
        .await;
    r.map(|x| x.0).unwrap_or(0)
}

#[tauri::command]
pub async fn get_dashboard_kpis(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
) -> Result<DashboardKpis, String> {
    let s = rbac::require(&session, Permission::DashboardView)?;
    let pool = pool.inner();

    let patients_total = if s.has(Permission::PatientsView) {
        // CR-11: count only active (non-soft-deleted) patients.
        count(pool, "SELECT COUNT(*) FROM patients WHERE deleted_at IS NULL").await
    } else { 0 };

    let (appointments_today, appointments_scheduled, appointments_completed) = if s.has(Permission::AppointmentsView) {
        let today = count(pool, "SELECT COUNT(*) FROM appointments WHERE appointment_date = CURRENT_DATE").await;
        let sched = count(pool, "SELECT COUNT(*) FROM appointments WHERE appointment_date = CURRENT_DATE AND status IN ('scheduled','confirmed')").await;
        let done = count(pool, "SELECT COUNT(*) FROM appointments WHERE appointment_date = CURRENT_DATE AND status = 'completed'").await;
        (today, sched, done)
    } else { (0, 0, 0) };

    let (queue_waiting, queue_in_progress) = if s.has(Permission::QueueView) {
        let w = count(pool, "SELECT COUNT(*) FROM queue_tokens WHERE status='waiting' AND issued_at::date = CURRENT_DATE").await;
        let ip = count(pool, "SELECT COUNT(*) FROM queue_tokens WHERE status='in-progress' AND issued_at::date = CURRENT_DATE").await;
        (w, ip)
    } else { (0, 0) };

    let (ipd_admitted, beds_available, beds_total) = if s.has(Permission::IpdView) {
        let adm = count(pool, "SELECT COUNT(*) FROM ipd_admissions WHERE status='admitted'").await;
        let avail = count(pool, "SELECT COUNT(*) FROM beds WHERE status='available'").await;
        let tot = count(pool, "SELECT COUNT(*) FROM beds").await;
        (adm, avail, tot)
    } else { (0, 0, 0) };

    let (revenue_today, revenue_month) = if s.has(Permission::BillingView) {
        let today: f64 = sqlx::query_scalar::<_, f64>(
            "SELECT COALESCE(SUM(amount),0)::float8 FROM payments WHERE paid_at::date = CURRENT_DATE",
        )
        .fetch_one(pool).await.unwrap_or(0.0);
        let month: f64 = sqlx::query_scalar::<_, f64>(
            "SELECT COALESCE(SUM(amount),0)::float8 FROM payments WHERE paid_at >= date_trunc('month', NOW())",
        )
        .fetch_one(pool).await.unwrap_or(0.0);
        (today, month)
    } else { (0.0, 0.0) };

    let pending_lab_orders = if s.has(Permission::LabView) {
        count(pool, "SELECT COUNT(*) FROM lab_orders WHERE status IN ('ordered','collected','in-progress')").await
    } else { 0 };

    let staff_on_duty = count(pool, "SELECT COUNT(*) FROM users WHERE is_active = TRUE").await;

    Ok(DashboardKpis {
        patients_total,
        appointments_today,
        appointments_scheduled,
        appointments_completed,
        queue_waiting,
        queue_in_progress,
        ipd_admitted,
        beds_available,
        beds_total,
        revenue_today,
        revenue_month,
        pending_lab_orders,
        staff_on_duty,
    })
}
