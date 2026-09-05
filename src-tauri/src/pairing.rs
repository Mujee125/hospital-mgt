/// Secure credential pairing over the LAN — TLS-wrapped.
///
/// Flow:
///   1. Server generates a short-lived 6-char code.
///   2. Client enters server IP + code into setup screen.
///   3. Client connects to server's TLS pairing port (42011), sends the code.
///   4. Server validates the code and returns DB credentials + its own TLS
///      certificate PEM (over the now-encrypted channel).
///   5. Client pins the fingerprint and saves all credentials to config.json.
///   6. Client immediately verifies the saved credentials work by attempting
///      a real PostgreSQL connection — so "Save & continue" only proceeds
///      when we KNOW the DB is reachable and the cert/password are correct.
///
/// Trust model: TOFU on first pairing (see tls_provision.rs for full notes).
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

use crate::tls_provision;

// Server-build-only imports: these are used by the pairing LISTENER which
// only runs on the server build. Gating them avoids unused-import warnings
// on client/dev builds.
#[cfg(feature = "server-build")]
use std::net::SocketAddr;
#[cfg(feature = "server-build")]
use tokio::net::TcpListener;
#[cfg(feature = "server-build")]
use tokio_rustls::TlsAcceptor;
#[cfg(feature = "server-build")]
use crate::tls_provision::TlsMaterial;

pub const PAIRING_PORT: u16 = 42011;
const CODE_TTL_SECS: u64 = 600; // 10 minutes
// SEC-03: a pairing code is intended for ONE client. Allowing 10 different
// machines to consume the same code turns a leaked code into 10 leaked
// credential sets. 3 is the safe ceiling (operator retries + minor typo
// attempts) without enabling mass credential leakage.
const MAX_USES: u32 = 3;
const MAX_LINE_BYTES: usize = 4096;
const CONNECTION_DEADLINE: Duration = Duration::from_secs(10);

// SEC-03: per-peer-IP brute-force protection.
//
// Threat: the 6-char code over a 32-symbol alphabet is ~30 bits of entropy.
// An attacker on the LAN who can connect to port 42011 and submit codes
// could previously brute-force at line-rate. The TLS handshake slows them
// to ~10-50 attempts/sec/connection, but with no per-IP tracking the
// listener would happily try every code forever.
//
// Mitigation: track failed `try_consume_with_peer` attempts per source IP.
// After MAX_FAILED_ATTEMPTS_PER_PEER failures within
// FAILED_ATTEMPT_WINDOW_SECS, lock that IP out for LOCKOUT_DURATION_SECS.
// `try_consume_with_peer` returns `LockedOut` immediately for a locked IP
// without even checking the submitted code, so the brute-force rate is
// capped at MAX_FAILED_ATTEMPTS_PER_PEER per FAILED_ATTEMPT_WINDOW_SECS
// per attacker IP — i.e. ~3 attempts / 5 min = ~860 attempts/day per IP,
// which is nowhere near the 30-bit keyspace.
// SEC-03 constants are only used by the server-side pairing listener.
#[cfg(feature = "server-build")]
const MAX_FAILED_ATTEMPTS_PER_PEER: u32 = 3;
#[cfg(feature = "server-build")]
const FAILED_ATTEMPT_WINDOW_SECS: u64 = 300;   // 5-minute sliding window
#[cfg(feature = "server-build")]
const LOCKOUT_DURATION_SECS: u64 = 900;        // 15-minute lockout

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PairingResponse {
    db_user: String,
    db_password: String,
    db_name: String,
    db_port: u16,
    /// Server's own TLS cert PEM — sent over the encrypted channel so the
    /// client can pin it for the subsequent Postgres SSL connection.
    server_cert_pem: String,
}

#[cfg(feature = "server-build")]
#[derive(Debug, Deserialize)]
struct PairingRequest {
    code: String,
}

