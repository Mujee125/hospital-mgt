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

// ── 5. Doctor performance report (SRS §4.20 — Phase 6.4) ─────────────────────

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct DoctorPerformanceRow {
    pub doctor_id: i32,
    pub doctor_name: String,
    pub appointments: i64,
    pub encounters: i64,
    pub lab_orders: i64,
    pub prescriptions: i64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct DoctorPerformanceReport {
    pub from_date: String,
    pub to_date: String,
    pub doctors: Vec<DoctorPerformanceRow>,
}

// ── 6. Diagnosis frequency report (Phase 6.4) ────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct DiagnosisCountRow {
    pub diagnosis: String,
    pub encounter_count: i64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct DiagnosisFrequencyReport {
    pub from_date: String,
    pub to_date: String,
    pub total_encounters: i64,
    pub top_diagnoses: Vec<DiagnosisCountRow>,
}

// ── 7. Pharmacy consumption report (Phase 6.4) ───────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct MedicationConsumptionRow {
    pub medication_name: String,
    pub times_dispensed: i64,
    pub total_quantity: f64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct PharmacyConsumptionReport {
    pub from_date: String,
    pub to_date: String,
    pub total_dispensed_items: i64,
    pub top_medications: Vec<MedicationConsumptionRow>,
}

// ── 8. Drug expiry report (Phase 6.4) ─────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct ExpiryItemRow {
    pub name: String,
    pub batch_number: Option<String>,
    pub stock_quantity: f64,
    pub expiry_date: Option<String>,
    pub days_until_expiry: Option<i64>,
    pub stock_value: f64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct DrugExpiryReport {
    pub expired_count: i64,
    pub expiring_90_days: i64,
    pub expiring_180_days: i64,
    pub expired_stock_value: f64,
    pub expiring_90_stock_value: f64,
    pub items: Vec<ExpiryItemRow>,
}

// ── 9. Daily collection report (Phase 6.4) ────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct CollectionDayRow {
    pub date: String,
    pub payments: i64,
    pub collected: f64,
    pub refunded: f64,
    pub net: f64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct DailyCollectionReport {
    pub from_date: String,
    pub to_date: String,
    pub total_collected: f64,
    pub total_refunded: f64,
    pub by_day: Vec<CollectionDayRow>,
}

// ── 10. Receivables aging report (Phase 6.4) ──────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct AgingBucketRow {
    pub bucket: String,
    pub bill_count: i64,
    pub outstanding: f64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct ReceivablesAgingReport {
    pub as_of_date: String,
    pub total_outstanding: f64,
    pub total_open_bills: i64,
    pub buckets: Vec<AgingBucketRow>,
}

// ── 11. Insurance claims status report (Phase 6.4) ───────────────────────────

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct ClaimStatusRow {
    pub status: String,
    pub claims: i64,
    pub claimed_amount: f64,
    pub approved_amount: f64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct InsuranceClaimsReport {
    pub as_of_date: String,
    pub total_claims: i64,
    pub total_claimed: f64,
    pub total_approved: f64,
    pub by_status: Vec<ClaimStatusRow>,
}

// ── 12. Stock status report (Phase 6.4) ────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct LowStockRow {
    pub name: String,
    pub stock_quantity: f64,
    pub reorder_level: f64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct StockStatusReport {
    pub as_of_date: String,
    pub active_items: i64,
    pub total_stock_value: f64,
    pub low_stock_count: i64,
    pub low_stock_items: Vec<LowStockRow>,
}

// ── 13. User activity report (Phase 6.4) ──────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct UserActivityRow {
    pub username: String,
    pub full_name: Option<String>,
    pub action_count: i64,
    pub last_action_at: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct UserActivityReport {
    pub from_date: String,
    pub to_date: String,
    pub total_actions: i64,
    pub by_user: Vec<UserActivityRow>,
}

// ── 14. Backup status report (Phase 6.4) ──────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct BackupFileRow {
    pub filename: String,
    pub size_bytes: i64,
    pub age_days: i64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct BackupStatusReport {
    pub backup_count: i64,
    pub latest_backup_age_days: Option<i64>,
    pub total_size_bytes: i64,
    pub files: Vec<BackupFileRow>,
}

// ── Phase 6.4 fetchers ───────────────────────────────────────────────────────

pub async fn fetch_doctor_performance(
    pool: &PgPool,
    from_date: String,
    to_date: String,
) -> Result<DoctorPerformanceReport, String> {
    // One row per active doctor; LEFT JOINs keep doctors with zero activity.
    let rows: Vec<(i32, String, i64, i64, i64, i64)> = sqlx::query_as(
        r#"
        SELECT d.id,
               d.first_name || ' ' || d.last_name AS doctor_name,
               (SELECT COUNT(*) FROM appointments a
                 WHERE a.doctor_id = d.id
                   AND a.appointment_date >= $1::date AND a.appointment_date <= $2::date) AS appointments,
               (SELECT COUNT(*) FROM encounters e
                 WHERE e.doctor_id = d.id
                   AND e.visit_date::date >= $1::date AND e.visit_date::date <= $2::date) AS encounters,
               (SELECT COUNT(*) FROM lab_orders lo
                 WHERE lo.ordered_by_doctor_id = d.id
                   AND lo.ordered_at::date >= $1::date AND lo.ordered_at::date <= $2::date) AS lab_orders,
               (SELECT COUNT(*) FROM prescriptions rx
                 WHERE rx.doctor_id = d.id
                   AND rx.created_at::date >= $1::date AND rx.created_at::date <= $2::date) AS prescriptions
        FROM doctors d
        WHERE d.is_active = TRUE
        ORDER BY doctor_name
        "#,
    )
    .bind(&from_date)
    .bind(&to_date)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Doctor performance report: {e}"))?;
    let doctors = rows
        .into_iter()
        .map(|(doctor_id, doctor_name, appointments, encounters, lab_orders, prescriptions)| {
            DoctorPerformanceRow { doctor_id, doctor_name, appointments, encounters, lab_orders, prescriptions }
        })
        .collect();
    Ok(DoctorPerformanceReport { from_date, to_date, doctors })
}

pub async fn fetch_diagnosis_frequency(
    pool: &PgPool,
    from_date: String,
    to_date: String,
) -> Result<DiagnosisFrequencyReport, String> {
    let (total_encounters,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM encounters \
         WHERE visit_date::date >= $1::date AND visit_date::date <= $2::date \
           AND diagnosis IS NOT NULL AND TRIM(diagnosis) <> ''",
    )
    .bind(&from_date)
    .bind(&to_date)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Diagnosis frequency report (total): {e}"))?;

    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT TRIM(diagnosis) AS diagnosis, COUNT(*)::bigint AS encounter_count \
         FROM encounters \
         WHERE visit_date::date >= $1::date AND visit_date::date <= $2::date \
           AND diagnosis IS NOT NULL AND TRIM(diagnosis) <> '' \
         GROUP BY TRIM(diagnosis) \
         ORDER BY COUNT(*) DESC, diagnosis ASC \
         LIMIT 20",
    )
    .bind(&from_date)
    .bind(&to_date)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Diagnosis frequency report (top): {e}"))?;
    let top_diagnoses = rows
        .into_iter()
        .map(|(diagnosis, encounter_count)| DiagnosisCountRow { diagnosis, encounter_count })
        .collect();
    Ok(DiagnosisFrequencyReport { from_date, to_date, total_encounters, top_diagnoses })
}

pub async fn fetch_pharmacy_consumption(
    pool: &PgPool,
    from_date: String,
    to_date: String,
) -> Result<PharmacyConsumptionReport, String> {
    let (total_dispensed_items,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM prescription_items pi \
         WHERE pi.dispensed = TRUE \
           AND pi.dispensed_at::date >= $1::date AND pi.dispensed_at::date <= $2::date",
    )
    .bind(&from_date)
    .bind(&to_date)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Pharmacy consumption report (total): {e}"))?;

    let rows: Vec<(String, i64, f64)> = sqlx::query_as(&format!(
        "SELECT pi.medication_name, COUNT(*)::bigint AS times_dispensed, \
                COALESCE(SUM(pi.quantity), 0){F8} AS total_quantity \
         FROM prescription_items pi \
         WHERE pi.dispensed = TRUE \
           AND pi.dispensed_at::date >= $1::date AND pi.dispensed_at::date <= $2::date \
         GROUP BY pi.medication_name \
         ORDER BY COUNT(*) DESC, pi.medication_name ASC \
         LIMIT 20",
    ))
    .bind(&from_date)
    .bind(&to_date)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Pharmacy consumption report (top): {e}"))?;
    let top_medications = rows
        .into_iter()
        .map(|(medication_name, times_dispensed, total_quantity)| {
            MedicationConsumptionRow { medication_name, times_dispensed, total_quantity }
        })
        .collect();
    Ok(PharmacyConsumptionReport { from_date, to_date, total_dispensed_items, top_medications })
}

pub async fn fetch_drug_expiry(pool: &PgPool) -> Result<DrugExpiryReport, String> {
    // Buckets over active medication-category items with an expiry date.
    let (expired_count, exp90, exp180, expired_value, exp90_value): (i64, i64, i64, f64, f64) =
        sqlx::query_as(&format!(
            r#"
            SELECT
              COUNT(*) FILTER (WHERE expiry_date < CURRENT_DATE)::bigint AS expired_count,
              COUNT(*) FILTER (WHERE expiry_date >= CURRENT_DATE AND expiry_date <= CURRENT_DATE + 90)::bigint AS exp90,
              COUNT(*) FILTER (WHERE expiry_date > CURRENT_DATE + 90 AND expiry_date <= CURRENT_DATE + 180)::bigint AS exp180,
              COALESCE(SUM(stock_quantity * unit_cost) FILTER (WHERE expiry_date < CURRENT_DATE), 0){F8} AS expired_value,
              COALESCE(SUM(stock_quantity * unit_cost) FILTER (WHERE expiry_date >= CURRENT_DATE AND expiry_date <= CURRENT_DATE + 90), 0){F8} AS exp90_value
            FROM inventory_items
            WHERE is_active = TRUE AND category = 'medication' AND expiry_date IS NOT NULL
            "#,
        ))
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Drug expiry report (buckets): {e}"))?;

    // Per-item detail: everything already expired or expiring within 180 days.
    #[derive(sqlx::FromRow)]
    struct ExpiryRaw {
        name: String,
        batch_number: Option<String>,
        stock_quantity: f64,
        expiry_date: Option<chrono::NaiveDate>,
        days_until_expiry: Option<i64>,
        stock_value: f64,
    }
    let rows: Vec<ExpiryRaw> = sqlx::query_as(&format!(
        r#"
            SELECT name, batch_number, stock_quantity{F8}, expiry_date,
                   (expiry_date - CURRENT_DATE)::bigint AS days_until_expiry,
                   (stock_quantity * unit_cost){F8} AS stock_value
            FROM inventory_items
            WHERE is_active = TRUE AND category = 'medication' AND expiry_date IS NOT NULL
              AND expiry_date <= CURRENT_DATE + 180
            ORDER BY expiry_date ASC
            LIMIT 100
            "#,
    ))
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Drug expiry report (items): {e}"))?;
    let items = rows
        .into_iter()
        .map(|r| ExpiryItemRow {
            name: r.name,
            batch_number: r.batch_number,
            stock_quantity: r.stock_quantity,
            expiry_date: r.expiry_date.map(|d| d.to_string()),
            days_until_expiry: r.days_until_expiry,
            stock_value: r.stock_value,
        })
        .collect();
    Ok(DrugExpiryReport {
        expired_count,
        expiring_90_days: exp90,
        expiring_180_days: exp180,
        expired_stock_value: expired_value,
        expiring_90_stock_value: exp90_value,
        items,
    })
}

pub async fn fetch_daily_collection(
    pool: &PgPool,
    from_date: String,
    to_date: String,
) -> Result<DailyCollectionReport, String> {
    // Payments and refunds aggregated per day; the join is on the date so
    // days with only refunds still appear.
    let rows: Vec<(String, i64, f64, f64)> = sqlx::query_as(&format!(
        r#"
        SELECT d::date::text AS date,
               COALESCE(p.cnt, 0)::bigint AS payments,
               COALESCE(p.amt, 0){F8} AS collected,
               COALESCE(r.amt, 0){F8} AS refunded
        FROM generate_series($1::date, $2::date, INTERVAL '1 day') AS d
        LEFT JOIN (
            SELECT paid_at::date AS day, COUNT(*) AS cnt, SUM(amount) AS amt
            FROM payments WHERE paid_at::date >= $1::date AND paid_at::date <= $2::date
            GROUP BY paid_at::date
        ) p ON p.day = d::date
        LEFT JOIN (
            SELECT refunded_at::date AS day, SUM(amount) AS amt
            FROM refunds WHERE refunded_at::date >= $1::date AND refunded_at::date <= $2::date
            GROUP BY refunded_at::date
        ) r ON r.day = d::date
        ORDER BY d
        "#,
    ))
    .bind(&from_date)
    .bind(&to_date)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Daily collection report: {e}"))?;
    let mut total_collected = 0.0;
    let mut total_refunded = 0.0;
    let by_day = rows
        .into_iter()
        .map(|(date, payments, collected, refunded)| {
            total_collected += collected;
            total_refunded += refunded;
            CollectionDayRow { date, payments, collected, refunded, net: collected - refunded }
        })
        .collect();
    Ok(DailyCollectionReport { from_date, to_date, total_collected, total_refunded, by_day })
}

pub async fn fetch_receivables_aging(pool: &PgPool) -> Result<ReceivablesAgingReport, String> {
    // Open bills (unpaid/partial/pending legacy) bucketed by age of the
    // OUTSTANDING part: net_amount − effective paid (payments − refunds).
    let rows: Vec<(String, i64, f64)> = sqlx::query_as(&format!(
        r#"
        SELECT bucket, COUNT(*)::bigint AS bill_count, SUM(outstanding){F8} AS outstanding
        FROM (
            SELECT b.id,
                   (b.net_amount
                    - COALESCE((SELECT SUM(amount) FROM payments WHERE bill_id = b.id), 0)
                    + COALESCE((SELECT SUM(amount) FROM refunds WHERE bill_id = b.id), 0)) AS outstanding,
                   CASE
                     WHEN CURRENT_DATE - b.created_at::date <= 30 THEN '0-30 days'
                     WHEN CURRENT_DATE - b.created_at::date <= 60 THEN '31-60 days'
                     WHEN CURRENT_DATE - b.created_at::date <= 90 THEN '61-90 days'
                     ELSE '90+ days'
                   END AS bucket
            FROM bills b
            WHERE b.status IN ('unpaid', 'partial', 'pending', 'draft')
        ) t
        WHERE outstanding > 0
        GROUP BY bucket
        ORDER BY bucket
        "#,
    ))
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Receivables aging report: {e}"))?;
    let total_outstanding = rows.iter().map(|r| r.2).sum();
    let total_open_bills = rows.iter().map(|r| r.1).sum();
    let buckets = rows
        .into_iter()
        .map(|(bucket, bill_count, outstanding)| AgingBucketRow { bucket, bill_count, outstanding })
        .collect();
    Ok(ReceivablesAgingReport {
        as_of_date: Utc::now().format("%Y-%m-%d").to_string(),
        total_outstanding,
        total_open_bills,
        buckets,
    })
}

pub async fn fetch_insurance_claims(pool: &PgPool) -> Result<InsuranceClaimsReport, String> {
    let rows: Vec<(String, i64, f64, f64)> = sqlx::query_as(&format!(
        "SELECT c.status, COUNT(*)::bigint AS claims, \
                COALESCE(SUM(c.claim_amount), 0){F8} AS claimed_amount, \
                COALESCE(SUM(c.approved_amount), 0){F8} AS approved_amount \
         FROM insurance_claims c \
         GROUP BY c.status \
         ORDER BY c.status",
    ))
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Insurance claims report: {e}"))?;
    let total_claims = rows.iter().map(|r| r.1).sum();
    let total_claimed = rows.iter().map(|r| r.2).sum();
    let total_approved = rows.iter().map(|r| r.3).sum();
    let by_status = rows
        .into_iter()
        .map(|(status, claims, claimed_amount, approved_amount)| {
            ClaimStatusRow { status, claims, claimed_amount, approved_amount }
        })
        .collect();
    Ok(InsuranceClaimsReport {
        as_of_date: Utc::now().format("%Y-%m-%d").to_string(),
        total_claims,
        total_claimed,
        total_approved,
        by_status,
    })
}

pub async fn fetch_stock_status(pool: &PgPool) -> Result<StockStatusReport, String> {
    let (active_items, total_stock_value, low_stock_count): (i64, f64, i64) =
        sqlx::query_as(&format!(
            "SELECT COUNT(*)::bigint, \
                    COALESCE(SUM(stock_quantity * unit_cost), 0){F8}, \
                    COUNT(*) FILTER (WHERE stock_quantity <= reorder_level)::bigint \
             FROM inventory_items WHERE is_active = TRUE",
        ))
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Stock status report (totals): {e}"))?;

    let rows: Vec<(String, f64, f64)> = sqlx::query_as(&format!(
        "SELECT name, stock_quantity{F8}, reorder_level{F8} \
         FROM inventory_items \
         WHERE is_active = TRUE AND stock_quantity <= reorder_level \
         ORDER BY (reorder_level - stock_quantity) DESC, name ASC \
         LIMIT 50",
    ))
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Stock status report (low stock): {e}"))?;
    let low_stock_items = rows
        .into_iter()
        .map(|(name, stock_quantity, reorder_level)| LowStockRow { name, stock_quantity, reorder_level })
        .collect();
    Ok(StockStatusReport {
        as_of_date: Utc::now().format("%Y-%m-%d").to_string(),
        active_items,
        total_stock_value,
        low_stock_count,
        low_stock_items,
    })
}

pub async fn fetch_user_activity(
    pool: &PgPool,
    from_date: String,
    to_date: String,
) -> Result<UserActivityReport, String> {
    // Aggregates only — raw audit rows stay behind the Audit-gated viewer.
    let rows: Vec<(String, Option<String>, i64, Option<String>)> = sqlx::query_as(
        "SELECT u.username, u.full_name, COUNT(*)::bigint AS action_count, \
                MAX(al.created_at)::text AS last_action_at \
         FROM audit_logs al \
         JOIN users u ON u.id = al.user_id \
         WHERE al.created_at::date >= $1::date AND al.created_at::date <= $2::date \
         GROUP BY u.id, u.username, u.full_name \
         ORDER BY COUNT(*) DESC, u.username ASC \
         LIMIT 50",
    )
    .bind(&from_date)
    .bind(&to_date)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("User activity report: {e}"))?;
    let total_actions = rows.iter().map(|r| r.2).sum();
    let by_user = rows
        .into_iter()
        .map(|(username, full_name, action_count, last_action_at)| {
            UserActivityRow { username, full_name, action_count, last_action_at }
        })
        .collect();
    Ok(UserActivityReport { from_date, to_date, total_actions, by_user })
}

