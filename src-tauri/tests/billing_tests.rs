//! Phase 6.3 — Billing workflow command-level integration tests.
//!
//! Exercises the REAL production cores (`create_bill_core`,
//! `record_refund_core`, `record_advance_core`, `apply_advance_core`,
//! `cancel_bill_core`, `create_insurance_claim_core`,
//! `update_insurance_claim_status_core`) against the real PostgreSQL test
//! DB. `record_payment` takes tauri::State (pre-6.3 signature, unchanged);
//! its cancelled-bill guard is asserted via the same statement the command
//! runs.
//!
//! Covers the six SRS §2.10 guarantees:
//!   BL-1  Invoice numbers are sequential (INV-YYYY-NNNNNN from a SEQUENCE —
//!         no COUNT race), strictly increasing, unique.
//!   BL-2  Refunds: manager-only (BillingApprove); never exceed the money
//!         actually paid; payments rows are never deleted (append-only).
//!   BL-3  Cancelled bills: credit note is issued; payments closed.
//!   BL-4  Discounts > 5% of gross require BillingApprove — a plain clerk
//!         (BillingCreate only) is rejected.
//!   BL-5  Advances: apply creates a payment row, decrements remaining,
//!         cannot over-draft, cannot cross patients.
//!   BL-6  Insurance claims: state machine (draft→submitted→settled) with
//!         illegal transitions rejected; UNIQUE per (bill, insurer).
//!
//! Requires: HMS_TEST_DB_URL env var + `--features hms-integration-tests`.

#![cfg(feature = "hms-integration-tests")]

mod common;

use common::*;
use hospital_mgmt_lib::commands::billing::{
    apply_advance_core, cancel_bill_core, create_bill_core, create_insurance_claim_core,
    record_advance_core, record_refund_core, update_insurance_claim_status_core,
};
use hospital_mgmt_lib::models::{
    BillItemInput, CreateAdvance, CreateBill, CreateInsuranceClaim, CreateRefund, UpdateClaimStatus,
};
use hospital_mgmt_lib::rbac::SessionState;
use sqlx::PgPool;
use std::sync::{Arc, Mutex};

// ── Local helpers ─────────────────────────────────────────────────────────────

async fn state_for(pool: &PgPool, user_id: i32, token_hash: &str) -> SessionState {
    let s = load_session_for(pool, user_id, token_hash).await;
    Arc::new(Mutex::new(Some(s)))
}

fn bill_for(patient_id: i32, qty: f64, price: f64, discount: f64) -> CreateBill {
    CreateBill {
        patient_id,
        encounter_id: None,
        ipd_admission_id: None,
        bill_type: Some("opd".into()),
        discount: Some(discount),
        tax: Some(0.0),
        items: vec![BillItemInput {
            item_type: "consultation".into(),
            description: "Test item".into(),
            quantity: qty,
            unit_price: price,
            reference_id: None,
        }],
    }
}

/// record_payment's guarded insert (mirrors the command body for the
/// State-bound surface): rejects cancelled bills before inserting.
async fn pay_bill(pool: &PgPool, bill_id: i32, amount: f64) -> Result<(), String> {
    let db_err = |e: sqlx::Error| format!("db: {}", e);
    let mut tx = pool.begin().await.map_err(db_err)?;
    let live: Option<(i32,)> =
        sqlx::query_as("SELECT id FROM bills WHERE id = $1 AND status <> 'cancelled' FOR UPDATE")
            .bind(bill_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_err)?;
    if live.is_none() {
        return Err("This bill is cancelled — payments are closed on it.".into());
    }
    sqlx::query(
        "INSERT INTO payments (bill_id, amount, payment_method, received_by_user_id) \
         VALUES ($1,$2,'cash',NULL)",
    )
    .bind(bill_id)
    .bind(amount)
    .execute(&mut *tx)
    .await
    .map_err(db_err)?;
    sqlx::query(
        "UPDATE bills SET status = CASE \
              WHEN (SELECT COALESCE(SUM(amount),0) FROM payments WHERE bill_id = $1) \
                   - (SELECT COALESCE(SUM(amount),0) FROM refunds WHERE bill_id = $1) >= net_amount \
              THEN 'paid' ELSE 'partial' END WHERE id = $1",
    )
    .bind(bill_id)
    .execute(&mut *tx)
    .await
    .map_err(db_err)?;
    tx.commit().await.map_err(|e| format!("db: {}", e))?;
    Ok(())
}

