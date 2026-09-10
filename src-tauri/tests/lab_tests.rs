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
use hospital_mgmt_lib::commands::lab::{
    approve_lab_result_core, collect_lab_sample_core, update_lab_result_core,
};
use hospital_mgmt_lib::models::UpdateLabResult;
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
    enter_result_by(pool, test_row_id, value, flag, None).await;
}

/// RCTF F-06 companion fixture: `enter_result` with completer attribution —
/// mirrors the REAL `update_lab_result` statement, which stamps
/// completed_by_user_id (the bare `enter_result` predates that column and
/// is kept for the legacy call sites).
async fn enter_result_by(
    pool: &PgPool,
    test_row_id: i32,
    value: &str,
    flag: Option<&str>,
    completed_by: Option<i32>,
) {
    sqlx::query(
        "UPDATE lab_order_tests SET \
              result_value = $1, result_abnormal_flag = $2, completed_at = NOW(), \
              completed_by_user_id = $4, \
              approval_status = CASE WHEN approval_status = 'approved' THEN 'amended' ELSE 'entered' END \
         WHERE id = $3",
    )
    .bind(value)
    .bind(flag)
    .bind(test_row_id)
    .bind(completed_by)
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
/// RCTF F-06: results are entered by the tech and approved by a SECOND
/// reviewer (doctor) — the tech role no longer holds lab.approve.
#[tokio::test]
async fn test_lw2_full_workflow_resulted_then_approved() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let tech_id = seed_user(&pool, "lw2_tech", &pw, &["lab_technician"]).await;
    seed_session_row(&pool, tech_id, "hash_lw2_tech").await;
    let tech_state = state_for(&pool, tech_id, "hash_lw2_tech").await;
    let doc_id = seed_user(&pool, "lw2_doc", &pw, &["doctor"]).await;
    seed_session_row(&pool, doc_id, "hash_lw2_doc").await;
    let doc_state = state_for(&pool, doc_id, "hash_lw2_doc").await;

    let patient_id = seed_patient_with_phone(&pool, "Lab", "Flow", "+92300lw2a").await;
    let (order_id, tests) = seed_lab_order(&pool, patient_id, 2).await;
    collect_lab_sample_core(&pool, &tech_state, order_id)
        .await
        .expect("collect");

    // Enter both results (mirrors the command's statements — tech-completed).
    enter_result_by(&pool, tests[0], "98", Some("normal"), Some(tech_id)).await;
    enter_result_by(&pool, tests[1], "5.2", Some("high"), Some(tech_id)).await;

    let (status,): (String,) = sqlx::query_as("SELECT status FROM lab_orders WHERE id = $1")
        .bind(order_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        status, "resulted",
        "fully-entered order must await approval, never auto-release"
    );

    // RCTF F-06: the tech who entered the results can no longer approve —
    // the seeded role lost lab.approve AND self-approval is refused.
    let err = approve_lab_result_core(&pool, &tech_state, tests[0], false)
        .await
        .unwrap_err();
    assert!(
        err.contains("requires the 'lab.approve'"),
        "tech must be denied approval after the F-06 seed change, got: {}",
        err
    );

    // Approve the first test only (by the doctor) — order must REMAIN 'resulted'.
    approve_lab_result_core(&pool, &doc_state, tests[0], false)
        .await
        .expect("doctor (second reviewer) approves");
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
    approve_lab_result_core(&pool, &doc_state, tests[1], false)
        .await
        .expect("approve second test");
    let (status, approver): (String, Option<i32>) =
        sqlx::query_as("SELECT status, approved_by_user_id FROM lab_orders WHERE id = $1")
            .bind(order_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "approved");
    assert_eq!(approver, Some(doc_id));

    // Already-approved rows cannot be re-approved.
    let err = approve_lab_result_core(&pool, &doc_state, tests[0], false)
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
    // RCTF F-06: the reviewer is a doctor, not the tech who entered the value.
    let doc_id = seed_user(&pool, "lw4_doc", &pw, &["doctor"]).await;
    seed_session_row(&pool, doc_id, "hash_lw4_doc").await;
    let doc_state = state_for(&pool, doc_id, "hash_lw4_doc").await;

    let patient_id = seed_patient_with_phone(&pool, "Lab", "Crit", "+92300lw4a").await;
    let (_order_id, tests) = seed_lab_order(&pool, patient_id, 1).await;
    enter_result_by(&pool, tests[0], "19.5", Some("critical"), Some(tech_id)).await;

    // Without acknowledgment: rejected by the command guard.
    let err = approve_lab_result_core(&pool, &doc_state, tests[0], false)
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
    approve_lab_result_core(&pool, &doc_state, tests[0], true)
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
        Some(doc_id),
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
    // RCTF F-06: doctor is the reviewer.
    let doc_id = seed_user(&pool, "lw4b_doc", &pw, &["doctor"]).await;
    seed_session_row(&pool, doc_id, "hash_lw4b_doc").await;
    let doc_state = state_for(&pool, doc_id, "hash_lw4b_doc").await;

    let patient_id = seed_patient_with_phone(&pool, "Lab", "Amd", "+92300lw4b").await;
    let (_order_id, tests) = seed_lab_order(&pool, patient_id, 1).await;
    enter_result_by(&pool, tests[0], "4.0", Some("normal"), Some(tech_id)).await;
    approve_lab_result_core(&pool, &doc_state, tests[0], false)
        .await
        .expect("initial approval");

    // Tech re-enters a corrected value → row becomes 'amended' (mirrors
    // update_lab_result's CASE), which must be re-approved — and because the
    // tech is now the last completer, even an approver-role holder that IS
    // the tech is refused (F-06 self-approval guard; the tech is refused at
    // the permission check first).
    enter_result_by(&pool, tests[0], "4.4", Some("normal"), Some(tech_id)).await;
    let (status,): (Option<String>,) =
        sqlx::query_as("SELECT approval_status FROM lab_order_tests WHERE id = $1")
            .bind(tests[0])
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status.as_deref(), Some("amended"));

    approve_lab_result_core(&pool, &doc_state, tests[0], false)
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

