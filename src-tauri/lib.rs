mod config;
mod db;
mod discovery;
mod messaging;
mod models;
mod pairing;
mod scheduler;
mod whatsapp;
#[cfg(feature = "server-build")]
mod pg_provision;
mod tls_provision;
mod commands {
    pub mod appointments;
    pub mod doctors;
    pub mod patients;
}

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use config::{get_config, get_local_ip, save_config, test_server_connection, AppConfig};
use discovery::{get_server_role, Role};
use messaging::{delete_message, get_messages, get_rooms, send_message};
use pairing::{generate_pairing_code, get_pairing_status, redeem_pairing_code, PairingService};
use whatsapp::{get_notification_log, send_whatsapp_notification};
use commands::appointments::*;
use commands::doctors::*;
use commands::patients::*;

use tauri::Manager;
use tauri::Emitter;

/// Flag that keeps the discovery broadcast loop alive for the server build.
static BROADCAST_RUNNING: AtomicBool = AtomicBool::new(false);
/// Guards against starting the pairing TCP listener more than once.
static PAIRING_LISTENER_STARTED: AtomicBool = AtomicBool::new(false);

// ── Database initialization command ──────────────────────────────────────────

#[tauri::command]
async fn initialize_database(app_handle: tauri::AppHandle) -> Result<String, String> {
    let role_state = app_handle.state::<Arc<Mutex<Option<Role>>>>();

    #[cfg(feature = "server-build")]
    let role = initialize_as_server(&app_handle).await?;

    #[cfg(feature = "client-build")]
    let role = initialize_as_client(&app_handle).await?;

    // Fallback for plain `cargo build` with neither feature set (dev/testing):
    // behaves like the server build but skips TLS/provisioning.
    #[cfg(not(any(feature = "server-build", feature = "client-build")))]
    let role = initialize_as_server_fallback(&app_handle).await?;

    *role_state.lock().unwrap() = Some(role.clone());

    // ── Connect & migrate ────────────────────────────────────────────────
    app_handle.emit("init_status", "Connecting to database...").ok();

    let cfg = AppConfig::load(&app_handle).unwrap_or_default();
    let (host, port) = match &role {
        Role::Server { .. } => (cfg.db_host.clone(), cfg.db_port),
        Role::Client { server_ip, db_port } => (server_ip.clone(), *db_port),
    };

    // Client builds have a pinned cert and must verify against it.
    // Server build connects to itself over loopback — no cert to pin,
    // but sslmode=require (enforced inside db::connect_root) still
    // encrypts the channel to satisfy pg_hba.conf's hostssl rules.
    let sslrootcert_path = cfg.materialize_pinned_cert(&app_handle);

    let pool = db::initialize(
        &host,
        port,
        &cfg.db_user,
        &cfg.db_password,
        &cfg.db_name,
        sslrootcert_path.as_deref(),
    )
    .await?;
    let pool = Arc::new(pool);
    app_handle.manage(pool.as_ref().clone());

    // ── Scheduler (server only) ──────────────────────────────────────────
    if matches!(role, Role::Server { .. }) {
        app_handle.emit("init_status", "Starting notification scheduler...").ok();
        scheduler::start_scheduler(app_handle.clone(), Arc::clone(&pool), Arc::new(cfg));
    }

    app_handle.emit("init_status", "Ready!").ok();

    let mode_str = match &role {
        Role::Server { local_ip } => format!("server:{}", local_ip),
        Role::Client { server_ip, .. } => format!("client:{}", server_ip),
    };
    Ok(mode_str)
}

