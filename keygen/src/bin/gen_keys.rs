//! gen_keys — generate an Ed25519 keypair for signing VitalFlow HMS licenses.
//!
//! Produces four files in the output directory (default: current directory):
//!   private_key.pem  + private_key.bin   — 32-byte Ed25519 secret seed
//!   public_key.pem   + public_key.bin    — 32-byte Ed25519 verifying key
//!
//! Also prints to stdout:
//!   - The public key as a Rust array literal, in the EXACT format used by
//!     `src-tauri/src/license.rs:49-54` (`COMPANY_PUBLIC_KEY`), ready to
//!     copy-paste.
//!   - The SHA-256 fingerprint of the public key, matching
//!     `license::get_license_public_key_fingerprint` (so you can confirm
//!     the key embedded in the app matches the key you just generated).
//!
//! SECURITY: the private key is the ONLY way to sign customer licenses.
//! NEVER commit it, NEVER ship it with the app, NEVER put it on a
//! networked machine. Store offline (encrypted USB / HSM / password-
//! manager vault). See README.md for the full operational workflow.

use base64::Engine as _;
use clap::Parser;
use ed25519_dalek::SigningKey;
use rand_core::OsRng;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

// Custom PEM headers — NOT standard PKCS#8, but clearly labelled and trivial
// to parse back in `sign_license`. The body is base64 of the raw 32 bytes.
const PEM_PRIVATE_HEADER: &str = "-----BEGIN VITALFLOW ED25519 PRIVATE KEY-----";
const PEM_PRIVATE_FOOTER: &str = "-----END VITALFLOW ED25519 PRIVATE KEY-----";
const PEM_PUBLIC_HEADER: &str = "-----BEGIN VITALFLOW ED25519 PUBLIC KEY-----";
const PEM_PUBLIC_FOOTER: &str = "-----END VITALFLOW ED25519 PUBLIC KEY-----";

#[derive(Parser)]
#[command(
    name = "gen_keys",
    about = "Generate an Ed25519 keypair for signing VitalFlow HMS licenses"
)]
struct Args {
    /// Directory to write the key files into (created if it doesn't exist).
    #[arg(long, default_value = ".")]
    out_dir: PathBuf,

    /// Overwrite existing key files. Refused by default to prevent
    /// accidentally destroying a production private key (which would
    /// invalidate every license ever signed with it).
    #[arg(long)]
    force: bool,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("[gen_keys] Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse();

    let priv_pem = args.out_dir.join("private_key.pem");
    let priv_bin = args.out_dir.join("private_key.bin");
    let pub_pem = args.out_dir.join("public_key.pem");
    let pub_bin = args.out_dir.join("public_key.bin");

    // Refuse to overwrite by default — losing a production private key is
    // catastrophic (every license signed with it becomes unverifiable).
    if !args.force {
        for f in [&priv_pem, &priv_bin, &pub_pem, &pub_bin] {
            if f.exists() {
                return Err(format!(
                    "Refusing to overwrite existing key file: {}\n\
                     Pass --force to overwrite. WARNING: overwriting the private key\n\
                     invalidates EVERY license signed with the old key — only do this\n\
                     if you are certain the old key is retired and no production\n\
                     licenses depend on it.",
                    f.display()
                ));
            }
        }
    }

    fs::create_dir_all(&args.out_dir)
        .map_err(|e| format!("Cannot create output directory {}: {}", args.out_dir.display(), e))?;

    // ── Generate the keypair using the OS CSPRNG ────────────────────────────
    let mut rng = OsRng;
    let signing_key = SigningKey::generate(&mut rng);
    let verifying_key = signing_key.verifying_key();
    let private_bytes = signing_key.to_bytes();  // [u8; 32]
    let public_bytes = verifying_key.to_bytes(); // [u8; 32]

    // ── Write raw 32-byte binary files ──────────────────────────────────────
    fs::write(&priv_bin, private_bytes)
        .map_err(|e| format!("Cannot write {}: {}", priv_bin.display(), e))?;
    fs::write(&pub_bin, public_bytes)
        .map_err(|e| format!("Cannot write {}: {}", pub_bin.display(), e))?;

    // ── Write PEM files (base64 of the 32 raw bytes, custom headers) ────────
    write_pem(&priv_pem, PEM_PRIVATE_HEADER, PEM_PRIVATE_FOOTER, &private_bytes)?;
    write_pem(&pub_pem, PEM_PUBLIC_HEADER, PEM_PUBLIC_FOOTER, &public_bytes)?;

    // ── Restrict private-key file permissions to owner-only on Unix ─────────
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::Permissions::from_mode(0o600);
        fs::set_permissions(&priv_pem, mode.clone())
            .map_err(|e| format!("Cannot set permissions on {}: {}", priv_pem.display(), e))?;
        fs::set_permissions(&priv_bin, mode)
            .map_err(|e| format!("Cannot set permissions on {}: {}", priv_bin.display(), e))?;
    }

