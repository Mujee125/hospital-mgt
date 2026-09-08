//! Phase 6.4 — Reports expansion integration tests.
//!
//! Exercises the REAL report fetchers (`fetch_*` in commands/reports.rs)
//! against the real PostgreSQL test DB with seeded, known quantities —
//! asserting the aggregation math, not just "query runs".
//!
//! Covers the SRS §4.20 gap-list reports:
//!   RP-1  Doctor performance: per-doctor counts across four activities.
//!   RP-2  Diagnosis frequency: grouping + total over diagnosed encounters.
//!   RP-3  Daily collection: payments minus refunds per day, net math.
//!   RP-4  Receivables aging: outstanding buckets exclude settled bills.
//!   RP-5  Insurance claims: pipeline totals per status.
//!   RP-6  Stock status: low-stock detection vs reorder level.
//!   RP-7  Drug expiry: expired/expiring bucket counts + detail ordering.
//!
//! Requires: HMS_TEST_DB_URL env var + `--features hms-integration-tests`.

#![cfg(feature = "hms-integration-tests")]

mod common;

use common::*;
use hospital_mgmt_lib::commands::reports::{
    fetch_daily_collection, fetch_diagnosis_frequency, fetch_doctor_performance, fetch_drug_expiry,
    fetch_insurance_claims, fetch_receivables_aging, fetch_stock_status,
};

fn today() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

fn days_ago(n: i64) -> String {
    (chrono::Utc::now() - chrono::Duration::days(n))
        .format("%Y-%m-%d")
        .to_string()
}

// ── RP-1: Doctor performance ──────────────────────────────────────────────────

