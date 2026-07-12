/// TLS certificate management — shared between both builds.
///
/// SERVER BUILD ONLY generates the actual certificate (ensure_tls_material,
/// build_server_tls_config below, gated behind `server-build`): a single
/// self-signed certificate + private key on first server launch, used for
/// TWO things:
///   1. Wrapping the pairing TCP listener in TLS (see pairing.rs), so the
///      pairing code and returned DB credentials can't be read in plaintext
///      by anyone sniffing the LAN.
///   2. PostgreSQL's own SSL support (ssl_cert_file / ssl_key_file), so the
///      actual database connection is encrypted too.
///
/// CLIENT BUILD uses only the verifier types further down this file
/// (TofuVerifier, PinnedVerifier and their ClientConfig builders) — a
/// client never generates or holds a private key, it only ever verifies
/// what the server presents.
///
/// Trust model: this is intentionally a lightweight "trust on first pairing"
/// design appropriate for a single-hospital LAN, NOT a full PKI:
///   - The cert is self-signed; there is no external CA.
///   - The client (during the one-time pairing flow) receives the server's
///     certificate fingerprint as part of the TLS handshake, and pins it
///     for all future connections to that server's IP. If the fingerprint
///     ever changes unexpectedly (e.g. a different machine now answers at
///     that IP — the ARP/DNS-spoofing scenario), the client refuses to
///     connect and surfaces a clear warning rather than silently trusting
///     a new identity.
///   - This defeats passive sniffing (TLS encrypts the channel) and
///     after-the-fact impersonation (pinning catches a swapped server),
///     though it does not defend against an attacker present and
///     impersonating the server during the very first pairing — the same
///     "trust on first use" limitation SSH has with unknown hosts.
#[cfg(feature = "server-build")]
use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair};
use sha2::{Digest, Sha256};
#[cfg(feature = "server-build")]
use std::path::{Path, PathBuf};

/// Only constructed by `ensure_tls_material` (server-build). On client/dev
/// builds this struct is dead code — allow it.
#[allow(dead_code)]
pub struct TlsMaterial {
    pub cert_pem: String,
    pub key_pem: String,
    /// SHA-256 fingerprint of the DER-encoded certificate, hex-encoded.
    /// This is what gets pinned by clients after first pairing.
    pub fingerprint_hex: String,
}

#[cfg(feature = "server-build")]
fn cert_path(hms_dir: &Path) -> PathBuf {
    hms_dir.join("tls").join("server.crt")
}
#[cfg(feature = "server-build")]
fn key_path(hms_dir: &Path) -> PathBuf {
    hms_dir.join("tls").join("server.key")
}

