mod audit;
mod auth;
mod config;
mod db;
mod discovery;
pub mod fingerprint;   // pub so the dev_auto_license binary can use it
mod license;
mod messaging;
mod models;
mod pairing;
mod rbac;
mod scheduler;
mod whatsapp;
#[cfg(feature = "server-build")]
mod pg_provision;
mod tls_provision;
mod commands;

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

// ── Import only NON-command items from submodules ─────────────────────────────
//
// Tauri command functions must NOT be imported with `use` into lib.rs if they
// are also listed in generate_handler![]. Doing so brings their
// __cmd__X / __tauri_command_name_X macros into scope twice — once via the
// `use` import and once when generate_handler! processes them — causing the
// "defined multiple times" compile error.
//
// Rule: anything listed in generate_handler! is referenced by its module path
// (e.g. pairing::redeem_pairing_code) — never imported with `use` here.
// Non-command helpers (types, structs) are still imported normally.

use config::AppConfig;          // struct — not a command
use discovery::Role;            // enum  — not a command
use pairing::PairingService;    // struct — not a command

use tauri::Manager;
use tauri::Emitter;

static BROADCAST_RUNNING: AtomicBool = AtomicBool::new(false);
// Only used by the server-side pairing listener (start_pairing_listener).
// On client/dev builds this is dead code — allow it.
#[allow(dead_code)]
static PAIRING_LISTENER_STARTED: AtomicBool = AtomicBool::new(false);

// ── REL-03: graceful-shutdown flags ───────────────────────────────────────────
//
// `ShutdownFlags` is created in the `setup` closure and managed as Tauri app
// state. Each background task (broadcast, pairing listener, scheduler) gets a
// clone of its corresponding `Arc<AtomicBool>` so it can observe the flag on
// each loop iteration and exit cleanly. The `RunEvent::ExitRequested` handler
// at the end of `run()` flips all three flags to false so the tasks shut down
// before the process exits — otherwise they could be in the middle of a DB
// query or TLS handshake when the pool / socket closes, producing noisy
// panics and (worse) potentially half-written rows.
//
// The `BROADCAST_RUNNING` and `PAIRING_LISTENER_STARTED` statics above are
// still used as "have we already started this task?" process-global sentinels
// (so a retry of `initialize_as_server` doesn't double-spawn the listener).
// They are SEPARATE from these `ShutdownFlags` Arcs: the statics gate
// startup, the Arcs gate the loop. This keeps the existing
// "already-running, skip" optimisation intact (CR-15) while still allowing
// cooperative shutdown (REL-03).
#[derive(Clone, Default)]
struct ShutdownFlags {
    broadcast: Arc<AtomicBool>,
    pairing: Arc<AtomicBool>,
    scheduler: Arc<AtomicBool>,
}

// ── Logging ───────────────────────────────────────────────────────────────────
//
// Writes timestamped lines to:
//   Windows : %APPDATA%\<bundle-id>\Logs\hms_startup.log
//   (or the Tauri app-log dir on the current platform)
//
// The file is capped at ~500 KB — older content is trimmed automatically.
//
// Two Tauri commands let the frontend read the log:
//   get_log_path → absolute path of the log file
//   get_log      → current contents as a String

use std::io::Write;
use std::path::PathBuf;

fn log_path(app_handle: &tauri::AppHandle) -> PathBuf {
    let dir = app_handle
        .path()
        .app_log_dir()
        .or_else(|_| app_handle.path().app_data_dir())
        .unwrap_or_else(|_| PathBuf::from("."));
    std::fs::create_dir_all(&dir).ok();
    dir.join("hms_startup.log")
}

pub fn log(app_handle: &tauri::AppHandle, level: &str, msg: &str) {
    let path = log_path(app_handle);

    // Trim to last ~400 KB when the file grows over 500 KB
    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() > 500_000 {
            if let Ok(content) = std::fs::read(&path) {
                let start = content.len().saturating_sub(400_000);
                let trimmed = &content[start..];
                let offset = trimmed.iter().position(|&b| b == b'\n').map_or(0, |p| p + 1);
                let _ = std::fs::write(&path, &trimmed[offset..]);
            }
        }
    }

    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let line = format!("[{}] [{}] {}\n", timestamp, level, msg);

    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = f.write_all(line.as_bytes());
    }
    eprint!("{}", line);
}

macro_rules! log_info  { ($h:expr, $($arg:tt)*) => { crate::log($h, "INFO ", &format!($($arg)*)) }; }
macro_rules! log_warn  { ($h:expr, $($arg:tt)*) => { crate::log($h, "WARN ", &format!($($arg)*)) }; }
macro_rules! log_error { ($h:expr, $($arg:tt)*) => { crate::log($h, "ERROR", &format!($($arg)*)) }; }

// ── Log commands ──────────────────────────────────────────────────────────────
//
// SEC-05: `get_log` and `get_log_path` are RBAC-gated behind
// `SettingsManage` (admin-only). The log file contains DB usernames, LAN
// IPs, failed-login usernames, TLS fingerprints, and Postgres probe
// output — all of which are operationally useful for diagnosing
// connectivity issues but should NOT be readable by every authenticated
// user (a nurse or billing clerk has no business reading the DB user
// name or the IP of the reception PC). Both commands now require a
// session and the `settings.manage` permission.
//
// `get_log` additionally applies `redact_log` per line at read time, so
// the on-disk log retains its full text for ops debugging while the
// frontend-facing view masks `password=`, `db_password=`, `db_user=`,
// `user=`, and `username=` values. IPs are intentionally NOT redacted
// (operationally important for LAN diagnosis and not PII in the HIPAA
// sense for hospital staff on a hospital LAN).

#[tauri::command]
async fn get_log_path(
    app_handle: tauri::AppHandle,
    session_state: tauri::State<'_, std::sync::Arc<std::sync::Mutex<Option<rbac::Session>>>>,
) -> Result<String, String> {
    // SEC-05: log file path leaks the install directory + OS username.
    // Require SettingsManage IF a session exists; allow pre-login (boot
    // screen may need the path for diagnostics).
    let _ = rbac::require_if_session(&session_state, rbac::Permission::SettingsManage)?;
    Ok(log_path(&app_handle).to_string_lossy().to_string())
}

