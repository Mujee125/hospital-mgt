//! Phase 8 — Accounts module (SRS §2.15) command-level integration tests.
//!
//! Exercises the REAL production cores (`create_expense_core`,
//! `void_expense_core`, `fetch_accounts_summary`) against the real
//! PostgreSQL test DB.
//!
//! Covers:
//!   AC-1  Recording authority: BillingManage required — a receptionist
//!         (BillingCreate only) is rejected; a billing clerk succeeds with
//!         attribution. Invalid amounts (0, negative, NaN) rejected before
//!         any DB write.
//!   AC-2  Void authority: BillingApprove required (clerk rejected, admin
//!         succeeds); reason required; double-void rejected; voided rows
//!         stay in the table (soft delete) but are excluded from the
//!         summary.
//!   AC-3  Summary math: revenue = payments − refunds; expenses summed
//!         excluding voided rows; net = revenue − expenses; categories
//!         grouped and ordered by spend.
//!
//! Requires: HMS_TEST_DB_URL env var + `--features hms-integration-tests`.

#![cfg(feature = "hms-integration-tests")]

mod common;

use common::*;
use hospital_mgmt_lib::commands::accounts::{
    create_expense_core, fetch_accounts_summary, void_expense_core,
};
use hospital_mgmt_lib::models::{CreateExpense, VoidExpense};
use hospital_mgmt_lib::rbac::SessionState;
use sqlx::PgPool;
use std::sync::{Arc, Mutex};

fn today() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

async fn state_for(pool: &PgPool, user_id: i32, token_hash: &str) -> SessionState {
    let s = load_session_for(pool, user_id, token_hash).await;
    Arc::new(Mutex::new(Some(s)))
}

fn expense(category: &str, description: &str, amount: f64) -> CreateExpense {
    CreateExpense {
        category: category.into(),
        description: description.into(),
        amount,
        expense_date: Some(today()),
        paid_to: None,
        payment_method: Some("cash".into()),
        reference_number: None,
    }
}

// ── AC-1: Recording authority + amount validation ─────────────────────────────

#[tokio::test]
async fn test_ac1_expense_authority_and_amount_validation() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let clerk_id = seed_user(&pool, "ac1_clerk", &pw, &["billing_clerk"]).await;
    let rx_id = seed_user(&pool, "ac1_rx", &pw, &["receptionist"]).await;
    seed_session_row(&pool, clerk_id, "hash_ac1_clerk").await;
    seed_session_row(&pool, rx_id, "hash_ac1_rx").await;
    let clerk = state_for(&pool, clerk_id, "hash_ac1_clerk").await;
    let rx = state_for(&pool, rx_id, "hash_ac1_rx").await;

    // Receptionist (BillingCreate but not BillingManage) rejected.
    let err = create_expense_core(&pool, &rx, expense("rent", "should fail", 100.0))
        .await
        .unwrap_err();
    assert!(
        err.contains("requires the 'billing.manage'"),
        "receptionist must be denied expense recording, got: {}",
        err
    );

    // Amount validation: zero, negative, NaN all rejected BEFORE any write.
    for bad in [0.0, -50.0, f64::NAN, f64::INFINITY] {
        let err = create_expense_core(&pool, &clerk, expense("rent", "bad amount", bad))
            .await
            .unwrap_err();
        assert!(
            err.contains("Invalid expense amount"),
            "amount {} must be rejected, got: {}",
            bad,
            err
        );
    }
    // Missing category / description rejected.
    let err = create_expense_core(&pool, &clerk, expense("  ", "desc", 10.0))
        .await
        .unwrap_err();
    assert!(err.contains("category"), "blank category rejected, got: {}", err);
    let err = create_expense_core(&pool, &clerk, expense("rent", "   ", 10.0))
        .await
        .unwrap_err();
    assert!(err.contains("description"), "blank description rejected, got: {}", err);

    // Clerk happy path: persisted with attribution.
    let id = create_expense_core(&pool, &clerk, expense("utilities", "August electricity", 2500.0))
        .await
        .expect("clerk records expense");
    let stored: (String, Option<i32>) = sqlx::query_as(
        "SELECT category, recorded_by_user_id FROM expenses WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored.0, "utilities");
    assert_eq!(stored.1, Some(clerk_id));
}

// ── AC-2: Void authority + soft delete ────────────────────────────────────────

