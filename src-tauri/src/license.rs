//! Hardware-bound, cryptographically signed licensing.
//!
//! Architectural requirements (see `docs/07-Licensing-Architecture.md`):
//! - Single-hospital license: each deployment is bound to one hospital and
//!   one designated server machine. No generic multi-hospital reuse.
//! - Hardware fingerprint: a SHA-256 over stable Windows hardware identifiers
//!   (CPU ProcessorId + baseboard serial + BIOS serial). Stable across OS
//!   updates; changes on hardware migration → license rejected.
//! - Signed license file: the software company issues a JSON license file
//!   containing hospital identity, module entitlements, validity window, and
//!   the target hardware fingerprint, then signs it with an offline-held
//!   Ed25519 private key. The app embeds only the public key and verifies the
//!   signature on every startup. Forged/tampered/expired/mismatched licenses
//!   are rejected and the application refuses normal operation.
//!
//! Deployment flow:
//!   1. Installer runs on the designated server PC.
//!   2. `get_hardware_fingerprint` is sent to the software company.
//!   3. Company generates a signed `license.json` for that fingerprint +
//!      hospital identity + purchased modules.
//!   4. `install_license` ingests it; signature + fingerprint are verified
//!      before persistence.
//!   5. On every startup `verify_license` re-checks signature + fingerprint +
//!      expiry before the app proceeds past the license gate.

use chrono::{DateTime, Duration, Utc};
use ed25519_dalek::{Signature, Verifier, VerifyingKey, SIGNATURE_LENGTH};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::path::PathBuf;

use base64::Engine as _;
use crate::rbac::{self, Permission, Session};

// LIC-DOC-07: post-expiry grace period. A license that has passed its
// `expiration_date` is allowed to remain in limited operation for this
// many days before the app refuses to start. This gives the operator a
// realistic window to renew with the software company without a hard
// outage at midnight on the expiry date (which could hit in the middle
// of a clinical shift). During grace, the license status is "grace"
// rather than "expired"; the app continues to function but the UI
// surfaces a "license expiring — please renew" warning.
const LICENSE_GRACE_PERIOD_DAYS: i64 = 7;

// ── Embedded software-company public key ──────────────────────────────────────
//
// This is the ONLY verification key shipped with the application. The matching
// private key is held offline by the software company and never distributed.
//
// For dev mode, the keypair below (DEV_PRIVATE_KEY in dev_auto_license.rs +
// COMPANY_PUBLIC_KEY here) is committed to the repo — it can only sign
// dev-flagged licenses, which release builds reject. Safe to commit.
//
// For production: generate a separate keypair with `cargo run --bin gen_keys`
// in the keygen project, replace COMPANY_PUBLIC_KEY below with the real public
// key, and destroy the production private key after signing customer licenses.
pub const COMPANY_PUBLIC_KEY: [u8; 32] = [
    0x09, 0xbb, 0xa3, 0x04, 0x12, 0x3e, 0x7a, 0x0a,
    0xa7, 0x81, 0xdc, 0xf1, 0x6f, 0x75, 0x59, 0x1e,
    0x94, 0xef, 0x9f, 0x9f, 0xdd, 0xcf, 0x40, 0xd5,
    0xaa, 0x28, 0x58, 0xc6, 0xa0, 0x4d, 0x6e, 0x8c,
];

// ── License file structure ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseFile {
    pub license_id: String,
    pub hospital_id: String,
    pub hospital_name: String,
    pub deployment_id: String,
    pub hardware_fingerprint: String,
    pub license_version: String,
    pub product_edition: String,
    pub enabled_modules: Vec<String>,
    /// ISO-8601 issue timestamp.
    pub issue_date: String,
    /// Optional ISO-8601 hard expiry. `None` = perpetual (maintenance window still applies).
    pub expiration_date: Option<String>,
    /// ISO-8601 date through which software updates are entitled.
    pub maintenance_until: String,
    pub software_version_min: String,
    pub software_version_max: String,
    /// True for dev-only licenses. Release builds reject licenses with `dev=true`.
    #[serde(default)]
    pub dev: bool,
    /// Base64 Ed25519 signature over the canonical form of every other field.
    pub signature: String,
}