async fn fetch_backup_status() -> Result<BackupStatusReport, String> {
    // Read-only listing of %ProgramData%\HMS\backups — metadata only (the
    // restore/delete paths remain in the server-build backup module behind
    // BackupsManage). This is a client-safe diagnostic so the report page
    // can surface "no backup in N days" without admin rights.
    let mut report = BackupStatusReport {
        backup_count: 0,
        latest_backup_age_days: None,
        total_size_bytes: 0,
        files: Vec::new(),
    };
    let Some(program_data) = std::env::var_os("ProgramData") else {
        return Ok(report);
    };
    let dir = std::path::Path::new(&program_data).join("HMS").join("backups");
    let Ok(read) = std::fs::read_dir(&dir) else {
        return Ok(report); // No backups yet (fresh install) — empty report.
    };
    let mut newest_age: Option<i64> = None;
    for entry in read.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let filename = entry.file_name().to_string_lossy().to_string();
        if !filename.ends_with(".sql") {
            continue;
        }
        let modified = meta
            .modified()
            .ok()
            .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let age_days = ((now - modified) / 86_400).max(0);
        report.backup_count += 1;
        report.total_size_bytes += meta.len() as i64;
        newest_age = Some(newest_age.map_or(age_days, |a| a.min(age_days)));
        report.files.push(BackupFileRow { filename, size_bytes: meta.len() as i64, age_days });
    }
    report.latest_backup_age_days = newest_age;
    // Oldest-first is unhelpful; newest-first matches list_backups.
    report.files.sort_by(|a, b| b.filename.cmp(&a.filename));
    Ok(report)
}

