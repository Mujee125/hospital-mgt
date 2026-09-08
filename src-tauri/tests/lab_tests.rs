//! Phase 6.2 — Lab workflow command-level integration tests.
//!
//! Exercises the REAL production cores (`collect_lab_sample_core`,
//! `approve_lab_result_core`) plus `update_lab_result` against the real
//! PostgreSQL test DB — the exact functions the Tauri commands call.
//!
//! Covers the four SRS §2.4 workflow guarantees:
//!   LW-1  Sample collection is a one-way state door: only 'ordered' rows
//!         can be collected; collection stamps a unique barcode + user.
//!   LW-2  Result entry routes through approval: results land
//!         approval_status='entered', order moves to 'resulted', never
//!         directly to released.
//!   LW-3  Approval authority is separate from entry authority:
//!         LabApprove is required (receptionists/nurses/pharmacists
//!         rejected); only when ALL tests are approved does the order
//!         reach 'approved'.
//!   LW-4  Critical-value protocol: a critical result cannot be released
//!         without acknowledgment (command guard + DB CHECK both enforce
//!         it — the CHECK is the defense-in-depth backstop).
//!
//! `update_lab_result` takes tauri::State (pre-6.2 surface, unchanged
//! signature); its SQL behavior is asserted directly here through the
//! same statements the command runs, mirroring the integration_tests.rs
//! pattern for State-bound logic.
//!
//! Requires: HMS_TEST_DB_URL env var + `--features hms-integration-tests`.

#![cfg(feature = "hms-integration-tests")]

mod common;

use common::*;
use hospital_mgmt_lib::commands::lab::{approve_lab_result_core, collect_lab_sample_core};
use hospital_mgmt_lib::rbac::SessionState;
use sqlx::PgPool;
use std::sync::{Arc, Mutex};

// ── Local fixtures ────────────────────────────────────────────────────────────

/// Insert a lab order with N catalog tests; return (order_id, [test_row_ids]).
/// Mirrors create_lab_order's INSERTs without the LabOrder permission so
/// low-privilege fixtures can be staged for the workflow tests.
async fn seed_lab_order(pool: &PgPool, patient_id: i32, n_tests: i32) -> (i32, Vec<i32>) {
    let order: (i32,) = sqlx::query_as(
        "INSERT INTO lab_orders (patient_id, status) VALUES ($1, 'ordered') RETURNING id",
    )
    .bind(patient_id)
    .fetch_one(pool)
    .await
    .expect("seed lab order");
    let mut tests = Vec::new();
    for i in 0..n_tests {
        // Reuse catalog rows when present; create the minimum otherwise.
        let tc: (i32,) = if let Some(row) = sqlx::query_as::<_, (i32,)>(
            "SELECT id FROM lab_test_catalog WHERE is_active = TRUE ORDER BY id LIMIT 1 OFFSET $1",
        )
        .bind(i)
        .fetch_optional(pool)
        .await
        .expect("catalog lookup")
        {
            row
        } else {
            sqlx::query_as::<_, (i32,)>(
                "INSERT INTO lab_test_catalog (name, code, price) \
                 VALUES ($1, $1, 0) ON CONFLICT (code) DO UPDATE SET name = EXCLUDED.name \
                 RETURNING id",
            )
            .bind(format!("LW-TC-{}", i))
            .fetch_one(pool)
            .await
            .expect("seed catalog test")
        };
        let row: (i32,) = sqlx::query_as(
            "INSERT INTO lab_order_tests (lab_order_id, test_catalog_id) VALUES ($1, $2) RETURNING id",
        )
        .bind(order.0)
        .bind(tc.0)
        .fetch_one(pool)
        .await
        .expect("seed order test");
        tests.push(row.0);
    }
    (order.0, tests)
}

async fn state_for(pool: &PgPool, user_id: i32, token_hash: &str) -> SessionState {
    let s = load_session_for(pool, user_id, token_hash).await;
    Arc::new(Mutex::new(Some(s)))
}

async fn enter_result(pool: &PgPool, test_row_id: i32, value: &str, flag: Option<&str>) {
    sqlx::query(
        "UPDATE lab_order_tests SET \
              result_value = $1, result_abnormal_flag = $2, completed_at = NOW(), \
              approval_status = CASE WHEN approval_status = 'approved' THEN 'amended' ELSE 'entered' END \
         WHERE id = $3",
    )
    .bind(value)
    .bind(flag)
    .bind(test_row_id)
    .execute(pool)
    .await
    .expect("enter result (mirrors update_lab_result core statement)");
    sqlx::query(
        "UPDATE lab_orders SET status = 'resulted' \
         WHERE id = (SELECT lab_order_id FROM lab_order_tests WHERE id = $1) \
           AND status IN ('ordered', 'sampled')",
    )
    .bind(test_row_id)
    .execute(pool)
    .await
    .expect("stamp order resulted (mirrors update_lab_result core statement)");
}

// ── LW-1: Sample collection state rules ───────────────────────────────────────