// ── BL-1: Sequential invoice numbers ─────────────────────────────────────────

/// Two bills created back-to-back receive strictly increasing INV-YYYY-NNNNNN
/// numbers from the SEQUENCE (the COUNT(*)+1 race is structurally gone —
/// NEXTVAL is atomic under concurrency).
#[tokio::test]
async fn test_bl1_invoice_numbers_sequential_from_sequence() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let clerk_id = seed_user(&pool, "bl1_clerk", &pw, &["billing_clerk"]).await;
    seed_session_row(&pool, clerk_id, "hash_bl1_clerk").await;
    let clerk = state_for(&pool, clerk_id, "hash_bl1_clerk").await;

    let patient_id = seed_patient_with_phone(&pool, "Bil", "Seq", "+92300bl1a").await;
    let b1 = create_bill_core(&pool, &clerk, bill_for(patient_id, 1.0, 100.0, 0.0), false)
        .await
        .expect("first bill");
    let b2 = create_bill_core(&pool, &clerk, bill_for(patient_id, 1.0, 100.0, 0.0), false)
        .await
        .expect("second bill");

    let (n1,): (String,) = sqlx::query_as("SELECT bill_number FROM bills WHERE id = $1")
        .bind(b1)
        .fetch_one(&pool)
        .await
        .unwrap();
    let (n2,): (String,) = sqlx::query_as("SELECT bill_number FROM bills WHERE id = $1")
        .bind(b2)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        n1.starts_with("INV-") && n2.starts_with("INV-"),
        "numbers must be INV- prefixed, got {} / {}",
        n1,
        n2
    );
    fn tail(n: &str) -> &str {
        n.rsplit('-').next().unwrap_or("")
    }
    assert_eq!(
        tail(&n1).len(),
        6,
        "number must be zero-padded to 6: {}",
        n1
    );
    assert!(
        tail(&n2) > tail(&n1),
        "sequence must be strictly increasing: {} vs {}",
        n1,
        n2
    );
}

// ── BL-2: Refunds ─────────────────────────────────────────────────────────────

/// Refund authority is manager-only: a billing clerk (PaymentsManage,
/// BillingManage but NOT BillingApprove) is rejected; the super admin
/// path succeeds; over-refund is blocked; payment rows are never deleted.
#[tokio::test]
async fn test_bl2_refund_manager_only_and_overrefund_guard() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let clerk_id = seed_user(&pool, "bl2_clerk", &pw, &["billing_clerk"]).await;
    let admin_id = seed_user(&pool, "bl2_admin", &pw, &["super_admin"]).await;
    seed_session_row(&pool, clerk_id, "hash_bl2_clerk").await;
    seed_session_row(&pool, admin_id, "hash_bl2_admin").await;
    let clerk = state_for(&pool, clerk_id, "hash_bl2_clerk").await;
    let admin = state_for(&pool, admin_id, "hash_bl2_admin").await;

    let patient_id = seed_patient_with_phone(&pool, "Bil", "Refund", "+92300bl2a").await;
    let bill_id = create_bill_core(&pool, &clerk, bill_for(patient_id, 1.0, 100.0, 0.0), false)
        .await
        .unwrap();
    pay_bill(&pool, bill_id, 100.0).await.unwrap();

    // Clerk (no BillingApprove) cannot refund.
    let err = record_refund_core(
        &pool,
        &clerk,
        CreateRefund {
            bill_id,
            amount: 10.0,
            reason: "clerk attempt".into(),
            payment_id: None,
        },
    )
    .await
    .unwrap_err();
    assert!(
        err.contains("requires the 'billing.approve'"),
        "clerk must be denied refunds, got: {}",
        err
    );

    // Missing reason rejected.
    let err = record_refund_core(
        &pool,
        &admin,
        CreateRefund {
            bill_id,
            amount: 10.0,
            reason: "  ".into(),
            payment_id: None,
        },
    )
    .await
    .unwrap_err();
    assert!(err.contains("reason"), "reason required, got: {}", err);

    // Over-refund blocked (paid 100 → refund of 150 rejected).
    let err = record_refund_core(
        &pool,
        &admin,
        CreateRefund {
            bill_id,
            amount: 150.0,
            reason: "too much".into(),
            payment_id: None,
        },
    )
    .await
    .unwrap_err();
    assert!(
        err.contains("exceeds the amount actually paid"),
        "over-refund blocked, got: {}",
        err
    );

    // Valid refund by admin: row appended, payment row intact, status re-rolled.
    record_refund_core(
        &pool,
        &admin,
        CreateRefund {
            bill_id,
            amount: 40.0,
            reason: "partial service refund".into(),
            payment_id: None,
        },
    )
    .await
    .expect("admin refund");

    let (status, payments, refunds): (String, i64, i64) = sqlx::query_as(
        "SELECT b.status, \
                (SELECT COUNT(*) FROM payments WHERE bill_id = b.id), \
                (SELECT COUNT(*) FROM refunds WHERE bill_id = b.id) \
         FROM bills b WHERE b.id = $1",
    )
    .bind(bill_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(payments, 1, "payment row must never be deleted");
    assert_eq!(refunds, 1, "refund appended");
    assert_eq!(
        status, "partial",
        "40 of 100 refunded → bill back to partial"
    );
}