// PairingState is used by both server and client builds (generate_code is
// a Tauri command on both). The peer_failures field is only read by the
// server-side listener — allow(dead_code) on the field for non-server builds.
#[allow(dead_code)]
struct PairingState {
    code: String,
    expires_at: Instant,
    uses_remaining: u32,
    // SEC-03: per-peer-IP brute-force tracking. Lives inside the same Mutex
    // as the code/uses_remaining so the consume check + failure increment
    // are atomic with respect to other concurrent pairing connections.
    #[allow(dead_code)]
    peer_failures: HashMap<IpAddr, PeerFailureTracker>,
}

// SEC-03: per-peer failure record. `first_failure_at` is the start of the
// current 5-minute sliding window; `locked_until` is set when
// `failed_attempts >= MAX_FAILED_ATTEMPTS_PER_PEER`.
// SEC-03: per-peer failure record. Only read by the server-side pairing
// listener (try_consume_with_peer). On client/dev builds the fields are
// dead code — allow it (the struct itself is needed for PairingState).
#[allow(dead_code)]
#[derive(Default)]
struct PeerFailureTracker {
    failed_attempts: u32,
    first_failure_at: Option<Instant>,
    locked_until: Option<Instant>,
}

/// Outcome of a `try_consume_with_peer` call.
///
/// `Accepted`     — code matched; uses_remaining decremented; peer's
///                  failure record (if any) cleared.
/// `Rejected`     — code did NOT match (or expired / no uses remaining);
///                  peer's failure counter incremented (may now be locked).
/// `LockedOut`    — peer IP has exceeded MAX_FAILED_ATTEMPTS_PER_PEER and
///                  is in LOCKOUT_DURATION_SECS. The code was NOT checked
///                  (so this is safe to return even after the code has
///                  rotated; no information about the current code leaks).
#[cfg(feature = "server-build")]
enum ConsumeResult {
    Accepted,
    Rejected,
    LockedOut,
}

#[derive(Clone)]
pub struct PairingService {
    state: Arc<Mutex<Option<PairingState>>>,
}

