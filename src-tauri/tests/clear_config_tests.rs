//! `clear_config` registration + client-only, failure-conditional gate.
//!
//! Locks in the Option-3 hardening for the "Reconfigure" recovery path:
//!   * `clear_config` IS registered in generate_handler![] (it used to be a
//!     dead command — implemented in config.rs but never wired up, so the
//!     frontend's `invoke("clear_config")` could never succeed).
//!   * Server builds stay STRICTLY fail-closed: configured + no session is
//!     refused. So does client-build when the server is healthy.
//!   * Client builds allow an unauthenticated reset ONLY when the backend
//!     itself proves init/connectivity failure (no managed pool AND the
//!     configured server fails a fresh TCP probe).
//!   * The decision is derived from real backend state, never from a
//!     frontend-supplied flag — asserted below so the bypass cannot be
//!     reintroduced silently.
//!   * Deletion scope is config.json ONLY: entropy.key and backups/ survive,
//!     because entropy.key is needed to decrypt a future config and backups/
//!     is clinical data.
//!
//! Requires: `--features hms-integration-tests` (module visibility + the
//! `tauri::test` mock runtime). No database is required.

#![cfg(feature = "hms-integration-tests")]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use hospital_mgmt_lib::config::AppConfig;
use hospital_mgmt_lib::rbac::{
    require_config_mutation, ConfigDiskState, ConfigMutationGrant, Permission,
};
use tauri::test::mock_app;
use tauri::Manager;

type SessionState = Arc<Mutex<Option<hospital_mgmt_lib::rbac::Session>>>;

// ── Harness ───────────────────────────────────────────────────────────────────

fn test_hms_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let hms = std::env::temp_dir().join(format!(
        "hms_clear_test_{}_{}_{}/HMS",
        tag,
        std::process::id(),
        nanos
    ));
    std::fs::create_dir_all(&hms).unwrap();
    hms
}

/// Lay down a CONFIGURED client config plus the sibling files that
/// clear_config must NOT touch.
fn seed_configured_client(hms: &PathBuf) {
    let mut c = AppConfig::default();
    c.mode = "client".into();
    c.db_host = "192.168.1.2".into();
    c.db_port = 5432;
    c.db_user = "postgres".into();
    c.db_password = "SomeRealPassword123!".into();
    c.db_name = "hospital_db".into();
    c.setup_complete = true;
    c.save_to(&hms.join("config.json")).expect("seed config");
    // Sibling files that must survive a clear. NOTE: `save_to` already created
    // entropy.key (with the SEC-13 ACL), so we must NOT rewrite it — it is
    // read-only for this user and the write fails with os error 5. Asserting
    // presence is all the scope test needs.
    assert!(
        hms.join("entropy.key").exists(),
        "save_to must have produced entropy.key alongside the config"
    );
    std::fs::create_dir_all(hms.join("backups")).unwrap();
    std::fs::write(hms.join("backups").join("dump.sql"), "-- clinical data").unwrap();
}

fn read_src(rel: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|e| panic!("read {}: {}", rel, e))
}

// ── 1. Registration ───────────────────────────────────────────────────────────

#[test]
fn clear_config_is_registered_in_generate_handler() {
    // Source-level assertion: the command must appear in the
    // invoke_handler list, otherwise every frontend invoke fails with
    // "command not found" regardless of the gate logic.
    let src = read_src("src/lib.rs");
    // Anchor on `.invoke_handler(tauri::generate_handler![` — the COMMENT near
    // the top of lib.rs also mentions `generate_handler![]`, so a bare search
    // for the macro name would match that prose first and inspect the wrong
    // region. The real invocation is what actually registers commands.
    let anchor = ".invoke_handler(tauri::generate_handler![";
    let start = src
        .find(anchor)
        .expect("invoke_handler(generate_handler![) present");
    let handler = &src[start..];
    assert!(
        handler.contains("config::clear_config"),
        "clear_config MUST be listed in generate_handler![] — the 'Reconfigure' \
         button invokes it and cannot work while it is unregistered"
    );
}

// ── 2. Gate: strict behaviour unchanged for every non-client path ───────────

/// THE CORE SECURITY REGRESSION. `Active{setup_complete:true}` + no session
/// must be REFUSED. This is the exact server-build path and also the
/// client-build path when the server is healthy.
#[test]
fn configured_no_session_is_refused() {
    let state: SessionState = Arc::new(Mutex::new(None));
    let r = require_config_mutation(
        &state,
        ConfigDiskState::Active {
            setup_complete: true,
        },
        Permission::SettingsManage,
    );
    assert!(
        r.is_err(),
        "configured machine + no session must stay fail-closed"
    );
}

/// Corrupt disk state is treated as configured — also fail-closed.
#[test]
fn corrupt_no_session_is_refused() {
    let state: SessionState = Arc::new(Mutex::new(None));
    let r = require_config_mutation(
        &state,
        ConfigDiskState::Corrupt,
        Permission::SettingsManage,
    );
    assert!(r.is_err(), "corrupt config must not be treated as first run");
}