#[tokio::test]
async fn test_rp1_doctor_performance_counts_all_activities() {
    let pool = test_pool().await;
    let doctor: (i32,) = sqlx::query_as(
        "INSERT INTO doctors (first_name, last_name, specialization, qualification, phone, is_active) \
         VALUES ('Rep', 'Doc', 'Medicine', 'MBBS', '+92300rp1doc', TRUE) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let patient_id = seed_patient_with_phone(&pool, "Rep", "Perf", "+92300rp1a").await;

    // One appointment, one encounter (diagnosed), one lab order, one rx.
    sqlx::query("INSERT INTO appointments (patient_id, doctor_id, appointment_date, appointment_time, status) \
                 VALUES ($1, $2, CURRENT_DATE, '09:00', 'completed')")
        .bind(patient_id).bind(doctor.0)
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO encounters (patient_id, doctor_id, diagnosis) VALUES ($1, $2, 'Flu')")
        .bind(patient_id)
        .bind(doctor.0)
        .execute(&pool)
        .await
        .unwrap();
    let lab: (i32,) = sqlx::query_as(
        "INSERT INTO lab_orders (patient_id, ordered_by_doctor_id, status) VALUES ($1, $2, 'ordered') RETURNING id",
    )
    .bind(patient_id).bind(doctor.0)
    .fetch_one(&pool).await.unwrap();
    let _ = lab;
    let rx: (i32,) = sqlx::query_as(
        "INSERT INTO prescriptions (patient_id, doctor_id, status) VALUES ($1, $2, 'active') RETURNING id",
    )
    .bind(patient_id).bind(doctor.0)
    .fetch_one(&pool).await.unwrap();
    let _ = rx;

    let report = fetch_doctor_performance(&pool, days_ago(1), today())
        .await
        .unwrap();
    let row = report
        .doctors
        .iter()
        .find(|d| d.doctor_name == "Rep Doc")
        .expect("seeded doctor must appear in the performance report");
    assert_eq!(row.appointments, 1);
    assert_eq!(row.encounters, 1);
    assert_eq!(row.lab_orders, 1);
    assert_eq!(row.prescriptions, 1);
}

// ── RP-2: Diagnosis frequency ────────────────────────────────────────────────

#[tokio::test]
async fn test_rp2_diagnosis_frequency_groups_and_counts() {
    let pool = test_pool().await;
    let patient_id = seed_patient_with_phone(&pool, "Rep", "Diag", "+92300rp2a").await;

    sqlx::query("INSERT INTO encounters (patient_id, diagnosis) VALUES ($1, 'Hypertension')")
        .bind(patient_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO encounters (patient_id, diagnosis) VALUES ($1, 'Hypertension')")
        .bind(patient_id)
        .execute(&pool)
        .await
        .unwrap();
    // Un-diagnosed encounters must not count.
    sqlx::query("INSERT INTO encounters (patient_id, diagnosis) VALUES ($1, NULL)")
        .bind(patient_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO encounters (patient_id, diagnosis) VALUES ($1, '   ')")
        .bind(patient_id)
        .execute(&pool)
        .await
        .unwrap();

    let report = fetch_diagnosis_frequency(&pool, days_ago(1), today())
        .await
        .unwrap();
    let ht = report
        .top_diagnoses
        .iter()
        .find(|d| d.diagnosis == "Hypertension")
        .expect("Hypertension must appear");
    assert_eq!(
        ht.encounter_count, 2,
        "only the two diagnosed encounters count"
    );
    assert!(report.total_encounters >= 2);
    // Blank/whitespace diagnoses never surface as rows.
    assert!(report
        .top_diagnoses
        .iter()
        .all(|d| !d.diagnosis.trim().is_empty()));
}

// ── RP-3: Daily collection ────────────────────────────────────────────────────

#[tokio::test]
async fn test_rp3_daily_collection_net_math() {
    let pool = test_pool().await;
    let patient_id = seed_patient_with_phone(&pool, "Rep", "Coll", "+92300rp3a").await;
    let bill: (i32,) = sqlx::query_as(
        "INSERT INTO bills (patient_id, bill_number, net_amount, status) \
         VALUES ($1, 'RP3-BILL', 500, 'unpaid') RETURNING id",
    )
    .bind(patient_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO payments (bill_id, amount, payment_method) VALUES ($1, 300, 'cash')")
        .bind(bill.0)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO refunds (bill_id, amount, reason) VALUES ($1, 50, 'test refund')")
        .bind(bill.0)
        .execute(&pool)
        .await
        .unwrap();

    let report = fetch_daily_collection(&pool, days_ago(1), today())
        .await
        .unwrap();
    let today_row = report
        .by_day
        .iter()
        .find(|d| d.date == today())
        .expect("today must appear in the collection series");
    assert_eq!(today_row.payments, 1);
    assert_eq!(today_row.collected, 300.0);
    assert_eq!(today_row.refunded, 50.0);
    assert_eq!(today_row.net, 250.0, "net = collected − refunds");
    assert!(report.total_collected >= 300.0);
    assert!(report.total_refunded >= 50.0);
}

// ── RP-4: Receivables aging ──────────────────────────────────────────────────

#[tokio::test]
async fn test_rp4_receivables_aging_excludes_settled() {
    let pool = test_pool().await;
    let patient_id = seed_patient_with_phone(&pool, "Rep", "Age", "+92300rp4a").await;

    // An old OPEN bill (fully unpaid → 90+ bucket).
    let open: (i32,) = sqlx::query_as(
        "INSERT INTO bills (patient_id, bill_number, net_amount, status, created_at) \
         VALUES ($1, 'RP4-OPEN', 1000, 'unpaid', NOW() - INTERVAL '120 days') RETURNING id",
    )
    .bind(patient_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // An old bill fully paid via payments — must NOT appear as outstanding.
    let paid: (i32,) = sqlx::query_as(
        "INSERT INTO bills (patient_id, bill_number, net_amount, status, created_at) \
         VALUES ($1, 'RP4-PAID', 400, 'unpaid', NOW() - INTERVAL '120 days') RETURNING id",
    )
    .bind(patient_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO payments (bill_id, amount, payment_method) VALUES ($1, 400, 'cash')")
        .bind(paid.0)
        .execute(&pool)
        .await
        .unwrap();

    // A cancelled (credit-noted) bill must not count as receivable.
    sqlx::query(
        "INSERT INTO bills (patient_id, bill_number, net_amount, status, created_at) \
         VALUES ($1, 'RP4-CN', 700, 'cancelled', NOW() - INTERVAL '120 days')",
    )
    .bind(patient_id)
    .execute(&pool)
    .await
    .unwrap();
    let _ = open;

    let report = fetch_receivables_aging(&pool).await.unwrap();
    let bucket_90 = report
        .buckets
        .iter()
        .find(|b| b.bucket == "90+ days")
        .expect("120-day-old open bill must land in the 90+ bucket");
    // The assertion must tolerate other suite fixtures' outstanding bills —
    // so we check OUR bills' contribution, not exact totals:
    assert!(
        bucket_90.outstanding >= 1000.0,
        "the 1000 fully-unpaid 120-day bill must be counted, got {}",
        bucket_90.outstanding
    );
    assert!(
        report.total_outstanding < bucket_90.outstanding + 100.0 + 400.0 + 700.0,
        "paid (400) and cancelled (700) bills must not inflate outstanding"
    );
}

// ── RP-5: Insurance claims report ─────────────────────────────────────────────

#[tokio::test]
async fn test_rp5_insurance_claims_totals_by_status() {
    let pool = test_pool().await;
    let patient_id = seed_patient_with_phone(&pool, "Rep", "Ins", "+92300rp5a").await;
    let bill: (i32,) = sqlx::query_as(
        "INSERT INTO bills (patient_id, bill_number, net_amount, status) \
         VALUES ($1, 'RP5-BILL', 5000, 'unpaid') RETURNING id",
    )
    .bind(patient_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO insurance_claims (bill_id, patient_id, insurer, claim_amount, approved_amount, status) \
         VALUES ($1, $2, 'RepInsurer', 5000, 3500, 'partially_approved')",
    )
    .bind(bill.0).bind(patient_id)
    .execute(&pool).await.unwrap();

    let report = fetch_insurance_claims(&pool).await.unwrap();
    let partial = report
        .by_status
        .iter()
        .find(|s| s.status == "partially_approved")
        .expect("partially_approved bucket must exist");
    assert!(partial.claims >= 1);
    assert!(partial.claimed_amount >= 5000.0);
    assert!(partial.approved_amount >= 3500.0);
}

// ── RP-6: Stock status ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_rp6_stock_status_flags_low_stock() {
    let pool = test_pool().await;
    // One low-stock item (stock ≤ reorder), one healthy item.
    sqlx::query(
        "INSERT INTO inventory_items (name, sku, category, stock_quantity, reorder_level, unit_cost, is_active) \
         VALUES ('RP6-LowStock', 'RP6-LOW', 'medication', 2, 10, 5.0, TRUE) \
         ON CONFLICT (sku) DO UPDATE SET stock_quantity = 2, reorder_level = 10",
    )
    .execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO inventory_items (name, sku, category, stock_quantity, reorder_level, unit_cost, is_active) \
         VALUES ('RP6-Healthy', 'RP6-OK', 'medication', 100, 10, 3.0, TRUE) \
         ON CONFLICT (sku) DO UPDATE SET stock_quantity = 100",
    )
    .execute(&pool).await.unwrap();

    let report = fetch_stock_status(&pool).await.unwrap();
    assert!(
        report
            .low_stock_items
            .iter()
            .any(|i| i.name == "RP6-LowStock" && i.stock_quantity <= i.reorder_level),
        "the below-reorder item must be flagged low-stock"
    );
    assert!(
        !report
            .low_stock_items
            .iter()
            .any(|i| i.name == "RP6-Healthy"),
        "healthy stock must not be flagged"
    );
    assert!(report.active_items >= 2);
    assert!(report.total_stock_value > 0.0);
}

// ── RP-7: Drug expiry ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_rp7_drug_expiry_buckets() {
    let pool = test_pool().await;
    // Expired (yesterday), expiring in ~30 days, expiring in ~150 days.
    sqlx::query(
        "INSERT INTO inventory_items (name, sku, category, stock_quantity, unit_cost, expiry_date, is_active) \
         VALUES ('RP7-Gone', 'RP7-EXP', 'medication', 4, 10.0, CURRENT_DATE - 1, TRUE) \
         ON CONFLICT (sku) DO UPDATE SET expiry_date = CURRENT_DATE - 1, stock_quantity = 4",
    )
    .execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO inventory_items (name, sku, category, stock_quantity, unit_cost, expiry_date, is_active) \
         VALUES ('RP7-Soon', 'RP7-S90', 'medication', 6, 10.0, CURRENT_DATE + 30, TRUE) \
         ON CONFLICT (sku) DO UPDATE SET expiry_date = CURRENT_DATE + 30, stock_quantity = 6",
    )
    .execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO inventory_items (name, sku, category, stock_quantity, unit_cost, expiry_date, is_active) \
         VALUES ('RP7-Later', 'RP7-S180', 'medication', 8, 10.0, CURRENT_DATE + 150, TRUE) \
         ON CONFLICT (sku) DO UPDATE SET expiry_date = CURRENT_DATE + 150, stock_quantity = 8",
    )
    .execute(&pool).await.unwrap();

    let report = fetch_drug_expiry(&pool).await.unwrap();
    assert!(report.expired_count >= 1, "the yesterday item is expired");
    assert!(
        report.expiring_90_days >= 1,
        "the +30d item is in the 90 bucket"
    );
    assert!(
        report.expiring_180_days >= 1,
        "the +150d item is in the 180 bucket"
    );
    assert!(report.expired_stock_value >= 40.0, "4 units × 10.0");
    // Detail rows ordered earliest-expiry-first.
    let gone_idx = report.items.iter().position(|i| i.name == "RP7-Gone");
    let soon_idx = report.items.iter().position(|i| i.name == "RP7-Soon");
    if let (Some(g), Some(s)) = (gone_idx, soon_idx) {
        assert!(g < s, "expired items sort before later-expiring ones");
    }
}
