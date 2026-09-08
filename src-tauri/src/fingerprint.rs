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
/// FAIL-CLOSED (licensing review, 2026-09-05): at least 2 of the 3
/// identifiers must be non-empty/usable. Blank serials are common on OEM
/// boards and VMs — two different machines with degenerate identifiers
/// would otherwise hash to the SAME fingerprint and share one license.
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

/// Values some OEM/VM vendors write into SMBIOS serial fields instead of a
/// real serial. Treating them as "present" would let fleets of identical
/// white-box machines share fingerprints.
const FILLER_VALUES: &[&str] = &[
    "default string",
    "to be filled by o.e.m.",
    "to be filled by oem",
    "tobefilledbyoem",
    "none",
    "unknown",
    "not specified",
    "not available",
    "n/a",
    "system serial number",
    "serial number",
    "0",
    "00000000",
    "1234567890",
];

/// Is this identifier usable for fingerprinting? (non-empty after trim,
/// and not one of the known filler values OEMs ship instead of a serial)
fn usable(value: &str) -> bool {
    let v = value.trim();
    !v.is_empty() && !FILLER_VALUES.iter().any(|f| v.eq_ignore_ascii_case(f))
}

/// The fail-closed fingerprint derivation: requires >= 2 of 3 usable
/// identifiers, then hashes the usable components (normalized by trim).
/// Exposed separately so tests can exercise the degenerate-identifier
/// matrix without owning the hardware that produces it.
fn effective_fingerprint(cpu_id: &str, board_sn: &str, bios_sn: &str) -> Result<String, String> {
    let components = [("cpu", cpu_id), ("baseboard", board_sn), ("bios", bios_sn)];
    let usable_count = components.iter().filter(|(_, v)| usable(v)).count();
    if usable_count < 2 {
        return Err(format!(
            "This machine reports {} usable hardware identifier(s) (CPU/baseboard/BIOS); \
             at least 2 are required for license binding. Blank or placeholder serials are \
             common on virtual machines and some OEM boards. For a VM, set a unique baseboard \
             serial; for physical hardware with a blank serial, contact the software company \
             for a manual license-issuance procedure.",
            usable_count
        ));
    }
    let mut hasher = Sha256::new();
    hasher.update(b"vitalflow-hms-fp-v1\0");
    hasher.update(cpu_id.trim().as_bytes());
    hasher.update(b"\0");
    hasher.update(board_sn.trim().as_bytes());
    hasher.update(b"\0");
    hasher.update(bios_sn.trim().as_bytes());
    Ok(hex::encode(hasher.finalize()))
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

    let com = COMLibrary::new().map_err(|e| format!("WMI COM init failed: {}", e))?;
    let wmi = WMIConnection::new(com).map_err(|e| format!("WMI connection failed: {}", e))?;

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

    effective_fingerprint(&cpu_id, &board_sn, &bios_sn)
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

    // ── Licensing review (2026-09-05): fail-closed degenerate matrices ──

    use super::effective_fingerprint;

    #[test]
    fn fp_all_blank_identifiers_rejected() {
        // The classic VM/OEM failure: all three identifiers blank.
        // Previously this hashed to ONE fixed value — every degenerate
        // machine in the world shared a single fingerprint.
        assert!(effective_fingerprint("", "", "").is_err());
    }

    #[test]
    fn fp_whitespace_only_identifiers_rejected() {
        assert!(effective_fingerprint("   ", " \t ", "").is_err());
    }

    #[test]
    fn fp_only_one_usable_identifier_rejected() {
        // 1 of 3 usable → fail closed.
        assert!(effective_fingerprint("BFEBFBFF000906EA", "", "").is_err());
    }

    #[test]
    fn fp_filler_values_rejected() {
        // OEM placeholder strings must NOT count as usable identifiers —
        // fleets of identical white-box machines ship with these.
        assert!(effective_fingerprint("BFEBFBFF000906EA", "Default string", "").is_err());
        assert!(effective_fingerprint("BFEBFBFF000906EA", "To Be Filled By O.E.M.", "").is_err());
        assert!(effective_fingerprint("BFEBFBFF000906EA", "None", "").is_err());
        // BIOS filler + CPU only = still 1 usable → rejected.
        assert!(effective_fingerprint("BFEBFBFF000906EA", "Unknown", "N/A").is_err());
    }

    #[test]
    fn fp_two_usable_identifiers_accepted() {
        // 2 of 3 usable → OK, produces a stable hex hash.
        let fp = effective_fingerprint("BFEBFBFF000906EA", "MB-12345", "")
            .expect("2 usable identifiers must pass");
        assert_eq!(fp.len(), 64, "fingerprint must be SHA-256 hex");
    }

    #[test]
    fn fp_distinct_blanks_do_not_collide() {
        // Two DIFFERENT machines, each with one blank serial — the original
        // bug class. With fail-closed, neither gets a fingerprint at all,
        // so they can never share one.
        let a = effective_fingerprint("CPU-A", "", "");
        let b = effective_fingerprint("CPU-B", "", "");
        assert!(a.is_err() && b.is_err());
    }

    #[test]
    fn fp_two_machines_with_two_usable_do_not_collide() {
        let a = effective_fingerprint("CPU-A", "MB-A", "").expect("a");
        let b = effective_fingerprint("CPU-B", "MB-B", "").expect("b");
        assert_ne!(a, b, "different hardware must yield different fingerprints");
    }
}
