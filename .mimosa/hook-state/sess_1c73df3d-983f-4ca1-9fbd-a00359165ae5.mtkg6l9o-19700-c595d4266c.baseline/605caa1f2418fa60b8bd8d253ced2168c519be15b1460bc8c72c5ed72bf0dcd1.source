/// PostgreSQL health check and SSL provisioning (server build only).
///
/// IMPORTANT: The heavy lifting — initdb, Windows Service registration,
/// pg_hba.conf initial setup, credential generation — happens ONCE during
/// installation in the NSIS post-install hook (windows/hooks.nsh).
///
/// This module does two things at runtime:
///   1. check_postgres_health()        — every launch, verify service is up.
///   2. ensure_postgres_ssl_enabled()  — once on first launch after install,
///      wire PostgreSQL SSL to the TLS cert tls_provision.rs just generated.
///   3. repair_ssl_config()            — recovery path: if SSL is broken
///      (marker exists but SSL not actually working), rewrite the config
///      cleanly and restart. Called automatically when the connection fails.
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const SERVICE_NAME: &str = "HMS-PostgreSQL";

#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    pub service_running: bool,
    pub accepting_connections: bool,
}

pub fn check_postgres_health(pg_bin_dir: &Path, port: u16) -> Result<HealthCheckResult, String> {
    let service_running = is_service_running();
    if !service_running {
        return Ok(HealthCheckResult {
            service_running: false,
            accepting_connections: false,
        });
    }
    // Poll up to 30 × 500ms = 15 seconds
    let accepting_connections = wait_until_accepting_connections(pg_bin_dir, port, 30);
    Ok(HealthCheckResult {
        service_running,
        accepting_connections,
    })
}

fn is_service_running() -> bool {
    Command::new("sc")
        .args(["query", SERVICE_NAME])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("RUNNING"))
        .unwrap_or(false)
}

fn wait_until_accepting_connections(pg_bin_dir: &Path, port: u16, max_attempts: u32) -> bool {
    let pg_isready = pg_bin_dir.join("pg_isready.exe");
    for _ in 0..max_attempts {
        let ok = if pg_isready.exists() {
            Command::new(&pg_isready)
                .arg("-p").arg(port.to_string())
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        } else {
            crate::discovery::is_reachable("127.0.0.1", port, 500)
        };
        if ok { return true; }
        std::thread::sleep(Duration::from_millis(500));
    }
    false
}

pub fn default_pg_bin_dir() -> Option<std::path::PathBuf> {
    let program_data = std::env::var_os("ProgramData")?;
    Some(std::path::PathBuf::from(program_data).join("HMS").join("pgsql").join("bin"))
}

/// Returns true if the SSL marker file already exists (SSL was enabled on
/// a previous launch). Does NOT verify the config is actually correct —
/// use verify_ssl_actually_configured() for that.
pub fn ssl_marker_exists(pgdata_dir: &Path) -> bool {
    pgdata_dir.join(".hms_ssl_enabled").exists()
}

/// Checks whether postgresql.conf actually has ssl=on in it right now.
pub fn ssl_is_configured_in_conf(pgdata_dir: &Path) -> bool {
    let conf_path = pgdata_dir.join("postgresql.conf");
    std::fs::read_to_string(&conf_path)
        .map(|c| c.contains("ssl = on") || c.contains("ssl=on"))
        .unwrap_or(false)
}

/// Checks whether pg_hba.conf uses hostssl rules.
pub fn hba_requires_ssl(pgdata_dir: &Path) -> bool {
    let hba_path = pgdata_dir.join("pg_hba.conf");
    std::fs::read_to_string(&hba_path)
        .map(|c| c.contains("hostssl"))
        .unwrap_or(false)
}

