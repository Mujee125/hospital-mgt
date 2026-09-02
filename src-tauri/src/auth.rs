//! Authentication & session management (ISO/IEC 27001 A.9).
//!
//! Threat model & controls:
//! - **Password storage**: Argon2id (memory-hard, OWASP-recommended) via the
//!   `argon2` crate. Only the PHC-string hash is persisted — never plaintext.
//! - **Brute force**: after 5 consecutive failures the account is locked for
//!   15 minutes (`locked_until`). Counters reset on success.
//! - **Session tokens**: 32 random bytes, base64url-encoded. Only the SHA-256
//!   hash of the token is stored in `sessions`; the raw token lives in
//!   desktop memory (Tauri app state) and is never written to disk. Login
//!   deletes prior sessions for the user (single active session).
//! - **Audit**: every login success/failure, logout, and password change is
//!   written to `audit_logs`.
//! - **Bootstrap admin**: a single `admin` / `ChangeMe123!` account is seeded
//!   with `must_change_password = true` so the first thing an installer does
//!   is rotate it. There is no hardcoded backdoor.

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Algorithm, Argon2, Params,
};
use base64::Engine;
use chrono::{Duration, Utc};
use rand::rngs::OsRng as RandOsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::collections::HashSet;
use std::sync::Mutex;
use tauri::Emitter;

use crate::rbac::{self, Permission, Session, ROLE_SUPER_ADMIN};

// Account-lockout policy (tunable, intentionally conservative).
const MAX_FAILED_ATTEMPTS: i32 = 5;
const LOCKOUT_MINUTES: i64 = 15;
const SESSION_HOURS: i64 = 12;

// ── Password hashing ──────────────────────────────────────────────────────────

/// Argon2id with m=19456 KiB, t=2, p=1 — OWASP-recommended minimum (2023).
fn argon2() -> Argon2<'static> {
    Argon2::new(
        Algorithm::Argon2id,
        argon2::Version::V0x13,
        Params::new(19_456, 2, 1, None).expect("valid argon2 params"),
    )
}

pub fn hash_password(plain: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    argon2()
        .hash_password(plain.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| format!("Password hash failed: {}", e))
}

pub fn verify_password(plain: &str, phc: &str) -> bool {
    match PasswordHash::new(phc) {
        Ok(parsed) => argon2().verify_password(plain.as_bytes(), &parsed).is_ok(),
        Err(_) => false,
    }
}

// ── Session token generation ──────────────────────────────────────────────────

fn random_token() -> String {
    let mut bytes = [0u8; 32];
    RandOsRng.fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

// ── Bootstrap credential generation (CR-2) ───────────────────────────────────

/// Generate a 24-character random password from the CSPRNG. Uses an
/// unambiguous alphabet (no 0/O/1/l/I) so it can be transcribed from the
/// bootstrap-credentials file by hand if needed.
fn generate_bootstrap_password() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";
    let mut bytes = [0u8; 24];
    RandOsRng.fill_bytes(&mut bytes);
    bytes
        .iter()
        .map(|b| ALPHABET[(*b as usize) % ALPHABET.len()] as char)
        .collect()
}

