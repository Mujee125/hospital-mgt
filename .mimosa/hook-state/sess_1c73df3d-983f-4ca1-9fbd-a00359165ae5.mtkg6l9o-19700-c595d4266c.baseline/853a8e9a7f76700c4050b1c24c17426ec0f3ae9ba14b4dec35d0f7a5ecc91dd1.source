//! Audit logging (ISO/IEC 27001 A.12.4 — logging and monitoring).
//!
//! Every state-changing command writes a single audit row via `record(...)`.
//! Read commands are intentionally NOT audited at row level (volume would be
//! untenable and would itself leak PHI access patterns); instead, login/
//! logout and explicit PHI exports are the auditable events. This matches a
//! proportionate reading of A.12.4 for a single-hospital desktop system.

use chrono::Utc;
use sqlx::PgPool;

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct AuditLog {
    pub id: i64,
    pub user_id: Option<i32>,
    pub username: Option<String>,
    pub action: String,
    pub resource: String,
    pub resource_id: Option<String>,
    pub details: Option<serde_json::Value>,
    pub ip: Option<String>,
    pub created_at: chrono::DateTime<Utc>,
}

/// Insert an audit entry. All arguments except `action` and `resource` are
/// optional. Failures are swallowed (logged to stderr) so a logging fault can
/// never block a clinical operation — availability over completeness.
pub async fn record(
    pool: &PgPool,
    user_id: Option<i32>,
    username: Option<&str>,
    action: &str,
    resource: &str,
    resource_id: Option<&str>,
    details: Option<serde_json::Value>,
) -> Result<(), String> {
    sqlx::query(
        r#"INSERT INTO audit_logs
              (user_id, username, action, resource, resource_id, details)
           VALUES ($1, $2, $3, $4, $5, $6)"#,
    )
    .bind(user_id)
    .bind(username)
    .bind(action)
    .bind(resource)
    .bind(resource_id)
    .bind(details)
    .execute(pool)
    .await
    .map(|_| ())
    .map_err(|e| format!("audit insert failed: {}", e))
}

/// Convenience wrapper used by command modules that already hold a `Session`.
pub async fn for_session(
    pool: &PgPool,
    session: &crate::rbac::Session,
    action: &str,
    resource: &str,
    resource_id: Option<&str>,
    details: Option<serde_json::Value>,
) {
    if let Err(e) = record(
        pool,
        Some(session.user_id),
        Some(&session.username),
        action,
        resource,
        resource_id,
        details,
    )
    .await
    {
        eprintln!("[HMS AUDIT] {}", e);
    }
}

// ── Tauri command ─────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_audit_logs(
    pool: tauri::State<'_, PgPool>,
    session_state: tauri::State<'_, std::sync::Arc<std::sync::Mutex<Option<crate::rbac::Session>>>>,
    limit: Option<i64>,
    action_filter: Option<String>,
    resource_filter: Option<String>,
) -> Result<Vec<AuditLog>, String> {
    let _ = crate::rbac::require(&session_state, crate::rbac::Permission::AuditView)?;
    let limit = limit.unwrap_or(500).clamp(1, 5000);

    let rows: Vec<AuditLog> = sqlx::query_as(
        r#"SELECT id, user_id, username, action, resource, resource_id, details, ip, created_at
           FROM audit_logs
           WHERE ($1::text IS NULL OR action = $1)
             AND ($2::text IS NULL OR resource = $2)
           ORDER BY created_at DESC
           LIMIT $3"#,
    )
    .bind(action_filter.as_deref())
    .bind(resource_filter.as_deref())
    .bind(limit)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| format!("Fetch audit logs: {}", e))?;

    Ok(rows)
}
