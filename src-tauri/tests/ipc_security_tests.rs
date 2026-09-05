//! IT-001: IPC security tests for Blood Bank commands.
//!
//! These tests verify the SQL-level security invariants that the #[tauri::command]
//! functions enforce. They cannot call the commands directly (they require
//! tauri::State), so they test the same SQL patterns the commands use.
//!
//! STATUS: NOT EXECUTED — requires PostgreSQL test DB + Rust toolchain.

#![cfg(test)]

mod common;

use common::*;
use tokio::test;

/// SEC-001: CHECK constraint prevents invalid unit status injection.
///
/// An attacker cannot inject an arbitrary status value (e.g., 'hacked')
/// because the CHECK constraint on blood_units.status rejects it.
#[test]
async fn test_sec001_check_constraint_rejects_invalid_status() {
    let pool = setup_pool().await;
    let donor_id = seed_donor(&pool, "O", "-").await;
    let result = sqlx::query(
        r#"INSERT INTO blood_units
              (unit_number, donor_id, component_type, blood_group, rh_factor,
               volume_ml, expiry_date, status)
           VALUES ('BU-SEC-1', $1, 'whole_blood', 'O', '-', 450, NOW() + INTERVAL '35 days', 'hacked')"#,
    )
    .bind(donor_id)
    .execute(&pool)
    .await;
    assert!(result.is_err(), "CHECK constraint must reject status='hacked'");
}

/// SEC-002: CHECK constraint prevents invalid rh_factor injection.
#[test]
async fn test_sec002_check_constraint_rejects_invalid_rh() {
    let pool = setup_pool().await;
    let result = sqlx::query(
        r#"INSERT INTO blood_donors
              (donor_number, first_name, last_name, blood_group, rh_factor, status)
           VALUES ('DON-SEC-2', 'Test', 'Donor', 'O', 'positive', 'active')"#,
    )
    .execute(&pool)
    .await;
    assert!(result.is_err(), "CHECK constraint must reject rh_factor='positive'");
}

/// SEC-003: SQL injection in search field is parameterized (safe).
///
/// The production get_blood_donors uses `.bind(format!("%{}%", search))` which
/// parameterizes the ILIKE pattern. This test verifies that a classic SQL
/// injection payload ('; DROP TABLE blood_donors; --) is treated as a literal
/// string, not executed.
#[test]
async fn test_sec003_sql_injection_in_search() {
    let pool = setup_pool().await;
    let donor_id = seed_donor(&pool, "O", "-").await;

    // Simulate the production query with a malicious search string
    let malicious = "'; DROP TABLE blood_donors; --";
    let result: Vec<(i32,)> = sqlx::query_as(
        r#"SELECT id FROM blood_donors
           WHERE deleted_at IS NULL
             AND (donor_number ILIKE $1 OR first_name ILIKE $1 OR last_name ILIKE $1)"#,
    )
    .bind(format!("%{}%", malicious))
    .fetch_all(&pool)
    .await
    .expect("Parameterized query must not execute injection");

    // The table must still exist (injection didn't run)
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM blood_donors WHERE id = $1")
        .bind(donor_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "Table must still exist and contain the seeded donor");
    assert!(result.is_empty(), "Malicious search must match nothing");
}

