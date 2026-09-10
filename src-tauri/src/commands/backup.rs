//! Backup & Restore (SRS §9 A-07) — server-build-only module.
//!
//! Lets an admin create a full database backup via `pg_dump` and restore from
//! a backup file via `pg_restore`. Backups are stored under
//! `%ProgramData%\HMS\backups\` and named `hospital_db_YYYYMMDD_HHMMSS.sql`
//! (the file is produced by `pg_dump -Fc`, i.e. PostgreSQL's custom-format
//! binary archive — compressed, parallelisable, and restorable via
//! `pg_restore --clean --if-exists`).
//!
//! All four commands require `Permission::BackupsManage` and write an audit
//! row — including `restore_backup` (which is itself destructive — the audit
//! row captures the operator's identity for compliance review even though the
//! restored audit_logs table will be the backup's snapshot).
//!
//! SECURITY: `backup_filename` parameters are strictly validated to be plain
//! filenames (no path separators, no `.`/`..`, must end in `.sql`) before
//! being joined to the backups directory. This prevents path-traversal attacks
//! from overwriting arbitrary files via `restore_backup` or `delete_backup`.
//!
//! Non-server builds: every item in this module is `#[cfg(feature =
//! "server-build")]`-gated, so the module compiles to an empty namespace on
//! client/dev builds. The `pub mod backup;` declaration in `commands/mod.rs`
//! is unconditional (per the worklog contract); the generate_handler!
//! registrations in `lib.rs` are individually `#[cfg(feature = "server-build")]`-
//! gated to match.

#[cfg(feature = "server-build")]
use std::fs;
#[cfg(feature = "server-build")]
use std::path::PathBuf;
#[cfg(feature = "server-build")]
use std::time::SystemTime;

#[cfg(feature = "server-build")]
use chrono::{Timelike, Utc};
#[cfg(feature = "server-build")]
use sqlx::PgPool;

#[cfg(feature = "server-build")]
use crate::audit;
#[cfg(feature = "server-build")]
use crate::config::AppConfig;
#[cfg(feature = "server-build")]
use crate::models::BackupInfo;
#[cfg(feature = "server-build")]
use crate::rbac::{self, Permission, SessionState};

// ── Helpers (server-build only) ─────────────────────────────────────────────

/// Resolve the backups directory WITHOUT the ACL-hardening side effect of
/// `backups_dir()` — used by the scheduler's due-check (every 5-minute
/// tick), where spawning icacls each tick would be pure overhead. Does not
/// create the directory either: a missing dir simply means "no auto backups
/// yet" for the due-check.
#[cfg(feature = "server-build")]
pub fn backups_dir_path() -> Result<PathBuf, String> {
    let program_data = std::env::var_os("ProgramData")
        .ok_or_else(|| "ProgramData env var is not set.".to_string())?;
    Ok(PathBuf::from(program_data).join("HMS").join("backups"))
}