impl PairingService {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(None)),
        }
    }

    /// Generate a fresh 6-character code (uppercase letters + digits,
    /// excluding visually-ambiguous characters: 0/O, 1/I).
    pub fn generate_code(&self) -> String {
        const CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
        // SEC-03 / SDD-12 / M-04: use OsRng (CSPRNG) instead of thread_rng().
        // OsRng reads from the OS CSPRNG (RtlGenRandom / getrandom) on every
        // call, so the generated code is unpredictable even to an attacker
        // who has observed prior codes. thread_rng() is a userspace PRNG
        // seeded once per thread — adequate for non-security use but not
        // for a credential that unlocks DB credentials over the LAN.
        let mut rng = OsRng;
        let mut idx_bytes = [0u8; 6];
        rng.fill_bytes(&mut idx_bytes);
        let code: String = (0..6)
            .map(|i| {
                let idx = (idx_bytes[i] as usize) % CHARSET.len();
                CHARSET[idx] as char
            })
            .collect();

        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        *guard = Some(PairingState {
            code: code.clone(),
            expires_at: Instant::now() + Duration::from_secs(CODE_TTL_SECS),
            uses_remaining: MAX_USES,
            peer_failures: HashMap::new(),
        });
        code
    }

    /// How many seconds remain on the current code, if any is active.
    pub fn remaining_seconds(&self) -> Option<u64> {
        let guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        guard.as_ref().and_then(|s| {
            let now = Instant::now();
            if s.expires_at > now {
                Some((s.expires_at - now).as_secs())
            } else {
                None
            }
        })
    }

    /// SEC-03: attempt to consume the submitted code, attributing the
    /// attempt to `peer_ip` for per-IP brute-force protection.
    ///
    /// See `ConsumeResult` for the three possible outcomes. The peer's
    /// failure counter is incremented on every `Rejected` result; once it
    /// reaches MAX_FAILED_ATTEMPTS_PER_PEER within the sliding window, all
    /// subsequent calls from that IP return `LockedOut` for
    /// LOCKOUT_DURATION_SECS — the code is NOT checked during lockout, so
    /// no information about the current code leaks to the attacker.
    #[cfg(feature = "server-build")]
    fn try_consume_with_peer(&self, submitted_code: &str, peer_ip: &IpAddr) -> ConsumeResult {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let state = match guard.as_mut() {
            Some(s) => s,
            None => return ConsumeResult::Rejected,
        };
        let now = Instant::now();

        // Lazy GC: drop stale peer-failure entries so the map can't grow
        // unbounded across the server process lifetime. An entry is kept
        // only if it is still locked OR within the sliding window.
        state.peer_failures.retain(|_, f| {
            let still_locked = f.locked_until.map(|lu| lu > now).unwrap_or(false);
            let in_window = f
                .first_failure_at
                .map(|t| now.duration_since(t).as_secs() < FAILED_ATTEMPT_WINDOW_SECS)
                .unwrap_or(false);
            still_locked || in_window
        });

        let entry = state.peer_failures.entry(*peer_ip).or_default();

        // 1. Active lockout?
        if let Some(lu) = entry.locked_until {
            if lu > now {
                return ConsumeResult::LockedOut;
            } else {
                // Lockout expired — reset and continue to code check.
                entry.failed_attempts = 0;
                entry.first_failure_at = None;
                entry.locked_until = None;
            }
        }

        // 2. Sliding window: if first_failure is older than the window,
        //    reset the counter so a slow trickle of failures from a peer
        //    (e.g. the legitimate operator mistyping once an hour) never
        //    accidentally trips the lockout.
        if let Some(t) = entry.first_failure_at {
            if now.duration_since(t).as_secs() >= FAILED_ATTEMPT_WINDOW_SECS {
                entry.failed_attempts = 0;
                entry.first_failure_at = None;
            }
        }

        // 3. Code match check (also enforces expiry + uses_remaining).
        let accepted = state.expires_at > now
            && state.uses_remaining > 0
            && state.code == submitted_code;

        if accepted {
            state.uses_remaining -= 1;
            // Clear the peer's failure record on success — the legitimate
            // operator who fat-fingered the code twice before getting it
            // right should not carry a partial failure count forward.
            state.peer_failures.remove(peer_ip);
            ConsumeResult::Accepted
        } else {
            entry.failed_attempts += 1;
            if entry.first_failure_at.is_none() {
                entry.first_failure_at = Some(now);
            }
            if entry.failed_attempts >= MAX_FAILED_ATTEMPTS_PER_PEER {
                entry.locked_until = Some(now + Duration::from_secs(LOCKOUT_DURATION_SECS));
                ConsumeResult::LockedOut
            } else {
                ConsumeResult::Rejected
            }
        }
    }
}

/// Used by the server-side pairing listener to pass DB credentials back
/// to the client over the TLS channel. Only constructed on server-build.
#[cfg(feature = "server-build")]
#[derive(Clone)]
pub struct PairingCreds {
    pub db_user: String,
    pub db_password: String,
    pub db_name: String,
    pub db_port: u16,
}

// ── Server-side listener (server-build only) ──────────────────────────────────