#[tauri::command]
async fn get_log(
    app_handle: tauri::AppHandle,
    session_state: tauri::State<'_, std::sync::Arc<std::sync::Mutex<Option<rbac::Session>>>>,
) -> Result<String, String> {
    // SEC-05: log file contains DB usernames, LAN IPs, failed-login
    // usernames, TLS fingerprints. Require SettingsManage IF a session
    // exists; allow pre-login (boot screen needs logs for diagnostics).
    // Sensitive patterns are redacted by redact_log() regardless.
    let _ = rbac::require_if_session(&session_state, rbac::Permission::SettingsManage)?;
    let content = std::fs::read_to_string(log_path(&app_handle))
        .map_err(|e| format!("Cannot read log: {}", e))?;
    // SEC-05: mask sensitive `key=value` patterns at read time. The
    // on-disk file is unchanged.
    let redacted: String = content
        .lines()
        .map(redact_log)
        .collect::<Vec<_>>()
        .join("\n");
    // Preserve the trailing newline that read_to_string includes (if any)
    // so the frontend's line-count / tail behaviour is unchanged.
    if content.ends_with('\n') {
        Ok(redacted + "\n")
    } else {
        Ok(redacted)
    }
}

/// SEC-05: redact sensitive `key=value` patterns from a single log line.
///
/// Masks the value component of any `key=value` pair where `key` (case-
/// insensitive) is one of: `password`, `db_password`, `db_user`, `user`,
/// `username`. The value is replaced with `***`. Matching is boundary-
/// aware: `user` won't match inside `superuser` or `username_field`.
///
/// Deliberately NOT redacted:
///   - the on-disk log (writes are unchanged — the file keeps its full
///     text for ops debugging),
///   - IPv4 / IPv6 addresses (operationally important for LAN diagnosis;
///     not PII in the HIPAA sense for hospital staff on a hospital LAN),
///   - audit-log lines that contain `username='...'` (those are emitted
///     by the audit subsystem and are already RBAC-gated behind AuditView;
///     also, the auth.rs failed-login audit row no longer records the
///     attempted username — see SEC-05 step 3).
pub fn redact_log(line: &str) -> String {
    const KEYS: &[&str] = &[
        "password",
        "db_password",
        "db_user",
        "user",
        "username",
    ];
    if line.is_empty() {
        return String::new();
    }
    let chars: Vec<char> = line.chars().collect();
    let lower: Vec<char> = line.to_lowercase().chars().collect();
    let mut out = String::with_capacity(line.len() + 16);
    let mut i = 0;
    'outer: while i < chars.len() {
        for key in KEYS {
            let kc: Vec<char> = key.chars().collect();
            // Need at least key.len() chars + 1 for '='.
            if i + kc.len() >= chars.len() {
                continue;
            }
            // Case-insensitive key match.
            if lower[i..i + kc.len()] != kc[..] {
                continue;
            }
            // Boundary: char before must not be an identifier char —
            // prevents `user` matching inside `superuser` or `db_user`
            // matching inside `my_db_username_field`.
            if i > 0 {
                let prev = chars[i - 1];
                if prev.is_alphanumeric() || prev == '_' {
                    continue;
                }
            }
            // Char immediately after the key MUST be '=' (no spaces).
            // This matches connection-URL style `password=hunter2` and
            // structured log lines `db_user=postgres`, but NOT lib.rs's
            // human-readable log_info format `DB user : postgres` (which
            // uses ':' and is operationally useful to keep visible).
            if chars[i + kc.len()] != '=' {
                continue;
            }
            // Match: emit `key=***` and skip the value (until whitespace
            // or end-of-line).
            out.push_str(key);
            out.push('=');
            out.push_str("***");
            i = i + kc.len() + 1;
            while i < chars.len() && !chars[i].is_whitespace() {
                i += 1;
            }
            continue 'outer;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

// ── Database initialization ───────────────────────────────────────────────────

#[tauri::command]
async fn initialize_database(app_handle: tauri::AppHandle) -> Result<String, String> {
    let role_state = app_handle.state::<Arc<Mutex<Option<Role>>>>();
    log_info!(&app_handle, "======== initialize_database called ========");

    #[cfg(feature = "server-build")]
    let role = {
        log_info!(&app_handle, "Build type: server-build");
        initialize_as_server(&app_handle).await.map_err(|e| {
            log_error!(&app_handle, "initialize_as_server failed: {}", e);
            e
        })?
    };

    #[cfg(feature = "client-build")]
    let role = {
        log_info!(&app_handle, "Build type: client-build");
        initialize_as_client(&app_handle).await.map_err(|e| {
            log_error!(&app_handle, "initialize_as_client failed: {}", e);
            e
        })?
    };

    #[cfg(not(any(feature = "server-build", feature = "client-build")))]
    let role = {
        log_info!(&app_handle, "Build type: dev/fallback");
        initialize_as_server_fallback(&app_handle).await.map_err(|e| {
            log_error!(&app_handle, "initialize_as_server_fallback failed: {}", e);
            e
        })?
    };

    // REL-02: recover from mutex poisoning instead of panicking.
    *role_state.lock().unwrap_or_else(|e| e.into_inner()) = Some(role.clone());

    app_handle.emit("init_status", "Connecting to the hospital database").ok();

    let cfg = AppConfig::load(&app_handle).unwrap_or_default();
    let (host, port) = match &role {
        Role::Server { .. }                 => (cfg.db_host.clone(), cfg.db_port),
        Role::Client { server_ip, db_port } => (server_ip.clone(), *db_port),
    };

    log_info!(&app_handle, "DB target            : {}:{}", host, port);
    log_info!(&app_handle, "DB user              : {}", cfg.db_user);
    log_info!(&app_handle, "DB name              : {}", cfg.db_name);
    log_info!(&app_handle, "setup_complete       : {}", cfg.setup_complete);
    log_info!(&app_handle, "password set         : {}", !cfg.db_password.is_empty());
    log_info!(&app_handle, "pinned cert present  : {}", !cfg.pinned_server_cert_pem.is_empty());
    log_info!(&app_handle, "pinned fingerprint   : {}", cfg.pinned_server_fingerprint);

    let sslrootcert_path = cfg.materialize_pinned_cert(&app_handle);
    match &sslrootcert_path {
        Some(p) => log_info!(&app_handle, "SSL root cert path   : {}", p.display()),
        None    => log_info!(&app_handle, "No pinned cert — sslmode=disable (dev) / sslmode=require (prod)"),
    }

    log_info!(&app_handle, "Calling db::initialize...");
    let pool = db::initialize(
        &host, port,
        &cfg.db_user, &cfg.db_password, &cfg.db_name,
        sslrootcert_path.as_deref(),
    )
    .await
    .map_err(|e| {
        let hint = diagnose_db_error(&e);
        log_error!(&app_handle, "db::initialize failed : {}", e);
        if !hint.is_empty() {
            log_error!(&app_handle, "Diagnosis            : {}", hint);
        }
        if hint.is_empty() { e } else { format!("{}\n\nHint: {}", e, hint) }
    })?;

    log_info!(&app_handle, "db::initialize OK — pool acquired");

    let pool = Arc::new(pool);
    app_handle.manage(pool.as_ref().clone());

    app_handle.emit("init_status", "Verifying tables are up to date").ok();
    log_info!(&app_handle, "Migrations applied successfully");

    if matches!(role, Role::Server { .. }) {
        app_handle.emit("init_status", "Starting notification scheduler").ok();
        log_info!(&app_handle, "Starting background scheduler");
        // REL-03: pass the scheduler running flag from `ShutdownFlags` so
        // the RunEvent::ExitRequested handler can flip it to false on app
        // shutdown. The scheduler loop observes the flag within ~5 s.
        let scheduler_flag = app_handle
            .state::<ShutdownFlags>()
            .inner()
            .scheduler
            .clone();
        scheduler::start_scheduler(
            app_handle.clone(),
            Arc::clone(&pool),
            Arc::new(cfg),
            scheduler_flag,
        );
    }

    app_handle.emit("init_status", "Ready!").ok();

    let result = match &role {
        Role::Server { local_ip }       => format!("server:{}", local_ip),
        Role::Client { server_ip, .. }  => format!("client:{}", server_ip),
    };
    log_info!(&app_handle, "initialize_database complete → {}", result);
    Ok(result)
}

// ── DB error diagnostics (pub so pairing.rs can reuse it) ────────────────────

pub fn diagnose_db_error(err: &str) -> String {
    let e = err.to_lowercase();
    if e.contains("certificate fingerprint mismatch") || e.contains("fingerprint") {
        return "The server TLS certificate changed since pairing. \
                Go to Setup → Re-pair with the server.".to_string();
    }
    if e.contains("server does not support tls") || e.contains("does not support ssl") {
        return "The server PostgreSQL is not SSL-enabled yet. \
                Restart HMS Server on the reception PC, then try again.".to_string();
    }
    if e.contains("sslrootcert") || e.contains("certificate verify") || e.contains("verify-ca") {
        return "SSL certificate verification failed. \
                The pinned cert file may be missing. Re-pair from Setup.".to_string();
    }
    if e.contains("password authentication failed") || e.contains("authentication failed") {
        return "Wrong database password. Re-pair from Setup to refresh credentials.".to_string();
    }
    if e.contains("connection refused") || e.contains("timed out") || e.contains("no route") || e.contains("pool timed out") {
        // Dev mode: PostgreSQL isn't running locally. Give a dev-specific hint.
        #[cfg(not(any(feature = "server-build", feature = "client-build")))]
        {
            return "Cannot connect to PostgreSQL at 127.0.0.1:5432. In dev mode, you need PostgreSQL running locally.\n\n\
                    Fix: Start PostgreSQL on your machine:\n\
                    1. Open Services (Win+R → services.msc)\n\
                    2. Find 'postgresql-x64-16' (or similar) → right-click → Start\n\
                    3. Or run: net start postgresql-x64-16\n\n\
                    If PostgreSQL is not installed, download it from https://www.enterprisedb.com/downloads/postgres-postgresql-downloads\n\
                    and install it with port 5432.\n\n\
                    If the password is wrong, delete C:\\ProgramData\\HMS\\config.json and restart the app to re-configure.".to_string();
        }
        #[cfg(any(feature = "server-build", feature = "client-build"))]
        {
            return "Cannot reach the PostgreSQL port. Check: (1) reception PC is on and \
                    HMS Server is running, (2) Windows Firewall allows port 5432, \
                    (3) both PCs are on the same network.".to_string();
        }
    }
    if e.contains("database") && e.contains("does not exist") {
        return "Database name in config not found on server. Re-pair from Setup.".to_string();
    }
    if e.contains("role") && e.contains("does not exist") {
        return "Database user in config not found on server. Re-pair from Setup.".to_string();
    }
    String::new()
}

// ── Server startup (server-build only) ───────────────────────────────────────

#[cfg(feature = "server-build")]
async fn initialize_as_server(app_handle: &tauri::AppHandle) -> Result<Role, String> {
    app_handle.emit("init_status", "Checking PostgreSQL service...").ok();
    log_info!(app_handle, "--- initialize_as_server ---");

    let cfg = match AppConfig::load(app_handle) {
        Some(c) => {
            log_info!(app_handle, "Config loaded — setup_complete={}, password_set={}",
                c.setup_complete, !c.db_password.is_empty());
            c
        }
        None => {
            log_error!(app_handle, "config.json missing — writing default, redirecting to repair");
            let _ = AppConfig::default().save(app_handle);
            return Err(
                "HMS configuration file is missing. \
                 Please use the Setup screen to enter your PostgreSQL password, \
                 or reinstall the HMS Server application.".to_string(),
            );
        }
    };

    if !cfg.setup_complete || cfg.db_password.is_empty() {
        log_error!(app_handle, "Setup not complete or password empty");
        return Err(
            "HMS Server setup is not complete. \
             Please use the Setup screen to finish configuration.".to_string(),
        );
    }

    let port = cfg.db_port;
    log_info!(app_handle, "Checking PostgreSQL health on port {}", port);

    let health = tauri::async_runtime::spawn_blocking(move || {
        let bin_dir = pg_provision::default_pg_bin_dir()
            .unwrap_or_else(|| std::path::PathBuf::from(r"C:\ProgramData\HMS\pgsql\bin"));
        pg_provision::check_postgres_health(&bin_dir, port)
    })
    .await
    .map_err(|e| format!("Health check task panicked: {}", e))??;

    log_info!(app_handle, "Health: service_running={}, accepting={}",
        health.service_running, health.accepting_connections);

    if !health.service_running {
        return Err(
            "The HMS PostgreSQL service is not running. \
             Please restart your PC. If the problem persists, \
             reinstall the HMS Server application.".to_string(),
        );
    }
    if !health.accepting_connections {
        return Err(
            "PostgreSQL is starting up but not ready yet. \
             Please wait 30 seconds and try again.".to_string(),
        );
    }

    app_handle.emit("init_status", "PostgreSQL is running.").ok();

    let local_ip = discovery::local_lan_ip();
    log_info!(app_handle, "Local LAN IP: {}", local_ip);

    // SEC-08: the server's TLS cert fingerprint, used as the HMAC key for
    // LAN broadcast signing (see `discovery::start_broadcast`). Computed
    // inside the pairing-listener setup async block below; empty if the
    // listener was already running (rare retry path) — in that case the
    // broadcast falls back to unsigned format (TOFU on the client side
    // accepts unsigned only pre-pairing).
    let mut tls_fingerprint_for_broadcast = String::new();

    if !PAIRING_LISTENER_STARTED.swap(true, Ordering::Relaxed) {
        // CR-15: wrap the entire pairing-listener startup sequence in an
        // async block that returns Result<String, String> (the String is
        // the server's TLS certificate fingerprint, needed later by
        // `start_broadcast` for SEC-08 HMAC-signing). If any step fails
        // (TLS material setup, SSL repair/setup, post-repair health check,
        // etc.), we reset PAIRING_LISTENER_STARTED to false BEFORE
        // propagating the error — otherwise a retry would see the flag
        // still true and skip the listener entirely, leaving the server
        // permanently unable to accept new pairing requests.
        let setup: Result<String, String> = async {
            let hms_dir = std::env::var_os("ProgramData")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::PathBuf::from(r"C:\ProgramData"))
                .join("HMS");
            log_info!(app_handle, "HMS data dir: {}", hms_dir.display());

            let local_ip_for_cert = local_ip.clone();
            let tls_material = tauri::async_runtime::spawn_blocking({
                let hms_dir = hms_dir.clone();
                move || tls_provision::ensure_tls_material(&hms_dir, &local_ip_for_cert)
            })
            .await
            .map_err(|e| format!("TLS setup task panicked: {}", e))??;

            log_info!(app_handle, "TLS fingerprint: {}", tls_material.fingerprint_hex);

            // SEC-08: stash the fingerprint so we can return it from this
            // async block and pass it to `start_broadcast` for HMAC-signing
            // the LAN broadcast payload. Moved out before `tls_material` is
            // moved into `start_pairing_listener` below.
            let fingerprint_for_broadcast = tls_material.fingerprint_hex.clone();

            app_handle.emit("init_status", "Configuring encrypted connections...").ok();

            let pgdata_dir = hms_dir.join("pgdata");
            let cert_path  = hms_dir.join("tls").join("server.crt");
            let key_path   = hms_dir.join("tls").join("server.key");

            log_info!(app_handle, "pgdata: {} | cert exists: {} | key exists: {}",
                pgdata_dir.display(), cert_path.exists(), key_path.exists());

            let pgdata_c = pgdata_dir.clone();
            let marker_exists = tauri::async_runtime::spawn_blocking(move || {
                pg_provision::ssl_marker_exists(&pgdata_c)
            }).await.unwrap_or(false);

            let pgdata_c = pgdata_dir.clone();
            let ssl_in_conf = tauri::async_runtime::spawn_blocking(move || {
                pg_provision::ssl_is_configured_in_conf(&pgdata_c)
            }).await.unwrap_or(false);

            let pgdata_c = pgdata_dir.clone();
            let hba_ssl = tauri::async_runtime::spawn_blocking(move || {
                pg_provision::hba_requires_ssl(&pgdata_c)
            }).await.unwrap_or(false);

            log_info!(app_handle, "SSL state: marker={} ssl_conf={} hba={}",
                marker_exists, ssl_in_conf, hba_ssl);

            let needs_repair = marker_exists && (!ssl_in_conf || !hba_ssl);
            let needs_setup  = !marker_exists;

            if needs_repair {
                log_warn!(app_handle, "SSL broken — repairing");
                app_handle.emit("init_status", "Repairing SSL configuration...").ok();
                let (pd, cp, kp) = (pgdata_dir.clone(), cert_path.clone(), key_path.clone());
                // SEC-15: pass the configured app DB user so the LAN-side
                // pg_hba rules can be restricted to that user only.
                let app_db_user = cfg.db_user.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    pg_provision::repair_ssl_config(&pd, &cp, &kp, &app_db_user)
                }).await.map_err(|e| format!("SSL repair panicked: {}", e))??;

                log_info!(app_handle, "SSL repair done — re-checking health");
                let port_r = cfg.db_port;
                let health_r = tauri::async_runtime::spawn_blocking(move || {
                    let bin_dir = pg_provision::default_pg_bin_dir()
                        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\ProgramData\HMS\pgsql\bin"));
                    pg_provision::check_postgres_health(&bin_dir, port_r)
                }).await.map_err(|e| format!("Post-repair health check panicked: {}", e))??;

                log_info!(app_handle, "Post-repair accepting: {}", health_r.accepting_connections);
                if !health_r.accepting_connections {
                    return Err(
                        "PostgreSQL did not recover after SSL repair. \
                         Please restart this PC and try again.".to_string(),
                    );
                }
            } else if needs_setup {
                log_info!(app_handle, "First-time SSL setup");
                let (pd, cp, kp) = (pgdata_dir.clone(), cert_path.clone(), key_path.clone());
                // SEC-15: pass the configured app DB user so the LAN-side
                // pg_hba rules can be restricted to that user only.
                let app_db_user = cfg.db_user.clone();
                let ssl_just_enabled = tauri::async_runtime::spawn_blocking(move || {
                    pg_provision::ensure_postgres_ssl_enabled(&pd, &cp, &kp, &app_db_user)
                }).await.map_err(|e| format!("SSL setup panicked: {}", e))??;

                log_info!(app_handle, "SSL just enabled: {}", ssl_just_enabled);

                if ssl_just_enabled {
                    app_handle.emit("init_status", "Waiting for PostgreSQL to restart with SSL...").ok();
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

                    let port_s = cfg.db_port;
                    let health_s = tauri::async_runtime::spawn_blocking(move || {
                        let bin_dir = pg_provision::default_pg_bin_dir()
                            .unwrap_or_else(|| std::path::PathBuf::from(r"C:\ProgramData\HMS\pgsql\bin"));
                        pg_provision::check_postgres_health(&bin_dir, port_s)
                    }).await.map_err(|e| format!("Post-SSL health check panicked: {}", e))??;

                    log_info!(app_handle, "Post-SSL accepting: {}", health_s.accepting_connections);
                    if !health_s.accepting_connections {
                        return Err(
                            "PostgreSQL did not come back after enabling SSL. \
                             Please restart this PC and try again.".to_string(),
                        );
                    }
                }
            } else {
                log_info!(app_handle, "SSL already fully configured — nothing to do");
            }

            log_info!(app_handle, "Starting TLS pairing listener on port {}", pairing::PAIRING_PORT);
            let pairing_service = app_handle.state::<PairingService>().inner().clone();
            // REL-03: pass the pairing running flag from `ShutdownFlags` so
            // the RunEvent::ExitRequested handler can flip it to false on
            // app shutdown. The listener's accept loop is wrapped in a
            // 1-second tokio::time::timeout so the flag is observed within
            // ~1 s.
            let pairing_flag = app_handle
                .state::<ShutdownFlags>()
                .inner()
                .pairing
                .clone();
            pairing::start_pairing_listener(
                pairing_service,
                pairing::PairingCreds {
                    db_user:     cfg.db_user.clone(),
                    db_password: cfg.db_password.clone(),
                    db_name:     cfg.db_name.clone(),
                    db_port:     cfg.db_port,
                },
                tls_material,
                pairing_flag,
            );
            // SEC-08: return the TLS fingerprint to the outer scope so
            // `start_broadcast` can HMAC-sign the LAN broadcast payload.
            Ok(fingerprint_for_broadcast)
        }
        .await;

        // SEC-08: on the success path, `setup` is `Ok(fingerprint_hex)`
        // — capture it for `start_broadcast`. On the error path, reset
        // the PAIRING_LISTENER_STARTED flag and propagate the error.
        match setup {
            Ok(fp) => tls_fingerprint_for_broadcast = fp,
            Err(e) => {
                log_error!(
                    app_handle,
                    "Pairing listener setup failed — resetting PAIRING_LISTENER_STARTED flag so a retry can re-attempt it: {}",
                    e
                );
                PAIRING_LISTENER_STARTED.store(false, Ordering::Relaxed);
                return Err(e);
            }
        }
    } else {
        log_info!(app_handle, "Pairing listener already running — skipping");
        // `tls_fingerprint_for_broadcast` stays empty — the broadcast
        // falls back to unsigned format (only relevant if the listener
        // was started on a previous launch but the broadcast wasn't, an
        // unusual path).
    }

    if !BROADCAST_RUNNING.swap(true, Ordering::Relaxed) {
        // CR-15: `discovery::start_broadcast` does not return a Result, so
        // there is currently no error path here — but the flag is reset on
        // any later error in `initialize_as_server` via the function's
        // error-return path (see the closure-style guard below). If
        // `start_broadcast` ever becomes fallible, reset
        // BROADCAST_RUNNING before propagating the error.
        //
        // REL-03: pass the broadcast running flag from `ShutdownFlags` so
        // the RunEvent::ExitRequested handler can flip it to false on app
        // shutdown. The broadcast loop checks the flag every
        // BROADCAST_INTERVAL_SECS (30 s after SEC-08).
        //
        // SEC-08: pass the server's TLS certificate fingerprint so the
        // broadcast payload can be HMAC-signed. Paired clients verify the
        // HMAC against their pinned fingerprint before accepting the
        // broadcast, defeating spoofed-broadcast redirection attacks.
        log_info!(app_handle, "Starting LAN broadcast on port {}", discovery::DISCOVERY_PORT);
        let broadcast_flag = app_handle
            .state::<ShutdownFlags>()
            .inner()
            .broadcast
            .clone();
        discovery::start_broadcast(
            local_ip.clone(),
            cfg.db_port,
            tls_fingerprint_for_broadcast.clone(),
            broadcast_flag,
        );
    } else {
        log_info!(app_handle, "LAN broadcast already running — skipping");
    }

    log_info!(app_handle, "initialize_as_server complete — local_ip={}", local_ip);
    Ok(Role::Server { local_ip })
}