// ── RCTF-FULL-SYSTEM-2026-09-09 F-06: lab self-approval separation ────────────

/// RCTF-F02-3 — a lab order whose encounter belongs to a DIFFERENT patient
/// is refused; the patient's own encounter still links fine.
#[tokio::test]
async fn rctf_f02_lab_order_encounter_patient_mismatch_refused() {
    use hospital_mgmt_lib::commands::lab::create_lab_order_core;
    use hospital_mgmt_lib::models::CreateLabOrder;

    let pool = test_pool().await;
    let pw = fixture_pw();
    let doc_id = seed_user(&pool, "f02_lab_doc", &pw, &["doctor"]).await;
    seed_session_row(&pool, doc_id, "hash_f02_lab_doc").await;
    let doc_state = state_for(&pool, doc_id, "hash_f02_lab_doc").await;

    let patient_a = seed_patient_with_phone(&pool, "Lab", "One", "+92300f02l1").await;
    let patient_b = seed_patient_with_phone(&pool, "Lab", "Two", "+92300f02l2").await;
    let enc_b: (i32,) = sqlx::query_as(
        "INSERT INTO encounters (patient_id, visit_type) VALUES ($1, 'opd') RETURNING id",
    )
    .bind(patient_b)
    .fetch_one(&pool)
    .await
    .unwrap();

    let catalog: (i32,) = sqlx::query_as(
        "INSERT INTO lab_test_catalog (name, code, price) VALUES ('F02 Test', 'F02-TC', 0) \
         ON CONFLICT (code) DO UPDATE SET name = EXCLUDED.name RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Encounter of patient B on patient A's order → refused.
    let err = create_lab_order_core(
        &pool,
        &doc_state,
        CreateLabOrder {
            patient_id: patient_a,
            encounter_id: Some(enc_b.0),
            ordered_by_doctor_id: None,
            test_catalog_ids: vec![catalog.0],
        },
    )
    .await
    .unwrap_err();
    assert!(
        err.contains("belongs to a different patient"),
        "mismatched encounter must be refused, got: {}",
        err
    );

    // Patient A's own encounter → order created and correctly linked.
    let enc_a: (i32,) = sqlx::query_as(
        "INSERT INTO encounters (patient_id, visit_type) VALUES ($1, 'opd') RETURNING id",
    )
    .bind(patient_a)
    .fetch_one(&pool)
    .await
    .unwrap();
    let order_id = create_lab_order_core(
        &pool,
        &doc_state,
        CreateLabOrder {
            patient_id: patient_a,
            encounter_id: Some(enc_a.0),
            ordered_by_doctor_id: None,
            test_catalog_ids: vec![catalog.0],
        },
    )
    .await
    .expect("own-patient encounter is valid");
    let (enc, pid): (Option<i32>, i32) =
        sqlx::query_as("SELECT encounter_id, patient_id FROM lab_orders WHERE id = $1")
            .bind(order_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(enc, Some(enc_a.0));
    assert_eq!(pid, patient_a);
}

/// RCTF-FULL-SYSTEM-2026-09-09 F-06: the seeded `lab_technician` role must NOT hold lab.approve
/// (it holds lab.result.manage; the combination voided the documented
/// separation of duties). A deployment that wants the tech to release
/// results must grant it explicitly in Users → Roles.
#[tokio::test]
async fn rctf_f06_tech_role_lacks_lab_approve() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let tech_id = seed_user(&pool, "f06_tech", &pw, &["lab_technician"]).await;
    seed_session_row(&pool, tech_id, "hash_f06_tech").await;
    let session = load_session_for(&pool, tech_id, "hash_f06_tech").await;

    assert!(
        session.permissions.contains("lab.result.manage"),
        "tech keeps result entry"
    );
    assert!(
        !session.permissions.contains("lab.approve"),
        "tech must NOT hold lab.approve after the F-06 seed change (got: {:?})",
        session.permissions
    );
}

/// RCTF-F06-2 — even a legit LabApprove holder cannot approve a result they
/// entered themselves: the completer check refuses self-approval regardless
/// of role grants (defense-in-depth for doctor-entered results, and for any
/// deployment that re-grants lab.approve to the tech).
#[tokio::test]
async fn rctf_f06_self_approval_refused_even_with_permission() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    // A super_admin user enters the result AND tries to approve it —
    // super_admin holds every permission, so only the F-06 completer check
    // can stop the self-approval.
    let admin_id = seed_user(&pool, "f06_admin", &pw, &["super_admin"]).await;
    seed_session_row(&pool, admin_id, "hash_f06_admin").await;
    let admin_state = state_for(&pool, admin_id, "hash_f06_admin").await;

    let patient_id = seed_patient_with_phone(&pool, "Lab", "Self", "+92300f06a").await;
    let (_order_id, tests) = seed_lab_order(&pool, patient_id, 1).await;
    enter_result_by(&pool, tests[0], "6.7", Some("normal"), Some(admin_id)).await;

    let err = approve_lab_result_core(&pool, &admin_state, tests[0], false)
        .await
        .unwrap_err();
    assert!(
        err.contains("cannot approve it yourself"),
        "self-approval must be refused with the SoD message, got: {}",
        err
    );
    // The row must remain unreleased.
    let (status,): (Option<String>,) =
        sqlx::query_as("SELECT approval_status FROM lab_order_tests WHERE id = $1")
            .bind(tests[0])
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status.as_deref(), Some("entered"));
}