#[cfg(feature = "server-build")]
pub fn start_pairing_listener(
    service: PairingService,
    creds: PairingCreds,
    tls_material: TlsMaterial,
    running: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    tokio::spawn(async move {
        let server_config = match tls_provision::build_server_tls_config(&tls_material) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("[HMS Pairing] Failed to build TLS config: {}", e);
                return;
            }
        };
        let acceptor = TlsAcceptor::from(server_config);

        let listener = match TcpListener::bind(format!("0.0.0.0:{}", PAIRING_PORT)).await {
            Ok(l) => l,
            Err(e) => {
                eprintln!(
                    "[HMS Pairing] Failed to bind pairing port {}: {}",
                    PAIRING_PORT, e
                );
                return;
            }
        };

        eprintln!("[HMS Pairing] Listening on port {}", PAIRING_PORT);

        // REL-03: cooperative shutdown. The accept loop wraps `listener.accept()`
        // in a 1-second `tokio::time::timeout` so the running flag is observed
        // within ~1 s of the app requesting exit. Without this, the accept
        // call would block indefinitely waiting for the next pairing request
        // and the task would never exit cleanly — leaving it to be killed
        // mid-handshake when the process finally exits, which could leave a
        // half-accepted TLS connection consuming a process file descriptor.
        loop {
            if !running.load(Ordering::Relaxed) {
                eprintln!("[HMS Pairing] Shutdown flag observed — exiting listener loop");
                break;
            }

            let accept_result = tokio::time::timeout(
                Duration::from_secs(1),
                listener.accept(),
            )
            .await;

            let (stream, peer) = match accept_result {
                Err(_) => continue, // timed out — loop back, re-check running flag
                Ok(Err(_)) => continue, // accept() error — non-fatal, try again
                Ok(Ok((s, p))) => (s, p),
            };
            eprintln!("[HMS Pairing] Incoming connection from {}", peer);

            let svc = service.clone();
            let creds = creds.clone();
            let acceptor = acceptor.clone();
            let cert_pem = tls_material.cert_pem.clone();

            tokio::spawn(async move {
                let _ = tokio::time::timeout(
                    CONNECTION_DEADLINE,
                    handle_connection(stream, peer, acceptor, svc, creds, cert_pem),
                )
                .await;
            });
        }
    });
}

#[cfg(feature = "server-build")]
async fn handle_connection(
    stream: TcpStream,
    peer: SocketAddr,
    acceptor: TlsAcceptor,
    service: PairingService,
    creds: PairingCreds,
    server_cert_pem: String,
) {
    let mut tls_stream = match acceptor.accept(stream).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[HMS Pairing] TLS handshake failed from {}: {}", peer, e);
            return;
        }
    };

    let mut buf = vec![0u8; MAX_LINE_BYTES];
    let n = match tls_stream.read(&mut buf).await {
        Ok(n) if n > 0 => n,
        _ => return,
    };

    let line = String::from_utf8_lossy(&buf[..n]);
    let request: Result<PairingRequest, _> = serde_json::from_str(line.trim());
    let request = match request {
        Ok(r) => r,
        Err(_) => {
            let _ =
                write_json_line(&mut tls_stream, &serde_json::json!({"error": "bad_request"}))
                    .await;
            return;
        }
    };

    // SEC-03: per-IP brute-force protection. The peer IP is the source of
    // the TCP connection (NOT the X-Forwarded-For header — there is no
    // proxy in front of the pairing listener, so the socket peer is the
    // true originator).
    let peer_ip = peer.ip();
    match service.try_consume_with_peer(&request.code, &peer_ip) {
        ConsumeResult::Accepted => {
            eprintln!("[HMS Pairing] Code accepted from {} — sending credentials", peer);
        }
        ConsumeResult::Rejected => {
            eprintln!("[HMS Pairing] Invalid/expired code from {}", peer);
            let _ = write_json_line(
                &mut tls_stream,
                &serde_json::json!({"error": "invalid_or_expired_code"}),
            )
            .await;
            return;
        }
        ConsumeResult::LockedOut => {
            eprintln!(
                "[HMS Pairing] Peer {} locked out after {} failed attempts — refusing",
                peer, MAX_FAILED_ATTEMPTS_PER_PEER
            );
            let _ = write_json_line(
                &mut tls_stream,
                &serde_json::json!({"error": "locked_out"}),
            )
            .await;
            return;
        }
    }

    let response = PairingResponse {
        db_user: creds.db_user,
        db_password: creds.db_password,
        db_name: creds.db_name,
        db_port: creds.db_port,
        server_cert_pem,
    };
    let _ = write_json_line(&mut tls_stream, &response).await;
    let _ = tls_stream.shutdown().await;
}

