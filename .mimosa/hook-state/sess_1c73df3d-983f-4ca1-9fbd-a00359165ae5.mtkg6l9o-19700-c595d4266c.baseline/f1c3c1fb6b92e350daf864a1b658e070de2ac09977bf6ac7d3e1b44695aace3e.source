//! Hardware fingerprinting — single source of truth.
//!
//! Used by:
//! - `license.rs` — verifies license-bound fingerprints at startup
//! - `dev_auto_license.rs` binary — generates dev licenses for the current machine
//! - `get_install_fingerprint` command — first-run license wizard
//!
//! Previously this logic was duplicated in three places, which is a bug magnet.
//! Now there's one function: `compute()`.

use sha2::{Digest, Sha256};

/// Compute a stable hardware fingerprint for this machine.
///
/// Windows: SHA-256 over CPU ProcessorId + baseboard serial + BIOS serial
/// (queried via WMI). These survive OS updates and routine driver changes but
/// change if the motherboard/CPU is replaced.
///
/// Non-Windows (dev/build machines): SHA-256 over hostname + OS + a fixed
/// salt. This is NOT a production fingerprint — production deployments run on
/// Windows where the WMI path is used.
pub fn compute() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        compute_windows()
    }
    #[cfg(not(target_os = "windows"))]
    {
        compute_fallback()
    }
}

#[cfg(target_os = "windows")]
fn compute_windows() -> Result<String, String> {
    use wmi::{COMLibrary, WMIConnection};

    // Typed structs for WMI deserialization. The `wmi` crate requires
    // Deserialize structs (not raw serde_json::Value) — raw Value
    // deserialization fails with "Only structs and maps can be deserialized
    // from WMI objects".
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Win32Processor {
        processor_id: Option<String>,
    }

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Win32BaseBoard {
        serial_number: Option<String>,
    }

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Win32Bios {
        serial_number: Option<String>,
    }

    let com = COMLibrary::new()
        .map_err(|e| format!("WMI COM init failed: {}", e))?;
    let wmi = WMIConnection::new(com)
        .map_err(|e| format!("WMI connection failed: {}", e))?;

    // CPU
    let cpu_results: Vec<Win32Processor> = wmi
        .raw_query("SELECT ProcessorId FROM Win32_Processor")
        .map_err(|e| format!("WMI CPU query failed: {}", e))?;
    let cpu_id = cpu_results
        .first()
        .and_then(|c| c.processor_id.as_deref())
        .unwrap_or("")
        .trim()
        .to_string();

    // Baseboard
    let board_results: Vec<Win32BaseBoard> = wmi
        .raw_query("SELECT SerialNumber FROM Win32_BaseBoard")
        .map_err(|e| format!("WMI baseboard query failed: {}", e))?;
    let board_sn = board_results
        .first()
        .and_then(|b| b.serial_number.as_deref())
        .unwrap_or("")
        .trim()
        .to_string();

    // BIOS
    let bios_results: Vec<Win32Bios> = wmi
        .raw_query("SELECT SerialNumber FROM Win32_BIOS")
        .map_err(|e| format!("WMI BIOS query failed: {}", e))?;
    let bios_sn = bios_results
        .first()
        .and_then(|b| b.serial_number.as_deref())
        .unwrap_or("")
        .trim()
        .to_string();

    let mut hasher = Sha256::new();
    hasher.update(b"vitalflow-hms-fp-v1\0");
    hasher.update(cpu_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(board_sn.as_bytes());
    hasher.update(b"\0");
    hasher.update(bios_sn.as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(not(target_os = "windows"))]
fn compute_fallback() -> Result<String, String> {
    let hostname = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "unknown-host".to_string());
    let mut hasher = Sha256::new();
    hasher.update(b"vitalflow-hms-fp-dev-v1\0");
    hasher.update(hostname.as_bytes());
    hasher.update(b"\0");
    hasher.update(std::env::consts::OS.as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::compute;

    #[test]
    fn fingerprint_is_stable() {
        // Computing twice should yield the same result on the same machine.
        let a = compute().expect("compute fingerprint (1)");
        let b = compute().expect("compute fingerprint (2)");
        assert_eq!(a, b, "fingerprint must be deterministic");
        assert!(!a.is_empty(), "fingerprint must not be empty");
    }
}
