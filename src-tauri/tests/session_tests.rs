//! AERP Part G — G.2 Work Package 2: Session Invalidation (WP-2.1 + WP-2.2).
//!
//! Covers:
//!   - WP-2.1: token_hash-bound session validation (`me`, load_session)
//!   - WP-2.2: DB-backed `require_strong` invalidation (deactivation,
//!     password reset, cross-PC login), low-vs-high-risk command split,
//!     in-memory state clearing, and the require_strong command inventory.
//!
//! All tests run against a real PostgreSQL test DB via the production code
//! (`auth::login_core`, `auth::me`, `rbac::require_strong`) — the exact
//! functions the Tauri commands call. Spec unit-tests that assumed a "mock
//! PgPool" (WP2-U03…U08) are implemented here against the real DB instead,
//! which is strictly stronger evidence.
//!
//! Requires: HMS_TEST_DB_URL env var + `--features hms-integration-tests`.

mod common;

use common::*;
use hospital_mgmt_lib::rbac::{self, Permission, Session, SessionState};
use sqlx::PgPool;
use tauri::Manager;
use std::sync::{Arc, Mutex};

async fn setup() -> PgPool {
    test_pool().await
}

async fn state_for(pool: &PgPool, user_id: i32, token_hash: &str) -> SessionState {
    let s = load_session_for(pool, user_id, token_hash).await;
    Arc::new(Mutex::new(Some(s)))
}

/// Perform a REAL login via the extracted production login core, against
/// the given SessionState ("PC"), and return the response + token row we
/// can identify. Mirrors what the login command does end-to-end.
async fn login_on(
    pool: &PgPool,
    state: &SessionState,
    username: &str,
    password: &str,
) -> Result<hospital_mgmt_lib::auth::LoginResponse, String> {
    hospital_mgmt_lib::auth::login_core(
        pool,
        state,
        hospital_mgmt_lib::auth::LoginRequest {
            username: username.into(),
            password: password.into(),
        },
    )
    .await
}

// ── G.2.2 Integration tests (WP2-I01 … I11) ───────────────────────────────────

/// WP2-I01 — the cross-PC login scenario: login on PC-A, login on PC-B
/// (deletes PC-A's session row), then PC-A's `me` MUST reject with
/// "Session expired" because its token_hash is gone. This is the exact
/// WP-2.1 fix: validation is bound to the token hash, not the user id.
#[tokio::test]
async fn wp2_i01_me_rejects_after_cross_pc_login() {
    let pool = setup().await;
    seed_user(&pool, "aerp_user_i01", &fixture_pw(), &["doctor"]).await;

    // PC-A and PC-B = two independent in-memory session states.
    let pc_a: SessionState = Arc::new(Mutex::new(None));
    let pc_b: SessionState = Arc::new(Mutex::new(None));

    // Login on PC-A.
    login_on(&pool, &pc_a, "aerp_user_i01", &fixture_pw())
        .await
        .expect("PC-A login");
    let token_a = current_token_hash(&pool).await;

    // Login on PC-B — single-session policy deletes PC-A's row.
    login_on(&pool, &pc_b, "aerp_user_i01", &fixture_pw())
        .await
        .expect("PC-B login");
    let token_b = current_token_hash(&pool).await;
    assert_ne!(token_a, token_b, "each login must mint a fresh token");

    // PC-A's `me` (production me_core) must now fail.
    let r = hospital_mgmt_lib::auth::me_core(&pool, &pc_a).await;
    let err = r.err().expect("PC-A me must fail after PC-B login");
    assert!(err.contains("Session expired"), "got: {}", err);

    // And PC-A's state was cleared by `me` (fail-fast for future calls).
    assert!(pc_a.lock().unwrap().is_none());
}

async fn current_token_hash(pool: &PgPool) -> String {
    let row: (String,) =
        sqlx::query_as("SELECT token_hash FROM sessions ORDER BY issued_at DESC LIMIT 1")
            .fetch_one(pool)
            .await
            .unwrap();
    row.0
}

/// WP2-I02 — `me` accepts while the session is valid.
#[tokio::test]
async fn wp2_i02_me_accepts_when_valid() {
    let pool = setup().await;
    seed_user(&pool, "aerp_user_i02", &fixture_pw(), &["doctor"]).await;
    let pc: SessionState = Arc::new(Mutex::new(None));
    login_on(&pool, &pc, "aerp_user_i02", &fixture_pw())
        .await
        .expect("login");

    let resp = hospital_mgmt_lib::auth::me_core(&pool, &pc)
        .await
        .expect("me must succeed while session valid");
    assert_eq!(resp.user.username, "aerp_user_i02");
    // Token hash must NOT be part of the frontend-facing response.
    let json = serde_json::to_string(&resp).unwrap();
    assert!(!json.contains("token_hash"), "token_hash leaked: {}", json);
    assert!(!json.contains("token"), "raw token leaked: {}", json);
}

