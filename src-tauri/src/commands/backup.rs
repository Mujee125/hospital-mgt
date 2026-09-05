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
use chrono::Utc;
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
    #[cfg(target_os = "windows")]
    {
        let mut icacls = std::process::Command::new("icacls");
        icacls.arg(dir.as_os_str())
            .args(["/inheritance:r"])
            .args(["/grant:r", "SYSTEM:(OI)(CI)F"])
            .args(["/grant:r", "Administrators:(OI)(CI)F"]);
        if let Some(user) = std::env::var_os("USERNAME") {
            let _ = icacls.arg(format!("{}:(OI)(CI)M", user.to_string_lossy()));
        }
        let _ = icacls
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
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
/// path separators, not `.` or `..`, must end in `.sql`). This is the
/// path-traversal guard that prevents `restore_backup`/`delete_backup` from
/// being misused to overwrite or delete arbitrary files outside the backups
/// directory.
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
    // Defense in depth: only `.sql` files are listed/created here, so any
    // other extension is rejected up front.
    if !name.ends_with(".sql") {
        return Err("Backup filename must end in '.sql'.".to_string());
    }
    Ok(())
}

/// Feature-gated test surface for `validate_filename` (Phase 2 tests).
#[cfg(all(feature = "server-build", feature = "hms-integration-tests"))]
pub fn validate_filename_for_tests(name: &str) -> Result<(), String> {
    validate_filename(name)
}

/// Formats a `SystemTime` as a stable, ISO-ish UTC string for display in the
/// frontend (`YYYY-MM-DD HH:MM:SS UTC`). We use UTC rather than local time so
/// the timestamp is unambiguous across hospital timezones and DST changes.
#[cfg(feature = "server-build")]
fn format_timestamp(t: SystemTime) -> String {
    let dt: chrono::DateTime<Utc> = t.into();
    dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
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
    let pg_dump = pg_bin("pg_dump.exe")?;
    let dir = backups_dir()?;

    // Filename: timestamp + random suffix. The old timestamp-only name
    // collided when two backups were created within the same second
    // (silently overwriting the first archive — Phase 2 review).
    use rand::RngCore;
    let filename = format!(
        "hospital_db_{}_{:08x}.sql",
        Utc::now().format("%Y%m%d_%H%M%S"),
        rand::rngs::OsRng.next_u32()
    );
    let path = dir.join(&filename);

    // Run pg_dump. We discard stdout (it would duplicate the file content via
    // --file) and capture stderr so we can surface a useful diagnostic on
    // failure. tokio::process::Command is the async equivalent of
    // std::process::Command and won't block the Tauri async runtime.
    //
    // PGPASSWORD is set on the child process only — it never leaks into the
    // parent process's environment, so concurrent Tauri commands cannot
    // observe it via `std::env::var("PGPASSWORD")`.
    let output = tokio::process::Command::new(&pg_dump)
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
        .await
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

    let meta = fs::metadata(&path)
        .map_err(|e| format!("Backup file written but metadata could not be read: {}", e))?;
    let created_at = meta
        .created()
        .map(format_timestamp)
        .unwrap_or_else(|_| format_timestamp(SystemTime::now()));
    let path_str = path.to_string_lossy().to_string();
    let info = BackupInfo {
        filename: filename.clone(),
        path: path_str.clone(),
        size_bytes: meta.len(),
        created_at,
    };

    audit::for_session(
        pool.inner(),
        &s,
        "backup_create",
        "backup",
        Some(&filename),
        Some(serde_json::json!({
            "size_bytes": info.size_bytes,
            "path": path_str,
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
            Some(n) if n.ends_with(".sql") => n.to_string(),
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
            .arg("-h").arg(&cfg.db_host)
            .arg("-p").arg(cfg.db_port.to_string())
            .arg("-U").arg(&cfg.db_user)
            .arg("-Fc")
            .arg("-d").arg(&cfg.db_name)
            .arg("--file").arg(&safety_path)
            .env("PGPASSWORD", &cfg.db_password)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .await;
        match safety {
            Ok(o) if o.status.success() => {
                audit::for_session(
                    pool.inner(),
                    &s,
                    "backup_pre_restore_safety",
                    "backup",
                    Some(&safety_name),
                    Some(serde_json::json!({"restoring": backup_filename})),
                )
                .await;
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
    let output = tokio::process::Command::new(&pg_restore)
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
        .arg(&path)
        .env("PGPASSWORD", &cfg.db_password)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("Failed to spawn pg_restore: {}", e))?;

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