// ── Phase 6.4 CSV formatters ────────────────────────────────────────────────

fn format_doctor_performance_csv(r: DoctorPerformanceReport) -> String {
    let mut rows = vec![csv_row(&["Report".into(), "Doctor Performance".into()])];
    rows.push(csv_row(&["From".into(), r.from_date.clone()]));
    rows.push(csv_row(&["To".into(), r.to_date.clone()]));
    rows.push(String::new());
    rows.push(csv_row(&[
        "Doctor".into(), "Appointments".into(), "Encounters".into(),
        "Lab orders".into(), "Prescriptions".into(),
    ]));
    for d in &r.doctors {
        rows.push(csv_row(&[
            d.doctor_name.clone(), d.appointments.to_string(), d.encounters.to_string(),
            d.lab_orders.to_string(), d.prescriptions.to_string(),
        ]));
    }
    csv_join(rows)
}

fn format_diagnosis_frequency_csv(r: DiagnosisFrequencyReport) -> String {
    let mut rows = vec![csv_row(&["Report".into(), "Diagnosis Frequency".into()])];
    rows.push(csv_row(&["From".into(), r.from_date.clone()]));
    rows.push(csv_row(&["To".into(), r.to_date.clone()]));
    rows.push(csv_row(&["Diagnosed encounters".into(), r.total_encounters.to_string()]));
    rows.push(String::new());
    rows.push(csv_row(&["Diagnosis".into(), "Encounter count".into()]));
    for d in &r.top_diagnoses {
        rows.push(csv_row(&[d.diagnosis.clone(), d.encounter_count.to_string()]));
    }
    csv_join(rows)
}

