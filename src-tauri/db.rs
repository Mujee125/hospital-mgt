use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::path::Path;
use std::time::Duration;

// NOTE: Embedded/auto-downloaded PostgreSQL (pg_embed) has been removed.
// The server build now provisions a *real* PostgreSQL installation as a
// Windows Service via `pg_provision.rs`, using binaries bundled in the
// installer. See that module for details.

/// Builds a Postgres connection URL.
///
/// Three SSL modes are possible:
///
/// 1. `sslrootcert_path = Some(path)` → `sslmode=verify-ca` against the
///    pinned cert. Used by client builds for all connections to the server
///    after pairing. This is the real security enforcement.
///
/// 2. `sslrootcert_path = None, require_ssl = true` → `sslmode=require`.
///    Used by the server build for its loopback connections AFTER SSL has
///    been enabled in PostgreSQL. Encrypts the channel; no cert validation
///    needed since loopback never crosses the network.
///
/// 3. `sslrootcert_path = None, require_ssl = false` → `sslmode=disable`.
///    Used ONLY by the one-time SSL-enablement provisioning step: the app
///    needs to connect BEFORE PostgreSQL has SSL configured so it can run
///    the configuration that enables it. After that single bootstrap
///    connection, this mode is never used again.
fn build_url(
    user: &str,
    password: &str,
    host: &str,
    port: u16,
    db_name: &str,
    sslrootcert_path: Option<&Path>,
    require_ssl: bool,
) -> String {
    let base = format!("postgresql://{}:{}@{}:{}/{}", user, password, host, port, db_name);
    match sslrootcert_path {
        Some(path) => {
            let encoded = path
                .to_string_lossy()
                .chars()
                .map(|c| match c {
                    ':' => "%3A".to_string(),
                    '\\' => "%5C".to_string(),
                    ' ' => "%20".to_string(),
                    _ => c.to_string(),
                })
                .collect::<String>();
            format!("{}?sslmode=verify-ca&sslrootcert={}", base, encoded)
        }
        None if require_ssl => format!("{}?sslmode=require", base),
        None => format!("{}?sslmode=disable", base),
    }
}

// ── Connection helpers ───────────────────────────────────────────────────────

pub async fn connect_root(
    host: &str,
    port: u16,
    user: &str,
    password: &str,
    sslrootcert_path: Option<&Path>,
) -> Result<PgPool, String> {
    // SSL is definitely already enabled when this is called from the normal
    // startup path (marker file exists) — require it so the connection is
    // encrypted. The loopback path doesn't need cert validation.
    connect_root_internal(host, port, user, password, sslrootcert_path, true).await
}

/// Used ONLY during the first-launch SSL-enablement bootstrap: connects
/// without TLS so we can reach PostgreSQL before its SSL is configured,
/// in order to run the provisioning step that enables SSL.
/// Never used after `ensure_postgres_ssl_enabled` has run.
pub async fn connect_root_no_ssl(
    host: &str,
    port: u16,
    user: &str,
    password: &str,
) -> Result<PgPool, String> {
    connect_root_internal(host, port, user, password, None, false).await
}

async fn connect_root_internal(
    host: &str,
    port: u16,
    user: &str,
    password: &str,
    sslrootcert_path: Option<&Path>,
    require_ssl: bool,
) -> Result<PgPool, String> {
    let url = build_url(user, password, host, port, "postgres", sslrootcert_path, require_ssl);
    PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_secs(15))
        .connect(&url)
        .await
        .map_err(|e| format!("Root connect failed: {}", e))
}

pub async fn connect_app(
    host: &str,
    port: u16,
    user: &str,
    password: &str,
    db_name: &str,
    sslrootcert_path: Option<&Path>,
) -> Result<PgPool, String> {
    let url = build_url(user, password, host, port, db_name, sslrootcert_path, true);
    PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(15))
        .connect(&url)
        .await
        .map_err(|e| format!("App connect failed: {}", e))
}

pub async fn ensure_database(
    root_pool: &PgPool,
    db_name: &str,
) -> Result<(), String> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)",
    )
    .bind(db_name)
    .fetch_one(root_pool)
    .await
    .map_err(|e| format!("DB check failed: {}", e))?;

    if !exists {
        sqlx::query(&format!("CREATE DATABASE \"{}\"", db_name))
            .execute(root_pool)
            .await
            .map_err(|e| format!("CREATE DATABASE failed: {}", e))?;
    }
    Ok(())
}

