//! System Health (Phase 7, SRS §9 A-07 companion) — one admin-facing
//! snapshot of everything that fails silently if unwatched:
//!
//!   • Database: size, active connections, server version
//!   • Scheduler: heartbeat age (a dead scheduler silently stops the
//!     1-hour reminders, the blood-expiry sweep, AND the nightly backup)
//!   • Backups: count + age of the newest archive in
//!     %ProgramData%\HMS\backups (no backup in N days = the classic
//!     "we thought it was running" outage)
//!   • Disk: free bytes on the volume hosting the backups directory —
//!     a full disk is the most common way backups stop
//!
//! Read-only; gated by SettingsManage (admin/owner screen). No audit row
//! (per audit.rs design: reads are not audited).

use serde::Serialize;
use sqlx::PgPool;

use crate::rbac::{self, Permission, SessionState};

#[derive(Debug, Serialize)]
pub struct SystemHealth {
    pub server_time: String,
    pub app_version: String,
    // Database
    pub db_name: String,
    pub db_size_bytes: i64,
    pub active_connections: i64,
    pub postgres_version: String,
    // Scheduler
    /// Seconds since the last scheduler tick; null = not ticked yet since
    /// process start.
    pub scheduler_last_tick_age_secs: Option<u64>,
    /// Convenience flag: tick age under 15 minutes (3× the 5-min interval).
    pub scheduler_healthy: bool,
    // Backups
    pub backup_count: i64,
    /// Age of the newest backup archive in HOURS; null = no backups at all.
    pub latest_backup_age_hours: Option<i64>,
    pub latest_backup_filename: Option<String>,
    // Disk (volume hosting the backups directory / ProgramData)
    pub disk_free_bytes: Option<u64>,
    pub disk_total_bytes: Option<u64>,
}

#[tauri::command]
pub async fn get_system_health(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
) -> Result<SystemHealth, String> {
    let _ = rbac::require(&session, Permission::SettingsManage)?;

    // ── Database facts (one round trip each; all trivially indexed) ─────
    let (db_size,): (i64,) = sqlx::query_as("SELECT pg_database_size(current_database())")
        .fetch_one(&*pool)
        .await
        .map_err(|e| format!("Health (db size): {}", e))?;

    let (db_name,): (String,) = sqlx::query_as("SELECT current_database()")
        .fetch_one(&*pool)
        .await
        .map_err(|e| format!("Health (db name): {}", e))?;

    let (conns,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM pg_stat_activity WHERE datname = current_database()",
    )
    .fetch_one(&*pool)
    .await
    .map_err(|e| format!("Health (connections): {}", e))?;

    let (pg_version_full,): (String,) = sqlx::query_as("SELECT version()")
        .fetch_one(&*pool)
        .await
        .map_err(|e| format!("Health (version): {}", e))?;
    // "PostgreSQL 16.4 (Debian ...)" → "PostgreSQL 16.4"
    let postgres_version = pg_version_full
        .split_whitespace()
        .take(2)
        .collect::<Vec<_>>()
        .join(" ");

    // ── Scheduler heartbeat (in-process static, no DB) ──────────────────
    let tick_age = crate::scheduler::last_tick_age_secs();
    // 3× the 5-minute tick interval — one missed tick is tolerable, three
    // consecutive misses means the loop is wedged.
    let scheduler_healthy = tick_age.map(|a| a < 900).unwrap_or(false);

    // ── Backup directory (filesystem; same dir as list_backups) ────────
    let (backup_count, latest_age_hours, latest_filename) = scan_backups();

    // ── Disk free on the backups volume (Windows) ────────────────────────
    let (disk_free, disk_total) = disk_free_on_backups_volume();

    Ok(SystemHealth {
        server_time: chrono::Utc::now()
            .format("%Y-%m-%d %H:%M:%S UTC")
            .to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        db_name,
        db_size_bytes: db_size,
        active_connections: conns,
        postgres_version,
        scheduler_last_tick_age_secs: tick_age,
        scheduler_healthy,
        backup_count,
        latest_backup_age_hours: latest_age_hours,
        latest_backup_filename: latest_filename,
        disk_free_bytes: disk_free,
        disk_total_bytes: disk_total,
    })
}

/// Scan %ProgramData%\HMS\backups for `.sql` archives; return (count, newest
/// age in hours, newest filename). Never fails — a missing directory is
/// simply "no backups" (fresh install before first run).
fn scan_backups() -> (i64, Option<i64>, Option<String>) {
    let Some(program_data) = std::env::var_os("ProgramData") else {
        return (0, None, None);
    };
    let dir = std::path::Path::new(&program_data)
        .join("HMS")
        .join("backups");
    let Ok(read) = std::fs::read_dir(&dir) else {
        return (0, None, None);
    };
    let mut count: i64 = 0;
    let mut newest: Option<(std::time::SystemTime, String)> = None;
    for entry in read.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.ends_with(".sql") {
            continue;
        }
        count += 1;
        let modified = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let is_newer = newest.as_ref().map(|(t, _)| modified > *t).unwrap_or(true);
        if is_newer {
            newest = Some((modified, name));
        }
    }
    let latest_age_hours = newest.as_ref().map(|(t, _)| {
        let age = std::time::SystemTime::now()
            .duration_since(*t)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        age / 3600
    });
    (count, latest_age_hours, newest.map(|(_, n)| n))
}

/// Free/total bytes on the volume hosting %ProgramData% (where the backups
/// directory lives) via GetDiskFreeSpaceExW. Returns (None, None) on
/// non-Windows builds or when the call fails — the dashboard treats null
/// as "unknown", never as an error.
#[cfg(target_os = "windows")]
fn disk_free_on_backups_volume() -> (Option<u64>, Option<u64>) {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let Some(program_data) = std::env::var_os("ProgramData") else {
        return (None, None);
    };
    // GetDiskFreeSpaceExW wants a root-anchored path; the ProgramData drive
    // prefix ("C:\") is the volume the backups live on.
    let path = std::path::Path::new(&program_data)
        .ancestors()
        .last()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from(&program_data));
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);

    let mut free_to_caller: u64 = 0;
    let mut total: u64 = 0;
    let mut free_total: u64 = 0;
    // SAFETY: three valid u64 out-pointers and a NUL-terminated wide path;
    // the call only writes the out-params.
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            PCWSTR(wide.as_ptr()),
            Some(&mut free_to_caller),
            Some(&mut total),
            Some(&mut free_total),
        )
    };
    if ok.is_ok() {
        (Some(free_to_caller), Some(total))
    } else {
        (None, None)
    }
}

#[cfg(not(target_os = "windows"))]
fn disk_free_on_backups_volume() -> (Option<u64>, Option<u64>) {
    (None, None)
}
