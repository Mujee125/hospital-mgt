//! Phase 9 — In-app notification center integration tests.
//!
//! Exercises the REAL emit/read cores (`emit`, `fetch_feed`,
//! `mark_read_core`, `mark_all_read_core`) plus the two EMITTER-INTEGRATED
//! clinical paths (critical lab value, released result) at command level
//! via `update_lab_result_core` / `approve_lab_result_core` against the
//! real PostgreSQL test DB.
//!
//! Covers the center's four guarantees:
//!   NC-1  Visibility: targeted → that user only; role broadcast → every
//!         holder of the role; broadcast-all → everyone. Others see nothing.
//!   NC-2  Per-user read state: a role broadcast is ONE row; user A marking
//!         it read does NOT clear it for user B.
//!   NC-3  Read marking is visibility-scoped: a user cannot mark an
//!         notification they cannot see (id tampering defense).
//!   NC-4  Emitter integration: entering a CRITICAL lab result and
//!         releasing a result both produce doctor-role notifications.
//!
//! Requires: HMS_TEST_DB_URL env var + `--features hms-integration-tests`.

#![cfg(feature = "hms-integration-tests")]

mod common;

use common::*;
use hospital_mgmt_lib::commands::lab::{
    approve_lab_result_core, collect_lab_sample_core, update_lab_result_core,
};
use hospital_mgmt_lib::commands::notifications::{
    emit, fetch_feed, mark_all_read_core, mark_read_core, NotificationOut,
};
use hospital_mgmt_lib::models::UpdateLabResult;
use hospital_mgmt_lib::rbac::SessionState;
use sqlx::PgPool;
use std::sync::{Arc, Mutex};

async fn state_for(pool: &PgPool, user_id: i32, token_hash: &str) -> SessionState {
    let s = load_session_for(pool, user_id, token_hash).await;
    Arc::new(Mutex::new(Some(s)))
}

fn out(
    role_target: Option<&str>,
    user_id: Option<i32>,
    kind: &str,
    title: &str,
) -> NotificationOut {
    NotificationOut {
        user_id,
        role_target: role_target.map(String::from),
        kind: kind.into(),
        title: title.into(),
        body: format!("body of {}", title),
        entity_type: None,
        entity_id: None,
    }
}

fn roles(r: &str) -> Vec<String> {
    vec![r.to_string()]
}

// ── NC-1 + NC-2: visibility and per-user read state ───────────────────────────

#[tokio::test]
async fn test_nc1_nc2_visibility_and_per_user_reads() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let doc1 = seed_user(&pool, "nc_doc1", &pw, &["doctor"]).await;
    let doc2 = seed_user(&pool, "nc_doc2", &pw, &["doctor"]).await;
    let nurse_id = seed_user(&pool, "nc_nurse", &pw, &["nurse"]).await;
    seed_session_row(&pool, doc1, "hash_nc_doc1").await;
    seed_session_row(&pool, doc2, "hash_nc_doc2").await;
    seed_session_row(&pool, nurse_id, "hash_nc_nurse").await;
    let _doc1_state = state_for(&pool, doc1, "hash_nc_doc1").await;
    let doc1_roles = roles("doctor");
    let doc2_roles = roles("doctor");
    let nurse_roles = roles("nurse");

    // Three targeting modes.
    emit(
        &pool,
        out(Some("doctor"), None, "role_bcast", "Role broadcast"),
    )
    .await
    .unwrap();
    emit(
        &pool,
        out(None, Some(nurse_id), "direct", "Direct to nurse"),
    )
    .await
    .unwrap();
    emit(&pool, out(None, None, "everyone", "Broadcast to all"))
        .await
        .unwrap();

    // Doctor feed: sees the role broadcast + the everyone broadcast, NOT the
    // nurse-direct one. 2 rows, 2 unread.
    let d1 = fetch_feed(&pool, doc1, &doc1_roles).await.unwrap();
    assert_eq!(d1.unread_count, 2, "doctor sees role bcast + everyone only");
    assert!(d1.notifications.iter().any(|n| n.title == "Role broadcast"));
    assert!(d1
        .notifications
        .iter()
        .any(|n| n.title == "Broadcast to all"));
    assert!(!d1
        .notifications
        .iter()
        .any(|n| n.title == "Direct to nurse"));

    // Nurse feed: sees the direct + everyone, NOT the doctor broadcast.
    let nf = fetch_feed(&pool, nurse_id, &nurse_roles).await.unwrap();
    assert_eq!(nf.unread_count, 2);
    assert!(nf
        .notifications
        .iter()
        .any(|n| n.title == "Direct to nurse"));
    assert!(!nf.notifications.iter().any(|n| n.title == "Role broadcast"));

    // NC-2: doc1 marks the role broadcast read — doc2 still sees it unread.
    let bcast = d1
        .notifications
        .iter()
        .find(|n| n.title == "Role broadcast")
        .unwrap();
    mark_read_core(&pool, doc1, &doc1_roles, bcast.id)
        .await
        .unwrap();

    let d1_after = fetch_feed(&pool, doc1, &doc1_roles).await.unwrap();
    assert_eq!(d1_after.unread_count, 1, "doc1 has everyone-broadcast left");
    let d2_after = fetch_feed(&pool, doc2, &doc2_roles).await.unwrap();
    assert_eq!(
        d2_after.unread_count, 2,
        "doc2's read state is independent — still 2 unread"
    );
    let d2_bcast = d2_after
        .notifications
        .iter()
        .find(|n| n.title == "Role broadcast")
        .unwrap();
    assert!(
        !d2_bcast.read,
        "the role broadcast must stay unread for doc2"
    );

    // mark_all for doc2 clears everything visible to them.
    let n = mark_all_read_core(&pool, doc2, &doc2_roles).await.unwrap();
    assert_eq!(n, 2);
    let d2_final = fetch_feed(&pool, doc2, &doc2_roles).await.unwrap();
    assert_eq!(d2_final.unread_count, 0);
}

