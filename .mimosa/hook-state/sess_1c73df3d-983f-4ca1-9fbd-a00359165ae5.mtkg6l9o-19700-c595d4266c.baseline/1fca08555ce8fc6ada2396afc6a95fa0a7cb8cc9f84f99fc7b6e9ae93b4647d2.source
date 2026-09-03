//! sign_license — sign a VitalFlow HMS license payload with the company
//! private key, producing a signed `.license` (JSON) file that the app's
//! `license::verify_license` can verify.
//!
//! Usage:
//!   sign_license --payload customer.json --key private_key.pem --out customer.license
//!
//! The payload JSON contains every `LicenseFile` field EXCEPT `signature`
//! (which this binary computes). See README.md for the full payload schema
//! and examples.
//!
//! CRITICAL COMPATIBILITY: the canonical signing representation is a
//! compact-JSON `BTreeMap` over the 14 non-signature fields (alphabetically
//! sorted keys), built with `serde_json::to_vec`. This MUST exactly match
//! `LicenseFile::canonical_bytes()` in `src-tauri/src/license.rs:92-108`.
//! If the struct gains/loses/renames a field, update BOTH this binary and
//! license.rs (and dev_auto_license.rs) — otherwise every signature will
//! fail to verify. The `canonical_bytes_round_trips` test in license.rs
//! catches drift on the verifier side.

use base64::Engine as _;
use clap::Parser;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

/// The license fields, as supplied by the issuer in the payload JSON.
///
/// This mirrors `LicenseFile` in `src-tauri/src/license.rs:58-81` MINUS the
/// `signature` field (which this binary computes). Field names and types
/// MUST stay in sync with `LicenseFile`.
#[derive(Debug, Clone, Deserialize)]
struct LicensePayload {
    license_id: String,
    hospital_id: String,
    hospital_name: String,
    deployment_id: String,
    /// 64-char hex SHA-256 of the customer's machine hardware. Obtain with
    /// `get_fingerprint` on the customer's server PC.
    hardware_fingerprint: String,
    license_version: String,
    product_edition: String,
    /// Entitled module names — e.g. ["dashboard","patients","appointments",...].
    enabled_modules: Vec<String>,
    /// ISO-8601 issue timestamp, e.g. "2026-07-03T00:00:00Z".
    issue_date: String,
    /// Optional ISO-8601 hard expiry. `null` or omitted = perpetual
    /// (maintenance_until still applies for upgrade entitlement).
    #[serde(default)]
    expiration_date: Option<String>,
    /// ISO-8601 date through which software updates are entitled.
    maintenance_until: String,
    software_version_min: String,
    software_version_max: String,
    /// Dev-only licenses are rejected by release builds
    /// (license.rs:207-213, gated on `cfg(not(debug_assertions))`).
    /// Leave `false` or omitted for production licenses.
    #[serde(default)]
    dev: bool,
}

#[derive(Parser)]
#[command(
    name = "sign_license",
    about = "Sign a VitalFlow HMS license payload with the company private key"
)]
struct Args {
    /// Path to the JSON license payload file (all LicenseFile fields except `signature`).
    #[arg(long)]
    payload: PathBuf,

    /// Path to the private key file (.pem produced by gen_keys, or raw 32-byte .bin).
    /// Auto-detected by content: files starting with `-----BEGIN` are parsed as PEM.
    #[arg(long, default_value = "private_key.pem")]
    key: PathBuf,

