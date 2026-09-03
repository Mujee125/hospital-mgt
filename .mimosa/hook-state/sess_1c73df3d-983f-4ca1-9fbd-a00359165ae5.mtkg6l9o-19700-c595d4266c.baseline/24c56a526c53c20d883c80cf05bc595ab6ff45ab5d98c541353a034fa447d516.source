/// service.rs — DEAD CODE, not declared as a module in lib.rs.
/// Superseded by pg_provision.rs, which installs PostgreSQL as a proper
/// Windows Service from bundled binaries. Safe to delete this file.
use std::process::Command;

pub fn start_postgres_service() -> Result<String, String> {
    let names = ["postgresql-x64-17","postgresql-x64-16","postgresql-x64-15","postgresql-x64-14","postgresql"];
    for name in &names {
        let out = Command::new("sc").args(["query", name]).output();
        if let Ok(o) = out {
            let s = String::from_utf8_lossy(&o.stdout);
            if s.contains("RUNNING") { return Ok(format!("Service '{}' already running.", name)); }
            if s.contains("STOPPED") || s.contains("SERVICE_NAME") {
                let _ = Command::new("sc").args(["start", name]).output();
                std::thread::sleep(std::time::Duration::from_secs(3));
                return Ok(format!("Service '{}' started.", name));
            }
        }
    }
    Err("No PostgreSQL service found.".to_string())
}

#[tauri::command]
pub async fn start_postgres() -> Result<String, String> { start_postgres_service() }

#[tauri::command]
pub async fn check_postgres_status() -> Result<String, String> {
    let names = ["postgresql-x64-17","postgresql-x64-16","postgresql-x64-15","postgresql-x64-14","postgresql"];
    for name in &names {
        let out = Command::new("sc").args(["query", name]).output();
        if let Ok(o) = out {
            let s = String::from_utf8_lossy(&o.stdout);
            if s.contains("RUNNING") { return Ok(format!("running:{}", name)); }
            if s.contains("STOPPED") { return Ok(format!("stopped:{}", name)); }
        }
    }
    Ok("not_found".to_string())
}