/// Backups directory: `%ProgramData%\HMS\backups\` on Windows.
///
/// Mirrors the `config.rs::machine_config_path` pattern. The directory is
/// created on first call so the very first `create_backup` invocation does
/// not fail with "path not found".
#[cfg(feature = "server-build")]
fn backups_dir() -> Result<PathBuf, String> {
    let program_data = std::env::var_os("ProgramData").ok_or_else(|| {
        "ProgramData env var is not set (backup commands require the server build on Windows)."
            .to_string()
    })?;
    let dir = PathBuf::from(program_data).join("HMS").join("backups");
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create backups directory: {}", e))?;

    // Phase 2 review (2026-09-05): a pg_dump custom-format archive is
    // compressed but NOT ENCRYPTED — a backup IS the entire PHI store in
    // readable form. This directory inherits Users:Modify from
    // ProgramData/HMS, so on pre-fix deployments every local user could
    // read (or silently replace) every backup. Harden the DIRECTORY on
    // every call (idempotent — also repairs existing deployments); files
    // created afterwards inherit the restrictive ACEs via (OI)(CI).
    // Pre-existing backup files keep their creation-time ACLs — delete or
    // re-create them after upgrading.
    //
    // Verification follow-up (2026-09-07, observed on the real install):
    // (1) the NSIS installer grants an EXPLICIT `BUILTIN\Users:(OI)(CI)(M)`
    //     ACE on this dir (so the non-elevated app can write backups) —
    //     `/inheritance:r` only strips INHERITED ACEs, so that explicit
    //     grant survived the original hardening. `/remove:g BUILTIN\Users`
    //     strips it; the app's own user is re-granted Modify below, so
    //     backup writes keep working.
    // (2) the status is NOT swallowed anymore: a non-elevated app cannot
    //     change ACLs (no WRITE_DAC), and a silent no-op left the PHI
    //     world-readable with no trace. It now logs, so an operator
    //     running elevated at least once gets the hardened posture.
    #[cfg(target_os = "windows")]
    {
        let mut icacls = std::process::Command::new("icacls");
        icacls
            .arg(dir.as_os_str())
            .args(["/inheritance:r"])
            .args(["/remove:g", "BUILTIN\\Users"])
            .args(["/grant:r", "SYSTEM:(OI)(CI)F"])
            .args(["/grant:r", "Administrators:(OI)(CI)F"]);
        if let Some(user) = std::env::var_os("USERNAME") {
            let _ = icacls.arg(format!("{}:(OI)(CI)M", user.to_string_lossy()));
        }
        match icacls
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .output()
        {
            Ok(out) if out.status.success() => {}
            Ok(out) => {
                eprintln!(
                    "[HMS Backup] ACL hardening failed (run the app elevated once to apply): {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                );
            }
            Err(e) => eprintln!("[HMS Backup] ACL hardening could not run: {}", e),
        }
    }

    Ok(dir)
}

/// Resolves the absolute path to a PostgreSQL binary (`pg_dump.exe` or
/// `pg_restore.exe`) inside the bundled pgsql/bin directory. Errors out
/// clearly if the binary is missing so the operator can repair the install
/// rather than getting an opaque "command not found" from `tokio::process`.
#[cfg(feature = "server-build")]
fn pg_bin(binary: &str) -> Result<PathBuf, String> {
    let bin_dir = crate::pg_provision::default_pg_bin_dir().ok_or_else(|| {
        "Could not locate PostgreSQL bin directory (ProgramData\\HMS\\pgsql\\bin). \
         Run the server-build installer to provision PostgreSQL."
            .to_string()
    })?;
    let exe = bin_dir.join(binary);
    if !exe.exists() {
        return Err(format!(
            "PostgreSQL tool '{}' not found at {}. The bundled PostgreSQL may be incomplete.",
            binary,
            exe.display()
        ));
    }
    Ok(exe)
}

/// Validates that a user-supplied `backup_filename` is a plain filename (no
/// path separators, not `.` or `..`, ends in `.sql` or the F-11 encrypted
/// extension `.sql.enc`). This is the path-traversal guard that prevents
/// `restore_backup`/`delete_backup` from being misused to overwrite or
/// delete arbitrary files outside the backups directory.
#[cfg(feature = "server-build")]
fn validate_filename(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Backup filename cannot be empty.".to_string());
    }
    if name.contains('/') || name.contains('\\') {
        return Err("Backup filename must not contain path separators.".to_string());
    }
    if name == "." || name == ".." {
        return Err("Backup filename must not be a path-traversal sequence.".to_string());
    }
    // Defense in depth: only `.sql` (legacy plaintext) and `.sql.enc`
    // (F-11 encrypted) files are listed/created here, so any other
    // extension is rejected up front.
    if !(name.ends_with(".sql.enc") || name.ends_with(".sql")) {
        return Err("Backup filename must end in '.sql' or '.sql.enc'.".to_string());
    }
    Ok(())
}

/// Feature-gated test surface for `validate_filename` (Phase 2 tests).
#[cfg(all(feature = "server-build", feature = "hms-integration-tests"))]
pub fn validate_filename_for_tests(name: &str) -> Result<(), String> {
    validate_filename(name)
}

/// F-11: a backup archive filename — legacy plaintext (`.sql`) or encrypted
/// (`.sql.enc`). Shared by every scan/filter site so the two formats stay
/// in step.
#[cfg(feature = "server-build")]
fn is_backup_archive(name: &str) -> bool {
    name.ends_with(".sql.enc") || name.ends_with(".sql")
}