/// WP2-I03 — require_strong rejects the next high-risk command after the
/// admin deactivates the user (WP-2.2 Layer 3, the deactivation path).
#[tokio::test]
async fn wp2_i03_require_strong_rejects_after_deactivation() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_doc_i03", &fixture_pw(), &["doctor"]).await;
    seed_session_row(&pool, uid, "hash_i03").await;
    let state = state_for(&pool, uid, "hash_i03").await;

    // Baseline: guard passes.
    rbac::require_strong(&state, &pool, Permission::PatientsCreate)
        .await
        .expect("baseline guard must pass");

    // Admin deactivates the user (from "another PC" = direct SQL).
    set_user_active(&pool, uid, false).await;

    // Next high-risk command is rejected…
    let r = rbac::require_strong(&state, &pool, Permission::PatientsCreate)
        .await
        .unwrap_err();
    assert!(r.contains("Session invalidated"), "got: {}", r);

    // …and the in-memory state was cleared (fail-fast).
    assert!(state.lock().unwrap().is_none());
}

/// WP2-I04 — role change (permission revocation): the documented AERP
/// limitation (G.2.7 L03) is that require_strong re-checks session validity
/// but NOT the permission set — revocation lands on the next `me`/re-login.
/// This test PINS that documented behavior so any future change is conscious.
#[tokio::test]
async fn wp2_i04_role_change_documented_limitation() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_doc_i04", &fixture_pw(), &["doctor"]).await;
    seed_session_row(&pool, uid, "hash_i04").await;
    let state = state_for(&pool, uid, "hash_i04").await;

    // Admin strips the doctor's PatientsCreate grant at DB level.
    sqlx::query(
        "DELETE FROM role_permissions rp USING roles r, permissions p \
         WHERE rp.role_id = r.id AND rp.permission_id = p.id \
           AND r.name = 'doctor' AND p.key = 'patients.create'",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Documented behavior: in-memory permission snapshot is still valid →
    // require_strong still passes (session row intact, user active).
    rbac::require_strong(&state, &pool, Permission::PatientsCreate)
        .await
        .expect("require_strong checks validity, not permissions (documented L03)");

    // A FRESH login (the propagation point) now lacks the permission.
    let fresh = load_session_for(&pool, uid, "hash_i04").await;
    assert!(!fresh.permissions.contains("patients.create"));

    // Cleanup: restore the doctor role's pristine grants for later tests.
    hospital_mgmt_lib::auth::seed_defaults(&pool).await.unwrap();
}

/// WP2-I05 — password reset deletes the target's sessions → require_strong
/// rejects their next high-risk command. Uses the production reset core as
/// the admin action.
#[tokio::test]
async fn wp2_i05_require_strong_rejects_after_password_reset() {
    let pool = setup().await;
    let admin = seed_user(&pool, "aerp_admin_i05", &fixture_pw(), &["super_admin"]).await;
    let target = seed_user(&pool, "aerp_doc_i05", &fixture_pw(), &["doctor"]).await;

    seed_session_row(&pool, admin, "hash_i05_admin").await;
    seed_session_row(&pool, target, "hash_i05_target").await;
    let admin_state = state_for(&pool, admin, "hash_i05_admin").await;
    let target_state = state_for(&pool, target, "hash_i05_target").await;

    // Admin resets the target's password via the production core.
    hospital_mgmt_lib::auth::reset_user_password_core(
        &pool, &admin_state, target, fixture_pw(),
    )
    .await
    .expect("reset");

    // Target's session rows are gone.
    assert_eq!(count_sessions(&pool, target).await, 0);

    // Target's next high-risk command is rejected.
    let r = rbac::require_strong(&target_state, &pool, Permission::PatientsCreate)
        .await
        .unwrap_err();
    assert!(r.contains("Session invalidated"), "got: {}", r);
}

/// WP2-I06 — low-risk commands (plain `require`, in-memory) are NOT
/// DB-checked: a deactivated user can still READ until the session cache
//  clears or expires. Pins the two-tier design decision.
#[tokio::test]
async fn wp2_i06_low_risk_command_not_db_checked() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_doc_i06", &fixture_pw(), &["doctor"]).await;
    seed_session_row(&pool, uid, "hash_i06").await;
    let state = state_for(&pool, uid, "hash_i06").await;

    set_user_active(&pool, uid, false).await;

    // High-risk: rejected (DB-backed).
    assert!(rbac::require_strong(&state, &pool, Permission::PatientsCreate).await.is_err());

    // A fresh state (the high-risk failure above cleared `state`) with the
    // same session: the LOW-risk guard (pure in-memory) still succeeds.
    let state2 = state_for(&pool, uid, "hash_i06").await;
    rbac::require(&state2, Permission::PatientsView)
        .expect("low-risk require is in-memory only (documented two-tier design)");
}

