//! Staff messaging — RBAC-guarded and audited.
//!
//! Per SRS NFR-15 / Security Matrix A.5.15 / A.8.16 (CR-16):
//!   - `send_message`    requires `MessagingSend`; sender is derived from the
//!     authenticated session (NOT a free client-supplied
//!     string — closes the authenticity / impersonation gap).
//!   - `get_messages`    requires `MessagingView`.
//!   - `delete_message`  requires `MessagingSend` (treat as a state change).
//!   - `get_rooms`       requires `MessagingView`.
//!
//! Every state-changing call writes an audit row.

use sqlx::PgPool;
use tauri::Emitter;
use uuid::Uuid;

use crate::audit;
use crate::models::{ChatMessage, SendMessage};
use crate::rbac::{self, Permission, SessionState};

#[tauri::command]
pub async fn send_message(
    app_handle: tauri::AppHandle,
    pool: tauri::State<'_, PgPool>,
    session_state: tauri::State<'_, SessionState>,
    message: SendMessage,
) -> Result<ChatMessage, String> {
    let s = rbac::require(&session_state, Permission::MessagingSend)?;

    if message.content.trim().is_empty() {
        return Err("Message content cannot be empty.".to_string());
    }

    // Sender is derived from the authenticated session — clients can no
    // longer impersonate another staff member by passing an arbitrary name.
    let sender = s.full_name.clone();
    if sender.trim().is_empty() {
        return Err("Sender name cannot be empty.".to_string());
    }

    let msg: ChatMessage = sqlx::query_as(
        r#"
        INSERT INTO messages (sender, content, room)
        VALUES ($1, $2, $3)
        RETURNING id, sender, content, room, created_at
        "#,
    )
    .bind(&sender)
    .bind(&message.content)
    .bind(&message.room)
    .fetch_one(pool.inner())
    .await
    .map_err(|e| format!("Failed to save message: {}", e))?;

    // Audit the send (per Matrix A.8.16 — every state-changing command is audited).
    audit::for_session(
        pool.inner(),
        &s,
        "message_send",
        "messages",
        Some(&msg.id.to_string()),
        Some(serde_json::json!({"room": message.room})),
    )
    .await;

    // Broadcast to all windows for real-time updates.
    app_handle.emit("new_message", &msg).ok();
    Ok(msg)
}

#[tauri::command]
pub async fn get_messages(
    pool: tauri::State<'_, PgPool>,
    session_state: tauri::State<'_, SessionState>,
    room: String,
    limit: Option<i64>,
) -> Result<Vec<ChatMessage>, String> {
    let _ = rbac::require(&session_state, Permission::MessagingView)?;
    let limit = limit.unwrap_or(100).min(500);

    sqlx::query_as(
        r#"
        SELECT id, sender, content, room, created_at FROM (
            SELECT * FROM messages WHERE room = $1
            ORDER BY created_at DESC LIMIT $2
        ) sub ORDER BY created_at ASC
        "#,
    )
    .bind(&room)
    .bind(limit)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| format!("Failed to fetch messages: {}", e))
}

#[tauri::command]
pub async fn delete_message(
    pool: tauri::State<'_, PgPool>,
    session_state: tauri::State<'_, SessionState>,
    id: String,
) -> Result<(), String> {
    let s = rbac::require(&session_state, Permission::MessagingSend)?;
    let uuid = Uuid::parse_str(&id).map_err(|_| "Invalid message ID".to_string())?;
    sqlx::query("DELETE FROM messages WHERE id = $1")
        .bind(uuid)
        .execute(pool.inner())
        .await
        .map_err(|e| format!("Delete failed: {}", e))?;

    audit::for_session(
        pool.inner(),
        &s,
        "message_delete",
        "messages",
        Some(&id),
        None,
    )
    .await;
    Ok(())
}

#[tauri::command]
pub async fn get_rooms(
    pool: tauri::State<'_, PgPool>,
    session_state: tauri::State<'_, SessionState>,
) -> Result<Vec<String>, String> {
    let _ = rbac::require(&session_state, Permission::MessagingView)?;
    let db_rooms: Vec<String> =
        sqlx::query_scalar("SELECT DISTINCT room FROM messages ORDER BY room")
            .fetch_all(pool.inner())
            .await
            .unwrap_or_default();

    let mut rooms = vec![
        "general".to_string(),
        "doctors".to_string(),
        "admin".to_string(),
    ];
    for r in db_rooms {
        if !rooms.contains(&r) {
            rooms.push(r);
        }
    }
    Ok(rooms)
}
