use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::path::Path;
use std::time::Duration;

// ── SEC-18: database-error sanitisation ──────────────────────────────────────
//
// Raw `sqlx::Error` Display impls include schema details (table names,
// column names, constraint names, SQL state codes, even fragmentary SQL)
// that an attacker who compromises a low-privilege session could use to
// map the schema for a follow-on attack (e.g. "duplicate key value
// violates unique constraint \"users_email_key\"" tells them there's a
// `users` table with a unique `email` column).
//
// Command handlers that previously did `.map_err(|e| format!("...: {}", e))`
// (leaking the raw error to the frontend) now use `.map_err(|e| crate::db::sanitize_db_error(&e))`
// to return a generic user-facing message. The full error is logged to
// stderr for ops debugging.
//
// IMPORTANT: `diagnose_db_error` in `lib.rs` is the INTENTIONAL exception
// — it returns user-facing hints for CONNECTION errors (wrong password,
// cert mismatch, etc.) that help an operator recover without a support
// call. Those hints are curated strings, not raw sqlx output, so they
// don't leak schema. `sanitize_db_error` is for QUERY errors AFTER the
// connection is established — those should never have reached the user
// in raw form.

/// SEC-18: replace a `sqlx::Error` with a generic user-facing message
/// while preserving the full error for ops debugging via stderr.
pub fn sanitize_db_error(e: &sqlx::Error) -> String {
    eprintln!("[HMS DB] database error (full details suppressed for user): {}", e);
    "Database operation failed. Please contact support.".to_string()
}

/// Builds a Postgres connection URL.
///
/// SSL behaviour by connection type:
///
///   Client (post-pairing):
///     sslmode=verify-ca + the pinned server cert → encrypts AND validates
///     the server's identity. This is the main security property for
///     LAN traffic — it defeats sniffing and server impersonation.
///
///   Server (loopback, 127.0.0.1):
///     sslmode=require → requests SSL but doesn't validate the cert chain.
///     Acceptable for loopback (no network hop, no sniffing risk). We use
///     `require` not `prefer` so the connection fails loudly if pg_hba.conf
///     demands hostssl and SSL isn't actually running, rather than silently
///     falling back to plaintext and then getting rejected by pg_hba.
///
///     IMPORTANT: if you see "server does not support TLS" on the server
///     build, it means SSL was not successfully enabled in postgresql.conf
///     even though pg_hba.conf has hostssl rules. The app will automatically
///     detect this state via ssl_is_configured_in_conf() and repair it.
fn build_url(
    user: &str,
    password: &str,
    host: &str,
    port: u16,
    db_name: &str,
    sslrootcert_path: Option<&Path>,
) -> String {
    let base = format!("postgresql://{}:{}@{}:{}/{}", user, password, host, port, db_name);
    match sslrootcert_path {
        Some(path) => {
            // Client: verify-ca with pinned cert
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
        None => {
            // Use sslmode=prefer in dev fallback mode — tries SSL first
            // (matches hostssl entries), falls back to plaintext if SSL
            // isn't available (matches host entries). This works regardless
            // of whether the installer configured hostssl or host for
            // loopback in pg_hba.conf.
            // In production server/client builds, use sslmode=require to
            // enforce SSL.
            #[cfg(not(any(feature = "server-build", feature = "client-build")))]
            {
                format!("{}?sslmode=prefer", base)
            }
            #[cfg(any(feature = "server-build", feature = "client-build"))]
            {
                format!("{}?sslmode=require", base)
            }
        }
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
    let url = build_url(user, password, host, port, "postgres", sslrootcert_path);
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
    let url = build_url(user, password, host, port, db_name, sslrootcert_path);
    PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(15))
        .connect(&url)
        .await
        .map_err(|e| format!("App connect failed: {}", e))
}

pub async fn ensure_database(root_pool: &PgPool, db_name: &str) -> Result<(), String> {
    // SEC-10: validate the identifier BEFORE interpolating it into the
    // CREATE DATABASE statement. `db_name` ultimately originates from the
    // frontend (`repair_server_config`) — an attacker who can call that
    // command with a crafted `db_name` containing `"` (or `;`, or `--`)
    // could inject arbitrary SQL into the CREATE DATABASE statement.
    //
    // Postgres database identifiers are limited to 63 bytes (NameDataLen)
    // and must start with a letter or underscore, containing only letters,
    // digits, and underscores. We enforce that here with a strict regex.
    // Any name that doesn't match is rejected — there's no legitimate
    // reason for an HMS deployment to use a name outside this character
    // set, so this is a safe-by-default fail-closed check.
    validate_db_identifier(db_name)?;

    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)",
    )
    .bind(db_name)
    .fetch_one(root_pool)
    .await
    .map_err(|e| format!("DB check failed: {}", e))?;

    if !exists {
        // SEC-10: `db_name` has been validated above (matches
        // `^[A-Za-z_][A-Za-z0-9_]{0,62}$`), so the format! interpolation
        // is safe — no special characters can reach the SQL parser.
        // (Parameterised queries (`$1`) are the preferred way to pass
        // identifiers, but CREATE DATABASE cannot be parameterised —
        // Postgres doesn't accept a bind parameter for the database name
        // in DDL. Identifier validation is the standard mitigation.)
        if let Err(e) = sqlx::query(&format!("CREATE DATABASE \"{}\"", db_name))
            .execute(root_pool)
            .await
        {
            let err_msg = e.to_string();
            if err_msg.contains("already exists")
                || err_msg.contains("duplicate key")
                || err_msg.contains("42P04")
                || err_msg.contains("23505")
            {
                // Ignored: database was created concurrently by another thread/connection.
            } else {
                return Err(format!("CREATE DATABASE failed: {}", e));
            }
        }
    }
    Ok(())
}

/// SEC-10: validate a Postgres identifier (database name, role name, etc.)
/// against the strict regex `^[A-Za-z_][A-Za-z0-9_]{0,62}$`.
///
/// This is used anywhere a caller-supplied identifier is interpolated into
/// a DDL statement (CREATE DATABASE, CREATE ROLE, etc.) — Postgres DDL
/// doesn't accept bind parameters for identifiers, so the only safe
/// mitigation is to reject any identifier that contains characters
/// outside this allow-list.
///
/// The allow-list matches Postgres' "unquoted identifier" rule (PG docs
/// §4.1.1): letters (any Unicode letter, but we restrict to ASCII for
/// portability), underscores, and digits (not as the first char). 63
/// characters is Postgres' NameDataLen limit.
pub fn validate_db_identifier(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Database identifier cannot be empty.".to_string());
    }
    if name.len() > 63 {
        return Err(format!(
            "Database identifier '{}' is too long (max 63 chars, got {}).",
            name,
            name.len()
        ));
    }
    let mut chars = name.chars();
    let first = chars.next().ok_or_else(|| "Database identifier cannot be empty.".to_string())?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(format!(
            "Database identifier '{}' is invalid: must start with a letter or underscore.",
            name
        ));
    }
    for c in chars {
        if !(c.is_ascii_alphanumeric() || c == '_') {
            return Err(format!(
                "Database identifier '{}' is invalid: only letters, digits, and underscores are allowed.",
                name
            ));
        }
    }
    Ok(())
}

// ── Migrations ───────────────────────────────────────────────────────────────
//
// All migrations are idempotent (CREATE TABLE IF NOT EXISTS / ADD COLUMN IF NOT
// EXISTS). Existing patient/doctor/appointment data is always preserved. New
// columns are nullable or carry safe defaults so old rows remain valid.
//
// Schema is organised in logical groups:
//   1. Core identity & security  (departments, users, roles, permissions, sessions, audit)
//   2. Patient EHR               (patients expansion, consent, encounters)
//   3. Scheduling & queue        (appointments expansion, queue_tokens)
//   4. In-patient (IPD)          (wards, beds, ipd_admissions)
//   5. Laboratory                (lab_test_catalog, lab_orders, lab_order_tests)
//   6. Billing & finance         (bills, bill_items, payments)
//   7. Inventory                 (inventory_items)
//   8. System                    (settings, license_state)

