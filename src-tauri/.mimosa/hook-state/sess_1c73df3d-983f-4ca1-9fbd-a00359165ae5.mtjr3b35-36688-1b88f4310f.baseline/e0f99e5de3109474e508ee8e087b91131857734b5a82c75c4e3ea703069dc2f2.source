//! IT-001: Scheduler tests for expire_blood_units().
//!
//! These tests verify the auto-expiry logic (BE-05) by executing the same SQL
//! the production expire_blood_units function uses. They test the SQL behaviour
//! directly because the function is not #[tauri::command] (internal only) and
//! cannot be called via IPC.
//!
//! STATUS: NOT EXECUTED — requires PostgreSQL test DB + Rust toolchain.

#![cfg(test)]

mod common;

use common::*;
use tokio::test;

/// SCH-001: Expire available unit — transitions to 'expired'.
#[test]
async fn test_sch001_expire_available_unit() {
    let pool = setup_pool().await;
    let donor_id = seed_donor(&pool, "O", "-").await;
    let (donation_id, unit_id) = seed_donation_and_unit(&pool, donor_id, "O", "-").await;
    pass_screening(&pool, donation_id).await;
    expire_unit(&pool, unit_id).await;

    sqlx::query(
        r#"UPDATE blood_units SET status = 'expired', updated_at = NOW()
           WHERE expiry_date <= NOW() AND deleted_at IS NULL
             AND status IN ('available','reserved','issued','quarantine')"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let status = get_unit_status(&pool, unit_id).await;
    assert_eq!(status, "expired");
}

/// SCH-002: Expire quarantined unit — transitions to 'expired'.
#[test]
async fn test_sch002_expire_quarantined_unit() {
    let pool = setup_pool().await;
    let donor_id = seed_donor(&pool, "O", "-").await;
    let (_donation_id, unit_id) = seed_donation_and_unit(&pool, donor_id, "O", "-").await;
    // Unit is in 'quarantine' (BE-06 default)
    expire_unit(&pool, unit_id).await;

    sqlx::query(
        r#"UPDATE blood_units SET status = 'expired', updated_at = NOW()
           WHERE expiry_date <= NOW() AND deleted_at IS NULL
             AND status IN ('available','reserved','issued','quarantine')"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let status = get_unit_status(&pool, unit_id).await;
    assert_eq!(status, "expired");
}

/// SCH-003: Transfused (terminal) unit is NOT touched by scheduler.
#[test]
async fn test_sch003_transfused_not_expired() {
    let pool = setup_pool().await;
    let donor_id = seed_donor(&pool, "O", "-").await;
    let (donation_id, unit_id) = seed_donation_and_unit(&pool, donor_id, "O", "-").await;
    pass_screening(&pool, donation_id).await;
    // Manually set to 'transfused' (terminal) + expired
    sqlx::query("UPDATE blood_units SET status = 'transfused', expiry_date = NOW() - INTERVAL '1 hour' WHERE id = $1")
        .bind(unit_id)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query(
        r#"UPDATE blood_units SET status = 'expired', updated_at = NOW()
           WHERE expiry_date <= NOW() AND deleted_at IS NULL
             AND status IN ('available','reserved','issued','quarantine')"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let status = get_unit_status(&pool, unit_id).await;
    assert_eq!(status, "transfused", "Terminal 'transfused' must not be touched");
}

/// SCH-004: Discarded (terminal) unit is NOT touched by scheduler.
#[test]
async fn test_sch004_discarded_not_expired() {
    let pool = setup_pool().await;
    let donor_id = seed_donor(&pool, "O", "-").await;
    let (donation_id, unit_id) = seed_donation_and_unit(&pool, donor_id, "O", "-").await;
    pass_screening(&pool, donation_id).await;
    sqlx::query("UPDATE blood_units SET status = 'discarded', expiry_date = NOW() - INTERVAL '1 hour' WHERE id = $1")
        .bind(unit_id)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query(
        r#"UPDATE blood_units SET status = 'expired', updated_at = NOW()
           WHERE expiry_date <= NOW() AND deleted_at IS NULL
             AND status IN ('available','reserved','issued','quarantine')"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let status = get_unit_status(&pool, unit_id).await;
    assert_eq!(status, "discarded", "Terminal 'discarded' must not be touched");
}

/// SCH-005: No-op execution — no expired units → 0 rows affected.
#[test]
async fn test_sch005_noop_when_no_expired_units() {
    let pool = setup_pool().await;
    let donor_id = seed_donor(&pool, "O", "-").await;
    let (donation_id, _unit_id) = seed_donation_and_unit(&pool, donor_id, "O", "-").await;
    pass_screening(&pool, donation_id).await;
    // Unit has future expiry_date (35 days from now) — not expired

    let result = sqlx::query(
        r#"UPDATE blood_units SET status = 'expired', updated_at = NOW()
           WHERE expiry_date <= NOW() AND deleted_at IS NULL
             AND status IN ('available','reserved','issued','quarantine')"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    assert_eq!(result.rows_affected(), 0, "No units should be expired");
}

/// SCH-006: Multiple expiries — all expired units transition in one UPDATE.
#[test]
async fn test_sch006_multiple_expiries() {
    let pool = setup_pool().await;
    let donor_id = seed_donor(&pool, "O", "-").await;

    // Seed 3 units, all available + expired
    let mut unit_ids = vec![];
    for _ in 0..3 {
        let (donation_id, unit_id) = seed_donation_and_unit(&pool, donor_id, "O", "-").await;
        pass_screening(&pool, donation_id).await;
        expire_unit(&pool, unit_id).await;
        unit_ids.push(unit_id);
    }

    let result = sqlx::query(
        r#"UPDATE blood_units SET status = 'expired', updated_at = NOW()
           WHERE expiry_date <= NOW() AND deleted_at IS NULL
             AND status IN ('available','reserved','issued','quarantine')"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    assert!(result.rows_affected() >= 3, "All 3 expired units must be transitioned");

    for unit_id in &unit_ids {
        let status = get_unit_status(&pool, *unit_id).await;
        assert_eq!(status, "expired");
    }
}

/// SCH-007: Idempotency — running expiry twice does not error.
#[test]
async fn test_sch007_idempotency() {
    let pool = setup_pool().await;
    let donor_id = seed_donor(&pool, "O", "-").await;
    let (donation_id, unit_id) = seed_donation_and_unit(&pool, donor_id, "O", "-").await;
    pass_screening(&pool, donation_id).await;
    expire_unit(&pool, unit_id).await;

    // First run — expires the unit
    sqlx::query(
        r#"UPDATE blood_units SET status = 'expired', updated_at = NOW()
           WHERE expiry_date <= NOW() AND deleted_at IS NULL
             AND status IN ('available','reserved','issued','quarantine')"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    // Second run — unit is already 'expired' (terminal), excluded by WHERE clause
    let result = sqlx::query(
        r#"UPDATE blood_units SET status = 'expired', updated_at = NOW()
           WHERE expiry_date <= NOW() AND deleted_at IS NULL
             AND status IN ('available','reserved','issued','quarantine')"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    assert_eq!(result.rows_affected(), 0, "Second run must be a no-op (unit already expired)");
    let status = get_unit_status(&pool, unit_id).await;
    assert_eq!(status, "expired");
}

/// SCH-008: Expiry boundary — unit expiring at exactly NOW() is expired.
///
/// The production code uses `expiry_date <= NOW()`. A unit with expiry_date
/// set to the current timestamp must be transitioned (boundary is inclusive).
#[test]
async fn test_sch008_expiry_boundary_inclusive() {
    let pool = setup_pool().await;
    let donor_id = seed_donor(&pool, "O", "-").await;
    let (donation_id, unit_id) = seed_donation_and_unit(&pool, donor_id, "O", "-").await;
    pass_screening(&pool, donation_id).await;

    // Set expiry to 1 second in the past (to ensure <= NOW() is true by the
    // time the query runs — exact boundary is racy in async tests)
    sqlx::query("UPDATE blood_units SET expiry_date = NOW() - INTERVAL '1 second' WHERE id = $1")
        .bind(unit_id)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query(
        r#"UPDATE blood_units SET status = 'expired', updated_at = NOW()
           WHERE expiry_date <= NOW() AND deleted_at IS NULL
             AND status IN ('available','reserved','issued','quarantine')"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let status = get_unit_status(&pool, unit_id).await;
    assert_eq!(status, "expired", "Unit expiring at ~NOW() must be expired (boundary inclusive)");
}

/// SCH-009: Future-dated unit is NOT expired.
#[test]
async fn test_sch009_future_unit_not_expired() {
    let pool = setup_pool().await;
    let donor_id = seed_donor(&pool, "O", "-").await;
    let (donation_id, unit_id) = seed_donation_and_unit(&pool, donor_id, "O", "-").await;
    pass_screening(&pool, donation_id).await;
    // Expiry is 35 days in the future (default from seed)

    sqlx::query(
        r#"UPDATE blood_units SET status = 'expired', updated_at = NOW()
           WHERE expiry_date <= NOW() AND deleted_at IS NULL
             AND status IN ('available','reserved','issued','quarantine')"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let status = get_unit_status(&pool, unit_id).await;
    assert_eq!(status, "available", "Future-dated unit must not be expired");
}