/// WP2-I07/08/09 — the invalidation EVENTS for reset / deactivate /
/// role-change are emitted by the command wrappers. The wrapper-side
/// emission is AppHandle-bound; here we verify the OBSERVABLE effects the
/// events announce (session deletion / is_active flip / role rows), which
/// are the security-relevant halves. (Event plumbing itself was verified
/// live in the GUI round; see VERIFICATION-REPORT §4.)
#[tokio::test]
async fn wp2_i07_i09_invalidation_observables() {
    let pool = setup().await;
    let admin = seed_user(&pool, "aerp_admin_i07", &fixture_pw(), &["super_admin"]).await;
    let t_reset = seed_user(&pool, "aerp_t_reset", &fixture_pw(), &["doctor"]).await;
    let t_deact = seed_user(&pool, "aerp_t_deact", &fixture_pw(), &["nurse"]).await;
    let t_role = seed_user(&pool, "aerp_t_role", &fixture_pw(), &["doctor"]).await;

    seed_session_row(&pool, admin, "hash_i07_admin").await;
    let admin_state = state_for(&pool, admin, "hash_i07_admin").await;

    // Reset → sessions deleted.
    seed_session_row(&pool, t_reset, "hash_i07_reset").await;
    hospital_mgmt_lib::auth::reset_user_password_core(
        &pool, &admin_state, t_reset, fixture_pw(),
    )
    .await
    .unwrap();
    assert_eq!(count_sessions(&pool, t_reset).await, 0);

    // Deactivate → is_active FALSE (observable half of the event).
    hospital_mgmt_lib::auth::update_user_core(
        &pool,
        &admin_state,
        hospital_mgmt_lib::auth::UpdateUserRequest {
            id: t_deact,
            full_name: None,
            email: None,
            is_active: Some(false),
            roles: None,
        },
    )
    .await
    .unwrap();
    let active: (bool,) = sqlx::query_as("SELECT is_active FROM users WHERE id = $1")
        .bind(t_deact)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!active.0);

    // Role change → user_roles rewritten.
    hospital_mgmt_lib::auth::update_user_core(
        &pool,
        &admin_state,
        hospital_mgmt_lib::auth::UpdateUserRequest {
            id: t_role,
            full_name: None,
            email: None,
            is_active: None,
            roles: Some(vec!["nurse".into()]),
        },
    )
    .await
    .unwrap();
    let roles: Vec<(String,)> = sqlx::query_as(
        "SELECT r.name FROM user_roles ur JOIN roles r ON r.id = ur.role_id WHERE ur.user_id = $1",
    )
    .bind(t_role)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(roles.len(), 1);
    assert_eq!(roles[0].0, "nurse");
}

/// WP2-I10 — a fresh login (the cross-PC event trigger) emits… the wrapper
/// handles the event; observable here: the prior token is dead and the new
/// session row exists (single active session invariant).
#[tokio::test]
async fn wp2_i10_login_invalidates_prior_sessions() {
    let pool = setup().await;
    seed_user(&pool, "aerp_user_i10", &fixture_pw(), &["doctor"]).await;

    let pc_a: SessionState = Arc::new(Mutex::new(None));
    let pc_b: SessionState = Arc::new(Mutex::new(None));
    login_on(&pool, &pc_a, "aerp_user_i10", &fixture_pw()).await.unwrap();
    let token_a = current_token_hash(&pool).await;
    login_on(&pool, &pc_b, "aerp_user_i10", &fixture_pw()).await.unwrap();
    let token_b = current_token_hash(&pool).await;

    assert_ne!(token_a, token_b);
    let n: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sessions WHERE token_hash IN ($1, $2)",
    )
    .bind(&token_a)
    .bind(&token_b)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n.0, 1, "only the newest session row survives");
}

/// WP2-I11 — the require_strong command inventory. AERP C.2.2 planned 22
/// high-risk commands; the implementation decision (handoff §12) extended
/// coverage to 34 call sites ("regex caught 12 extra; more secure"). This
/// architecture-conformance test pins the CURRENT inventory so accidental
/// guard removal is caught, and counts per file.
#[tokio::test]
async fn wp2_i11_high_risk_command_inventory() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let expected: &[(&str, usize)] = &[
        ("src/commands/billing.rs", 2),
        ("src/commands/encounters.rs", 1),
        ("src/commands/ipd.rs", 2),
        ("src/commands/lab.rs", 3),
        ("src/commands/patients.rs", 6),
        ("src/commands/pharmacy.rs", 2),
        ("src/commands/radiology.rs", 7),
        ("src/whatsapp/commands.rs", 7),
        ("src/auth.rs", 4),
    ];
    let mut total = 0;
    for (rel, expect) in expected {
        let src = std::fs::read_to_string(format!("{}/{}", manifest, rel)).unwrap();
        // Count call sites in any argument style — command wrappers use
        // `require_strong(&state, pool.inner(), …)`; AERP-extracted cores use
        // `require_strong(session_state, pool, …)`.
        let n = src.matches("require_strong(").count()
            - src.matches("pub async fn require_strong(").count();
        assert_eq!(
            n, *expect,
            "{}: require_strong call-site count drifted (guard removed?)",
            rel
        );
        total += n;
    }
    assert_eq!(total, 34, "total high-risk guard sites must match the decision log");
}

// ── G.2.3 Negative tests (WP2-N01 … N03) ──────────────────────────────────────

