/// LAN connectivity module.
///
/// Role is no longer auto-detected at runtime by a boot race — it is fixed
/// at compile/install time:
///   - The "server" build (reception PC) always provisions + owns PostgreSQL.
///   - The "client" build (doctor/nurse PCs) always connects outward.
///
/// Clients store the server's IP in their saved AppConfig after a one-time
/// setup step. On every launch, a client:
///   1. Tries the saved IP first (fast path, works even with no UDP/broadcast).
///   2. If that fails (server PC's IP changed), falls back to listening for
///      a LAN broadcast from the server for a few seconds, to self-heal.
///
/// The server build periodically broadcasts its presence so clients can
/// recover automatically if reception's IP changes (DHCP renewal, etc.),
/// without requiring a hospital IT visit.
///
/// SEC-08: the broadcast payload is now HMAC-signed with the server's TLS
/// certificate fingerprint as the key. Clients that have already paired
/// (and thus pinned the fingerprint) verify the HMAC before accepting a
/// broadcast, so an attacker on the LAN cannot spoof HMS_SERVER broadcasts
/// to redirect clients to a rogue DB. Pre-pairing clients (no pinned
/// fingerprint) accept any broadcast — same TOFU trust model as before.
use sha2::{Digest, Sha256};
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const DISCOVERY_PORT: u16 = 42010;
// SEC-08: reduced from 5s → 30s. A 5s broadcast gave an attacker on the
// LAN ~17,280 passive enumeration samples per day (just listen on UDP
// 42010). At 30s the same attacker gets ~2,880 samples/day — still
// fast enough for client auto-recovery (a client falling back to
// broadcast discovery listens for RECOVERY_LISTEN_TIMEOUT_SECS and
// will catch at least one broadcast in that window) but ~6x less
// passive exposure. The client recovery timeout was correspondingly
// bumped from 6s → 35s so the client still reliably catches one
// broadcast within the discovery window.
pub const BROADCAST_INTERVAL_SECS: u64 = 30;
// Used only by client-build (via `detect_server` / `detect_server_with_fp`)
// to size the recovery listen window. Kept here so the comment above stays
// accurate in both builds; allow dead code in server-build.
#[allow(dead_code)]
const RECOVERY_LISTEN_TIMEOUT_SECS: u64 = 35;
// SEC-08: reject broadcasts whose timestamp is older than this (replay
// protection). 120s is generous: the server broadcasts every 30s, so a
// freshly-captured broadcast reaches the client within 30s, but an
// attacker replaying an old capture is bounded by this window.
// Used only by client-build (broadcast-listening path); allow dead code in
// server-build.
#[allow(dead_code)]
const BROADCAST_MAX_AGE_SECS: u64 = 120;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum Role {
    Server { local_ip: String },
    // Constructed only in the client-build (LAN discovery path). Allow dead
    // code in server-build to avoid spurious warnings — the variant is still
    // deserialised/matched by `get_server_role` in both builds.
    #[allow(dead_code)]
    Client { server_ip: String, db_port: u16 },
}