impl LicenseFile {
    /// Deterministic canonical byte representation: every field except
    /// `signature`, serialised as compact JSON with BTreeMap-enforced sorted
    /// keys. Both signer and verifier use this exact construction.
    ///
    /// CRITICAL: the set of fields here MUST exactly match the signer
    /// (`dev_auto_license.rs` and `keygen/src/bin/sign_license.rs`). If you
    /// add/remove/rename a field, update ALL THREE or signature verification
    /// will fail. The `canonical_bytes_round_trips` test catches drift.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut map: BTreeMap<&str, serde_json::Value> = BTreeMap::new();
        map.insert("license_id", serde_json::json!(self.license_id));
        map.insert("hospital_id", serde_json::json!(self.hospital_id));
        map.insert("hospital_name", serde_json::json!(self.hospital_name));
        map.insert("deployment_id", serde_json::json!(self.deployment_id));
        map.insert("hardware_fingerprint", serde_json::json!(self.hardware_fingerprint));
        map.insert("license_version", serde_json::json!(self.license_version));
        map.insert("product_edition", serde_json::json!(self.product_edition));
        map.insert("enabled_modules", serde_json::json!(self.enabled_modules));
        map.insert("issue_date", serde_json::json!(self.issue_date));
        map.insert("expiration_date", serde_json::json!(self.expiration_date));
        map.insert("maintenance_until", serde_json::json!(self.maintenance_until));
        map.insert("software_version_min", serde_json::json!(self.software_version_min));
        map.insert("software_version_max", serde_json::json!(self.software_version_max));
        map.insert("dev", serde_json::json!(self.dev));
        serde_json::to_vec(&map).expect("canonical serialization is infallible")
    }

    /// Verify the embedded Ed25519 signature against the embedded company
    /// public key. Returns `Ok(())` on success.
    pub fn verify_signature(&self) -> Result<(), String> {
        let sig_bytes = base64::engine::general_purpose::STANDARD
            .decode(&self.signature)
            .map_err(|e| format!("License signature is not valid base64: {}", e))?;
        let signature = Signature::from_slice(&sig_bytes)
            .map_err(|_| format!(
                "License signature is wrong length ({} bytes, expected {}).",
                sig_bytes.len(), SIGNATURE_LENGTH
            ))?;
        let vk = VerifyingKey::from_bytes(&COMPANY_PUBLIC_KEY)
            .map_err(|e| format!("Embedded company public key is invalid: {}", e))?;

        let canonical = self.canonical_bytes();
        // Debug: log the canonical bytes hash so signature mismatches can be
        // diagnosed (compare with the hash printed by dev_auto_license).
        let canonical_hash = {
            let mut h = Sha256::new();
            h.update(&canonical);
            hex::encode(h.finalize())
        };
        eprintln!("[HMS LICENSE] Canonical bytes SHA-256: {}", canonical_hash);
        eprintln!("[HMS LICENSE] Canonical bytes (first 200): {}",
            String::from_utf8_lossy(&canonical[..canonical.len().min(200)]));

        vk.verify(&canonical, &signature)
            .map_err(|_| "License signature verification FAILED — the license is forged, corrupted, or was not issued by the software company.".to_string())
    }
}

// ── License verification result ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct LicenseInfo {
    pub license_id: String,
    pub hospital_id: String,
    pub hospital_name: String,
    pub deployment_id: String,
    pub product_edition: String,
    pub enabled_modules: Vec<String>,
    pub issue_date: String,
    pub expiration_date: Option<String>,
    pub maintenance_until: String,
    pub hardware_fingerprint: String,
    pub fingerprint_matches: bool,
    // LIC-DOC-07: "grace" is a new status — the license has passed its
    // expiration_date but is within LICENSE_GRACE_PERIOD_DAYS of it, so
    // the app continues to operate (with a UI warning) to give the
    // operator time to renew without a hard mid-shift outage.
    pub status: String, // "valid" | "grace" | "expired" | "fingerprint_mismatch" | "unsigned" | "missing" | "revoked"
}

