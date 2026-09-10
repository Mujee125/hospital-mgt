use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use tauri::Manager;

/// Serializes every config write within this process (RCTF-FULL-SYSTEM-
/// 2026-09-09 F-01). The v3 save is a TWO-FILE commit — `entropy.key`
/// (possibly created) and `config.json` — so the write path must be atomic
/// as a PAIR, not just per file. Without the lock, two racing first-time
/// saves could interleave: thread A re-reads entropy E1, thread B installs
/// E2, A then writes a config blob that no longer matches the on-disk key
/// → undecryptable config. Config saves are rare (Settings screen /
/// first-run setup), so a process-wide mutex costs nothing. Poisoning is
/// ignored (`unwrap_or_else`) — a panicking saver leaves no partially-
/// written state (each file write is already temp+rename atomic), so the
/// next save can safely proceed.
static CONFIG_SAVE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn config_save_lock() -> MutexGuard<'static, ()> {
    let m = CONFIG_SAVE_LOCK.get_or_init(|| Mutex::new(()));
    // Poison-tolerant acquire: see the static's doc comment.
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

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
    /// RCTF-FULL-SYSTEM-2026-09-08 F-19: hex of the per-install broadcast
    /// secret (delivered over the TLS pairing channel). Verifies
    /// HMS_SERVER_V2 LAN broadcasts whose HMAC key, unlike the TLS
    /// fingerprint, is never published. Empty on pre-F-19 deployments —
    /// the client falls back to V1 (fingerprint-keyed) broadcasts.
    #[serde(default)]
    pub pinned_broadcast_secret_hex: String,
    /// RCTF-IMPL-001 WP-3: config format version.
    /// 1 = legacy (plaintext db_password); 2 = DPAPI-encrypted db_password
    /// (machine scope, no per-install entropy — RCTF F-01: decryptable by
    /// every process on the machine); 3 = DPAPI + per-install entropy from
    /// `entropy.key` (see `secrets.rs` — decrypting now requires the machine
    /// scope AND the ACL-hardened entropy file, raising the bar from "any
    /// local account" to "admin or the app's own user").
    #[serde(default = "default_config_version")]
    pub config_version: u32,
    /// RCTF-IMPL-001 WP-3: encrypted db_password (base64 DPAPI blob on Windows).
    /// Written by `save()`, read by `load()`. Never serialized to frontend.
    #[serde(skip_serializing)]
    pub db_password_encrypted: Option<String>,
    // ── Phase 7: automatic backups + system health ─────────────────────────
    //
    // All four fields carry serde defaults so every pre-Phase-7 config.json
    // deserializes unchanged (backward compatibility — no config_version
    // bump needed for additive optional fields).
    /// Nightly automatic pg_dump (server build only). Defaults ON for
    /// server builds — a hospital deployment without backups is one disk
    /// failure from losing everything.
    #[serde(default = "default_auto_backup_enabled")]
    pub auto_backup_enabled: bool,
    /// Local hour (0-23) the nightly backup runs at. Default 02:00 —
    /// the quietest window for a clinic.
    #[serde(default = "default_auto_backup_hour")]
    pub auto_backup_hour: u32,
    /// How many recent backup archives to keep in the backups directory.
    /// Oldest beyond this count are deleted after each successful backup.
    #[serde(default = "default_backup_retention_count")]
    pub backup_retention_count: u32,
    /// Optional directory (e.g. a USB drive path) that every new backup is
    /// copied to after creation — an offline copy survives machine theft/
    /// ransomware. Empty = disabled.
    #[serde(default)]
    pub usb_backup_path: String,
}

fn default_auto_backup_enabled() -> bool {
    cfg!(feature = "server-build")
}

fn default_auto_backup_hour() -> u32 {
    2
}

fn default_backup_retention_count() -> u32 {
    14
}