/// Formats a `SystemTime` as a stable, ISO-ish UTC string for display in the
/// frontend (`YYYY-MM-DD HH:MM:SS UTC`). We use UTC rather than local time so
/// the timestamp is unambiguous across hospital timezones and DST changes.
#[cfg(feature = "server-build")]
fn format_timestamp(t: SystemTime) -> String {
    let dt: chrono::DateTime<Utc> = t.into();
    dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

// ── RCTF-FULL-SYSTEM-2026-09-08 F-11: backup encryption at rest ─────────────
//
// A pg_dump archive IS the entire PHI store in readable form — the code's
// own comment has said so since 2026-09-05. Every archive is now
// AES-256-GCM encrypted immediately after the pg_restore -l verification:
//   magic   "HMSE1"       (5 bytes — distinguishes from legacy plaintext)
//   nonce   12 bytes      (CSPRNG, unique per archive)
//   ct||tag ciphertext    (AES-256-GCM over the archive bytes)
// The key is SHA-256(entropy.key || b"hms-backup-v1") — derived from the
// same per-install secret that protects the DB password (config v3), so a
// stolen archive (or the whole USB stick) decrypts only on the install
// that made it, for the principals that can read entropy.key. Restore
// decrypts to a temp file, runs pg_restore, and deletes the plaintext
// immediately — encrypted archives are NEVER left lying in %TEMP% beyond
// the restore itself. Legacy `.sql` archives made before this change
// remain restorable (detected by the missing magic).
//
// All server-build gated (the backup module itself is server-only).

/// File magic for the encrypted-backup envelope.
#[cfg(feature = "server-build")]
pub const ENC_BACKUP_MAGIC: &str = "HMSE1";

/// Derive the backup key from the install entropy + a domain-separation
/// constant (the same entropy never reused raw for two purposes).
#[cfg(feature = "server-build")]
fn backup_key() -> Result<aes_gcm::Key<aes_gcm::Aes256Gcm>, String> {
    use sha2::Digest;
    let entropy = crate::config::AppConfig::load_or_create_entropy_for_secrets()?;
    let mut h = sha2::Sha256::new();
    h.update(&entropy);
    h.update(b"hms-backup-v1");
    // Digest → [u8; 32] key for Aes256Gcm (Key::clone_from_slice accepts
    // any AsRef<[u8]>; the digest's GenericArray converts losslessly).
    let hash = h.finalize();
    Ok(aes_gcm::Key::<aes_gcm::Aes256Gcm>::clone_from_slice(&hash))
}

/// Encrypt a finished backup archive in place: `name.sql` → `name.sql.enc`.
/// Returns the new filename. Idempotence: never re-encrypts (the magic
/// check); a missing source is an error (the verification step above must
/// have succeeded before we got here).
#[cfg(feature = "server-build")]
pub fn encrypt_backup_file(path: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let plaintext = fs::read(path).map_err(|e| format!("Read archive for encryption: {}", e))?;
    if plaintext.starts_with(ENC_BACKUP_MAGIC.as_bytes()) {
        // Already encrypted — nothing to do (defense against double calls).
        return Ok(path.to_path_buf());
    }
    use aes_gcm::AeadInPlace;
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
    use rand::RngCore;
    let key = backup_key()?;
    let cipher = Aes256Gcm::new(&key);
    let mut nonce_bytes = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let mut buffer = plaintext; // encrypt in place: buffer = ct || tag
    cipher
        .encrypt_in_place(nonce, b"", &mut buffer)
        .map_err(|_| "AES-GCM encryption of the archive failed".to_string())?;
    let mut out = Vec::with_capacity(5 + 12 + buffer.len());
    out.extend_from_slice(ENC_BACKUP_MAGIC.as_bytes());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&buffer);
    let enc_path = path.with_extension("sql.enc");
    fs::write(&enc_path, &out).map_err(|e| format!("Write encrypted archive: {}", e))?;
    // Replace the plaintext archive with the encrypted one — the plaintext
    // copy must not remain on disk.
    fs::remove_file(path).map_err(|e| format!("Remove plaintext archive: {}", e))?;
    Ok(enc_path)
}

/// Decrypt an encrypted archive to a TEMP file for restore. Returns the
/// temp path (caller must delete it after use). Plaintext-format archives
/// return the original path unchanged — restore stays compatible with
/// pre-F-11 archives.
#[cfg(feature = "server-build")]
pub fn decrypt_backup_for_restore(path: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let head = read_prefix(path, ENC_BACKUP_MAGIC.len() + 1)?;
    if !head.starts_with(ENC_BACKUP_MAGIC.as_bytes()) {
        return Ok(path.to_path_buf()); // legacy plaintext archive
    }
    let blob = fs::read(path).map_err(|e| format!("Read encrypted archive: {}", e))?;
    if blob.len() < 5 + 12 + 16 {
        return Err("Encrypted archive is truncated/corrupt.".to_string());
    }
    use aes_gcm::AeadInPlace;
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
    let key = backup_key()?;
    let cipher = Aes256Gcm::new(&key);
    let nonce = Nonce::from_slice(&blob[5..17]);
    let mut buffer = blob[17..].to_vec();
    cipher
        .decrypt_in_place(nonce, b"", &mut buffer)
        .map_err(|_| {
            "Backup decryption FAILED (wrong key or corrupt archive). This archive was \
             encrypted on a different install, or entropy.key has changed."
                .to_string()
        })?;
    let tmp = std::env::temp_dir().join(format!(
        "hms_restore_{}_{}.sql.enc.tmp",
        std::process::id(),
        {
            use rand::RngCore;
            rand::rngs::OsRng.next_u32()
        }
    ));
    fs::write(&tmp, &buffer).map_err(|e| format!("Write decrypted temp archive: {}", e))?;
    Ok(tmp)
}

