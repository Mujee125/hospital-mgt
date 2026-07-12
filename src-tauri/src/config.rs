use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

/// Application configuration persisted to config.json.
///
/// SECURITY: `db_password` is tagged `#[serde(skip_serializing)]` so that the
/// `get_config` Tauri command — which returns this struct to the frontend —
/// never exposes the production PostgreSQL password to the webview. The
/// password is still deserialised from disk (so the backend can use it), it
/// just is never re-serialised over IPC. (Per Security Matrix A.5.15 /
/// A.8.3 / SRS NFR-15 — CR-4.)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub mode: String,
    pub db_host: String,
    pub db_port: u16,
    pub db_user: String,
    #[serde(skip_serializing)]
    pub db_password: String,
    pub db_name: String,
    pub clinic_name: String,
    pub doctors_whatsapp_group: String,
    #[serde(default)]
    pub setup_complete: bool,
    #[serde(default)]
    pub pinned_server_cert_pem: String,
    #[serde(default)]
    pub pinned_server_fingerprint: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            #[cfg(feature = "server-build")]
            mode: "server".to_string(),
            #[cfg(feature = "client-build")]
            mode: "client".to_string(),
            #[cfg(not(any(feature = "server-build", feature = "client-build")))]
            mode: "server".to_string(),
            db_host: "127.0.0.1".to_string(),
            db_port: 5432,
            db_user: "postgres".to_string(),
            db_password: String::new(),
            db_name: "hospital_db".to_string(),
            clinic_name: "VitalFlow Clinic".to_string(),
            doctors_whatsapp_group: String::new(),
            setup_complete: false,
            pinned_server_cert_pem: String::new(),
            pinned_server_fingerprint: String::new(),
        }
    }
}

impl AppConfig {
    fn machine_config_path() -> Option<PathBuf> {
        let program_data = std::env::var_os("ProgramData")?;
        Some(PathBuf::from(program_data).join("HMS").join("config.json"))
    }

    fn user_config_path(app_handle: &tauri::AppHandle) -> PathBuf {
        let dir = app_handle
            .path()
            .app_config_dir()
            .expect("Failed to get app config dir");
        fs::create_dir_all(&dir).ok();
        dir.join("config.json")
    }

    pub fn config_path(app_handle: &tauri::AppHandle) -> PathBuf {
        if let Some(machine_path) = Self::machine_config_path() {
            if machine_path.exists() {
                return machine_path;
            }
        }
        Self::user_config_path(app_handle)
    }