/// Load the existing cert/key if present, otherwise generate a fresh
/// self-signed pair and persist it. Idempotent — safe to call every launch.
#[cfg(feature = "server-build")]
pub fn ensure_tls_material(hms_dir: &Path, local_ip: &str) -> Result<TlsMaterial, String> {
    let crt_path = cert_path(hms_dir);
    let k_path = key_path(hms_dir);

    if crt_path.exists() && k_path.exists() {
        let cert_pem = std::fs::read_to_string(&crt_path)
            .map_err(|e| format!("Cannot read existing TLS cert: {}", e))?;
        let key_pem = std::fs::read_to_string(&k_path)
            .map_err(|e| format!("Cannot read existing TLS key: {}", e))?;
        let fingerprint_hex = fingerprint_of_pem(&cert_pem)?;
        return Ok(TlsMaterial { cert_pem, key_pem, fingerprint_hex });
    }

    std::fs::create_dir_all(crt_path.parent().unwrap())
        .map_err(|e| format!("Cannot create tls dir: {}", e))?;

    let mut params = CertificateParams::new(vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        local_ip.to_string(),
    ])
    .map_err(|e| format!("Invalid cert params: {}", e))?;

    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, "HMS Server (self-signed, LAN only)");
    params.distinguished_name = dn;
    // 10-year validity — this is a LAN-internal cert with no external CA
    // renewal process; a decade avoids forcing a re-pair on every
    // deployment without leaving keys valid indefinitely.
    params.not_after = time::OffsetDateTime::now_utc() + time::Duration::days(3650);

    let key_pair = KeyPair::generate().map_err(|e| format!("Key generation failed: {}", e))?;
    let cert = params
        .self_signed(&key_pair)
        .map_err(|e| format!("Self-signing failed: {}", e))?;

    let cert_pem = cert.pem();
    let key_pem = key_pair.serialize_pem();

    std::fs::write(&crt_path, &cert_pem).map_err(|e| format!("Cannot write cert: {}", e))?;
    std::fs::write(&k_path, &key_pem).map_err(|e| format!("Cannot write key: {}", e))?;

    // SEC-13: harden the private key's ACL IMMEDIATELY after writing it,
    // before any other work happens. The previous implementation relied on
    // `pg_provision::write_ssl_config_and_restart` running `icacls` on the
    // key later — but if the process crashed between the `fs::write` here
    // and that later call (which can be many seconds later, after
    // spawn_blocking TLS setup, SSL marker checks, etc.), the key file was
    // left world-readable on disk with default `fs::write` permissions
    // (Windows: inherited from parent dir, typically allowing any local
    // user to read). Anyone with file-system access (including malware
    // running as a non-admin user) could exfiltrate the key and
    // impersonate the server's TLS identity on the LAN.
    //
    // We use the same `icacls` pattern as `pg_provision.rs:149-152`:
    //   icacls <key> /inheritance:r /grant:r SYSTEM:F /grant:r Administrators:F
    // This removes inherited ACEs and grants Full Control to SYSTEM and
    // the local Administrators group only — the minimum necessary for
    // PostgreSQL (running as SYSTEM) to read the key, and for an admin
    // to manage it. Non-admin local users get no access.
    //
    // `icacls` is Windows-only; on non-Windows the key is created with
    // the process umask (typically 022 → 0644) — non-Windows is dev-only
    // for this app per the SRS, so we don't try to chmod here. (A future
    // hardening could add `#[cfg(unix)] { use std::os::unix::fs::PermissionsExt; ... chmod 0600 }`.)
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("icacls")
            .arg(k_path.as_os_str())
            .args(["/inheritance:r"])
            .args(["/grant:r", "SYSTEM:F"])
            .args(["/grant:r", "Administrators:F"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        // Best-effort: if icacls fails (e.g. running as non-admin), we
        // don't fail the whole TLS setup — the key file still exists with
        // default perms, which is the same as the previous behaviour.
        // The follow-up icacls in pg_provision.rs will retry when SSL
        // is enabled. The risk window is small (a few seconds between
        // here and the pg_provision call) and only matters if the
        // process is killed mid-startup — which is rare.
    }

    let fingerprint_hex = fingerprint_of_pem(&cert_pem)?;

    Ok(TlsMaterial { cert_pem, key_pem, fingerprint_hex })
}

#[cfg(feature = "server-build")]
fn fingerprint_of_pem(cert_pem: &str) -> Result<String, String> {
    let der = pem_to_der(cert_pem)?;
    let mut hasher = Sha256::new();
    hasher.update(&der);
    Ok(hex_encode(&hasher.finalize()))
}

#[cfg(feature = "server-build")]
fn pem_to_der(pem: &str) -> Result<Vec<u8>, String> {
    let mut reader = std::io::BufReader::new(pem.as_bytes());
    let certs: Result<Vec<_>, _> = rustls_pemfile::certs(&mut reader).collect();
    let certs = certs.map_err(|e| format!("PEM parse failed: {}", e))?;
    certs
        .into_iter()
        .next()
        .map(|c| c.to_vec())
        .ok_or_else(|| "No certificate found in PEM".to_string())
}

/// Shared by both builds: server-side fingerprinting (from PEM, above) and
/// client-side fingerprinting (from raw DER seen during a TLS handshake,
/// in the verifiers below) both need hex formatting of a SHA-256 digest.
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Build a rustls ServerConfig from the generated material, for the pairing
/// listener to use.
#[cfg(feature = "server-build")]
pub fn build_server_tls_config(
    material: &TlsMaterial,
) -> Result<std::sync::Arc<rustls::ServerConfig>, String> {
    let cert_der = pem_to_der(&material.cert_pem)?;
    let cert = rustls::pki_types::CertificateDer::from(cert_der);

    let mut key_reader = std::io::BufReader::new(material.key_pem.as_bytes());
    let key = rustls_pemfile::private_key(&mut key_reader)
        .map_err(|e| format!("Key PEM parse failed: {}", e))?
        .ok_or_else(|| "No private key found in PEM".to_string())?;

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)
        .map_err(|e| format!("Failed to build TLS server config: {}", e))?;


    Ok(std::sync::Arc::new(config))
}