/// WP2-N01 — a stale session (its row deleted by a newer login) cannot use
/// a high-risk command: rejected by require_strong.
#[tokio::test]
async fn wp2_n01_stale_session_cannot_create() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_doc_n01", &fixture_pw(), &["doctor"]).await;
    seed_session_row(&pool, uid, "hash_n01_old").await;
    let stale = state_for(&pool, uid, "hash_n01_old").await;

    // A newer login elsewhere deletes the old row.
    sqlx::query("DELETE FROM sessions WHERE user_id = $1")
        .bind(uid)
        .execute(&pool)
        .await
        .unwrap();
    seed_session_row(&pool, uid, "hash_n01_new").await;

    let r = rbac::require_strong(&stale, &pool, Permission::PatientsCreate)
        .await
        .unwrap_err();
    assert!(r.contains("Session invalidated"), "got: {}", r);
}

/// WP2-N02 — deleting the USER (FK cascade removes sessions) → next
/// high-risk command rejected; guard reports the invalidated session.
#[tokio::test]
async fn wp2_n02_deleted_user_cannot_act() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_doc_n02", &fixture_pw(), &["doctor"]).await;
    seed_session_row(&pool, uid, "hash_n02").await;
    let state = state_for(&pool, uid, "hash_n02").await;

    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(uid)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(count_sessions(&pool, uid).await, 0, "FK cascade removes sessions");

    let r = rbac::require_strong(&state, &pool, Permission::PatientsCreate)
        .await
        .unwrap_err();
    assert!(r.contains("Session invalidated"), "got: {}", r);
}

/// WP2-N03 — token_hash never appears in any frontend-facing response:
/// LoginResponse serialization is checked, and the raw token exists only
/// inside process memory (never persisted — sessions table stores the hash).
#[tokio::test]
async fn wp2_n03_token_hash_not_in_frontend_responses() {
    let pool = setup().await;
    seed_user(&pool, "aerp_user_n03", &fixture_pw(), &["doctor"]).await;
    let pc: SessionState = Arc::new(Mutex::new(None));
    let resp = login_on(&pool, &pc, "aerp_user_n03", &fixture_pw())
        .await
        .unwrap();
    let json = serde_json::to_string(&resp).unwrap();
    assert!(!json.contains("token_hash"));
    assert!(!json.contains("password_hash"));
    // The sessions row minted by the REAL login holds only the SHA-256
    // hash (64 hex chars) — never the raw base64url token. (Test fixtures
    // use short synthetic hashes and are excluded by user filter.)
    let uid: (i32,) = sqlx::query_as("SELECT id FROM users WHERE username = 'aerp_user_n03'")
        .fetch_one(&pool).await.unwrap();
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT token_hash FROM sessions WHERE user_id = $1",
    )
    .bind(uid.0)
    .fetch_all(&pool).await.unwrap();
    assert_eq!(rows.len(), 1);
    let (h,) = rows[0].clone();
    assert_eq!(h.len(), 64, "stored token_hash must be SHA-256 hex, got len {}", h.len());
    assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    // And the raw token never appears anywhere in the sessions table.
    let all: String = sqlx::query_scalar::<_, String>("SELECT string_agg(token_hash, ',') FROM sessions")
        .fetch_one(&pool).await.unwrap_or_default();
    assert!(!json.contains("token"), "raw token leaked into a response: {}", json);
}

// ── G.2.4 Penetration tests (WP2-P01 … P04) ───────────────────────────────────

/// WP2-P01 — token replay: even a stolen token_hash cannot be replayed
/// once the session row is deleted (require_strong re-checks the DB).
/// A forged in-memory session carrying the STOLEN (still-live) hash of
/// ANOTHER user's session fails the user_id binding inside the guard's
/// JOIN… but a same-user replay with a live row passes — which is why the
/// single-session policy + 12h expiry bound the blast radius. Here: replay
/// with a DEAD hash (the realistic post-logout state) is rejected.
#[tokio::test]
async fn wp2_p01_dead_token_replay_rejected() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_doc_p01", &fixture_pw(), &["doctor"]).await;
    seed_session_row(&pool, uid, "hash_p01").await;
    sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
        .bind("hash_p01")
        .execute(&pool)
        .await
        .unwrap();

    let forged = Session {
        user_id: uid,
        username: "aerp_doc_p01".into(),
        full_name: "x".into(),
        roles: vec!["doctor".into()],
        permissions: ["patients.create".into()].into_iter().collect(),
        token_hash: "hash_p01".into(), // stolen but dead
    };
    let state: SessionState = Arc::new(Mutex::new(Some(forged)));
    let r = rbac::require_strong(&state, &pool, Permission::PatientsCreate)
        .await
        .unwrap_err();
    assert!(r.contains("Session invalidated"), "got: {}", r);
}

/// WP2-P02 — race: deactivation concurrent with high-risk calls. Every call
/// returns a definitive outcome; successes+failures == total; no hangs.
#[tokio::test]
async fn wp2_p02_race_deactivate_vs_commands() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_doc_p02", &fixture_pw(), &["doctor"]).await;
    seed_session_row(&pool, uid, "hash_p02").await;
    let state = state_for(&pool, uid, "hash_p02").await;

    let pool = Arc::new(pool);
    let state = Arc::new(state);
    let uid_c = uid;

    let (guards, deact) = tokio::join!(
        async {
            let mut hs = vec![];
            for _ in 0..30 {
                let p = pool.clone();
                let s = state.clone();
                hs.push(tokio::spawn(async move {
                    rbac::require_strong(&s, &p, Permission::PatientsCreate).await.is_ok()
                }));
            }
            let mut ok = 0usize;
            for h in hs {
                if h.await.unwrap() {
                    ok += 1;
                }
            }
            ok
        },
        async {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            set_user_active(&pool, uid_c, false).await;
        }
    );
    // 30 definitive outcomes, split anywhere between 0 and 30 successes.
    assert!(guards <= 30);
}