    pub fn load(app_handle: &tauri::AppHandle) -> Option<Self> {
        let content = fs::read_to_string(Self::config_path(app_handle)).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Persist the config to disk atomically and ACL-harden it on Windows.
    ///
    /// Atomicity: write to `config.json.tmp` then `rename` → `config.json`. A
    /// crash mid-write leaves the temp file, not a truncated config. (REL-10.)
    ///
    /// ACL hardening (CR-5, Windows only): config.json contains the plaintext
    /// DB password, so we restrict the file ACL to `SYSTEM` + `Administrators`
    /// only — the same treatment `pg_provision.rs` applies to the Postgres
    /// private key. Any local user can no longer read the credentials. (Full
    /// DPAPI encryption of the password field is tracked as a Batch 3 item.)
    pub fn save(&self, app_handle: &tauri::AppHandle) -> Result<(), String> {
        let content = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let path = Self::config_path(app_handle);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        // Atomic write: temp file + rename.
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, &content).map_err(|e| format!("Write config tmp: {}", e))?;
        fs::rename(&tmp, &path).map_err(|e| {
            // Best-effort cleanup of the temp file on rename failure.
            let _ = fs::remove_file(&tmp);
            format!("Rename config: {}", e)
        })?;

        // Windows: ACL-harden the file so only SYSTEM + Administrators can read it.
        #[cfg(target_os = "windows")]
        {
            // icacls: reset inheritance, grant SYSTEM + Administrators full, remove everyone else.
            let icacls = std::process::Command::new("icacls")
                .arg(path.as_os_str())
                .args(["/inheritance:r"])
                .args(["/grant:r", "SYSTEM:F"])
                .args(["/grant:r", "Administrators:F"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            if let Err(e) = icacls {
                eprintln!("[HMS CONFIG] Warning: could not ACL-harden config.json ({}). \
                           The file may be readable by other local users.", e);
            }
        }

        Ok(())
    }

    pub fn materialize_pinned_cert(&self, app_handle: &tauri::AppHandle) -> Option<PathBuf> {
        if self.pinned_server_cert_pem.is_empty() {
            return None;
        }
        let dir = app_handle.path().app_data_dir().ok()?;
        fs::create_dir_all(&dir).ok()?;
        let path = dir.join("pinned-server-cert.pem");
        fs::write(&path, &self.pinned_server_cert_pem).ok()?;
        Some(path)
    }
}

// ── Tauri commands ───────────────────────────────────────────────────────────
//
// RBAC policy (CR-4, per SRS NFR-15 / Security Matrix A.5.15):
//   - `get_config`: returns a REDACTED view (db_password is skip_serializing),
//     so it is safe to call pre-login during first-run setup AND the boot
//     flow. Once a user IS logged in, only SettingsManage holders may call
//     it. Pre-login (no session) access is allowed so the boot screen can
//     read the config to determine server/client mode.
//   - `save_config` / `repair_server_config` / `clear_config`: require
//     SettingsManage once setup_complete is true AND a session exists.
//     Pre-login (no session) access is allowed for first-run Setup.
//
// The `require_if_session` helper returns Ok(None) when there's no session
// (pre-login boot/setup) and Ok(Some(session)) when authorized. This fixes
// the boot-flow regression where `get_config` was called during startup
// before any user logged in.

#[tauri::command]
pub async fn get_config(
    app_handle: tauri::AppHandle,
    session_state: tauri::State<'_, crate::rbac::SessionState>,
) -> Result<Option<AppConfig>, String> {
    let cfg = AppConfig::load(&app_handle);
    // Once setup is complete, require SettingsManage to read config — but
    // ONLY if a session exists. Pre-login (boot screen) access is allowed
    // because db_password is skip_serializing (never sent to frontend).
    if let Some(c) = &cfg {
        if c.setup_complete {
            let _ = crate::rbac::require_if_session(
                &session_state,
                crate::rbac::Permission::SettingsManage,
            )?;
        }
    }
    Ok(cfg)
}

#[tauri::command]
pub async fn save_config(
    app_handle: tauri::AppHandle,
    session_state: tauri::State<'_, crate::rbac::SessionState>,
    config: AppConfig,
) -> Result<(), String> {
    // If setup is already complete AND a session exists, require SettingsManage.
    // Pre-login (first-run Setup) is allowed.
    let existing = AppConfig::load(&app_handle);
    if existing.as_ref().map(|c| c.setup_complete).unwrap_or(false) {
        if let Some(s) = crate::rbac::require_if_session(
            &session_state,
            crate::rbac::Permission::SettingsManage,
        )? {
            crate::audit::for_session(
                &app_handle.state::<sqlx::PgPool>(),
                &s,
                "config_save",
                "config",
                None,
                Some(serde_json::json!({"db_host": config.db_host, "db_port": config.db_port})),
            ).await;
        }
    }
    config.save(&app_handle)
}

#[tauri::command]
pub async fn get_local_ip() -> Result<String, String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").map_err(|e| e.to_string())?;
    socket.connect("8.8.8.8:80").map_err(|e| e.to_string())?;
    Ok(socket.local_addr().map_err(|e| e.to_string())?.ip().to_string())
}

#[tauri::command]
pub async fn test_server_connection(host: String, port: u16) -> Result<bool, String> {
    Ok(crate::discovery::is_reachable(&host, port, 3000))
}

/// Called by the Setup/Repair screen on the server build when config.json
/// is missing or incomplete. Writes a valid config so the app can start.
///
/// The frontend should call this during first-run setup, passing the
/// PostgreSQL password that the NSIS installer generated (which it prints
/// on screen during installation).
#[tauri::command]
pub async fn repair_server_config(
    app_handle: tauri::AppHandle,
    session_state: tauri::State<'_, crate::rbac::SessionState>,
    db_password: String,
    db_user: Option<String>,
    db_name: Option<String>,
    db_port: Option<u16>,
    clinic_name: Option<String>,
) -> Result<(), String> {
    if db_password.trim().is_empty() {
        return Err("Database password cannot be empty.".to_string());
    }
    // If setup is already complete AND a session exists, require SettingsManage.
    // Pre-login (first-run Setup) is allowed.
    let existing = AppConfig::load(&app_handle);
    if existing.as_ref().map(|c| c.setup_complete).unwrap_or(false) {
        if let Some(s) = crate::rbac::require_if_session(
            &session_state,
            crate::rbac::Permission::SettingsManage,
        )? {
            crate::audit::for_session(
                &app_handle.state::<sqlx::PgPool>(),
                &s,
                "config_repair",
                "config",
                None,
                None,
            ).await;
        }
    }
    let mut cfg = AppConfig::load(&app_handle).unwrap_or_default();
    cfg.mode          = "server".to_string();
    cfg.db_host       = "127.0.0.1".to_string();
    cfg.db_password   = db_password.trim().to_string();
    cfg.setup_complete = true;
    if let Some(u) = db_user    { cfg.db_user     = u; }
    if let Some(n) = db_name    { cfg.db_name     = n; }
    if let Some(p) = db_port    { cfg.db_port     = p; }
    if let Some(c) = clinic_name { cfg.clinic_name = c; }
    cfg.save(&app_handle)
}

/// Returns the path where config.json is (or would be) saved.
/// Useful for the Setup screen to tell the user where to look.
#[tauri::command]
pub async fn get_config_path(app_handle: tauri::AppHandle) -> Result<String, String> {
    Ok(AppConfig::config_path(&app_handle).to_string_lossy().to_string())
}

/// Deletes the config file to allow re-setup (used on client builds when reconfiguring).
/// Registered in generate_handler![] — cargo doesn't see the macro-expanded usage.
#[allow(dead_code)]
#[tauri::command]
pub async fn clear_config(
    app_handle: tauri::AppHandle,
    session_state: tauri::State<'_, crate::rbac::SessionState>,
) -> Result<(), String> {
    // If setup is already complete AND a session exists, require SettingsManage.
    // Pre-login (first-run Setup / reconfigure) is allowed.
    let existing = AppConfig::load(&app_handle);
    if existing.as_ref().map(|c| c.setup_complete).unwrap_or(false) {
        if let Some(s) = crate::rbac::require_if_session(
            &session_state,
            crate::rbac::Permission::SettingsManage,
        )? {
            crate::audit::for_session(
                &app_handle.state::<sqlx::PgPool>(),
                &s,
                "config_clear",
                "config",
                None,
                None,
            ).await;
        }
    }
    let path = AppConfig::config_path(&app_handle);
    if path.exists() {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}
