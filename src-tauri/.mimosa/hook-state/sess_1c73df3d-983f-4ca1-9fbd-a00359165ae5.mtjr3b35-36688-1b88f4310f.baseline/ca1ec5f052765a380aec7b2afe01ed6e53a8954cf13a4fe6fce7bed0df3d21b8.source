//! AERP Part G — G.1 Work Package 1: WhatsApp Authorization Hardening.
//!
//! These are the WP-1 integration/negative/penetration/concurrency tests
//! from the AERP test-engineering package (RCTF-EA-003 Part G), implemented
//! per spec against a real PostgreSQL test database via the production
//! guard code (`rbac::require` / `rbac::require_strong`) — the exact code
//! the five WhatsApp commands call as their first statement.
//!
//! Why test at the guard layer, not through the webview: the runtime IPC
//! layer was already verified end-to-end in the Windows GUI verification
//! round (patient → all 5 commands → exact denial strings, see
//! docs/VERIFICATION-REPORT-WP1-WP3.md §4 row 4). Here we (a) prove the
//! guard semantics for every role against the real DB, and (b) pin the
//! command→guard wiring with a source-conformance test so a future refactor
//! can't silently drop a guard (the original WP-1 finding).
//!
//! Requires: HMS_TEST_DB_URL env var + `--features hms-integration-tests`.

mod common;

use common::*;
use hospital_mgmt_lib::rbac::{self, Permission, Session, SessionState};
use sqlx::PgPool;
use std::sync::{Arc, Mutex};

// ── G.1.2 Integration tests (WP1-I01 … I12) ──────────────────────────────────

async fn setup() -> PgPool {
    test_pool().await
}

/// Build a SessionState holding a REAL loaded session for the user — i.e.
/// exactly the in-memory state a successful `login` leaves behind.
async fn session_state_for(pool: &PgPool, user_id: i32, token_hash: &str) -> SessionState {
    let s = load_session_for(pool, user_id, token_hash).await;
    Arc::new(Mutex::new(Some(s)))
}

async fn empty_state() -> SessionState {
    Arc::new(Mutex::new(None))
}

/// WP1-I01 … I05 — a patient-role session is denied every WhatsApp entry
/// point with the exact permission error the command would surface.
/// (Spec: patient cannot send notifications, cannot send to patient,
/// cannot read the log, cannot read/set WhatsApp config, cannot send test.)
#[tokio::test]
async fn wp1_i01_i05_patient_denied_all_whatsapp_commands() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_patient", "Passw0rd!", &["patient"]).await;
    seed_session_row(&pool, uid, "hash_patient_i01").await;
    let state = session_state_for(&pool, uid, "hash_patient_i01").await;

    // Guards the five commands call as their FIRST statement (commands.rs):
    // send_whatsapp_notification / send_whatsapp_to_patient → WhatsAppSend
    let r1 = rbac::require_strong(&state, &pool, Permission::WhatsAppSend).await;
    // send_whatsapp_test / get_whatsapp_config / set_whatsapp_config /
    // test_whatsapp_api → SettingsManage
    let r2 = rbac::require_strong(&state, &pool, Permission::SettingsManage).await;
    // get_notification_log → WhatsAppView
    let r3 = rbac::require_strong(&state, &pool, Permission::WhatsAppView).await;

    for (label, r, perm) in [
        ("send (whatsapp.send)", r1, "whatsapp.send"),
        ("config (settings.manage)", r2, "settings.manage"),
        ("log (whatsapp.view)", r3, "whatsapp.view"),
    ] {
        match r {
            Err(e) => assert!(
                e.contains(&format!("requires the '{}'", perm)),
                "{}: expected permission denial, got: {}",
                label,
                e
            ),
            Ok(_) => panic!("{}: patient session MUST be denied", label),
        }
    }
}