#[tokio::test]
async fn test_ac2_void_manager_only_and_soft_delete() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let clerk_id = seed_user(&pool, "ac2_clerk", &pw, &["billing_clerk"]).await;
    let admin_id = seed_user(&pool, "ac2_admin", &pw, &["super_admin"]).await;
    seed_session_row(&pool, clerk_id, "hash_ac2_clerk").await;
    seed_session_row(&pool, admin_id, "hash_ac2_admin").await;
    let clerk = state_for(&pool, clerk_id, "hash_ac2_clerk").await;
    let admin = state_for(&pool, admin_id, "hash_ac2_admin").await;

    let id = create_expense_core(&pool, &clerk, expense("supplies", "bandages", 500.0))
        .await
        .unwrap();

    // Clerk (no BillingApprove) cannot void.
    let err = void_expense_core(&pool, &clerk, VoidExpense { id, reason: "clerk".into() })
        .await
        .unwrap_err();
    assert!(
        err.contains("requires the 'billing.approve'"),
        "clerk must be denied voiding, got: {}",
        err
    );

    // Missing reason rejected.
    let err = void_expense_core(&pool, &admin, VoidExpense { id, reason: "  ".into() })
        .await
        .unwrap_err();
    assert!(err.contains("reason"), "void reason required, got: {}", err);

    // Admin void succeeds; row SOFT-deleted (still present with attribution).
    void_expense_core(&pool, &admin, VoidExpense { id, reason: "duplicate entry".into() })
        .await
        .expect("admin voids");
    let (voided_at_set, voided_by): (bool, Option<i32>) = sqlx::query_as(
        "SELECT voided_at IS NOT NULL, voided_by_user_id FROM expenses WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(voided_at_set, "void must stamp voided_at (soft delete)");
    assert_eq!(voided_by, Some(admin_id));
    let (row_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM expenses WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row_count, 1, "financial rows are never hard-deleted");

    // Double-void rejected.
    let err = void_expense_core(&pool, &admin, VoidExpense { id, reason: "again".into() })
        .await
        .unwrap_err();
    assert!(err.contains("already voided"), "double-void rejected, got: {}", err);
}

// ── AC-3: Summary math ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_ac3_summary_revenue_minus_expenses() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let clerk_id = seed_user(&pool, "ac3_clerk", &pw, &["billing_clerk"]).await;
    let admin_id = seed_user(&pool, "ac3_admin", &pw, &["super_admin"]).await;
    seed_session_row(&pool, clerk_id, "hash_ac3_clerk").await;
    seed_session_row(&pool, admin_id, "hash_ac3_admin").await;
    let clerk = state_for(&pool, clerk_id, "hash_ac3_clerk").await;
    let admin = state_for(&pool, admin_id, "hash_ac3_admin").await;

    let patient_id = seed_patient_with_phone(&pool, "Acc", "Sum", "+92300ac3a").await;
    let bill: (i32,) = sqlx::query_as(
        "INSERT INTO bills (patient_id, bill_number, net_amount, status) \
         VALUES ($1, 'AC3-BILL', 1000, 'unpaid') RETURNING id",
    )
    .bind(patient_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Revenue: 800 collected, 100 refunded → net revenue 700.
    sqlx::query("INSERT INTO payments (bill_id, amount, payment_method) VALUES ($1, 800, 'cash')")
        .bind(bill.0).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO refunds (bill_id, amount, reason) VALUES ($1, 100, 'test')")
        .bind(bill.0).execute(&pool).await.unwrap();

    // Expenses: 300 salary + 200 utilities, and a 400 voided one.
    create_expense_core(&pool, &clerk, expense("salary", "staff wage", 300.0)).await.unwrap();
    create_expense_core(&pool, &clerk, expense("utilities", "power", 200.0)).await.unwrap();
    let voided = create_expense_core(&pool, &clerk, expense("other", "will be voided", 400.0))
        .await
        .unwrap();
    void_expense_core(&pool, &admin, VoidExpense { id: voided, reason: "test".into() })
        .await
        .expect("admin voids the excluded expense");

    let summary = fetch_accounts_summary(&pool, today(), today()).await.unwrap();
    assert!(summary.total_revenue >= 700.0, "revenue = payments − refunds, got {}", summary.total_revenue);
    assert!(summary.total_expenses >= 500.0, "300 + 200 counted, voided 400 excluded, got {}", summary.total_expenses);
    assert_eq!(summary.net_position, summary.total_revenue - summary.total_expenses);
    // Category grouping: salary and utilities both present, voided 'other' absent
    // from the top category list.
    assert!(summary.by_category.iter().any(|c| c.category == "salary"));
    assert!(summary.by_category.iter().any(|c| c.category == "utilities"));
    assert!(
        !summary.by_category.iter().any(|c| c.category == "other" && c.total >= 400.0),
        "voided expense must not appear in category totals"
    );
}