// ── Hardware fingerprint ──────────────────────────────────────────────────────

/// Compute a stable hardware fingerprint for this machine.
/// Delegates to the shared `fingerprint` module (single source of truth).
pub fn compute_hardware_fingerprint() -> Result<String, String> {
    crate::fingerprint::compute()
}

// ── License file location ─────────────────────────────────────────────────────

/// Resolves the on-disk license path.
///
/// In debug builds (dev mode), prefers a per-user dev license at
/// `~/.vitalflow-dev/license.json` — auto-generated by `dev_auto_license.rs`.
/// This keeps dev licenses out of the machine-wide ProgramData path so they
/// don't accidentally get used by a release build.
///
/// In release builds, uses the machine-wide `C:\ProgramData\HMS\license.json`
/// written by the installer, falling back to per-user app data.
pub fn license_file_path(app_handle: &tauri::AppHandle) -> PathBuf {
    // Dev builds look for dev-only license first.
    #[cfg(debug_assertions)]
    {
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from);
        if let Some(h) = home {
            let dev_path = h.join(".vitalflow-dev").join("license.json");
            if dev_path.exists() {
                return dev_path;
            }
        }
        // Fall through to production path if dev license missing.
    }

    // Production: machine-wide ProgramData\HMS (written by the elevated installer).
    if let Some(pd) = std::env::var_os("ProgramData") {
        return PathBuf::from(pd).join("HMS").join("license.json");
    }
    // Fallback to per-user app data (dev mode / non-Windows).
    use tauri::Manager;
    match app_handle.path().app_data_dir() {
        Ok(d) => d.join("license.json"),
        Err(_) => PathBuf::from("license.json"),
    }
}

// ── Core verification ─────────────────────────────────────────────────────────

/// Reads the license file from disk, verifies signature + fingerprint + expiry,
/// and returns a `LicenseInfo` summary. Never panics; returns a descriptive
/// error string on any failure (the caller surfaces it on the license gate).
pub fn verify_license_file(path: &std::path::Path) -> Result<LicenseInfo, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("License file not readable ({}): {}", path.display(), e))?;

    let license: LicenseFile = serde_json::from_str(&content)
        .map_err(|e| format!("License file is not valid JSON: {}", e))?;

    // 0. Release builds MUST reject dev-only licenses.
    #[cfg(not(debug_assertions))]
    if license.dev {
        return Err(
            "This is a development-only license and cannot be used in production builds."
                .to_string(),
        );
    }

    // 1. Signature first — if this fails, nothing else matters.
    license.verify_signature()?;

    // 2. Hardware fingerprint must match this machine.
    let actual_fp = compute_hardware_fingerprint()?;
    let fingerprint_matches = license.hardware_fingerprint == actual_fp;

    // 3. Hard expiry (optional) with LIC-DOC-07 grace period.
    //
    // If the license has passed its `expiration_date`, we don't
    // immediately mark it "expired" — instead we grant a
    // LICENSE_GRACE_PERIOD_DAYS window during which the license status
    // is "grace" and the app continues to operate (with a UI warning).
    // This prevents a hard mid-shift outage at midnight on the expiry
    // date and gives the operator a realistic window to renew with the
    // software company. Only after the grace window elapses does the
    // status become "expired" and the app refuse to start.
    let now = Utc::now();
    let mut status = "valid".to_string();
    if let Some(exp) = &license.expiration_date {
        if let Ok(exp_dt) = DateTime::parse_from_rfc3339(exp) {
            let exp_utc = exp_dt.with_timezone(&Utc);
            if now > exp_utc {
                let grace_end = exp_utc + Duration::days(LICENSE_GRACE_PERIOD_DAYS);
                if now <= grace_end {
                    status = "grace".to_string();
                } else {
                    status = "expired".to_string();
                }
            }
        }
    }
    if !fingerprint_matches && status == "valid" {
        status = "fingerprint_mismatch".to_string();
    }

    Ok(LicenseInfo {
        license_id: license.license_id,
        hospital_id: license.hospital_id,
        hospital_name: license.hospital_name,
        deployment_id: license.deployment_id,
        product_edition: license.product_edition,
        enabled_modules: license.enabled_modules,
        issue_date: license.issue_date,
        expiration_date: license.expiration_date,
        maintenance_until: license.maintenance_until,
        hardware_fingerprint: license.hardware_fingerprint,
        fingerprint_matches,
        status,
    })
}