// ── BL-3: Credit-note cancellation ───────────────────────────────────────────

/// Cancelling issues a CN- number, closes the bill to payments, and is
/// idempotent-rejected on repeat.
#[tokio::test]
async fn test_bl3_cancel_issues_credit_note_and_closes_payments() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let admin_id = seed_user(&pool, "bl3_admin", &pw, &["super_admin"]).await;
    seed_session_row(&pool, admin_id, "hash_bl3_admin").await;
    let admin = state_for(&pool, admin_id, "hash_bl3_admin").await;

    let patient_id = seed_patient_with_phone(&pool, "Bil", "Cancel", "+92300bl3a").await;
    let bill_id = create_bill_core(&pool, &admin, bill_for(patient_id, 2.0, 50.0, 0.0), false)
        .await
        .unwrap();

    // Missing reason rejected.
    let err = cancel_bill_core(&pool, &admin, bill_id, "  ".into())
        .await
        .unwrap_err();
    assert!(
        err.contains("reason"),
        "cancel reason required, got: {}",
        err
    );

    let credit_note = cancel_bill_core(&pool, &admin, bill_id, "billed in error".into())
        .await
        .expect("cancel");
    assert!(
        credit_note.starts_with("CN-"),
        "credit note number, got: {}",
        credit_note
    );

    // Payments on the cancelled bill are closed (guarded insert).
    let err = pay_bill(&pool, bill_id, 10.0).await.unwrap_err();
    assert!(
        err.contains("cancelled"),
        "payment on cancelled bill blocked, got: {}",
        err
    );

    // Repeat cancellation rejected.
    let err = cancel_bill_core(&pool, &admin, bill_id, "again".into())
        .await
        .unwrap_err();
    assert!(
        err.contains("already cancelled"),
        "double-cancel rejected, got: {}",
        err
    );

    let (status, cn): (String, Option<String>) =
        sqlx::query_as("SELECT status, credit_note_number FROM bills WHERE id = $1")
            .bind(bill_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "cancelled");
    assert_eq!(cn.as_deref(), Some(credit_note.as_str()));
}

// ── BL-4: Two-level discount approval ─────────────────────────────────────────