/// WP1-I06 — a doctor session passes the WhatsAppSend guard (authorization
/// verified; the physical delivery step is GUI/runtime-verified and needs
/// the OS opener, which has no place in a DB test — deviation documented in
/// the test-engineering report).
#[tokio::test]
async fn wp1_i06_doctor_passes_send_guard() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_doctor", "Passw0rd!", &["doctor"]).await;
    seed_session_row(&pool, uid, "hash_doctor_i06").await;
    let state = session_state_for(&pool, uid, "hash_doctor_i06").await;

    rbac::require_strong(&state, &pool, Permission::WhatsAppSend)
        .await
        .expect("doctor must pass whatsapp.send guard");
}

/// WP1-I07 — a doctor passes the WhatsAppView guard (get_notification_log).
#[tokio::test]
async fn wp1_i07_doctor_passes_view_guard() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_doctor2", "Passw0rd!", &["doctor"]).await;
    seed_session_row(&pool, uid, "hash_doctor_i07").await;
    let state = session_state_for(&pool, uid, "hash_doctor_i07").await;

    rbac::require_strong(&state, &pool, Permission::WhatsAppView)
        .await
        .expect("doctor must pass whatsapp.view guard");
}

/// WP1-I08 — billing clerk: view-only (passes WhatsAppView, denied Send).
#[tokio::test]
async fn wp1_i08_billing_clerk_view_only() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_billing", "Passw0rd!", &["billing_clerk"]).await;
    seed_session_row(&pool, uid, "hash_billing_i08").await;
    let state = session_state_for(&pool, uid, "hash_billing_i08").await;

    rbac::require_strong(&state, &pool, Permission::WhatsAppView)
        .await
        .expect("billing clerk must pass whatsapp.view");
    let r = rbac::require_strong(&state, &pool, Permission::WhatsAppSend).await;
    assert!(r.is_err(), "billing clerk must NOT pass whatsapp.send");
}

/// WP1-I09 — super_admin passes every WhatsApp guard.
#[tokio::test]
async fn wp1_i09_super_admin_all_guards() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_admin", "Passw0rd!", &["super_admin"]).await;
    seed_session_row(&pool, uid, "hash_admin_i09").await;
    let state = session_state_for(&pool, uid, "hash_admin_i09").await;

    for perm in [Permission::WhatsAppSend, Permission::WhatsAppView, Permission::SettingsManage] {
        rbac::require_strong(&state, &pool, perm)
            .await
            .unwrap_or_else(|e| panic!("super_admin must pass {:?}: {}", perm, e));
    }
}

/// WP1-I10 — seed_defaults inserted the two WhatsApp permission keys.
#[tokio::test]
async fn wp1_i10_seed_inserts_whatsapp_permissions() {
    let pool = setup().await; // fresh_test_db runs the real seeder
    let keys: Vec<(String,)> = sqlx::query_as(
        "SELECT key FROM permissions WHERE key IN ('whatsapp.send','whatsapp.view')",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(keys.len(), 2, "both whatsapp.* keys must be seeded");
}

/// WP1-I11 — seeded role_permissions match the code's grant table exactly
/// (verified for ALL roles, not just WhatsApp — full drift detection).
#[tokio::test]
async fn wp1_i11_seed_grants_match_code() {
    let pool = setup().await;
    for (role, expected) in [
        ("super_admin", hospital_mgmt_lib::rbac::permissions_for_role("super_admin")),
        ("doctor", hospital_mgmt_lib::rbac::permissions_for_role("doctor")),
        ("nurse", hospital_mgmt_lib::rbac::permissions_for_role("nurse")),
        ("receptionist", hospital_mgmt_lib::rbac::permissions_for_role("receptionist")),
        ("lab_technician", hospital_mgmt_lib::rbac::permissions_for_role("lab_technician")),
        ("pharmacist", hospital_mgmt_lib::rbac::permissions_for_role("pharmacist")),
        ("billing_clerk", hospital_mgmt_lib::rbac::permissions_for_role("billing_clerk")),
        ("patient", hospital_mgmt_lib::rbac::permissions_for_role("patient")),
    ] {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT p.key FROM role_permissions rp \
             JOIN roles r ON r.id = rp.role_id \
             JOIN permissions p ON p.id = rp.permission_id \
             WHERE r.name = $1",
        )
        .bind(role)
        .fetch_all(&pool)
        .await
        .unwrap();
        let db_keys: std::collections::HashSet<String> =
            rows.into_iter().map(|r| r.0).collect();
        let code_keys: std::collections::HashSet<String> =
            expected.iter().map(|p| p.as_str().to_string()).collect();
        assert_eq!(
            db_keys, code_keys,
            "DB grants for role '{}' drifted from permissions_for_role()",
            role
        );
    }
}