// ── Persistence to license_state table ────────────────────────────────────────

pub async fn persist_verification(
    pool: &PgPool,
    license: &LicenseFile,
    status: &str,
) -> Result<(), String> {
    let json = serde_json::to_string(license)
        .map_err(|e| format!("License serialize: {}", e))?;
    sqlx::query(
        r#"INSERT INTO license_state
              (license_json, hardware_fingerprint, installed_at, last_verified_at, verification_status)
           VALUES ($1, $2, NOW(), NOW(), $3)
           ON CONFLICT (id) DO UPDATE SET
              license_json = EXCLUDED.license_json,
              hardware_fingerprint = EXCLUDED.hardware_fingerprint,
              last_verified_at = NOW(),
              verification_status = EXCLUDED.verification_status"#,
    )
    .bind(&json)
    .bind(&license.hardware_fingerprint)
    .bind(status)
    .execute(pool)
    .await
    .map(|_| ())
    .map_err(|e| format!("Persist license state: {}", e))
}

// ── Tauri commands ────────────────────────────────────────────────────────────

/// Called at startup BEFORE `initialize_database` proceeds to the auth gate.
/// Returns the license info if valid; an error otherwise. The frontend routes
/// to a license-error screen on error.
///
/// Deliberately DB-free: license verification is a precondition for opening
/// the database connection, so it cannot itself depend on the pool.
#[tauri::command]
pub async fn verify_license(
    app_handle: tauri::AppHandle,
) -> Result<LicenseInfo, String> {
    let path = license_file_path(&app_handle);
    let info = verify_license_file(&path)?;

    // LIC-DOC-07: accept "grace" status as well as "valid" — the license
    // is past its expiration_date but within the 7-day grace window, so
    // the app continues to operate. The frontend surfaces a "license
    // expiring — please renew" warning based on the status field.
    if info.status != "valid" && info.status != "grace" {
        return Err(match info.status.as_str() {
            "expired" => format!(
                "This license has expired. Contact the software company to renew. \
                 (The {}-day grace period has elapsed.)",
                LICENSE_GRACE_PERIOD_DAYS
            ),
            "fingerprint_mismatch" => "This license is bound to a different computer and cannot be used here.".to_string(),
            _ => "License verification failed.".to_string(),
        });
    }
    Ok(info)
}

/// Returns the current machine's hardware fingerprint, so the installer can
/// send it to the software company to generate a bound license.
#[tauri::command]
pub async fn get_hardware_fingerprint() -> Result<String, String> {
    compute_hardware_fingerprint()
}

/// Returns the embedded public key (hex) — used by the Settings → License
/// panel to display the key fingerprint the company must match.
#[tauri::command]
pub async fn get_license_public_key_fingerprint() -> Result<String, String> {
    let mut hasher = Sha256::new();
    hasher.update(COMPANY_PUBLIC_KEY);
    Ok(hex::encode(hasher.finalize()))
}