/// A clerk cannot create a bill discounting more than 5% of gross without
/// BillingApprove; the admin can (with the explicit approve flag). Small
/// discounts pass for the clerk.
#[tokio::test]
async fn test_bl4_large_discount_requires_manager_approval() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let clerk_id = seed_user(&pool, "bl4_clerk", &pw, &["billing_clerk"]).await;
    let admin_id = seed_user(&pool, "bl4_admin", &pw, &["super_admin"]).await;
    seed_session_row(&pool, clerk_id, "hash_bl4_clerk").await;
    seed_session_row(&pool, admin_id, "hash_bl4_admin").await;
    let clerk = state_for(&pool, clerk_id, "hash_bl4_clerk").await;
    let admin = state_for(&pool, admin_id, "hash_bl4_admin").await;

    let patient_id = seed_patient_with_phone(&pool, "Bil", "Disc", "+92300bl4a").await;

    // Clerk, small discount (4%): fine.
    create_bill_core(
        &pool,
        &clerk,
        bill_for(patient_id, 1.0, 1000.0, 40.0),
        false,
    )
    .await
    .expect("clerk small discount passes");

    // Clerk, big discount (10%): rejected.
    let err = create_bill_core(
        &pool,
        &clerk,
        bill_for(patient_id, 1.0, 1000.0, 100.0),
        false,
    )
    .await
    .unwrap_err();
    assert!(
        err.contains("manager approval"),
        "clerk big discount must be rejected, got: {}",
        err
    );

    // Admin, big discount without the confirm flag: rejected (belt+braces).
    let err = create_bill_core(
        &pool,
        &admin,
        bill_for(patient_id, 1.0, 1000.0, 100.0),
        false,
    )
    .await
    .unwrap_err();
    assert!(
        err.contains("not confirmed"),
        "confirm flag required, got: {}",
        err
    );

    // Admin + confirm: passes and the discount is stored.
    let id = create_bill_core(
        &pool,
        &admin,
        bill_for(patient_id, 1.0, 1000.0, 100.0),
        true,
    )
    .await
    .expect("admin approves big discount");
    let (discount,): (rust_decimal::Decimal,) =
        sqlx::query_as("SELECT discount FROM bills WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(discount.normalize().to_string(), "100");
}

// ── BL-5: Advances ────────────────────────────────────────────────────────────