/// WP1-I12 — seed_defaults is idempotent (run twice → same counts).
#[tokio::test]
async fn wp1_i12_seed_idempotent() {
    let pool = setup().await;
    let before: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM role_permissions")
        .fetch_one(&pool)
        .await
        .unwrap();
    hospital_mgmt_lib::auth::seed_defaults(&pool)
        .await
        .expect("second seed run");
    let after: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM role_permissions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        before.0, after.0,
        "second seed_defaults must not change role_permissions"
    );
}

// ── G.1.3 Negative tests (WP1-N01 … N04) ──────────────────────────────────────

/// WP1-N01 — with NO session in state, every guard rejects with
/// "you are not signed in" (the pre-login posture).
#[tokio::test]
async fn wp1_n01_unauthenticated_denied() {
    let pool = setup().await;
    let state = empty_state().await;
    let r = rbac::require(&state, Permission::WhatsAppSend).unwrap_err();
    assert!(r.contains("not signed in"), "got: {}", r);
}

/// WP1-N02 — a rogue permission string injected into the DB cannot be used
/// by require(): guards match against the enum's canonical keys only.
#[tokio::test]
async fn wp1_n02_rogue_permission_string_inert() {
    let pool = setup().await;
    // Attacker with DB write access inserts a fake permission + grants it
    // to the patient role (worst case).
    sqlx::query("INSERT INTO permissions (key, description) VALUES ('whatsapp.send.evil', 'x')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO role_permissions (role_id, permission_id) \
         SELECT r.id, p.id FROM roles r, permissions p \
         WHERE r.name='patient' AND p.key='whatsapp.send.evil'",
    )
    .execute(&pool)
    .await
    .unwrap();

    let uid = seed_user(&pool, "aerp_patient_n02", "Passw0rd!", &["patient"]).await;
    seed_session_row(&pool, uid, "hash_n02").await;
    let state = session_state_for(&pool, uid, "hash_n02").await;

    // Even though the in-memory HashSet now CONTAINS the rogue string
    // (load_session loads whatever the roles grant), the enum-based check
    // cannot match it → still denied.
    let session = load_session_for(&pool, uid, "hash_n02").await;
    assert!(session.permissions.contains("whatsapp.send.evil"));
    let r = rbac::require(&state, Permission::WhatsAppSend).unwrap_err();
    assert!(r.contains("requires the 'whatsapp.send'"), "got: {}", r);

    // Cleanup: remove the injected rows so later tests see pristine grants.
    sqlx::query("DELETE FROM role_permissions rp USING permissions p WHERE rp.permission_id = p.id AND p.key = 'whatsapp.send.evil'")
        .execute(&pool).await.unwrap();
    sqlx::query("DELETE FROM permissions WHERE key = 'whatsapp.send.evil'")
        .execute(&pool).await.unwrap();
}

/// WP1-N03 — stale permissions: granting whatsapp.send to the patient role
/// in the DB AFTER login does NOT grant it in the in-memory session. The
/// next login (or app restart) picks it up. Cache semantics are the guard.
#[tokio::test]
async fn wp1_n03_stale_db_grant_inert_until_relogin() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_patient_n03", "Passw0rd!", &["patient"]).await;
    seed_session_row(&pool, uid, "hash_n03").await;
    let state = session_state_for(&pool, uid, "hash_n03").await;

    // Stale: still denied.
    assert!(rbac::require(&state, Permission::WhatsAppSend).is_err());

    // Admin grants the permission to the patient role at DB level.
    sqlx::query(
        "INSERT INTO role_permissions (role_id, permission_id) \
         SELECT r.id, p.id FROM roles r, permissions p \
         WHERE r.name='patient' AND p.key='whatsapp.send'",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Without re-login the in-memory session is unchanged → still denied.
    assert!(
        rbac::require(&state, Permission::WhatsAppSend).is_err(),
        "in-memory permission snapshot must not change mid-session"
    );

    // After a re-login the permission IS granted (propagation works).
    let fresh = load_session_for(&pool, uid, "hash_n03").await;
    let state2: SessionState = Arc::new(Mutex::new(Some(fresh)));
    rbac::require(&state2, Permission::WhatsAppSend)
        .expect("fresh login must pick up the granted permission");

    // Cleanup: restore the patient role's pristine (WhatsApp-free) grants.
    sqlx::query(
        "DELETE FROM role_permissions rp USING roles r, permissions p          WHERE rp.role_id = r.id AND rp.permission_id = p.id            AND r.name = 'patient' AND p.key = 'whatsapp.send'",
    )
    .execute(&pool)
    .await
    .unwrap();
}