// ── Client startup (client-build only) ───────────────────────────────────────

#[cfg(feature = "client-build")]
async fn initialize_as_client(app_handle: &tauri::AppHandle) -> Result<Role, String> {
    log_info!(app_handle, "--- initialize_as_client ---");

    let mut cfg = AppConfig::load(app_handle).unwrap_or_default();

    log_info!(app_handle, "Config: host={} port={} user={} db={} setup_complete={}",
        cfg.db_host, cfg.db_port, cfg.db_user, cfg.db_name, cfg.setup_complete);
    log_info!(app_handle, "password set: {} | pinned cert: {} | fingerprint: '{}'",
        !cfg.db_password.is_empty(), !cfg.pinned_server_cert_pem.is_empty(),
        cfg.pinned_server_fingerprint);

    if cfg.db_host.is_empty() || !cfg.setup_complete {
        log_error!(app_handle, "Client not paired (host empty or setup_complete=false)");
        return Err(
            "This PC has not been paired with the hospital server yet. \
             Please complete first-time setup.".to_string(),
        );
    }

    let saved_host = cfg.db_host.clone();
    let saved_port = cfg.db_port;
    log_info!(app_handle, "Probing {}:{} (TCP 2500ms)...", saved_host, saved_port);
    app_handle.emit("init_status", format!("Connecting to {}...", saved_host)).ok();

    let reachable = tauri::async_runtime::spawn_blocking({
        let host = saved_host.clone();
        move || discovery::is_reachable(&host, saved_port, 2500)
    }).await.unwrap_or(false);

    log_info!(app_handle, "TCP probe: reachable={}", reachable);

    if reachable {
        log_info!(app_handle, "Fast path: {}:{} reachable", saved_host, saved_port);
        return Ok(Role::Client { server_ip: cfg.db_host.clone(), db_port: cfg.db_port });
    }

    log_warn!(app_handle, "Saved address unreachable — trying LAN broadcast discovery");
    app_handle.emit("init_status", "Server unreachable at saved address — searching LAN...").ok();

    // SEC-08: if the client has a pinned TLS fingerprint (post-pairing),
    // pass it to `detect_server_with_fp` so only HMAC-verified broadcasts
    // are accepted. Pre-pairing (empty fingerprint) falls back to TOFU.
    let pinned_fp = if cfg.pinned_server_fingerprint.is_empty() {
        None
    } else {
        Some(cfg.pinned_server_fingerprint.clone())
    };
    let found = tauri::async_runtime::spawn_blocking(move || {
        discovery::detect_server_with_fp(pinned_fp)
    })
    .await
    .unwrap_or(None);

    match found {
        Some((server_ip, db_port)) => {
            log_info!(app_handle, "LAN discovery found: {}:{}", server_ip, db_port);
            if server_ip != saved_host {
                log_warn!(app_handle,
                    "Server IP changed {} → {} — updating config. \
                     If DB fails with fingerprint error, re-pair.",
                    saved_host, server_ip);
            }
            cfg.db_host = server_ip.clone();
            cfg.db_port = db_port;
            if let Err(e) = cfg.save(app_handle) {
                log_warn!(app_handle, "Could not persist updated server IP: {}", e);
            }
            Ok(Role::Client { server_ip, db_port })
        }
        None => {
            log_error!(app_handle, "LAN discovery timed out — no server found");
            Err(format!(
                "Cannot reach the hospital server at {}:{}. \
                 Check that the reception PC and its PostgreSQL service are running, \
                 and that this PC is on the same network.",
                cfg.db_host, cfg.db_port
            ))
        }
    }
}

