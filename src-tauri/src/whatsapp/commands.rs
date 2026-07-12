//! Tauri command entry points for WhatsApp messaging.
//!
//! Commands:
//! - `send_whatsapp_notification` — generic send (used by appointment flows)
//! - `send_whatsapp_to_patient`   — send to a phone number with auto-normalization
//! - `send_whatsapp_test`         — quick connectivity test from Settings
//! - `get_notification_log`       — audit log of past sends
//! - `get_whatsapp_config`        — read Business API config (token masked)
//! - `set_whatsapp_config`        — save Business API credentials
//! - `test_whatsapp_api`          — test Business API connectivity
use sqlx::PgPool;

use super::{automation, log, WhatsAppMessage};
use crate::rbac::{self, SessionState};

/// Generic WhatsApp send. Accepts a fully-formed `WhatsAppMessage`.
/// Used by the appointment booking/confirmation/cancellation flows.
#[tauri::command]
pub async fn send_whatsapp_notification(
    app_handle: tauri::AppHandle,
    pool: tauri::State<'_, PgPool>,
    session_state: tauri::State<'_, SessionState>,
    message: WhatsAppMessage,
) -> Result<(), String> {
    let session = rbac::require_session(&session_state)?;
    let result = automation::send_whatsapp(&app_handle, pool.inner(), message.clone()).await;
    crate::audit::for_session(
        pool.inner(), &session, "whatsapp_send", "whatsapp",
        None,
        Some(serde_json::json!({
            "recipient": message.recipient,
            "notification_type": message.notification_type,
            "is_group": message.is_group,
            "success": result.is_ok()
        })),
    ).await;
    result
}

/// Send a WhatsApp message to a specific phone number.
///
/// IPC-09: hardens the `send_whatsapp_to_patient` entry point against
/// abuse. Previously this command accepted ANY phone number and ANY
/// message text from any authenticated user — an abuse vector for both
/// spam (a receptionist could send arbitrary text to any number) and
/// PHI exfiltration (a compromised account could text patient details
/// to an attacker-controlled number). Two new guards:
///   1. The recipient phone MUST belong to a registered, non-deleted
///      patient. The function is named `send_whatsapp_to_patient` —
///      sends to non-patient numbers (e.g. an arbitrary external number)
///      are refused. (The consent gate in `automation::send_whatsapp`
///      still applies on top — the patient must have an explicit
///      `whatsapp` consent row with `granted = true`.)
///   2. The message body is capped at MAX_MESSAGE_LEN characters to
///      match WhatsApp's own limits and to prevent URL-overflow
///      truncation in the wa.me deep-link fallback path.
#[tauri::command]
pub async fn send_whatsapp_to_patient(
    app_handle: tauri::AppHandle,
    pool: tauri::State<'_, PgPool>,
    session_state: tauri::State<'_, SessionState>,
    phone: String,
    message: String,
    notification_type: Option<String>,
) -> Result<(), String> {
    let session = rbac::require_session(&session_state)?;

    // IPC-09 (1): the recipient must be a registered patient. Reuse the
    // same normalization + last-9-digit suffix matching that the consent
    // gate uses (see `automation::check_patient_consent`) so the two
    // lookups can't disagree on what "the same patient" means.
    //
    // The CR-12 consent gate inside `automation::send_whatsapp` ALLOWS
    // sends to non-patient numbers (its comment: "the recipient is not
    // a patient, so HIPAA consent does not apply — e.g. a doctor's
    // phone used for a manual ad-hoc send"). That's correct for the
    // generic `send_whatsapp_notification` entry point, but
    // `send_whatsapp_to_patient` is the patient-facing API and must
    // refuse non-patient recipients entirely.
    let clinic_default_cc: Option<String> = std::env::var("HMS_DEFAULT_CC").ok();
    let normalized = automation::normalize_phone(&phone, clinic_default_cc.as_deref())?;
    let digits: String = normalized.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 9 {
        return Err("Phone number does not belong to a registered patient.".to_string());
    }
    let suffix_len = digits.len().min(9);
    let suffix = &digits[digits.len() - suffix_len..];
    let pattern = format!("%{}", suffix);

    // CR-11: only consider ACTIVE patients (deleted_at IS NULL). A
    // soft-deleted patient's stale phone record must NOT be usable as
    // a send target.
    let patient: Option<(i32,)> = sqlx::query_as(
        "SELECT id FROM patients WHERE phone LIKE $1 AND deleted_at IS NULL ORDER BY id DESC LIMIT 1",
    )
    .bind(&pattern)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    if patient.is_none() {
        return Err("Phone number does not belong to a registered patient.".to_string());
    }

    // IPC-09 (2): enforce a max message length. WhatsApp's Business API
    // caps text messages at 4096 chars, but the wa.me deep-link fallback
    // encodes the message into the URL and browsers cap URL length at
    // ~2000-8000 chars depending on the platform — longer messages get
    // silently truncated, which can drop critical clinical content
    // (e.g. appointment times, prep instructions). 1000 chars is a safe
    // ceiling that fits any reasonable clinical notification while
    // keeping the deep-link URL well under browser limits.
    const MAX_MESSAGE_LEN: usize = 1000;
    let message_len = message.chars().count();
    if message_len > MAX_MESSAGE_LEN {
        return Err(format!(
            "Message is too long ({} characters). The maximum is {}.",
            message_len, MAX_MESSAGE_LEN
        ));
    }

    let msg = WhatsAppMessage {
        recipient: phone,
        message,
        is_group: false,
        appointment_id: None,
        notification_type: notification_type.unwrap_or_else(|| "custom".to_string()),
    };
    let result = automation::send_whatsapp(&app_handle, pool.inner(), msg.clone()).await;
    crate::audit::for_session(
        pool.inner(), &session, "whatsapp_send", "whatsapp",
        None,
        Some(serde_json::json!({
            "recipient": msg.recipient,
            "notification_type": msg.notification_type,
            "success": result.is_ok()
        })),
    ).await;
    result
}