/// Installs a license from raw JSON contents (read by the frontend via a file
/// picker). Verifies signature + fingerprint before persisting; rejects
/// anything invalid. Requires the `license.manage` permission when a session
/// is active (first-run install has no session yet, so the guard is optional).
///
/// The `pool` is NOT a command parameter — during first-run license setup
/// (before DB initialization), the pool doesn't exist in Tauri state, which
/// would cause "state not managed for field pool". Instead, we use
/// `app_handle.try_state()` inside the function body to optionally get the
/// pool. If available (post-DB-init), we persist + audit. If not (first run),
/// we skip those — the license file on disk is the source of truth.
#[tauri::command]
pub async fn install_license(
    app_handle: tauri::AppHandle,
    session_state: tauri::State<'_, std::sync::Arc<std::sync::Mutex<Option<Session>>>>,
    license_json: String,
) -> Result<LicenseInfo, String> {
    use tauri::Manager;

    // If a session exists, require the manage permission; on first run there
    // is no session yet and the installer flow is allowed to proceed.
    let has_session = session_state.lock().unwrap_or_else(|e| e.into_inner()).is_some();
    if has_session {
        let _ = rbac::require(&session_state, Permission::LicenseManage)?;
    }

    let license: LicenseFile = serde_json::from_str(&license_json)
        .map_err(|e| format!("License JSON is malformed: {}", e))?;

    license.verify_signature()?;

    let actual_fp = compute_hardware_fingerprint()?;
    if license.hardware_fingerprint != actual_fp {
        return Err("This license is bound to a different computer. It cannot be installed here.".to_string());
    }

    let path = license_file_path(&app_handle);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create license directory: {}", e))?;
    }
    std::fs::write(&path, &license_json)
        .map_err(|e| format!("Cannot write license file: {}", e))?;

    // Try to get the pool from Tauri state. During first-run license setup,
    // the pool hasn't been managed yet — try_state returns None, and we skip
    // the DB persist + audit. The license file on disk is the source of truth.
    if let Some(pool) = app_handle.try_state::<PgPool>() {
        let _ = persist_verification(pool.inner(), &license, "valid").await;

        let session_snapshot = {
            let guard = session_state.lock().unwrap_or_else(|e| e.into_inner());
            guard.as_ref().map(|s| (s.user_id, s.username.clone()))
        };
        if let Some((user_id, username)) = session_snapshot {
            crate::audit::record(
                pool.inner(),
                Some(user_id),
                Some(&username),
                "license_install",
                "license",
                Some(&license.license_id),
                Some(serde_json::json!({"hospital_id": license.hospital_id})),
            ).await.ok();
        }
    }

    verify_license_file(&path)
}