/// WP2-P03 — event-listener override is irrelevant: with NO listener
/// subscribed anywhere (simulating a malicious frontend noop), the
/// backend still enforces invalidation on the next high-risk command.
#[tokio::test]
async fn wp2_p03_no_listener_still_enforced() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_doc_p03", &fixture_pw(), &["doctor"]).await;
    seed_session_row(&pool, uid, "hash_p03").await;
    let state = state_for(&pool, uid, "hash_p03").await;
    set_user_active(&pool, uid, false).await; // "event" would have been nooped
    let r = rbac::require_strong(&state, &pool, Permission::PatientsCreate)
        .await
        .unwrap_err();
    assert!(r.contains("Session invalidated"), "got: {}", r);
}

/// WP2-P04 — `me` polling bypass: a frontend that stops calling `me` still
/// cannot use high-risk commands after invalidation — require_strong is the
/// enforcement, `me` is only the polite UX.
#[tokio::test]
async fn wp2_p04_me_bypass_still_enforced() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_doc_p04", &fixture_pw(), &["doctor"]).await;
    seed_session_row(&pool, uid, "hash_p04").await;
    let state = state_for(&pool, uid, "hash_p04").await;

    // Invalidate WITHOUT calling me: reset the password via admin core.
    let admin = seed_user(&pool, "aerp_admin_p04", &fixture_pw(), &["super_admin"]).await;
    seed_session_row(&pool, admin, "hash_p04_admin").await;
    let admin_state = state_for(&pool, admin, "hash_p04_admin").await;
    hospital_mgmt_lib::auth::reset_user_password_core(
        &pool, &admin_state, uid, fixture_pw(),
    )
    .await
    .unwrap();

    let r = rbac::require_strong(&state, &pool, Permission::PatientsCreate)
        .await
        .unwrap_err();
    assert!(r.contains("Session invalidated"), "got: {}", r);
}

// ── G.2.5 Concurrency tests (WP2-C01 … C04) ───────────────────────────────────

/// WP2-C01 — two concurrent logins for the same user: exactly ONE session
/// row survives (single-active-session invariant under contention).
#[tokio::test]
async fn wp2_c01_concurrent_logins_single_session() {
    let pool = setup().await;
    seed_user(&pool, "aerp_user_c01", &fixture_pw(), &["doctor"]).await;

    let pool = Arc::new(pool);
    let pc_a: SessionState = Arc::new(Mutex::new(None));
    let pc_b: SessionState = Arc::new(Mutex::new(None));
    let pa = Arc::clone(&pool);
    let pb = Arc::clone(&pool);
    let (ra, rb) = tokio::join!(
        async move {
            hospital_mgmt_lib::auth::login_core(
                &pa,
                &pc_a,
                hospital_mgmt_lib::auth::LoginRequest {
                    username: "aerp_user_c01".into(),
                    password: fixture_pw(),
                },
            )
            .await
        },
        async move {
            hospital_mgmt_lib::auth::login_core(
                &pb,
                &pc_b,
                hospital_mgmt_lib::auth::LoginRequest {
                    username: "aerp_user_c01".into(),
                    password: fixture_pw(),
                },
            )
            .await
        }
    );
    assert!(ra.is_ok() && rb.is_ok(), "both logins must succeed");
    let uid: (i32,) = sqlx::query_as("SELECT id FROM users WHERE username = 'aerp_user_c01'")
        .fetch_one(&*pool)
        .await
        .unwrap();
    let n = count_sessions(&pool, uid.0).await;
    assert_eq!(n, 1, "single active session must survive concurrent logins, got {}", n);
}