fn format_pharmacy_consumption_csv(r: PharmacyConsumptionReport) -> String {
    let mut rows = vec![csv_row(&["Report".into(), "Pharmacy Consumption".into()])];
    rows.push(csv_row(&["From".into(), r.from_date.clone()]));
    rows.push(csv_row(&["To".into(), r.to_date.clone()]));
    rows.push(csv_row(&["Dispensed items".into(), r.total_dispensed_items.to_string()]));
    rows.push(String::new());
    rows.push(csv_row(&["Medication".into(), "Times dispensed".into(), "Total quantity".into()]));
    for m in &r.top_medications {
        rows.push(csv_row(&[
            m.medication_name.clone(), m.times_dispensed.to_string(), fmt_f64(m.total_quantity),
        ]));
    }
    csv_join(rows)
}

fn format_drug_expiry_csv(r: DrugExpiryReport) -> String {
    let mut rows = vec![csv_row(&["Report".into(), "Drug Expiry".into()])];
    rows.push(csv_row(&["Expired items".into(), r.expired_count.to_string()]));
    rows.push(csv_row(&["Expiring ≤90 days".into(), r.expiring_90_days.to_string()]));
    rows.push(csv_row(&["Expiring ≤180 days".into(), r.expiring_180_days.to_string()]));
    rows.push(csv_row(&["Expired stock value".into(), fmt_f64(r.expired_stock_value)]));
    rows.push(csv_row(&["Expiring-90 stock value".into(), fmt_f64(r.expiring_90_stock_value)]));
    rows.push(String::new());
    rows.push(csv_row(&[
        "Item".into(), "Batch".into(), "Stock".into(), "Expiry".into(),
        "Days left".into(), "Stock value".into(),
    ]));
    for i in &r.items {
        rows.push(csv_row(&[
            i.name.clone(),
            i.batch_number.clone().unwrap_or_else(|| "—".into()),
            fmt_f64(i.stock_quantity),
            i.expiry_date.clone().unwrap_or_else(|| "—".into()),
            i.days_until_expiry.map(|d| d.to_string()).unwrap_or_else(|| "—".into()),
            fmt_f64(i.stock_value),
        ]));
    }
    csv_join(rows)
}