/// Write the bootstrap admin credentials to a protected file so the
/// installer / first-run operator can read the one-time password.
///
/// Location: same directory as `config.json` (`C:\ProgramData\HMS` on
/// Windows). On Windows the file is ACL-hardened to SYSTEM + Administrators
/// only. The operator reads it once, logs in, and is forced to rotate.
fn write_bootstrap_credentials(username: &str, password: &str) -> Result<(), String> {
    let program_data = std::env::var_os("ProgramData")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let hms_dir = program_data.join("HMS");
    std::fs::create_dir_all(&hms_dir).map_err(|e| format!("create HMS dir: {}", e))?;

    let cred_path = hms_dir.join("bootstrap-credentials.txt");
    let content = format!(
        "VitalFlow HMS — Bootstrap Administrator Credentials\n\
         ====================================================\n\
         \n\
         Username: {}\n\
         Password: {}\n\
         \n\
         This password was randomly generated at install time.\n\
         You will be required to change it on first login.\n\
         \n\
         IMPORTANT: After logging in and changing your password, delete this file.\n\
         This file is ACL-restricted to SYSTEM and Administrators only.\n",
        username, password
    );

    std::fs::write(&cred_path, &content).map_err(|e| format!("write bootstrap creds: {}", e))?;

    // Windows: ACL-harden the file.
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("icacls")
            .arg(cred_path.as_os_str())
            .args(["/inheritance:r"])
            .args(["/grant:r", "SYSTEM:F"])
            .args(["/grant:r", "Administrators:F"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }

    Ok(())
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

// ── DTOs ──────────────────────────────────────────────────────────────────────

// Used as a Tauri command return/parameter type via #[tauri::command] —
// clippy doesn't see the macro-expanded usage, so flag as allowed dead code.
#[allow(dead_code)]
#[derive(Debug, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub full_name: String,
    pub email: Option<String>,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub is_active: bool,
    pub must_change_password: bool,
    pub failed_login_count: i32,
    pub locked_until: Option<chrono::DateTime<Utc>>,
    pub last_login_at: Option<chrono::DateTime<Utc>>,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, serde::Serialize)]
pub struct LoginResponse {
    pub user: UserProfile,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    pub must_change_password: bool,
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct UserProfile {
    pub id: i32,
    pub username: String,
    pub full_name: String,
    pub email: Option<String>,
    pub is_active: bool,
    pub must_change_password: bool,
    pub last_login_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, serde::Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub full_name: String,
    #[serde(default)]
    pub email: Option<String>,
    pub password: String,
    pub roles: Vec<String>,
    #[serde(default)]
    pub must_change_password: Option<bool>,
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateUserRequest {
    pub id: i32,
    #[serde(default)]
    pub full_name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub is_active: Option<bool>,
    #[serde(default)]
    pub roles: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

// ── Seeding (called once from db::run_migrations) ─────────────────────────────

pub async fn seed_defaults(pool: &PgPool) -> Result<(), String> {
    // 1. Permissions
    for p in Permission::all() {
        sqlx::query(
            "INSERT INTO permissions (key, description) VALUES ($1, $2)
             ON CONFLICT (key) DO NOTHING",
        )
        .bind(p.as_str())
        .bind(format!("System permission: {}", p.as_str()))
        .execute(pool)
        .await
        .map_err(|e| format!("seed permission {}: {}", p.as_str(), e))?;
    }

    // 2. Roles + role_permissions
    for (name, desc) in rbac::seed_roles() {
        sqlx::query(
            "INSERT INTO roles (name, description) VALUES ($1, $2)
             ON CONFLICT (name) DO UPDATE SET description = EXCLUDED.description",
        )
        .bind(name)
        .bind(desc)
        .execute(pool)
        .await
        .map_err(|e| format!("seed role {}: {}", name, e))?;

        let role_id: (i32,) = sqlx::query_as("SELECT id FROM roles WHERE name = $1")
            .bind(name)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("lookup role {}: {}", name, e))?;

        // Re-sync permissions for this role (idempotent; allows policy updates
        // to propagate on the next app start).
        sqlx::query("DELETE FROM role_permissions WHERE role_id = $1")
            .bind(role_id.0)
            .execute(pool)
            .await
            .map_err(|e| format!("clear role_permissions: {}", e))?;

        for perm in rbac::permissions_for_role(name) {
            sqlx::query(
                "INSERT INTO role_permissions (role_id, permission_id)
                 SELECT $1, id FROM permissions WHERE key = $2
                 ON CONFLICT DO NOTHING",
            )
            .bind(role_id.0)
            .bind(perm.as_str())
            .execute(pool)
            .await
            .map_err(|e| format!("seed role_permission {}: {}", perm.as_str(), e))?;
        }
    }

    // 3. Bootstrap super-admin (only if no users exist at all).
    let user_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await
        .map_err(|e| format!("count users: {}", e))?;

    if user_count.0 == 0 {
        // ── Random bootstrap password (CR-2) ───────────────────────────────
        //
        // The previous implementation seeded `admin / ChangeMe123!` — a
        // publicly-known password. Even with `must_change_password = true`,
        // an attacker who reaches the login screen before the admin rotates
        // it gains super-admin access.
        //
        // We now generate a 24-character random password from a CSPRNG and
        // write it to a protected bootstrap-credentials file. The file lives
        // in the same directory as config.json (C:\ProgramData\HMS on
        // Windows) and is ACL-hardened to SYSTEM + Administrators only. The
        // installer / first-run operator reads it once, logs in, and is
        // forced to rotate.
        let bootstrap_password = generate_bootstrap_password();
        let hash = hash_password(&bootstrap_password)?;
        let admin_id: (i32,) = sqlx::query_as(
            "INSERT INTO users (username, full_name, email, password_hash, must_change_password)
             VALUES ($1, $2, $3, $4, TRUE) RETURNING id",
        )
        .bind("admin")
        .bind("System Administrator")
        .bind(Option::<String>::None)
        .bind(&hash)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("seed admin user: {}", e))?;

        let role_id: (i32,) = sqlx::query_as("SELECT id FROM roles WHERE name = $1")
            .bind(ROLE_SUPER_ADMIN)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("lookup super_admin role: {}", e))?;

        sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
            .bind(admin_id.0)
            .bind(role_id.0)
            .execute(pool)
            .await
            .map_err(|e| format!("seed admin role: {}", e))?;

        // Persist the bootstrap credentials so the operator can read them.
        // Best-effort — if this fails the admin can still reset via DB.
        if let Err(e) = write_bootstrap_credentials("admin", &bootstrap_password) {
            eprintln!("[HMS AUTH] Warning: could not write bootstrap credentials file: {}. \
                       The admin password was set but NOT saved to disk — use a DB-side \
                       password reset if needed.", e);
        }

        // Audit the bootstrap (no session yet, so use record() directly).
        let _ = crate::audit::record(
            pool,
            Some(admin_id.0),
            Some("admin"),
            "system_bootstrap",
            "users",
            Some(&admin_id.0.to_string()),
            Some(serde_json::json!({"username": "admin", "role": ROLE_SUPER_ADMIN})),
        ).await;
    }