// ── LIC-DOC-04: license revocation ───────────────────────────────────────────
//
// Revocation is the operational "undo" for a license install: it removes
// the on-disk license file and writes an audit row recording who revoked
// it and when. After revocation, the next `verify_license` call will fail
// with "License file not readable" (because the file is gone), and the
// app will route to the license-error screen — at which point the
// operator can either install a new license (e.g. a renewed one) or
// decommission the machine.
//
// Use cases (per SDD §5.4 revocation flow):
//   - License transfer: revoke on the OLD machine, then install_license
//     on the NEW machine (using a freshly-signed license bound to the
//     new machine's hardware fingerprint). The software company must
//     issue a new license because the fingerprint is different —
//     revocation alone doesn't "move" a license, it just frees the
//     old machine. See LIC-DOC-08 below.
//   - Suspected compromise: if an operator suspects the license file
//     has been exfiltrated, revoke it (which only affects the LOCAL
//     machine) AND contact the software company to invalidate the
//     license_id server-side (the company can refuse to re-issue for
//     that license_id).
//   - Machine decommission: revoke before wiping / disposing of the
//     hardware so the license file doesn't survive on disk.
//
// LIC-DOC-08 (license transfer flow): there is NO dedicated
// `transfer_license` command. Transfer is the operational sequence:
//   1. On the OLD machine: Settings → License → Revoke (this command).
//   2. On the NEW machine: run `get_install_fingerprint`, send the
//      fingerprint to the software company.
//   3. The software company issues a new signed license bound to the
//      new fingerprint (the OLD license's fingerprint won't match the
//      new hardware, so it can't be reused anyway).
//   4. On the NEW machine: Settings → License → Install License.
//
// This is simpler than a dedicated `transfer_license` command because
// it reuses the existing install + revoke primitives and doesn't
// require a server-side "transfer" API (which would itself be a
// security-sensitive operation requiring careful auth). The trade-off
// is that the software company's offline process must handle the
// "is this customer allowed to re-issue?" check manually.
//
// Gated behind `LicenseManage` — the same permission as `install_license`.
#[tauri::command]
pub async fn revoke_license(
    app_handle: tauri::AppHandle,
    pool: tauri::State<'_, PgPool>,
    session_state: tauri::State<'_, std::sync::Arc<std::sync::Mutex<Option<Session>>>>,
) -> Result<(), String> {
    let session = rbac::require(&session_state, Permission::LicenseManage)?;

    let path = license_file_path(&app_handle);

    // Best-effort: capture the license_id BEFORE deleting the file so
    // the audit row can reference it. If the file is missing or
    // unreadable, we still proceed with the revocation (the operator's
    // intent is "ensure no license is active on this machine").
    let license_id_for_audit: Option<String> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|content| serde_json::from_str::<LicenseFile>(&content).ok())
        .map(|l| l.license_id);

    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|e| format!("Cannot remove license file: {}", e))?;
    }

    // Mark the persisted license_state row as revoked (if one exists).
    // Best-effort: a failure here doesn't block the revoke — the on-disk
    // file is already gone, which is the primary revocation signal.
    let _ = sqlx::query(
        "UPDATE license_state \
         SET verification_status = 'revoked', last_verified_at = NOW() \
         WHERE id = 1",
    )
    .execute(pool.inner())
    .await;

    // Audit row — the audit is the durable record that revocation
    // happened, even after the license file itself is gone.
    //
    // Note: we `clone()` license_id_for_audit into the JSON because the
    // `as_deref()` borrow passed as `resource_id` (previous arg) is
    // still live when this arg is evaluated — moving the original would
    // trip the borrow checker. The clone is cheap (one Option<String>).
    // `path.to_string_lossy().into_owned()` converts the Cow<str> to an
    // owned String so serde_json::json! can serialize it without
    // ambiguity (Value: From<Cow<str>> is implemented but explicit
    // conversion is safer).
    let path_str = path.to_string_lossy().into_owned();
    crate::audit::for_session(
        pool.inner(),
        &session,
        "license_revoke",
        "license",
        license_id_for_audit.as_deref(),
        Some(serde_json::json!({
            "license_id": license_id_for_audit.clone(),
            "path": path_str,
        })),
    )
    .await;

    Ok(())
}

/// Returns the persisted license info from the `license_state` table (last
/// known verification), for display in Settings. Does NOT re-verify the
/// signature — use `verify_license` for that.
#[tauri::command]
pub async fn get_license_info(
    pool: tauri::State<'_, PgPool>,
    session_state: tauri::State<'_, std::sync::Arc<std::sync::Mutex<Option<Session>>>>,
) -> Result<Option<LicenseInfo>, String> {
    let _ = rbac::require(&session_state, Permission::SettingsManage)
        .or_else(|_| rbac::require(&session_state, Permission::LicenseManage))?;

    let row: Option<(String, String, Option<DateTime<Utc>>, String)> = sqlx::query_as(
        "SELECT license_json, hardware_fingerprint, last_verified_at, verification_status
         FROM license_state ORDER BY id DESC LIMIT 1",
    )
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| format!("Load license state: {}", e))?;

    match row {
        None => Ok(None),
        Some((json, fp, _verified_at, status)) => {
            let license: LicenseFile = serde_json::from_str(&json)
                .map_err(|e| format!("Stored license JSON is corrupt: {}", e))?;
            Ok(Some(LicenseInfo {
                license_id: license.license_id,
                hospital_id: license.hospital_id,
                hospital_name: license.hospital_name,
                deployment_id: license.deployment_id,
                product_edition: license.product_edition,
                enabled_modules: license.enabled_modules,
                issue_date: license.issue_date,
                expiration_date: license.expiration_date,
                maintenance_until: license.maintenance_until,
                hardware_fingerprint: fp,
                fingerprint_matches: status == "valid",
                status,
            }))
        }
    }
}