/// Listen briefly for a server broadcast. Used by clients only as a
/// recovery path when the saved IP is unreachable.
///
/// SEC-08: if `expected_fingerprint_hex` is `Some`, only broadcasts whose
/// HMAC (keyed with the fingerprint) validates are accepted. If `None`
/// (pre-pairing / TOFU), any well-formed broadcast is accepted — same
/// behaviour as before this fix.
///
/// Only called from client-build (via `detect_server_with_fp`); allow dead
/// code in server-build.
#[allow(dead_code)]
pub fn listen_for_broadcast_with_fp(
    timeout_secs: u64,
    expected_fingerprint_hex: Option<&str>,
) -> Option<(String, u16)> {
    let socket = UdpSocket::bind(format!("0.0.0.0:{}", DISCOVERY_PORT)).ok()?;
    socket
        .set_read_timeout(Some(Duration::from_secs(timeout_secs)))
        .ok()?;

    let mut buf = [0u8; 256];
    loop {
        match socket.recv_from(&mut buf) {
            Ok((n, _src)) => {
                let msg = String::from_utf8_lossy(&buf[..n]);
                if let Some(payload) = msg.strip_prefix("HMS_SERVER:") {
                    let parts: Vec<&str> = payload.split(':').collect();
                    // SEC-08: new signed format is `ip:port:hmac:timestamp`
                    // (4 parts). Legacy unsigned format is `ip:port` (2
                    // parts). Accept legacy for the upgrade window but
                    // ONLY when no fingerprint is pinned (TOFU); a paired
                    // client rejects unsigned broadcasts.
                    if parts.len() == 4 {
                        let ip = parts[0];
                        let port_str = parts[1];
                        let hmac_hex = parts[2];
                        let ts_str = parts[3];
                        let port = match port_str.parse::<u16>() {
                            Ok(p) => p,
                            Err(_) => continue,
                        };
                        let ts: u64 = match ts_str.parse::<u64>() {
                            Ok(t) => t,
                            Err(_) => continue,
                        };
                        // Replay protection: reject stale broadcasts.
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        // Guard against wrap-around / far-future timestamps
                        // (an attacker could set ts = u64::MAX to make the
                        // age check underflow). `now.saturating_sub(ts)` is
                        // safe; reject if the broadcast is in the future
                        // by more than 60s (clock-skew tolerance).
                        if ts > now + 60 {
                            continue; // far-future timestamp — reject
                        }
                        let age = now.saturating_sub(ts);
                        if age > BROADCAST_MAX_AGE_SECS {
                            continue; // stale — likely a replay
                        }
                        // If a fingerprint is pinned, validate the HMAC.
                        // The signed message is `<timestamp>:<ip>:<port>`
                        // (per SEC-08 spec) — the same construction the
                        // server uses in `start_broadcast`.
                        if let Some(fp) = expected_fingerprint_hex {
                            let signed_msg = format!("{}:{}:{}", ts_str, ip, port_str);
                            let expected = hmac_sha256_hex(fp.as_bytes(), signed_msg.as_bytes());
                            if !constant_time_eq(hmac_hex.as_bytes(), expected.as_bytes()) {
                                // HMAC mismatch — this broadcast was NOT
                                // signed by the server we paired with.
                                // Silently drop it and keep listening.
                                continue;
                            }
                        }
                        return Some((ip.to_string(), port));
                    } else if parts.len() == 2 {
                        // Legacy unsigned format. Accept ONLY when no
                        // fingerprint is pinned (pre-pairing TOFU, or
                        // upgrade window where the server is an older
                        // build that doesn't sign broadcasts yet).
                        if expected_fingerprint_hex.is_some() {
                            // Paired client — refuse unsigned broadcasts.
                            continue;
                        }
                        if let Ok(port) = parts[1].parse::<u16>() {
                            return Some((parts[0].to_string(), port));
                        }
                    }
                }
            }
            Err(_) => return None, // timeout
        }
    }
}

/// Backward-compatible wrapper: TOFU listen (no fingerprint validation).
/// Used by callers that don't have a pinned fingerprint yet.
///
/// Only called from client-build; allow dead code in server-build.
#[allow(dead_code)]
pub fn listen_for_broadcast(timeout_secs: u64) -> Option<(String, u16)> {
    listen_for_broadcast_with_fp(timeout_secs, None)
}

/// Convenience wrapper using the default recovery timeout.
///
/// Only called from client-build; allow dead code in server-build.
#[allow(dead_code)]
pub fn detect_server() -> Option<(String, u16)> {
    listen_for_broadcast(RECOVERY_LISTEN_TIMEOUT_SECS)
}