    Ok(())
}

// ── Session loading helper ────────────────────────────────────────────────────

async fn load_session(pool: &PgPool, user_id: i32, token_hash: &str) -> Result<Session, String> {
    let roles: Vec<(String,)> =
        sqlx::query_as("SELECT r.name FROM user_roles ur JOIN roles r ON r.id = ur.role_id WHERE ur.user_id = $1")
            .bind(user_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("load roles: {}", e))?;

    let perms: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT p.key FROM user_roles ur
         JOIN role_permissions rp ON rp.role_id = ur.role_id
         JOIN permissions p ON p.id = rp.permission_id
         WHERE ur.user_id = $1",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("load permissions: {}", e))?;

    let user: (String, String) =
        sqlx::query_as("SELECT username, full_name FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("load user: {}", e))?;

    Ok(Session {
        user_id,
        username: user.0,
        full_name: user.1,
        roles: roles.into_iter().map(|r| r.0).collect(),
        permissions: perms.into_iter().map(|p| p.0).collect::<HashSet<_>>(),
        token_hash: token_hash.to_string(),
    })
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn login(
    pool: tauri::State<'_, PgPool>,
    session_state: tauri::State<'_, std::sync::Arc<Mutex<Option<Session>>>>,
    app_handle: tauri::AppHandle,
    request: LoginRequest,
) -> Result<LoginResponse, String> {
    // Fetch user row. Use a left-join-free scalar fetch to avoid leaking which
    // usernames exist via timing — we still run a dummy verify below on failure.
    // Clippy: the row tuple type is intentionally inline (one-shot, single use);
    // a `type` alias would obscure which columns are bound below.
    #[allow(clippy::type_complexity)]
    let row: Option<(
        i32, String, String, String, bool, bool, i32, Option<chrono::DateTime<Utc>>,
    )> = sqlx::query_as(
        "SELECT id, username, full_name, password_hash, is_active, must_change_password,
                failed_login_count, locked_until
         FROM users WHERE username = $1",
    )
    .bind(&request.username)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    let now = Utc::now();

    let (user_id, username, full_name, hash, is_active, must_change, failed, locked_until) =
        match row {
            Some(r) => r,
            None => {
                // Unknown user — run a dummy verify to flatten timing, then audit.
                let _ = verify_password(&request.password, "$argon2id$v=19$m=19456,t=2,p=1$AAAAAAAAAAAAAAAAAAAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
                // SEC-05: do NOT record the attempted username in the audit
                // row. Logging unknown usernames (even in the admin-only
                // audit log) creates a user-enumeration oracle: an attacker
                // who later compromises an admin account can see which
                // usernames were probed. The audit row keeps the reason
                // ("unknown_user") + the IP (in the `ip` column, captured
                // separately by audit::record's IP column) so brute-force
                // detection still works.
                crate::audit::record(pool.inner(), None, None, "login_failed", "auth", None,
                    Some(serde_json::json!({"reason": "unknown_user"}))).await.ok();
                // SEC-05: do NOT include the attempted username in the WARN
                // log line either — the on-disk log is now RBAC-gated
                // (admin-only) AND redacted at read time for `key=value`
                // patterns, but the previous "unknown user '<name>'"
                // free-form format wouldn't be caught by the key=value
                // redactor. Emit a generic message instead.
                crate::log(&app_handle, "WARN ", "Login failed: unknown user (username not logged)");
                return Err("Invalid username or password.".to_string());
            }
        };

    if !is_active {
        crate::audit::record(pool.inner(), Some(user_id), Some(&username), "login_failed", "auth", None,
            Some(serde_json::json!({"reason": "inactive"}))).await.ok();
        return Err("This account has been deactivated. Contact your administrator.".to_string());
    }

    if let Some(until) = locked_until {
        if until > now {
            crate::audit::record(pool.inner(), Some(user_id), Some(&username), "login_failed", "auth", None,
                Some(serde_json::json!({"reason": "locked", "locked_until": until.to_rfc3339()}))).await.ok();
            return Err(format!("Account locked. Try again after {}.", until.format("%H:%M")));
        }
    }

    if !verify_password(&request.password, &hash) {
        let new_failed = failed + 1;
        let lock_until = if new_failed >= MAX_FAILED_ATTEMPTS {
            Some(now + Duration::minutes(LOCKOUT_MINUTES))
        } else {
            None
        };
        sqlx::query("UPDATE users SET failed_login_count = $1, locked_until = $2 WHERE id = $3")
            .bind(new_failed)
            .bind(lock_until)
            .bind(user_id)
            .execute(pool.inner())
            .await
            .map_err(|e| crate::db::sanitize_db_error(&e))?;

        crate::audit::record(pool.inner(), Some(user_id), Some(&username), "login_failed", "auth", None,
            Some(serde_json::json!({"reason": "bad_password", "attempts": new_failed}))).await.ok();

        if new_failed >= MAX_FAILED_ATTEMPTS {
            return Err(format!("Too many failed attempts. Account locked for {} minutes.", LOCKOUT_MINUTES));
        }
        let remaining = MAX_FAILED_ATTEMPTS - new_failed;
        return Err(format!("Invalid username or password. {} attempt(s) remaining.", remaining));
    }

    // ── Success ──
    sqlx::query("UPDATE users SET failed_login_count = 0, locked_until = NULL, last_login_at = NOW() WHERE id = $1")
        .bind(user_id)
        .execute(pool.inner())
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;

    // Single active session: invalidate previous tokens.
    sqlx::query("DELETE FROM sessions WHERE user_id = $1")
        .bind(user_id)
        .execute(pool.inner())
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;

    // WP-2.2 Layer 2: emit session_invalidated event so other PCs running
    // the same Tauri process can clear their in-memory session. (Cross-PC
    // propagation is handled by Layer 1: `me` polling + Layer 3:
    // `require_strong` DB check on high-risk commands.)
    let _ = app_handle.emit("session_invalidated", serde_json::json!({"user_id": user_id}));

    let token = random_token();
    let token_hash = hash_token(&token);
    let expires = now + Duration::hours(SESSION_HOURS);
    sqlx::query("INSERT INTO sessions (token_hash, user_id, expires_at) VALUES ($1, $2, $3)")
        .bind(&token_hash)
        .bind(user_id)
        .bind(expires)
        .execute(pool.inner())
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;

    let session = load_session(pool.inner(), user_id, &token_hash).await?;
    crate::audit::record(pool.inner(), Some(user_id), Some(&username), "login_success", "auth", None, None).await.ok();

    // REL-02: recover from mutex poisoning instead of panicking.
    *session_state.lock().unwrap_or_else(|e| e.into_inner()) = Some(session.clone());

    Ok(LoginResponse {
        user: UserProfile {
            id: user_id,
            username,
            full_name,
            email: None,
            is_active: true,
            must_change_password: must_change,
            last_login_at: Some(now),
        },
        roles: session.roles.clone(),
        permissions: session.permissions.iter().cloned().collect(),
        must_change_password: must_change,
    })
}

#[tauri::command]
pub async fn logout(
    pool: tauri::State<'_, PgPool>,
    session_state: tauri::State<'_, std::sync::Arc<Mutex<Option<Session>>>>,
) -> Result<(), String> {
    let session = {
        // REL-02: recover from mutex poisoning instead of panicking.
        let mut g = session_state.lock().unwrap_or_else(|e| e.into_inner());
        g.take()
    };
    if let Some(s) = session {
        sqlx::query("DELETE FROM sessions WHERE user_id = $1")
            .bind(s.user_id)
            .execute(pool.inner())
            .await
            .ok();
        crate::audit::record(pool.inner(), Some(s.user_id), Some(&s.username), "logout", "auth", None, None)
            .await.ok();
    }
    Ok(())
}

/// Returns the current session profile, or an error if not signed in.
/// The frontend polls this on boot to decide Login vs. main app.
#[tauri::command]
pub async fn me(
    pool: tauri::State<'_, PgPool>,
    session_state: tauri::State<'_, std::sync::Arc<Mutex<Option<Session>>>>,
) -> Result<LoginResponse, String> {
    let session = rbac::require_session(&session_state)?;

    // Verify the session is still valid server-side (expiry/active).
    let valid: Option<(chrono::DateTime<Utc>,)> = sqlx::query_as(
        "SELECT s.expires_at FROM sessions s
         JOIN users u ON u.id = s.user_id
         WHERE s.token_hash = $1 AND u.is_active = TRUE",
    )
    .bind(&session.token_hash)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    let valid = matches!(valid, Some((exp,)) if exp > Utc::now());
    if !valid {
        // REL-02: recover from mutex poisoning instead of panicking.
        *session_state.lock().unwrap_or_else(|e| e.into_inner()) = None;
        return Err("Session expired. Please sign in again.".to_string());
    }

    let profile: (String, Option<String>, bool, Option<chrono::DateTime<Utc>>) = sqlx::query_as(
        "SELECT full_name, email, must_change_password, last_login_at FROM users WHERE id = $1",
    )
    .bind(session.user_id)
    .fetch_one(pool.inner())
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    Ok(LoginResponse {
        user: UserProfile {
            id: session.user_id,
            username: session.username.clone(),
            full_name: profile.0,
            email: profile.1,
            is_active: true,
            must_change_password: profile.2,
            last_login_at: profile.3,
        },
        roles: session.roles.clone(),
        permissions: session.permissions.iter().cloned().collect(),
        must_change_password: profile.2,
    })
}

#[tauri::command]
pub async fn change_password(
    pool: tauri::State<'_, PgPool>,
    session_state: tauri::State<'_, std::sync::Arc<Mutex<Option<Session>>>>,
    request: ChangePasswordRequest,
) -> Result<(), String> {
    if request.new_password.len() < 8 {
        return Err("Password must be at least 8 characters.".to_string());
    }
    let session = rbac::require_session(&session_state)?;

    let hash: (String,) = sqlx::query_as("SELECT password_hash FROM users WHERE id = $1")
        .bind(session.user_id)
        .fetch_one(pool.inner())
        .await
        .map_err(|e| format!("Load password: {}", e))?;

    if !verify_password(&request.current_password, &hash.0) {
        return Err("Current password is incorrect.".to_string());
    }

    let new_hash = hash_password(&request.new_password)?;
    sqlx::query("UPDATE users SET password_hash = $1, must_change_password = FALSE, updated_at = NOW() WHERE id = $2")
        .bind(&new_hash)
        .bind(session.user_id)
        .execute(pool.inner())
        .await
        .map_err(|e| format!("Update password: {}", e))?;

    crate::audit::record(pool.inner(), Some(session.user_id), Some(&session.username),
        "password_change", "auth", None, None).await.ok();
    Ok(())
}

// ── User management commands ──────────────────────────────────────────────────

#[tauri::command]
pub async fn list_users(
    pool: tauri::State<'_, PgPool>,
    session_state: tauri::State<'_, std::sync::Arc<Mutex<Option<Session>>>>,
) -> Result<Vec<UserProfile>, String> {
    let _ = rbac::require(&session_state, Permission::UsersView)?;
    sqlx::query_as(
        "SELECT id, username, full_name, email, is_active, must_change_password, last_login_at
         FROM users ORDER BY username",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| format!("List users: {}", e))
}

#[tauri::command]
pub async fn create_user(
    pool: tauri::State<'_, PgPool>,
    session_state: tauri::State<'_, std::sync::Arc<Mutex<Option<Session>>>>,
    request: CreateUserRequest,
) -> Result<i32, String> {
    let session = rbac::require_strong(&session_state, pool.inner(), Permission::UsersManage).await?;

    if request.username.trim().is_empty() || request.password.len() < 8 {
        return Err("Username required and password must be at least 8 characters.".to_string());
    }

    let hash = hash_password(&request.password)?;
    let must_change = request.must_change_password.unwrap_or(true);
    let id: (i32,) = sqlx::query_as(
        "INSERT INTO users (username, full_name, email, password_hash, must_change_password)
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(&request.username)
    .bind(&request.full_name)
    .bind(&request.email)
    .bind(&hash)
    .bind(must_change)
    .fetch_one(pool.inner())
    .await
    .map_err(|e| format!("Create user: {}", e))?;

    sync_user_roles(pool.inner(), id.0, &request.roles).await?;

    crate::audit::record(pool.inner(), Some(session.user_id), Some(&session.username),
        "user_create", "users", Some(&id.0.to_string()),
        Some(serde_json::json!({"username": request.username, "roles": request.roles}))).await.ok();
    Ok(id.0)
}

#[tauri::command]
pub async fn update_user(
    app_handle: tauri::AppHandle,
    pool: tauri::State<'_, PgPool>,
    session_state: tauri::State<'_, std::sync::Arc<Mutex<Option<Session>>>>,
    request: UpdateUserRequest,
) -> Result<(), String> {
    let session = rbac::require_strong(&session_state, pool.inner(), Permission::UsersManage).await?;

    if let Some(name) = &request.full_name {
        sqlx::query("UPDATE users SET full_name = $1, updated_at = NOW() WHERE id = $2")
            .bind(name).bind(request.id)
            .execute(pool.inner()).await
            .map_err(|e| format!("Update user: {}", e))?;
    }
    if let Some(email) = &request.email {
        sqlx::query("UPDATE users SET email = $1, updated_at = NOW() WHERE id = $2")
            .bind(email).bind(request.id)
            .execute(pool.inner()).await
            .map_err(|e| format!("Update user: {}", e))?;
    }
    if let Some(active) = request.is_active {
        sqlx::query("UPDATE users SET is_active = $1, updated_at = NOW() WHERE id = $2")
            .bind(active).bind(request.id)
            .execute(pool.inner()).await
            .map_err(|e| format!("Update user: {}", e))?;
    }
    if let Some(roles) = &request.roles {
        sync_user_roles(pool.inner(), request.id, roles).await?;
    }

    crate::audit::record(pool.inner(), Some(session.user_id), Some(&session.username),
        "user_update", "users", Some(&request.id.to_string()), None).await.ok();

    // WP-2.2 Layer 2: if the target user was deactivated or had roles changed,
    // emit session_invalidated so their in-memory session on this PC is cleared.
    // (Cross-PC propagation via Layer 1: `me` polling + Layer 3: require_strong.)
    if request.is_active == Some(false) || request.roles.is_some() {
        let _ = app_handle.emit("session_invalidated", serde_json::json!({"user_id": request.id}));
    }
    Ok(())
}

#[tauri::command]
pub async fn delete_user(
    pool: tauri::State<'_, PgPool>,
    session_state: tauri::State<'_, std::sync::Arc<Mutex<Option<Session>>>>,
    id: i32,
) -> Result<(), String> {
    let session = rbac::require_strong(&session_state, pool.inner(), Permission::UsersManage).await?;
    if id == session.user_id {
        return Err("You cannot delete your own account.".to_string());
    }
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| format!("Delete user: {}", e))?;
    crate::audit::record(pool.inner(), Some(session.user_id), Some(&session.username),
        "user_delete", "users", Some(&id.to_string()), None).await.ok();
    Ok(())
}

#[tauri::command]
pub async fn reset_user_password(
    app_handle: tauri::AppHandle,
    pool: tauri::State<'_, PgPool>,
    session_state: tauri::State<'_, std::sync::Arc<Mutex<Option<Session>>>>,
    id: i32,
    new_password: String,
) -> Result<(), String> {
    let session = rbac::require_strong(&session_state, pool.inner(), Permission::UsersManage).await?;
    if new_password.len() < 8 {
        return Err("Password must be at least 8 characters.".to_string());
    }
    let hash = hash_password(&new_password)?;
    sqlx::query("UPDATE users SET password_hash = $1, must_change_password = TRUE, updated_at = NOW() WHERE id = $2")
        .bind(&hash).bind(id)
        .execute(pool.inner()).await
        .map_err(|e| format!("Reset password: {}", e))?;
    // Invalidate sessions for the target user.
    sqlx::query("DELETE FROM sessions WHERE user_id = $1").bind(id)
        .execute(pool.inner()).await.ok();
    // WP-2.2 Layer 2: emit session_invalidated for the target user.
    let _ = app_handle.emit("session_invalidated", serde_json::json!({"user_id": id}));
    crate::audit::record(pool.inner(), Some(session.user_id), Some(&session.username),
        "password_reset", "users", Some(&id.to_string()), None).await.ok();
    Ok(())
}

#[tauri::command]
pub async fn list_roles(
    pool: tauri::State<'_, PgPool>,
    session_state: tauri::State<'_, std::sync::Arc<Mutex<Option<Session>>>>,
) -> Result<Vec<(i32, String, String)>, String> {
    let _ = rbac::require(&session_state, Permission::UsersView)?;
    sqlx::query_as("SELECT id, name, description FROM roles ORDER BY name")
        .fetch_all(pool.inner())
        .await
        .map_err(|e| format!("List roles: {}", e))
}

#[tauri::command]
pub async fn list_user_roles(
    pool: tauri::State<'_, PgPool>,
    session_state: tauri::State<'_, std::sync::Arc<Mutex<Option<Session>>>>,
    user_id: i32,
) -> Result<Vec<String>, String> {
    let _ = rbac::require(&session_state, Permission::UsersView)?;
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT r.name FROM user_roles ur JOIN roles r ON r.id = ur.role_id WHERE ur.user_id = $1",
    )
    .bind(user_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| format!("List user roles: {}", e))?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

async fn sync_user_roles(pool: &PgPool, user_id: i32, roles: &[String]) -> Result<(), String> {
    sqlx::query("DELETE FROM user_roles WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Clear user roles: {}", e))?;
    for role_name in roles {
        sqlx::query(
            "INSERT INTO user_roles (user_id, role_id)
             SELECT $1, id FROM roles WHERE name = $2
             ON CONFLICT DO NOTHING",
        )
        .bind(user_id)
        .bind(role_name)
        .execute(pool)
        .await
        .map_err(|e| format!("Insert user role {}: {}", role_name, e))?;
    }
    Ok(())
}