/// Server build startup sequence:
///
///   1. Verify the HMS-PostgreSQL Windows Service is running (the NSIS
///      installer hook set it up; we never provision here).
///   2. Generate (or load) the TLS certificate for the pairing listener
///      and for PostgreSQL's own SSL connection.
///   3. On the VERY FIRST launch after install: enable PostgreSQL SSL.
///      This requires connecting WITHOUT SSL first (the bootstrap connect),
///      because pg_hba.conf still uses plain `host` rules at that point —
///      the `hostssl` upgrade and the ssl=on config change happen together
///      in ensure_postgres_ssl_enabled, which then restarts the service.
///   4. On every subsequent launch: PostgreSQL already has SSL on; the
///      normal sslmode=require path in db::connect_root is used.
///   5. Start the pairing TCP listener (TLS-wrapped) for client setup.
#[cfg(feature = "server-build")]
async fn initialize_as_server(app_handle: &tauri::AppHandle) -> Result<Role, String> {
    app_handle.emit("init_status", "Checking PostgreSQL service...").ok();

    let cfg = AppConfig::load(app_handle).unwrap_or_default();

    if !cfg.setup_complete || cfg.db_password.is_empty() {
        return Err(
            "PostgreSQL has not been set up on this PC yet. Please run the HMS Server installer (not just this app) to complete setup.".to_string(),
        );
    }

    let port = cfg.db_port;
    let health = tauri::async_runtime::spawn_blocking(move || {
        let bin_dir = pg_provision::default_pg_bin_dir()
            .unwrap_or_else(|| std::path::PathBuf::from(r"C:\ProgramData\HMS\pgsql\bin"));
        pg_provision::check_postgres_health(&bin_dir, port)
    })
    .await
    .map_err(|e| format!("Health check task panicked: {}", e))??;

    if !health.service_running {
        return Err(
            "The PostgreSQL Windows Service (HMS-PostgreSQL) is not running. Try restarting this PC, or reinstall the HMS Server application to repair it.".to_string(),
        );
    }
    if !health.accepting_connections {
        return Err(
            "PostgreSQL is running but not responding yet. Please wait a moment and try again.".to_string(),
        );
    }

    app_handle.emit("init_status", "PostgreSQL service is running.").ok();

    let local_ip = discovery::local_lan_ip();
    if !BROADCAST_RUNNING.swap(true, Ordering::Relaxed) {
        discovery::start_broadcast(local_ip.clone(), cfg.db_port, Arc::new(AtomicBool::new(true)));
    }

    if !PAIRING_LISTENER_STARTED.swap(true, Ordering::Relaxed) {
        let hms_dir = std::env::var_os("ProgramData")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from(r"C:\ProgramData"))
            .join("HMS");
        let local_ip_for_cert = local_ip.clone();

        // Generate or load the TLS certificate. This is idempotent —
        // on every launch after the first it just reads the existing files.
        app_handle.emit("init_status", "Preparing encrypted connections...").ok();
        let tls_material = tauri::async_runtime::spawn_blocking({
            let hms_dir = hms_dir.clone();
            move || tls_provision::ensure_tls_material(&hms_dir, &local_ip_for_cert)
        })
        .await
        .map_err(|e| format!("TLS setup task panicked: {}", e))??;

        // ── SSL-enablement bootstrap (first launch only) ─────────────────
        //
        // Check whether this is the first launch (no marker file yet).
        // If so, we connect to PostgreSQL WITHOUT SSL to run the config
        // change, then restart the service. If the marker already exists,
        // this block is skipped entirely and the normal ssl path is used.
        let pgdata_dir = hms_dir.join("pgdata");
        let ssl_already_on = pg_provision::is_ssl_already_enabled(&pgdata_dir);

        if !ssl_already_on {
            app_handle.emit("init_status", "Enabling encrypted database connections (first-time setup)...").ok();

            // This connect uses sslmode=disable — the ONLY time we ever do
            // that. pg_hba.conf still has plain `host` rules here (installed
            // by hooks.nsh), so this connect will succeed. The function
            // below rewrites pg_hba.conf to hostssl and adds ssl=on to
            // postgresql.conf, then restarts the service.
            let bootstrap_pool = db::connect_root_no_ssl(
                "127.0.0.1",
                cfg.db_port,
                &cfg.db_user,
                &cfg.db_password,
            )
            .await
            .map_err(|e| format!(
                "Could not connect to PostgreSQL for SSL setup: {}. \
                 Ensure the HMS Server installer completed successfully.", e
            ))?;
            // We only needed to verify we can reach Postgres before enabling
            // SSL. Close immediately — the SSL provisioning is done via
            // file writes + service restart, not SQL commands.
            bootstrap_pool.close().await;

            let cert_path = hms_dir.join("tls").join("server.crt");
            let key_path = hms_dir.join("tls").join("server.key");
            let ssl_just_enabled = tauri::async_runtime::spawn_blocking(move || {
                pg_provision::ensure_postgres_ssl_enabled(&pgdata_dir, &cert_path, &key_path)
            })
            .await
            .map_err(|e| format!("SSL setup task panicked: {}", e))??;

            if ssl_just_enabled {
                // Service was restarted — wait for it to come back up.
                app_handle.emit("init_status", "Waiting for PostgreSQL to restart...").ok();
                let port = cfg.db_port;
                let health_after = tauri::async_runtime::spawn_blocking(move || {
                    let bin_dir = pg_provision::default_pg_bin_dir()
                        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\ProgramData\HMS\pgsql\bin"));
                    pg_provision::check_postgres_health(&bin_dir, port)
                })
                .await
                .map_err(|e| format!("Post-SSL health check panicked: {}", e))??;

                if !health_after.accepting_connections {
                    return Err(
                        "PostgreSQL did not come back up after enabling encrypted connections. \
                         Please restart this PC and launch HMS again.".to_string()
                    );
                }
                app_handle.emit("init_status", "Encrypted connections enabled.").ok();
            }
        }
        // ── End SSL-enablement bootstrap ─────────────────────────────────

        // Start the pairing listener (TLS-wrapped) so client PCs can
        // securely receive DB credentials using a short-lived code.
        let pairing_service = app_handle.state::<PairingService>().inner().clone();
        pairing::start_pairing_listener(
            pairing_service,
            pairing::PairingCreds {
                db_user: cfg.db_user.clone(),
                db_password: cfg.db_password.clone(),
                db_name: cfg.db_name.clone(),
                db_port: cfg.db_port,
            },
            tls_material,
        );
    }

    Ok(Role::Server { local_ip })
}

