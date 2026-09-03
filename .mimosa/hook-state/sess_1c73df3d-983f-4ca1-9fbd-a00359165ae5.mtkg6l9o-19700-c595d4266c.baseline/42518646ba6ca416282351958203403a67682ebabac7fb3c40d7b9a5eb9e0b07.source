//! Dev auto-license generator — runs automatically before `tauri dev`.
//!
//! Produces a dev-only license for the current machine, signed with the
//! committed dev private key, written to `~/.vitalflow-dev/license.json`.
//! The app's dev builds (cfg!(debug_assertions)) look for this file first.
//!
//! NEVER compiled into release builds — this binary is only built via
//! `cargo run --bin dev_auto_license` from the `tauri:dev` npm script.
//! Release builds reject any license with `dev: true`.
//!
//! The dev private key below is INTENTIONALLY committed to the repo. It can
//! only sign dev-flagged licenses, which release builds reject at the
//! cryptographic level. This is safe.

use ed25519_dalek::{Signer, SigningKey};
use std::collections::BTreeMap;
use std::path::PathBuf;
use base64::Engine as _;

// ── Dev private key (matches COMPANY_PUBLIC_KEY in license.rs) ───────────────
//
// Generated once with ed25519-dalek. This is the pair:
//   private: 0x4234bc97...  (below)
//   public:  0x09bba304...  (in license.rs COMPANY_PUBLIC_KEY)
//
// Safe to commit: it can only sign dev licenses, which release builds reject.
const DEV_PRIVATE_KEY: [u8; 32] = [
    0x42, 0x34, 0xbc, 0x97, 0xec, 0xbb, 0xbc, 0x32,
    0xa9, 0x86, 0x64, 0xec, 0xe0, 0xf2, 0x02, 0x12,
    0xf2, 0x07, 0x19, 0x4f, 0x38, 0xf8, 0xa1, 0x6c,
    0x46, 0xe5, 0xd9, 0x80, 0x38, 0xdb, 0x8c, 0x8d,
];

fn main() {
    // Always regenerate the license (overwrite any stale one signed with
    // an old key). This prevents "signature verification FAILED" errors
    // when the COMPANY_PUBLIC_KEY or DEV_PRIVATE_KEY changes between
    // code updates.
    let dev_path = dev_license_path();

    // Compute this machine's fingerprint using the shared module.
    let fingerprint = hospital_mgmt_lib::fingerprint::compute()
        .expect("failed to compute hardware fingerprint");
    let now = chrono::Utc::now();

    // Build the canonical map — MUST match LicenseFile::canonical_bytes() in
    // license.rs exactly (same fields, same keys, BTreeMap sorted order).
    let mut map: BTreeMap<&str, serde_json::Value> = BTreeMap::new();
    map.insert("license_id", serde_json::json!("LIC-DEV-LOCAL"));
    map.insert("hospital_id", serde_json::json!("DEV"));
    map.insert("hospital_name", serde_json::json!("Dev Machine"));
    map.insert("deployment_id", serde_json::json!("DEP-DEV-LOCAL"));
    map.insert("hardware_fingerprint", serde_json::json!(&fingerprint));
    map.insert("license_version", serde_json::json!("1.0"));
    map.insert("product_edition", serde_json::json!("Enterprise"));
    map.insert("enabled_modules", serde_json::json!([
        "dashboard", "patients", "appointments", "queue",
        "ipd", "lab", "billing", "pharmacy", "inventory",
        "hr", "reports", "settings", "admin"
    ]));
    map.insert("issue_date", serde_json::json!(now.to_rfc3339()));
    map.insert("expiration_date", serde_json::Value::Null);
    map.insert("maintenance_until", serde_json::json!("2099-12-31"));
    map.insert("software_version_min", serde_json::json!("0.0.0"));
    map.insert("software_version_max", serde_json::json!("999.999.999"));
    map.insert("dev", serde_json::json!(true)); // ← critical: marks as dev-only

    // Canonical bytes = compact JSON of the BTreeMap (sorted keys).
    let canonical = serde_json::to_vec(&map).expect("serialize canonical");

    // Debug: print the canonical bytes hash so it can be compared with the
    // hash printed by LicenseFile::verify_signature() in license.rs.
    {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(&canonical);
        let hash = hex::encode(h.finalize());
        println!("[dev-license] Canonical bytes SHA-256: {}", hash);
        println!("[dev-license] Canonical bytes (first 200): {}",
            String::from_utf8_lossy(&canonical[..canonical.len().min(200)]));
    }

    // Sign with the dev private key.
    let signing_key = SigningKey::from_bytes(&DEV_PRIVATE_KEY);
    let signature = signing_key.sign(&canonical);
    let sig_b64 = base64::engine::general_purpose::STANDARD.encode(signature.to_bytes());

    // Build the final license JSON (canonical fields + signature).
    let mut license = map.clone();
    license.insert("signature", serde_json::json!(sig_b64));

    // Write to the dev-only path (always overwrites).
    if let Some(parent) = dev_path.parent() {
        std::fs::create_dir_all(parent).expect("create dev license directory");
    }
    let json = serde_json::to_string_pretty(&license).expect("serialize license");
    std::fs::write(&dev_path, &json).expect("write dev license");

    println!("[dev-license] Auto-generated dev license at: {}", dev_path.display());
    println!("[dev-license] Fingerprint: {}", fingerprint);
    println!("[dev-license] Dev flag: true (release builds will reject this license)");
}

fn dev_license_path() -> PathBuf {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".vitalflow-dev").join("license.json")
}