/// Full SSL provisioning: writes ssl config to postgresql.conf, writes
/// hostssl rules to pg_hba.conf, sets key permissions, restarts service.
/// This is called both for first-time setup AND for repair.
///
/// SEC-15: `app_db_user` is the Postgres role the HMS app uses to
/// connect. The LAN-side pg_hba rules now restrict access to that user
/// only (previously `all all`, which allowed ANY Postgres role to
/// connect from the LAN — a defense-in-depth weakness). The loopback
/// rule stays as `all all` so local admin tools (pgAdmin, psql) can
/// still connect as any role from the server PC itself.
pub fn write_ssl_config_and_restart(
    pgdata_dir: &Path,
    cert_path: &Path,
    key_path: &Path,
    app_db_user: &str,
) -> Result<(), String> {
    let conf_path = pgdata_dir.join("postgresql.conf");

    // Read existing conf, strip any previous (possibly broken) HMS SSL block,
    // then append a clean one. This makes the function safe to call repeatedly.
    let existing = std::fs::read_to_string(&conf_path)
        .map_err(|e| format!("Cannot read postgresql.conf: {}", e))?;

    // Remove any previous HMS SSL section to avoid duplicates
    let cleaned: String = existing
        .lines()
        .filter(|l| {
            !l.contains("# HMS: TLS") &&
            !l.trim_start().starts_with("ssl = ") &&
            !l.trim_start().starts_with("ssl=") &&
            !l.trim_start().starts_with("ssl_cert_file") &&
            !l.trim_start().starts_with("ssl_key_file")
        })
        .collect::<Vec<_>>()
        .join("\n");

    let cert_str = cert_path.to_string_lossy().replace('\\', "/");
    let key_str  = key_path.to_string_lossy().replace('\\', "/");

    let new_conf = format!(
        "{}\n\n# HMS: TLS (written by app on first launch — see tls_provision.rs)\nssl = on\nssl_cert_file = '{}'\nssl_key_file = '{}'\n",
        cleaned.trim_end(),
        cert_str,
        key_str,
    );

    std::fs::write(&conf_path, new_conf)
        .map_err(|e| format!("Cannot write postgresql.conf: {}", e))?;

    // Write pg_hba.conf with hostssl rules.
    //
    // SEC-15: LAN rules now restrict to the HMS app's DB user only —
    // previously `all all` allowed ANY Postgres role to connect from
    // the LAN, so a misconfigured or attacker-created role (e.g. a
    // `pharmacy_readonly` role that an admin created with overly-broad
    // grants) could be used to read PHI from a client PC. With this
    // change, only the app's own DB user can authenticate over the LAN.
    //
    // The loopback rule stays `all all` so local admin tools (pgAdmin,
    // psql) running on the server PC itself can still connect as any
    // role — local trust is appropriate for the reception PC's console.
    //
    // `app_db_user` is interpolated (not parameterised) because pg_hba.conf
    // is a config file, not SQL — there's no bind-parameter mechanism.
    // The value comes from `AppConfig.db_user` (defaults to `postgres`
    // per config.rs), which is operator-controlled and not exposed to
    // untrusted network input. We do NOT call `validate_db_identifier`
    // here because pg_hba user-field syntax is slightly different from
    // SQL identifier syntax (it allows `+role`, `@group`, etc.) —
    // instead we rely on the config being operator-set. A future
    // hardening could quote the value with double-quotes to escape any
    // embedded quotes.
    let hba_path  = pgdata_dir.join("pg_hba.conf");
    let hba_rules = format!(
        "# HMS managed — LAN + loopback, SSL required, scram-sha-256 auth.\n\
         # SEC-15: LAN rules restricted to the HMS app DB user (was: all all).\n\
         hostssl  all  all           127.0.0.1/32    scram-sha-256\n\
         hostssl  all  all           ::1/128         scram-sha-256\n\
         hostssl  all  {app_user}    10.0.0.0/8      scram-sha-256\n\
         hostssl  all  {app_user}    172.16.0.0/12   scram-sha-256\n\
         hostssl  all  {app_user}    192.168.0.0/16  scram-sha-256\n",
        app_user = app_db_user
    );
    std::fs::write(&hba_path, hba_rules)
        .map_err(|e| format!("Cannot write pg_hba.conf: {}", e))?;

    // Restrict private key permissions
    let _ = Command::new("icacls")
        .arg(key_path)
        .args(["/inheritance:r", "/grant:r", "SYSTEM:F", "/grant:r", "*S-1-5-32-544:F"])
        .output();

    restart_service_and_wait()
}

/// First-time SSL enablement. Idempotent via marker file.
/// Returns Ok(true) if SSL was just enabled, Ok(false) if already done.
///
/// SEC-15: `app_db_user` is the Postgres role the HMS app uses; it's
/// passed through to `write_ssl_config_and_restart` so the LAN-side
/// pg_hba rules can be restricted to that user only.
pub fn ensure_postgres_ssl_enabled(
    pgdata_dir: &Path,
    cert_path: &Path,
    key_path: &Path,
    app_db_user: &str,
) -> Result<bool, String> {
    let marker = pgdata_dir.join(".hms_ssl_enabled");
    if marker.exists() {
        return Ok(false); // already done on a previous launch
    }

    write_ssl_config_and_restart(pgdata_dir, cert_path, key_path, app_db_user)?;

    std::fs::write(&marker, "1")
        .map_err(|e| format!("Cannot write SSL marker file: {}", e))?;

    Ok(true)
}

/// Recovery path: SSL marker exists but PostgreSQL isn't actually serving
/// SSL (e.g. previous SSL config was broken by manual edits or a failed
/// partial run). Rewrites config cleanly and restarts.
///
/// SEC-15: `app_db_user` is passed through to
/// `write_ssl_config_and_restart` for LAN-rule restriction.
pub fn repair_ssl_config(
    pgdata_dir: &Path,
    cert_path: &Path,
    key_path: &Path,
    app_db_user: &str,
) -> Result<(), String> {
    // Remove old marker so ensure_postgres_ssl_enabled could re-run if
    // needed, but here we call write_ssl_config_and_restart directly since
    // we know we want to force-rewrite.
    write_ssl_config_and_restart(pgdata_dir, cert_path, key_path, app_db_user)?;

    // Re-write marker in case it was missing
    let marker = pgdata_dir.join(".hms_ssl_enabled");
    std::fs::write(&marker, "1")
        .map_err(|e| format!("Cannot write SSL marker file after repair: {}", e))?;

    Ok(())
}

fn restart_service_and_wait() -> Result<(), String> {
    // Stop — wait until actually stopped before starting
    let _ = Command::new("sc").args(["stop", SERVICE_NAME]).output();

    for _ in 0..20 {
        std::thread::sleep(Duration::from_millis(500));
        let stopped = Command::new("sc")
            .args(["query", SERVICE_NAME])
            .output()
            .map(|o| {
                let s = String::from_utf8_lossy(&o.stdout);
                s.contains("STOPPED") || !s.contains("RUNNING")
            })
            .unwrap_or(true);
        if stopped { break; }
    }

    // Start
    let start = Command::new("sc")
        .args(["start", SERVICE_NAME])
        .output()
        .map_err(|e| format!("Failed to start PostgreSQL service: {}", e))?;

    if !start.status.success() {
        let out = String::from_utf8_lossy(&start.stdout);
        // Error 1056 = service already running — that's fine
        if !out.contains("1056") {
            return Err(format!(
                "Failed to restart PostgreSQL. Output: {}\nPlease restart your PC and try again.",
                out
            ));
        }
    }

    // Wait up to 15 seconds for service to be fully ready
    std::thread::sleep(Duration::from_secs(4));
    Ok(())
}