fn format_daily_collection_csv(r: DailyCollectionReport) -> String {
    let mut rows = vec![csv_row(&["Report".into(), "Daily Collection".into()])];
    rows.push(csv_row(&["From".into(), r.from_date.clone()]));
    rows.push(csv_row(&["To".into(), r.to_date.clone()]));
    rows.push(csv_row(&["Total collected".into(), fmt_f64(r.total_collected)]));
    rows.push(csv_row(&["Total refunded".into(), fmt_f64(r.total_refunded)]));
    rows.push(String::new());
    rows.push(csv_row(&[
        "Date".into(), "Payments".into(), "Collected".into(), "Refunded".into(), "Net".into(),
    ]));
    for d in &r.by_day {
        rows.push(csv_row(&[
            d.date.clone(), d.payments.to_string(),
            fmt_f64(d.collected), fmt_f64(d.refunded), fmt_f64(d.net),
        ]));
    }
    csv_join(rows)
}

fn format_receivables_aging_csv(r: ReceivablesAgingReport) -> String {
    let mut rows = vec![csv_row(&["Report".into(), "Receivables Aging".into()])];
    rows.push(csv_row(&["As of".into(), r.as_of_date.clone()]));
    rows.push(csv_row(&["Open bills".into(), r.total_open_bills.to_string()]));
    rows.push(csv_row(&["Total outstanding".into(), fmt_f64(r.total_outstanding)]));
    rows.push(String::new());
    rows.push(csv_row(&["Age bucket".into(), "Bills".into(), "Outstanding".into()]));
    for b in &r.buckets {
        rows.push(csv_row(&[b.bucket.clone(), b.bill_count.to_string(), fmt_f64(b.outstanding)]));
    }
    csv_join(rows)
}

