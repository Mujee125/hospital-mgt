//! In-app notification center (Phase 9) — the titlebar bell made real.
//!
//! Emit side: `emit()` is a tiny INSERT called from existing command paths
//! when operationally relevant things happen (critical lab value entered,
//! result released, appointment booked, claim settled, stock below reorder,
//! nightly backup failed). Emits are best-effort by design — a failing
//! notification must never fail the clinical/financial action that produced
//! it; the error is logged and the action proceeds.
//!
//! Read side: three commands, all session-gated (the feed is per-user —
//! every signed-in role can read ITS OWN notifications; no permission
//! variant needed because there is nothing to authorize beyond identity).
//!
//! Targeting model:
//!   • user_id set          → that user only
//!   • role_target set      → broadcast to every user holding that role
//!   • both NULL            → broadcast to everyone
//! Read state lives in `app_notification_reads` (per-user) so a role
//! broadcast is ONE row but each recipient marks it read independently.

use serde::Serialize;
use sqlx::PgPool;

use crate::rbac::{self, SessionState};

// ── Models ───────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AppNotification {
    pub id: i32,
    /// Kept for the FromRow column mapping + tests; not serialized — the
    /// frontend only needs to know it can SEE the notification, not how it
    /// was targeted.
    #[serde(skip_serializing)]
    #[allow(dead_code)]
    pub user_id: Option<i32>,
    #[serde(skip_serializing)]
    #[allow(dead_code)]
    pub role_target: Option<String>,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<i32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Per-reading-user read state (EXISTS against app_notification_reads).
    pub read: bool,
}

#[derive(Debug, Serialize)]
pub struct AppNotificationFeed {
    pub notifications: Vec<AppNotification>,
    pub unread_count: i64,
}

// ── Emit side (internal helper — NOT a Tauri command) ───────────────────────

/// Insert one notification. Best-effort: errors are returned but every
/// caller treats them as non-fatal (log + continue) — the producing action
/// must never roll back because the center could not be pinged.
pub async fn emit(pool: &PgPool, n: NotificationOut) -> Result<(), String> {
    sqlx::query(
        r#"INSERT INTO app_notifications
               (user_id, role_target, kind, title, body, entity_type, entity_id)
           VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
    )
    .bind(n.user_id)
    .bind(&n.role_target)
    .bind(&n.kind)
    .bind(&n.title)
    .bind(&n.body)
    .bind(&n.entity_type)
    .bind(n.entity_id)
    .execute(pool)
    .await
    .map(|_| ())
    .map_err(|e| format!("notification emit failed: {}", e))
}

/// What to emit — the fields of a notification minus the DB-generated ones.
#[derive(Debug)]
pub struct NotificationOut {
    /// Direct user target, OR:
    pub user_id: Option<i32>,
    /// role broadcast (every user holding the role sees it), OR:
    pub role_target: Option<String>,
    /// both None = broadcast to everyone.
    pub kind: String,
    pub title: String,
    pub body: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<i32>,
}

// ── Read side (Tauri commands) ───────────────────────────────────────────────

/// The visibility filter shared by the feed and the unread count: a
/// notification is visible to a user when it targets them directly, targets
/// a role they hold, or broadcasts to everyone. $2 is cast because sqlx
/// binds `&[String]` as an untyped parameter — without the explicit
/// `text[]` cast Postgres cannot infer an array type for `= ANY($2)`.
const VISIBILITY: &str = r#"
    (n.user_id = $1
     OR n.role_target IS NOT NULL AND n.role_target = ANY($2::text[])
     OR (n.user_id IS NULL AND n.role_target IS NULL))
"#;

#[tauri::command]
pub async fn get_app_notifications(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
) -> Result<AppNotificationFeed, String> {
    let s = rbac::require_session(&session)?;
    fetch_feed(pool.inner(), s.user_id, &s.roles).await
}

/// Feed core (AERP Part G extraction pattern) — callable without a Tauri
/// AppHandle for the integration tests.
pub async fn fetch_feed(
    pool: &PgPool,
    user_id: i32,
    roles: &[String],
) -> Result<AppNotificationFeed, String> {
    let notifications: Vec<AppNotification> = sqlx::query_as(&format!(
        r#"SELECT n.id, n.user_id, n.role_target, n.kind, n.title, n.body,
                  n.entity_type, n.entity_id, n.created_at,
                  EXISTS (SELECT 1 FROM app_notification_reads r
                          WHERE r.notification_id = n.id AND r.user_id = $1) AS read
           FROM app_notifications n
           WHERE {}
           ORDER BY n.created_at DESC
           LIMIT 50"#,
        VISIBILITY
    ))
    .bind(user_id)
    .bind(roles)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Get notifications: {}", e))?;

    let (unread_count,): (i64,) = sqlx::query_as(&format!(
        r#"SELECT COUNT(*)::bigint FROM app_notifications n
           WHERE {}
             AND NOT EXISTS (SELECT 1 FROM app_notification_reads r
                             WHERE r.notification_id = n.id AND r.user_id = $1)"#,
        VISIBILITY
    ))
    .bind(user_id)
    .bind(roles)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Count unread: {}", e))?;

    Ok(AppNotificationFeed {
        notifications,
        unread_count,
    })
}

#[tauri::command]
pub async fn mark_notification_read(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    notification_id: i32,
) -> Result<(), String> {
    let s = rbac::require_session(&session)?;
    mark_read_core(pool.inner(), s.user_id, &s.roles, notification_id).await
}

/// Mark-read core — visibility-scoped (a user can only mark notifications
/// they can actually see; prevents cross-user tampering by id).
///
/// Placeholders follow the VISIBILITY contract ($1 = user_id, $2 = roles);
/// the notification id is $3. Binding (user_id, roles, id) in that order —
/// NOT the order the ids appear in the SELECT — is what keeps the shared
/// fragment correct.
pub async fn mark_read_core(
    pool: &PgPool,
    user_id: i32,
    roles: &[String],
    notification_id: i32,
) -> Result<(), String> {
    sqlx::query(&format!(
        r#"INSERT INTO app_notification_reads (notification_id, user_id)
           SELECT $3, $1 FROM app_notifications n
           WHERE n.id = $3 AND {}
           ON CONFLICT DO NOTHING"#,
        VISIBILITY
    ))
    .bind(user_id)
    .bind(roles)
    .bind(notification_id)
    .execute(pool)
    .await
    .map_err(|e| format!("Mark read: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn mark_all_notifications_read(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
) -> Result<u64, String> {
    let s = rbac::require_session(&session)?;
    mark_all_read_core(pool.inner(), s.user_id, &s.roles).await
}

/// Mark-all core — returns how many notifications were marked.
pub async fn mark_all_read_core(
    pool: &PgPool,
    user_id: i32,
    roles: &[String],
) -> Result<u64, String> {
    let result = sqlx::query(&format!(
        r#"INSERT INTO app_notification_reads (notification_id, user_id)
           SELECT n.id, $1 FROM app_notifications n
           WHERE {}
             AND NOT EXISTS (SELECT 1 FROM app_notification_reads r
                             WHERE r.notification_id = n.id AND r.user_id = $1)
           ON CONFLICT DO NOTHING"#,
        VISIBILITY
    ))
    .bind(user_id)
    .bind(roles)
    .execute(pool)
    .await
    .map_err(|e| format!("Mark all read: {}", e))?;
    Ok(result.rows_affected())
}
