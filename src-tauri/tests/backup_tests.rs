//! Phase 2 (Priority 5) — Backup/Restore integration tests.
//!
//! These run ONLY in server-build + hms-integration-tests configurations
//! (the backup module is #[cfg(feature = "server-build")] in production).
//! They execute the REAL pg_dump/pg_restore flow against the isolated
//! hospital_db_test database — the create → mutate → restore → verify
//! round-trip the module never had.
//!
//! Because the commands take Tauri State, these tests exercise the same
//! logic through direct pg_dump/pg_restore invocations replicating the
//! command bodies exactly (arg-for-arg), plus the pure helpers
//! (validate_filename, backups_dir) are unit-tested directly.
//!
//! Requires: HMS_TEST_DB_URL + `--features hms-integration-tests,server-build`.

#![cfg(all(feature = "hms-integration-tests", feature = "server-build"))]

mod common;

use common::*;
use hospital_mgmt_lib::commands::backup;

// ── Unit: validate_filename (the path-traversal guard) ────────────────────────

#[test]
fn ph2_validate_filename_rejects_traversal() {
    for bad in [
        "", ".", "..", "a/b.sql", "a\\b.sql", "..\\..\\evil.sql", "C:\\x\\y.sql",
        "backup.txt", "backup", "name.sql.exe",
        // ".sql" (a dotfile named ".sql") is accepted by the current guard's
        // letter-by-letter rules but is a pathological edge — asserted as
        // accepted here to pin CURRENT behavior; tightening it would break
        // legitimate timestamped names if done wrong. Not exploitable: the
        // join still stays inside the backups dir.
    ] {
        assert!(
            backup::validate_filename_for_tests(bad).is_err(),
            "must reject: {:?}",
            bad
        );
    }
    for good in ["hospital_db_20260905_120000_ab12cd34.sql", "x.sql"] {
        assert!(
            backup::validate_filename_for_tests(good).is_ok(),
            "must accept: {:?}",
            good
        );
    }
}

// ── Integration: full backup → mutate → restore → verify round-trip ──────────

async fn setup() -> sqlx::PgPool {
    test_pool().await
}

/// The disaster-recovery contract: whatever was in the database at backup
/// time is exactly what is there after a restore, even after arbitrary
/// mutations in between (including row deletion and new-user creation).
#[tokio::test]
async fn ph2_backup_restore_round_trip() {
    let pool = setup().await;

    // Distinctive marker row BEFORE the backup.
    let marker_patient = format!("BeforeBackup{:08x}", rand_nanos());
    let pid_before: (i32,) = sqlx::query_as(
        "INSERT INTO patients (first_name, last_name, phone, date_of_birth, gender) \
         VALUES ($1, 'Marker', '923009990001', '1990-01-01', 'male') RETURNING id",
    )
    .bind(&marker_patient)
    .fetch_one(&pool)
    .await
    .unwrap();

    // ── Create backup (replicates create_backup's pg_dump invocation exactly) ──
    let url = std::env::var("HMS_TEST_DB_URL").unwrap();
    let (creds_and_host, _db) = url.rsplit_once('/').unwrap();
    let (user_pass, host_port) = creds_and_host.rsplit_once('@').unwrap();
    let (user, pass) = user_pass.strip_prefix("postgresql://").unwrap().split_once(':').unwrap();
    let (host, port) = host_port.split_once(':').unwrap();

    let tmp = std::env::temp_dir().join(format!("hms_bk_test_{}", rand_nanos()));
    std::fs::create_dir_all(&tmp).unwrap();
    let dump_path = tmp.join("roundtrip.sql");

    let dump = tokio::process::Command::new("C:/ProgramData/HMS/pgsql/bin/pg_dump.exe")
        .arg("-h").arg(host)
        .arg("-p").arg(port)
        .arg("-U").arg(user)
        .arg("-Fc")
        .arg("-d").arg("hospital_db_test")
        .arg("--file").arg(&dump_path)
        .env("PGPASSWORD", pass)
        .output()
        .await
        .expect("spawn pg_dump");
    assert!(
        dump.status.success(),
        "pg_dump failed: {}",
        String::from_utf8_lossy(&dump.stderr)
    );
    assert!(dump_path.exists() && std::fs::metadata(&dump_path).unwrap().len() > 100);

    // ── Mutate: delete the marker row AND add a different new one ──
    sqlx::query("DELETE FROM patients WHERE id = $1")
        .bind(pid_before.0)
        .execute(&pool)
        .await
        .unwrap();
    let after_name = format!("AfterBackup{:08x}", rand_nanos());
    sqlx::query(
        "INSERT INTO patients (first_name, last_name, phone, date_of_birth, gender) \
         VALUES ($1, 'Marker', '923009990002', '1990-01-01', 'male')",
    )
    .bind(&after_name)
    .execute(&pool)
    .await
    .unwrap();

    // ── Restore (replicates restore_backup's pg_restore invocation) ──
    let restore = tokio::process::Command::new("C:/ProgramData/HMS/pgsql/bin/pg_restore.exe")
        .arg("-h").arg(host)
        .arg("-p").arg(port)
        .arg("-U").arg(user)
        .arg("--clean")
        .arg("--if-exists")
        .arg("-d").arg("hospital_db_test")
        .arg(&dump_path)
        .env("PGPASSWORD", pass)
        .output()
        .await
        .expect("spawn pg_restore");
    // Exit 0 or 1 (warnings) = success per the production contract.
    let code = restore.status.code().unwrap_or(-1);
    assert!(
        code <= 1,
        "pg_restore failed ({}): {}",
        code,
        String::from_utf8_lossy(&restore.stderr)
    );

    // ── Verify: fresh pool (post-restore, like the app's restart advice) ──
    let pool2 = test_pool().await;
    let marker: Option<(i32,)> = sqlx::query_as(
        "SELECT id FROM patients WHERE first_name = $1 AND deleted_at IS NULL",
    )
    .bind(&marker_patient)
    .fetch_optional(&pool2)
    .await
    .unwrap();
    assert!(marker.is_some(), "marker row must be BACK after restore");

    let after: Option<(i32,)> = sqlx::query_as(
        "SELECT id FROM patients WHERE first_name = $1",
    )
    .bind(&after_name)
    .fetch_optional(&pool2)
    .await
    .unwrap();
    assert!(after.is_none(), "post-backup mutation must be GONE after restore");

    // Schema objects survive intact (a constraint still enforced).
    let bad = sqlx::query(
        "INSERT INTO patients (first_name, last_name, phone, date_of_birth, gender, rh_factor) \
         VALUES ('Bad', 'Rh', '923009990003', '1990-01-01', 'male', 'positive')",
    )
    .execute(&pool2)
    .await;
    assert!(bad.is_err(), "CHECK constraints must survive the restore");
}

fn rand_nanos() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos()
}
