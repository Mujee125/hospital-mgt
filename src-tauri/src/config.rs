use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
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
    /// SECURITY: `#[serde(skip_serializing)]` keeps the password out of every
    /// JSON payload — including the `get_config` IPC reply, so the frontend
    /// never receives it. `default` additionally makes the field OPTIONAL on
    /// deserialization: a `save_config` round-trip (which cannot echo the
    /// password back because it never received it) must not fail with
    /// "missing field db_password" — VF-VERIF-002. `AppConfig::load` and
    /// `repair_server_config` set the real value in memory after parsing.
    #[serde(skip_serializing, default)]
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
    /// RCTF-IMPL-001 WP-3: config format version.
    /// 1 = legacy (plaintext db_password); 2 = DPAPI-encrypted db_password.
    #[serde(default = "default_config_version")]
    pub config_version: u32,
    /// RCTF-IMPL-001 WP-3: encrypted db_password (base64 DPAPI blob on Windows).
    /// Written by `save()`, read by `load()`. Never serialized to frontend.
    #[serde(skip_serializing)]
    pub db_password_encrypted: Option<String>,
}

fn default_config_version() -> u32 { 1 }

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
            config_version: 1,
            db_password_encrypted: None,
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

    /// AERP Part G (config_tests): same resolution as `config_path` without
    /// needing an AppHandle. Only the app-config-dir fallback differs —
    /// tests always place their config at the machine path (ProgramData),
    /// which `config_path` prefers whenever the file exists, so behavior
    /// under test is identical to production resolution.
    #[cfg(feature = "hms-integration-tests")]
    pub fn config_path_for_tests() -> Option<PathBuf> {
        Self::machine_config_path().filter(|p| p.exists())
    }

    /// Review Pass 3, P3-4: tri-state disk probe for the config-mutation gate.
    /// Distinguishes "never set up" (Missing) from "exists but unreadable"
    /// (Corrupt) so a corrupt file on a configured machine cannot be treated
    /// as first-run by the auth gate.
    pub(crate) fn disk_config_state(app_handle: &tauri::AppHandle) -> crate::rbac::ConfigDiskState {
        let path = Self::config_path(app_handle);
        if !path.exists() {
            return crate::rbac::ConfigDiskState::Missing;
        }
        match Self::load_from_inner(&path) {
            Some(c) => crate::rbac::ConfigDiskState::Active { setup_complete: c.setup_complete },
            None => crate::rbac::ConfigDiskState::Corrupt,
        }
    }

    pub fn load(app_handle: &tauri::AppHandle) -> Option<Self> {
        let path = Self::config_path(app_handle);
        Self::load_from_inner(&path)
    }

    /// AERP Part G (config_tests): AppHandle-free load from an explicit
    /// path. Identical semantics to `load` — same parse / migrate /
    /// unknown-version-reject / decrypt pipeline.
    #[cfg(feature = "hms-integration-tests")]
    pub fn load_from(path: &Path) -> Option<Self> {
        Self::load_from_inner(path)
    }

    fn load_from_inner(path: &Path) -> Option<Self> {
        let content = fs::read_to_string(path).ok()?;

        // Parse as generic JSON to check config_version.
        let json: serde_json::Value = serde_json::from_str(&content).ok()?;
        let version = json.get("config_version").and_then(|v| v.as_u64()).unwrap_or(1) as u32;

        // WP3-N04 (AERP Part G): reject config files from a NEWER format
        // version than this binary understands. Silently treating an unknown
        // future version as v1/v2 could mis-handle fields we don't know
        // about (e.g. a v3 with a different encryption envelope would be
        // decrypted as garbage or, worse, parsed as plaintext v1).
        if version > 2 {
            eprintln!(
                "[HMS CONFIG] ERROR: config.json has unknown config_version {} (this build supports 1 and 2). \
                 Refusing to load — update the application or restore a compatible config backup.",
                version
            );
            return None;
        }

        let mut cfg: AppConfig = serde_json::from_value(json).ok()?;

        if version < 2 {
            // V1 (legacy): db_password is plaintext. Mark for migration.
            // The actual encryption happens on the next save() call.
            // For now, populate db_password_encrypted so save() knows to encrypt.
            cfg.config_version = 2; // upgrade in-memory; disk upgrades on next save
        } else {
            // V2: decrypt db_password from db_password_encrypted.
            if let Some(enc) = &cfg.db_password_encrypted {
                match crate::secrets::decrypt(enc) {
                    Ok(plain) => { cfg.db_password = plain; }
                    Err(e) => {
                        eprintln!("[HMS CONFIG] ERROR: failed to decrypt db_password: {}. Database connection will fail.", e);
                        cfg.db_password = String::new();
                    }
                }
            }
        }

        Some(cfg)
    }

    /// Persist the config to disk atomically and ACL-harden it on Windows.
    ///
    /// Atomicity: write to `config.json.tmp` then `rename` → `config.json`. A
    /// crash mid-write leaves the temp file, not a truncated config. (REL-10.)
    ///
    /// ACL hardening (CR-5, Windows only). RESIDUAL-THREAT NOTE (Review
    /// Pass 3, P3-9): the v2 file no longer contains a plaintext password —
    /// the blob is DPAPI LOCAL_MACHINE-encrypted, which ANY process on THIS
    /// machine can transparently decrypt. The ACL's real value is therefore
    /// TAMPER-INTEGRITY (stop other accounts modifying/replacing the config
    /// the app trusts at boot), not confidentiality. The .bak is the one
    /// artifact whose ACL is confidentiality-load-bearing (genuine v1
    /// plaintext). Do not cite "plaintext password" as the reason for this
    /// ACL again — it misdirects future hardening work.
    pub fn save(&self, app_handle: &tauri::AppHandle) -> Result<(), String> {
        let path = Self::config_path(app_handle);
        self.save_to_inner(&path)
    }

    /// AERP Part G (config_tests): AppHandle-free save to an explicit
    /// path. Identical semantics to `save` — encryption, .bak backup on
    /// v1→v2 migration, atomic write, ACL hardening.
    #[cfg(feature = "hms-integration-tests")]
    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        self.save_to_inner(path)
    }

    fn save_to_inner(&self, path: &Path) -> Result<(), String> {
        // RCTF-IMPL-001 WP-3: encrypt db_password before writing to disk.
        let mut save_cfg = self.clone();
        if !save_cfg.db_password.is_empty() {
            let enc = crate::secrets::encrypt(&save_cfg.db_password)
                .map_err(|e| format!("Failed to encrypt db_password: {}", e))?;
            save_cfg.db_password_encrypted = Some(enc);
            save_cfg.config_version = 2;
        }

        // VF-VERIF-003: `db_password_encrypted` is `#[serde(skip_serializing)]`
        // so it never leaks through the `get_config` IPC reply. But `save()`
        // serializes this same struct for the DISK write, where the field is
        // required — a v2 file without the blob decrypts to an empty password
        // on next launch and bricks the DB connection. Re-inject it into the
        // JSON object after struct serialization, keeping the IPC-safe
        // skip_serializing property intact.
        let mut json: serde_json::Value =
            serde_json::to_value(&save_cfg).map_err(|e| e.to_string())?;
        if save_cfg.db_password_encrypted.is_some() {
            json["db_password_encrypted"] =
                serde_json::Value::String(save_cfg.db_password_encrypted.clone().unwrap());
        }
        let content = serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        // WP3-I05 (AERP Part G): before the FIRST v1→v2 migration overwrites
        // the on-disk file, preserve the current file as `config.json.bak`.
        // The v1 file contains the last known-good plaintext password; if the
        // v2 write later turns out unreadable (corrupt blob, ACL issue, DPAPI
        // key loss), the operator has a recovery path that does not require
        // re-provisioning PostgreSQL. Only written when the existing file is
        // still v1 (a v2 file already exists → this is not a migration) and
        // no .bak already exists (never clobber the original v1 backup).
        if save_cfg.config_version == 2 {
            let is_v1_on_disk = fs::read_to_string(path)
                .ok()
                .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                .map(|j| j.get("config_version").and_then(|v| v.as_u64()).unwrap_or(1) < 2)
                .unwrap_or(false);
            if is_v1_on_disk {
                let bak = path.with_extension("json.bak");
                if !bak.exists() {
                    // Best-effort: a failed backup must not block the save —
                    // the atomic write below is the critical path.
                    let _ = fs::copy(path, &bak);

                    // The .bak holds the PLAINTEXT v1 password, so it MUST get
                    // the same ACL hardening as config.json itself. A plain
                    // fs::copy inherits the parent dir's ACEs (Users:Modify on
                    // C:\ProgramData\HMS) — leaving the last known-good
                    // password readable by every local user. Match the
                    // config.json ACL policy below (VF-VERIF-004 grants).
                    #[cfg(target_os = "windows")]
                    {
                        let mut icacls = std::process::Command::new("icacls");
                        icacls.arg(bak.as_os_str())
                            .args(["/inheritance:r"])
                            .args(["/grant:r", "SYSTEM:F"])
                            .args(["/grant:r", "Administrators:F"]);
                        if let Some(user) = std::env::var_os("USERNAME") {
                            let _ = icacls.arg(format!("{}:(R)", user.to_string_lossy()));
                        }
                        let _ = icacls
                            .stdout(std::process::Stdio::null())
                            .stderr(std::process::Stdio::null())
                            .status();
                    }
                }
            }
        }

        // Atomic write: temp file + rename. WP3-C01 (AERP Part G): the temp
        // name must be UNIQUE per call — a fixed "config.json.tmp" lets two
        // concurrent saves race on the same temp file (one thread renames it
        // away while the other is mid-write → the loser's rename fails with
        // os error 2). Uniqueness by pid + nanos + a process-local counter
        // keeps each writer's rename atomic with no shared intermediate.
        let unique = format!(
            "{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0),
            {
                use std::sync::atomic::{AtomicU64, Ordering};
                static SEQ: AtomicU64 = AtomicU64::new(0);
                SEQ.fetch_add(1, Ordering::Relaxed)
            }
        );
        let tmp = path.with_file_name(format!("config.{}.json.tmp", unique));
        fs::write(&tmp, &content).map_err(|e| format!("Write config tmp: {}", e))?;

        // Review F-5 (independent pass, 2026-09-03): ACL-harden the TEMP
        // file BEFORE the rename. The previous order (rename → icacls) left a
        // window in which the live config.json carried the parent dir's
        // inherited ACEs (Users:Modify on C:\ProgramData\HMS). Low practical
        // severity — the content is DPAPI-encrypted v2 by this point — but
        // pre-applying the ACL to the temp file closes the window entirely:
        // the rename swaps one hardened file for another, atomically.
        #[cfg(target_os = "windows")]
        {
            let mut icacls = std::process::Command::new("icacls");
            icacls.arg(tmp.as_os_str())
                .args(["/inheritance:r"])
                .args(["/grant:r", "SYSTEM:F"])
                .args(["/grant:r", "Administrators:F"]);
            if let Some(user) = std::env::var_os("USERNAME") {
                // VF-VERIF-004: Modify (not just Read) — `save()` renames a
                // NEW temp file over this one on every later save; the grant
                // must survive that, and the app itself must be able to
                // replace its own config.
                let _ = icacls.arg(format!("{}:(M)", user.to_string_lossy()));
            }
            let status = icacls
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            if let Err(e) = status {
                eprintln!("[HMS CONFIG] Warning: could not ACL-harden the config temp file ({}). \
                           The renamed config.json may briefly inherit weaker permissions.", e);
            }
        }

        fs::rename(&tmp, path).map_err(|e| {
            // Best-effort cleanup of the temp file on rename failure.
            let _ = fs::remove_file(&tmp);
            format!("Rename config: {}", e)
        })?;

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
    // Review Pass 2, Finding 1 (P0, 2026-09-04) + Pass 3 P3-4/P3-5 hardening:
    // FAIL CLOSED once the system is configured — and crucially, "configured"
    // is now a TRI-STATE read of disk, not `existing.map(setup_complete)`:
    //   Missing            → first-run window (no session required)
    //   Corrupt            → treated as configured (fail closed)
    //   Active{true}       → SettingsManage session REQUIRED
    // The previous formulation collapsed "file exists but unparseable" into
    // "first run", re-opening the fail-open hole one level down (P3-4).
    let disk = AppConfig::disk_config_state(&app_handle);
    let existing = AppConfig::load(&app_handle);
    match crate::rbac::require_config_mutation(
        &session_state,
        disk,
        crate::rbac::Permission::SettingsManage,
    )? {
        crate::rbac::ConfigMutationGrant::Authorized(s) => {
            crate::audit::for_session(
                &app_handle.state::<sqlx::PgPool>(),
                &s,
                "config_save",
                "config",
                None,
                Some(serde_json::json!({"db_host": config.db_host, "db_port": config.db_port})),
            ).await;
        }
        crate::rbac::ConfigMutationGrant::FirstRun => { /* no principal to audit */ }
    }
    // VF-VERIF-002: merge instead of blind persist. The frontend receives
    // AppConfig from `get_config` WITHOUT db_password (skip_serializing — the
    // password must never cross IPC), so a save_config round-trip posts a
    // payload whose db_password is empty. Persisting that payload verbatim
    // would wipe the stored password and break the DB connection on next
    // launch. Merge the UI-editable fields onto the freshly-loaded config so
    // credentials (and any on-disk encryption state) are preserved from disk,
    // not from the webview.
    let mut merged = existing.unwrap_or_else(|| config.clone());

    // Review Pass 3, P3-5: the loopback window is computed from DISK state,
    // never from the payload. With no existing config, `merged` IS the raw
    // payload — a first-run payload of `{setup_complete: true, db_host:
    // <remote>}` previously set `in_setup_window = false` itself and skipped
    // this pin. Only disk proof of a completed setup releases the pin;
    // Missing and Corrupt both count as "window" (conservative).
    #[cfg(feature = "server-build")]
    {
        let in_setup_window =
            !matches!(disk, crate::rbac::ConfigDiskState::Active { setup_complete: true });
        if in_setup_window {
            let host = config.db_host.trim().to_string();
            let is_loopback = host == "127.0.0.1" || host == "localhost" || host == "::1";
            if !is_loopback {
                return Err(
                    "During first-run setup the database host must be this machine \
                     (127.0.0.1). Remote hosts can be configured later from Settings \
                     after signing in.".to_string(),
                );
            }
        }
    }

    merged.mode = config.mode;
    merged.db_host = config.db_host;
    merged.db_port = config.db_port;
    merged.db_user = config.db_user;
    merged.db_name = config.db_name;
    merged.clinic_name = config.clinic_name;
    merged.doctors_whatsapp_group = config.doctors_whatsapp_group;
    merged.setup_complete = config.setup_complete;
    merged.pinned_server_cert_pem = config.pinned_server_cert_pem;
    merged.pinned_server_fingerprint = config.pinned_server_fingerprint;
    merged.save(&app_handle)
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
    // Review Pass 2 Finding 1 + Pass 3 P3-4: fail-closed tri-state gate —
    // see save_config. This command is the more dangerous one: it writes a
    // NEW db_password (not merged), so the unauthenticated window made it a
    // credential-replacement primitive. Corrupt disk counts as configured.
    let disk = AppConfig::disk_config_state(&app_handle);
    match crate::rbac::require_config_mutation(
        &session_state,
        disk,
        crate::rbac::Permission::SettingsManage,
    )? {
        crate::rbac::ConfigMutationGrant::Authorized(s) => {
            crate::audit::for_session(
                &app_handle.state::<sqlx::PgPool>(),
                &s,
                "config_repair",
                "config",
                None,
                None,
            ).await;
        }
        crate::rbac::ConfigMutationGrant::FirstRun => { /* no principal to audit */ }
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
    // Review Pass 2 Finding 1 + Pass 3 P3-4: fail-closed tri-state gate —
    // see save_config. clear_config on a configured machine is effectively
    // an auth-reset primitive (deleting config re-opens the unauthenticated
    // first-run window), so it must never be reachable without a
    // SettingsManage session once the machine is configured (or corrupt).
    let disk = AppConfig::disk_config_state(&app_handle);
    match crate::rbac::require_config_mutation(
        &session_state,
        disk,
        crate::rbac::Permission::SettingsManage,
    )? {
        crate::rbac::ConfigMutationGrant::Authorized(s) => {
            crate::audit::for_session(
                &app_handle.state::<sqlx::PgPool>(),
                &s,
                "config_clear",
                "config",
                None,
                None,
            ).await;
        }
        crate::rbac::ConfigMutationGrant::FirstRun => { /* no principal to audit */ }
    }
    let path = AppConfig::config_path(&app_handle);
    if path.exists() {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}