/// Quick connectivity test — sends a test message to the given phone number.
#[tauri::command]
pub async fn send_whatsapp_test(
    app_handle: tauri::AppHandle,
    pool: tauri::State<'_, PgPool>,
    session_state: tauri::State<'_, SessionState>,
    phone: String,
    clinic_name: Option<String>,
) -> Result<String, String> {
    let _session = rbac::require_session(&session_state)?;
    let clinic = clinic_name.unwrap_or_else(|| "VitalFlow HMS".to_string());
    let msg = WhatsAppMessage {
        recipient: phone,
        message: format!(
            "✅ *WhatsApp Test*\n\nThis is a test message from *{}*.\n\nYour WhatsApp integration is working correctly!\n\n_VitalFlow HMS_",
            clinic
        ),
        is_group: false,
        appointment_id: None,
        notification_type: "test".to_string(),
    };
    automation::send_whatsapp(&app_handle, pool.inner(), msg).await?;
    Ok("WhatsApp message sent successfully.".to_string())
}

/// Fetch the WhatsApp notification audit log.
#[tauri::command]
pub async fn get_notification_log(
    pool: tauri::State<'_, PgPool>,
    session_state: tauri::State<'_, SessionState>,
    limit: Option<i64>,
) -> Result<Vec<serde_json::Value>, String> {
    let _ = rbac::require_session(&session_state)?;
    log::fetch_notification_log(pool.inner(), limit.unwrap_or(50)).await
}

// ── Business API config management ───────────────────────────────────────────

/// Read the WhatsApp config including the preferred sending method.
/// The access token is masked for security — only the last 4 chars are shown.
///
/// CR-9: `whatsapp_config` is a singleton (id=1 enforced by a CHECK
/// constraint). We read the singleton row directly instead of relying on
/// `ORDER BY id DESC LIMIT 1`, which previously returned an arbitrary row
/// from the accumulated duplicates.
#[tauri::command]
pub async fn get_whatsapp_config(
    pool: tauri::State<'_, PgPool>,
    session_state: tauri::State<'_, SessionState>,
) -> Result<serde_json::Value, String> {
    let _ = rbac::require_session(&session_state)?;
    let row = sqlx::query_as::<_, (Option<String>, Option<String>, bool, String)>(
        "SELECT access_token, phone_number_id, enabled, preferred_method \
         FROM whatsapp_config WHERE id = 1",
    )
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| format!("Load WhatsApp config: {}", e))?;

    match row {
        None => Ok(serde_json::json!({
            "configured": false,
            "enabled": false,
            "preferred_method": "deep_link",
        })),
        Some((token, phone_id, enabled, preferred_method)) => {
            let masked_token = token.as_deref().map(|t| {
                if t.len() > 8 {
                    format!("••••••••{}", &t[t.len()-4..])
                } else {
                    "••••".to_string()
                }
            });
            Ok(serde_json::json!({
                "configured": token.is_some() && phone_id.is_some() && !token.as_deref().unwrap_or("").is_empty(),
                "enabled": enabled,
                "preferred_method": preferred_method,
                "access_token_masked": masked_token,
                "phone_number_id": phone_id,
            }))
        }
    }
}