/// Advance lifecycle: received → applied to a bill (payment row created,
/// remaining decremented) → over-draft and cross-patient blocked.
#[tokio::test]
async fn test_bl5_advance_apply_overdraft_and_crosspatient_guards() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let clerk_id = seed_user(&pool, "bl5_clerk", &pw, &["billing_clerk"]).await;
    seed_session_row(&pool, clerk_id, "hash_bl5_clerk").await;
    let clerk = state_for(&pool, clerk_id, "hash_bl5_clerk").await;

    let p1 = seed_patient_with_phone(&pool, "Bil", "Adv1", "+92300bl5a").await;
    let p2 = seed_patient_with_phone(&pool, "Bil", "Adv2", "+92300bl5b").await;

    let adv_id = record_advance_core(
        &pool,
        &clerk,
        CreateAdvance {
            patient_id: p1,
            amount: 500.0,
            method: Some("cash".into()),
            reference_number: None,
            notes: Some("IPD deposit".into()),
        },
    )
    .await
    .expect("advance received");

    let bill1 = create_bill_core(&pool, &clerk, bill_for(p1, 1.0, 800.0, 0.0), false)
        .await
        .unwrap();
    let bill2 = create_bill_core(&pool, &clerk, bill_for(p2, 1.0, 100.0, 0.0), false)
        .await
        .unwrap();

    // Cross-patient: p1's advance on p2's bill is rejected.
    let err = apply_advance_core(&pool, &clerk, adv_id, bill2, 50.0)
        .await
        .unwrap_err();
    assert!(
        err.contains("does not belong to this patient"),
        "cross-patient apply blocked, got: {}",
        err
    );

    // Over-draft: 600 > 500 remaining.
    let err = apply_advance_core(&pool, &clerk, adv_id, bill1, 600.0)
        .await
        .unwrap_err();
    assert!(
        err.contains("exceeds the advance balance"),
        "overdraft blocked, got: {}",
        err
    );

    // Valid apply: payment row created, remaining decremented, status still active.
    apply_advance_core(&pool, &clerk, adv_id, bill1, 300.0)
        .await
        .expect("apply 300");
    let (remaining, status, payment_rows): (rust_decimal::Decimal, String, i64) = sqlx::query_as(
        "SELECT a.remaining, a.status, \
                (SELECT COUNT(*) FROM payments WHERE bill_id = $1 AND payment_method = 'advance') \
         FROM patient_advances a WHERE a.id = $2",
    )
    .bind(bill1)
    .bind(adv_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(remaining.normalize().to_string(), "200");
    assert_eq!(status, "active");
    assert_eq!(payment_rows, 1, "advance apply must create a payment row");

    // Exhaust the advance → status flips to 'applied'.
    apply_advance_core(&pool, &clerk, adv_id, bill1, 200.0)
        .await
        .expect("apply remaining 200");
    let (remaining, status): (rust_decimal::Decimal, String) =
        sqlx::query_as("SELECT remaining, status FROM patient_advances WHERE id = $1")
            .bind(adv_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(remaining.normalize().to_string(), "0");
    assert_eq!(status, "applied", "fully applied advance flips status");

    // Applying an exhausted advance is rejected.
    let err = apply_advance_core(&pool, &clerk, adv_id, bill1, 10.0)
        .await
        .unwrap_err();
    assert!(
        err.contains("applied — only active"),
        "exhausted advance blocked, got: {}",
        err
    );
}

// ── BL-6: Insurance claims ────────────────────────────────────────────────────

/// Claim state machine: legal transitions succeed, illegal jumps are
/// rejected, one claim per (bill, insurer), cancelled bills excluded.
#[tokio::test]
async fn test_bl6_claim_state_machine_and_uniqueness() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let clerk_id = seed_user(&pool, "bl6_clerk", &pw, &["billing_clerk"]).await;
    let admin_id = seed_user(&pool, "bl6_admin", &pw, &["super_admin"]).await;
    seed_session_row(&pool, clerk_id, "hash_bl6_clerk").await;
    seed_session_row(&pool, admin_id, "hash_bl6_admin").await;
    let clerk = state_for(&pool, clerk_id, "hash_bl6_clerk").await;
    let admin = state_for(&pool, admin_id, "hash_bl6_admin").await;

    let patient_id = seed_patient_with_phone(&pool, "Bil", "Claim", "+92300bl6a").await;
    let bill_id = create_bill_core(&pool, &clerk, bill_for(patient_id, 1.0, 5000.0, 0.0), false)
        .await
        .unwrap();

    let claim_id = create_insurance_claim_core(
        &pool,
        &clerk,
        CreateInsuranceClaim {
            bill_id,
            insurer: "Jubilee TPA".into(),
            policy_number: Some("POL-77".into()),
            claim_amount: 4000.0,
            notes: None,
        },
    )
    .await
    .expect("claim created");

    // UNIQUE (bill_id, insurer): same insurer again maps to the same claim row
    // (upsert), not a duplicate.
    let claim_id2 = create_insurance_claim_core(
        &pool,
        &clerk,
        CreateInsuranceClaim {
            bill_id,
            insurer: "Jubilee TPA".into(),
            policy_number: None,
            claim_amount: 4000.0,
            notes: None,
        },
    )
    .await
    .expect("upsert same insurer");
    assert_eq!(
        claim_id, claim_id2,
        "same (bill, insurer) must reuse the claim row"
    );

    // Illegal transition: draft → settled.
    let err = update_insurance_claim_status_core(
        &pool,
        &clerk,
        UpdateClaimStatus {
            id: claim_id,
            status: "settled".into(),
            approved_amount: None,
            notes: None,
        },
    )
    .await
    .unwrap_err();
    assert!(
        err.contains("Invalid claim transition"),
        "illegal jump rejected, got: {}",
        err
    );

    // draft → submitted → partially_approved (amount required) → settled.
    update_insurance_claim_status_core(
        &pool,
        &clerk,
        UpdateClaimStatus {
            id: claim_id,
            status: "submitted".into(),
            approved_amount: None,
            notes: None,
        },
    )
    .await
    .expect("submit");

    let err = update_insurance_claim_status_core(
        &pool,
        &clerk,
        UpdateClaimStatus {
            id: claim_id,
            status: "partially_approved".into(),
            approved_amount: None,
            notes: None,
        },
    )
    .await
    .unwrap_err();
    assert!(
        err.contains("approved amount is required"),
        "partial approval needs amount, got: {}",
        err
    );

    update_insurance_claim_status_core(
        &pool,
        &clerk,
        UpdateClaimStatus {
            id: claim_id,
            status: "partially_approved".into(),
            approved_amount: Some(3500.0),
            notes: None,
        },
    )
    .await
    .expect("partial approval");
    update_insurance_claim_status_core(
        &pool,
        &clerk,
        UpdateClaimStatus {
            id: claim_id,
            status: "settled".into(),
            approved_amount: None,
            notes: None,
        },
    )
    .await
    .expect("settle");

    let (status, approved, settled_at_is_set): (String, Option<rust_decimal::Decimal>, bool) = sqlx::query_as(
        "SELECT status, approved_amount, settled_at IS NOT NULL FROM insurance_claims WHERE id = $1",
    )
    .bind(claim_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "settled");
    assert_eq!(
        approved.map(|a| a.normalize().to_string()),
        Some("3500".into())
    );
    assert!(settled_at_is_set);

    // Claims on cancelled bills are rejected.
    let cancelled_bill =
        create_bill_core(&pool, &clerk, bill_for(patient_id, 1.0, 300.0, 0.0), false)
            .await
            .unwrap();
    // Cancellation needs the manager.
    cancel_bill_core(&pool, &admin, cancelled_bill, "test".into())
        .await
        .unwrap();
    let err = create_insurance_claim_core(
        &pool,
        &clerk,
        CreateInsuranceClaim {
            bill_id: cancelled_bill,
            insurer: "State Life".into(),
            policy_number: None,
            claim_amount: 100.0,
            notes: None,
        },
    )
    .await
    .unwrap_err();
    assert!(
        err.contains("cancelled"),
        "claim on cancelled bill rejected, got: {}",
        err
    );
}