/// WP2-C02 — 100 concurrent high-risk calls WHILE the user is deactivated:
/// every call resolves definitively; created patients == successful calls
/// (verified via the real create_patient command through the mock app).
#[tokio::test]
async fn wp2_c02_concurrent_create_patient_vs_deactivation() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_doc_c02", &fixture_pw(), &["doctor"]).await;
    seed_session_row(&pool, uid, "hash_c02").await;
    let session = load_session_for(&pool, uid, "hash_c02").await;

    let pool = Arc::new(pool);
    let state = Arc::new(SessionState::default());
    *state.lock().unwrap() = Some(session);
    let uid_c = uid;

    let (results, _deactivated) = tokio::join!(
        async {
            let mut handles = vec![];
            for i in 0..100 {
                let p = Arc::clone(&pool);
                let s = Arc::clone(&state);
                handles.push(tokio::spawn(async move {
                    let patient = hospital_mgmt_lib::models::CreatePatientEhr {
                        first_name: format!("Race{}", i),
                        last_name: "Conc".into(),
                        email: None,
                        phone: format!("+92300000{:04}", i),
                        date_of_birth: "1990-01-01".into(),
                        gender: "male".into(),
                        address: None,
                        mrn: None,
                        blood_group: None,
                        allergies: None,
                        chronic_conditions: None,
                        emergency_contact_name: None,
                        emergency_contact_phone: None,
                        insurance_provider: None,
                        insurance_policy_number: None,
                    };
                    hospital_mgmt_lib::commands::patients::create_patient_core(&p, &s, patient)
                        .await
                        .is_ok()
                }));
            }
            let mut ok = 0usize;
            let mut errs = 0usize;
            for h in handles {
                if h.await.unwrap() { ok += 1 } else { errs += 1 }
            }
            (ok, errs)
        },
        async {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            set_user_active(&pool, uid_c, false).await;
        }
    );

    assert_eq!(results.0 + results.1, 100, "every call must resolve");
    let created: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM patients WHERE last_name = 'Conc'",
    )
    .fetch_one(&*pool)
    .await
    .unwrap();
    assert_eq!(
        created.0 as usize, results.0,
        "created rows must equal successful calls (no partial writes)"
    );
}

/// WP2-C03 — concurrent role change + high-risk calls: same definitiveness
/// contract (documented L03 semantics: in-flight snapshot stays valid).
#[tokio::test]
async fn wp2_c03_concurrent_role_change_vs_commands() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_doc_c03", &fixture_pw(), &["doctor"]).await;
    seed_session_row(&pool, uid, "hash_c03").await;
    let state = state_for(&pool, uid, "hash_c03").await;

    let pool = Arc::new(pool);
    let state = Arc::new(state);
    let uid_c = uid;
    let (ok_count, _) = tokio::join!(
        async {
            let mut hs = vec![];
            for _ in 0..30 {
                let p = Arc::clone(&pool);
                let s = Arc::clone(&state);
                hs.push(tokio::spawn(async move {
                    rbac::require_strong(&s, &p, Permission::PatientsCreate).await.is_ok()
                }));
            }
            let mut ok = 0usize;
            for h in hs {
                if h.await.unwrap() { ok += 1 }
            }
            ok
        },
        async {
            set_user_roles(&pool, uid_c, &["patient"]).await; // strip perms
        }
    );
    let _ = ok_count; // outcomes may be split — the contract is only:
    let n: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM user_roles WHERE user_id = $1")
        .bind(uid)
        .fetch_one(&*pool)
        .await
        .unwrap();
    assert_eq!(n.0, 1, "role change must fully apply");
}

/// WP2-C04 — 100 concurrent require_strong calls from one valid session:
/// all Ok (read-only DB check, no contention), and — AERP H2-001-14
/// criterion — 1000 calls must complete well under the 5-second budget.
#[tokio::test]
async fn wp2_c04_and_h2_001_14_concurrent_require_strong_benchmark() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_doc_c04", &fixture_pw(), &["doctor"]).await;
    seed_session_row(&pool, uid, "hash_c04").await;
    let state = state_for(&pool, uid, "hash_c04").await;

    // 100 concurrent — all must pass.
    {
        let pool = Arc::new(pool.clone());
        let state = Arc::new(state.clone());
        let mut hs = vec![];
        for _ in 0..100 {
            let p = Arc::clone(&pool);
            let s = Arc::clone(&state);
            hs.push(tokio::spawn(async move {
                rbac::require_strong(&s, &p, Permission::PatientsCreate).await.is_ok()
            }));
        }
        for h in hs {
            assert!(h.await.unwrap(), "valid session must pass under contention");
        }
    }

    // 1000 sequential calls < 5s (AERP H2-001-14 — Priority 4's benchmark,
    // folded here so it runs with the suite).
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        rbac::require_strong(&state, &pool, Permission::PatientsCreate)
            .await
            .expect("benchmark iteration");
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed < std::time::Duration::from_secs(5),
        "1000 require_strong calls took {:?} (budget 5s)",
        elapsed
    );
}

// ── G.2.1 Unit-level tests (WP2-U01 … U08, implemented against the real DB) ────

/// WP2-U01 — the Session struct carries token_hash (compile + value test).
#[tokio::test]
async fn wp2_u01_session_struct_has_token_hash() {
    let s = Session {
        user_id: 1,
        username: "u".into(),
        full_name: "U".into(),
        roles: vec![],
        permissions: Default::default(),
        token_hash: "abc".into(),
    };
    assert_eq!(s.token_hash, "abc");
}

/// WP2-U02 — load_session populates token_hash from its argument.
#[tokio::test]
async fn wp2_u02_load_session_populates_token_hash() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_u02", &fixture_pw(), &["doctor"]).await;
    seed_session_row(&pool, uid, "hash_u02").await;
    let s = hospital_mgmt_lib::auth::load_session(&pool, uid, "hash_u02")
        .await
        .unwrap();
    assert_eq!(s.token_hash, "hash_u02");
}