fn format_insurance_claims_csv(r: InsuranceClaimsReport) -> String {
    let mut rows = vec![csv_row(&["Report".into(), "Insurance Claims Status".into()])];
    rows.push(csv_row(&["As of".into(), r.as_of_date.clone()]));
    rows.push(csv_row(&["Total claims".into(), r.total_claims.to_string()]));
    rows.push(csv_row(&["Total claimed".into(), fmt_f64(r.total_claimed)]));
    rows.push(csv_row(&["Total approved".into(), fmt_f64(r.total_approved)]));
    rows.push(String::new());
    rows.push(csv_row(&["Status".into(), "Claims".into(), "Claimed".into(), "Approved".into()]));
    for s in &r.by_status {
        rows.push(csv_row(&[
            s.status.clone(), s.claims.to_string(), fmt_f64(s.claimed_amount), fmt_f64(s.approved_amount),
        ]));
    }
    csv_join(rows)
}

fn format_stock_status_csv(r: StockStatusReport) -> String {
    let mut rows = vec![csv_row(&["Report".into(), "Stock Status".into()])];
    rows.push(csv_row(&["As of".into(), r.as_of_date.clone()]));
    rows.push(csv_row(&["Active items".into(), r.active_items.to_string()]));
    rows.push(csv_row(&["Total stock value".into(), fmt_f64(r.total_stock_value)]));
    rows.push(csv_row(&["Low-stock items".into(), r.low_stock_count.to_string()]));
    rows.push(String::new());
    rows.push(csv_row(&["Item".into(), "Stock".into(), "Reorder level".into()]));
    for i in &r.low_stock_items {
        rows.push(csv_row(&[i.name.clone(), fmt_f64(i.stock_quantity), fmt_f64(i.reorder_level)]));
    }
    csv_join(rows)
}

fn format_user_activity_csv(r: UserActivityReport) -> String {
    let mut rows = vec![csv_row(&["Report".into(), "User Activity".into()])];
    rows.push(csv_row(&["From".into(), r.from_date.clone()]));
    rows.push(csv_row(&["To".into(), r.to_date.clone()]));
    rows.push(csv_row(&["Total actions".into(), r.total_actions.to_string()]));
    rows.push(String::new());
    rows.push(csv_row(&["User".into(), "Actions".into(), "Last action (UTC)".into()]));
    for u in &r.by_user {
        rows.push(csv_row(&[
            u.full_name.clone().unwrap_or_else(|| u.username.clone()),
            u.action_count.to_string(),
            u.last_action_at.clone().unwrap_or_else(|| "—".into()),
        ]));
    }
    csv_join(rows)
}