/// SEC-08: like `detect_server` but validates broadcast HMAC against the
/// pinned TLS fingerprint. Pass `None` for pre-pairing TOFU discovery.
///
/// Only called from client-build; allow dead code in server-build.
#[allow(dead_code)]
pub fn detect_server_with_fp(
    expected_fingerprint_hex: Option<String>,
) -> Option<(String, u16)> {
    listen_for_broadcast_with_fp(
        RECOVERY_LISTEN_TIMEOUT_SECS,
        expected_fingerprint_hex.as_deref(),
    )
}

// ── SEC-08: HMAC-SHA256 (RFC 2104) ───────────────────────────────────────────
//
// Implemented inline using the existing `sha2` crate (already a dep) so we
// don't have to add the `hmac` crate to Cargo.toml + regenerate Cargo.lock
// in a sandbox without a Rust toolchain. The construction is ~20 lines and
// is the well-known RFC 2104 spec.
//
// Key    = the server's TLS certificate fingerprint (hex string, 64 chars
//          for SHA-256). Treated as opaque bytes here.
// Message = `<timestamp>:<ip>:<port>` — see `start_broadcast` and
//          `listen_for_broadcast_with_fp` for the construction.
fn hmac_sha256_hex(key: &[u8], message: &[u8]) -> String {
    const BLOCK_SIZE: usize = 64; // SHA-256 block size in bytes

    // Key normalization: if longer than block size, hash; if shorter,
    // zero-pad to block size.
    let mut key_block = [0u8; BLOCK_SIZE];
    if key.len() > BLOCK_SIZE {
        let mut h = Sha256::new();
        h.update(key);
        let digest = h.finalize();
        key_block[..digest.len()].copy_from_slice(&digest);
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }

    // Build ipad / opad.
    let mut ipad = [0u8; BLOCK_SIZE];
    let mut opad = [0u8; BLOCK_SIZE];
    for i in 0..BLOCK_SIZE {
        ipad[i] = key_block[i] ^ 0x36;
        opad[i] = key_block[i] ^ 0x5c;
    }

    // Inner: H(ipad || message)
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(message);
    let inner_hash = inner.finalize();

    // Outer: H(opad || inner_hash)
    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_hash);
    let outer_hash = outer.finalize();

    // Hex-encode the 32-byte digest → 64-char lowercase string.
    let mut out = String::with_capacity(64);
    for b in outer_hash.iter() {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

/// Constant-time byte-slice comparison. Prevents the classic
/// early-exit timing oracle on HMAC verification (where comparing
/// byte-by-byte and returning on the first mismatch leaks the
/// position of the differing byte, enabling a blind forgery).
///
/// Only used by `listen_for_broadcast_with_fp`, which is client-build only;
/// allow dead code in server-build.
#[allow(dead_code)]
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Spawn a background thread that broadcasts the server's presence every
/// few seconds. Only ever called by the server build.
///
/// SEC-08: the broadcast payload is now HMAC-signed with the server's TLS
/// certificate fingerprint as the key, so a paired client can verify the
/// broadcast originated from the server it trusts (and isn't a spoofed
/// broadcast from an attacker on the LAN trying to redirect clients to a
/// rogue DB). `tls_fingerprint_hex` is the SHA-256 fingerprint of the
/// server's self-signed TLS cert (see `tls_provision.rs`). An empty
/// string disables signing — used only in dev/fallback mode where no TLS
/// cert has been provisioned; the broadcast is then unsigned and clients
/// in TOFU mode (pre-pairing) will still accept it.
pub fn start_broadcast(
    local_ip: String,
    db_port: u16,
    tls_fingerprint_hex: String,
    running: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        let socket = match UdpSocket::bind("0.0.0.0:0") {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[HMS Discovery] Failed to bind broadcast socket: {}", e);
                return;
            }
        };

        if let Err(e) = socket.set_broadcast(true) {
            eprintln!("[HMS Discovery] set_broadcast failed: {}", e);
            return;
        }

        let dest: SocketAddr = format!("255.255.255.255:{}", DISCOVERY_PORT)
            .parse()
            .unwrap();

        // SEC-08: if a fingerprint is available, sign every broadcast.
        // Otherwise fall back to the legacy unsigned format (dev mode).
        let signing_enabled = !tls_fingerprint_hex.is_empty();

        while running.load(Ordering::Relaxed) {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let msg = if signing_enabled {
                // Signed format: HMS_SERVER:<ip>:<port>:<hmac>:<timestamp>
                // The signed message is `<timestamp>:<ip>:<port>` — clients
                // reconstruct the same string from the parsed fields to
                // verify the HMAC.
                let signed_msg = format!("{}:{}:{}", now, local_ip, db_port);
                let hmac = hmac_sha256_hex(tls_fingerprint_hex.as_bytes(), signed_msg.as_bytes());
                format!("HMS_SERVER:{}:{}:{}:{}", local_ip, db_port, hmac, now)
            } else {
                // Legacy unsigned format (dev/fallback only).
                format!("HMS_SERVER:{}:{}", local_ip, db_port)
            };
            if let Err(e) = socket.send_to(msg.as_bytes(), dest) {
                eprintln!("[HMS Discovery] Broadcast error: {}", e);
            }
            std::thread::sleep(Duration::from_secs(BROADCAST_INTERVAL_SECS));
        }
    });
}