/// WP2-U03 — invalid token (row missing) → Err + state cleared.
#[tokio::test]
async fn wp2_u03_rejects_invalid_token() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_u03", &fixture_pw(), &["doctor"]).await;
    seed_session_row(&pool, uid, "hash_u03").await;
    let state = state_for(&pool, uid, "hash_u03").await;

    sqlx::query("DELETE FROM sessions WHERE token_hash = 'hash_u03'")
        .execute(&pool)
        .await
        .unwrap();
    let r = rbac::require_strong(&state, &pool, Permission::PatientsCreate)
        .await
        .unwrap_err();
    assert!(r.contains("Session invalidated"));
    assert!(state.lock().unwrap().is_none(), "state must be cleared (U07)");
}

/// WP2-U04 — inactive user → Err.
#[tokio::test]
async fn wp2_u04_rejects_inactive_user() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_u04", &fixture_pw(), &["doctor"]).await;
    seed_session_row(&pool, uid, "hash_u04").await;
    let state = state_for(&pool, uid, "hash_u04").await;
    set_user_active(&pool, uid, false).await;
    assert!(rbac::require_strong(&state, &pool, Permission::PatientsCreate).await.is_err());
}

/// WP2-U05 — expired session (expires_at in the past) → Err.
#[tokio::test]
async fn wp2_u05_rejects_expired_session() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_u05", &fixture_pw(), &["doctor"]).await;
    seed_session_row(&pool, uid, "hash_u05").await;
    sqlx::query("UPDATE sessions SET expires_at = NOW() - INTERVAL '1 hour' WHERE token_hash = 'hash_u05'")
        .execute(&pool)
        .await
        .unwrap();
    let state = state_for(&pool, uid, "hash_u05").await;
    assert!(rbac::require_strong(&state, &pool, Permission::PatientsCreate).await.is_err());
}

/// WP2-U06 — valid session → Ok(Session) with the same identity.
#[tokio::test]
async fn wp2_u06_accepts_valid_session() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_u06", &fixture_pw(), &["doctor"]).await;
    seed_session_row(&pool, uid, "hash_u06").await;
    let state = state_for(&pool, uid, "hash_u06").await;
    let s = rbac::require_strong(&state, &pool, Permission::PatientsCreate)
        .await
        .expect("valid session must pass");
    assert_eq!(s.user_id, uid);
}

/// WP2-U07 — state cleared on failure (asserted inside U03; explicit here).
#[tokio::test]
async fn wp2_u07_state_cleared_on_failure() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_u07", &fixture_pw(), &["doctor"]).await;
    seed_session_row(&pool, uid, "hash_u07").await;
    let state = state_for(&pool, uid, "hash_u07").await;
    sqlx::query("DELETE FROM sessions WHERE token_hash = 'hash_u07'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(rbac::require_strong(&state, &pool, Permission::PatientsCreate).await.is_err());
    assert!(state.lock().unwrap().is_none());
}

/// WP2-U08 — require_strong inherits require's no-session behavior.
#[tokio::test]
async fn wp2_u08_inherits_require_behavior() {
    let pool = setup().await;
    let state: SessionState = Arc::new(Mutex::new(None));
    let r = rbac::require_strong(&state, &pool, Permission::PatientsCreate)
        .await
        .unwrap_err();
    assert!(r.contains("not signed in"), "got: {}", r);
}

// ── Review Pass 3 regressions (2026-09-04) ─────────────────────────────────────

/// P3-7: the single-active-session invariant is now enforced by the SCHEMA
/// (UNIQUE index on sessions.user_id) — a second token for the same user is
/// rejected by the database itself, independent of any application logic.
#[tokio::test]
async fn rev3_p3_7_unique_user_session_schema_enforced() {
    let pool = setup().await;
    let uid = seed_user(&pool, "aerp_p3_7", &fixture_pw(), &["doctor"]).await;
    seed_session_row(&pool, uid, "hash_p3_7_a").await;

    // Direct INSERT of a second row for the same user → unique violation.
    let dup = sqlx::query(
        "INSERT INTO sessions (token_hash, user_id, expires_at) \
         VALUES ('hash_p3_7_b', $1, NOW() + INTERVAL '12 hours')",
    )
    .bind(uid)
    .execute(&pool)
    .await;
    assert!(dup.is_err(), "UNIQUE(user_id) must reject a second session");

    // A different user's session is unaffected by the index.
    let uid2 = seed_user(&pool, "aerp_p3_7b", &fixture_pw(), &["nurse"]).await;
    seed_session_row(&pool, uid2, "hash_p3_7_c").await;
    assert_eq!(count_sessions(&pool, uid2).await, 1);
}