    /// Output license file path. If omitted, the signed license JSON is
    /// written to stdout (useful for piping, but prefer --out for real
    /// customer licenses so the file isn't intermixed with stderr logs).
    #[arg(long)]
    out: Option<PathBuf>,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("[sign_license] Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse();

    // 1. Read + parse the payload JSON.
    let payload_text = fs::read_to_string(&args.payload)
        .map_err(|e| format!("Cannot read payload file {}: {}", args.payload.display(), e))?;
    let payload: LicensePayload = serde_json::from_str(&payload_text)
        .map_err(|e| format!("Payload JSON is malformed: {}", e))?;

    // Clone for the stderr summary BEFORE we move the fields into the map.
    let summary = payload.clone();

    // 2. Read the private key (PEM or raw .bin).
    let private_key_bytes = read_private_key(&args.key)?;
    let signing_key = SigningKey::from_bytes(&private_key_bytes);

    // 3. Build the canonical BTreeMap — MUST match LicenseFile::canonical_bytes()
    //    in src-tauri/src/license.rs:92-108 EXACTLY:
    //      - same 14 fields, same &str keys
    //      - BTreeMap<&str, serde_json::Value> (alphabetically sorted by key)
    //      - serde_json::to_vec (compact JSON, no whitespace)
    //    Any deviation here will cause verify_signature() to reject the license.
    let mut map: BTreeMap<&str, serde_json::Value> = BTreeMap::new();
    map.insert("license_id", serde_json::json!(payload.license_id));
    map.insert("hospital_id", serde_json::json!(payload.hospital_id));
    map.insert("hospital_name", serde_json::json!(payload.hospital_name));
    map.insert("deployment_id", serde_json::json!(payload.deployment_id));
    map.insert("hardware_fingerprint", serde_json::json!(payload.hardware_fingerprint));
    map.insert("license_version", serde_json::json!(payload.license_version));
    map.insert("product_edition", serde_json::json!(payload.product_edition));
    map.insert("enabled_modules", serde_json::json!(payload.enabled_modules));
    map.insert("issue_date", serde_json::json!(payload.issue_date));
    map.insert("expiration_date", serde_json::json!(payload.expiration_date));
    map.insert("maintenance_until", serde_json::json!(payload.maintenance_until));
    map.insert("software_version_min", serde_json::json!(payload.software_version_min));
    map.insert("software_version_max", serde_json::json!(payload.software_version_max));
    map.insert("dev", serde_json::json!(payload.dev));

    let canonical = serde_json::to_vec(&map)
        .map_err(|e| format!("Canonical serialization failed (infallible for these types): {}", e))?;

    // 4. Sign with Ed25519 and base64-encode.
    //    verify_signature() in license.rs:114-116 decodes with the STANDARD
    //    engine, so we MUST encode with the same engine here.
    let signature = signing_key.sign(&canonical);
    let sig_bytes = signature.to_bytes(); // [u8; 64]
    let sig_b64 = base64::engine::general_purpose::STANDARD.encode(sig_bytes);

    // 5. Sanity check: verify the signature against the public key derived
    //    from the private key, using the SAME canonical bytes. This catches
    //    gross errors (wrong bytes signed, base64 mismatch). It does NOT
    //    catch drift vs license.rs's canonical_bytes() (same construction
    //    is used on both sides here) — that's caught by loading the license
    //    in the actual app, or by the canonical_bytes_round_trips test.
    let verifying_key = signing_key.verifying_key();
    let sig_for_verify = Signature::from_slice(&sig_bytes)
        .map_err(|e| format!("Internal: signature reconstruction failed: {}", e))?;
    verifying_key
        .verify(&canonical, &sig_for_verify)
        .map_err(|e| format!("Internal: self-verification FAILED (this is a bug): {}", e))?;

    // 6. Build the final license JSON: canonical fields + signature.
    //    The output is a BTreeMap too, so keys are sorted — serde_json
    //    deserialization in verify_license_file() is order-agnostic, so this
    //    is purely cosmetic. dev_auto_license.rs uses the same approach.
    //    `sig_b64.as_str()` borrows so sig_b64 stays available for the
    //    stderr summary below (json!() of a &str clones into Value::String).
    let mut license = map.clone();
    license.insert("signature", serde_json::json!(sig_b64.as_str()));
    let json = serde_json::to_string_pretty(&license)
        .map_err(|e| format!("License serialization failed: {}", e))?;

    // 7. Write output.
    match &args.out {
        Some(path) => {
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    fs::create_dir_all(parent).map_err(|e| {
                        format!("Cannot create output directory {}: {}", parent.display(), e)
                    })?;
                }
            }
            fs::write(path, &json)
                .map_err(|e| format!("Cannot write output file {}: {}", path.display(), e))?;
        }
        None => {
            // stdout — ONLY the JSON, so it can be redirected cleanly.
            println!("{}", json);
        }
    }

    // 8. Summary to stderr.
    let exp_display = match &summary.expiration_date {
        Some(e) => e.clone(),
        None => "<perpetual>".to_string(),
    };
    eprintln!("[sign_license] License signed successfully (self-verification OK).");
    eprintln!("[sign_license]   license_id:           {}", summary.license_id);
    eprintln!("[sign_license]   hospital:             {} (id={})", summary.hospital_name, summary.hospital_id);
    eprintln!("[sign_license]   deployment_id:        {}", summary.deployment_id);
    eprintln!("[sign_license]   hardware_fingerprint: {}", summary.hardware_fingerprint);
    eprintln!("[sign_license]   product_edition:      {}", summary.product_edition);
    eprintln!("[sign_license]   enabled_modules:      {} module(s): {}",
              summary.enabled_modules.len(),
              summary.enabled_modules.join(", "));
    eprintln!("[sign_license]   issue_date:           {}", summary.issue_date);
    eprintln!("[sign_license]   expiration_date:      {}", exp_display);
    eprintln!("[sign_license]   maintenance_until:    {}", summary.maintenance_until);
    eprintln!("[sign_license]   version range:        {} .. {}",
              summary.software_version_min, summary.software_version_max);
    eprintln!("[sign_license]   dev:                  {}", summary.dev);
    eprintln!("[sign_license]   signature:            {} base64 chars ({} raw bytes)",
              sig_b64.len(), sig_bytes.len());
    match &args.out {
        Some(path) => eprintln!("[sign_license]   output:               {}", path.display()),
        None => eprintln!("[sign_license]   output:               <stdout>"),
    }
    eprintln!();
    eprintln!("[sign_license] Next: install this license on the customer's server PC and");
    eprintln!("[sign_license] confirm verify_license accepts it. If it rejects with");
    eprintln!("[sign_license] 'signature verification FAILED', the canonical_bytes()");
    eprintln!("[sign_license] construction has drifted between this binary and license.rs.");

    Ok(())
}