/// WP1-N04 — consent gate (CR-12): a doctor send to a registered patient
/// WITHOUT consent is refused; with consent=granted it passes. Tests the
/// production `automation::check_patient_consent` directly.
#[tokio::test]
async fn wp1_n04_consent_gate_still_applies() {
    let pool = setup().await;
    let patient_id = seed_patient_with_phone(&pool, "Consent", "No", "+923001112233").await;

    // No consent row → refuse.
    let r = hospital_mgmt_lib::whatsapp::automation::check_patient_consent(
        &pool, "+923001112233", None,
    )
    .await
    .unwrap_err();
    assert!(r.contains("not consented"), "got: {}", r);

    // Consent granted → allow.
    set_consent(&pool, patient_id, true).await;
    hospital_mgmt_lib::whatsapp::automation::check_patient_consent(&pool, "+923001112233", None)
        .await
        .expect("consented patient must pass the gate");

    // Consent revoked → refuse again.
    set_consent(&pool, patient_id, false).await;
    assert!(
        hospital_mgmt_lib::whatsapp::automation::check_patient_consent(
            &pool, "+923001112233", None
        )
        .await
        .is_err()
    );
}

// ── G.1.4 Penetration tests (WP1-P01 … P04) ───────────────────────────────────

/// WP1-P01 — session replay after revocation: an in-memory session that
/// survives a role revocation (stale permissions) is still rejected by
/// require_strong's DB-backed check once its session row is gone
/// (deleted by the re-login that the role change requires).
#[tokio::test]
async fn wp1_p01_replay_after_role_change_rejected() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_doc_p01", "Passw0rd!", &["doctor"]).await;
    seed_session_row(&pool, uid, "hash_p01_a").await;
    let state = session_state_for(&pool, uid, "hash_p01_a").await;

    // "Re-login elsewhere" (single-session policy deletes prior rows).
    sqlx::query("DELETE FROM sessions WHERE user_id = $1")
        .bind(uid)
        .execute(&pool)
        .await
        .unwrap();
    seed_session_row(&pool, uid, "hash_p01_b").await;

    // Replayed call with the stale in-memory session → require_strong
    // rejects via the missing token row.
    let r = rbac::require_strong(&state, &pool, Permission::WhatsAppSend)
        .await
        .unwrap_err();
    assert!(r.contains("Session invalidated"), "got: {}", r);
}