async fn write_json_line<S, T>(stream: &mut S, value: &T) -> std::io::Result<()>
where
    S: tokio::io::AsyncWrite + Unpin,
    T: Serialize,
{
    let mut text = serde_json::to_string(value).unwrap_or_default();
    text.push('\n');
    stream.write_all(text.as_bytes()).await
}

// ── Client-side: exchange code for credentials ────────────────────────────────

/// Connects to the server's pairing port over TLS (TOFU), sends the code,
/// and returns: (db_user, db_password, db_name, db_port, cert_pem, fingerprint).
pub async fn redeem_code(
    server_ip: &str,
    code: &str,
) -> Result<(String, String, String, u16, String, String), String> {
    let addr = format!("{}:{}", server_ip, PAIRING_PORT);
    let tcp_stream = tokio::time::timeout(CONNECTION_DEADLINE, TcpStream::connect(&addr))
        .await
        .map_err(|_| {
            format!(
                "Connection to {}:{} timed out. \
                 Check the server IP and that the reception PC is reachable.",
                server_ip, PAIRING_PORT
            )
        })?
        .map_err(|e| {
            format!(
                "Cannot reach server at {}:{} — {}. \
                 Make sure the HMS Server app is running on the reception PC.",
                server_ip, PAIRING_PORT, e
            )
        })?;

    let verifier = tls_provision::TofuVerifier::new();
    let client_config = tls_provision::build_tofu_client_config(verifier.clone());
    let connector = TlsConnector::from(client_config);

    // ServerName is a required rustls parameter but our custom TofuVerifier
    // ignores it — we verify by fingerprint, not hostname.
    let server_name = rustls_pki_types_server_name();

    let mut tls_stream =
        tokio::time::timeout(CONNECTION_DEADLINE, connector.connect(server_name, tcp_stream))
            .await
            .map_err(|_| "TLS handshake timed out.".to_string())?
            .map_err(|e| format!("TLS handshake failed: {}", e))?;

    let request = serde_json::json!({ "code": code.trim().to_uppercase() });
    write_json_line(&mut tls_stream, &request)
        .await
        .map_err(|e| format!("Failed to send pairing request: {}", e))?;

    let mut buf = vec![0u8; MAX_LINE_BYTES];
    let n = tokio::time::timeout(CONNECTION_DEADLINE, tls_stream.read(&mut buf))
        .await
        .map_err(|_| "No response from server (timed out).".to_string())?
        .map_err(|e| format!("No response from server: {}", e))?;

    if n == 0 {
        return Err("Server closed the connection without responding.".to_string());
    }

    let line = String::from_utf8_lossy(&buf[..n]);
    let value: serde_json::Value = serde_json::from_str(line.trim())
        .map_err(|_| "Malformed response from server".to_string())?;

    if let Some(err) = value.get("error") {
        return Err(match err.as_str() {
            Some("invalid_or_expired_code") => {
                "That pairing code is wrong or has expired. \
                 Ask reception to generate a fresh code."
                    .to_string()
            }
            // SEC-03: server-side per-IP lockout. The operator must wait
            // ~15 minutes before retrying (or have reception restart the
            // server to clear in-memory state — codes are short-lived).
            Some("locked_out") => {
                "Too many failed pairing attempts from this PC. \
                 Wait 15 minutes and try again, or ask reception to \
                 generate a fresh code."
                    .to_string()
            }
            _ => "The server rejected the pairing request.".to_string(),
        });
    }

    let response: PairingResponse = serde_json::from_value(value)
        .map_err(|_| "Unexpected response format from server".to_string())?;

    let fingerprint = verifier
        .captured_fingerprint
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .ok_or_else(|| "Could not capture server certificate fingerprint.".to_string())?;

    Ok((
        response.db_user,
        response.db_password,
        response.db_name,
        response.db_port,
        response.server_cert_pem,
        fingerprint,
    ))
}