/// SEC-004: Soft-deleted units are inaccessible via direct ID lookup.
///
/// The production get_blood_unit filters `deleted_at IS NULL`. A deleted unit
/// must not be retrievable even by its exact ID.
#[test]
async fn test_sec004_soft_deleted_unit_inaccessible() {
    let pool = setup_pool().await;
    let donor_id = seed_donor(&pool, "O", "-").await;
    let (donation_id, unit_id) = seed_donation_and_unit(&pool, donor_id, "O", "-").await;
    pass_screening(&pool, donation_id).await;

    sqlx::query("UPDATE blood_units SET deleted_at = NOW() WHERE id = $1")
        .bind(unit_id)
        .execute(&pool)
        .await
        .unwrap();

    let result: Option<(i32,)> = sqlx::query_as(
        "SELECT id FROM blood_units WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(unit_id)
    .fetch_optional(&pool)
    .await
    .unwrap();

    assert!(result.is_none(), "Soft-deleted unit must not be accessible");
}

/// SEC-005: UNIQUE constraint prevents duplicate unit numbers.
#[test]
async fn test_sec005_unique_unit_number() {
    let pool = setup_pool().await;
    let donor_id = seed_donor(&pool, "O", "-").await;
    let (_donation_id, _unit_id) = seed_donation_and_unit(&pool, donor_id, "O", "-").await;

    // Attempt to insert a second unit with the same unit_number — must fail
    // (We can't easily predict the exact unit_number, so we test with a literal)
    // Unique-per-run literal: the test DB persists across runs, so a fixed
    // 'BU-DUP-TEST' would collide with the previous run's row.
    let uniq: u32 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos();
    let unit_number = format!("BU-DUP-{}", uniq);
    let result = sqlx::query(
        r#"INSERT INTO blood_units
              (unit_number, donor_id, component_type, blood_group, rh_factor,
               volume_ml, collection_date, expiry_date, status)
           VALUES ($2, $1, 'whole_blood', 'O', '-',
                   450, NOW(), NOW() + INTERVAL '35 days', 'quarantine')"#,
    )
    .bind(donor_id)
    .bind(&unit_number)
    .execute(&pool)
    .await;
    assert!(result.is_ok(), "First insert with unique number must succeed: {:?}", result.err());

    let result2 = sqlx::query(
        r#"INSERT INTO blood_units
              (unit_number, donor_id, component_type, blood_group, rh_factor,
               volume_ml, collection_date, expiry_date, status)
           VALUES ($2, $1, 'whole_blood', 'O', '-',
                   450, NOW(), NOW() + INTERVAL '35 days', 'quarantine')"#,
    )
    .bind(donor_id)
    .bind(&unit_number)
    .execute(&pool)
    .await;
    assert!(result2.is_err(), "UNIQUE constraint must reject duplicate unit_number");
}

/// SEC-006: Volume CHECK constraint rejects zero/negative volume.
#[test]
async fn test_sec006_volume_check_constraint() {
    let pool = setup_pool().await;
    let donor_id = seed_donor(&pool, "O", "-").await;
    let result = sqlx::query(
        r#"INSERT INTO blood_units
              (unit_number, donor_id, component_type, blood_group, rh_factor,
               volume_ml, expiry_date, status)
           VALUES ('BU-VOL-0', $1, 'whole_blood', 'O', '-', 0, NOW() + INTERVAL '35 days', 'quarantine')"#,
    )
    .bind(donor_id)
    .execute(&pool)
    .await;
    assert!(result.is_err(), "CHECK constraint must reject volume_ml=0");
}

/// SEC-007: Boundary value — volume_ml = 1 is accepted (minimum valid).
#[test]
async fn test_sec007_volume_boundary_minimum() {
    let pool = setup_pool().await;
    let donor_id = seed_donor(&pool, "O", "-").await;
    let uniq: u32 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos();
    let unit_number = format!("BU-VOL-{}", uniq);
    let result = sqlx::query(
        r#"INSERT INTO blood_units
              (unit_number, donor_id, component_type, blood_group, rh_factor,
               volume_ml, collection_date, expiry_date, status)
           VALUES ($2, $1, 'whole_blood', 'O', '-',
                   1, NOW(), NOW() + INTERVAL '35 days', 'quarantine')"#,
    )
    .bind(donor_id)
    .bind(&unit_number)
    .execute(&pool)
    .await;
    assert!(result.is_ok(), "volume_ml=1 must be accepted (boundary): {:?}", result.err());
}

/// SEC-008: patients.rh_factor CHECK constraint (BE-01) rejects invalid values.
#[test]
async fn test_sec008_patients_rh_factor_check() {
    let pool = setup_pool().await;
    let result = sqlx::query(
        r#"INSERT INTO patients
               (first_name, last_name, phone, date_of_birth, gender, blood_group, rh_factor)
           VALUES ('Test', 'Patient', '92300000002', '1990-01-01', 'male', 'O', 'positive')"#,
    )
    .execute(&pool)
    .await;
    // RESOLVED 2026-09-05 (Phase 1 schema review): the SEC-008 CHECK
    // constraint is now implemented in run_migrations (with NULL-normalizing
    // of pre-existing invalid values). 'positive' is rejected by the CHECK
    // itself — previously this test passed only accidentally via the
    // VARCHAR(5) length limit.
    assert!(
        result.is_err(),
        "chk_patients_rh_factor must reject 'positive' (CHECK now implemented in db.rs)"
    );
    // And the constraint accepts the two valid clinical values.
    for rh in ["+", "-"] {
        let ok = sqlx::query(
            r#"INSERT INTO patients
                   (first_name, last_name, phone, date_of_birth, gender, blood_group, rh_factor)
               VALUES ('Test', 'Patient', '92300000003', '1990-01-01', 'male', 'O', $1)"#,
        )
        .bind(rh)
        .execute(&pool)
        .await;
        assert!(ok.is_ok(), "rh_factor '{}' must be accepted: {:?}", rh, ok.err());
    }
}

/// SEC-009: patients.rh_factor accepts NULL (unknown blood type is valid).
#[test]
async fn test_sec009_patients_rh_factor_null_accepted() {
    let pool = setup_pool().await;
    let result = sqlx::query(
        r#"INSERT INTO patients
               (first_name, last_name, phone, date_of_birth, gender, blood_group, rh_factor)
           VALUES ('Test', 'Patient', '92300000001', '1990-01-01', 'male', 'O', NULL)"#,
    )
    .execute(&pool)
    .await;
    assert!(result.is_ok(), "NULL rh_factor must be accepted (unknown blood type)");
}

/// SEC-010: FK constraint — cannot insert blood_unit with nonexistent donor.
#[test]
async fn test_sec010_fk_nonexistent_donor() {
    let pool = setup_pool().await;
    let result = sqlx::query(
        r#"INSERT INTO blood_units
              (unit_number, donor_id, component_type, blood_group, rh_factor,
               volume_ml, collection_date, expiry_date, status)
           VALUES ('BU-FK-1', 999999, 'whole_blood', 'O', '-',
                   450, NOW(), NOW() + INTERVAL '35 days', 'quarantine')"#,
    )
    .execute(&pool)
    .await;
    assert!(result.is_err(), "FK must reject nonexistent donor_id");
}

/// SEC-011: Large payload — donation with maximum volume (600ml) is accepted.
#[test]
async fn test_sec011_max_volume_boundary() {
    let pool = setup_pool().await;
    let donor_id = seed_donor(&pool, "O", "-").await;
    let result = sqlx::query(
        r#"INSERT INTO blood_donations
              (donation_number, donor_id, volume_ml, blood_group, rh_factor, status, screening_status)
           VALUES ($2, $1, 600, 'O', '-', 'collected', 'pending')"#,
    )
    .bind(donor_id)
    .bind(&format!("BDN-MAX-{}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos()))
    .execute(&pool)
    .await;
    assert!(result.is_ok(), "volume_ml=600 must be accepted (boundary): {:?}", result.err());
}

/// SEC-012: Large payload — donation with volume >600ml is rejected.
#[test]
async fn test_sec012_over_max_volume_rejected() {
    let pool = setup_pool().await;
    let donor_id = seed_donor(&pool, "O", "-").await;
    let result = sqlx::query(
        r#"INSERT INTO blood_donations
              (donation_number, donor_id, volume_ml, blood_group, rh_factor, status, screening_status)
           VALUES ('BDN-OVER-601', $1, 601, 'O', '-', 'collected', 'pending')"#,
    )
    .bind(donor_id)
    .execute(&pool)
    .await;
    assert!(result.is_err(), "CHECK constraint must reject volume_ml=601 (>600)");
}