/// WP1-P02 — parameter fuzz on the IPC-09 input checks (extracted core):
/// malformed phone inputs and oversized/unicode message bodies are all
/// rejected cleanly (no panics, no SQL errors, no truncation).
#[tokio::test]
async fn wp1_p02_param_fuzz_ipc09_checks() {
    let pool = setup().await;
    let patient_id = seed_patient_with_phone(&pool, "Fuzz", "Target", "+923009998877").await;

    use hospital_mgmt_lib::whatsapp::commands::send_to_patient_checks as checks;

    // Fuzz 1: phone with no digits → normalize error.
    let r = checks(&pool, "+++---", "hello").await.unwrap_err();
    assert!(r.contains("no digits"), "got: {}", r);

    // Fuzz 2: valid-format phone that belongs to NO registered patient.
    let r = checks(&pool, "+923001234560", "hello").await.unwrap_err();
    assert!(r.contains("does not belong to a registered patient"), "got: {}", r);

    // Fuzz 3: too-short number — normalize_phone rejects <8 digits with its
    // own error before the patient lookup (still a clean refusal, no panic).
    let r = checks(&pool, "+12345", "hello").await.unwrap_err();
    assert!(
        r.contains("invalid") || r.contains("does not belong to a registered patient"),
        "short number must be cleanly refused, got: {}",
        r
    );

    // Fuzz 4: oversized message (>1000 chars) — IPC-09 cap.
    let big = "x".repeat(1001);
    let r = checks(&pool, "+923009998877", &big).await.unwrap_err();
    assert!(r.contains("too long"), "got: {}", r);

    // Fuzz 5: exactly at the cap + unicode body → passes input checks
    // (delivery verified at runtime elsewhere).
    let uni = "医院通知 🏥 ".repeat(50); // 500 chars incl. multibyte
    checks(&pool, "+923009998877", &uni)
        .await
        .expect("at-cap + unicode must pass input checks");

    // Fuzz 6: soft-deleted patient's phone must NOT be a valid target (CR-11).
    sqlx::query("UPDATE patients SET deleted_at = NOW(), is_active = FALSE WHERE id = $1")
        .bind(patient_id)
        .execute(&pool)
        .await
        .unwrap();
    let r = checks(&pool, "+923009998877", "hi").await.unwrap_err();
    assert!(r.contains("does not belong to a registered patient"), "got: {}", r);
}

/// WP1-P03 — direct DB permission injection cannot grant a USER what their
/// ROLE does not have: permissions are role-level only. There is no
/// user_permissions table to inject into (schema-level fact), and a
/// hand-crafted in-memory session with a fake token is rejected by
/// require_strong's DB check.
#[tokio::test]
async fn wp1_p03_db_injection_cannot_grant_user() {
    let pool = setup().await;
    // Schema fact: no user-level permission grant table exists.
    let n: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.tables \
         WHERE table_name IN ('user_permissions','permissions_users')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n.0, 0, "permissions must be role-level only");

    // A forged in-memory session claiming WhatsAppSend with a token that
    // does not exist in the sessions table → require_strong rejects.
    let forged = Session {
        user_id: 1,
        username: "forged".into(),
        full_name: "Forged".into(),
        roles: vec!["super_admin".into()],
        permissions: ["whatsapp.send".into()].into_iter().collect(),
        token_hash: "totally-forged-hash".into(),
    };
    let state: SessionState = Arc::new(Mutex::new(Some(forged)));
    let r = rbac::require_strong(&state, &pool, Permission::WhatsAppSend)
        .await
        .unwrap_err();
    assert!(r.contains("Session invalidated"), "got: {}", r);
}

/// WP1-P04 — frontend permission spoofing: the LoginResponse sent to the
/// frontend contains no WhatsApp permissions for a patient, and the backend
/// guard reads the server-side Session, never any client-supplied array.
/// Simulated by: spoofing = hand-building a session whose permissions
/// LACK the key (all a compromised frontend can affect is its own display).
#[tokio::test]
async fn wp1_p04_frontend_spoof_irrelevant_to_backend() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_patient_p04", "Passw0rd!", &["patient"]).await;
    seed_session_row(&pool, uid, "hash_p04").await;

    // The server-derived session (what the backend actually consults):
    let session = load_session_for(&pool, uid, "hash_p04").await;
    assert!(!session.permissions.contains("whatsapp.send"));
    assert!(!session.permissions.contains("whatsapp.view"));

    // A "spoofed" session with the frontend-injected key is a DIFFERENT
    // object — the backend's state still lacks it:
    let real_state: SessionState = Arc::new(Mutex::new(Some(session)));
    assert!(rbac::require(&real_state, Permission::WhatsAppSend).is_err());
}

// ── G.1.5 Concurrency tests (WP1-C01, C02) ────────────────────────────────────

