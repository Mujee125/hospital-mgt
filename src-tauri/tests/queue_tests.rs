//! Phase 9 follow-up — queue module integration tests.
//!
//! The queue module shipped in Phase 1 with ZERO test coverage, and that
//! blind spot hid a live bug until it surfaced in the GUI: the token-issue
//! INSERT skipped `$4` (token_number comes from the CTE, not a bind), so the
//! SQL numbered parameters $1..$6 while sqlx bound 5 values — "bind message
//! supplies 5 parameters, but prepared statement requires 6". Issuing a queue
//! token had never once worked. These tests pin the module at command level
//! via the `*_core` extractions (AERP Part G pattern) so the blind spot
//! cannot reopen.
//!
//!   QT-1  Token issue works end-to-end, numbers sequentially from 1 per day,
//!         and is unique per (day, token_number).
//!   QT-2  Race-free numbering: two issues are distinct tokens with distinct
//!         numbers (the EXCLUSIVE table lock serializes the MAX+1 reads).
//!   QT-3  Priority ordering + the atomic call-next state machine: a priority
//!         token is called first, and calling the next token completes the
//!         in-progress one.
//!   QT-4  Scoped call-next (department filter) skips tokens of other scopes.
//!   QT-5  RBAC: QueueManage is required to issue/call (doctor role denied,
//!         nurse role allowed); QueueView sees the feed.
//!
//! Requires: HMS_TEST_DB_URL env var + `--features hms-integration-tests`.

#![cfg(feature = "hms-integration-tests")]

mod common;

use common::*;
use hospital_mgmt_lib::commands::queue::{
    call_next_token_core, create_queue_token_core, get_queue_core, set_token_status_core,
};
use hospital_mgmt_lib::models::CreateQueueToken;
use hospital_mgmt_lib::rbac::SessionState;
use sqlx::PgPool;
use std::sync::{Arc, Mutex};

async fn state_for(pool: &PgPool, user_id: i32, token_hash: &str) -> SessionState {
    let s = load_session_for(pool, user_id, token_hash).await;
    Arc::new(Mutex::new(Some(s)))
}

async fn seed_queue_patient(pool: &PgPool, tag: &str) -> i32 {
    seed_patient_with_phone(pool, "Queue", tag, &format!("+92300qt{}", tag)).await
}

async fn seed_department(pool: &PgPool, tag: &str) -> i32 {
    let (id,): (i32,) =
        sqlx::query_as("INSERT INTO departments (name, code) VALUES ($1, $2) RETURNING id")
            .bind(format!("QT Department {}", tag))
            .bind(tag)
            .fetch_one(pool)
            .await
            .unwrap();
    id
}

async fn token_numbers_today(pool: &PgPool) -> Vec<i32> {
    sqlx::query_as::<_, (i32,)>(
        "SELECT token_number FROM queue_tokens \
         WHERE issued_at::date = CURRENT_DATE ORDER BY token_number",
    )
    .fetch_all(pool)
    .await
    .unwrap()
    .into_iter()
    .map(|r| r.0)
    .collect()
}

// ── QT-1 + QT-2: issue works, numbers sequentially, unique per day ────────────