// ── RCTF-FULL-SYSTEM-2026-09-09 F-02/F-07: linkage + payment guards ──────────

/// Seed an encounter row for a patient (mirrors create_encounter's INSERT
/// shape without the RBAC layer).
async fn seed_encounter(pool: &PgPool, patient_id: i32) -> i32 {
    let row: (i32,) = sqlx::query_as(
        "INSERT INTO encounters (patient_id, visit_type, chief_complaint) \
         VALUES ($1, 'opd', 'RCTF fixture') RETURNING id",
    )
    .bind(patient_id)
    .fetch_one(pool)
    .await
    .expect("seed encounter");
    row.0
}

/// RCTF-F02-1 — a bill whose encounter belongs to a DIFFERENT patient is
/// rejected; the linkage to the correct patient's encounter still works.
#[tokio::test]
async fn rctf_f02_bill_encounter_patient_mismatch_refused() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let clerk_id = seed_user(&pool, "f02_clerk", &pw, &["billing_clerk"]).await;
    seed_session_row(&pool, clerk_id, "hash_f02_clerk").await;
    let clerk = state_for(&pool, clerk_id, "hash_f02_clerk").await;

    let patient_a = seed_patient_with_phone(&pool, "Ann", "Right", "+92300f02a").await;
    let patient_b = seed_patient_with_phone(&pool, "Bob", "Wrong", "+92300f02b").await;
    let enc_b = seed_encounter(&pool, patient_b).await;

    // Encounter of patient B attached to patient A's bill → refused.
    let mut bad = bill_for(patient_a, 1.0, 100.0, 0.0);
    bad.encounter_id = Some(enc_b);
    let err = create_bill_core(&pool, &clerk, bad, false)
        .await
        .unwrap_err();
    assert!(
        err.contains("belongs to a different patient"),
        "mismatched encounter must be refused, got: {}",
        err
    );

    // Nonexistent encounter → refused with the not-exist message.
    let mut ghost = bill_for(patient_a, 1.0, 100.0, 0.0);
    ghost.encounter_id = Some(9_999_999);
    let err = create_bill_core(&pool, &clerk, ghost, false)
        .await
        .unwrap_err();
    assert!(
        err.contains("does not exist"),
        "ghost encounter must be refused, got: {}",
        err
    );

    // Own encounter → bill created (no rows leaked from the refusals above).
    let enc_a = seed_encounter(&pool, patient_a).await;
    let mut good = bill_for(patient_a, 1.0, 100.0, 0.0);
    good.encounter_id = Some(enc_a);
    let bill_id = create_bill_core(&pool, &clerk, good, false)
        .await
        .expect("own-patient encounter is valid");
    let (enc, pid): (Option<i32>, i32) =
        sqlx::query_as("SELECT encounter_id, patient_id FROM bills WHERE id = $1")
            .bind(bill_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(enc, Some(enc_a));
    assert_eq!(pid, patient_a);
}

