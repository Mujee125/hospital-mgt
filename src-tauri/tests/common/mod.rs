//! IT-001: Shared test infrastructure for Blood Bank integration tests.
//!
//! This module provides:
//!   - Database connection helper (connects to DATABASE_URL env var)
//!   - Deterministic seed data fixtures
//!   - Audit verification helpers
//!   - History/movement verification helpers
//!
//! Design principle: each test cleans up after itself using unique identifiers
//! (random suffixes). No test touches a production database — the test DB is
//! separate (configured via docker-compose.test.yml).
//!
//! These tests are NOT #[tauri::command] IPC tests — they test the SQL logic
//! directly by executing the same queries the commands use, against a real
//! PostgreSQL instance. This proves the transactions, constraints, and
//! concurrency behaviour without requiring a Tauri runtime.

use sqlx::PgPool;

/// Connect to the test database. The DATABASE_URL env var must point to a
/// PostgreSQL instance that has had migrations run against it (see
/// docker-compose.test.yml + init script).
pub async fn setup_pool() -> PgPool {
    let url = std::env::var("DATABASE_URL").expect(
        "DATABASE_URL must be set for integration tests. \
         Use docker-compose.test.yml to provision a test DB.",
    );
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .expect("Failed to connect to test database")
}

/// Run migrations on the test database. Called once before the test suite.
pub async fn run_migrations(pool: &PgPool) {
    // The test harness calls the production migration runner directly.
    // This is safe because the test DB is isolated.
    hospital_mgmt::db::run_migrations(pool)
        .await
        .expect("Failed to run migrations on test DB");
}

/// Seed a donor and return its id. Used by most tests as a precondition.
pub async fn seed_donor(pool: &PgPool, blood_group: &str, rh_factor: &str) -> i32 {
    let donor_number = format!("DON-TEST-{}", rand_u32());
    let row: (i32,) = sqlx::query_as(
        r#"INSERT INTO blood_donors
              (donor_number, first_name, last_name, blood_group, rh_factor, status)
           VALUES ($1, 'Test', 'Donor', $2, $3, 'active') RETURNING id"#,
    )
    .bind(&donor_number)
    .bind(blood_group)
    .bind(rh_factor)
    .fetch_one(pool)
    .await
    .expect("Failed to seed donor");
    row.0
}

/// Seed a patient with blood_group + rh_factor (BE-01 column). Returns id.
pub async fn seed_patient(pool: &PgPool, blood_group: &str, rh_factor: &str) -> i32 {
    let row: (i32,) = sqlx::query_as(
        r#"INSERT INTO patients (first_name, last_name, blood_group, rh_factor)
           VALUES ('Test', 'Patient', $1, $2) RETURNING id"#,
    )
    .bind(blood_group)
    .bind(rh_factor)
    .fetch_one(pool)
    .await
    .expect("Failed to seed patient");
    row.0
}

/// Seed a donation + linked blood unit. By default the unit is in 'quarantine'
/// (BE-06) with screening_status='pending'. Returns (donation_id, unit_id).
pub async fn seed_donation_and_unit(
    pool: &PgPool,
    donor_id: i32,
    blood_group: &str,
    rh_factor: &str,
) -> (i32, i32) {
    let mut tx = pool.begin().await.expect("Failed to begin tx");

    let donation_number = format!("BDN-TEST-{}", rand_u32());
    let donation_row: (i32,) = sqlx::query_as(
        r#"INSERT INTO blood_donations
              (donation_number, donor_id, volume_ml, blood_group, rh_factor,
               status, screening_status)
           VALUES ($1, $2, 450, $3, $4, 'collected', 'pending') RETURNING id"#,
    )
    .bind(&donation_number)
    .bind(donor_id)
    .bind(blood_group)
    .bind(rh_factor)
    .fetch_one(&mut *tx)
    .await
    .expect("Failed to seed donation");
    let donation_id = donation_row.0;

    let unit_number = format!("BU-TEST-{}", rand_u32());
    let unit_row: (i32,) = sqlx::query_as(
        r#"INSERT INTO blood_units
              (unit_number, donation_id, donor_id, component_type, blood_group, rh_factor,
               volume_ml, collection_date, expiry_date, status)
           VALUES ($1, $2, $3, 'whole_blood', $4, $5, 450, NOW(),
                   NOW() + INTERVAL '35 days', 'quarantine') RETURNING id"#,
    )
    .bind(&unit_number)
    .bind(donation_id)
    .bind(donor_id)
    .bind(blood_group)
    .bind(rh_factor)
    .fetch_one(&mut *tx)
    .await
    .expect("Failed to seed unit");
    let unit_id = unit_row.0;

    tx.commit().await.expect("Failed to commit seed");
    (donation_id, unit_id)
}

/// Mark a donation's screening as 'passed' and transition the linked unit to
/// 'available' (mirrors update_blood_donation_screening logic).
pub async fn pass_screening(pool: &PgPool, donation_id: i32) {
    sqlx::query(
        "UPDATE blood_donations SET screening_status = 'passed', status = 'screened' WHERE id = $1",
    )
    .bind(donation_id)
    .execute(pool)
    .await
    .expect("Failed to update screening");
    sqlx::query("UPDATE blood_units SET status = 'available' WHERE donation_id = $1")
        .bind(donation_id)
        .execute(pool)
        .await
        .expect("Failed to release unit");
}

/// Set a unit's expiry to the past (for BE-02 expiry tests).
pub async fn expire_unit(pool: &PgPool, unit_id: i32) {
    sqlx::query("UPDATE blood_units SET expiry_date = NOW() - INTERVAL '1 hour' WHERE id = $1")
        .bind(unit_id)
        .execute(pool)
        .await
        .expect("Failed to expire unit");
}

/// Count audit_log entries for a given action + resource_id.
pub async fn count_audit_entries(pool: &PgPool, action: &str, resource_id: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM audit_logs WHERE action = $1 AND resource_id = $2")
        .bind(action)
        .bind(resource_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0)
}

/// Count status_history entries for a unit.
pub async fn count_history_entries(pool: &PgPool, unit_id: i32) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM blood_unit_status_history WHERE unit_id = $1")
        .bind(unit_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0)
}

/// Count inventory_movement entries for a unit.
pub async fn count_movement_entries(pool: &PgPool, unit_id: i32) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM blood_inventory_movements WHERE unit_id = $1")
        .bind(unit_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0)
}

/// Get the current status of a unit.
pub async fn get_unit_status(pool: &PgPool, unit_id: i32) -> String {
    let row: (String,) = sqlx::query_as("SELECT status FROM blood_units WHERE id = $1")
        .bind(unit_id)
        .fetch_one(pool)
        .await
        .expect("Unit not found");
    row.0
}

/// Simple random u32 for unique identifiers. Uses std random — sufficient for
/// test data, not for production.
fn rand_u32() -> u32 {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    now.wrapping_add(unsafe { std::arch::x86_64::_rdtsc() as u32 })
}