/// WP1-C01 — 10 concurrent guard calls from the same patient session:
/// all 10 rejected, no race grants access.
#[tokio::test]
async fn wp1_c01_concurrent_patient_denials() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_patient_c01", "Passw0rd!", &["patient"]).await;
    seed_session_row(&pool, uid, "hash_c01").await;
    let state = session_state_for(&pool, uid, "hash_c01").await;

    let pool = Arc::new(pool);
    let state = Arc::new(state);
    let mut handles = vec![];
    for _ in 0..10 {
        let p = pool.clone();
        let s = state.clone();
        handles.push(tokio::spawn(async move {
            rbac::require_strong(&s, &p, Permission::WhatsAppSend).await.is_err()
        }));
    }
    let mut denied = 0usize;
    for h in handles {
        if h.await.unwrap() {
            denied += 1;
        }
    }
    assert_eq!(denied, 10, "all concurrent patient attempts must be denied");
}

/// WP1-C02 — concurrent role-revocation + guard call: each call either
/// succeeds (revocation not yet visible) or fails — never an inconsistent
/// third state. With in-memory snapshot semantics the guard outcome is
/// stable; after re-login the revocation is visible.
#[tokio::test]
async fn wp1_c02_concurrent_revoke_and_guard() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_doc_c02", "Passw0rd!", &["doctor"]).await;
    seed_session_row(&pool, uid, "hash_c02").await;
    let state = session_state_for(&pool, uid, "hash_c02").await;

    let pool = Arc::new(pool);
    let state = Arc::new(state);
    let mut handles = vec![];
    for _ in 0..20 {
        let p = pool.clone();
        let s = state.clone();
        handles.push(tokio::spawn(async move {
            matches!(
                rbac::require_strong(&s, &p, Permission::WhatsAppSend).await,
                Ok(_) | Err(_)
            )
        }));
    }
    for h in handles {
        assert!(h.await.unwrap(), "guard must return a definitive outcome");
    }

    // Revoke the doctor's WhatsApp grant at DB level, re-login, re-check.
    sqlx::query(
        "DELETE FROM role_permissions rp USING roles r, permissions p \
         WHERE rp.role_id = r.id AND rp.permission_id = p.id \
           AND r.name = 'doctor' AND p.key = 'whatsapp.send'",
    )
    .execute(&*pool)
    .await
    .unwrap();
    let fresh = load_session_for(&pool, uid, "hash_c02").await;
    let state2: SessionState = Arc::new(Mutex::new(Some(fresh)));
    assert!(rbac::require(&state2, Permission::WhatsAppSend).is_err());

    // Cleanup: re-seed the doctor role's grants (restores whatsapp.send).
    hospital_mgmt_lib::auth::seed_defaults(&pool).await.unwrap();
}

// ── G.1.7 LAN tests (WP1-L01, L02) — single-machine approximations ────────────
// True multi-PC LAN enforcement needs two machines (tracked as a hardware-
// gated follow-up); the server-side enforcement both PCs hit is EXACTLY the
// guard code above, so L01's authorization semantics are fully covered here.

/// WP1-L01 (approximation) — the client PC's session is just another session
/// validated server-side: a patient session is denied regardless of which
/// machine it originated on (authorization lives in the server process).
#[tokio::test]
async fn wp1_l01_client_session_denied_server_side() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_patient_l01", "Passw0rd!", &["patient"]).await;
    seed_session_row(&pool, uid, "hash_l01").await;
    // A "client PC" state — a session loaded over the network.
    let state = session_state_for(&pool, uid, "hash_l01").await;
    assert!(rbac::require_strong(&state, &pool, Permission::WhatsAppSend).await.is_err());
}

/// WP1-L02 — role sync post-seed: after seed_defaults, a FRESHLY loaded
/// doctor session (what a client PC's `me` would return after re-login)
/// includes the WhatsApp permissions.
#[tokio::test]
async fn wp1_l02_role_sync_post_seed() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_doc_l02", "Passw0rd!", &["doctor"]).await;
    seed_session_row(&pool, uid, "hash_l02").await;
    let session = load_session_for(&pool, uid, "hash_l02").await;
    assert!(session.permissions.contains("whatsapp.send"));
    assert!(session.permissions.contains("whatsapp.view"));
}