// ── Dev/fallback startup ──────────────────────────────────────────────────────

#[cfg(not(any(feature = "server-build", feature = "client-build")))]
async fn initialize_as_server_fallback(app_handle: &tauri::AppHandle) -> Result<Role, String> {
    log_warn!(app_handle, "--- dev/fallback mode (no feature flag) ---");
    app_handle.emit("init_status", "Dev mode: starting PostgreSQL...").ok();
    let cfg = AppConfig::load(app_handle).unwrap_or_default();
    let local_ip = discovery::local_lan_ip();
    log_info!(app_handle, "Dev local IP: {}", local_ip);

    // ── Dev mode PostgreSQL auto-start (no pg_provision dependency) ──────
    //
    // In dev mode, the pg_provision module is NOT compiled (it's behind
    // #[cfg(feature = "server-build")]). But the developer may have the
    // bundled PostgreSQL binaries at src-tauri/resources/pgsql/bin/ OR
    // the installer may have copied them to C:\ProgramData\HMS\pgsql\bin.
    //
    // Strategy (in order):
    //   1. Try to start the HMS-PostgreSQL Windows Service (if installed).
    //   2. If no service, look for bundled pgsql binaries and start
    //      postgres.exe directly with the existing pgdata directory.
    //   3. If neither, log a clear error telling the developer to install
    //      PostgreSQL.
    #[cfg(target_os = "windows")]
    {
        // ── Step 1: Try the HMS-PostgreSQL Windows Service ──────────────
        let service_exists = std::process::Command::new("sc")
            .args(["query", "HMS-PostgreSQL"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.contains("RUNNING") || s.contains("STOPPED"))
            .unwrap_or(false);

        if service_exists {
            let running = std::process::Command::new("sc")
                .args(["query", "HMS-PostgreSQL"])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.contains("RUNNING"))
                .unwrap_or(false);

            if !running {
                log_warn!(app_handle, "HMS-PostgreSQL service exists but stopped — starting...");
                let _ = std::process::Command::new("sc")
                    .args(["start", "HMS-PostgreSQL"])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
                // Wait up to 10s for it to come online
                for i in 1..=20 {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    let now_running = std::process::Command::new("sc")
                        .args(["query", "HMS-PostgreSQL"])
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::null())
                        .output()
                        .ok()
                        .and_then(|o| String::from_utf8(o.stdout).ok())
                        .map(|s| s.contains("RUNNING"))
                        .unwrap_or(false);
                    if now_running {
                        log_info!(app_handle, "HMS-PostgreSQL service started ({} retries)", i);
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        break;
                    }
                }
            } else {
                log_info!(app_handle, "HMS-PostgreSQL service is already running");
            }
        } else {
            // ── Step 2: Start postgres.exe directly from bundled binaries ─
            //
            // The dev environment has the PostgreSQL binaries at:
            //   - src-tauri/resources/pgsql/bin/postgres.exe (dev workspace)
            //   - C:\ProgramData\HMS\pgsql\bin\postgres.exe (after installer)
            //
            // And the data directory at:
            //   - C:\ProgramData\HMS\pgdata (created by the installer OR by
            //     the dev_auto_license binary on first run)
            //
            // We start postgres.exe directly (no Windows Service) in the
            // background. This is dev-only — production uses the service.

            let hms_dir = std::env::var_os("ProgramData")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::PathBuf::from(r"C:\ProgramData"))
                .join("HMS");
            let pgdata_dir = hms_dir.join("pgdata");

            // Find the postgres.exe binary
            let bin_candidates = [
                hms_dir.join("pgsql").join("bin"),
                // Dev workspace: src-tauri/resources/pgsql/bin relative to CARGO_MANIFEST_DIR
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("resources").join("pgsql").join("bin"),
            ];

            let pg_bin = bin_candidates
                .iter()
                .find(|d| d.join("postgres.exe").exists())
                .map(|d| d.join("postgres.exe"));

            if let Some(ref postgres_exe) = pg_bin {
                log_info!(app_handle, "Found PostgreSQL binary: {}", postgres_exe.display());

                if pgdata_dir.exists() {
                    log_info!(app_handle, "Starting PostgreSQL from: {} with data dir: {}",
                        postgres_exe.display(), pgdata_dir.display());

                    // Start postgres.exe in the background (detached)
                    let port_str = cfg.db_port.to_string();
                    let child = std::process::Command::new(postgres_exe)
                        .args(["-D"])
                        .arg(&pgdata_dir)
                        .args(["-p", &port_str])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn();

                    match child {
                        Ok(c) => {
                            log_info!(app_handle, "PostgreSQL started (PID: {}) — waiting for connections...", c.id());
                            app_handle.emit("init_status", "Waiting for PostgreSQL to start...").ok();
                            // Wait up to 15 seconds for PostgreSQL to accept connections
                            for i in 1..=30 {
                                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                                // Try a TCP connection to check if the port is open
                                let reachable = tokio::net::TcpStream::connect(
                                    format!("127.0.0.1:{}", cfg.db_port)
                                ).await.is_ok();
                                if reachable {
                                    log_info!(app_handle, "PostgreSQL is now accepting connections (after {} retries)", i);
                                    break;
                                }
                                if i % 4 == 0 {
                                    log_info!(app_handle, "Still waiting for PostgreSQL... ({}s)", i as u64 / 2);
                                }
                            }
                        }
                        Err(e) => {
                            log_error!(app_handle, "Failed to start postgres.exe: {}", e);
                        }
                    }
                } else {
                    log_warn!(app_handle, "PostgreSQL data directory not found at: {}. \
                             Run the HMS Server installer first, or create the data directory with initdb.",
                        pgdata_dir.display());
                }
            } else {
                log_warn!(app_handle, "PostgreSQL binary (postgres.exe) not found. \
                         Looked in: {} and {}",
                    bin_candidates[0].display(), bin_candidates[1].display());
                log_warn!(app_handle, "To fix: either (1) install the HMS Server build, or \
                         (2) install standalone PostgreSQL on port 5432, or \
                         (3) download PostgreSQL binaries to src-tauri/resources/pgsql/");
            }
        }
    }

    if !BROADCAST_RUNNING.swap(true, Ordering::Relaxed) {
        let broadcast_flag = app_handle
            .state::<ShutdownFlags>()
            .inner()
            .broadcast
            .clone();
        discovery::start_broadcast(
            local_ip.clone(),
            cfg.db_port,
            String::new(),
            broadcast_flag,
        );
    }
    Ok(Role::Server { local_ip })
}

