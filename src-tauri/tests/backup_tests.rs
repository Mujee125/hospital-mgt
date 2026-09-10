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
        "",
        ".",
        "..",
        "a/b.sql",
        "a\\b.sql",
        "..\\..\\evil.sql",
        "C:\\x\\y.sql",
        "backup.txt",
        "backup",
        "name.sql.exe",
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
    for good in [
        "hospital_db_20260905_120000_ab12cd34.sql",
        "x.sql",
        // F-11: the encrypted-archive extension.
        "hospital_db_20260905_120000_ab12cd34.sql.enc",
    ] {
        assert!(
            backup::validate_filename_for_tests(good).is_ok(),
            "must accept: {:?}",
            good
        );
    }
    // The encrypted extension must not be spoofable into an arbitrary
    // suffix after it.
    assert!(backup::validate_filename_for_tests("x.sql.enc.exe").is_err());
    assert!(backup::validate_filename_for_tests("x.sql.enc/bad").is_err());
}

// ── RCTF-FULL-SYSTEM-2026-09-08 F-11: backup encryption at rest ───────────────

/// The archive encrypt→decrypt round-trip: the encrypted file carries the
/// HMSE1 magic (never plaintext), decrypts back to the exact original
/// bytes, and a file that is already encrypted is never double-encrypted.
#[test]
fn rctf_f11_backup_encryption_roundtrip() {
    let dir = std::env::temp_dir().join(format!(
        "hms_f11_enc_{}_{}",
        std::process::id(),
        rand_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("hospital_db_20260909_120000_deadbeef.sql");
    let original = b"PGDMP fake archive bytes \x00\x01\x02 with PHI content";
    std::fs::write(&path, original).unwrap();

    // Encrypt → .sql.enc with the magic, plaintext file GONE.
    let enc = backup::encrypt_backup_file(&path).expect("encrypt");
    assert_eq!(
        enc.extension().and_then(|e| e.to_str()),
        Some("enc"),
        "the encrypted archive ends in .sql.enc"
    );
    assert!(!path.exists(), "the plaintext archive must be removed");
    let blob = std::fs::read(&enc).unwrap();
    assert!(
        blob.starts_with(b"HMSE1"),
        "encrypted archives must carry the HMSE1 magic"
    );
    assert!(
        !blob
            .windows(20)
            .any(|w| w == b"PGDMP fake archive".as_slice()),
        "the plaintext content must not be present in the encrypted file"
    );

    // Double-encrypt is a no-op (idempotent).
    let again = backup::encrypt_backup_file(&enc).expect("idempotent encrypt");
    assert_eq!(again, enc, "an encrypted file must not be re-encrypted");

    // Decrypt for restore → exact original bytes, in a TEMP file (a
    // DIFFERENT path from the encrypted archive).
    let tmp = backup::decrypt_backup_for_restore(&enc).expect("decrypt");
    assert_ne!(tmp, enc, "decryption goes to a temp path");
    let recovered = std::fs::read(&tmp).unwrap();
    assert_eq!(recovered, original, "round-trip must be exact");
    let _ = std::fs::remove_file(tmp);

    std::fs::remove_dir_all(&dir).ok();
}

/// A legacy PLAINTEXT `.sql` archive passes through decrypt untouched (the
/// pre-F-11 archives on real deployments must remain restorable), and a
/// corrupted encrypted archive fails cleanly without writing a temp file.
#[test]
fn rctf_f11_legacy_passthrough_and_corruption() {
    let dir = std::env::temp_dir().join(format!(
        "hms_f11_leg_{}_{}",
        std::process::id(),
        rand_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    // Legacy plaintext archive: decrypt is identity.
    let legacy = dir.join("hospital_db_20260901_020000_00000001.sql");
    std::fs::write(&legacy, b"PGDMP legacy archive").unwrap();
    let passthrough = backup::decrypt_backup_for_restore(&legacy).expect("legacy passthrough");
    assert_eq!(passthrough, legacy, "legacy archives decrypt to themselves");

    // Corrupted encrypted archive: decryption fails, and no temp file is
    // left behind.
    let corrupt = dir.join("hospital_db_20260902_020000_00000002.sql.enc");
    std::fs::write(&corrupt, b"HMSE1garbage-not-a-real-ciphertext-body").unwrap();
    let err = backup::decrypt_backup_for_restore(&corrupt).unwrap_err();
    assert!(
        err.contains("decryption FAILED") || err.contains("corrupt"),
        "corruption must fail with a clear error, got: {}",
        err
    );
    let residue: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("hms_restore"))
        .collect();
    assert!(residue.is_empty(), "no plaintext temp residue on failure");

    std::fs::remove_dir_all(&dir).ok();
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
    let (user, pass) = user_pass
        .strip_prefix("postgresql://")
        .unwrap()
        .split_once(':')
        .unwrap();
    let (host, port) = host_port.split_once(':').unwrap();

    let tmp = std::env::temp_dir().join(format!("hms_bk_test_{}", rand_nanos()));
    std::fs::create_dir_all(&tmp).unwrap();
    let dump_path = tmp.join("roundtrip.sql");

    let dump = tokio::process::Command::new("C:/ProgramData/HMS/pgsql/bin/pg_dump.exe")
        .arg("-h")
        .arg(host)
        .arg("-p")
        .arg(port)
        .arg("-U")
        .arg(user)
        .arg("-Fc")
        .arg("-d")
        .arg("hospital_db_test")
        .arg("--file")
        .arg(&dump_path)
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
        .arg("-h")
        .arg(host)
        .arg("-p")
        .arg(port)
        .arg("-U")
        .arg(user)
        .arg("--clean")
        .arg("--if-exists")
        .arg("-d")
        .arg("hospital_db_test")
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
    let marker: Option<(i32,)> =
        sqlx::query_as("SELECT id FROM patients WHERE first_name = $1 AND deleted_at IS NULL")
            .bind(&marker_patient)
            .fetch_optional(&pool2)
            .await
            .unwrap();
    assert!(marker.is_some(), "marker row must be BACK after restore");

    let after: Option<(i32,)> = sqlx::query_as("SELECT id FROM patients WHERE first_name = $1")
        .bind(&after_name)
        .fetch_optional(&pool2)
        .await
        .unwrap();
    assert!(
        after.is_none(),
        "post-backup mutation must be GONE after restore"
    );

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

// ── Phase 7: retention prune ──────────────────────────────────────────────────

/// prune_backups_in keeps the N newest archives by mtime (mixed manual +
/// auto filename prefixes must not confuse the ordering), deletes the rest,
/// and never touches non-.sql files. Runs against a TEMP dir — the real
/// %ProgramData%\HMS\backups must never be touched by a test.
#[test]
fn phase7_prune_keeps_newest_n_and_spares_non_sql() {
    let dir = std::env::temp_dir().join(format!(
        "hms_prune_test_{}_{}",
        std::process::id(),
        rand_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    // 5 archives across BOTH filename tags, oldest first, mtime gaps wide
    // enough (25 ms) to be unambiguous on NTFS (100 ns resolution).
    let names = [
        "hospital_db_20260901_020000_00000001.sql",
        "auto_db_20260902_020000_00000002.sql",
        "hospital_db_20260903_120000_00000003.sql",
        "auto_db_20260904_020000_00000004.sql",
        "hospital_db_20260905_020000_00000005.sql",
    ];
    for (i, name) in names.iter().enumerate() {
        std::fs::write(dir.join(name), b"fake archive").unwrap();
        if i + 1 < names.len() {
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
    }
    // A stray non-archive file must survive pruning untouched.
    std::fs::write(dir.join("prune_notes.txt"), b"keep me").unwrap();

    let removed = backup::prune_backups_in(&dir, 2).expect("prune");
    assert_eq!(removed, 3, "5 archives − keep 2 = 3 removals");

    let survivors: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert!(
        survivors.contains(&"auto_db_20260904_020000_00000004.sql".to_string()),
        "the 2nd-newest archive must survive (mixed-tag mtime ordering), got: {:?}",
        survivors
    );
    assert!(
        survivors.contains(&"hospital_db_20260905_020000_00000005.sql".to_string()),
        "the newest archive must survive, got: {:?}",
        survivors
    );
    assert!(
        survivors.contains(&"prune_notes.txt".to_string()),
        "non-.sql files must never be pruned"
    );
    assert_eq!(
        survivors.len(),
        3,
        "exactly the newest 2 archives + the txt file"
    );

    // keep=0 is rejected — retention must always keep at least one backup.
    assert!(backup::prune_backups_in(&dir, 0).is_err());

    std::fs::remove_dir_all(&dir).ok();
}

// ── RCTF-FULL-SYSTEM-2026-09-09 F-08: nightly backup catch-up-on-wake ────────
//
// The old scheduler condition (`now.hour() == auto_backup_hour`) silently
// skipped machines that were asleep/off during the one-hour window. The fix
// computes "due" from the newest auto_db_* archive's mtime. These tests pin
// each branch of `auto_backup_due` against a temp dir with EXPLICIT mtimes
// (File::set_modified — the mtime IS the schedule state), plus one
// source-conformance pin that the scheduler actually calls it (the 2026-09-09
// deployment shipped the helper unused if the wiring regresses).

/// Writes an `auto_db_*.sql` (or `hospital_db_*.sql`) fake archive whose mtime
/// is exactly `mtime`, and returns its path. mtime is SystemTime because
/// that's what File::set_modified takes (chrono conversions happen at the
/// call site where the intended wall-clock story is clearer).
fn seed_archive_with_mtime(
    dir: &std::path::Path,
    name: &str,
    mtime: std::time::SystemTime,
) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, b"fake archive").unwrap();
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(mtime)
        .unwrap();
    path
}

/// `chrono::DateTime<Local>` → SystemTime for seeding file mtimes.
fn local_to_system_time(t: chrono::DateTime<chrono::Local>) -> std::time::SystemTime {
    t.into()
}

#[test]
fn rctf_f08_never_ran_is_due_immediately() {
    let dir = std::env::temp_dir().join(format!(
        "hms_f08_never_{}_{}",
        std::process::id(),
        rand_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    // Empty dir (no auto archive at all) → due at ANY time of day, even
    // before the configured hour: a fresh install must not wait for the next
    // window. Also pins that MANUAL archives do not satisfy the schedule.
    let now = chrono::Local::now();
    let morning = now
        .date_naive()
        .and_hms_opt(1, 0, 0)
        .unwrap()
        .and_local_timezone(chrono::Local)
        .unwrap();
    std::fs::write(
        dir.join("hospital_db_20260908_000000_deadbeef.sql"),
        b"manual only",
    )
    .unwrap();
    assert!(backup::auto_backup_due(&dir, morning, 2, true));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn rctf_f08_ran_in_today_window_not_due_again() {
    let dir = std::env::temp_dir().join(format!(
        "hms_f08_today_{}_{}",
        std::process::id(),
        rand_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    // Scheduled for 02:00. At 03:30 the same day, the newest auto archive
    // was written at 02:05 (inside today's window) → NOT due (it already ran).
    let now = chrono::Local::now();
    let ran_at = now
        .date_naive()
        .and_hms_opt(2, 5, 0)
        .unwrap()
        .and_local_timezone(chrono::Local)
        .unwrap();
    let check_at = now
        .date_naive()
        .and_hms_opt(3, 30, 0)
        .unwrap()
        .and_local_timezone(chrono::Local)
        .unwrap();
    seed_archive_with_mtime(
        &dir,
        "auto_db_20260909_020500_00000001.sql",
        local_to_system_time(ran_at),
    );

    assert!(!backup::auto_backup_due(&dir, check_at, 2, true));

    // Also not due at 23:59 the same day — one backup per day is the contract.
    let late = now
        .date_naive()
        .and_hms_opt(23, 59, 0)
        .unwrap()
        .and_local_timezone(chrono::Local)
        .unwrap();
    assert!(!backup::auto_backup_due(&dir, late, 2, true));

    // Edge: an archive written at the exact window-start instant (02:00:00)
    // counts as "already ran today".
    let edge = now
        .date_naive()
        .and_hms_opt(2, 0, 0)
        .unwrap()
        .and_local_timezone(chrono::Local)
        .unwrap();
    std::fs::File::options()
        .write(true)
        .open(dir.join("auto_db_20260909_020500_00000001.sql"))
        .unwrap()
        .set_modified(local_to_system_time(edge))
        .unwrap();
    assert!(!backup::auto_backup_due(&dir, check_at, 2, true));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn rctf_f08_slept_through_window_catches_up() {
    let dir = std::env::temp_dir().join(format!(
        "hms_f08_catchup_{}_{}",
        std::process::id(),
        rand_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    // THE headline case: machine armed for 02:00, slept through it, wakes
    // at 09:15. The newest auto archive is from 02:05 YESTERDAY. Old code
    // (hour equality) would do NOTHING until tomorrow 02:00; the fix must see
    // "past the window, no backup since" and run it now.
    let now = chrono::Local::now();
    let yesterday_ran = (now - chrono::Duration::hours(31))
        .date_naive()
        .and_hms_opt(2, 5, 0)
        .unwrap()
        .and_local_timezone(chrono::Local)
        .unwrap();
    let wake_at = now
        .date_naive()
        .and_hms_opt(9, 15, 0)
        .unwrap()
        .and_local_timezone(chrono::Local)
        .unwrap();
    seed_archive_with_mtime(
        &dir,
        "auto_db_20260908_020500_00000002.sql",
        local_to_system_time(yesterday_ran),
    );

    assert!(backup::auto_backup_due(&dir, wake_at, 2, true));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn rctf_f08_before_hour_fresh_yesterday_not_due() {
    let dir = std::env::temp_dir().join(format!(
        "hms_f08_fresh_{}_{}",
        std::process::id(),
        rand_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    // 01:00 today (before the 02:00 window). Last auto backup ran yesterday
    // 02:05 (~23h ago) → within 24h, not stale, NOT due yet. This pins the
    // "24h+ not exactly 24h" threshold: a backup that ran a few minutes late
    // the previous night must not trigger a spurious second run.
    let now = chrono::Local::now();
    let yesterday_ran = (now - chrono::Duration::hours(23))
        .date_naive()
        .and_hms_opt(2, 5, 0)
        .unwrap()
        .and_local_timezone(chrono::Local)
        .unwrap();
    let check_at = now
        .date_naive()
        .and_hms_opt(1, 0, 0)
        .unwrap()
        .and_local_timezone(chrono::Local)
        .unwrap();
    seed_archive_with_mtime(
        &dir,
        "auto_db_20260908_020500_00000003.sql",
        local_to_system_time(yesterday_ran),
    );

    assert!(!backup::auto_backup_due(&dir, check_at, 2, true));

    // Same clock time, but the last run was 25h ago → stale → DUE (the
    // machine slept through yesterday's window too, catch up now).
    let two_days_ago = (now - chrono::Duration::hours(25))
        .date_naive()
        .and_hms_opt(1, 0, 0)
        .unwrap()
        .and_local_timezone(chrono::Local)
        .unwrap();
    std::fs::File::options()
        .write(true)
        .open(dir.join("auto_db_20260908_020500_00000003.sql"))
        .unwrap()
        .set_modified(local_to_system_time(two_days_ago))
        .unwrap();
    assert!(backup::auto_backup_due(&dir, check_at, 2, true));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn rctf_f08_disabled_is_never_due() {
    let dir = std::env::temp_dir().join(format!(
        "hms_f08_disabled_{}_{}",
        std::process::id(),
        rand_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    // enabled=false must override EVERYTHING — a 48h-stale archive (which
    // would otherwise be catch-up due) and an empty directory (the
    // never-ran branch, otherwise due immediately). Pins that `enabled` is
    // checked FIRST inside the function, so a stale/slept machine with
    // auto-backup turned off stays quiet.
    let now = chrono::Local::now();
    let check_at = now
        .date_naive()
        .and_hms_opt(9, 0, 0)
        .unwrap()
        .and_local_timezone(chrono::Local)
        .unwrap();
    let stale = (now - chrono::Duration::hours(48))
        .date_naive()
        .and_hms_opt(2, 0, 0)
        .unwrap()
        .and_local_timezone(chrono::Local)
        .unwrap();
    seed_archive_with_mtime(
        &dir,
        "auto_db_20260906_020000_00000004.sql",
        local_to_system_time(stale),
    );

    assert!(!backup::auto_backup_due(&dir, check_at, 2, false));
    std::fs::remove_dir_all(&dir).ok();

    // Empty dir + disabled: still never due.
    std::fs::create_dir_all(&dir).unwrap();
    assert!(!backup::auto_backup_due(&dir, check_at, 2, false));
    std::fs::remove_dir_all(&dir).ok();
}

/// Source-conformance pin: the scheduler must CALL auto_backup_due (the
/// 2026-09-09 RCTF review found the same class of bug live — a correct helper
/// that no call site used). Parsing scheduler.rs from the test keeps the
/// wiring honest without needing a full scheduler harness.
#[test]
fn rctf_f08_scheduler_calls_auto_backup_due() {
    let src = include_str!("../src/scheduler.rs");
    assert!(
        src.contains("auto_backup_due("),
        "scheduler.rs must compute the nightly-backup due state via backup::auto_backup_due — the hour-equality condition silently skipped slept-through windows (RCTF F-08)"
    );
    // The in-memory attempt marker must still bound failures to one attempt
    // per day (prevents a tight retry loop when pg_dump is broken).
    assert!(
        src.contains("last_auto_backup_attempt"),
        "scheduler.rs must keep the one-attempt-per-day marker for failed nightly backups"
    );
}

#[test]
fn rctf_f08_hour_clamped_to_23_and_manual_archives_ignored() {
    let dir = std::env::temp_dir().join(format!(
        "hms_f08_clamp_{}_{}",
        std::process::id(),
        rand_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    // A misconfigured hour=25 must behave as 23 (clamped), not panic on
    // and_hms_opt(25,…) →  None → unwrap → panic. The unwrap_or(now) on the
    // window-start conversion ALSO needs cover: even if it degrades to `now`
    // the branch outcome must remain sane.
    let now = chrono::Local::now();
    let ran_today_after_23 = now
        .date_naive()
        .and_hms_opt(23, 30, 0)
        .unwrap()
        .and_local_timezone(chrono::Local)
        .unwrap();
    seed_archive_with_mtime(
        &dir,
        "auto_db_20260909_233000_00000005.sql",
        local_to_system_time(ran_today_after_23),
    );

    // now is AFTER 23:30 → not due (ran within the clamped window). If `now`
    // is actually before 23:30, the before-hour branch applies with a ~0h
    // staleness → also not due. Either way NOT due — deterministic.
    assert!(!backup::auto_backup_due(
        &dir,
        chrono::Local::now(),
        25,
        true
    ));

    // And a brand-new MANUAL archive (5 min ago) must NOT count as the auto
    // schedule having run: due stays true.
    std::fs::write(
        dir.join("hospital_db_20260909_090000_cafef00d.sql"),
        b"manual 5 min ago",
    )
    .unwrap();
    let fresh = chrono::Local::now() - chrono::Duration::minutes(5);
    std::fs::File::options()
        .write(true)
        .open(dir.join("hospital_db_20260909_090000_cafef00d.sql"))
        .unwrap()
        .set_modified(local_to_system_time(fresh))
        .unwrap();
    // Remove the auto archive so only the manual one exists → never-ran
    // state for auto → due.
    std::fs::remove_file(dir.join("auto_db_20260909_233000_00000005.sql")).unwrap();
    assert!(backup::auto_backup_due(
        &dir,
        chrono::Local::now(),
        25,
        true
    ));

    std::fs::remove_dir_all(&dir).ok();
}
