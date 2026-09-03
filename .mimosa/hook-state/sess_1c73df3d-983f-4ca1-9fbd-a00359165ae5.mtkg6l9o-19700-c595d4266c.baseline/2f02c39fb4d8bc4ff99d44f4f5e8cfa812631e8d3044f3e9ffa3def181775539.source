//! Reports (SRS §4.20, FR-0220–FR-0223) — read-only operational reports.
//!
//! Five Tauri commands, all RBAC-guarded by `Permission::ReportsView`:
//!
//! 1. `get_daily_opd_report` — daily OPD summary (appointments by status,
//!    encounters, new patients, top doctors).
//! 2. `get_ipd_census_report` — current IPD bed census + ward-by-ward
//!    breakdown + discharges for the day.
//! 3. `get_revenue_report` — billed vs collected vs outstanding, bill count
//!    by status, revenue by billing type, top 5 bill items.
//! 4. `get_lab_turnaround_report` — lab order volume, status breakdown,
//!    average turnaround (ordered → last result completed) in hours, top 5
//!    most ordered tests.
//! 5. `export_report_csv` — generic CSV exporter; dispatches on
//!    `report_type` to one of the four fetchers and formats the result as
//!    RFC-4180 CSV with a leading BOM.
//!
//! All five commands are read-only — no audit row is written (per the
//! `audit.rs` design: reads are not audited). All queries run server-side
//! (COUNT / SUM / GROUP BY) so the wire only carries the rolled-up result.
//!
//! Reports DO NOT introduce new tables — they query the existing
//! `patients`, `appointments`, `encounters`, `bills`, `bill_items`,
//! `payments`, `lab_orders`, `lab_order_tests`, `lab_test_catalog`,
//! `ipd_admissions`, `wards`, `beds` tables created by the Phase-1
//! migrations in `db.rs::run_migrations`.
//!
//! Date parameters are ISO-8601 `YYYY-MM-DD` strings. The OPD/IPD commands
//! accept `Option<String>` for `date` — `None` defaults to today (UTC). The
//! Revenue/Lab commands take mandatory `from_date` + `to_date` strings.
//! Postgres performs the cast with `::date`; invalid date strings surface
//! as a user-facing error from `map_err`.
//!
//! The return structs are defined locally in this file (NOT in
//! `models.rs`) because the task scope forbids editing `models.rs`, and
//! because each report's shape is purely an output contract — there's no
//! `sqlx::FromRow` derivation that would require the struct to live next
//! to the other DB-row structs.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::rbac::{self, Permission, SessionState};

// ── Return-shape structs ─────────────────────────────────────────────────────
//
// `#[allow(dead_code)]` is added defensively on every struct: clippy doesn't
// see `#[tauri::command]` macro-expanded usage, and these structs are only
// ever constructed inside their respective report commands (which themselves
// are referenced by `generate_handler!`).