fn rustls_pki_types_server_name() -> rustls::pki_types::ServerName<'static> {
    rustls::pki_types::ServerName::try_from("localhost").expect("static value is always valid")
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn generate_pairing_code(
    service: tauri::State<'_, PairingService>,
) -> Result<String, String> {
    Ok(service.generate_code())
}

#[tauri::command]
pub async fn get_pairing_status(
    service: tauri::State<'_, PairingService>,
) -> Result<Option<u64>, String> {
    Ok(service.remaining_seconds())
}

/// Step 1 of client setup: exchange the pairing code for credentials and
/// save them to config.json. This does NOT yet attempt a DB connection —
/// that happens in verify_pairing (step 2), which the frontend calls when
/// the user presses "Save & continue".
///
/// Separating the two steps means the UI can show a clear "saving…" spinner
/// and then a separate "connecting to database…" step with its own error
/// message, rather than one opaque operation that can fail for two very
/// different reasons.
#[tauri::command]
pub async fn redeem_pairing_code(
    app_handle: tauri::AppHandle,
    session_state: tauri::State<'_, crate::rbac::SessionState>,
    server_ip: String,
    code: String,
) -> Result<serde_json::Value, String> {
    // ── Phase 4 review, S-2 (2026-09-05): this command WRITES config and
    // previously bypassed the config-mutation gate entirely. On a CONFIGURED
    // machine a pre-login webview could call it (pairing commands were
    // NO_GUARD) and overwrite the local config — mode→client, host→an
    // attacker-controlled IP, credentials→the attacker's — so the next
    // launch would connect to the attacker's database. The tri-state gate
    // preserves every legitimate flow: client first-run (config Missing →
    // FirstRun allowed), client re-setup (goes through clear_config first,
    // so the disk is Missing again), and an admin re-pairing a configured
    // machine (signed-in SettingsManage session → Authorized).
    let disk = crate::config::AppConfig::disk_config_state(&app_handle);
    match crate::rbac::require_config_mutation(
        &session_state,
        disk,
        crate::rbac::Permission::SettingsManage,
    )? {
        crate::rbac::ConfigMutationGrant::FirstRun => { /* first-run setup */ }
        crate::rbac::ConfigMutationGrant::Authorized(_) => { /* admin re-pairing */ }
    }

    let (db_user, db_password, db_name, db_port, server_cert_pem, fingerprint) =
        redeem_code(&server_ip, &code).await?;

    // ── Persist credentials + pin atomically ─────────────────────────────
    // We always start from the loaded config (or default) and overwrite only
    // the fields we received from the server, so any manually-set clinic_name
    // etc. is preserved.
    let mut cfg = crate::config::AppConfig::load(&app_handle).unwrap_or_default();
    cfg.mode                       = "client".to_string();
    cfg.db_host                    = server_ip.clone();
    cfg.db_port                    = db_port;
    cfg.db_user                    = db_user.clone();
    cfg.db_password                = db_password;
    cfg.db_name                    = db_name.clone();
    cfg.pinned_server_cert_pem     = server_cert_pem;
    cfg.pinned_server_fingerprint  = fingerprint.clone();
    // setup_complete is set to TRUE only after verify_pairing succeeds,
    // so that a partially-completed pairing (credentials saved but DB
    // unreachable) doesn't leave the client in a broken "setup complete"
    // state on next launch.
    cfg.setup_complete = false;
    cfg.save(&app_handle)
        .map_err(|e| format!("Failed to save config after pairing: {}", e))?;

    // ── Phase 4 review, S-1 (2026-09-05): the response previously included
    // the raw db_password — the ONE remaining IPC path that crossed the
    // untrusted-webview boundary with the database credential (get_config
    // strips it; save_config merges it away). The frontend never needed
    // it: the backend has already persisted it, verify_pairing re-reads
    // config, and the visible summary shows user/host/db/port only.
    Ok(serde_json::json!({
        "db_user":     db_user,
        "db_name":     db_name,
        "db_port":     db_port,
        "fingerprint": fingerprint,
        "server_ip":   server_ip,
    }))
}