/// Read at most `n` bytes of a file (prefix probe) without loading it all.
#[cfg(feature = "server-build")]
fn read_prefix(path: &std::path::Path, n: usize) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut f = fs::File::open(path).map_err(|e| format!("Open archive: {}", e))?;
    let mut buf = vec![0u8; n];
    let mut read = 0;
    while read < n {
        let got = f
            .read(&mut buf[read..])
            .map_err(|e| format!("Read archive head: {}", e))?;
        if got == 0 {
            break;
        }
        read += got;
    }
    buf.truncate(read);
    Ok(buf)
}

// ── Phase 7: backup core (shared by the manual command and the nightly
//    scheduler job) ─────────────────────────────────────────────────────────

/// Run one pg_dump archive + verify its readability + return its metadata.
/// Extracted from `create_backup` (Phase 7) so the scheduler can run the
/// EXACT same code path — the command adds RBAC + audit on top.
///
/// `tag` distinguishes the trigger in the filename prefix ("hospital_db"
/// for manual, "auto_db" for scheduled) so an operator can tell a 2 AM
/// machine backup from a click.
#[cfg(feature = "server-build")]
pub fn run_backup_core(cfg: &AppConfig, tag: &str) -> Result<BackupInfo, String> {
    let pg_dump = pg_bin("pg_dump.exe")?;
    let dir = backups_dir()?;

    // Filename: timestamp + random suffix. The old timestamp-only name
    // collided when two backups were created within the same second
    // (silently overwriting the first archive — Phase 2 review).
    use rand::RngCore;
    let filename = format!(
        "{}_{}_{:08x}.sql",
        tag,
        Utc::now().format("%Y%m%d_%H%M%S"),
        rand::rngs::OsRng.next_u32()
    );
    let path = dir.join(&filename);

    // Run pg_dump (custom format `-Fc`: compressed, restorable via
    // pg_restore). PGPASSWORD is set on the child process only — it never
    // leaks into the parent process's environment.
    let output = std::process::Command::new(&pg_dump)
        .arg("-h")
        .arg(&cfg.db_host)
        .arg("-p")
        .arg(cfg.db_port.to_string())
        .arg("-U")
        .arg(&cfg.db_user)
        .arg("-Fc")
        .arg("-d")
        .arg(&cfg.db_name)
        .arg("--file")
        .arg(&path)
        .env("PGPASSWORD", &cfg.db_password)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to spawn pg_dump: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Best-effort cleanup of the partial file so the next list_backups
        // call doesn't show a 0-byte artifact.
        let _ = fs::remove_file(&path);
        return Err(format!(
            "pg_dump failed (exit {}): {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ));
    }

    // Verify the archive is a readable pg_dump custom-format archive BEFORE
    // declaring success (Phase 7 restore-verification): `pg_restore -l`
    // lists the archive's table-of-contents — a truncated/corrupt dump
    // fails here, so we delete the partial file and surface the error
    // instead of silently keeping a backup that cannot be restored.
    let pg_restore = pg_bin("pg_restore.exe")?;
    let check = std::process::Command::new(&pg_restore)
        .arg("-l")
        .arg(&path)
        .env("PGPASSWORD", &cfg.db_password)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to spawn pg_restore (verify): {}", e))?;
    if !check.status.success() {
        let stderr = String::from_utf8_lossy(&check.stderr);
        let _ = fs::remove_file(&path);
        return Err(format!(
            "Backup written but FAILED archive verification (pg_restore -l): {}",
            stderr.trim()
        ));
    }

    let meta = fs::metadata(&path)
        .map_err(|e| format!("Backup file written but metadata could not be read: {}", e))?;
    let created_at = meta
        .created()
        .map(format_timestamp)
        .unwrap_or_else(|_| format_timestamp(SystemTime::now()));
    let size_bytes = meta.len();

    // F-11: encrypt the verified archive at rest. Failure to ENCRYPT is
    // NOT tolerable (that would leave the PHI archive plaintext on disk
    // after this fix shipped) — but the backup itself already succeeded,
    // so on encryption failure we DELETE the plaintext archive and
    // surface the error rather than keeping an unprotected PHI file.
    let (filename, path) = match encrypt_backup_file(&path) {
        Ok(enc_path) => {
            let name = enc_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&filename)
                .to_string();
            (name, enc_path)
        }
        Err(e) => {
            let _ = fs::remove_file(&path);
            return Err(format!(
                "Backup created and verified, but encryption FAILED — the plaintext \
                 archive was deleted rather than left unprotected: {}",
                e
            ));
        }
    };

    Ok(BackupInfo {
        filename,
        path: path.to_string_lossy().to_string(),
        size_bytes,
        created_at,
    })
}