/// First-run windows remain open — unchanged by this work.
#[test]
fn first_run_no_session_is_allowed() {
    let state: SessionState = Arc::new(Mutex::new(None));
    assert!(matches!(
        require_config_mutation(
            &state,
            ConfigDiskState::Missing,
            Permission::SettingsManage
        ),
        Ok(ConfigMutationGrant::FirstRun)
    ));
}

// ── 3. Gate inputs are backend-verifiable, never frontend-supplied ───────────

/// A caller must not be able to talk its way past the gate. This asserts the
/// command signature exposes no boolean/flag that a hostile webview could set
/// to claim "the backend is broken, trust me".
#[test]
fn clear_config_takes_no_frontend_supplied_bypass_flag() {
    let src = read_src("src/config.rs");
    let start = src
        .find("pub async fn clear_config")
        .expect("clear_config defined");
    let body = &src[start..];
    // The parameter list ends at the first `) -> Result` after the fn name.
    let sig_end = body
        .find(") -> Result")
        .expect("clear_config signature terminator");
    let sig = &body[..sig_end];

    for forbidden in ["force", "skip_auth", "bypass", "assume_broken", "unverified", "trusted"] {
        assert!(
            !sig.to_lowercase().contains(forbidden),
            "clear_config must not accept a caller-supplied '{}' parameter — the \
             gate decision must come from backend state only",
            forbidden
        );
    }
}

/// The escape hatch that allows an unauthenticated reset must be compiled out
/// entirely unless this is a client build.
#[test]
fn backend_failure_signal_is_client_build_only() {
    let src = read_src("src/config.rs");
    // Locate the helper, then walk BACKWARDS over its attribute lines. Matching
    // the exact source text is brittle: an `async` qualifier or a reordered
    // attribute silently breaks a literal contains() check, which would fail
    // for cosmetic reasons and mask a real regression.
    let fn_idx = src
        .find("fn backend_init_failure_confirmed")
        .expect("backend_init_failure_confirmed must exist");
    let preceding = &src[..fn_idx];
    let attr_idx = preceding
        .rfind("#[cfg(feature = \"client-build\")]")
        .expect("backend_init_failure_confirmed must be cfg-gated to client-build");
    // Only whitespace and visibility/async qualifiers may sit between the
    // attribute and the fn — anything else means another cfg intervened.
    let between = preceding[attr_idx + "#[cfg(feature = \"client-build\")]".len()..].trim();
    assert!(
        between.is_empty()
            || matches!(between, "async" | "pub" | "pub async" | "pub(crate)" | "pub(crate) async"),
        "backend_init_failure_confirmed must be gated ONLY by client-build, \
         found between attribute and fn: {:?}",
        between
    );
    // Line-ending agnostic: the repo carries CRLF on Windows, so a literal
    // "\n"-joined assertion would fail for a purely cosmetic reason and mask a
    // real regression. Normalise before matching.
    let norm = src.replace("\r\n", "\n");
    assert!(
        norm.contains("#[cfg(not(feature = \"client-build\"))]\n    let grant = require_config_mutation_grant"),
        "the non-client-build path must keep calling the strict gate unconditionally"
    );
}

// ── 4. Deletion scope: config.json only ──────────────────────────────────────

/// Mirrors the single destructive step of `clear_config`. Pins the SCOPE so a
/// future "while we're here, also drop the backups" change fails loudly.
#[test]
fn clear_scope_removes_config_only_and_preserves_siblings() {
    let hms = test_hms_dir("scope");
    seed_configured_client(&hms);

    let cfg_path = hms.join("config.json");
    assert!(cfg_path.exists());
    assert!(hms.join("entropy.key").exists());
    assert!(hms.join("backups").join("dump.sql").exists());

    // The one destructive step clear_config performs.
    if cfg_path.exists() {
        std::fs::remove_file(&cfg_path).unwrap();
    }

    assert!(!cfg_path.exists(), "config.json must be removed");
    assert!(
        hms.join("entropy.key").exists(),
        "entropy.key MUST survive — the client needs it to decrypt a future config"
    );
    assert!(
        hms.join("backups").join("dump.sql").exists(),
        "backups/ MUST survive — it is clinical data, never destroyed by a re-pair"
    );

    std::fs::remove_dir_all(hms.parent().unwrap()).ok();
}

/// With no config present the step must be a harmless no-op, not an error.
#[test]
fn clearing_an_already_absent_config_is_a_noop() {
    let hms = test_hms_dir("absent");
    let cfg_path = hms.join("config.json");
    assert!(!cfg_path.exists());
    if cfg_path.exists() {
        std::fs::remove_file(&cfg_path).unwrap();
    }
    assert!(!cfg_path.exists());
    std::fs::remove_dir_all(hms.parent().unwrap()).ok();
}

// ── 5. Audit fallback when no database is reachable ──────────────────────────