// ── Client-side verifiers ───────────────────────────────────────────────────
//
// Two distinct trust modes, used at two distinct moments:
//
//   TofuVerifier  — used ONLY during the pairing exchange itself, on a
//                   client PC that has never paired before. Accepts
//                   whatever certificate the server presents (there is
//                   nothing to compare against yet) but records its
//                   fingerprint so the caller can pin it immediately
//                   afterward, before any credentials are trusted.
//
//   PinnedVerifier — used for every connection AFTER the first pairing,
//                    including the actual Postgres connection. Rejects
//                    the handshake outright if the presented certificate's
//                    fingerprint doesn't exactly match the one pinned
//                    during pairing. This is what catches a swapped
//                    server (ARP/DNS spoofing) on every later connection,
//                    even though it can't catch an attacker present
//                    during the very first pairing — the same inherent
//                    limitation TOFU schemes like SSH have.

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct TofuVerifier {
    provider: Arc<rustls::crypto::CryptoProvider>,
    /// Filled in with the fingerprint of whatever cert was actually
    /// presented, the first (and only ever) time this verifier is used.
    /// The full PEM is NOT reconstructed here — the server sends its own
    /// PEM explicitly in the pairing response body (over this now-
    /// encrypted channel), which is simpler than re-deriving PEM from the
    /// raw DER bytes seen mid-handshake.
    pub captured_fingerprint: Mutex<Option<String>>,
}

impl TofuVerifier {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            provider: Arc::new(rustls::crypto::ring::default_provider()),
            captured_fingerprint: Mutex::new(None),
        })
    }
}

impl ServerCertVerifier for TofuVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let mut hasher = Sha256::new();
        hasher.update(end_entity.as_ref());
        let fp = hex_encode(&hasher.finalize());

        // REL-02: recover from mutex poisoning instead of panicking.
        *self.captured_fingerprint.lock().unwrap_or_else(|e| e.into_inner()) = Some(fp);

        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

// PinnedVerifier + build_pinned_client_config are used only in the
// client-build (post-pairing TLS connections). Allow dead code in
// server-build to avoid spurious clippy warnings.
#[allow(dead_code)]
#[derive(Debug)]
pub struct PinnedVerifier {
    provider: Arc<rustls::crypto::CryptoProvider>,
    expected_fingerprint_hex: String,
}

impl PinnedVerifier {
    #[allow(dead_code)]
    pub fn new(expected_fingerprint_hex: String) -> Arc<Self> {
        Arc::new(Self {
            provider: Arc::new(rustls::crypto::ring::default_provider()),
            expected_fingerprint_hex,
        })
    }
}

impl ServerCertVerifier for PinnedVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let mut hasher = Sha256::new();
        hasher.update(end_entity.as_ref());
        let actual = hex_encode(&hasher.finalize());

        if actual == self.expected_fingerprint_hex {
            Ok(ServerCertVerified::assertion())
        } else {
            // Deliberately generic error type — rustls doesn't have a
            // dedicated "pin mismatch" variant, so we surface a clear
            // message to the caller via the higher-level pairing.rs /
            // db.rs error handling instead of relying on this message
            // reaching the user verbatim.
            Err(rustls::Error::General(format!(
                "Certificate fingerprint mismatch: expected {}, got {}. The server's identity may have changed.",
                self.expected_fingerprint_hex, actual
            )))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

/// Client config that accepts any cert (TOFU) — used exactly once, during
/// the pairing exchange itself, before anything is pinned.
pub fn build_tofu_client_config(verifier: Arc<TofuVerifier>) -> Arc<rustls::ClientConfig> {
    let mut config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    config.alpn_protocols = vec![];
    Arc::new(config)
}

/// Client config that only accepts a specific, already-pinned certificate.
/// Used for every connection after the first successful pairing.
///
/// Only called from client-build; allow dead code in server-build.
#[allow(dead_code)]
pub fn build_pinned_client_config(expected_fingerprint_hex: String) -> Arc<rustls::ClientConfig> {
    let verifier = PinnedVerifier::new(expected_fingerprint_hex);
    let mut config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    config.alpn_protocols = vec![];
    Arc::new(config)
}