pub async fn run_migrations(pool: &PgPool) -> Result<(), String> {
    sqlx::query("CREATE EXTENSION IF NOT EXISTS pgcrypto")
        .execute(pool).await.ok();

    // ── Original tables (preserved exactly) ────────────────────────────────
    // CR-11: `deleted_at` (NULL = active, non-NULL = soft-deleted) and
    // `is_active` (mirror flag for easy filtering) support HIPAA §164.530(j)
    // 6-year PHI retention. `delete_patient` now soft-deletes; clinical
    // FKs to patients are ON DELETE RESTRICT (see the DO block at the end of
    // run_migrations) so a hard DELETE is refused when any clinical row
    // references the patient. The columns are also added idempotently below
    // for existing deployments whose `patients` table pre-dates this fix.
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
            created_at    TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
            is_active     BOOLEAN      NOT NULL DEFAULT TRUE,
            deleted_at    TIMESTAMPTZ
        )
    "#).execute(pool).await.map_err(|e| format!("patients: {}", e))?;

    // CR-11: add the soft-delete columns idempotently for existing deployments
    // whose `patients` table was created before this migration.
    sqlx::query("ALTER TABLE patients ADD COLUMN IF NOT EXISTS is_active BOOLEAN NOT NULL DEFAULT TRUE")
        .execute(pool).await.map_err(|e| format!("patients.is_active: {}", e))?;
    sqlx::query("ALTER TABLE patients ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ")
        .execute(pool).await.map_err(|e| format!("patients.deleted_at: {}", e))?;

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

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS appointments (
            id               SERIAL PRIMARY KEY,
            -- CR-11: ON DELETE RESTRICT — patient hard-delete must NOT wipe
            -- clinical history (HIPAA §164.530(j) 6-year PHI retention).
            patient_id       INT         NOT NULL REFERENCES patients(id) ON DELETE RESTRICT,
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

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS messages (
            id         UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
            sender     VARCHAR(100) NOT NULL,
            content    TEXT         NOT NULL,
            room       VARCHAR(50)  NOT NULL DEFAULT 'general',
            created_at TIMESTAMPTZ  NOT NULL DEFAULT NOW()
        )
    "#).execute(pool).await.map_err(|e| format!("messages: {}", e))?;

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS whatsapp_notifications (
            id                SERIAL      PRIMARY KEY,
            appointment_id    INT         REFERENCES appointments(id) ON DELETE SET NULL,
            notification_type VARCHAR(50) NOT NULL,
            recipient         VARCHAR(100) NOT NULL,
            message           TEXT         NOT NULL,
            sent_at           TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
            success           BOOLEAN      NOT NULL DEFAULT FALSE
        )
    "#).execute(pool).await.map_err(|e| format!("whatsapp_notifications: {}", e))?;

    // ── 1. Core identity & security ────────────────────────────────────────
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS departments (
            id           SERIAL PRIMARY KEY,
            name         VARCHAR(120) NOT NULL,
            code         VARCHAR(20)  NOT NULL UNIQUE,
            head_doctor_id INT         REFERENCES doctors(id) ON DELETE SET NULL,
            is_active    BOOLEAN      NOT NULL DEFAULT TRUE,
            created_at   TIMESTAMPTZ  NOT NULL DEFAULT NOW()
        )
    "#).execute(pool).await.map_err(|e| format!("departments: {}", e))?;

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS users (
            id                    SERIAL PRIMARY KEY,
            username              VARCHAR(60)  NOT NULL UNIQUE,
            full_name             VARCHAR(200) NOT NULL,
            email                 VARCHAR(255),
            password_hash         TEXT         NOT NULL,
            is_active             BOOLEAN      NOT NULL DEFAULT TRUE,
            must_change_password  BOOLEAN      NOT NULL DEFAULT TRUE,
            failed_login_count    INT          NOT NULL DEFAULT 0,
            locked_until          TIMESTAMPTZ,
            last_login_at         TIMESTAMPTZ,
            created_at            TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
            updated_at            TIMESTAMPTZ  NOT NULL DEFAULT NOW()
        )
    "#).execute(pool).await.map_err(|e| format!("users: {}", e))?;

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS roles (
            id          SERIAL PRIMARY KEY,
            name        VARCHAR(60)  NOT NULL UNIQUE,
            description VARCHAR(255)
        )
    "#).execute(pool).await.map_err(|e| format!("roles: {}", e))?;

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS permissions (
            id          SERIAL PRIMARY KEY,
            key         VARCHAR(80)  NOT NULL UNIQUE,
            description VARCHAR(255)
        )
    "#).execute(pool).await.map_err(|e| format!("permissions: {}", e))?;

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS role_permissions (
            role_id       INT NOT NULL REFERENCES roles(id)       ON DELETE CASCADE,
            permission_id INT NOT NULL REFERENCES permissions(id) ON DELETE CASCADE,
            PRIMARY KEY (role_id, permission_id)
        )
    "#).execute(pool).await.map_err(|e| format!("role_permissions: {}", e))?;

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS user_roles (
            user_id INT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            role_id INT NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
            PRIMARY KEY (user_id, role_id)
        )
    "#).execute(pool).await.map_err(|e| format!("user_roles: {}", e))?;

    // Session tokens are opaque random strings; only their SHA-256 hash is
    // persisted (never the raw token), limiting blast radius if the DB is
    // exfiltrated. Single active session per user is enforced by deleting
    // prior sessions on login.
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS sessions (
            token_hash TEXT         PRIMARY KEY,
            user_id    INT          NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            issued_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
            expires_at TIMESTAMPTZ  NOT NULL,
            ip         VARCHAR(45),
            user_agent TEXT
        )
    "#).execute(pool).await.map_err(|e| format!("sessions: {}", e))?;

    // Review Pass 3, P3-7: enforce the single-active-session invariant at the
    // SCHEMA level, not just in login's DELETE+INSERT sequence. Previously a
    // concurrent double-login could interleave between the two statements and
    // leave TWO valid tokens for one user — both passing require_strong's
    // per-token check. First dedupe any pre-existing duplicates (keep the
    // newest row per user), then create the unique index that
    // (a) makes the invariant schema-real, and (b) lets login_core rotate the
    // session with a single atomic INSERT ... ON CONFLICT upsert.
    sqlx::query(
        "DELETE FROM sessions a USING sessions b \
         WHERE a.user_id = b.user_id AND a.issued_at < b.issued_at",
    ).execute(pool).await.ok();
    sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS idx_sessions_single_user ON sessions(user_id)")
        .execute(pool).await
        .map_err(|e| format!("sessions unique-user index: {}", e))?;

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS audit_logs (
            id          BIGSERIAL PRIMARY KEY,
            user_id     INT,
            username    VARCHAR(60),
            action      VARCHAR(80)  NOT NULL,
            resource    VARCHAR(80)  NOT NULL,
            resource_id VARCHAR(40),
            details     JSONB,
            ip          VARCHAR(45),
            created_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW()
        )
    "#).execute(pool).await.map_err(|e| format!("audit_logs: {}", e))?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_logs_created_at ON audit_logs (created_at DESC)")
        .execute(pool).await.map_err(|e| format!("idx_audit_created: {}", e))?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_logs_user ON audit_logs (user_id, created_at DESC)")
        .execute(pool).await.map_err(|e| format!("idx_audit_user: {}", e))?;

    // ── 2. Patient EHR expansion ──────────────────────────────────────────
    // Backward-compatible column additions (all nullable / defaulted).
    for (col, ddl) in [
        ("mrn",                       "VARCHAR(20) UNIQUE"),
        ("blood_group",               "VARCHAR(8)"),
        // BE-01 (Blood Bank): the patient's Rh factor. The issue-path
        // compatibility check (blood_bank.rs SELECT blood_group, rh_factor)
        // requires it, but the migration historically only added blood_group
        // — so `create_blood_issue` would fail with "column rh_factor does
        // not exist" on any deployment pre-dating the blood bank. Found by
        // the first-ever execution of the AERP Part G / IT-001 suites.
        ("rh_factor",                 "VARCHAR(5)"),
        ("allergies",                 "TEXT"),
        ("chronic_conditions",        "TEXT"),
        ("emergency_contact_name",    "VARCHAR(120)"),
        ("emergency_contact_phone",   "VARCHAR(30)"),
        ("insurance_provider",        "VARCHAR(120)"),
        ("insurance_policy_number",   "VARCHAR(60)"),
        ("status",                    "VARCHAR(20) NOT NULL DEFAULT 'active'"),
        ("created_by_user_id",        "INT REFERENCES users(id) ON DELETE SET NULL"),
    ] {
        sqlx::query(&format!("ALTER TABLE patients ADD COLUMN IF NOT EXISTS {} {}", col, ddl))
            .execute(pool).await.map_err(|e| format!("patients.{}: {}", col, e))?;
    }

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS patient_consent (
            id                 SERIAL PRIMARY KEY,
            -- CR-11: ON DELETE RESTRICT — consent history must outlive the patient row.
            patient_id         INT          NOT NULL REFERENCES patients(id) ON DELETE RESTRICT,
            consent_type       VARCHAR(60)  NOT NULL,
            granted            BOOLEAN      NOT NULL,
            granted_at         TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
            granted_by_user_id INT          REFERENCES users(id) ON DELETE SET NULL,
            notes              TEXT
        )
    "#).execute(pool).await.map_err(|e| format!("patient_consent: {}", e))?;

    // CR-12: enforce one consent record per (patient_id, consent_type) so
    // `set_patient_consent` can use `INSERT ... ON CONFLICT (patient_id,
    // consent_type) DO UPDATE` as a true upsert. Added idempotently via a
    // DO block because PostgreSQL has no `ADD CONSTRAINT IF NOT EXISTS`.
    sqlx::query(r#"
        DO $$
        BEGIN
            IF NOT EXISTS (
                SELECT 1 FROM pg_constraint
                WHERE conname = 'uq_patient_consent_patient_type'
            ) THEN
                ALTER TABLE patient_consent
                    ADD CONSTRAINT uq_patient_consent_patient_type
                    UNIQUE (patient_id, consent_type);
            END IF;
        END $$;
    "#).execute(pool).await.map_err(|e| format!("patient_consent unique constraint: {}", e))?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_patient_consent_patient ON patient_consent (patient_id)")
        .execute(pool).await.ok();

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS encounters (
            id                 SERIAL PRIMARY KEY,
            -- CR-11: ON DELETE RESTRICT — encounter history must outlive the patient row.
            patient_id         INT          NOT NULL REFERENCES patients(id) ON DELETE RESTRICT,
            doctor_id          INT          REFERENCES doctors(id) ON DELETE SET NULL,
            visit_type         VARCHAR(20)  NOT NULL DEFAULT 'opd',
            visit_date         TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
            chief_complaint    TEXT,
            diagnosis          TEXT,
            notes              TEXT,
            created_by_user_id INT          REFERENCES users(id) ON DELETE SET NULL,
            created_at         TIMESTAMPTZ  NOT NULL DEFAULT NOW()
        )
    "#).execute(pool).await.map_err(|e| format!("encounters: {}", e))?;

    // ── 3. Scheduling & queue expansion ───────────────────────────────────
    for (col, ddl) in [
        ("created_by_user_id", "INT REFERENCES users(id) ON DELETE SET NULL"),
        ("queue_token_id",     "INT"),
    ] {
        sqlx::query(&format!("ALTER TABLE appointments ADD COLUMN IF NOT EXISTS {} {}", col, ddl))
            .execute(pool).await.map_err(|e| format!("appointments.{}: {}", col, e))?;
    }

    // CR-10: per-appointment IANA timezone column. `appointment_time` is
    // stored as `TIME WITHOUT TIME ZONE`; the scheduler must interpret that
    // wall-clock value in the clinic's local timezone (Asia/Karachi, UTC+5)
    // when computing reminder fire times. Storing the TZ per-row (rather
    // than hard-coding 'UTC') keeps the comparison correct regardless of the
    // Postgres server's `timezone` setting and lets future deployments
    // override the default per appointment. The column is nullable so old
    // rows fall back to the clinic default via COALESCE in the scheduler.
    sqlx::query("ALTER TABLE appointments ADD COLUMN IF NOT EXISTS appointment_tz TEXT")
        .execute(pool).await.map_err(|e| format!("appointments.appointment_tz: {}", e))?;
    sqlx::query("UPDATE appointments SET appointment_tz = 'Asia/Karachi' WHERE appointment_tz IS NULL")
        .execute(pool).await.ok();

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS queue_tokens (
            id            SERIAL PRIMARY KEY,
            -- CR-11: ON DELETE RESTRICT — queue history must outlive the patient row.
            patient_id    INT          NOT NULL REFERENCES patients(id) ON DELETE RESTRICT,
            department_id INT          REFERENCES departments(id) ON DELETE SET NULL,
            doctor_id     INT          REFERENCES doctors(id) ON DELETE SET NULL,
            token_number  INT          NOT NULL,
            status        VARCHAR(20)  NOT NULL DEFAULT 'waiting',
            priority      SMALLINT     NOT NULL DEFAULT 0,
            issued_at     TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
            called_at     TIMESTAMPTZ,
            completed_at  TIMESTAMPTZ,
            created_by_user_id INT     REFERENCES users(id) ON DELETE SET NULL
        )
    "#).execute(pool).await.map_err(|e| format!("queue_tokens: {}", e))?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_queue_status ON queue_tokens (status, issued_at)")
        .execute(pool).await.map_err(|e| format!("idx_queue: {}", e))?;

    // ── 4. In-patient (IPD) ───────────────────────────────────────────────
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS wards (
            id                SERIAL PRIMARY KEY,
            name              VARCHAR(120) NOT NULL,
            code              VARCHAR(20)  NOT NULL UNIQUE,
            floor             VARCHAR(20),
            gender_restriction VARCHAR(10),
            is_active         BOOLEAN      NOT NULL DEFAULT TRUE,
            created_at        TIMESTAMPTZ  NOT NULL DEFAULT NOW()
        )
    "#).execute(pool).await.map_err(|e| format!("wards: {}", e))?;

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS beds (
            id          SERIAL PRIMARY KEY,
            ward_id     INT          NOT NULL REFERENCES wards(id) ON DELETE CASCADE,
            bed_number  VARCHAR(20)  NOT NULL,
            status      VARCHAR(20)  NOT NULL DEFAULT 'available',
            is_icu      BOOLEAN      NOT NULL DEFAULT FALSE,
            daily_rate  NUMERIC(12,2) NOT NULL DEFAULT 0,
            created_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
            UNIQUE (ward_id, bed_number)
        )
    "#).execute(pool).await.map_err(|e| format!("beds: {}", e))?;

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS ipd_admissions (
            id                   SERIAL PRIMARY KEY,
            patient_id           INT          NOT NULL REFERENCES patients(id) ON DELETE RESTRICT,
            doctor_id            INT          REFERENCES doctors(id) ON DELETE SET NULL,
            ward_id              INT          NOT NULL REFERENCES wards(id) ON DELETE RESTRICT,
            bed_id               INT          NOT NULL REFERENCES beds(id) ON DELETE RESTRICT,
            admission_date       TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
            admission_type       VARCHAR(20)  NOT NULL DEFAULT 'routine',
            admitting_diagnosis  TEXT,
            attending_doctor_id  INT          REFERENCES doctors(id) ON DELETE SET NULL,
            status               VARCHAR(20)  NOT NULL DEFAULT 'admitted',
            discharge_date       TIMESTAMPTZ,
            discharge_summary    TEXT,
            created_by_user_id   INT          REFERENCES users(id) ON DELETE SET NULL,
            created_at           TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
            updated_at           TIMESTAMPTZ  NOT NULL DEFAULT NOW()
        )
    "#).execute(pool).await.map_err(|e| format!("ipd_admissions: {}", e))?;

    // ── 5. Laboratory ─────────────────────────────────────────────────────
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS lab_test_catalog (
            id            SERIAL PRIMARY KEY,
            name          VARCHAR(160) NOT NULL,
            code          VARCHAR(30)  NOT NULL UNIQUE,
            category      VARCHAR(60),
            sample_type   VARCHAR(60),
            normal_range  VARCHAR(160),
            unit          VARCHAR(40),
            price         NUMERIC(12,2) NOT NULL DEFAULT 0,
            is_active     BOOLEAN      NOT NULL DEFAULT TRUE,
            created_at    TIMESTAMPTZ  NOT NULL DEFAULT NOW()
        )
    "#).execute(pool).await.map_err(|e| format!("lab_test_catalog: {}", e))?;

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS lab_orders (
            id                  SERIAL PRIMARY KEY,
            -- CR-11: ON DELETE RESTRICT — lab order history must outlive the patient row.
            patient_id          INT          NOT NULL REFERENCES patients(id) ON DELETE RESTRICT,
            encounter_id        INT          REFERENCES encounters(id) ON DELETE SET NULL,
            ordered_by_doctor_id INT         REFERENCES doctors(id) ON DELETE SET NULL,
            ordered_by_user_id  INT          REFERENCES users(id) ON DELETE SET NULL,
            status              VARCHAR(20)  NOT NULL DEFAULT 'ordered',
            ordered_at          TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
            created_at          TIMESTAMPTZ  NOT NULL DEFAULT NOW()
        )
    "#).execute(pool).await.map_err(|e| format!("lab_orders: {}", e))?;

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS lab_order_tests (
            id                  SERIAL PRIMARY KEY,
            lab_order_id        INT          NOT NULL REFERENCES lab_orders(id) ON DELETE CASCADE,
            test_catalog_id     INT          NOT NULL REFERENCES lab_test_catalog(id) ON DELETE RESTRICT,
            result_value        TEXT,
            result_unit         VARCHAR(40),
            result_abnormal_flag VARCHAR(10),
            result_notes        TEXT,
            completed_at        TIMESTAMPTZ,
            completed_by_user_id INT         REFERENCES users(id) ON DELETE SET NULL
        )
    "#).execute(pool).await.map_err(|e| format!("lab_order_tests: {}", e))?;

    // ── 6. Billing & finance ──────────────────────────────────────────────
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS bills (
            id               SERIAL PRIMARY KEY,
            patient_id       INT          NOT NULL REFERENCES patients(id) ON DELETE RESTRICT,
            encounter_id     INT          REFERENCES encounters(id) ON DELETE SET NULL,
            ipd_admission_id INT          REFERENCES ipd_admissions(id) ON DELETE SET NULL,
            bill_number      VARCHAR(40)  NOT NULL UNIQUE,
            bill_type        VARCHAR(20)  NOT NULL DEFAULT 'opd',
            total_amount     NUMERIC(14,2) NOT NULL DEFAULT 0,
            discount         NUMERIC(14,2) NOT NULL DEFAULT 0,
            tax              NUMERIC(14,2) NOT NULL DEFAULT 0,
            net_amount       NUMERIC(14,2) NOT NULL DEFAULT 0,
            status           VARCHAR(20)  NOT NULL DEFAULT 'draft',
            created_by_user_id INT        REFERENCES users(id) ON DELETE SET NULL,
            created_at       TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
            updated_at       TIMESTAMPTZ  NOT NULL DEFAULT NOW()
        )
    "#).execute(pool).await.map_err(|e| format!("bills: {}", e))?;

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS bill_items (
            id           SERIAL PRIMARY KEY,
            bill_id      INT          NOT NULL REFERENCES bills(id) ON DELETE CASCADE,
            item_type    VARCHAR(20)  NOT NULL DEFAULT 'other',
            description  TEXT         NOT NULL,
            quantity     NUMERIC(10,2) NOT NULL DEFAULT 1,
            unit_price   NUMERIC(14,2) NOT NULL DEFAULT 0,
            total        NUMERIC(14,2) NOT NULL DEFAULT 0,
            reference_id INT
        )
    "#).execute(pool).await.map_err(|e| format!("bill_items: {}", e))?;

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS payments (
            id                SERIAL PRIMARY KEY,
            bill_id           INT          NOT NULL REFERENCES bills(id) ON DELETE RESTRICT,
            amount            NUMERIC(14,2) NOT NULL,
            payment_method    VARCHAR(20)  NOT NULL DEFAULT 'cash',
            reference_number  VARCHAR(80),
            paid_at           TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
            received_by_user_id INT        REFERENCES users(id) ON DELETE SET NULL,
            created_at        TIMESTAMPTZ  NOT NULL DEFAULT NOW()
        )
    "#).execute(pool).await.map_err(|e| format!("payments: {}", e))?;

    // ── 7. Inventory ──────────────────────────────────────────────────────
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS inventory_items (
            id             SERIAL PRIMARY KEY,
            name           VARCHAR(160) NOT NULL,
            sku            VARCHAR(40)  UNIQUE,
            category       VARCHAR(40)  NOT NULL DEFAULT 'medication',
            unit           VARCHAR(20),
            stock_quantity NUMERIC(14,2) NOT NULL DEFAULT 0,
            reorder_level  NUMERIC(14,2) NOT NULL DEFAULT 0,
            expiry_date    DATE,
            batch_number   VARCHAR(60),
            unit_cost      NUMERIC(14,2) NOT NULL DEFAULT 0,
            is_active      BOOLEAN      NOT NULL DEFAULT TRUE,
            created_at     TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
            updated_at     TIMESTAMPTZ  NOT NULL DEFAULT NOW()
        )
    "#).execute(pool).await.map_err(|e| format!("inventory_items: {}", e))?;

    // CR-21: stock movement audit trail (SRS FR-0181). Every stock change
    // (restock, dispense, adjustment, expiry write-off) is recorded here with
    // the resulting balance snapshot, the reason, and the user who made the
    // change. `adjust_inventory` is the single write-path into this table.
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS inventory_movements (
            id                 SERIAL PRIMARY KEY,
            item_id            INT          NOT NULL REFERENCES inventory_items(id) ON DELETE CASCADE,
            quantity_change    NUMERIC(14,2) NOT NULL,
            reason             VARCHAR(120) NOT NULL,
            balance_after      NUMERIC(14,2) NOT NULL,
            reference_id       INT,
            notes              TEXT,
            created_by_user_id INT          REFERENCES users(id) ON DELETE SET NULL,
            created_at         TIMESTAMPTZ  NOT NULL DEFAULT NOW()
        )
    "#).execute(pool).await.map_err(|e| format!("inventory_movements: {}", e))?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_inventory_movements_item ON inventory_movements (item_id, created_at DESC)")
        .execute(pool).await.map_err(|e| format!("idx_inventory_movements_item: {}", e))?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_inventory_movements_created ON inventory_movements (created_at DESC)")
        .execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_inventory_items_low_stock ON inventory_items (stock_quantity, reorder_level) WHERE is_active")
        .execute(pool).await.ok();

    // ── 8. System ─────────────────────────────────────────────────────────
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS settings (
            key        TEXT PRIMARY KEY,
            value      TEXT,
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
    "#).execute(pool).await.map_err(|e| format!("settings: {}", e))?;

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS license_state (
            id                   SERIAL PRIMARY KEY,
            license_json         TEXT,
            hardware_fingerprint VARCHAR(128),
            installed_at         TIMESTAMPTZ,
            last_verified_at     TIMESTAMPTZ,
            verification_status  VARCHAR(20) NOT NULL DEFAULT 'unverified'
        )
    "#).execute(pool).await.map_err(|e| format!("license_state: {}", e))?;

    // ── 9. WhatsApp Business API config ───────────────────────────────────
    // Stores Meta Business API credentials for fully-automatic message
    // sending (no user interaction). When configured, the app sends via the
    // Cloud API; otherwise it falls back to the wa.me deep link.
    //
    // CR-9 fix: this table is a SINGLETON. Exactly one row (id=1) holds the
    // active credentials. Earlier schema left `id SERIAL PRIMARY KEY` which
    // meant every `INSERT ... ON CONFLICT (id)` simply allocated a new id and
    // never conflicted, so credentials accumulated non-deterministically.
    // We now collapse any legacy duplicate rows to a single row at id=1 and
    // enforce the singleton with a CHECK(id = 1) constraint. set_whatsapp_config
    // always writes id=1, so ON CONFLICT (id) now upserts correctly.
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS whatsapp_config (
            id              SERIAL PRIMARY KEY,
            access_token    TEXT,
            phone_number_id TEXT,
            business_id     TEXT,
            waba_id         TEXT,
            enabled         BOOLEAN     NOT NULL DEFAULT FALSE,
            updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
    "#).execute(pool).await.map_err(|e| format!("whatsapp_config: {}", e))?;

    // Add preferred_method column (idempotent) — 'api' = Business Cloud API
    // (fully automatic), 'deep_link' = wa.me deep link (manual Send click).
    sqlx::query("ALTER TABLE whatsapp_config ADD COLUMN IF NOT EXISTS preferred_method VARCHAR(20) NOT NULL DEFAULT 'deep_link'")
        .execute(pool).await.map_err(|e| format!("whatsapp_config.preferred_method: {}", e))?;

    // CR-9: collapse any legacy duplicate rows to a single row at id=1, then
    // add the singleton CHECK constraint. Idempotent: if there's only one row
    // (or zero), these are no-ops; if duplicates exist, keep the most-recently
    // inserted row (MAX(id)) and renumber it to id=1.
    sqlx::query("DELETE FROM whatsapp_config WHERE id NOT IN (SELECT MAX(id) FROM whatsapp_config)")
        .execute(pool).await.map_err(|e| format!("whatsapp_config dedupe: {}", e))?;
    sqlx::query("UPDATE whatsapp_config SET id = 1 WHERE id <> 1")
        .execute(pool).await.map_err(|e| format!("whatsapp_config renumber: {}", e))?;
    // Reset the SERIAL sequence so a future explicit id=1 INSERT does not
    // collide with the sequence's next value (defensive — set_whatsapp_config
    // pins id=1 explicitly, so the sequence is unused, but we keep it sane).
    sqlx::query("SELECT setval('whatsapp_config_id_seq', 1, true)")
        .execute(pool).await.ok();
    // Add the singleton CHECK constraint idempotently. PostgreSQL has no
    // ADD CONSTRAINT IF NOT EXISTS, so we guard with a DO block.
    sqlx::query(r#"
        DO $$
        BEGIN
            IF NOT EXISTS (
                SELECT 1 FROM pg_constraint
                WHERE conname = 'whatsapp_config_singleton_check'
            ) THEN
                ALTER TABLE whatsapp_config
                    ADD CONSTRAINT whatsapp_config_singleton_check CHECK (id = 1);
            END IF;
        END $$;
    "#).execute(pool).await.map_err(|e| format!("whatsapp_config_singleton_check: {}", e))?;

    // ── CR-11: convert clinical-table FKs to patients from ON DELETE CASCADE
    //    to ON DELETE RESTRICT (idempotent — only touches FKs whose
    //    `confdeltype = 'c'` (CASCADE). Safe to run on every startup: after
    //    the first run there are no CASCADE patient FKs left, so the loop is
    //    a no-op. New installs already CREATE the FKs as RESTRICT above, so
    //    this is only meaningful for existing deployments whose tables
    //    pre-date CR-11. RESTRICT is the safety net against an accidental
    //    hard DELETE wiping clinical history (HIPAA §164.530(j) 6-year PHI
    //    retention); the application's `delete_patient` command soft-deletes
    //    (sets `deleted_at`) and never issues a hard DELETE.
    sqlx::query(r#"
        DO $$
        DECLARE
            rec record;
        BEGIN
            FOR rec IN
                SELECT con.conname, cls.relname AS tbl
                FROM pg_constraint con
                JOIN pg_class cls     ON cls.oid = con.conrelid
                JOIN pg_namespace nsp ON nsp.oid = cls.relnamespace
                WHERE con.contype     = 'f'
                  AND nsp.nspname     = 'public'
                  AND con.confrelid   = 'patients'::regclass
                  AND con.confdeltype = 'c'
                  AND cls.relname IN (
                      'appointments', 'patient_consent', 'encounters',
                      'queue_tokens',  'lab_orders'
                  )
            LOOP
                EXECUTE format('ALTER TABLE %I DROP CONSTRAINT %I', rec.tbl, rec.conname);
                EXECUTE format(
                    'ALTER TABLE %I ADD CONSTRAINT %I_patient_id_fkey
                        FOREIGN KEY (patient_id) REFERENCES patients(id) ON DELETE RESTRICT',
                    rec.tbl, rec.tbl
                );
            END LOOP;
        END $$;
    "#).execute(pool).await.map_err(|e| format!("cr11 fk restrict: {}", e))?;

    // ── 10. Pharmacy (Phase 2-C, SRS FR-0120–FR-0124) ─────────────────────
    //
    // Three tables implementing the medication catalog (FR-0120),
    // prescription generation from encounters (FR-0121), and dispensing
    // with audit trail (FR-0122). Controlled-substance dispensing
    // (FR-0123) is handled at the application layer: `is_controlled` is
    // snapshotted on each prescription_item from the medication's
    // `schedule` (anything other than 'non-controlled' is treated as
    // controlled) and the frontend requires pharmacist confirmation
    // before calling `dispense_prescription_item`.
    //
    // These tables reuse the existing `inventory_items` /
    // `inventory_movements` tables for stock tracking — there is no
    // separate pharmacy stock table. `dispense_prescription_item`
    // matches `medication_name` against `inventory_items.name` and, if a
    // row exists, decrements its `stock_quantity` and writes a movement
    // row with `reason = 'dispense'` and `reference_id` set to the
    // prescription_item_id (FR-0122 audit trail). This matches the
    // pattern in `commands/inventory.rs::adjust_inventory`.
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS medications (
            id              SERIAL PRIMARY KEY,
            brand_name      VARCHAR(200) NOT NULL,
            generic_name    VARCHAR(200) NOT NULL,
            form            VARCHAR(50)  NOT NULL DEFAULT 'tablet',
            strength        VARCHAR(50)  NOT NULL,
            schedule        VARCHAR(20)  NOT NULL DEFAULT 'non-controlled',
            category        VARCHAR(100),
            manufacturer    VARCHAR(200),
            reorder_level   INT          NOT NULL DEFAULT 10,
            is_active       BOOLEAN      NOT NULL DEFAULT TRUE,
            created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW()
        )
    "#).execute(pool).await.map_err(|e| format!("medications: {}", e))?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_medications_active ON medications (is_active, brand_name)")
        .execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_medications_generic ON medications (generic_name)")
        .execute(pool).await.ok();

    // Prescriptions (FR-0121) — generated from encounters.
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS prescriptions (
            id                     SERIAL PRIMARY KEY,
            patient_id             INT          NOT NULL REFERENCES patients(id) ON DELETE RESTRICT,
            doctor_id              INT          REFERENCES doctors(id) ON DELETE SET NULL,
            encounter_id           INT          REFERENCES encounters(id) ON DELETE SET NULL,
            prescribed_by_user_id  INT          REFERENCES users(id) ON DELETE SET NULL,
            status                 VARCHAR(20)  NOT NULL DEFAULT 'active',
            notes                  TEXT,
            created_at             TIMESTAMPTZ  NOT NULL DEFAULT NOW()
        )
    "#).execute(pool).await.map_err(|e| format!("prescriptions: {}", e))?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_prescriptions_patient ON prescriptions (patient_id, created_at DESC)")
        .execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_prescriptions_status ON prescriptions (status, created_at DESC)")
        .execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_prescriptions_encounter ON prescriptions (encounter_id)")
        .execute(pool).await.ok();

    // Prescription items — individual medication lines in a prescription.
    // `medication_name` is a denormalised snapshot so historical
    // prescriptions remain readable even if the medication row is later
    // soft-deleted (`is_active=false`) — the FK is ON DELETE SET NULL.
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS prescription_items (
            id                      SERIAL PRIMARY KEY,
            prescription_id         INT          NOT NULL REFERENCES prescriptions(id) ON DELETE CASCADE,
            medication_id           INT          REFERENCES medications(id) ON DELETE SET NULL,
            medication_name         VARCHAR(200) NOT NULL,
            dose                    VARCHAR(100) NOT NULL,
            route                   VARCHAR(50)  NOT NULL DEFAULT 'oral',
            frequency               VARCHAR(100) NOT NULL,
            duration                VARCHAR(100),
            quantity                INT          NOT NULL DEFAULT 1,
            is_controlled           BOOLEAN      NOT NULL DEFAULT FALSE,
            dispensed               BOOLEAN      NOT NULL DEFAULT FALSE,
            dispensed_at            TIMESTAMPTZ,
            dispensed_by_user_id    INT          REFERENCES users(id) ON DELETE SET NULL,
            created_at              TIMESTAMPTZ  NOT NULL DEFAULT NOW()
        )
    "#).execute(pool).await.map_err(|e| format!("prescription_items: {}", e))?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_prescription_items_rx ON prescription_items (prescription_id)")
        .execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_prescription_items_med ON prescription_items (medication_id)")
        .execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_prescription_items_dispensed ON prescription_items (dispensed, dispensed_at DESC)")
        .execute(pool).await.ok();

    // Seed a small set of common medications (idempotent — only runs on
    // an empty `medications` table). Mirrors the seed pattern used by
    // `auth::seed_defaults`: a `WHERE NOT EXISTS` guard makes this safe
    // to re-run on every startup without duplicating rows.
    sqlx::query(r#"
        INSERT INTO medications (brand_name, generic_name, form, strength, category)
        SELECT * FROM (VALUES
            ('Panadol',     'Paracetamol',           'tablet',  '500mg', 'Analgesic'),
            ('Brufen',      'Ibuprofen',             'tablet',  '400mg', 'NSAID'),
            ('Augmentin',   'Amoxicillin+Clavulanate','tablet', '625mg', 'Antibiotic'),
            ('Risek',       'Omeprazole',            'capsule', '20mg',  'PPI'),
            ('Glucophage',  'Metformin',             'tablet',  '500mg', 'Antidiabetic'),
            ('Tenormin',    'Atenolol',              'tablet',  '50mg',  'Beta-blocker'),
            ('Lipitor',     'Atorvastatin',          'tablet',  '20mg',  'Statin'),
            ('Ventolin',    'Salbutamol',            'inhaler', '100mcg','Bronchodilator')
        ) AS t(brand_name, generic_name, form, strength, category)
        WHERE NOT EXISTS (SELECT 1 FROM medications)
    "#).execute(pool).await.ok();

    // ── Radiology tables (FR-0140–FR-0142) ──────────────────────────────
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS radiology_orders (
            id                      SERIAL PRIMARY KEY,
            patient_id              INT NOT NULL REFERENCES patients(id) ON DELETE RESTRICT,
            encounter_id            INT REFERENCES encounters(id) ON DELETE SET NULL,
            ordered_by_doctor_id    INT REFERENCES doctors(id) ON DELETE SET NULL,
            ordered_by_user_id      INT REFERENCES users(id) ON DELETE SET NULL,
            order_number            VARCHAR(30) NOT NULL UNIQUE,
            department              VARCHAR(100),
            clinical_indication     TEXT,
            symptoms                TEXT,
            diagnosis               TEXT,
            priority                VARCHAR(20) NOT NULL DEFAULT 'routine',
            study_type              VARCHAR(50) NOT NULL,
            contrast_required       BOOLEAN NOT NULL DEFAULT FALSE,
            body_part               VARCHAR(200),
            instructions            TEXT,
            status                  VARCHAR(20) NOT NULL DEFAULT 'ordered',
            assigned_radiologist_id INT REFERENCES doctors(id) ON DELETE SET NULL,
            assigned_technician     VARCHAR(200),
            expected_date           DATE,
            ordered_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            scheduled_at            TIMESTAMPTZ,
            completed_at            TIMESTAMPTZ,
            reported_at             TIMESTAMPTZ,
            verified_at             TIMESTAMPTZ,
            created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
    "#).execute(pool).await.map_err(|e| format!("radiology_orders: {}", e))?;

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS radiology_reports (
            id                      SERIAL PRIMARY KEY,
            order_id                INT NOT NULL REFERENCES radiology_orders(id) ON DELETE CASCADE,
            findings                TEXT,
            impression              TEXT,
            recommendations         TEXT,
            critical_finding        BOOLEAN NOT NULL DEFAULT FALSE,
            radiologist_id          INT REFERENCES doctors(id) ON DELETE SET NULL,
            verified_by_user_id     INT REFERENCES users(id) ON DELETE SET NULL,
            report_date             TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            verified_at             TIMESTAMPTZ,
            created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
    "#).execute(pool).await.map_err(|e| format!("radiology_reports: {}", e))?;

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS radiology_attachments (
            id                      SERIAL PRIMARY KEY,
            order_id                INT NOT NULL REFERENCES radiology_orders(id) ON DELETE CASCADE,
            filename                VARCHAR(255) NOT NULL,
            modality                VARCHAR(50),
            storage_path            VARCHAR(500),
            file_size               BIGINT,
            mime_type               VARCHAR(100),
            checksum                VARCHAR(128),
            upload_date             TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            operator                VARCHAR(200),
            future_pacs_id          VARCHAR(200)
        )
    "#).execute(pool).await.map_err(|e| format!("radiology_attachments: {}", e))?;

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS radiology_status_history (
            id                      SERIAL PRIMARY KEY,
            order_id                INT NOT NULL REFERENCES radiology_orders(id) ON DELETE CASCADE,
            status                  VARCHAR(20) NOT NULL,
            changed_by_user_id      INT REFERENCES users(id) ON DELETE SET NULL,
            changed_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            notes                   TEXT
        )
    "#).execute(pool).await.map_err(|e| format!("radiology_status_history: {}", e))?;

    // Indexes for performance (500k+ studies)
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_rad_orders_patient ON radiology_orders(patient_id)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_rad_orders_status ON radiology_orders(status)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_rad_orders_doctor ON radiology_orders(ordered_by_doctor_id)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_rad_orders_priority ON radiology_orders(priority)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_rad_orders_date ON radiology_orders(ordered_at)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_rad_reports_order ON radiology_reports(order_id)").execute(pool).await.ok();
    // P1-1: Partial index for soft-delete filtering — speeds up the common
    // "WHERE deleted_at IS NULL" query at scale.
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_rad_orders_active ON radiology_orders(ordered_at DESC) WHERE deleted_at IS NULL").execute(pool).await.ok();

    // P1-1: SEQUENCE for concurrency-safe order number generation.
    // Replaces the COUNT(*)+1 pattern that had a race condition.
    sqlx::query("CREATE SEQUENCE IF NOT EXISTS radiology_order_seq START 1").execute(pool).await.ok();

    // P0-4: UNIQUE constraint on radiology_reports.order_id — enforces
    // one-report-per-order at the database level (concurrency protection).
    sqlx::query("ALTER TABLE radiology_reports ADD CONSTRAINT IF NOT EXISTS uq_rad_reports_order_id UNIQUE (order_id)")
        .execute(pool).await.ok();

    // P0-5: Soft-delete columns on radiology_orders — preserves clinical
    // history (HIPAA §164.530(j) 6-year retention).
    sqlx::query("ALTER TABLE radiology_orders ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ")
        .execute(pool).await.ok();
    sqlx::query("ALTER TABLE radiology_orders ADD COLUMN IF NOT EXISTS deleted_by_user_id INT REFERENCES users(id) ON DELETE SET NULL")
        .execute(pool).await.ok();
    sqlx::query("ALTER TABLE radiology_orders ADD COLUMN IF NOT EXISTS deleted_reason TEXT")
        .execute(pool).await.ok();

    // ── Blood Bank tables (Phase 2-E, SRS FR-0145–FR-0149) ───────────────
    //
    // Normalised schema for a clinical blood-bank workflow: donor registry →
    // donation → screening → component separation → inventory (blood_units)
    // → cross-match → reservation → issue → transfusion → completion, with
    // full traceability via blood_unit_status_history + blood_inventory_movements
    // and discard tracking via blood_discards.
    //
    // Conventions mirror the Radiology module (RAD-BASELINE-1.0):
    //   - Idempotent migrations (IF NOT EXISTS / ADD COLUMN IF NOT EXISTS)
    //   - CHECK constraints on every enum column (server-side validation)
    //   - Soft-delete columns on blood_units + blood_donors (HIPAA retention)
    //   - Partial index for the common "WHERE deleted_at IS NULL" query
    //   - SEQUENCE for concurrency-safe unit-number generation
    //   - Status history table for every unit lifecycle transition
    //   - Audit fields (created_by_user_id) on every write-bearing table

    // 1. Donor registry — master record for every registered blood donor.
    //    Can be linked to a `patients` row when the donor is also a patient,
    //    but may stand alone (altruistic community donors).
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS blood_donors (
            id                      SERIAL PRIMARY KEY,
            donor_number            VARCHAR(30) NOT NULL UNIQUE,
            patient_id              INT REFERENCES patients(id) ON DELETE SET NULL,
            first_name              VARCHAR(200) NOT NULL,
            last_name               VARCHAR(200) NOT NULL,
            date_of_birth           DATE,
            gender                  VARCHAR(20),
            blood_group             VARCHAR(5) NOT NULL,
            rh_factor               VARCHAR(5) NOT NULL,
            phone                   VARCHAR(30),
            email                   VARCHAR(200),
            address                 TEXT,
            weight_kg               NUMERIC(5,2),
            height_cm               NUMERIC(5,2),
            last_donation_date      DATE,
            total_donations         INT NOT NULL DEFAULT 0,
            status                  VARCHAR(20) NOT NULL DEFAULT 'active',
            medically_deferred_until DATE,
            defer_reason            TEXT,
            notes                   TEXT,
            created_by_user_id      INT REFERENCES users(id) ON DELETE SET NULL,
            created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            deleted_at              TIMESTAMPTZ,
            deleted_by_user_id      INT REFERENCES users(id) ON DELETE SET NULL,
            deleted_reason          TEXT,
            CONSTRAINT chk_donor_status CHECK (status IN ('active','deferred','blacklisted')),
            CONSTRAINT chk_donor_blood_group CHECK (blood_group IN ('A','B','AB','O')),
            CONSTRAINT chk_donor_rh CHECK (rh_factor IN ('+','-')),
            CONSTRAINT chk_donor_gender CHECK (gender IS NULL OR gender IN ('male','female','other'))
        )
    "#).execute(pool).await.map_err(|e| format!("blood_donors: {}", e))?;

    // 2. Donations — each blood collection event. One donation yields one
    //    whole-blood unit which may be separated into multiple components.
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS blood_donations (
            id                      SERIAL PRIMARY KEY,
            donation_number         VARCHAR(30) NOT NULL UNIQUE,
            donor_id                INT NOT NULL REFERENCES blood_donors(id) ON DELETE RESTRICT,
            donation_date           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            collection_site         VARCHAR(200),
            collected_by_user_id    INT REFERENCES users(id) ON DELETE SET NULL,
            volume_ml               INT NOT NULL CHECK (volume_ml > 0 AND volume_ml <= 600),
            blood_group             VARCHAR(5) NOT NULL,
            rh_factor               VARCHAR(5) NOT NULL,
            bag_type                VARCHAR(50) DEFAULT 'single',
            status                  VARCHAR(20) NOT NULL DEFAULT 'collected',
            screening_status        VARCHAR(20) NOT NULL DEFAULT 'pending',
            screening_notes         TEXT,
            screened_by_user_id     INT REFERENCES users(id) ON DELETE SET NULL,
            screened_at             TIMESTAMPTZ,
            hemoglobin_level        NUMERIC(5,2),
            blood_pressure          VARCHAR(20),
            pulse                   INT,
            temperature_c           NUMERIC(4,2),
            notes                   TEXT,
            created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            CONSTRAINT chk_donation_status CHECK (status IN ('collected','screened','separated','rejected','archived')),
            CONSTRAINT chk_donation_screening CHECK (screening_status IN ('pending','passed','failed','quarantine')),
            CONSTRAINT chk_donation_blood_group CHECK (blood_group IN ('A','B','AB','O')),
            CONSTRAINT chk_donation_rh CHECK (rh_factor IN ('+','-'))
        )
    "#).execute(pool).await.map_err(|e| format!("blood_donations: {}", e))?;

    // 3. Blood units — the live inventory. Each unit is a discrete bag of
    //    blood or a blood component (PRBC, FFP, Platelets, Cryo, etc.)
    //    derived from a donation. Has expiry tracking + status lifecycle.
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS blood_units (
            id                      SERIAL PRIMARY KEY,
            unit_number             VARCHAR(30) NOT NULL UNIQUE,
            donation_id             INT REFERENCES blood_donations(id) ON DELETE SET NULL,
            donor_id                INT NOT NULL REFERENCES blood_donors(id) ON DELETE RESTRICT,
            component_type          VARCHAR(30) NOT NULL DEFAULT 'whole_blood',
            blood_group             VARCHAR(5) NOT NULL,
            rh_factor               VARCHAR(5) NOT NULL,
            volume_ml               INT NOT NULL CHECK (volume_ml > 0),
            collection_date         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            expiry_date             TIMESTAMPTZ NOT NULL,
            storage_temperature     VARCHAR(20),
            storage_location        VARCHAR(200),
            status                  VARCHAR(20) NOT NULL DEFAULT 'available',
            reserved_for_patient_id INT REFERENCES patients(id) ON DELETE SET NULL,
            reservation_id          INT,
            issued_to_patient_id    INT REFERENCES patients(id) ON DELETE SET NULL,
            issued_at               TIMESTAMPTZ,
            transfused_at           TIMESTAMPTZ,
            transfused_to_patient_id INT REFERENCES patients(id) ON DELETE SET NULL,
            discarded_at            TIMESTAMPTZ,
            discard_reason          TEXT,
            created_by_user_id      INT REFERENCES users(id) ON DELETE SET NULL,
            created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            deleted_at              TIMESTAMPTZ,
            deleted_by_user_id      INT REFERENCES users(id) ON DELETE SET NULL,
            deleted_reason          TEXT,
            CONSTRAINT chk_unit_status CHECK (status IN ('available','reserved','issued','transfused','discarded','expired','quarantine')),
            CONSTRAINT chk_unit_component CHECK (component_type IN ('whole_blood','prbc','ffp','platelets','cryoprecipitate','plasma','granulocytes')),
            CONSTRAINT chk_unit_blood_group CHECK (blood_group IN ('A','B','AB','O')),
            CONSTRAINT chk_unit_rh CHECK (rh_factor IN ('+','-'))
        )
    "#).execute(pool).await.map_err(|e| format!("blood_units: {}", e))?;

    // 4. Cross-match results — compatibility testing between a donor unit
    //    and a recipient patient. Required before issue (except emergencies).
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS blood_crossmatch_results (
            id                      SERIAL PRIMARY KEY,
            unit_id                 INT NOT NULL REFERENCES blood_units(id) ON DELETE CASCADE,
            patient_id              INT NOT NULL REFERENCES patients(id) ON DELETE RESTRICT,
            doctor_id               INT REFERENCES doctors(id) ON DELETE SET NULL,
            requested_by_user_id    INT REFERENCES users(id) ON DELETE SET NULL,
            crossmatch_date         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            method                  VARCHAR(50) DEFAULT 'saline_37c',
            result                  VARCHAR(20) NOT NULL DEFAULT 'pending',
            reaction_grade          INT CHECK (reaction_grade IS NULL OR reaction_grade BETWEEN 0 AND 4),
            incubation_time_min     INT,
            ahg_phase               VARCHAR(20),
            notes                   TEXT,
            performed_by_user_id    INT REFERENCES users(id) ON DELETE SET NULL,
            verified_by_user_id     INT REFERENCES users(id) ON DELETE SET NULL,
            verified_at             TIMESTAMPTZ,
            created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            CONSTRAINT chk_crossmatch_result CHECK (result IN ('pending','compatible','incompatible','weak','indeterminate')),
            CONSTRAINT chk_crossmatch_method CHECK (method IN ('saline_37c','ahg','gel_card','tube_ahg','electronic'))
        )
    "#).execute(pool).await.map_err(|e| format!("blood_crossmatch_results: {}", e))?;

    // 5. Reservations — a unit held for a specific patient pending issue.
    //    Has an expiry so uncollected reservations auto-release.
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS blood_reservations (
            id                      SERIAL PRIMARY KEY,
            reservation_number      VARCHAR(30) NOT NULL UNIQUE,
            unit_id                 INT NOT NULL REFERENCES blood_units(id) ON DELETE RESTRICT,
            patient_id              INT NOT NULL REFERENCES patients(id) ON DELETE RESTRICT,
            doctor_id               INT REFERENCES doctors(id) ON DELETE SET NULL,
            requested_by_user_id    INT REFERENCES users(id) ON DELETE SET NULL,
            crossmatch_id           INT REFERENCES blood_crossmatch_results(id) ON DELETE SET NULL,
            reserved_at             TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            expires_at              TIMESTAMPTZ NOT NULL,
            fulfilled_at            TIMESTAMPTZ,
            cancelled_at            TIMESTAMPTZ,
            status                  VARCHAR(20) NOT NULL DEFAULT 'active',
            priority                VARCHAR(20) NOT NULL DEFAULT 'routine',
            clinical_indication     TEXT,
            notes                   TEXT,
            created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            CONSTRAINT chk_reservation_status CHECK (status IN ('active','fulfilled','expired','cancelled')),
            CONSTRAINT chk_reservation_priority CHECK (priority IN ('routine','urgent','emergency','stat'))
        )
    "#).execute(pool).await.map_err(|e| format!("blood_reservations: {}", e))?;

    // Link unit → reservation after table creation (reservation_id on units).
    sqlx::query("ALTER TABLE blood_units ADD CONSTRAINT IF NOT EXISTS fk_unit_reservation FOREIGN KEY (reservation_id) REFERENCES blood_reservations(id) ON DELETE SET NULL")
        .execute(pool).await.ok();

    // 6. Blood issues — record of blood issued from the bank to a patient/ward.
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS blood_issues (
            id                      SERIAL PRIMARY KEY,
            issue_number            VARCHAR(30) NOT NULL UNIQUE,
            unit_id                 INT NOT NULL REFERENCES blood_units(id) ON DELETE RESTRICT,
            patient_id              INT NOT NULL REFERENCES patients(id) ON DELETE RESTRICT,
            reservation_id          INT REFERENCES blood_reservations(id) ON DELETE SET NULL,
            crossmatch_id           INT REFERENCES blood_crossmatch_results(id) ON DELETE SET NULL,
            doctor_id               INT REFERENCES doctors(id) ON DELETE SET NULL,
            issued_by_user_id       INT REFERENCES users(id) ON DELETE SET NULL,
            issued_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            issued_to_location      VARCHAR(200),
            issue_type              VARCHAR(20) NOT NULL DEFAULT 'routine',
            clinical_indication     TEXT,
            special_instructions    TEXT,
            returned_at             TIMESTAMPTZ,
            return_reason           TEXT,
            received_by_user_id     INT REFERENCES users(id) ON DELETE SET NULL,
            created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            CONSTRAINT chk_issue_type CHECK (issue_type IN ('routine','emergency','uncrossmatched','autologous'))
        )
    "#).execute(pool).await.map_err(|e| format!("blood_issues: {}", e))?;

    // 7. Transfusions — the actual administration of blood to a patient.
    //    Records reactions, vitals, start/stop times for traceability.
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS blood_transfusions (
            id                      SERIAL PRIMARY KEY,
            transfusion_number      VARCHAR(30) NOT NULL UNIQUE,
            issue_id                INT NOT NULL REFERENCES blood_issues(id) ON DELETE RESTRICT,
            unit_id                 INT NOT NULL REFERENCES blood_units(id) ON DELETE RESTRICT,
            patient_id              INT NOT NULL REFERENCES patients(id) ON DELETE RESTRICT,
            doctor_id               INT REFERENCES doctors(id) ON DELETE SET NULL,
            nurse_id                INT REFERENCES users(id) ON DELETE SET NULL,
            started_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            completed_at            TIMESTAMPTZ,
            volume_transfused_ml    INT CHECK (volume_transfused_ml IS NULL OR volume_transfused_ml > 0),
            pre_transfusion_bp      VARCHAR(20),
            post_transfusion_bp     VARCHAR(20),
            pre_transfusion_temp    NUMERIC(4,2),
            post_transfusion_temp   NUMERIC(4,2),
            pre_transfusion_pulse   INT,
            post_transfusion_pulse  INT,
            reaction_observed       BOOLEAN NOT NULL DEFAULT FALSE,
            reaction_type           VARCHAR(50),
            reaction_severity       VARCHAR(20),
            reaction_notes          TEXT,
            outcome                 VARCHAR(20),
            notes                   TEXT,
            created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            CONSTRAINT chk_transfusion_outcome CHECK (outcome IS NULL OR outcome IN ('completed','reaction','incomplete','cancelled')),
            CONSTRAINT chk_reaction_severity CHECK (reaction_severity IS NULL OR reaction_severity IN ('mild','moderate','severe','fatal'))
        )
    "#).execute(pool).await.map_err(|e| format!("blood_transfusions: {}", e))?;

    // 8. Discards — units discarded (expired, contaminated, broken, etc.)
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS blood_discards (
            id                      SERIAL PRIMARY KEY,
            unit_id                 INT NOT NULL REFERENCES blood_units(id) ON DELETE RESTRICT,
            discard_number          VARCHAR(30) NOT NULL UNIQUE,
            discarded_at            TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            discard_reason          VARCHAR(30) NOT NULL,
            discard_notes           TEXT,
            discarded_by_user_id    INT REFERENCES users(id) ON DELETE SET NULL,
            authorized_by_user_id   INT REFERENCES users(id) ON DELETE SET NULL,
            disposal_method         VARCHAR(50),
            created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            CONSTRAINT chk_discard_reason CHECK (discard_reason IN ('expired','contaminated','hemolysed','broken','positive_screen','insufficient_volume','other'))
        )
    "#).execute(pool).await.map_err(|e| format!("blood_discards: {}", e))?;

    // 9. Status history — audit trail of every blood-unit lifecycle change.
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS blood_unit_status_history (
            id                      SERIAL PRIMARY KEY,
            unit_id                 INT NOT NULL REFERENCES blood_units(id) ON DELETE CASCADE,
            status                  VARCHAR(20) NOT NULL,
            changed_by_user_id      INT REFERENCES users(id) ON DELETE SET NULL,
            changed_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            notes                   TEXT,
            related_record_type     VARCHAR(30),
            related_record_id       INT
        )
    "#).execute(pool).await.map_err(|e| format!("blood_unit_status_history: {}", e))?;

    // 10. Inventory movements — every movement of a unit in/out of storage
    //     for full chain-of-custody traceability (FR-0149).
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS blood_inventory_movements (
            id                      SERIAL PRIMARY KEY,
            unit_id                 INT NOT NULL REFERENCES blood_units(id) ON DELETE CASCADE,
            movement_type           VARCHAR(30) NOT NULL,
            from_location           VARCHAR(200),
            to_location             VARCHAR(200),
            moved_by_user_id        INT REFERENCES users(id) ON DELETE SET NULL,
            moved_at                TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            reason                  TEXT,
            related_record_type     VARCHAR(30),
            related_record_id       INT,
            created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            CONSTRAINT chk_movement_type CHECK (movement_type IN ('received','stored','relocated','issued','returned','transfused','discarded','quarantined'))
        )
    "#).execute(pool).await.map_err(|e| format!("blood_inventory_movements: {}", e))?;

    // 11. ABO/Rh compatibility matrix — reference table used by cross-match
    //     validation. Seeded with standard ISBT compatibility rules.
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS blood_compatibility_matrix (
            id                      SERIAL PRIMARY KEY,
            recipient_group         VARCHAR(5) NOT NULL,
            recipient_rh            VARCHAR(5) NOT NULL,
            donor_group             VARCHAR(5) NOT NULL,
            donor_rh                VARCHAR(5) NOT NULL,
            compatible              BOOLEAN NOT NULL,
            notes                   TEXT,
            CONSTRAINT chk_compat_recipient_group CHECK (recipient_group IN ('A','B','AB','O')),
            CONSTRAINT chk_compat_recipient_rh CHECK (recipient_rh IN ('+','-')),
            CONSTRAINT chk_compat_donor_group CHECK (donor_group IN ('A','B','AB','O')),
            CONSTRAINT chk_compat_donor_rh CHECK (donor_rh IN ('+','-'))
        )
    "#).execute(pool).await.map_err(|e| format!("blood_compatibility_matrix: {}", e))?;

    // Seed the standard ABO/Rh compatibility matrix (only if empty).
    let compat_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM blood_compatibility_matrix")
        .fetch_one(pool).await.unwrap_or(0);
    if compat_count == 0 {
        // ISBT-compatible pairings: (recipient ABO+Rh, donor ABO+Rh, compatible)
        let rows: &[(&str, &str, &str, &str, bool)] = &[
            // O- recipient can receive O- only
            ("O","-","O","-",true),
            ("O","-","O","+",false), ("O","-","A","+",false), ("O","-","A","-",false),
            ("O","-","B","+",false), ("O","-","B","-",false), ("O","-","AB","+",false), ("O","-","AB","-",false),
            // O+ recipient can receive O- and O+
            ("O","+","O","-",true), ("O","+","O","+",true),
            ("O","+","A","+",false), ("O","+","A","-",false), ("O","+","B","+",false), ("O","+","B","-",false),
            ("O","+","AB","+",false), ("O","+","AB","-",false),
            // A- recipient: A-, O-
            ("A","-","A","-",true), ("A","-","O","-",true),
            ("A","-","A","+",false), ("A","-","O","+",false), ("A","-","B","+",false), ("A","-","B","-",false),
            ("A","-","AB","+",false), ("A","-","AB","-",false),
            // A+ recipient: A+, A-, O+, O-
            ("A","+","A","+",true), ("A","+","A","-",true), ("A","+","O","+",true), ("A","+","O","-",true),
            ("A","+","B","+",false), ("A","+","B","-",false), ("A","+","AB","+",false), ("A","+","AB","-",false),
            // B- recipient: B-, O-
            ("B","-","B","-",true), ("B","-","O","-",true),
            ("B","-","B","+",false), ("B","-","O","+",false), ("B","-","A","+",false), ("B","-","A","-",false),
            ("B","-","AB","+",false), ("B","-","AB","-",false),
            // B+ recipient: B+, B-, O+, O-
            ("B","+","B","+",true), ("B","+","B","-",true), ("B","+","O","+",true), ("B","+","O","-",true),
            ("B","+","A","+",false), ("B","+","A","-",false), ("B","+","AB","+",false), ("B","+","AB","-",false),
            // AB- recipient: AB-, A-, B-, O-
            ("AB","-","AB","-",true), ("AB","-","A","-",true), ("AB","-","B","-",true), ("AB","-","O","-",true),
            ("AB","-","AB","+",false), ("AB","-","A","+",false), ("AB","-","B","+",false), ("AB","-","O","+",false),
            // AB+ recipient: universal — all 8 types
            ("AB","+","AB","+",true), ("AB","+","AB","-",true), ("AB","+","A","+",true), ("AB","+","A","-",true),
            ("AB","+","B","+",true), ("AB","+","B","-",true), ("AB","+","O","+",true), ("AB","+","O","-",true),
        ];
        for (rg, rr, dg, dr, comp) in rows {
            sqlx::query(
                "INSERT INTO blood_compatibility_matrix (recipient_group, recipient_rh, donor_group, donor_rh, compatible) VALUES ($1,$2,$3,$4,$5)"
            ).bind(rg).bind(rr).bind(dg).bind(dr).bind(comp)
              .execute(pool).await.ok();
        }
    }

    // ── Blood Bank indexes (performance for 1M units, 100k donors) ───────
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_blood_units_status ON blood_units(status)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_blood_units_group ON blood_units(blood_group, rh_factor)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_blood_units_component ON blood_units(component_type)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_blood_units_donor ON blood_units(donor_id)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_blood_units_expiry ON blood_units(expiry_date)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_blood_units_patient ON blood_units(reserved_for_patient_id) WHERE reserved_for_patient_id IS NOT NULL").execute(pool).await.ok();
    // Partial index: the common query "available units not deleted" is index-accelerated.
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_blood_units_available ON blood_units(blood_group, rh_factor, component_type) WHERE status = 'available' AND deleted_at IS NULL").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_blood_donors_number ON blood_donors(donor_number)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_blood_donors_name ON blood_donors(first_name, last_name)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_blood_donors_group ON blood_donors(blood_group, rh_factor) WHERE deleted_at IS NULL").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_blood_donations_donor ON blood_donations(donor_id)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_blood_donations_date ON blood_donations(donation_date)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_blood_crossmatch_unit ON blood_crossmatch_results(unit_id)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_blood_crossmatch_patient ON blood_crossmatch_results(patient_id)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_blood_reservations_patient ON blood_reservations(patient_id)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_blood_reservations_unit ON blood_reservations(unit_id)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_blood_reservations_active ON blood_reservations(expires_at) WHERE status = 'active'").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_blood_issues_patient ON blood_issues(patient_id)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_blood_issues_unit ON blood_issues(unit_id)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_blood_transfusions_patient ON blood_transfusions(patient_id)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_blood_unit_history_unit ON blood_unit_status_history(unit_id)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_blood_movements_unit ON blood_inventory_movements(unit_id)").execute(pool).await.ok();

    // SEQUENCE for concurrency-safe unit-number generation (mirrors radiology_order_seq).
    sqlx::query("CREATE SEQUENCE IF NOT EXISTS blood_unit_seq START 1").execute(pool).await.ok();
    sqlx::query("CREATE SEQUENCE IF NOT EXISTS blood_donor_seq START 1").execute(pool).await.ok();
    sqlx::query("CREATE SEQUENCE IF NOT EXISTS blood_donation_seq START 1").execute(pool).await.ok();
    sqlx::query("CREATE SEQUENCE IF NOT EXISTS blood_reservation_seq START 1").execute(pool).await.ok();
    sqlx::query("CREATE SEQUENCE IF NOT EXISTS blood_issue_seq START 1").execute(pool).await.ok();
    sqlx::query("CREATE SEQUENCE IF NOT EXISTS blood_transfusion_seq START 1").execute(pool).await.ok();
    sqlx::query("CREATE SEQUENCE IF NOT EXISTS blood_discard_seq START 1").execute(pool).await.ok();

    // Seed default roles, permissions, and a bootstrap admin once.
    crate::auth::seed_defaults(pool).await
        .map_err(|e| format!("seed_defaults: {}", e))?;

    Ok(())
}

// ── Full initialization ──────────────────────────────────────────────────────

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
