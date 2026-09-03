//! get_fingerprint — compute the hardware fingerprint of the current
//! machine, for binding VitalFlow HMS licenses.
//!
//! This is a STANDALONE version of `fingerprint::compute()` from the main
//! app (`src-tauri/src/fingerprint.rs`). Customers run this binary on their
//! designated server PC and send the 64-character hex output to the
//! software company, which includes it in the signed license.
//!
//! The algorithm is COPIED VERBATIM from fingerprint.rs so the fingerprint
//! produced here is byte-identical to what `license::verify_license_file`
//! computes on the same machine. If fingerprint.rs changes, update this
//! file too.
//!
//! Windows-only in production (the app is Windows-only per SRS §1.4). On
//! non-Windows the tool exits with an error by default; pass
//! `--insecure-dev-fallback` to compute a non-production hostname-based
//! fingerprint (useful for testing the sign → verify round-trip on a dev
//! machine — NEVER use for real customer licenses).

use clap::Parser;
use sha2::{Digest, Sha256};

#[derive(Parser)]
#[command(
    name = "get_fingerprint",
    about = "Compute the hardware fingerprint of this machine (for VitalFlow HMS license binding)"
)]
struct Args {
    /// On non-Windows, compute a hostname-based dev fingerprint instead of
    /// exiting with an error. DEV/TEST ONLY — the result is NOT a production
    /// fingerprint and must NEVER appear in a real customer license.
    #[arg(long)]
    insecure_dev_fallback: bool,

    /// Print the individual hardware component values (CPU id, baseboard
    /// serial, BIOS serial) to stderr for debugging fingerprint mismatches.
    /// NOTE: these are hardware serial numbers — treat the verbose output as
    /// sensitive and send it only to your software vendor over a secure channel.
    #[arg(long)]
    verbose: bool,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("[get_fingerprint] Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse();

    #[cfg(target_os = "windows")]
    {
        let fp = compute_windows(args.verbose)?;
        // stdout: ONLY the 64-char hex fingerprint (so it can be piped /
        // redirected cleanly). Everything else goes to stderr.
        println!("{}", fp);
        return Ok(());
    }

    #[cfg(not(target_os = "windows"))]
    {
        if args.insecure_dev_fallback {
            let fp = compute_fallback();
            eprintln!("[get_fingerprint] WARNING: using non-Windows dev fallback.");
            eprintln!("[get_fingerprint] This fingerprint is NOT valid for production licenses.");
            eprintln!("[get_fingerprint] Use it only to test the sign_license -> verify_license");
            eprintln!("[get_fingerprint] round-trip on a dev machine.");
            println!("{}", fp);
            return Ok(());
        }
        return Err(
            "This tool is Windows-only. VitalFlow HMS production deployments run on Windows\n\
             (SRS section 1.4). On a dev/test machine, pass --insecure-dev-fallback to compute\n\
             a non-production hostname-based fingerprint (for testing the sign -> verify\n\
             round-trip only — never use it in a real customer license)."
                .to_string(),
        );
    }
}

// ── Windows fingerprint (verbatim from src-tauri/src/fingerprint.rs) ──────────
//
// SHA-256 over: b"vitalflow-hms-fp-v1\0" + CPU ProcessorId + b"\0"
//               + baseboard SerialNumber + b"\0" + BIOS SerialNumber.
//
// These three identifiers survive OS updates and routine driver changes but
// change if the motherboard/CPU is replaced — exactly the stability profile
// we want for license binding.

#[cfg(target_os = "windows")]
fn compute_windows(verbose: bool) -> Result<String, String> {
    use wmi::{COMLibrary, WMIConnection};

    // Typed structs for WMI deserialization. The `wmi` crate requires
    // Deserialize structs (not raw serde_json::Value) — raw Value
    // deserialization fails with "Only structs and maps can be deserialized
    // from WMI objects". (Same caveat as fingerprint.rs.)
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

    if verbose {
        eprintln!("[get_fingerprint] CPU ProcessorId:  {:?}", cpu_id);
        eprintln!("[get_fingerprint] BaseBoard Serial: {:?}", board_sn);
        eprintln!("[get_fingerprint] BIOS Serial:      {:?}", bios_sn);
        eprintln!("[get_fingerprint] (treat the above as sensitive hardware serial numbers.)");
    }

    let mut hasher = Sha256::new();
    hasher.update(b"vitalflow-hms-fp-v1\0");
    hasher.update(cpu_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(board_sn.as_bytes());
    hasher.update(b"\0");
    hasher.update(bios_sn.as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

// ── Non-Windows dev fallback (verbatim algorithm from fingerprint.rs) ─────────
//
// NOT a production fingerprint. SHA-256 over hostname + OS + a fixed salt.
// Used only to test the sign -> verify round-trip on dev/build machines.

#[cfg(not(target_os = "windows"))]
fn compute_fallback() -> String {
    let hostname = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "unknown-host".to_string());
    let mut hasher = Sha256::new();
    hasher.update(b"vitalflow-hms-fp-dev-v1\0");
    hasher.update(hostname.as_bytes());
    hasher.update(b"\0");
    hasher.update(std::env::consts::OS.as_bytes());
    hex::encode(hasher.finalize())
}