// ── 1. Daily OPD report ──────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct DailyOpdStatusCount {
    pub status: String,
    pub count: i64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct DailyOpdDoctorCount {
    pub doctor_name: String,
    pub appointment_count: i64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct DailyOpdReport {
    pub date: String,
    pub total_appointments: i64,
    pub appointments_by_status: Vec<DailyOpdStatusCount>,
    pub total_encounters: i64,
    pub new_patients: i64,
    pub top_doctors: Vec<DailyOpdDoctorCount>,
}

// ── 2. IPD census report ─────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct IpdCensusWardRow {
    pub ward_id: i32,
    pub ward_name: String,
    pub total_beds: i64,
    pub occupied_beds: i64,
    pub available_beds: i64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct IpdCensusReport {
    pub date: String,
    pub total_beds: i64,
    pub available_beds: i64,
    pub occupied_beds: i64,
    pub maintenance_beds: i64,
    pub current_admissions: i64,
    pub discharges_today: i64,
    pub by_ward: Vec<IpdCensusWardRow>,
}

// ── 3. Revenue report ────────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct BillStatusCount {
    pub status: String,
    pub count: i64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct RevenueByTypeRow {
    pub bill_type: String,
    pub total: f64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct TopBillItemRow {
    pub description: String,
    pub revenue: f64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct RevenueReport {
    pub from_date: String,
    pub to_date: String,
    pub total_billed: f64,
    pub total_collected: f64,
    pub total_outstanding: f64,
    pub bill_count_by_status: Vec<BillStatusCount>,
    pub revenue_by_type: Vec<RevenueByTypeRow>,
    pub top_bill_items: Vec<TopBillItemRow>,
}

// ── 4. Lab turnaround report ─────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct LabOrderStatusCount {
    pub status: String,
    pub count: i64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct TopLabTestRow {
    pub test_name: String,
    pub order_count: i64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct LabTurnaroundReport {
    pub from_date: String,
    pub to_date: String,
    pub total_orders: i64,
    pub orders_by_status: Vec<LabOrderStatusCount>,
    pub average_turnaround_hours: f64,
    pub top_tests: Vec<TopLabTestRow>,
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Resolves an `Option<String>` date parameter to a `YYYY-MM-DD` string,
/// defaulting to today's UTC date when `None` or empty. Used by the OPD
/// and IPD census commands whose "as-of" date is optional.
fn resolve_date_or_today(date: &Option<String>) -> String {
    match date {
        Some(d) if !d.is_empty() => d.clone(),
        _ => Utc::now().format("%Y-%m-%d").to_string(),
    }
}

/// Casts a NUMERIC/Decimal SUM to f64 cleanly. Postgres returns `numeric`
/// for SUM(numeric); the `::float8` cast is the standard escape hatch.
const F8: &str = "::float8";

// ── Fetchers (shared by typed-report commands + CSV exporter) ────────────────

async fn fetch_daily_opd(
    pool: &PgPool,
    date: Option<String>,
) -> Result<DailyOpdReport, String> {
    let resolved = resolve_date_or_today(&date);

    // Total appointments for the day. `appointments.appointment_date` is a
    // DATE (not TIMESTAMPTZ), so direct `= $1::date` comparison is exact.
    let (total_appointments,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM appointments WHERE appointment_date = $1::date",
    )
    .bind(&resolved)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Daily OPD report (total appointments): {e}"))?;

    // Appointments grouped by status. Order by status ascending so the
    // frontend can rely on a stable column order.
    let status_rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT a.status, COUNT(*)::bigint AS count \
         FROM appointments a \
         WHERE a.appointment_date = $1::date \
         GROUP BY a.status \
         ORDER BY a.status",
    )
    .bind(&resolved)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Daily OPD report (by status): {e}"))?;
    let appointments_by_status = status_rows
        .into_iter()
        .map(|(status, count)| DailyOpdStatusCount { status, count })
        .collect::<Vec<_>>();

    // Total encounters/visits for the day. `encounters.visit_date` is
    // TIMESTAMPTZ; cast to date for the comparison.
    let (total_encounters,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM encounters WHERE visit_date::date = $1::date",
    )
    .bind(&resolved)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Daily OPD report (encounters): {e}"))?;

    // New patients registered that day (active only — soft-deleted rows are
    // excluded so a deletion after-the-fact doesn't inflate the count).
    let (new_patients,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM patients \
         WHERE created_at::date = $1::date AND deleted_at IS NULL",
    )
    .bind(&resolved)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Daily OPD report (new patients): {e}"))?;

    // Top 5 doctors by appointment count for the day.
    let doctor_rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT (d.first_name || ' ' || d.last_name) AS doctor_name, \
                COUNT(*)::bigint AS appointment_count \
         FROM appointments a \
         JOIN doctors d ON d.id = a.doctor_id \
         WHERE a.appointment_date = $1::date \
         GROUP BY d.id, d.first_name, d.last_name \
         ORDER BY COUNT(*) DESC, doctor_name ASC \
         LIMIT 5",
    )
    .bind(&resolved)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Daily OPD report (top doctors): {e}"))?;
    let top_doctors = doctor_rows
        .into_iter()
        .map(|(doctor_name, appointment_count)| DailyOpdDoctorCount {
            doctor_name,
            appointment_count,
        })
        .collect::<Vec<_>>();

    Ok(DailyOpdReport {
        date: resolved,
        total_appointments,
        appointments_by_status,
        total_encounters,
        new_patients,
        top_doctors,
    })
}

async fn fetch_ipd_census(
    pool: &PgPool,
    date: Option<String>,
) -> Result<IpdCensusReport, String> {
    let resolved = resolve_date_or_today(&date);

    // Bed snapshot — total + per-status counts in a single pass. The beds
    // table uses statuses 'available' / 'occupied' / 'maintenance' /
    // 'cleaning' (see db.rs migration). We treat 'maintenance' as the
    // canonical "out of service" bucket per the SRS wording; 'cleaning'
    // rows are folded into 'maintenance' for the report so the totals
    // reconcile (total = available + occupied + maintenance).
    let (total_beds, available_beds, occupied_beds, cleaning_beds): (i64, i64, i64, i64) =
        sqlx::query_as(
            r#"
            SELECT
              COUNT(*)::bigint AS total_beds,
              COUNT(*) FILTER (WHERE status = 'available')::bigint AS available_beds,
              COUNT(*) FILTER (WHERE status = 'occupied')::bigint  AS occupied_beds,
              COUNT(*) FILTER (WHERE status IN ('maintenance', 'cleaning'))::bigint AS cleaning_beds
            FROM beds
            "#,
        )
        .fetch_one(pool)
        .await
        .map_err(|e| format!("IPD census report (beds): {e}"))?;

    // The "maintenance" bucket folds in 'cleaning' rows so the totals
    // reconcile: total = available + occupied + maintenance.
    let maintenance_beds = cleaning_beds;

    // Current admissions — patients physically in a bed right now.
    let (current_admissions,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM ipd_admissions WHERE status = 'admitted'",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| format!("IPD census report (admissions): {e}"))?;

    // Discharges for the day. `discharge_date` is TIMESTAMPTZ; cast to
    // date for the comparison.
    let (discharges_today,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM ipd_admissions \
         WHERE status = 'discharged' AND discharge_date::date = $1::date",
    )
    .bind(&resolved)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("IPD census report (discharges): {e}"))?;

    // Per-ward breakdown. Left-join wards to beds so wards with zero beds
    // still appear (showing 0/0). Active wards only.
    let ward_rows: Vec<(i32, String, i64, i64, i64)> = sqlx::query_as(
        r#"
        SELECT
          w.id                                              AS ward_id,
          w.name                                            AS ward_name,
          COUNT(b.id)::bigint                               AS total_beds,
          COUNT(b.id) FILTER (WHERE b.status = 'occupied')::bigint AS occupied_beds,
          COUNT(b.id) FILTER (WHERE b.status = 'available')::bigint AS available_beds
        FROM wards w
        LEFT JOIN beds b ON b.ward_id = w.id
        WHERE w.is_active = TRUE
        GROUP BY w.id, w.name
        ORDER BY w.name
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("IPD census report (wards): {e}"))?;
    let by_ward = ward_rows
        .into_iter()
        .map(|(ward_id, ward_name, total_beds, occupied_beds, available_beds)| {
            IpdCensusWardRow {
                ward_id,
                ward_name,
                total_beds,
                occupied_beds,
                available_beds,
            }
        })
        .collect::<Vec<_>>();

    Ok(IpdCensusReport {
        date: resolved,
        total_beds,
        available_beds,
        occupied_beds,
        maintenance_beds,
        current_admissions,
        discharges_today,
        by_ward,
    })
}

async fn fetch_revenue(
    pool: &PgPool,
    from_date: String,
    to_date: String,
) -> Result<RevenueReport, String> {
    // Total billed over the range. Bills are counted by `created_at`.
    let (total_billed,): (f64,) = sqlx::query_as(&format!(
        "SELECT COALESCE(SUM(b.net_amount), 0){F8} \
         FROM bills b \
         WHERE b.created_at::date >= $1::date AND b.created_at::date <= $2::date",
    ))
    .bind(&from_date)
    .bind(&to_date)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Revenue report (billed): {e}"))?;

    // Total collected over the range. Payments are counted by `paid_at`.
    let (total_collected,): (f64,) = sqlx::query_as(&format!(
        "SELECT COALESCE(SUM(p.amount), 0){F8} \
         FROM payments p \
         WHERE p.paid_at::date >= $1::date AND p.paid_at::date <= $2::date",
    ))
    .bind(&from_date)
    .bind(&to_date)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Revenue report (collected): {e}"))?;

    let total_outstanding = (total_billed - total_collected).max(0.0);

    // Bill count by status (paid / unpaid / partial / draft).
    let status_rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT b.status, COUNT(*)::bigint AS count \
         FROM bills b \
         WHERE b.created_at::date >= $1::date AND b.created_at::date <= $2::date \
         GROUP BY b.status \
         ORDER BY b.status",
    )
    .bind(&from_date)
    .bind(&to_date)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Revenue report (status): {e}"))?;
    let bill_count_by_status = status_rows
        .into_iter()
        .map(|(status, count)| BillStatusCount { status, count })
        .collect::<Vec<_>>();

    // Revenue by billing type (opd / ipd / lab / pharmacy / other).
    let type_rows: Vec<(String, f64)> = sqlx::query_as(&format!(
        "SELECT b.bill_type, COALESCE(SUM(b.net_amount), 0){F8} AS total \
         FROM bills b \
         WHERE b.created_at::date >= $1::date AND b.created_at::date <= $2::date \
         GROUP BY b.bill_type \
         ORDER BY SUM(b.net_amount) DESC",
    ))
    .bind(&from_date)
    .bind(&to_date)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Revenue report (by type): {e}"))?;
    let revenue_by_type = type_rows
        .into_iter()
        .map(|(bill_type, total)| RevenueByTypeRow { bill_type, total })
        .collect::<Vec<_>>();

    // Top 5 bill items by revenue. Joins bill_items to bills so the date
    // range filter applies (a bill_item's date is its parent bill's
    // created_at).
    let item_rows: Vec<(String, f64)> = sqlx::query_as(&format!(
        "SELECT bi.description, COALESCE(SUM(bi.total), 0){F8} AS revenue \
         FROM bill_items bi \
         JOIN bills b ON b.id = bi.bill_id \
         WHERE b.created_at::date >= $1::date AND b.created_at::date <= $2::date \
         GROUP BY bi.description \
         ORDER BY SUM(bi.total) DESC \
         LIMIT 5",
    ))
    .bind(&from_date)
    .bind(&to_date)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Revenue report (top items): {e}"))?;
    let top_bill_items = item_rows
        .into_iter()
        .map(|(description, revenue)| TopBillItemRow { description, revenue })
        .collect::<Vec<_>>();

    Ok(RevenueReport {
        from_date,
        to_date,
        total_billed,
        total_collected,
        total_outstanding,
        bill_count_by_status,
        revenue_by_type,
        top_bill_items,
    })
}

async fn fetch_lab_turnaround(
    pool: &PgPool,
    from_date: String,
    to_date: String,
) -> Result<LabTurnaroundReport, String> {
    // Total lab orders in range. `lab_orders.ordered_at` is TIMESTAMPTZ.
    let (total_orders,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM lab_orders lo \
         WHERE lo.ordered_at::date >= $1::date AND lo.ordered_at::date <= $2::date",
    )
    .bind(&from_date)
    .bind(&to_date)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Lab turnaround report (total): {e}"))?;

    // Orders by status (ordered / completed / collected / etc.).
    let status_rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT lo.status, COUNT(*)::bigint AS count \
         FROM lab_orders lo \
         WHERE lo.ordered_at::date >= $1::date AND lo.ordered_at::date <= $2::date \
         GROUP BY lo.status \
         ORDER BY COUNT(*) DESC, lo.status",
    )
    .bind(&from_date)
    .bind(&to_date)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Lab turnaround report (status): {e}"))?;
    let orders_by_status = status_rows
        .into_iter()
        .map(|(status, count)| LabOrderStatusCount { status, count })
        .collect::<Vec<_>>();

    // Average turnaround time in hours, computed only over orders whose
    // results are all completed. For each completed order, take the
    // MAX(lab_order_tests.completed_at) as the "last result completed"
    // timestamp and subtract `ordered_at`. The LATERAL join makes this
    // per-order calculation clean. Returns 0 if no completed orders.
    let (avg_turnaround_hours,): (f64,) = sqlx::query_as(
        r#"
        SELECT COALESCE(
            AVG(EXTRACT(EPOCH FROM (lc.last_completed - lo.ordered_at)) / 3600.0),
            0
        )::float8
        FROM lab_orders lo
        JOIN LATERAL (
            SELECT MAX(lot.completed_at) AS last_completed
            FROM lab_order_tests lot
            WHERE lot.lab_order_id = lo.id AND lot.completed_at IS NOT NULL
        ) lc ON true
        WHERE lo.status = 'completed'
          AND lo.ordered_at::date >= $1::date
          AND lo.ordered_at::date <= $2::date
        "#,
    )
    .bind(&from_date)
    .bind(&to_date)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Lab turnaround report (avg): {e}"))?;

    // Top 5 most ordered tests in range.
    let test_rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT tc.name AS test_name, COUNT(*)::bigint AS order_count \
         FROM lab_order_tests lot \
         JOIN lab_orders lo ON lo.id = lot.lab_order_id \
         JOIN lab_test_catalog tc ON tc.id = lot.test_catalog_id \
         WHERE lo.ordered_at::date >= $1::date AND lo.ordered_at::date <= $2::date \
         GROUP BY tc.id, tc.name \
         ORDER BY COUNT(*) DESC, tc.name ASC \
         LIMIT 5",
    )
    .bind(&from_date)
    .bind(&to_date)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Lab turnaround report (top tests): {e}"))?;
    let top_tests = test_rows
        .into_iter()
        .map(|(test_name, order_count)| TopLabTestRow {
            test_name,
            order_count,
        })
        .collect::<Vec<_>>();

    Ok(LabTurnaroundReport {
        from_date,
        to_date,
        total_orders,
        orders_by_status,
        average_turnaround_hours: avg_turnaround_hours,
        top_tests,
    })
}