/// Collection works exactly once from 'ordered', stamps barcode + collector,
/// and moves the order to 'sampled'. A second attempt is rejected — a lost
/// sample must go through an audited reset, not silent recollection.
#[tokio::test]
async fn test_lw1_collect_sample_state_door() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let tech_id = seed_user(&pool, "lw1_tech", &pw, &["lab_technician"]).await;
    seed_session_row(&pool, tech_id, "hash_lw1_tech").await;
    let tech_state = state_for(&pool, tech_id, "hash_lw1_tech").await;

    let patient_id = seed_patient_with_phone(&pool, "Lab", "Sample", "+92300lw1a").await;
    let (order_id, tests) = seed_lab_order(&pool, patient_id, 2).await;
    assert!(!tests.is_empty());

    // Receptionist (no LabResultManage) cannot collect.
    let rx_id = seed_user(&pool, "lw1_rx", &pw, &["receptionist"]).await;
    seed_session_row(&pool, rx_id, "hash_lw1_rx").await;
    let rx_state = state_for(&pool, rx_id, "hash_lw1_rx").await;
    let err = collect_lab_sample_core(&pool, &rx_state, order_id)
        .await
        .unwrap_err();
    assert!(
        err.contains("requires the 'lab.result.manage'"),
        "receptionist must be denied sample collection, got: {}",
        err
    );

    // Tech collects: barcode + attribution + status transition.
    let barcode = collect_lab_sample_core(&pool, &tech_state, order_id)
        .await
        .expect("tech must collect the sample");
    assert!(
        barcode.starts_with(&format!("{}-", order_id)),
        "barcode must be '<order_id>-<test_row>' shaped, got: {}",
        barcode
    );
    let stored: (String, Option<i32>) =
        sqlx::query_as("SELECT status, sampled_by_user_id FROM lab_orders WHERE id = $1")
            .bind(order_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stored.0, "sampled");
    assert_eq!(stored.1, Some(tech_id));

    // Second collection from 'sampled' is rejected by the state guard.
    let err = collect_lab_sample_core(&pool, &tech_state, order_id)
        .await
        .unwrap_err();
    assert!(
        err.contains("awaiting sample collection"),
        "double-collection must be rejected, got: {}",
        err
    );
}

// ── LW-2/LW-3: Approval workflow + authority split ────────────────────────────

/// The full happy path: collect → enter all results → order 'resulted' →
/// approve all → order 'approved' with approver attribution. Also proves
/// the order stays 'resulted' while ANY test is unapproved.
#[tokio::test]
async fn test_lw2_full_workflow_resulted_then_approved() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let tech_id = seed_user(&pool, "lw2_tech", &pw, &["lab_technician"]).await;
    seed_session_row(&pool, tech_id, "hash_lw2_tech").await;
    let tech_state = state_for(&pool, tech_id, "hash_lw2_tech").await;

    let patient_id = seed_patient_with_phone(&pool, "Lab", "Flow", "+92300lw2a").await;
    let (order_id, tests) = seed_lab_order(&pool, patient_id, 2).await;
    collect_lab_sample_core(&pool, &tech_state, order_id)
        .await
        .expect("collect");

    // Enter both results (mirrors the command's statements).
    enter_result(&pool, tests[0], "98", Some("normal")).await;
    enter_result(&pool, tests[1], "5.2", Some("high")).await;

    let (status,): (String,) = sqlx::query_as("SELECT status FROM lab_orders WHERE id = $1")
        .bind(order_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        status, "resulted",
        "fully-entered order must await approval, never auto-release"
    );

    // Approve the first test only — order must REMAIN 'resulted'.
    approve_lab_result_core(&pool, &tech_state, tests[0], false)
        .await
        .expect("lab tech (in-charge role v1) approves");
    let (status,): (String,) = sqlx::query_as("SELECT status FROM lab_orders WHERE id = $1")
        .bind(order_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        status, "resulted",
        "order with one unapproved test must not be 'approved'"
    );

    // Approve the second → order 'approved' with attribution.
    approve_lab_result_core(&pool, &tech_state, tests[1], false)
        .await
        .expect("approve second test");
    let (status, approver): (String, Option<i32>) =
        sqlx::query_as("SELECT status, approved_by_user_id FROM lab_orders WHERE id = $1")
            .bind(order_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "approved");
    assert_eq!(approver, Some(tech_id));

    // Already-approved rows cannot be re-approved.
    let err = approve_lab_result_core(&pool, &tech_state, tests[0], false)
        .await
        .unwrap_err();
    assert!(
        err.contains("not awaiting approval"),
        "re-approval must be rejected, got: {}",
        err
    );
}