// ── Migrations ───────────────────────────────────────────────────────────────

pub async fn run_migrations(pool: &PgPool) -> Result<(), String> {
    // pgcrypto for gen_random_uuid()
    sqlx::query("CREATE EXTENSION IF NOT EXISTS pgcrypto")
        .execute(pool)
        .await
        .ok();

    // patients
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS patients (
            id            SERIAL PRIMARY KEY,
            first_name    VARCHAR(100) NOT NULL,
            last_name     VARCHAR(100) NOT NULL,
            email         VARCHAR(255),
            phone         VARCHAR(30)  NOT NULL,
            date_of_birth DATE         NOT NULL,
            gender        VARCHAR(10)  NOT NULL,
            address       TEXT,
            created_at    TIMESTAMPTZ  NOT NULL DEFAULT NOW()
        )
    "#).execute(pool).await.map_err(|e| format!("patients: {}", e))?;

    // doctors
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS doctors (
            id             SERIAL PRIMARY KEY,
            first_name     VARCHAR(100) NOT NULL,
            last_name      VARCHAR(100) NOT NULL,
            email          VARCHAR(255),
            phone          VARCHAR(30)  NOT NULL,
            specialization VARCHAR(100) NOT NULL,
            qualification  VARCHAR(200) NOT NULL,
            available_from TIME         NOT NULL DEFAULT '09:00',
            available_to   TIME         NOT NULL DEFAULT '17:00',
            is_active      BOOLEAN      NOT NULL DEFAULT TRUE,
            created_at     TIMESTAMPTZ  NOT NULL DEFAULT NOW()
        )
    "#).execute(pool).await.map_err(|e| format!("doctors: {}", e))?;

    // appointments
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS appointments (
            id               SERIAL PRIMARY KEY,
            patient_id       INT         NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
            doctor_id        INT         NOT NULL REFERENCES doctors(id)  ON DELETE CASCADE,
            appointment_date DATE        NOT NULL,
            appointment_time TIME        NOT NULL,
            duration_minutes INT         NOT NULL DEFAULT 30,
            status           VARCHAR(20) NOT NULL DEFAULT 'scheduled',
            reason           TEXT,
            notes            TEXT,
            created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
    "#).execute(pool).await.map_err(|e| format!("appointments: {}", e))?;

    // messages (staff instant chat)
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS messages (
            id         UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
            sender     VARCHAR(100) NOT NULL,
            content    TEXT         NOT NULL,
            room       VARCHAR(50)  NOT NULL DEFAULT 'general',
            created_at TIMESTAMPTZ  NOT NULL DEFAULT NOW()
        )
    "#).execute(pool).await.map_err(|e| format!("messages: {}", e))?;

    // whatsapp_notifications log (for tracking + avoiding duplicates)
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS whatsapp_notifications (
            id              SERIAL      PRIMARY KEY,
            appointment_id  INT         REFERENCES appointments(id) ON DELETE SET NULL,
            notification_type VARCHAR(50) NOT NULL,
            recipient       VARCHAR(100) NOT NULL,
            message         TEXT         NOT NULL,
            sent_at         TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
            success         BOOLEAN      NOT NULL DEFAULT FALSE
        )
    "#).execute(pool).await.map_err(|e| format!("whatsapp_notifications: {}", e))?;

    Ok(())
}

// ── Full initialization ──────────────────────────────────────────────────────

/// Initialize DB: ensure DB exists, connect, run migrations.
/// `ssl_is_enabled` must be true when PostgreSQL already has SSL on
/// (normal startup after first launch). Pass false only for the initial
/// bootstrap connect that happens *before* SSL is enabled.
pub async fn initialize(
    host: &str,
    port: u16,
    user: &str,
    password: &str,
    db_name: &str,
    sslrootcert_path: Option<&Path>,
) -> Result<PgPool, String> {
    let root = connect_root(host, port, user, password, sslrootcert_path).await?;
    ensure_database(&root, db_name).await?;
    root.close().await;

    let pool = connect_app(host, port, user, password, db_name, sslrootcert_path).await?;
    run_migrations(&pool).await?;
    Ok(pool)
}