/// Read a 32-byte Ed25519 private key from a `.pem` (custom PEM-wrapped
/// base64, as written by `gen_keys`) or a raw `.bin` file. Format is
/// auto-detected by content: files beginning with `-----BEGIN` are parsed
/// as PEM; everything else is treated as raw 32 bytes.
fn read_private_key(path: &PathBuf) -> Result<[u8; 32], String> {
    let bytes = fs::read(path)
        .map_err(|e| format!("Cannot read private key file {}: {}", path.display(), e))?;

    if bytes.starts_with(b"-----BEGIN") {
        // PEM: strip header/footer lines, base64-decode the rest.
        let text = std::str::from_utf8(&bytes)
            .map_err(|e| format!("Private key file is not valid UTF-8 (PEM expected): {}", e))?;
        let b64: String = text
            .lines()
            .filter(|line| !line.starts_with("-----"))
            .collect();
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&b64)
            .map_err(|e| format!("PEM base64 decode failed: {}", e))?;
        if decoded.len() != 32 {
            return Err(format!(
                "Private key in PEM is {} bytes — expected exactly 32 (raw Ed25519 seed).",
                decoded.len()
            ));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&decoded);
        Ok(arr)
    } else {
        // Raw binary.
        if bytes.len() != 32 {
            return Err(format!(
                "Private key file is {} bytes — expected exactly 32 (raw Ed25519 seed),\n\
                 or a PEM file starting with '-----BEGIN'.",
                bytes.len()
            ));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(arr)
    }
}