fn format_backup_status_csv(r: BackupStatusReport) -> String {
    let mut rows = vec![csv_row(&["Report".into(), "Backup Status".into()])];
    rows.push(csv_row(&["Backup count".into(), r.backup_count.to_string()]));
    rows.push(csv_row(&[
        "Latest backup age (days)".into(),
        r.latest_backup_age_days.map(|d| d.to_string()).unwrap_or_else(|| "never".into()),
    ]));
    rows.push(csv_row(&["Total size (bytes)".into(), r.total_size_bytes.to_string()]));
    rows.push(String::new());
    rows.push(csv_row(&["File".into(), "Size (bytes)".into(), "Age (days)".into()]));
    for f in &r.files {
        rows.push(csv_row(&[f.filename.clone(), f.size_bytes.to_string(), f.age_days.to_string()]));
    }
    csv_join(rows)
}

fn format_accounts_summary_csv(r: crate::models::AccountsSummary) -> String {
    let mut rows = vec![csv_row(&["Report".into(), "Accounts Summary".into()])];
    rows.push(csv_row(&["From".into(), r.from_date.clone()]));
    rows.push(csv_row(&["To".into(), r.to_date.clone()]));
    rows.push(csv_row(&["Total collected".into(), fmt_f64(r.total_revenue + r.total_refunded)]));
    rows.push(csv_row(&["Total refunded".into(), fmt_f64(r.total_refunded)]));
    rows.push(csv_row(&["Net revenue".into(), fmt_f64(r.total_revenue)]));
    rows.push(csv_row(&["Total expenses".into(), fmt_f64(r.total_expenses)]));
    rows.push(csv_row(&["Net position".into(), fmt_f64(r.net_position)]));
    rows.push(String::new());
    rows.push(csv_row(&["Category".into(), "Total".into(), "Count".into()]));
    for c in &r.by_category {
        rows.push(csv_row(&[c.category.clone(), fmt_f64(c.total), c.count.to_string()]));
    }
    csv_join(rows)
}

// ── Phase 6.4 Tauri commands ────────────────────────────────────────────────

#[tauri::command]
pub async fn get_doctor_performance_report(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    from_date: String,
    to_date: String,
) -> Result<DoctorPerformanceReport, String> {
    let _ = rbac::require(&session, Permission::ReportsView)?;
    fetch_doctor_performance(pool.inner(), from_date, to_date).await
}

#[tauri::command]
pub async fn get_diagnosis_frequency_report(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    from_date: String,
    to_date: String,
) -> Result<DiagnosisFrequencyReport, String> {
    let _ = rbac::require(&session, Permission::ReportsView)?;
    fetch_diagnosis_frequency(pool.inner(), from_date, to_date).await
}

#[tauri::command]
pub async fn get_pharmacy_consumption_report(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    from_date: String,
    to_date: String,
) -> Result<PharmacyConsumptionReport, String> {
    let _ = rbac::require(&session, Permission::ReportsView)?;
    fetch_pharmacy_consumption(pool.inner(), from_date, to_date).await
}

#[tauri::command]
pub async fn get_drug_expiry_report(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
) -> Result<DrugExpiryReport, String> {
    let _ = rbac::require(&session, Permission::ReportsView)?;
    fetch_drug_expiry(pool.inner()).await
}

#[tauri::command]
pub async fn get_daily_collection_report(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    from_date: String,
    to_date: String,
) -> Result<DailyCollectionReport, String> {
    let _ = rbac::require(&session, Permission::ReportsView)?;
    fetch_daily_collection(pool.inner(), from_date, to_date).await
}

#[tauri::command]
pub async fn get_receivables_aging_report(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
) -> Result<ReceivablesAgingReport, String> {
    let _ = rbac::require(&session, Permission::ReportsView)?;
    fetch_receivables_aging(pool.inner()).await
}

#[tauri::command]
pub async fn get_insurance_claims_report(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
) -> Result<InsuranceClaimsReport, String> {
    let _ = rbac::require(&session, Permission::ReportsView)?;
    fetch_insurance_claims(pool.inner()).await
}

#[tauri::command]
pub async fn get_stock_status_report(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
) -> Result<StockStatusReport, String> {
    let _ = rbac::require(&session, Permission::ReportsView)?;
    fetch_stock_status(pool.inner()).await
}

/// Aggregated audit activity per user. In addition to ReportsView this
/// requires AuditView — the aggregation derives from the audit trail, which
/// must not leak to report-only viewers.
#[tauri::command]
pub async fn get_user_activity_report(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    from_date: String,
    to_date: String,
) -> Result<UserActivityReport, String> {
    let _ = rbac::require(&session, Permission::ReportsView)?;
    let _ = rbac::require(&session, Permission::AuditView)?;
    fetch_user_activity(pool.inner(), from_date, to_date).await
}