/// Determine the LAN IP of this machine (not loopback).
pub fn local_lan_ip() -> String {
    let socket = UdpSocket::bind("0.0.0.0:0").ok();
    if let Some(s) = socket {
        // Option 1: Try connecting to a public address (standard, works when online)
        if s.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = s.local_addr() {
                let ip = addr.ip();
                if !ip.is_loopback() && ip.is_ipv4() {
                    return ip.to_string();
                }
            }
        }

        // Option 2: Try connecting to broadcast (works offline to get primary network interface IP)
        if s.connect("255.255.255.255:80").is_ok() {
            if let Ok(addr) = s.local_addr() {
                let ip = addr.ip();
                if !ip.is_loopback() && ip.is_ipv4() {
                    return ip.to_string();
                }
            }
        }
    }

    // Option 3: Resolve hostname (useful in completely disconnected LANs)
    if let Ok(computer_name) = std::env::var("COMPUTERNAME") {
        use std::net::ToSocketAddrs;
        if let Ok(addrs) = format!("{}:0", computer_name).to_socket_addrs() {
            for addr in addrs {
                let ip = addr.ip();
                if ip.is_ipv4() && !ip.is_loopback() {
                    return ip.to_string();
                }
            }
        }
    }

    "127.0.0.1".to_string()
}

/// Quick reachability probe — used by the client fast path to check the
/// saved server IP before falling back to broadcast discovery.
pub fn is_reachable(host: &str, port: u16, timeout_ms: u64) -> bool {
    use std::net::TcpStream;
    let addr = format!("{}:{}", host, port);
    match addr.parse::<SocketAddr>() {
        Ok(socket_addr) => TcpStream::connect_timeout(&socket_addr, Duration::from_millis(timeout_ms)).is_ok(),
        Err(_) => false,
    }
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_server_role(
    role: tauri::State<'_, Arc<std::sync::Mutex<Option<Role>>>>,
) -> Result<serde_json::Value, String> {
    let guard = role.lock().unwrap_or_else(|e| e.into_inner());
    match guard.as_ref() {
        Some(Role::Server { local_ip }) => Ok(serde_json::json!({
            "role": "server",
            "local_ip": local_ip,
        })),
        Some(Role::Client { server_ip, db_port }) => Ok(serde_json::json!({
            "role": "client",
            "server_ip": server_ip,
            "db_port": db_port,
        })),
        None => Ok(serde_json::json!({ "role": "detecting" })),
    }
}