/// Returns this machine's hardware fingerprint + a copy-to-clipboard-friendly
/// display string. Used by the first-run license wizard so the customer can
/// send the fingerprint to the software company for signing.
///
/// No session required — this runs BEFORE login (the user hasn't licensed yet).
#[tauri::command]
pub async fn get_install_fingerprint() -> Result<InstallFingerprint, String> {
    let fp = crate::fingerprint::compute()?;
    Ok(InstallFingerprint {
        fingerprint: fp.clone(),
        display: format!(
            "{}\n\nCopy this 64-character string and send it to your software vendor to receive your license file.",
            fp
        ),
    })
}

#[derive(Debug, serde::Serialize)]
pub struct InstallFingerprint {
    pub fingerprint: String,
    pub display: String,
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// A license's canonical bytes must be stable across a serialize →
    /// deserialize round-trip. If this fails, the signer and verifier disagree
    /// on the canonical format and every signature will fail to verify.
    #[test]
    fn canonical_bytes_round_trips() {
        let license = LicenseFile {
            license_id: "LIC-TEST-001".to_string(),
            hospital_id: "H001".to_string(),
            hospital_name: "Test Hospital".to_string(),
            deployment_id: "DEP-001".to_string(),
            hardware_fingerprint: "abc123".to_string(),
            license_version: "1.0".to_string(),
            product_edition: "Enterprise".to_string(),
            enabled_modules: vec!["dashboard".to_string(), "patients".to_string()],
            issue_date: "2026-07-03T00:00:00Z".to_string(),
            expiration_date: None,
            maintenance_until: "2099-12-31".to_string(),
            software_version_min: "0.0.0".to_string(),
            software_version_max: "999.999.999".to_string(),
            dev: false,
            signature: "placeholder".to_string(),
        };
        let bytes1 = license.canonical_bytes();
        let serialized = serde_json::to_string(&license).unwrap();
        let deserialized: LicenseFile = serde_json::from_str(&serialized).unwrap();
        let bytes2 = deserialized.canonical_bytes();
        assert_eq!(
            bytes1, bytes2,
            "canonical bytes must be stable across serialization round-trip"
        );
    }

    /// The canonical bytes must include the `dev` field — otherwise dev/prod
    /// separation breaks. This test catches accidental removal of the field
    /// from `canonical_bytes`.
    #[test]
    fn canonical_bytes_includes_dev_field() {
        let license = LicenseFile {
            license_id: "x".to_string(),
            hospital_id: "x".to_string(),
            hospital_name: "x".to_string(),
            deployment_id: "x".to_string(),
            hardware_fingerprint: "x".to_string(),
            license_version: "1.0".to_string(),
            product_edition: "x".to_string(),
            enabled_modules: vec![],
            issue_date: "x".to_string(),
            expiration_date: None,
            maintenance_until: "x".to_string(),
            software_version_min: "x".to_string(),
            software_version_max: "x".to_string(),
            dev: true,
            signature: "x".to_string(),
        };
        let bytes = String::from_utf8(license.canonical_bytes()).unwrap();
        assert!(
            bytes.contains("\"dev\":true"),
            "canonical bytes must include the dev field; got: {}",
            bytes
        );
    }

    /// A license missing the `dev` field (e.g. signed by an old signer that
    /// doesn't know about it) must deserialize with `dev = false` thanks to
    /// `#[serde(default)]`. This ensures backward compatibility.
    #[test]
    fn missing_dev_field_defaults_to_false() {
        let json = r#"{
            "license_id": "x",
            "hospital_id": "x",
            "hospital_name": "x",
            "deployment_id": "x",
            "hardware_fingerprint": "x",
            "license_version": "1.0",
            "product_edition": "x",
            "enabled_modules": [],
            "issue_date": "x",
            "expiration_date": null,
            "maintenance_until": "x",
            "software_version_min": "x",
            "software_version_max": "x",
            "signature": "x"
        }"#;
        let license: LicenseFile = serde_json::from_str(json).unwrap();
        assert!(!license.dev, "missing dev field must default to false");
    }
}