// ── NC-3: read-marking is visibility-scoped ──────────────────────────────────

#[tokio::test]
async fn test_nc3_cannot_mark_invisible_notification() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let nurse_id = seed_user(&pool, "nc3_nurse", &pw, &["nurse"]).await;
    let pharmacist_id = seed_user(&pool, "nc3_pharm", &pw, &["pharmacist"]).await;

    // Targeted at the nurse only.
    emit(
        &pool,
        out(None, Some(nurse_id), "direct", "Nurse-only secret"),
    )
    .await
    .unwrap();
    let (id,): (i32,) = sqlx::query_as(
        "SELECT id FROM app_notifications WHERE title = 'Nurse-only secret' ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // The pharmacist CANNOT mark it read (not visible to them)…
    let pharm_roles = roles("pharmacist");
    mark_read_core(&pool, pharmacist_id, &pharm_roles, id)
        .await
        .unwrap();
    let (reads,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM app_notification_reads WHERE notification_id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(reads, 0, "an invisible notification must not be markable");

    // …and the pharmacist's feed does not contain it at all.
    let pf = fetch_feed(&pool, pharmacist_id, &pharm_roles)
        .await
        .unwrap();
    assert!(!pf
        .notifications
        .iter()
        .any(|n| n.title == "Nurse-only secret"));

    // The nurse CAN mark it read.
    let nurse_roles = roles("nurse");
    mark_read_core(&pool, nurse_id, &nurse_roles, id)
        .await
        .unwrap();
    let (reads,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM app_notification_reads WHERE notification_id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(reads, 1);
}

// ── NC-4: emitter integration (critical lab value + release) ──────────────────

#[tokio::test]
async fn test_nc4_lab_emitters_fire_notifications() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let tech_id = seed_user(&pool, "nc4_tech", &pw, &["lab_technician"]).await;
    seed_session_row(&pool, tech_id, "hash_nc4_tech").await;
    let tech = state_for(&pool, tech_id, "hash_nc4_tech").await;

    let patient_id = seed_patient_with_phone(&pool, "Lab", "Notif", "+92300nc4a").await;
    let (order_id, tests) = seed_lab_order_rows(&pool, patient_id, 1).await;
    collect_lab_sample_core(&pool, &tech, tests[0].0)
        .await
        .unwrap();

    // Enter a CRITICAL result through the REAL core — the emitter inside
    // update_lab_result_core must fire (not a mirrored SQL statement).
    update_lab_result_core(
        &pool,
        &tech,
        UpdateLabResult {
            id: tests[0].1,
            result_value: Some("19.5".into()),
            result_unit: Some("mg/dL".into()),
            result_abnormal_flag: Some("critical".into()),
            result_notes: Some("recheck advised".into()),
        },
    )
    .await
    .unwrap();

    let (critical_rows,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM app_notifications WHERE kind = 'lab_critical'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        critical_rows >= 1,
        "critical entry must emit a doctor-role notification"
    );
    let (role,): (Option<String>,) = sqlx::query_as(
        "SELECT role_target FROM app_notifications WHERE kind = 'lab_critical' ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        role.as_deref(),
        Some("doctor"),
        "critical notifications target the doctor role"
    );
    let (entity,): (Option<i32>,) = sqlx::query_as(
        "SELECT entity_id FROM app_notifications WHERE kind = 'lab_critical' ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        entity,
        Some(order_id),
        "critical notification links back to the lab order"
    );

    // Release it through the REAL approve core → a lab_released
    // notification must appear for the doctor role.
    approve_lab_result_core(&pool, &tech, tests[0].1, true)
        .await
        .unwrap();
    let (released_rows,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM app_notifications WHERE kind = 'lab_released'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        released_rows >= 1,
        "result release must emit a doctor-role notification"
    );
}

// ── Local lab fixture (independent of lab_tests.rs) ───────────────────────────

/// Insert a lab order with N tests; returns (order_id, Vec<(order_id, row_id)>)
/// so the caller has both the order id (for entity linkage asserts) and each
/// lab_order_tests row id (result entry target).
async fn seed_lab_order_rows(pool: &PgPool, patient_id: i32, n: i32) -> (i32, Vec<(i32, i32)>) {
    let order: (i32,) = sqlx::query_as(
        "INSERT INTO lab_orders (patient_id, status) VALUES ($1, 'ordered') RETURNING id",
    )
    .bind(patient_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let mut tests = Vec::new();
    for i in 0..n {
        let tc: (i32,) = sqlx::query_as::<_, (i32,)>(
            "INSERT INTO lab_test_catalog (name, code, price) \
             VALUES ($1, $1, 0) ON CONFLICT (code) DO UPDATE SET name = EXCLUDED.name RETURNING id",
        )
        .bind(format!("NC4-TC-{}", i))
        .fetch_one(pool)
        .await
        .unwrap();
        let row: (i32,) = sqlx::query_as(
            "INSERT INTO lab_order_tests (lab_order_id, test_catalog_id) VALUES ($1, $2) RETURNING id",
        )
        .bind(order.0)
        .bind(tc.0)
        .fetch_one(pool)
        .await
        .unwrap();
        tests.push((order.0, row.0));
    }
    (order.0, tests)
}