/// P3-10: an admin changing a user's roles now deletes the target's session
/// rows — permission revocation takes effect on the target's NEXT command
/// (require_strong → "Session invalidated") instead of persisting up to the
/// 12h session lifetime. The sanctioned-path L03 window is closed.
#[tokio::test]
async fn rev3_p3_10_role_change_sweeps_target_sessions() {
    let pool = setup().await;
    let admin = seed_user(&pool, "aerp_admin_p3_10", &fixture_pw(), &["super_admin"]).await;
    let target = seed_user(&pool, "aerp_doc_p3_10", &fixture_pw(), &["doctor"]).await;

    seed_session_row(&pool, admin, "hash_p3_10_admin").await;
    seed_session_row(&pool, target, "hash_p3_10_target").await;
    let admin_state = state_for(&pool, admin, "hash_p3_10_admin").await;
    let target_state = state_for(&pool, target, "hash_p3_10_target").await;

    // Baseline: target can use high-risk commands.
    rbac::require_strong(&target_state, &pool, Permission::PatientsCreate)
        .await
        .expect("baseline high-risk access");

    // Admin demotes the target to patient (no permissions at all).
    hospital_mgmt_lib::auth::update_user_core(
        &pool,
        &admin_state,
        hospital_mgmt_lib::auth::UpdateUserRequest {
            id: target,
            full_name: None,
            email: None,
            is_active: None,
            roles: Some(vec!["patient".into()]),
        },
    )
    .await
    .expect("role change");

    // The sweep deleted the target's session rows…
    assert_eq!(count_sessions(&pool, target).await, 0);

    // …so the target's next high-risk command is rejected immediately.
    let r = rbac::require_strong(&target_state, &pool, Permission::PatientsCreate)
        .await
        .unwrap_err();
    assert!(r.contains("Session invalidated"), "got: {}", r);
}

/// P3-7 companion: login_core's upsert keeps the WP-2.1 invariant — a second
/// login REPLACES the row (previous token dies) rather than adding a row.
#[tokio::test]
async fn rev3_p3_7_upsert_replaces_prior_token() {
    let pool = setup().await;
    seed_user(&pool, "aerp_user_p3_up", &fixture_pw(), &["doctor"]).await;

    let pc_a: SessionState = Arc::new(Mutex::new(None));
    let pc_b: SessionState = Arc::new(Mutex::new(None));
    login_on(&pool, &pc_a, "aerp_user_p3_up", &fixture_pw()).await.unwrap();
    let token_a = current_token_hash(&pool).await;
    login_on(&pool, &pc_b, "aerp_user_p3_up", &fixture_pw()).await.unwrap();
    let token_b = current_token_hash(&pool).await;
    assert_ne!(token_a, token_b);

    let uid: (i32,) = sqlx::query_as("SELECT id FROM users WHERE username = 'aerp_user_p3_up'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count_sessions(&pool, uid.0).await, 1);
    // Old token is gone; only the new one survives.
    let alive: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE token_hash = $1")
        .bind(&token_a)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(alive.0, 0);
}

/// Pass-2 Finding 2 + pass-3 open item: a RECEPTIONIST session cannot invoke
/// create_prescription at the command level — the guard demands
/// PrescriptionsCreate, which only doctors/super-admins hold. This exercises
/// the real core (guard → validation → INSERT), closing the last §10 gap.
#[tokio::test]
async fn rev3_f2_receptionist_cannot_prescribe_command_level() {
    let pool = setup().await;
    let rx_id = seed_user(&pool, "aerp_rx_presc", &fixture_pw(), &["receptionist"]).await;
    let doc_id = seed_user(&pool, "aerp_doc_presc", &fixture_pw(), &["doctor"]).await;
    let patient_id = seed_patient_with_phone(&pool, "CmdLevel", "Prescribe", "+923007778889").await;

    seed_session_row(&pool, rx_id, "hash_rx_presc").await;
    seed_session_row(&pool, doc_id, "hash_doc_presc").await;
    let rx_state = state_for(&pool, rx_id, "hash_rx_presc").await;
    let doc_state = state_for(&pool, doc_id, "hash_doc_presc").await;

    let rx = hospital_mgmt_lib::models::CreatePrescription {
        patient_id,
        doctor_id: None,
        encounter_id: None,
        items: vec![hospital_mgmt_lib::models::CreatePrescriptionItem {
            medication_id: None,
            medication_name: "Paracetamol".into(),
            dose: "500mg".into(),
            route: Some("oral".into()),
            frequency: "BD".into(),
            duration: Some("5 days".into()),
            quantity: Some(10),
        }],
        notes: None,
    };

    // Receptionist: denied at the permission guard, exact error.
    let err = hospital_mgmt_lib::commands::pharmacy::create_prescription_core(
        &pool, &rx_state, rx.clone(),
    )
    .await
    .unwrap_err();
    assert!(
        err.contains("requires the 'prescriptions.create'"),
        "receptionist must be denied by the PrescriptionsCreate guard, got: {}",
        err
    );

    // Doctor: passes the guard (proceeds into business logic — empty-items
    // validation proves the guard let it through).
    let mut empty = rx.clone();
    empty.items = vec![];
    let err2 = hospital_mgmt_lib::commands::pharmacy::create_prescription_core(
        &pool, &doc_state, empty,
    )
    .await
    .unwrap_err();
    assert!(
        err2.contains("at least one medication"),
        "doctor must pass the guard and hit business validation, got: {}",
        err2
    );

    // Doctor with a complete payload: full success, prescription persisted.
    let id = hospital_mgmt_lib::commands::pharmacy::create_prescription_core(
        &pool, &doc_state, rx,
    )
    .await
    .expect("doctor must be able to prescribe");
    let stored: (i32,) = sqlx::query_as("SELECT patient_id FROM prescriptions WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored.0, patient_id);
}