/// RCTF-F07-1 — duplicate payment reference on the same bill is refused
/// (the double-click / IPC-retry double charge), and the overpay guard
/// caps a payment at the outstanding balance. A full happy path (pay to
/// exactly the balance, then a second payment of any size is refused).
#[tokio::test]
async fn rctf_f07_payment_idempotency_and_overpay_guard() {
    use hospital_mgmt_lib::commands::billing::record_payment_core;
    use hospital_mgmt_lib::models::CreatePayment;

    let pool = test_pool().await;
    let pw = fixture_pw();
    let clerk_id = seed_user(&pool, "f07_clerk", &pw, &["billing_clerk"]).await;
    seed_session_row(&pool, clerk_id, "hash_f07_clerk").await;
    let clerk = state_for(&pool, clerk_id, "hash_f07_clerk").await;

    let patient_id = seed_patient_with_phone(&pool, "Pay", "Guard", "+92300f07a").await;
    let bill_id = create_bill_core(&pool, &clerk, bill_for(patient_id, 1.0, 100.0, 0.0), false)
        .await
        .expect("bill");

    // First payment with a reference: succeeds.
    record_payment_core(
        &pool,
        &clerk,
        CreatePayment {
            bill_id,
            amount: 40.0,
            payment_method: Some("card".into()),
            reference_number: Some("RCTF-F07-REF".into()),
        },
    )
    .await
    .expect("first payment with reference");

    // Same reference again (double-click shape): refused.
    let err = record_payment_core(
        &pool,
        &clerk,
        CreatePayment {
            bill_id,
            amount: 40.0,
            payment_method: Some("card".into()),
            reference_number: Some("RCTF-F07-REF".into()),
        },
    )
    .await
    .unwrap_err();
    assert!(
        err.contains("already posted"),
        "duplicate reference must be refused, got: {}",
        err
    );
    // And the DB backstop: only one payment row exists for the bill so far.
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM payments WHERE bill_id = $1")
        .bind(bill_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 1, "no duplicate row may be created");

    // Overpay: 40 paid of 100 → a 61 payment must be refused.
    let err = record_payment_core(
        &pool,
        &clerk,
        CreatePayment {
            bill_id,
            amount: 61.0,
            payment_method: Some("cash".into()),
            reference_number: None,
        },
    )
    .await
    .unwrap_err();
    assert!(
        err.contains("exceeds the outstanding balance"),
        "overpay must be refused, got: {}",
        err
    );

    // Exact outstanding (60) is fine — pays the bill in full.
    record_payment_core(
        &pool,
        &clerk,
        CreatePayment {
            bill_id,
            amount: 60.0,
            payment_method: Some("cash".into()),
            reference_number: None,
        },
    )
    .await
    .expect("exact balance payment");

    // After full payment, ANY further payment exceeds the (now zero)
    // outstanding balance — the bill can no longer absorb money.
    let err = record_payment_core(
        &pool,
        &clerk,
        CreatePayment {
            bill_id,
            amount: 1.0,
            payment_method: Some("cash".into()),
            reference_number: Some("RCTF-F07-AFTER".into()),
        },
    )
    .await
    .unwrap_err();
    assert!(
        err.contains("exceeds the outstanding balance"),
        "paid-in-full bill must refuse more money, got: {}",
        err
    );
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM payments WHERE bill_id = $1")
        .bind(bill_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 2, "exactly the two legitimate payments exist");
}