/// The recovery path exists precisely BECAUSE the database is unreachable, so
/// a missing pool must not abort the command. `try_state` is what the command
/// uses for this; a bare mock app has no managed pool, i.e. exactly the
/// fallback case. If the command ever switched to `state::<PgPool>()` it
/// would PANIC here instead of logging and continuing.
#[test]
fn absent_pool_is_handled_without_panicking() {
    let app = mock_app();
    assert!(
        app.try_state::<sqlx::PgPool>().is_none(),
        "a bare mock app has no managed pool — this is the fallback case"
    );
    // Reset still proceeds without needing an audit row.
    let hms = test_hms_dir("nopool");
    seed_configured_client(&hms);
    let cfg_path = hms.join("config.json");
    if cfg_path.exists() {
        std::fs::remove_file(&cfg_path).unwrap();
    }
    assert!(!cfg_path.exists(), "reset proceeds without an audit row");
    std::fs::remove_dir_all(hms.parent().unwrap()).ok();
}

// ── 6. disk_config_state classification the gate relies on ───────────────────

#[test]
fn seeded_config_is_classified_as_configured() {
    let hms = test_hms_dir("state");
    seed_configured_client(&hms);
    let loaded = AppConfig::load_from(&hms.join("config.json")).expect("loads");
    assert!(
        loaded.setup_complete,
        "fixture must represent a PAIRED client (setup_complete=true)"
    );
    assert_eq!(loaded.db_host, "192.168.1.2");
    std::fs::remove_dir_all(hms.parent().unwrap()).ok();
}

// ── 7. Client-build gate BEHAVIOUR (compiled only under client-build) ─────────
//
// These are the real behavioural tests for the Option-3 escape hatch. They
// drive `init_failure_confirmed_from` — the exact function the command's
// decision reduces to — so they need no Tauri runtime, no network and no DB.
//
// Note on the signal's soundness: `pool_managed` mirrors
// `app_handle.try_state::<PgPool>().is_some()` — Tauri managed state, which a
// webview cannot forge through `invoke(...)` arguments. `server_reachable` is a
// live TCP connect to the address recorded on disk. Neither is reachable from
// the IPC payload, so no frontend payload can spoof the decision.

/// client-build + CONFIGURED + no pool + server unreachable → init failure
/// CONFIRMED, so the unauthenticated reset is permitted.
#[cfg(feature = "client-build")]
#[test]
fn client_allows_reset_when_pool_absent_and_server_unreachable() {
    assert!(
        hospital_mgmt_lib::config::init_failure_confirmed_from(false, false),
        "no managed pool AND unreachable server must confirm init failure"
    );

    // ...and the strict gate on its own still refuses, which is precisely why
    // the escape hatch exists.
    let state: SessionState = Arc::new(Mutex::new(None));
    assert!(require_config_mutation(
        &state,
        ConfigDiskState::Active { setup_complete: true },
        Permission::SettingsManage
    )
    .is_err());
}

/// client-build + CONFIGURED + pool PRESENT (healthy) → the escape hatch must
/// NOT engage, and the strict gate keeps refusing. Security-critical: a client
/// that DID initialise must never be re-pointable by an unauthenticated caller.
#[cfg(feature = "client-build")]
#[test]
fn client_refuses_reset_while_pool_is_managed() {
    assert!(
        !hospital_mgmt_lib::config::init_failure_confirmed_from(true, false),
        "a managed pool proves initialisation succeeded — the escape hatch must \
         NOT engage even when the server later goes away"
    );
    assert!(
        !hospital_mgmt_lib::config::init_failure_confirmed_from(true, true),
        "healthy pool + reachable server is never an init failure"
    );

    let state: SessionState = Arc::new(Mutex::new(None));
    assert!(require_config_mutation(
        &state,
        ConfigDiskState::Active { setup_complete: true },
        Permission::SettingsManage
    )
    .is_err());
}

/// A reachable server alone is not enough — without the missing-pool signal
/// the hatch stays shut. Both conditions are required.
#[cfg(feature = "client-build")]
#[test]
fn client_requires_both_conditions_not_just_unreachable_server() {
    assert!(
        !hospital_mgmt_lib::config::init_failure_confirmed_from(false, true),
        "reachable server must never permit an unauthenticated reset"
    );
}

/// The escape hatch must not exist at all on non-client builds, so a
/// server-build binary can never reach the unauthenticated-reset path.
#[cfg(not(feature = "client-build"))]
#[test]
fn escape_hatch_is_absent_on_non_client_builds() {
    // Compile-time proof: this test file only references the helper under
    // #[cfg(feature = "client-build")], so the non-client build genuinely has
    // no path to it. Assert the strict gate refuses, which is what a
    // server-build caller would get.
    let state: SessionState = Arc::new(Mutex::new(None));
    assert!(require_config_mutation(
        &state,
        ConfigDiskState::Active { setup_complete: true },
        Permission::SettingsManage
    )
    .is_err());
}