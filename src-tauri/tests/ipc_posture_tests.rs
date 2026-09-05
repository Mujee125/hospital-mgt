//! Phase 4 (Priority 5 security pass) — Full-IPC guard conformance test.
//!
//! STRIDE-shaped regression net for the authorization posture of EVERY
//! #[tauri::command] in src/. A command must satisfy exactly one of:
//!   1. DIRECT-GUARD — its body calls one of the guard fns
//!      (require / require_strong / require_session / require_if_session /
//!      require_config_mutation);
//!   2. CORE-DELEGATE — its body delegates the whole operation to a
//!      `*_core` fn that carries the guard (the AERP extraction pattern:
//!      login/me/update_user/reset_user_password/create_patient/
//!      create_prescription wrappers);
//!   3. ALLOWLIST — pre-login/boot-flow commands (login itself, license
//!      gate, pairing, discovery, first-run setup) where the guard would
//!      make the flow impossible. This list is the reviewed, deliberate
//!      NO_GUARD posture — adding to it requires a documented reason.
//!
//! Any new #[tauri::command] that lacks all three FAILS this test, so the
//! "someone added a command and forgot the guard" bug class can never
//! silently regress.
//!
//! Requires: `--features hms-integration-tests` (module visibility only;
//! the scan itself is pure source analysis like wp2_i11).

#![cfg(feature = "hms-integration-tests")]

use std::path::PathBuf;

/// The reviewed NO_GUARD allowlist (Phase 4 audit, 2026-09-05).
/// Boot flow / pre-login / license gate / pairing — each is reachable
/// BEFORE any user exists, so a permission guard would be unsatisfiable.
const ALLOWLIST: &[&str] = &[
    // auth: login IS authentication; me/change_password self-guard via session
    "login",
    // license gate — must run before login is possible at all
    "verify_license",
    "get_hardware_fingerprint",
    "get_license_public_key_fingerprint",
    "get_install_fingerprint",
    // pairing: first-run client setup, pre-login by definition
    "generate_pairing_code",
    "get_pairing_status",
    "redeem_pairing_code",   // gated itself via require_config_mutation
    "verify_pairing",        // gated itself via require_config_mutation
    // boot diagnostics the Setup screens call pre-login
    "get_local_ip",
    "test_server_connection",
    "get_config_path",
    "get_server_role",
    "initialize_database",
    "check_db_connection",
    "complete_pairing_and_connect",
    // reads config with require_if_session inside its own body (pre-login
    // allowed because db_password is skip_serializing)
    "get_config",
];

/// Wrappers whose entire logic delegates to a `*_core` fn carrying the
/// guard (verified separately below by checking the core body).
const CORE_DELEGATES: &[(&str, &str)] = &[
    ("me", "me_core"),
    ("update_user", "update_user_core"),
    ("reset_user_password", "reset_user_password_core"),
    ("create_patient", "create_patient_core"),
    ("create_prescription", "create_prescription_core"),
];

const GUARD_TOKENS: &[&str] = &[
    "require_strong(",
    "require_session(",
    "require_if_session(",
    "require_config_mutation(",
    "rbac::require(",
];

fn scan_all_commands() -> Vec<(String, String)> {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let mut out = Vec::new();
    let mut stack = vec![PathBuf::from(manifest).join("src")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("read src dir") {
            let entry = entry.expect("dir entry");
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                let src = std::fs::read_to_string(&path).expect("read source file");
                let lines: Vec<&str> = src.lines().collect();
                let rel = path
                    .strip_prefix(manifest)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");

                for i in 0..lines.len() {
                    let t = lines[i].trim();
                    // Attribute must start its own line (not /// doc text).
                    if t != "#[tauri::command]" {
                        continue;
                    }
                    // The fn signature may be the next line, or after
                    // additional attributes (e.g. #[allow(...)]) — skip
                    // attribute lines until the fn line.
                    let mut j = i + 1;
                    let mut fn_name = String::new();
                    while j < lines.len() {
                        let l = lines[j].trim();
                        if l.starts_with("#[") {
                            j += 1;
                            continue;
                        }
                        if l.starts_with("fn ") || l.starts_with("pub fn ") || l.starts_with("pub async fn ") || l.starts_with("async fn ") {
                            let after_fn = l
                                .trim_start_matches("pub")
                                .trim_start()
                                .trim_start_matches("async")
                                .trim_start()
                                .trim_start_matches("fn")
                                .trim_start();
                            fn_name = after_fn
                                .chars()
                                .take_while(|c| c.is_alphanumeric() || *c == '_')
                                .collect();
                            break;
                        }
                        break;
                    }
                    if fn_name.is_empty() {
                        continue;
                    }
                    // Body: from the fn line to the next command attribute
                    // (or 2000 lines ahead — plenty for any command fn).
                    let body: String = lines[j..(j + 2000).min(lines.len())]
                        .iter()
                        .take_while(|l| !l.trim().starts_with("#[tauri::command]"))
                        .map(|l| *l)
                        .collect::<Vec<&str>>()
                        .join("\n");
                    out.push((format!("{}:{}", rel, fn_name), body));
                }
            }
        }
    }
    out
}

#[test]
fn ph4_every_ipc_command_is_guarded_or_allowlisted() {
    let commands = scan_all_commands();
    assert!(
        commands.len() >= 150,
        "expected a large command surface, found {} — scan may be broken",
        commands.len()
    );

    let mut violations: Vec<String> = Vec::new();
    for (name_body, body) in &commands {
        let name = name_body.split(':').nth(1).unwrap_or("");
        // 1. Direct guard?
        if GUARD_TOKENS.iter().any(|t| body.contains(t)) {
            continue;
        }
        // 2. Core delegate? Verify the wrapper delegates AND the core body
        //    carries a guard.
        if let Some((_, core)) = CORE_DELEGATES.iter().find(|(w, _)| *w == name) {
            if body.contains(&format!("{}(", core)) && core_has_guard(core) {
                continue;
            }
        }
        // 3. Allowlist?
        if ALLOWLIST.contains(&name) {
            continue;
        }
        violations.push(name_body.clone());
    }

    assert!(
        violations.is_empty(),
        "UNGUARDED IPC commands (must add a guard, a gated core, or a documented \
         allowlist entry):\n  {}",
        violations.join("\n  ")
    );
}

/// The core fn must itself contain a guard call (searched across all files,
/// since cores may live in a different module than the wrapper).
fn core_has_guard(core: &str) -> bool {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let needle = format!("fn {}", core);
    let mut stack = vec![PathBuf::from(manifest).join("src")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("read src dir") {
            let entry = entry.expect("dir entry");
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                let src = std::fs::read_to_string(&path).expect("read source file");
                if let Some(pos) = src.find(&needle) {
                    let tail = &src[pos..];
                    let end = tail.find("\n}").unwrap_or(tail.len());
                    let body = &tail[..end];
                    return GUARD_TOKENS.iter().any(|t| body.contains(t));
                }
            }
        }
    }
    false
}

#[test]
fn ph4_allowlist_has_no_stale_entries() {
    // Every allowlisted name must still exist as a command — stale entries
    // would let a future command reuse the name without review.
    let commands = scan_all_commands();
    let names: Vec<String> = commands
        .iter()
        .map(|(nb, _)| nb.split(':').nth(1).unwrap_or("").to_string())
        .collect();
    for a in ALLOWLIST {
        assert!(
            names.iter().any(|n| n == a),
            "allowlist entry '{}' no longer exists as a command — remove it",
            a
        );
    }
}