// ── RCTF-FULL-SYSTEM-2026-09-08 F-18: result-before-sample + amendment ────────

/// A result cannot be entered for an order still in 'ordered' status —
/// there is no specimen. Previously the entry succeeded and even flipped
/// the order to 'resulted', skipping collection entirely.
#[tokio::test]
async fn rctf_f18_result_before_sample_refused() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let tech_id = seed_user(&pool, "f18_tech", &pw, &["lab_technician"]).await;
    seed_session_row(&pool, tech_id, "hash_f18_tech").await;
    let tech_state = state_for(&pool, tech_id, "hash_f18_tech").await;

    let patient_id = seed_patient_with_phone(&pool, "Foh", "Nosample", "+92300f18a").await;
    let (order_id, tests) = seed_lab_order(&pool, patient_id, 1).await;
    // Order is still 'ordered' — no sample collected.

    let err = update_lab_result_core(
        &pool,
        &tech_state,
        UpdateLabResult {
            id: tests[0],
            result_value: Some("7.2".into()),
            result_unit: Some("mg/dL".into()),
            result_abnormal_flag: Some("normal".into()),
            result_notes: None,
        },
    )
    .await
    .unwrap_err();
    assert!(
        err.contains("sample has not been collected"),
        "result-before-sample must be refused, got: {}",
        err
    );
    // Neither the result row nor the order status changed.
    let (value, order_status): (Option<String>, String) = sqlx::query_as(
        "SELECT lot.result_value, lo.status FROM lab_order_tests lot \
         JOIN lab_orders lo ON lo.id = lot.lab_order_id WHERE lot.id = $1",
    )
    .bind(tests[0])
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(value.is_none(), "no result may be written without a sample");
    assert_eq!(order_status, "ordered", "the order stays 'ordered'");

    // After collection, the same entry succeeds.
    collect_lab_sample_core(&pool, &tech_state, order_id)
        .await
        .expect("collection must succeed");
    update_lab_result_core(
        &pool,
        &tech_state,
        UpdateLabResult {
            id: tests[0],
            result_value: Some("7.2".into()),
            result_unit: Some("mg/dL".into()),
            result_abnormal_flag: Some("normal".into()),
            result_notes: None,
        },
    )
    .await
    .expect("entry after collection must succeed");
}