/// Save the WhatsApp config including the preferred sending method.
/// Upserts the single config row. If `access_token` is empty, the existing
/// token is preserved (so the user can change just the method/preference
/// without re-entering the token).
#[tauri::command]
pub async fn set_whatsapp_config(
    pool: tauri::State<'_, PgPool>,
    session_state: tauri::State<'_, SessionState>,
    access_token: String,
    phone_number_id: String,
    enabled: bool,
    preferred_method: String,
) -> Result<(), String> {
    // CR-4: require SettingsManage — any logged-in user (including patient role)
    // must NOT be able to replace WhatsApp credentials and redirect PHI-laden
    // notifications to an attacker-controlled endpoint.
    let session = rbac::require(&session_state, rbac::Permission::SettingsManage)?;

    // Validate preferred_method
    if preferred_method != "api" && preferred_method != "deep_link" {
        return Err("preferred_method must be 'api' or 'deep_link'.".to_string());
    }

    // If access_token is empty, preserve the existing one (COALESCE pattern).
    // We use a separate query path for insert vs update to handle this cleanly.
    //
    // CR-9: read from the singleton row (id=1) — not `ORDER BY id DESC LIMIT 1`
    // which previously returned an arbitrary row from accumulated duplicates.
    let existing = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT access_token FROM whatsapp_config WHERE id = 1",
    )
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| format!("Check existing config: {}", e))?;

    let token_to_save = if access_token.is_empty() {
        existing.and_then(|(t,)| t).unwrap_or_default()
    } else {
        access_token
    };

    // CR-9: pin id=1 on INSERT so ON CONFLICT (id) actually fires and upserts
    // the singleton row. Without the explicit id, SERIAL would allocate a new
    // id each call and the conflict would never trigger (the original bug).
    sqlx::query(
        r#"INSERT INTO whatsapp_config (id, access_token, phone_number_id, enabled, preferred_method, updated_at)
           VALUES (1, $1, $2, $3, $4, NOW())
           ON CONFLICT (id) DO UPDATE SET
              access_token = EXCLUDED.access_token,
              phone_number_id = EXCLUDED.phone_number_id,
              enabled = EXCLUDED.enabled,
              preferred_method = EXCLUDED.preferred_method,
              updated_at = NOW()"#,
    )
    .bind(&token_to_save)
    .bind(&phone_number_id)
    .bind(enabled)
    .bind(&preferred_method)
    .execute(pool.inner())
    .await
    .map_err(|e| format!("Save WhatsApp config: {}", e))?;

    crate::audit::for_session(
        pool.inner(), &session, "whatsapp_config_update", "whatsapp",
        None,
        Some(serde_json::json!({
            "enabled": enabled,
            "phone_number_id": phone_number_id,
            "preferred_method": preferred_method
        })),
    ).await;
    Ok(())
}

/// Test the Business API by sending a test message to the given phone.
/// Uses the currently saved config — no need to pass credentials.
#[tauri::command]
pub async fn test_whatsapp_api(
    pool: tauri::State<'_, PgPool>,
    session_state: tauri::State<'_, SessionState>,
    test_phone: String,
) -> Result<String, String> {
    // CR-4: require SettingsManage — testing the API sends a real WhatsApp
    // message using stored credentials; restrict to admins.
    let _ = rbac::require(&session_state, rbac::Permission::SettingsManage)?;
    let config = automation::load_whatsapp_config(pool.inner())
        .await
        .filter(|c| !c.access_token.is_empty() && !c.phone_number_id.is_empty())
        .ok_or_else(|| "WhatsApp Business API is not configured. Enter your access token and phone number ID first.".to_string())?;

    let phone = automation::normalize_phone(&test_phone, None)?;
    let message = "✅ WhatsApp Business API test from VitalFlow HMS. If you received this, your API integration is working correctly!";
    automation::send_via_business_api(&config, &phone, message).await?;
    Ok("Test message sent via Business API. Check the recipient's WhatsApp.".to_string())
}