#[tauri::command]
pub async fn get_backup_status_report(
    _pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
) -> Result<BackupStatusReport, String> {
    let _ = rbac::require(&session, Permission::ReportsView)?;
    fetch_backup_status().await
}

/// Generic CSV exporter. `report_type` selects the report; `params` is a
/// JSON object whose shape depends on the report:
///   - `daily_opd`           → `{"date": "YYYY-MM-DD"}` (optional, defaults to today)
///   - `ipd_census`          → `{"date": "YYYY-MM-DD"}` (optional, defaults to today)
///   - `revenue`             → `{"from_date", "to_date"}` (required)
///   - `lab_turnaround`      → `{"from_date", "to_date"}` (required)
///   - `doctor_performance`  → `{"from_date", "to_date"}` (required)
///   - `diagnosis_frequency` → `{"from_date", "to_date"}` (required)
///   - `pharmacy_consumption`→ `{"from_date", "to_date"}` (required)
///   - `daily_collection`    → `{"from_date", "to_date"}` (required)
///   - `user_activity`       → `{"from_date", "to_date"}` (required)
///   - `drug_expiry` / `receivables_aging` / `insurance_claims` /
///     `stock_status` / `backup_status` → `{}` (as-of reports, no params)
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
        "doctor_performance" => {
            let from_date = range_param(&p, "from_date", "doctor_performance")?;
            let to_date = range_param(&p, "to_date", "doctor_performance")?;
            let report = fetch_doctor_performance(pool, from_date, to_date).await?;
            Ok(format_doctor_performance_csv(report))
        }
        "diagnosis_frequency" => {
            let from_date = range_param(&p, "from_date", "diagnosis_frequency")?;
            let to_date = range_param(&p, "to_date", "diagnosis_frequency")?;
            let report = fetch_diagnosis_frequency(pool, from_date, to_date).await?;
            Ok(format_diagnosis_frequency_csv(report))
        }
        "pharmacy_consumption" => {
            let from_date = range_param(&p, "from_date", "pharmacy_consumption")?;
            let to_date = range_param(&p, "to_date", "pharmacy_consumption")?;
            let report = fetch_pharmacy_consumption(pool, from_date, to_date).await?;
            Ok(format_pharmacy_consumption_csv(report))
        }
        "daily_collection" => {
            let from_date = range_param(&p, "from_date", "daily_collection")?;
            let to_date = range_param(&p, "to_date", "daily_collection")?;
            let report = fetch_daily_collection(pool, from_date, to_date).await?;
            Ok(format_daily_collection_csv(report))
        }
        "user_activity" => {
            let from_date = range_param(&p, "from_date", "user_activity")?;
            let to_date = range_param(&p, "to_date", "user_activity")?;
            // Same dual gate as the typed command — the CSV derives from
            // the audit trail.
            let _ = rbac::require(&session, Permission::AuditView)?;
            let report = fetch_user_activity(pool, from_date, to_date).await?;
            Ok(format_user_activity_csv(report))
        }
        "drug_expiry" => {
            let report = fetch_drug_expiry(pool).await?;
            Ok(format_drug_expiry_csv(report))
        }
        "receivables_aging" => {
            let report = fetch_receivables_aging(pool).await?;
            Ok(format_receivables_aging_csv(report))
        }
        "insurance_claims" => {
            let report = fetch_insurance_claims(pool).await?;
            Ok(format_insurance_claims_csv(report))
        }
        "stock_status" => {
            let report = fetch_stock_status(pool).await?;
            Ok(format_stock_status_csv(report))
        }
        "backup_status" => {
            let report = fetch_backup_status().await?;
            Ok(format_backup_status_csv(report))
        }
        "accounts_summary" => {
            // SRS §2.15 (Phase 8) — the summary fetcher lives in the
            // accounts module; the CSV surface stays unified here.
            let from_date = range_param(&p, "from_date", "accounts_summary")?;
            let to_date = range_param(&p, "to_date", "accounts_summary")?;
            let r = crate::commands::accounts::fetch_accounts_summary(pool, from_date.clone(), to_date.clone()).await?;
            Ok(format_accounts_summary_csv(r))
        }
        other => Err(format!(
            "export_report_csv: unknown report_type '{other}'. Expected one of: \
             daily_opd, ipd_census, revenue, lab_turnaround, doctor_performance, \
             diagnosis_frequency, pharmacy_consumption, drug_expiry, daily_collection, \
             receivables_aging, insurance_claims, stock_status, user_activity, backup_status."
        )),
    }
}

/// Extract a mandatory range parameter for the CSV exporter dispatch.
fn range_param(
    p: &serde_json::Value,
    key: &str,
    report_type: &str,
) -> Result<String, String> {
    p.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("export_report_csv: '{key}' is required for the {report_type} report"))
}