/// An amendment (overwriting an APPROVED result) must preserve the
/// ORIGINAL completer's identity — the previous code overwrote
/// completed_by_user_id with the amender, losing the tech who first
/// resulted the test from the row itself.
#[tokio::test]
async fn rctf_f18_amendment_preserves_original_completer() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let tech_id = seed_user(&pool, "f18b_tech", &pw, &["lab_technician"]).await;
    seed_session_row(&pool, tech_id, "hash_f18b_tech").await;
    let tech_state = state_for(&pool, tech_id, "hash_f18b_tech").await;
    let doc_id = seed_user(&pool, "f18b_doc", &pw, &["doctor"]).await;
    seed_session_row(&pool, doc_id, "hash_f18b_doc").await;
    let doc_state = state_for(&pool, doc_id, "hash_f18b_doc").await;

    let patient_id = seed_patient_with_phone(&pool, "Foi", "Amend", "+92300f18b").await;
    let (order_id, tests) = seed_lab_order(&pool, patient_id, 1).await;
    collect_lab_sample_core(&pool, &tech_state, order_id)
        .await
        .expect("collect");
    // The ORIGINAL tech enters and the doctor approves → released.
    update_lab_result_core(
        &pool,
        &tech_state,
        UpdateLabResult {
            id: tests[0],
            result_value: Some("6.5".into()),
            result_unit: Some("mg/dL".into()),
            result_abnormal_flag: Some("normal".into()),
            result_notes: None,
        },
    )
    .await
    .expect("original entry");
    approve_lab_result_core(&pool, &doc_state, tests[0], false)
        .await
        .expect("approval");

    // The same tech amends the released value (a correction).
    update_lab_result_core(
        &pool,
        &tech_state,
        UpdateLabResult {
            id: tests[0],
            result_value: Some("6.9".into()),
            result_unit: Some("mg/dL".into()),
            result_abnormal_flag: Some("normal".into()),
            result_notes: Some("corrected value".into()),
        },
    )
    .await
    .expect("amendment entry");

    let (value, status, completer): (String, Option<String>, Option<i32>) = sqlx::query_as(
        "SELECT result_value, approval_status, completed_by_user_id FROM lab_order_tests WHERE id = $1",
    )
    .bind(tests[0])
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(value, "6.9", "the amended value is stored");
    assert_eq!(
        status.as_deref(),
        Some("amended"),
        "an amendment demotes to 'amended' (re-approval required)"
    );
    assert_eq!(
        completer,
        Some(tech_id),
        "the ORIGINAL completer's identity must survive the amendment — \
         the previous code overwrote it with the amender's id"
    );
}