fn default_config_version() -> u32 {
    1
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
            pinned_broadcast_secret_hex: String::new(),
            config_version: 1,
            db_password_encrypted: None,
            auto_backup_enabled: cfg!(feature = "server-build"),
            auto_backup_hour: 2,
            backup_retention_count: 14,
            usb_backup_path: String::new(),
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

    /// RCTF-FULL-SYSTEM-2026-09-09 F-01: on-disk config version (None if the
    /// file is unreadable — treated as "nothing to migrate").
    fn disk_config_version(path: &Path) -> Option<u32> {
        let content = fs::read_to_string(path).ok()?;
        let json: serde_json::Value = serde_json::from_str(&content).ok()?;
        json.get("config_version")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            // A missing config_version field means v1.
            .or(Some(1))
    }

    /// RCTF-FULL-SYSTEM-2026-09-09 F-01: one-shot boot migration. The lazy
    /// v1/v2→v3 upgrade in `load_from_inner` only lands when something
    /// calls `save()` — and nothing saves until an admin edits Settings,
    /// which could be never. This runs at every boot after a successful
    /// load: if the on-disk file is still v1/v2, it re-saves immediately,
    /// so the no-entropy blob leaves disk at the new binary's FIRST
    /// launch, not whenever someone happens to open Settings.
    ///
    /// Failure-tolerant by design: a failed migration (unreadable dir, ACL,
    /// DPAPI) logs and continues booting with the in-memory config — the
    /// v1/v2 file remains loadable by both this binary and the previous
    /// one, so a boot must never strand the machine on the repair screen
    /// over an opportunistic hardening write. The v2-shaped `.bak` written
    /// by the migration save covers the reverse direction.
    pub fn maybe_migrate_to_v3(app_handle: &tauri::AppHandle) {
        let path = Self::config_path(app_handle);
        match Self::disk_config_version(&path) {
            Some(v) if v < 3 => {
                if let Some(cfg) = Self::load_from_inner(&path) {
                    if !cfg.db_password.is_empty() {
                        match cfg.save_to_inner(&path) {
                            Ok(()) => {
                                eprintln!(
                                    "[HMS CONFIG] Migrated config.json v{} → v3 \
                                     (per-install DPAPI entropy, RCTF F-01)",
                                    v
                                );
                            }
                            Err(e) => {
                                eprintln!(
                                    "[HMS CONFIG] WARNING: v{}→v3 migration failed ({}); \
                                     keeping the v{} file — it remains loadable.",
                                    v, e, v
                                );
                            }
                        }
                    }
                }
            }
            _ => {}
        }
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

    // ── RCTF-FULL-SYSTEM-2026-09-09 F-01: per-install DPAPI entropy ──────────

    /// Path of the per-install entropy file, always next to the config file
    /// it protects (`C:\ProgramData\HMS\entropy.key` in production).
    fn entropy_key_path(config_path: &Path) -> PathBuf {
        config_path.with_file_name("entropy.key")
    }

    /// RCTF-FULL-SYSTEM-2026-09-08 F-11: entropy access for OTHER at-rest
    /// secrets (the WhatsApp access token). Resolves the machine config
    /// path (where entropy.key lives) without requiring an AppHandle;
    /// on machines where ProgramData is unset (tests, non-Windows CI)
    /// the caller's encrypt/decrypt is a pass-through anyway, so an
    /// error here only fails on real deployments where it matters.
    pub fn load_or_create_entropy_for_secrets() -> Result<Vec<u8>, String> {
        let path = Self::machine_config_path().ok_or_else(|| {
            "ProgramData env var is not set (entropy key unavailable).".to_string()
        })?;
        Self::load_or_create_entropy(&path)
    }

    /// RCTF-FULL-SYSTEM-2026-09-08 F-19: the per-install LAN-broadcast
    /// secret. Previously the discovery HMAC was keyed on the TLS cert
    /// FINGERPRINT — public by construction (it sits in cleartext in every
    /// paired client's config.json and in get_config), so anyone who ever
    /// read one client's config could forge signed broadcasts (redirect/
    /// DoS against the whole LAN). This secret never leaves the install
    /// except through the TLS pairing channel to a client being paired —
    /// the same out-of-band path the DB credentials already use. Stored as
    /// `broadcast.key` next to entropy.key with the same ACL posture.
    pub fn load_or_create_broadcast_secret() -> Result<Vec<u8>, String> {
        let dir = Self::machine_config_path()
            .ok_or_else(|| {
                "ProgramData env var is not set (broadcast key unavailable).".to_string()
            })?
            .parent()
            .map(|p| p.to_path_buf())
            .ok_or_else(|| "Config path has no parent directory.".to_string())?;
        let key_path = dir.join("broadcast.key");
        if let Ok(bytes) = fs::read(&key_path) {
            if !bytes.is_empty() {
                return Ok(bytes);
            }
        }
        use rand::RngCore;
        let mut fresh = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut fresh);
        let tmp = dir.join(format!(
            "broadcast.{}-{}.key.tmp",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0)
        ));
        fs::write(&tmp, fresh).map_err(|e| format!("Write broadcast tmp: {}", e))?;
        // Same (M)-then-downgrade ACL order as entropy.key — see the
        // F-01 notes there for why the rename needs Modify.
        #[cfg(target_os = "windows")]
        {
            let mut icacls = std::process::Command::new("icacls");
            icacls
                .arg(tmp.as_os_str())
                .args(["/inheritance:r"])
                .args(["/grant:r", "SYSTEM:F"])
                .args(["/grant:r", "Administrators:F"]);
            if let Some(user) = std::env::var_os("USERNAME") {
                let _ = icacls.arg(format!("{}:(M)", user.to_string_lossy()));
            }
            let _ = icacls
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }
        fs::rename(&tmp, &key_path).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            format!("Rename broadcast.key: {}", e)
        })?;
        #[cfg(target_os = "windows")]
        {
            let mut icacls = std::process::Command::new("icacls");
            icacls.arg(key_path.as_os_str()).args(["/grant:r"]);
            if let Some(user) = std::env::var_os("USERNAME") {
                let _ = icacls.arg(format!("{}:(R)", user.to_string_lossy()));
            }
            let _ = icacls
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }
        fs::read(&key_path).map_err(|e| format!("Re-read broadcast.key: {}", e))
    }

    /// Read the install's entropy bytes for DECRYPTING a v3 blob. Strict by
    /// design: a missing/empty file is an error, never a "create a fresh
    /// one" — a freshly generated key would not match the existing blob and
    /// would convert a recoverable situation (restore config.json.bak) into
    /// a silent permanent one.
    fn read_entropy(config_path: &Path) -> Result<Vec<u8>, String> {
        let p = Self::entropy_key_path(config_path);
        let bytes = fs::read(&p).map_err(|e| {
            format!(
                "entropy.key unreadable ({}): {} — v3 configs need it to decrypt. \
                 Restore config.json.bak (v2 shape) to recover.",
                p.display(),
                e
            )
        })?;
        if bytes.is_empty() {
            return Err(format!(
                "entropy.key is empty ({}): v3 configs need it to decrypt. \
                 Restore config.json.bak (v2 shape) to recover.",
                p.display()
            ));
        }
        Ok(bytes)
    }

    /// Load the install's entropy bytes, creating the file on first use
    /// (32 CSPRNG bytes, ACL-hardened to SYSTEM/Administrators/app user).
    ///
    /// Callers hold `CONFIG_SAVE_LOCK` (see `save_to_inner`): the entropy
    /// creation + config write is a TWO-FILE commit, and the mutex makes
    /// the pair atomic within this process — without it, a concurrent
    /// first-time save could observe an entropy key that a sibling racer is
    /// about to replace, writing a config blob no longer decryptable by
    /// what is on disk.
    ///
    /// Race safety beyond the mutex: two creators both write a UNIQUE temp
    /// file and atomically rename it onto `entropy.key`; the creator then
    /// RE-READS the final on-disk value, so even if its rename lost the
    /// race it encrypts with the winning bytes. (The remaining
    /// cross-process window — two app processes doing first-time saves —
    /// is not a supported configuration; single-instance is the deployment
    /// contract, and the v2-shaped `.bak` recovers from any torn state.)
    fn load_or_create_entropy(config_path: &Path) -> Result<Vec<u8>, String> {
        let key_path = Self::entropy_key_path(config_path);

        // Fast path: the file already exists with content.
        if let Ok(bytes) = fs::read(&key_path) {
            if !bytes.is_empty() {
                return Ok(bytes);
            }
            // An empty file (truncated by an old crash/tamper) falls
            // through and is replaced by the create path below.
        }

        use rand::RngCore;
        let mut fresh = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut fresh);

        // Unique temp name (mirrors the config-save pattern) so concurrent
        // creators never share an intermediate file.
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
        let tmp = key_path.with_file_name(format!("entropy.{}.key.tmp", unique));
        fs::write(&tmp, fresh).map_err(|e| format!("Write entropy tmp: {}", e))?;

        // ACL-harden the temp BEFORE the rename (same order as config.json),
        // but the app user gets MODIFY here, not Read: on Windows the rename
        // below needs DELETE permission on the source file, which (R) does
        // not carry — observed live: a UAC-filtered token could WRITE the
        // temp, harden it to (R), and then be DENIED its own rename, leaving
        // an orphan entropy.*.key.tmp and no entropy.key. The downgrade to
        // read-only happens on the FINAL file after the rename (the key is
        // created once and never rewritten, so (R) is the steady state).
        #[cfg(target_os = "windows")]
        {
            let mut icacls = std::process::Command::new("icacls");
            icacls
                .arg(tmp.as_os_str())
                .args(["/inheritance:r"])
                .args(["/grant:r", "SYSTEM:F"])
                .args(["/grant:r", "Administrators:F"]);
            if let Some(user) = std::env::var_os("USERNAME") {
                let _ = icacls.arg(format!("{}:(M)", user.to_string_lossy()));
            }
            if let Err(e) = icacls
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
            {
                eprintln!(
                    "[HMS CONFIG] Warning: could not ACL-harden entropy.key ({}). \
                     The per-install entropy is still applied, but the file may \
                     be readable by other local accounts.",
                    e
                );
            }
        }

        // Atomic install. fs::rename on Windows replaces an existing file
        // (MoveFileEx MOVEFILE_REPLACE_EXISTING) — the same atomic-replace
        // primitive the config write itself relies on.
        fs::rename(&tmp, &key_path).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            format!("Rename entropy.key: {}", e)
        })?;

        // Post-rename downgrade: the FINAL key is read-only for the app
        // user (the entropy never rotates), stripping the MODIFY the rename
        // needed. Best-effort — a failure leaves (M), which still denies
        // every OTHER account (the confidentiality goal) and the key works.
        #[cfg(target_os = "windows")]
        {
            let mut icacls = std::process::Command::new("icacls");
            icacls.arg(key_path.as_os_str()).args(["/grant:r"]);
            if let Some(user) = std::env::var_os("USERNAME") {
                let _ = icacls.arg(format!("{}:(R)", user.to_string_lossy()));
            }
            let _ = icacls
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }

        // Re-read the winner's bytes (see race note above). This read cannot
        // see a torn file: the content arrived via rename, atomically.
        fs::read(&key_path).map_err(|e| format!("Re-read entropy.key: {}", e))
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
            Some(c) => crate::rbac::ConfigDiskState::Active {
                setup_complete: c.setup_complete,
            },
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
        let version = json
            .get("config_version")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as u32;

        // WP3-N04 (AERP Part G): reject config files from a NEWER format
        // version than this binary understands. Silently treating an unknown
        // future version as v1/v2 could mis-handle fields we don't know
        // about (e.g. a v4 with a different encryption envelope would be
        // decrypted as garbage or, worse, parsed as plaintext v1).
        if version > 3 {
            eprintln!(
                "[HMS CONFIG] ERROR: config.json has unknown config_version {} (this build supports 1, 2 and 3). \
                 Refusing to load — update the application or restore a compatible config backup.",
                version
            );
            return None;
        }

        let mut cfg: AppConfig = serde_json::from_value(json).ok()?;

        match version {
            // V3: decrypt with the per-install entropy (RCTF F-01).
            3 => {
                if let Some(enc) = &cfg.db_password_encrypted {
                    match Self::read_entropy(path)
                        .and_then(|entropy| crate::secrets::decrypt_with_entropy(enc, &entropy))
                    {
                        Ok(plain) => {
                            cfg.db_password = plain;
                        }
                        Err(e) => {
                            eprintln!(
                                "[HMS CONFIG] ERROR: failed to decrypt db_password: {}. Database connection will fail. \
                                 Restore config.json.bak (v2 shape) to recover.",
                                e
                            );
                            cfg.db_password = String::new();
                        }
                    }
                }
            }
            // V1 (plaintext) and V2 (DPAPI, no per-install entropy): read
            // the password now, and mark for the v3 upgrade on the next
            // save. V2 blobs were written by the pre-F-01 binary with no
            // entropy — the legacy `decrypt` reads them.
            _ => {
                if version == 2 {
                    if let Some(enc) = &cfg.db_password_encrypted {
                        match crate::secrets::decrypt(enc) {
                            Ok(plain) => {
                                cfg.db_password = plain;
                            }
                            Err(e) => {
                                eprintln!(
                                    "[HMS CONFIG] ERROR: failed to decrypt db_password: {}. Database connection will fail.",
                                    e
                                );
                                cfg.db_password = String::new();
                            }
                        }
                    }
                }
                // In-memory upgrade marker: the on-disk file stays v1/v2
                // until an explicit save() re-encrypts it as v3 — the same
                // lazy-migration pattern the v1→v2 upgrade has used since
                // WP-3 (a read must never rewrite disk).
                cfg.config_version = 3;
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

    /// The v2 (legacy, no-entropy DPAPI) on-disk JSON payload. Used ONLY
    /// for the `.bak` rollback artifact now: the previous binary cannot
    /// read v3, and entropy-loss recovery must work without `entropy.key`
    /// — both need a blob the no-entropy path can decrypt. Shared by the
    /// v1→v3 and v2→v3 migration backup paths in `save_to_inner`.
    fn serialize_v2(&self) -> Result<String, String> {
        let mut save_cfg = self.clone();
        if !save_cfg.db_password.is_empty() {
            let enc = crate::secrets::encrypt(&save_cfg.db_password)
                .map_err(|e| format!("Failed to encrypt db_password: {}", e))?;
            save_cfg.db_password_encrypted = Some(enc);
            save_cfg.config_version = 2;
        }
        Self::inject_blob_and_serialize(&save_cfg)
    }

    /// The v3 (RCTF F-01) on-disk JSON payload — DPAPI machine scope PLUS
    /// the per-install entropy from `entropy.key` (created on first use
    /// next to `config_path`). This is the shape every LIVE config write
    /// persists.
    fn serialize_v3(&self, config_path: &Path) -> Result<String, String> {
        let mut save_cfg = self.clone();
        if !save_cfg.db_password.is_empty() {
            let entropy = Self::load_or_create_entropy(config_path)?;
            let enc = crate::secrets::encrypt_with_entropy(&save_cfg.db_password, &entropy)
                .map_err(|e| format!("Failed to encrypt db_password: {}", e))?;
            save_cfg.db_password_encrypted = Some(enc);
            save_cfg.config_version = 3;
        }
        Self::inject_blob_and_serialize(&save_cfg)
    }

    /// Shared tail of both serializers: struct → JSON, then re-inject
    /// `db_password_encrypted`.
    ///
    /// VF-VERIF-003: `db_password_encrypted` is `#[serde(skip_serializing)]`
    /// so it never leaks through the `get_config` IPC reply. But the disk
    /// write needs the field — a v2/v3 file without the blob decrypts to an
    /// empty password on next launch and bricks the DB connection.
    /// Re-inject it into the JSON object after struct serialization.
    fn inject_blob_and_serialize(save_cfg: &AppConfig) -> Result<String, String> {
        let mut json: serde_json::Value =
            serde_json::to_value(save_cfg).map_err(|e| e.to_string())?;
        if save_cfg.db_password_encrypted.is_some() {
            json["db_password_encrypted"] =
                serde_json::Value::String(save_cfg.db_password_encrypted.clone().unwrap());
        }
        serde_json::to_string_pretty(&json).map_err(|e| e.to_string())
    }

    fn save_to_inner(&self, path: &Path) -> Result<(), String> {
        // RCTF F-01: entropy creation + config write is a two-file commit —
        // hold the process-wide save lock for the whole function so the
        // pair lands atomically (see CONFIG_SAVE_LOCK).
        let _save_lock = config_save_lock();

        // RCTF-IMPL-001 WP-3 + QA-2026-09-08 C2 + RCTF-FULL-SYSTEM-2026-09-09
        // F-01: the live write is the v3 shape (per-install entropy). The
        // migration backup and the live write use SEPARATE serializers now:
        // the .bak deliberately keeps the v2 (no-entropy) shape — the
        // previous binary can't read v3, and entropy-loss recovery must work
        // without entropy.key.
        let content = self.serialize_v3(path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        // WP3-I05 (AERP Part G) + QA-2026-09-08 C2 + RCTF F-01: before the
        // FIRST v1/v2→v3 migration overwrites the on-disk file, preserve the
        // current file as `config.json.bak` in the v2 (no-entropy) shape.
        // The .bak is the recovery path for BOTH rollback directions:
        //   • binary rollback: the previous build rejects version > 2 —
        //     restore the .bak and it loads;
        //   • entropy loss: `entropy.key` deleted/corrupt → the v3 blob is
        //     undecryptable — restore the .bak and it loads (without the
        //     entropy file).
        // Only written when the existing file is still v1/v2 (a v3 file
        // already exists → this is not a migration) and no .bak already
        // exists (never clobber the original migration backup).
        let writing_v3 = !self.db_password.is_empty() || self.config_version >= 3;
        if writing_v3 {
            let is_legacy_on_disk = fs::read_to_string(path)
                .ok()
                .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                .map(|j| {
                    j.get("config_version")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(1)
                        < 3
                })
                .unwrap_or(false);
            if is_legacy_on_disk {
                let bak = path.with_extension("json.bak");
                if !bak.exists() {
                    // Best-effort: a failed backup must not block the save —
                    // the atomic write below is the critical path. If even
                    // the v2 serialization fails (DPAPI unavailable), fall
                    // back to the legacy byte-for-byte copy.
                    //
                    // Empty-password edge: serialize_v2 with no password
                    // would stamp the in-memory version (3) with no blob —
                    // a file the PREVIOUS binary rejects and that carries no
                    // recovery value anyway. The old on-disk bytes are the
                    // correct artifact in that case.
                    if !self.db_password.is_empty() {
                        if let Ok(v2_content) = self.serialize_v2() {
                            let _ = fs::write(&bak, &v2_content);
                        } else {
                            let _ = fs::copy(path, &bak);
                        }
                    } else {
                        let _ = fs::copy(path, &bak);
                    }

                    // The .bak may hold the password (DPAPI blob, or v1
                    // plaintext in the encryption-failure fallback), so it
                    // MUST NOT keep the parent dir's inherited ACEs
                    // (Users:Modify on C:\ProgramData\HMS). Match the
                    // config.json ACL policy below (VF-VERIF-004 grants).
                    #[cfg(target_os = "windows")]
                    {
                        let mut icacls = std::process::Command::new("icacls");
                        icacls
                            .arg(bak.as_os_str())
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
            icacls
                .arg(tmp.as_os_str())
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
                eprintln!(
                    "[HMS CONFIG] Warning: could not ACL-harden the config temp file ({}). \
                           The renamed config.json may briefly inherit weaker permissions.",
                    e
                );
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
            )
            .await;
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
        let in_setup_window = !matches!(
            disk,
            crate::rbac::ConfigDiskState::Active {
                setup_complete: true
            }
        );
        if in_setup_window {
            let host = config.db_host.trim().to_string();
            let is_loopback = host == "127.0.0.1" || host == "localhost" || host == "::1";
            if !is_loopback {
                return Err(
                    "During first-run setup the database host must be this machine \
                     (127.0.0.1). Remote hosts can be configured later from Settings \
                     after signing in."
                        .to_string(),
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
    merged.pinned_broadcast_secret_hex = config.pinned_broadcast_secret_hex;
    merged.save(&app_handle)
}

#[tauri::command]
pub async fn get_local_ip() -> Result<String, String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").map_err(|e| e.to_string())?;
    socket.connect("8.8.8.8:80").map_err(|e| e.to_string())?;
    Ok(socket
        .local_addr()
        .map_err(|e| e.to_string())?
        .ip()
        .to_string())
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
            )
            .await;
        }
        crate::rbac::ConfigMutationGrant::FirstRun => { /* no principal to audit */ }
    }
    let mut cfg = AppConfig::load(&app_handle).unwrap_or_default();
    cfg.mode = "server".to_string();
    cfg.db_host = "127.0.0.1".to_string();
    cfg.db_password = db_password.trim().to_string();
    cfg.setup_complete = true;
    if let Some(u) = db_user {
        cfg.db_user = u;
    }
    if let Some(n) = db_name {
        cfg.db_name = n;
    }
    if let Some(p) = db_port {
        cfg.db_port = p;
    }
    if let Some(c) = clinic_name {
        cfg.clinic_name = c;
    }
    cfg.save(&app_handle)
}

/// Returns the path where config.json is (or would be) saved.
/// Useful for the Setup screen to tell the user where to look.
#[tauri::command]
pub async fn get_config_path(app_handle: tauri::AppHandle) -> Result<String, String> {
    Ok(AppConfig::config_path(&app_handle)
        .to_string_lossy()
        .to_string())
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
            )
            .await;
        }
        crate::rbac::ConfigMutationGrant::FirstRun => { /* no principal to audit */ }
    }
    let path = AppConfig::config_path(&app_handle);
    if path.exists() {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}