// ── CSV formatting helpers ───────────────────────────────────────────────────
//
// RFC-4180 CSV: a cell containing a comma, double-quote, or newline is
// wrapped in double-quotes and any embedded double-quotes are doubled.
// We prepend a UTF-8 BOM (`\u{FEFF}`) so Excel detects UTF-8 and doesn't
// mis-read non-ASCII characters (Pakistani hospital ops routinely open
// CSVs in Excel).

fn csv_escape_cell(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn csv_row(cells: &[String]) -> String {
    cells
        .iter()
        .map(|c| csv_escape_cell(c))
        .collect::<Vec<_>>()
        .join(",")
}

fn csv_join(rows: Vec<String>) -> String {
    // Prepend BOM and join rows with CRLF (Excel convention).
    format!("\u{FEFF}{}", rows.join("\r\n"))
}

fn fmt_f64(v: f64) -> String {
    // Two decimal places for money; the frontend renders with formatMoney
    // but the CSV is plain text for spreadsheet import.
    format!("{:.2}", v)
}

fn format_daily_opd_csv(r: DailyOpdReport) -> String {
    let mut rows: Vec<String> = vec![csv_row(&[
        "Report".to_string(),
        "Daily OPD Summary".to_string(),
    ])];
    rows.push(csv_row(&["Date".to_string(), r.date.clone()]));
    rows.push(csv_row(&[
        "Total appointments".to_string(),
        r.total_appointments.to_string(),
    ]));
    rows.push(csv_row(&[
        "Total encounters".to_string(),
        r.total_encounters.to_string(),
    ]));
    rows.push(csv_row(&[
        "New patients".to_string(),
        r.new_patients.to_string(),
    ]));
    rows.push(String::new()); // blank separator
    rows.push(csv_row(&["Status".to_string(), "Count".to_string()]));
    for s in &r.appointments_by_status {
        rows.push(csv_row(&[s.status.clone(), s.count.to_string()]));
    }
    rows.push(String::new());
    rows.push(csv_row(&[
        "Doctor".to_string(),
        "Appointment count".to_string(),
    ]));
    for d in &r.top_doctors {
        rows.push(csv_row(&[
            d.doctor_name.clone(),
            d.appointment_count.to_string(),
        ]));
    }
    csv_join(rows)
}

fn format_ipd_census_csv(r: IpdCensusReport) -> String {
    let mut rows: Vec<String> = vec![csv_row(&[
        "Report".to_string(),
        "IPD Census".to_string(),
    ])];
    rows.push(csv_row(&["Date".to_string(), r.date.clone()]));
    rows.push(csv_row(&[
        "Total beds".to_string(),
        r.total_beds.to_string(),
    ]));
    rows.push(csv_row(&[
        "Available beds".to_string(),
        r.available_beds.to_string(),
    ]));
    rows.push(csv_row(&[
        "Occupied beds".to_string(),
        r.occupied_beds.to_string(),
    ]));
    rows.push(csv_row(&[
        "Maintenance beds".to_string(),
        r.maintenance_beds.to_string(),
    ]));
    rows.push(csv_row(&[
        "Current admissions".to_string(),
        r.current_admissions.to_string(),
    ]));
    rows.push(csv_row(&[
        "Discharges today".to_string(),
        r.discharges_today.to_string(),
    ]));
    rows.push(String::new());
    rows.push(csv_row(&[
        "Ward".to_string(),
        "Total beds".to_string(),
        "Occupied".to_string(),
        "Available".to_string(),
    ]));
    for w in &r.by_ward {
        rows.push(csv_row(&[
            w.ward_name.clone(),
            w.total_beds.to_string(),
            w.occupied_beds.to_string(),
            w.available_beds.to_string(),
        ]));
    }
    csv_join(rows)
}

fn format_revenue_csv(r: RevenueReport) -> String {
    let mut rows: Vec<String> = vec![csv_row(&[
        "Report".to_string(),
        "Revenue".to_string(),
    ])];
    rows.push(csv_row(&["From".to_string(), r.from_date.clone()]));
    rows.push(csv_row(&["To".to_string(), r.to_date.clone()]));
    rows.push(csv_row(&[
        "Total billed".to_string(),
        fmt_f64(r.total_billed),
    ]));
    rows.push(csv_row(&[
        "Total collected".to_string(),
        fmt_f64(r.total_collected),
    ]));
    rows.push(csv_row(&[
        "Total outstanding".to_string(),
        fmt_f64(r.total_outstanding),
    ]));
    rows.push(String::new());
    rows.push(csv_row(&["Bill status".to_string(), "Count".to_string()]));
    for s in &r.bill_count_by_status {
        rows.push(csv_row(&[s.status.clone(), s.count.to_string()]));
    }
    rows.push(String::new());
    rows.push(csv_row(&[
        "Billing type".to_string(),
        "Revenue".to_string(),
    ]));
    for t in &r.revenue_by_type {
        rows.push(csv_row(&[t.bill_type.clone(), fmt_f64(t.total)]));
    }
    rows.push(String::new());
    rows.push(csv_row(&[
        "Bill item (description)".to_string(),
        "Revenue".to_string(),
    ]));
    for i in &r.top_bill_items {
        rows.push(csv_row(&[i.description.clone(), fmt_f64(i.revenue)]));
    }
    csv_join(rows)
}

fn format_lab_turnaround_csv(r: LabTurnaroundReport) -> String {
    let mut rows: Vec<String> = vec![csv_row(&[
        "Report".to_string(),
        "Lab Turnaround".to_string(),
    ])];
    rows.push(csv_row(&["From".to_string(), r.from_date.clone()]));
    rows.push(csv_row(&["To".to_string(), r.to_date.clone()]));
    rows.push(csv_row(&[
        "Total orders".to_string(),
        r.total_orders.to_string(),
    ]));
    rows.push(csv_row(&[
        "Average turnaround (hours)".to_string(),
        format!("{:.2}", r.average_turnaround_hours),
    ]));
    rows.push(String::new());
    rows.push(csv_row(&["Order status".to_string(), "Count".to_string()]));
    for s in &r.orders_by_status {
        rows.push(csv_row(&[s.status.clone(), s.count.to_string()]));
    }
    rows.push(String::new());
    rows.push(csv_row(&[
        "Test name".to_string(),
        "Order count".to_string(),
    ]));
    for t in &r.top_tests {
        rows.push(csv_row(&[t.test_name.clone(), t.order_count.to_string()]));
    }
    csv_join(rows)
}

// ── Tauri commands ───────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_daily_opd_report(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    date: Option<String>,
) -> Result<DailyOpdReport, String> {
    let _ = rbac::require(&session, Permission::ReportsView)?;
    fetch_daily_opd(pool.inner(), date).await
}