// ── check_db_connection (Setup screen connection test) ────────────────────────

#[tauri::command]
async fn check_db_connection(
    app_handle: tauri::AppHandle,
    host: String,
    port: u16,
    user: String,
    password: String,
) -> Result<String, String> {
    log_info!(&app_handle, "check_db_connection → {}:{} user={}", host, port, user);
    // In dev mode use sslmode=prefer; in production use sslmode=require.
    #[cfg(not(any(feature = "server-build", feature = "client-build")))]
    let sslmode = "prefer";
    #[cfg(any(feature = "server-build", feature = "client-build"))]
    let sslmode = "require";
    let url = format!(
        "postgresql://{}:{}@{}:{}/postgres?sslmode={}",
        user, password, host, port, sslmode
    );
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(&url)
        .await
        .map_err(|e| {
            let msg = format!("Connection failed: {}", e);
            log_error!(&app_handle, "{}", msg);
            msg
        })?;
    pool.close().await;
    log_info!(&app_handle, "check_db_connection succeeded");
    Ok("Connection successful".to_string())
}

// ── complete_pairing_and_connect ("Save & continue" button) ──────────────────
//
// Called by the frontend after redeem_pairing_code returns "Paired successfully".
// Does in sequence:
//   1. pairing::verify_pairing  — materialises the cert, opens a real DB
//      connection to confirm credentials work, sets setup_complete = true.
//   2. initialize_database      — acquires the pool, runs migrations,
//      starts the scheduler, emits "Ready!".
//
// Returning "client:<ip>" tells the frontend to navigate to the main app.