/// Copy a finished backup archive to the optional USB/secondary directory
/// (Phase 7). Best-effort: a missing/unplugged drive logs a warning and
/// returns Ok — the primary archive is already safe. Returns Ok(true) when
/// the copy happened.
#[cfg(feature = "server-build")]
pub fn copy_to_usb(usb_path: &str, source: &std::path::Path) -> Result<bool, String> {
    if usb_path.trim().is_empty() {
        return Ok(false);
    }
    let dest_dir = PathBuf::from(usb_path.trim());
    // Only attempt when the destination volume is actually mounted — an
    // unplugged USB stick must not fail the (already-successful) backup.
    if !dest_dir.is_dir() {
        return Ok(false);
    }
    let Some(filename) = source.file_name() else {
        return Ok(false);
    };
    fs::copy(source, dest_dir.join(filename))
        .map(|_| true)
        .map_err(|e| format!("USB copy failed: {}", e))
}

/// Retention: delete the oldest backup files beyond `keep`, newest-first
/// ordering by file modification time. Manual backups and auto backups
/// share one pool — an operator's manual snapshot counts toward retention
/// too, so the directory can never grow unbounded. (Sorted by mtime, not
/// filename: manual and scheduled archives carry different prefixes, so a
/// filename sort would interleave the two tags non-chronologically.)
/// Returns how many files were removed. Only deletes `.sql` files directly
/// inside the given directory.
#[cfg(feature = "server-build")]
pub fn prune_backups(keep: usize) -> Result<usize, String> {
    prune_backups_in(&backups_dir()?, keep)
}

/// RCTF-FULL-SYSTEM-2026-09-09 F-08: should the nightly auto-backup run NOW?
///
/// The previous scheduler condition fired only when
/// `now.hour() == auto_backup_hour` — a machine that was asleep (or the app
/// closed) during that one-hour window simply NEVER backed up, with no
/// error and no signal (observed live on the 2026-09-09 deployment: armed
/// for 00:00, never executed). The window is widened to "missed the
/// scheduled moment and has not backed up since" via the mtime of the
/// newest `auto_db_*` archive:
///   • normal night: the first tick at/after the configured hour runs it;
///   • slept-through night: the first tick after wake sees the newest auto
///     archive is older than ~24h and runs it immediately (catch-up);
///   • app closed for days: same — first boot after reopening backs up.
/// The 24h+ threshold (not exactly 24h) tolerates a backup that ran a few
/// minutes late the previous night. Manual `hospital_db_*` archives do NOT
/// count — an operator's manual snapshot is not the scheduled regime.
#[cfg(feature = "server-build")]
pub fn auto_backup_due(
    dir: &std::path::Path,
    now: chrono::DateTime<chrono::Local>,
    auto_backup_hour: u32,
    enabled: bool,
) -> bool {
    if !enabled {
        return false;
    }
    let hour = auto_backup_hour.min(23);
    // Before the scheduled hour today (and we already have a recent auto
    // backup from yesterday's window) → not due yet.
    let newest_auto = newest_auto_backup_mtime(dir);
    let last_run_or_dawn = match newest_auto {
        Some(t) => t,
        None => {
            // Never ran: due immediately — a fresh install (or auto-backup
            // newly enabled) must not wait for the next window when there
            // is not a single auto archive at all.
            return true;
        }
    };
    if now.hour() < hour {
        // Not yet the window today; due only if the last auto backup is
        // already > 24h stale (machine was asleep through yesterday too).
        let stale = now.signed_duration_since(last_run_or_dawn).num_hours() >= 24;
        return stale;
    }
    // At/after the scheduled hour: due unless an auto backup already ran
    // within this calendar day's window (i.e. within the last 24h AND after
    // the most recent scheduled-hour boundary).
    let today_window_start = now
        .date_naive()
        .and_hms_opt(hour, 0, 0)
        .and_then(|naive| naive.and_local_timezone(chrono::Local).single())
        .unwrap_or(now);
    let already_ran_today = last_run_or_dawn >= today_window_start;
    !already_ran_today
}