#[tokio::test]
async fn test_qt1_qt2_issue_sequential_unique() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let nurse_id = seed_user(&pool, "qt_nurse", &pw, &["nurse"]).await;
    seed_session_row(&pool, nurse_id, "hash_qt_nurse").await;
    let nurse = state_for(&pool, nurse_id, "hash_qt_nurse").await;

    let p1 = seed_queue_patient(&pool, "One").await;
    let p2 = seed_queue_patient(&pool, "Two").await;

    // QT-1: this is the exact call that failed in the GUI with "bind message
    // supplies 5 parameters, but prepared statement requires 6".
    let id1 = create_queue_token_core(
        &pool,
        &nurse,
        CreateQueueToken {
            patient_id: p1,
            department_id: None,
            doctor_id: None,
            priority: None,
        },
    )
    .await
    .expect("token issue must not fail with a parameter-count error");

    let (n1,): (i32,) = sqlx::query_as("SELECT token_number FROM queue_tokens WHERE id = $1")
        .bind(id1)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n1, 1, "first token of the day must be number 1");

    // QT-2: a second issue gets a distinct id and the next number.
    let id2 = create_queue_token_core(
        &pool,
        &nurse,
        CreateQueueToken {
            patient_id: p2,
            department_id: None,
            doctor_id: None,
            priority: None,
        },
    )
    .await
    .unwrap();
    assert_ne!(id1, id2);
    let nums = token_numbers_today(&pool).await;
    assert_eq!(
        nums,
        vec![1, 2],
        "tokens must number sequentially from 1 per day"
    );

    // The per-day UNIQUE index must hold.
    let (dups,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM (SELECT token_number FROM queue_tokens \
         WHERE issued_at::date = CURRENT_DATE \
         GROUP BY token_number HAVING COUNT(*) > 1) d",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(dups, 0, "no duplicate (day, token_number) may exist");
}

// ── QT-3: priority ordering + atomic complete-current/call-next ───────────────

