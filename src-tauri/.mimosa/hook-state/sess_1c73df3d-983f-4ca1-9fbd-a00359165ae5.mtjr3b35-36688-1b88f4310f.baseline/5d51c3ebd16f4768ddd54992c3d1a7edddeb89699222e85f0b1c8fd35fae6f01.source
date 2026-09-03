//! Production license generator — generates a license with `dev: false`
//! that production builds will accept.
//!
//! Usage:
//!   cargo run --bin gen_production_license --features server-build
//!
//! Or from the npm script:
//!   npm run gen:prod-license
//!
//! This binary uses the SAME keypair as dev_auto_license (the committed dev
//! keypair). In a real production deployment, you would use the `keygen/`
//! project with a separate production keypair. For testing the production
//! build locally, this is fine — the license has `dev: false` so the
//! production build accepts it, and it's signed with the key that matches
//! the embedded COMPANY_PUBLIC_KEY.
//!
//! The license is written to C:\ProgramData\HMS\license.json (the
//! production path that release builds check).

use ed25519_dalek::{Signer, SigningKey};
use std::collections::BTreeMap;
use std::path::PathBuf;
use base64::Engine as _;

// Same dev keypair — matches COMPANY_PUBLIC_KEY in license.rs.
const DEV_PRIVATE_KEY: [u8; 32] = [
    0x42, 0x34, 0xbc, 0x97, 0xec, 0xbb, 0xbc, 0x32,
    0xa9, 0x86, 0x64, 0xec, 0xe0, 0xf2, 0x02, 0x12,
    0xf2, 0x07, 0x19, 0x4f, 0x38, 0xf8, 0xa1, 0x6c,
    0x46, 0xe5, 0xd9, 0x80, 0x38, 0xdb, 0x8c, 0x8d,
];

fn main() {
    let fingerprint = hospital_mgmt_lib::fingerprint::compute()
        .expect("failed to compute hardware fingerprint");
    let now = chrono::Utc::now();

    let mut map: BTreeMap<&str, serde_json::Value> = BTreeMap::new();
    map.insert("license_id", serde_json::json!("LIC-PROD-LOCAL"));
    map.insert("hospital_id", serde_json::json!("HMS-001"));
    map.insert("hospital_name", serde_json::json!("VitalFlow Hospital"));
    map.insert("deployment_id", serde_json::json!("DEP-001"));
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
    map.insert("dev", serde_json::json!(false)); // ← FALSE: production builds accept this

    let canonical = serde_json::to_vec(&map).expect("serialize canonical");

    let signing_key = SigningKey::from_bytes(&DEV_PRIVATE_KEY);
    let signature = signing_key.sign(&canonical);
    let sig_b64 = base64::engine::general_purpose::STANDARD.encode(signature.to_bytes());

    let mut license = map.clone();
    license.insert("signature", serde_json::json!(sig_b64));

    // Write to the PRODUCTION path (C:\ProgramData\HMS\license.json)
    let prod_path = production_license_path();
    if let Some(parent) = prod_path.parent() {
        std::fs::create_dir_all(parent).expect("create license directory");
    }
    let json = serde_json::to_string_pretty(&license).expect("serialize license");
    std::fs::write(&prod_path, &json).expect("write production license");

    println!("[prod-license] Generated PRODUCTION license at: {}", prod_path.display());
    println!("[prod-license] Fingerprint: {}", fingerprint);
    println!("[prod-license] Dev flag: false (production builds will accept this license)");
}

fn production_license_path() -> PathBuf {
    // Windows: C:\ProgramData\HMS\license.json
    if let Some(pd) = std::env::var_os("ProgramData") {
        return PathBuf::from(pd).join("HMS").join("license.json");
    }
    // Fallback for non-Windows
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".vitalflow-dev").join("license.json")
}