/// mtime of the newest `auto_db_*.sql` archive in `dir` (None if none).
#[cfg(feature = "server-build")]
fn newest_auto_backup_mtime(dir: &std::path::Path) -> Option<chrono::DateTime<chrono::Local>> {
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .map(is_backup_archive)
                    .unwrap_or(false)
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("auto_db_"))
                    .unwrap_or(false)
        })
        .filter_map(|p| fs::metadata(&p).ok()?.modified().ok())
        .map(|t| -> chrono::DateTime<chrono::Local> { t.into() })
        .max()
}

/// The retention core over an explicit directory — testable against a temp
/// dir (the real backups directory must never be touched by tests).
#[cfg(feature = "server-build")]
pub fn prune_backups_in(dir: &std::path::Path, keep: usize) -> Result<usize, String> {
    if keep == 0 {
        return Err("Retention must keep at least 1 backup.".to_string());
    }
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = fs::read_dir(dir)
        .map_err(|e| format!("Read backups dir: {}", e))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .map(is_backup_archive)
                    .unwrap_or(false)
        })
        .filter_map(|p| {
            let mtime = fs::metadata(&p).ok()?.modified().ok()?;
            Some((mtime, p))
        })
        .collect();
    // Newest first by modification time; ties broken by filename for
    // determinism in tests.
    files.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.file_name().cmp(&a.1.file_name())));
    let mut removed = 0;
    for (_, old) in files.into_iter().skip(keep) {
        if fs::remove_file(&old).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

// ── Commands (server-build only) ────────────────────────────────────────────

/// Create a new full-database backup. Runs
///   `pg_dump -h <host> -p <port> -U <user> -Fc -d <dbname> --file=<path>`
/// and returns the resulting `BackupInfo` (filename, path, size, creation
/// time).
///
/// The DB password is supplied to pg_dump via the `PGPASSWORD` environment
/// variable — the canonical PostgreSQL auth affordance for non-interactive
/// invocations. This avoids URL-encoding pitfalls with passwords containing
/// URL-special characters (`@`, `:`, `/`, `?`, `#`) and matches the SRS §9
/// A-07 spec wording ("Set PGPASSWORD env var from `AppConfig.db_password`").
///
/// The custom format (`-Fc`) is used (rather than plain SQL) because it is
/// compressed, supports parallel restore, and works correctly with
/// `pg_restore --clean`. The filename ends in `.sql` per the SRS spec; the
/// extension is a presentation choice — the file's actual contents are a
/// custom-format binary archive, not plain SQL text.
#[cfg(feature = "server-build")]
#[tauri::command]
pub async fn create_backup(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    app_handle: tauri::AppHandle,
) -> Result<BackupInfo, String> {
    let s = rbac::require_strong(&session, pool.inner(), Permission::BackupsManage).await?;

    let cfg = AppConfig::load(&app_handle).ok_or_else(|| {
        "Server config not found — run first-run setup before creating a backup.".to_string()
    })?;

    // Phase 7: pg_dump + archive verification moved into `run_backup_core`
    // (shared verbatim with the nightly scheduler job). This call is
    // spawn_blocking because the core runs the dump synchronously; the
    // async runtime must not block on a potentially minutes-long dump.
    let retention = cfg.backup_retention_count.max(1) as usize;
    let info = tauri::async_runtime::spawn_blocking(move || run_backup_core(&cfg, "hospital_db"))
        .await
        .map_err(|e| format!("Backup task join failed: {}", e))??;

    // Retention is applied after every successful backup — manual or
    // scheduled — so the backups directory can never grow unbounded.
    match prune_backups(retention) {
        Ok(n) if n > 0 => {
            eprintln!(
                "[HMS Backup] Retention: removed {} archive(s) beyond keep={}",
                n, retention
            );
        }
        Ok(_) => {}
        Err(e) => eprintln!("[HMS Backup] Retention prune failed (non-fatal): {}", e),
    }

    audit::for_session(
        pool.inner(),
        &s,
        "backup_create",
        "backup",
        Some(&info.filename),
        Some(serde_json::json!({
            "size_bytes": info.size_bytes,
            "path": info.path,
        })),
    )
    .await;

    Ok(info)
}

/// List available backups (sorted newest first by filename, which encodes the
/// creation timestamp). Scans the backups directory for `.sql` files and
/// reads size + creation time from filesystem metadata.
///
/// `pool` and `app_handle` are accepted for API symmetry with the other three
/// commands; this command does not touch the database or the config, so they
/// are intentionally unused.
#[cfg(feature = "server-build")]
#[tauri::command]
pub async fn list_backups(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    app_handle: tauri::AppHandle,
) -> Result<Vec<BackupInfo>, String> {
    let _ = rbac::require(&session, Permission::BackupsManage)?;
    // API-symmetry placeholders — see doc comment.
    let _ = &pool;
    let _ = &app_handle;

    let dir = backups_dir()?;
    let mut entries: Vec<BackupInfo> = Vec::new();
    let read = match fs::read_dir(&dir) {
        Ok(r) => r,
        Err(_) => return Ok(Vec::new()), // Directory doesn't exist yet → no backups.
    };
    for entry in read.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let filename = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) if is_backup_archive(n) => n.to_string(),
            _ => continue,
        };
        let meta = match fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let created_at = meta
            .created()
            .map(format_timestamp)
            .unwrap_or_else(|_| format_timestamp(SystemTime::now()));
        entries.push(BackupInfo {
            filename,
            path: path.to_string_lossy().to_string(),
            size_bytes: meta.len(),
            created_at,
        });
    }
    // Sort newest first by filename (the filename's YYYYMMDD_HHMMSS segment
    // sorts lexically ascending chronologically).
    entries.sort_by(|a, b| b.filename.cmp(&a.filename));
    Ok(entries)
}