/// Nurses hold IpdManage but NOT LabApprove — the approval authority is a
/// separate grant, so a nurse cannot release results.
#[tokio::test]
async fn test_lw3_nurse_cannot_approve_results() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let nurse_id = seed_user(&pool, "lw3_nurse", &pw, &["nurse"]).await;
    seed_session_row(&pool, nurse_id, "hash_lw3_nurse").await;
    let nurse_state = state_for(&pool, nurse_id, "hash_lw3_nurse").await;

    let patient_id = seed_patient_with_phone(&pool, "Lab", "Authz", "+92300lw3a").await;
    let (order_id, tests) = seed_lab_order(&pool, patient_id, 1).await;
    let _ = order_id;
    enter_result(&pool, tests[0], "7.1", Some("normal")).await;

    let err = approve_lab_result_core(&pool, &nurse_state, tests[0], false)
        .await
        .unwrap_err();
    assert!(
        err.contains("requires the 'lab.approve'"),
        "nurse must be denied result approval, got: {}",
        err
    );

    // Unapproved state remains.
    let (status,): (Option<String>,) =
        sqlx::query_as("SELECT approval_status FROM lab_order_tests WHERE id = $1")
            .bind(tests[0])
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status.as_deref(), Some("entered"));
}

// ── LW-4: Critical-value protocol ──────────────────────────────────────────────

/// A critical result cannot be released without acknowledgment: the command
/// guard rejects, and — defense-in-depth — the DB CHECK rejects a direct
/// UPDATE that would release an unacknowledged critical row.
#[tokio::test]
async fn test_lw4_critical_release_requires_acknowledgment() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let tech_id = seed_user(&pool, "lw4_tech", &pw, &["lab_technician"]).await;
    seed_session_row(&pool, tech_id, "hash_lw4_tech").await;
    let tech_state = state_for(&pool, tech_id, "hash_lw4_tech").await;

    let patient_id = seed_patient_with_phone(&pool, "Lab", "Crit", "+92300lw4a").await;
    let (_order_id, tests) = seed_lab_order(&pool, patient_id, 1).await;
    enter_result(&pool, tests[0], "19.5", Some("critical")).await;

    // Without acknowledgment: rejected by the command guard.
    let err = approve_lab_result_core(&pool, &tech_state, tests[0], false)
        .await
        .unwrap_err();
    assert!(
        err.contains("CRITICAL") && err.contains("contacted"),
        "unacknowledged critical release must be rejected with the protocol message, got: {}",
        err
    );

    // Defense-in-depth: even a raw SQL attempt to release the unacknowledged
    // critical row violates chk_lot_critical_release.
    let raw = sqlx::query(
        "UPDATE lab_order_tests SET approval_status = 'approved' \
         WHERE id = $1 AND result_abnormal_flag = 'critical' AND critical_acknowledged_at IS NULL",
    )
    .bind(tests[0])
    .execute(&pool)
    .await;
    match raw {
        Ok(res) => {
            assert_eq!(
                res.rows_affected(),
                0,
                "CHECK should have blocked the release, or no row matched"
            );
        }
        Err(_) => {
            // Postgres raises the CHECK violation — both outcomes prove the backstop.
        }
    }

    // With acknowledgment: release succeeds and stamps the ack audit columns.
    approve_lab_result_core(&pool, &tech_state, tests[0], true)
        .await
        .expect("acknowledged critical release must succeed");
    let (status, ack_by): (Option<String>, Option<i32>) = sqlx::query_as(
        "SELECT approval_status, critical_acknowledged_by_user_id FROM lab_order_tests WHERE id = $1",
    )
    .bind(tests[0])
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status.as_deref(), Some("approved"));
    assert_eq!(
        ack_by,
        Some(tech_id),
        "acknowledgment must be attributed to the approver"
    );
}

/// An amended (post-release corrected) result must go back through approval
/// — the 'amended' status is releasable but never silently released.
#[tokio::test]
async fn test_lw4_amendment_requires_reapproval() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let tech_id = seed_user(&pool, "lw4b_tech", &pw, &["lab_technician"]).await;
    seed_session_row(&pool, tech_id, "hash_lw4b_tech").await;
    let tech_state = state_for(&pool, tech_id, "hash_lw4b_tech").await;

    let patient_id = seed_patient_with_phone(&pool, "Lab", "Amd", "+92300lw4b").await;
    let (_order_id, tests) = seed_lab_order(&pool, patient_id, 1).await;
    enter_result(&pool, tests[0], "4.0", Some("normal")).await;
    approve_lab_result_core(&pool, &tech_state, tests[0], false)
        .await
        .expect("initial approval");

    // Tech re-enters a corrected value → row becomes 'amended' (mirrors
    // update_lab_result's CASE), which must be re-approved.
    enter_result(&pool, tests[0], "4.4", Some("normal")).await;
    let (status,): (Option<String>,) =
        sqlx::query_as("SELECT approval_status FROM lab_order_tests WHERE id = $1")
            .bind(tests[0])
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status.as_deref(), Some("amended"));

    approve_lab_result_core(&pool, &tech_state, tests[0], false)
        .await
        .expect("amendment re-approval");
    let (status,): (Option<String>,) =
        sqlx::query_as("SELECT approval_status FROM lab_order_tests WHERE id = $1")
            .bind(tests[0])
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status.as_deref(), Some("approved"));
}