/// Client build: try the saved server IP first (fast path). If unreachable,
/// fall back to a brief LAN broadcast listen to self-heal after the
/// reception PC's IP changes, then save the recovered IP for next time.
#[cfg(feature = "client-build")]
async fn initialize_as_client(app_handle: &tauri::AppHandle) -> Result<Role, String> {
    let mut cfg = AppConfig::load(app_handle).unwrap_or_default();

    if cfg.db_host.is_empty() || !cfg.setup_complete {
        return Err(
            "No server address configured yet. Please complete first-time setup.".to_string(),
        );
    }

    app_handle.emit("init_status", format!("Connecting to {}...", cfg.db_host)).ok();

    let saved_host = cfg.db_host.clone();
    let saved_port = cfg.db_port;
    let reachable = tauri::async_runtime::spawn_blocking(move || {
        discovery::is_reachable(&saved_host, saved_port, 2500)
    })
    .await
    .unwrap_or(false);

    if reachable {
        return Ok(Role::Client {
            server_ip: cfg.db_host.clone(),
            db_port: cfg.db_port,
        });
    }

    // Saved IP didn't respond — try to self-heal via broadcast discovery.
    app_handle
        .emit("init_status", "Server unreachable at saved address — searching LAN...")
        .ok();

    let found = tauri::async_runtime::spawn_blocking(discovery::detect_server)
        .await
        .unwrap_or(None);

    match found {
        Some((server_ip, db_port)) => {
            cfg.db_host = server_ip.clone();
            cfg.db_port = db_port;
            cfg.save(app_handle).ok();
            Ok(Role::Client { server_ip, db_port })
        }
        None => Err(format!(
            "Cannot reach the hospital server at {}:{}. Check that the reception PC and its PostgreSQL service are running, and that this PC is on the same network.",
            cfg.db_host, cfg.db_port
        )),
    }
}

/// Used only for plain dev builds without a feature flag selected.
#[cfg(not(any(feature = "server-build", feature = "client-build")))]
async fn initialize_as_server_fallback(app_handle: &tauri::AppHandle) -> Result<Role, String> {
    app_handle
        .emit("init_status", "Dev mode: assuming local PostgreSQL is already running...")
        .ok();
    let cfg = AppConfig::load(app_handle).unwrap_or_default();
    let local_ip = discovery::local_lan_ip();
    if !BROADCAST_RUNNING.swap(true, Ordering::Relaxed) {
        discovery::start_broadcast(local_ip.clone(), cfg.db_port, Arc::new(AtomicBool::new(true)));
    }
    Ok(Role::Server { local_ip })
}

#[tauri::command]
async fn check_db_connection(
    host: String,
    port: u16,
    user: String,
    password: String,
) -> Result<String, String> {
    // Diagnostic only — uses sslmode=require since by the time this command
    // is called from the UI, the server will already have SSL enabled.
    let url = format!(
        "postgresql://{}:{}@{}:{}/postgres?sslmode=require",
        user, password, host, port
    );
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(&url)
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;
    pool.close().await;
    Ok("Connection successful".to_string())
}

// ── App entry point ───────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            app.manage(Arc::new(Mutex::new(None::<Role>)));
            app.manage(PairingService::new());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Config
            get_config,
            save_config,
            get_local_ip,
            test_server_connection,
            // Discovery
            get_server_role,
            // Pairing (secure credential exchange for client setup)
            generate_pairing_code,
            get_pairing_status,
            redeem_pairing_code,
            // Database
            initialize_database,
            check_db_connection,
            // Patients
            create_patient,
            get_patients,
            get_patient,
            update_patient,
            delete_patient,
            // Doctors
            create_doctor,
            get_doctors,
            get_doctor,
            update_doctor,
            delete_doctor,
            get_specializations,
            // Appointments
            create_appointment,
            get_appointments,
            get_appointment,
            update_appointment,
            update_appointment_status,
            delete_appointment,
            get_today_appointments,
            get_appointment_stats,
            // Messaging
            send_message,
            get_messages,
            delete_message,
            get_rooms,
            // WhatsApp
            send_whatsapp_notification,
            get_notification_log,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