#[tauri::command]
pub async fn get_ipd_census_report(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    date: Option<String>,
) -> Result<IpdCensusReport, String> {
    let _ = rbac::require(&session, Permission::ReportsView)?;
    fetch_ipd_census(pool.inner(), date).await
}

#[tauri::command]
pub async fn get_revenue_report(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    from_date: String,
    to_date: String,
) -> Result<RevenueReport, String> {
    let _ = rbac::require(&session, Permission::ReportsView)?;
    fetch_revenue(pool.inner(), from_date, to_date).await
}

#[tauri::command]
pub async fn get_lab_turnaround_report(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    from_date: String,
    to_date: String,
) -> Result<LabTurnaroundReport, String> {
    let _ = rbac::require(&session, Permission::ReportsView)?;
    fetch_lab_turnaround(pool.inner(), from_date, to_date).await
}

/// Generic CSV exporter. `report_type` selects the report; `params` is a
/// JSON object whose shape depends on the report:
///   - `daily_opd`      → `{"date": "YYYY-MM-DD"}` (date optional, defaults to today)
///   - `ipd_census`     → `{"date": "YYYY-MM-DD"}` (date optional, defaults to today)
///   - `revenue`        → `{"from_date": "YYYY-MM-DD", "to_date": "YYYY-MM-DD"}` (required)
///   - `lab_turnaround` → `{"from_date": "YYYY-MM-DD", "to_date": "YYYY-MM-DD"}` (required)
///
/// Returns the CSV as a single string (with a leading UTF-8 BOM so Excel
/// detects UTF-8). The frontend wraps it in a Blob and triggers a download.
#[tauri::command]
pub async fn export_report_csv(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    report_type: String,
    params: String,
) -> Result<String, String> {
    let _ = rbac::require(&session, Permission::ReportsView)?;
    let pool = pool.inner();

    // Parse the params JSON. We accept a loose `serde_json::Value` and
    // extract the fields we need per report_type — this avoids defining a
    // per-report params struct and keeps the dispatch logic compact.
    let p: serde_json::Value = serde_json::from_str(&params)
        .map_err(|e| format!("export_report_csv: invalid params JSON: {e}"))?;

    let get_str = |key: &str| -> Option<String> {
        p.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
    };

    match report_type.as_str() {
        "daily_opd" => {
            let date = get_str("date");
            let report = fetch_daily_opd(pool, date).await?;
            Ok(format_daily_opd_csv(report))
        }
        "ipd_census" => {
            let date = get_str("date");
            let report = fetch_ipd_census(pool, date).await?;
            Ok(format_ipd_census_csv(report))
        }
        "revenue" => {
            let from_date = get_str("from_date").ok_or_else(|| {
                "export_report_csv: 'from_date' is required for the revenue report".to_string()
            })?;
            let to_date = get_str("to_date").ok_or_else(|| {
                "export_report_csv: 'to_date' is required for the revenue report".to_string()
            })?;
            let report = fetch_revenue(pool, from_date, to_date).await?;
            Ok(format_revenue_csv(report))
        }
        "lab_turnaround" => {
            let from_date = get_str("from_date").ok_or_else(|| {
                "export_report_csv: 'from_date' is required for the lab_turnaround report"
                    .to_string()
            })?;
            let to_date = get_str("to_date").ok_or_else(|| {
                "export_report_csv: 'to_date' is required for the lab_turnaround report"
                    .to_string()
            })?;
            let report = fetch_lab_turnaround(pool, from_date, to_date).await?;
            Ok(format_lab_turnaround_csv(report))
        }
        other => Err(format!(
            "export_report_csv: unknown report_type '{other}'. Expected one of: \
             daily_opd, ipd_census, revenue, lab_turnaround."
        )),
    }
}