    // ── Print the public key as a Rust array literal ───────────────────────
    // Exact format of license.rs:49-54: 8 bytes per line, lowercase 0x hex,
    // comma + space separator, trailing comma on every line, 4-space indent.
    println!();
    println!("================================================================");
    println!("Public key as a Rust array literal");
    println!("Paste into src-tauri/src/license.rs, replacing COMPANY_PUBLIC_KEY:");
    println!("================================================================");
    println!("pub const COMPANY_PUBLIC_KEY: [u8; 32] = [");
    for chunk in public_bytes.chunks(8) {
        let parts: Vec<String> = chunk.iter().map(|b| format!("0x{:02x}", b)).collect();
        println!("    {},", parts.join(", "));
    }
    println!("];");
    println!();

    // ── Print the SHA-256 fingerprint of the public key ────────────────────
    // Matches license::get_license_public_key_fingerprint (hex of SHA-256
    // over the 32 raw public-key bytes). After pasting the array literal into
    // license.rs and rebuilding the app, the Settings → License panel should
    // display this same fingerprint.
    let mut hasher = Sha256::new();
    hasher.update(public_bytes);
    let pubkey_fp = hex::encode(hasher.finalize());
    println!("================================================================");
    println!("Public key SHA-256 fingerprint");
    println!("(matches license::get_license_public_key_fingerprint in the app)");
    println!("================================================================");
    println!("{}", pubkey_fp);
    println!();

    // ── Summary + security warning to stderr ───────────────────────────────
    eprintln!("[gen_keys] Keypair generated:");
    eprintln!("[gen_keys]   Private key:  {}  +  {}", priv_pem.display(), priv_bin.display());
    eprintln!("[gen_keys]   Public key:   {}  +  {}", pub_pem.display(), pub_bin.display());
    eprintln!();
    eprintln!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
    eprintln!("!!  SECURITY WARNING                                                          !!");
    eprintln!("!!                                                                            !!");
    eprintln!("!!  The private key files (private_key.pem / private_key.bin) are the ONLY   !!");
    eprintln!("!!  way to sign customer licenses. ANYONE who obtains the private key can    !!");
    eprintln!("!!  issue forged licenses for your ENTIRE customer base.                     !!");
    eprintln!("!!                                                                            !!");
    eprintln!("!!  - NEVER commit these files to version control. (.gitignore covers them   !!");
    eprintln!("!!    — but double-check before every `git add`.)                            !!");
    eprintln!("!!  - NEVER ship them with the application, installer, or any customer       !!");
    eprintln!("!!    deliverable.                                                            !!");
    eprintln!("!!  - Store them OFFLINE: encrypted USB, HSM, or a password-manager vault.   !!");
    eprintln!("!!  - Back them up. Losing the private key means you can NEVER issue or      !!");
    eprintln!("!!    renew licenses without rotating the embedded public key (which forces  !!");
    eprintln!("!!    every customer to upgrade the app).                                     !!");
    eprintln!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");

    Ok(())
}

/// Write a 32-byte key as a custom PEM file: header line, single base64 line,
/// footer line. Not standard PKCS#8 — just a clearly-labelled wrapper so the
/// key is human-inspectable and easy to parse back in `sign_license`.
fn write_pem(path: &PathBuf, header: &str, footer: &str, bytes: &[u8]) -> Result<(), String> {
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    let content = format!("{}\n{}\n{}\n", header, b64, footer);
    fs::write(path, &content)
        .map_err(|e| format!("Cannot write {}: {}", path.display(), e))?;
    Ok(())
}