/// Restore from a backup. **Destructive** — replaces all current data.
///
/// Runs `pg_restore -h <host> -p <port> -U <user> --clean --if-exists -d
/// <dbname> <path>`. After a successful restore, the app's DB pool may hold
/// connections with stale prepared statements (the underlying tables have
/// been dropped+recreated); the user MUST restart the app to get a clean
/// pool. The frontend shows a prominent warning banner to this effect.
///
/// pg_restore exit codes: 0 = success, 1 = success-with-warnings (e.g. "DROP
/// TABLE IF EXISTS" notices), 2+ = fatal error. We treat code > 1 as failure.
///
/// `backup_filename` is a bare filename (no path) that is joined to the
/// backups directory after passing `validate_filename`. This is a deliberate
/// defense-in-depth measure: the frontend only ever passes filenames returned
/// by `list_backups`, but the backend is the security boundary so it
/// re-validates. (The function parameter is named `backup_filename` rather
/// than `backup_path` for backward compatibility with the existing
/// `Backup.tsx` page — the value is a filename, not a path, despite the
/// spec's wording.)
#[cfg(feature = "server-build")]
#[tauri::command]
pub async fn restore_backup(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    app_handle: tauri::AppHandle,
    backup_filename: String,
) -> Result<(), String> {
    let s = rbac::require_strong(&session, pool.inner(), Permission::BackupsManage).await?;
    validate_filename(&backup_filename)?;

    let cfg = AppConfig::load(&app_handle).ok_or_else(|| {
        "Server config not found — run first-run setup before restoring.".to_string()
    })?;
    let pg_restore = pg_bin("pg_restore.exe")?;
    let dir = backups_dir()?;
    let path = dir.join(&backup_filename);
    if !path.exists() {
        return Err(format!("Backup file not found: {}", backup_filename));
    }

    // Phase 2 review (2026-09-05): SAFETY BACKUP before the destructive
    // restore. pg_restore --clean drops every object before recreating from
    // the archive; if the archive is corrupt or the restore aborts midway,
    // the database can be left partially dropped with no way back. A
    // pre-restore dump gives the operator a recovery point. Best-effort by
    // design: if the safety dump itself fails, we still proceed — the
    // operator explicitly requested the restore and the audit row records
    // it (blocking on a failing safety dump could strand a hospital with
    // no path forward during an incident).
    {
        use rand::RngCore;
        let safety_name = format!(
            "hospital_db_pre_restore_{}_{:08x}.sql",
            Utc::now().format("%Y%m%d_%H%M%S"),
            rand::rngs::OsRng.next_u32()
        );
        let safety_path = dir.join(&safety_name);
        let pg_dump = pg_bin("pg_dump.exe")?;
        let safety = tokio::process::Command::new(&pg_dump)
            .arg("-h")
            .arg(&cfg.db_host)
            .arg("-p")
            .arg(cfg.db_port.to_string())
            .arg("-U")
            .arg(&cfg.db_user)
            .arg("-Fc")
            .arg("-d")
            .arg(&cfg.db_name)
            .arg("--file")
            .arg(&safety_path)
            .env("PGPASSWORD", &cfg.db_password)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .await;
        match safety {
            Ok(o) if o.status.success() => {
                // F-11: the safety dump is as much PHI as any archive —
                // encrypt it too (best-effort: a failed encryption removes
                // the plaintext file and logs; the RESTORE proceeds as the
                // operator requested, exactly like a failed safety dump).
                let safety_name_enc = match encrypt_backup_file(&safety_path) {
                    Ok(enc) => enc
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&safety_name)
                        .to_string(),
                    Err(e) => {
                        let _ = fs::remove_file(&safety_path);
                        eprintln!(
                            "[HMS BACKUP] Warning: safety-dump encryption failed, plaintext \
                             removed ({}); continuing the restore as requested.",
                            e
                        );
                        String::new()
                    }
                };
                if !safety_name_enc.is_empty() {
                    audit::for_session(
                        pool.inner(),
                        &s,
                        "backup_pre_restore_safety",
                        "backup",
                        Some(&safety_name_enc),
                        Some(serde_json::json!({"restoring": backup_filename})),
                    )
                    .await;
                }
            }
            _ => {
                let _ = fs::remove_file(&safety_path);
                eprintln!(
                    "[HMS BACKUP] Warning: pre-restore safety dump FAILED — proceeding with restore of {} as requested (audit row notes this).",
                    backup_filename
                );
            }
        }
    }

    // Run pg_restore with --clean --if-exists so existing objects are dropped
    // before being recreated. We do NOT pass --no-owner / --no-acl — those
    // would skip ownership + access rules, which are part of the snapshot and
    // should be restored.
    //
    // PGPASSWORD is set on the child process only — never on the parent env.
    //
    // F-11: encrypted archives are decrypted to a TEMP file first; the
    // plaintext temp is deleted after the restore regardless of outcome
    // (it must never outlive the operation). Legacy `.sql` archives pass
    // through `decrypt_backup_for_restore` unchanged.
    let restore_source = decrypt_backup_for_restore(&path)?;
    let temp_to_clean: Option<&std::path::Path> = if restore_source != path {
        Some(&restore_source)
    } else {
        None
    };
    let restore_result: Result<std::process::Output, String> =
        tokio::process::Command::new(&pg_restore)
            .arg("-h")
            .arg(&cfg.db_host)
            .arg("-p")
            .arg(cfg.db_port.to_string())
            .arg("-U")
            .arg(&cfg.db_user)
            .arg("--clean")
            .arg("--if-exists")
            .arg("-d")
            .arg(&cfg.db_name)
            .arg(&restore_source)
            .env("PGPASSWORD", &cfg.db_password)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .await
            .map_err(|e| format!("Failed to spawn pg_restore: {}", e));
    // The decrypted temp (if any) is removed IMMEDIATELY — success or
    // failure — so no plaintext PHI archive is left in %TEMP%.
    if let Some(tmp) = temp_to_clean {
        let _ = fs::remove_file(tmp);
    }
    let output = restore_result?;

    let code = output.status.code().unwrap_or(-1);
    if code > 1 {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "pg_restore failed (exit {}): {}",
            code,
            stderr.trim()
        ));
    }

    // Write the audit row AFTER pg_restore. The restored `audit_logs` table is
    // now the backup's snapshot; this row records that "backup_restore" was
    // performed by this operator at this time. If the pool's prepared
    // statements are stale post-restore, sqlx will re-prepare on the next
    // execute; if that still fails, audit::for_session swallows the error so
    // the command still returns Ok — the operator has already been told to
    // restart the app.
    audit::for_session(
        pool.inner(),
        &s,
        "backup_restore",
        "backup",
        Some(&backup_filename),
        Some(serde_json::json!({
            "path": path.to_string_lossy().to_string(),
            "pg_restore_exit": code,
        })),
    )
    .await;

    Ok(())
}

/// Delete a backup file (irreversible — the file is removed from disk). The
/// database itself is NOT touched; if you want to roll back to a previous
/// state, use `restore_backup` first.
///
/// `backup_filename` is a bare filename (no path) that is joined to the
/// backups directory after passing `validate_filename`. See `restore_backup`
/// for the rationale.
#[cfg(feature = "server-build")]
#[tauri::command]
pub async fn delete_backup(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    app_handle: tauri::AppHandle,
    backup_filename: String,
) -> Result<(), String> {
    let s = rbac::require_strong(&session, pool.inner(), Permission::BackupsManage).await?;
    validate_filename(&backup_filename)?;

    // `app_handle` is unused here but kept in the signature for symmetry with
    // the other three commands (so the frontend can call all four with the
    // same invoke contract).
    let _ = &app_handle;

    let dir = backups_dir()?;
    let path = dir.join(&backup_filename);
    if !path.exists() {
        return Err(format!("Backup file not found: {}", backup_filename));
    }
    fs::remove_file(&path).map_err(|e| format!("Failed to delete backup file: {}", e))?;

    audit::for_session(
        pool.inner(),
        &s,
        "backup_delete",
        "backup",
        Some(&backup_filename),
        None,
    )
    .await;

    Ok(())
}