/// Step 2 of client setup: verify that the saved credentials actually work
/// by opening a real PostgreSQL connection using the pinned TLS cert.
///
/// Called by the frontend when the user presses "Save & continue" after a
/// successful code exchange.  On success it sets setup_complete = true and
/// returns so the frontend can navigate to the main app.
///
/// This is the command that was MISSING before — without it, "Save &
/// continue" had no backend to call and therefore did nothing (the button
/// spinner ran forever or the page just hung).
#[tauri::command]
pub async fn verify_pairing(
    app_handle: tauri::AppHandle,
    session_state: tauri::State<'_, crate::rbac::SessionState>,
) -> Result<String, String> {
    // Phase 4 review, S-2: verify_pairing also WRITES config (flips
    // setup_complete=true below) and previously bypassed the mutation
    // gate. Same tri-state gate as redeem_pairing_code: first-run open,
    // configured machine requires a SettingsManage session.
    let disk = crate::config::AppConfig::disk_config_state(&app_handle);
    match crate::rbac::require_config_mutation(
        &session_state,
        disk,
        crate::rbac::Permission::SettingsManage,
    )? {
        crate::rbac::ConfigMutationGrant::FirstRun => {}
        crate::rbac::ConfigMutationGrant::Authorized(_) => {}
    }

    let cfg = crate::config::AppConfig::load(&app_handle)
        .ok_or_else(|| "Configuration not found. Please pair again.".to_string())?;

    if cfg.db_host.is_empty() || cfg.db_password.is_empty() {
        return Err(
            "Credentials not saved correctly. Please pair again.".to_string(),
        );
    }

    // Materialize the pinned cert to disk so sqlx/libpq can read it.
    let sslrootcert_path = cfg.materialize_pinned_cert(&app_handle);

    match &sslrootcert_path {
        Some(p) => eprintln!("[HMS Pairing] verify_pairing: cert at {}", p.display()),
        None    => eprintln!("[HMS Pairing] verify_pairing: no pinned cert — will use sslmode=require"),
    }

    // Attempt a real DB connection. Uses the same code path as normal
    // startup so if this works, startup will work too.
    crate::db::initialize(
        &cfg.db_host,
        cfg.db_port,
        &cfg.db_user,
        &cfg.db_password,
        &cfg.db_name,
        sslrootcert_path.as_deref(),
    )
    .await
    .map_err(|e| {
        // Give a richer error than the raw sqlx message
        let hint = crate::diagnose_db_error(&e);
        if hint.is_empty() {
            format!(
                "Could not connect to the hospital database:\n{}\n\n\
                 Make sure the reception PC is on and HMS Server is running.",
                e
            )
        } else {
            format!("Could not connect to the hospital database:\n{}\n\n{}", e, hint)
        }
    })?
    .close()
    .await; // close immediately — initialize_database will open the real pool

    // ── Mark setup complete ───────────────────────────────────────────────
    // Only written here, after a successful DB connection, so that a
    // failed verify leaves the client in "not set up" state and shows
    // the setup screen on next launch rather than a confusing startup error.
    let mut cfg2 = crate::config::AppConfig::load(&app_handle)
        .ok_or_else(|| "Config disappeared between verify steps.".to_string())?;
    cfg2.setup_complete = true;
    cfg2.save(&app_handle)
        .map_err(|e| format!("Failed to finalise setup: {}", e))?;

    eprintln!("[HMS Pairing] verify_pairing succeeded — setup_complete = true");
    Ok("connected".to_string())
}
