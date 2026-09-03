/// Persistence for the WhatsApp notification audit log
/// (`whatsapp_notifications` table) — both writing a record after every
/// send attempt and reading them back for the Settings page's log view.
use sqlx::PgPool;

use super::WhatsAppMessage;

pub async fn log_notification(pool: &PgPool, msg: &WhatsAppMessage, success: bool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO whatsapp_notifications
            (appointment_id, notification_type, recipient, message, success)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(msg.appointment_id)
    .bind(&msg.notification_type)
    .bind(&msg.recipient)
    .bind(&msg.message)
    .bind(success)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn fetch_notification_log(pool: &PgPool, limit: i64) -> Result<Vec<serde_json::Value>, String> {
    let rows = sqlx::query_as::<_, (i32, Option<i32>, String, String, String, chrono::DateTime<chrono::Utc>, bool)>(
        r#"
        SELECT id, appointment_id, notification_type, recipient, message, sent_at, success
        FROM whatsapp_notifications
        ORDER BY sent_at DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to fetch notification log: {}", e))?;

    Ok(rows
        .into_iter()
        .map(|(id, appt_id, ntype, recipient, message, sent_at, success)| {
            serde_json::json!({
                "id": id,
                "appointment_id": appt_id,
                "notification_type": ntype,
                "recipient": recipient,
                "message": message,
                "sent_at": sent_at,
                "success": success,
            })
        })
        .collect())
}