#[tokio::test]
async fn test_qt3_priority_and_atomic_call_next() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let nurse_id = seed_user(&pool, "qt3_nurse", &pw, &["nurse"]).await;
    seed_session_row(&pool, nurse_id, "hash_qt3_nurse").await;
    let nurse = state_for(&pool, nurse_id, "hash_qt3_nurse").await;

    let standard = seed_queue_patient(&pool, "Std").await;
    let urgent = seed_queue_patient(&pool, "Urg").await;
    let standard_id = create_queue_token_core(
        &pool,
        &nurse,
        CreateQueueToken {
            patient_id: standard,
            department_id: None,
            doctor_id: None,
            priority: None,
        },
    )
    .await
    .unwrap();
    let urgent_id = create_queue_token_core(
        &pool,
        &nurse,
        CreateQueueToken {
            patient_id: urgent,
            department_id: None,
            doctor_id: None,
            priority: Some(5),
        },
    )
    .await
    .unwrap();

    // Feed order: priority DESC. The urgent token must sort above the standard
    // one — a relative assert, since suites share the per-day DB and other
    // tests' tokens exist too.
    let feed = get_queue_core(&pool, &nurse, None).await.unwrap();
    let urgent_pos = feed.iter().position(|t| t.id == urgent_id).unwrap();
    let standard_pos = feed.iter().position(|t| t.id == standard_id).unwrap();
    assert!(
        urgent_pos < standard_pos,
        "priority > 0 must float ahead in the feed"
    );

    // Call next: the PRIORITY token is claimed first, not the earliest one.
    let next = call_next_token_core(&pool, &nurse, None, None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(next.id, urgent_id);
    let (status,): (String,) = sqlx::query_as("SELECT status FROM queue_tokens WHERE id = $1")
        .bind(urgent_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "in-progress");

    // Calling the next token atomically completes the in-progress one.
    let second = call_next_token_core(&pool, &nurse, None, None)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(second.id, urgent_id);
    let (prev_status, prev_completed): (String, Option<chrono::DateTime<chrono::Utc>>) =
        sqlx::query_as("SELECT status, completed_at FROM queue_tokens WHERE id = $1")
            .bind(urgent_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(prev_status, "completed");
    assert!(prev_completed.is_some(), "completion must be timestamped");

    // Status transitions via the core: completed → waiting resets cleanly.
    set_token_status_core(&pool, &nurse, second.id, "waiting".to_string())
        .await
        .unwrap();
    let (s2,): (String,) = sqlx::query_as("SELECT status FROM queue_tokens WHERE id = $1")
        .bind(second.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(s2, "waiting");
}

// ── QT-4: scoped call-next skips other departments' tokens ────────────────────

#[tokio::test]
async fn test_qt4_scoped_call_next() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let nurse_id = seed_user(&pool, "qt4_nurse", &pw, &["nurse"]).await;
    seed_session_row(&pool, nurse_id, "hash_qt4_nurse").await;
    let nurse = state_for(&pool, nurse_id, "hash_qt4_nurse").await;

    // Two fixture departments (real FKs — ids are SERIAL, never assume 1/2).
    let p_other = seed_queue_patient(&pool, "Other").await;
    let p_mine = seed_queue_patient(&pool, "Mine").await;
    let dep1 = seed_department(&pool, "QT4A").await;
    let dep2 = seed_department(&pool, "QT4B").await;
    let other_id = create_queue_token_core(
        &pool,
        &nurse,
        CreateQueueToken {
            patient_id: p_other,
            department_id: Some(dep1),
            doctor_id: None,
            priority: None,
        },
    )
    .await
    .unwrap();
    let mine_id = create_queue_token_core(
        &pool,
        &nurse,
        CreateQueueToken {
            patient_id: p_mine,
            department_id: Some(dep2),
            doctor_id: None,
            priority: None,
        },
    )
    .await
    .unwrap();

    // Calling department 2's queue must NOT claim department 1's waiting
    // token even though it is older.
    let next = call_next_token_core(&pool, &nurse, Some(dep2), None)
        .await
        .unwrap()
        .expect("department 2 has a waiting token");
    assert_eq!(next.id, mine_id);
    let (other_status,): (String,) =
        sqlx::query_as("SELECT status FROM queue_tokens WHERE id = $1")
            .bind(other_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        other_status, "waiting",
        "a scoped call must not touch other departments"
    );

    // An empty scope returns None instead of falling through to the global queue.
    let empty = call_next_token_core(&pool, &nurse, Some(999), None)
        .await
        .unwrap();
    assert!(
        empty.is_none(),
        "empty scope must return None, not a global token"
    );
}

// ── QT-5: RBAC — QueueManage required; QueueView is read-only ─────────────────

#[tokio::test]
async fn test_qt5_rbac_guards() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let doctor_id = seed_user(&pool, "qt5_doc", &pw, &["doctor"]).await;
    let nurse_id = seed_user(&pool, "qt5_nurse", &pw, &["nurse"]).await;
    seed_session_row(&pool, doctor_id, "hash_qt5_doc").await;
    seed_session_row(&pool, nurse_id, "hash_qt5_nurse").await;
    let doctor = state_for(&pool, doctor_id, "hash_qt5_doc").await;
    let nurse = state_for(&pool, nurse_id, "hash_qt5_nurse").await;

    let p = seed_queue_patient(&pool, "Rbac").await;

    // Doctor (QueueView only) can read the feed…
    get_queue_core(&pool, &doctor, None)
        .await
        .expect("QueueView must allow reading the feed");
    // …but NOT issue or call tokens.
    let err = create_queue_token_core(
        &pool,
        &doctor,
        CreateQueueToken {
            patient_id: p,
            department_id: None,
            doctor_id: None,
            priority: None,
        },
    )
    .await
    .expect_err("doctors must not hold QueueManage");
    assert!(
        err.contains("queue.manage"),
        "expected a queue.manage permission error, got: {}",
        err
    );
    let err = call_next_token_core(&pool, &doctor, None, None)
        .await
        .expect_err("doctors must not hold QueueManage");
    assert!(err.contains("queue.manage"));

    // Nurse (QueueManage) may do both.
    create_queue_token_core(
        &pool,
        &nurse,
        CreateQueueToken {
            patient_id: p,
            department_id: None,
            doctor_id: None,
            priority: None,
        },
    )
    .await
    .expect("nurses hold QueueManage");
    call_next_token_core(&pool, &nurse, None, None)
        .await
        .expect("nurses hold QueueManage");
}