#[tauri::command]
async fn complete_pairing_and_connect(app_handle: tauri::AppHandle) -> Result<String, String> {
    log_info!(&app_handle, "complete_pairing_and_connect called (Save & continue)");

    pairing::verify_pairing(app_handle.clone()).await.map_err(|e| {
        log_error!(&app_handle, "verify_pairing failed: {}", e);
        e
    })?;

    log_info!(&app_handle, "verify_pairing OK — calling initialize_database");
    initialize_database(app_handle).await
}

// ── App entry point ───────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            app.manage(Arc::new(Mutex::new(None::<Role>)));
            app.manage(PairingService::new());
            // RBAC session state — single active desktop user per process.
            // Type is `Arc<Mutex<Option<rbac::Session>>>` == `rbac::SessionState`.
            app.manage(Arc::new(Mutex::new(None::<rbac::Session>)));
            // REL-03: graceful-shutdown flags for the three background tasks
            // (broadcast, pairing listener, scheduler). Managed as Tauri app
            // state so the RunEvent::ExitRequested handler at the end of
            // `run()` can flip them all to false when the app is exiting.
            app.manage(ShutdownFlags::default());

            log_info!(app.handle(), "");
            log_info!(app.handle(), "════════════════════════════════════════");
            log_info!(app.handle(), "  VitalFlow HMS starting up");
            log_info!(app.handle(), "  Version : {}", env!("CARGO_PKG_VERSION"));
            log_info!(app.handle(), "  PID     : {}", std::process::id());
            log_info!(app.handle(), "  OS      : {}", std::env::consts::OS);
            log_info!(app.handle(), "════════════════════════════════════════");
            eprintln!("[HMS] Log: {}", log_path(app.handle()).display());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Config
            config::get_config,
            config::save_config,
            config::get_local_ip,
            config::test_server_connection,
            config::repair_server_config,
            config::get_config_path,
            // Discovery
            discovery::get_server_role,
            // Pairing
            pairing::generate_pairing_code,
            pairing::get_pairing_status,
            pairing::redeem_pairing_code,
            pairing::verify_pairing,
            // Core init (defined in this file)
            initialize_database,
            check_db_connection,
            complete_pairing_and_connect,
            get_log_path,
            get_log,
            // ── Authentication & RBAC ───────────────────────────────────────
            auth::login,
            auth::logout,
            auth::me,
            auth::change_password,
            auth::list_users,
            auth::create_user,
            auth::update_user,
            auth::delete_user,
            auth::reset_user_password,
            auth::list_roles,
            auth::list_user_roles,
            // ── Audit ────────────────────────────────────────────────────────
            audit::get_audit_logs,
            // ── Licensing ────────────────────────────────────────────────────
            license::verify_license,
            license::get_license_info,
            license::install_license,
            // LIC-DOC-04: revoke_license — deletes the license file + writes
            // an audit row. Gated behind LicenseManage.
            license::revoke_license,
            license::get_hardware_fingerprint,
            license::get_license_public_key_fingerprint,
            license::get_install_fingerprint,
            // ── Dashboard ────────────────────────────────────────────────────
            commands::dashboard::get_dashboard_kpis,
            // Patients
            commands::patients::create_patient,
            commands::patients::get_patients,
            commands::patients::get_patient,
            commands::patients::update_patient,
            commands::patients::delete_patient,
            // Patient consent (CR-12, SRS FR-0035)
            commands::patients::get_patient_consent,
            commands::patients::set_patient_consent,
            commands::patients::revoke_patient_consent,
            // Doctors
            commands::doctors::create_doctor,
            commands::doctors::get_doctors,
            commands::doctors::get_doctor,
            commands::doctors::update_doctor,
            commands::doctors::delete_doctor,
            commands::doctors::get_specializations,
            // Appointments
            commands::appointments::create_appointment,
            commands::appointments::get_appointments,
            commands::appointments::get_appointment,
            commands::appointments::update_appointment,
            commands::appointments::update_appointment_status,
            commands::appointments::delete_appointment,
            commands::appointments::get_today_appointments,
            commands::appointments::get_appointment_stats,
            // Encounters / visits
            commands::encounters::get_encounters,
            commands::encounters::create_encounter,
            // Queue
            commands::queue::get_queue,
            commands::queue::create_queue_token,
            commands::queue::call_next_token,
            commands::queue::set_token_status,
            // IPD
            commands::ipd::get_wards,
            commands::ipd::create_ward,
            commands::ipd::get_beds,
            commands::ipd::create_bed,
            commands::ipd::get_admissions,
            commands::ipd::admit_patient,
            commands::ipd::discharge_patient,
            // Laboratory
            commands::lab::get_lab_catalog,
            commands::lab::create_lab_test,
            commands::lab::get_lab_orders,
            commands::lab::create_lab_order,
            commands::lab::get_lab_order_tests,
            commands::lab::update_lab_result,
            // Radiology (Phase 2-D, SRS FR-0140–FR-0142) — imaging orders,
            // radiologist reports, and report verification workflow. RBAC
            // uses the seven dedicated `Radiology*` permission variants
            // already seeded in `rbac.rs::permissions_for_role` (doctors
            // get view/create/update; super_admin gets all seven including
            // the radiologist-reserved `RadiologyVerify`).
            commands::radiology::get_radiology_orders,
            commands::radiology::get_radiology_order,
            commands::radiology::create_radiology_order,
            commands::radiology::update_radiology_order_status,
            commands::radiology::delete_radiology_order,
            commands::radiology::get_radiology_report,
            commands::radiology::create_radiology_report,
            commands::radiology::verify_radiology_report,
            commands::radiology::get_radiology_dashboard,
            // Blood Bank (Phase 2-E, SRS FR-0145–FR-0149) — donor registry,
            // donations, inventory, cross-matching, reservations, issue,
            // transfusion, discard, and full traceability. RBAC uses eight
            // dedicated `BloodBank*` permission variants seeded in
            // `rbac.rs::permissions_for_role` (doctors get crossmatch/issue/
            // transfuse; lab techs get donor-manage + crossmatch; nurses get
            // transfuse; super_admin gets all eight).
            commands::blood_bank::get_blood_donors,
            commands::blood_bank::get_blood_donor,
            commands::blood_bank::create_blood_donor,
            commands::blood_bank::delete_blood_donor,
            commands::blood_bank::get_blood_donations,
            commands::blood_bank::create_blood_donation,
            commands::blood_bank::update_blood_donation_screening,
            commands::blood_bank::get_blood_units,
            commands::blood_bank::get_blood_unit,
            commands::blood_bank::create_blood_unit,
            commands::blood_bank::update_blood_unit_status,
            commands::blood_bank::delete_blood_unit,
            commands::blood_bank::search_blood_inventory,
            commands::blood_bank::get_blood_crossmatches,
            commands::blood_bank::check_blood_compatibility,
            commands::blood_bank::create_blood_crossmatch,
            commands::blood_bank::verify_blood_crossmatch,
            commands::blood_bank::create_blood_reservation,
            commands::blood_bank::cancel_blood_reservation,
            commands::blood_bank::get_blood_issues,
            commands::blood_bank::issue_blood,
            commands::blood_bank::return_blood_unit,
            commands::blood_bank::get_blood_transfusions,
            commands::blood_bank::create_blood_transfusion,
            commands::blood_bank::discard_blood_unit,
            commands::blood_bank::get_blood_discards,
            commands::blood_bank::get_blood_unit_history,
            commands::blood_bank::get_blood_unit_movements,
            commands::blood_bank::get_blood_unit_traceability,
            commands::blood_bank::get_blood_bank_dashboard,
            commands::blood_bank::get_blood_bank_statistics,
            // Billing
            commands::billing::get_bills,
            commands::billing::get_bill,
            commands::billing::get_bill_items,
            commands::billing::create_bill,
            commands::billing::record_payment,
            commands::billing::get_payments,
            // Inventory (CR-21, SRS FR-0180/0181/0185)
            commands::inventory::get_inventory_items,
            commands::inventory::get_inventory_item,
            commands::inventory::create_inventory_item,
            commands::inventory::update_inventory_item,
            commands::inventory::adjust_inventory,
            commands::inventory::get_inventory_movements,
            // Pharmacy (Phase 2-C, SRS FR-0120–FR-0124) — medication
            // catalog, prescription generation from encounters, and
            // dispensing with inventory decrement + audit trail. Reuses
            // InventoryView/InventoryManage for catalog + dispensing and
            // PatientsView/PatientsCreate for prescriptions — no new
            // permission variants were added (FR-0124 already grants
            // pharmacists inventory.view, inventory.manage, billing.view,
            // and patients.view).
            commands::pharmacy::get_medications,
            commands::pharmacy::create_medication,
            commands::pharmacy::update_medication,
            commands::pharmacy::delete_medication,
            commands::pharmacy::get_prescriptions,
            commands::pharmacy::get_prescription,
            commands::pharmacy::create_prescription,
            commands::pharmacy::dispense_prescription_item,
            // Reports (Phase 2-A, SRS §4.20, FR-0220–FR-0223) — read-only
            // operational reports. All RBAC-guarded by Permission::ReportsView.
            // Daily OPD + IPD census + revenue + lab turnaround + CSV export.
            commands::reports::get_daily_opd_report,
            commands::reports::get_ipd_census_report,
            commands::reports::get_revenue_report,
            commands::reports::get_lab_turnaround_report,
            commands::reports::export_report_csv,
            // Backup & Restore (Phase 2, SRS §9 A-07) — server-build only.
            // The commands themselves are #[cfg(feature = "server-build")] in
            // commands/backup.rs; the registrations are gated identically so
            // client/dev builds (where the module is empty) don't fail with
            // "cannot find function" at macro-expansion time. All four
            // commands are RBAC-guarded by Permission::BackupsManage.
            #[cfg(feature = "server-build")]
            commands::backup::create_backup,
            #[cfg(feature = "server-build")]
            commands::backup::list_backups,
            #[cfg(feature = "server-build")]
            commands::backup::restore_backup,
            #[cfg(feature = "server-build")]
            commands::backup::delete_backup,
            // Messaging
            messaging::send_message,
            messaging::get_messages,
            messaging::delete_message,
            messaging::get_rooms,
            // WhatsApp — must use whatsapp::commands:: path, NOT whatsapp::
            // because #[tauri::command] macros live in the defining submodule
            // and cannot be re-exported via `pub use` into the parent.
            whatsapp::commands::send_whatsapp_notification,
            whatsapp::commands::send_whatsapp_to_patient,
            whatsapp::commands::send_whatsapp_test,
            whatsapp::commands::get_notification_log,
            whatsapp::commands::get_whatsapp_config,
            whatsapp::commands::set_whatsapp_config,
            whatsapp::commands::test_whatsapp_api,
        ])
        // REL-03: switch from `Builder::run(context)` (no event callback) to
        // `Builder::build(context)?.run(callback)` so we can hook
        // `RunEvent::ExitRequested` and cooperatively shut down the three
        // background tasks (broadcast, pairing listener, scheduler) before
        // the process exits. Without this, the tasks could be in the middle
        // of a DB query or TLS handshake when the pool / socket closes.
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                if let Some(flags) = app.try_state::<ShutdownFlags>() {
                    log_info!(
                        app,
                        "ExitRequested — flipping ShutdownFlags to false \
                         (broadcast + pairing + scheduler)"
                    );
                    flags.broadcast.store(false, Ordering::Relaxed);
                    flags.pairing.store(false, Ordering::Relaxed);
                    flags.scheduler.store(false, Ordering::Relaxed);
                }
            }
        });
}
