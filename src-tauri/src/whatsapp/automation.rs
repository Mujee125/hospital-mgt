//! WhatsApp messaging — multi-strategy delivery.
//!
//! ## Architecture
//!
//! **Strategy 1 — WhatsApp Business Cloud API (fully automatic):**
//! When Meta Business API credentials are configured (access_token +
//! phone_number_id), messages are sent directly via the Cloud API HTTP
//! endpoint. This is fully automatic — no WhatsApp client opens, no user
//! interaction required. Requires a Meta Business account and a WhatsApp
//! Business number. Costs ~$0.004 per conversation. This is the ONLY
//! ToS-compliant way to send WhatsApp messages programmatically.
//!
//! **Strategy 2 — `wa.me` deep link (fallback):**
//! If Business API is not configured, opens `https://wa.me/<phone>?text=<msg>`
//! in the system default browser. WhatsApp Desktop or WhatsApp Web opens with
//! the message pre-filled; the user presses Send. This is the standard
//! approach for non-Business-API integrations (used by Calendly, etc.).
//!
//! Both strategies log the outcome to `whatsapp_notifications` for audit.

use tauri_plugin_opener::OpenerExt;
use tauri_plugin_clipboard_manager::ClipboardExt;

use super::WhatsAppMessage;

// ── Phone normalization ──────────────────────────────────────────────────────

/// Normalize a phone number for WhatsApp's APIs.
///
/// WhatsApp expects the full international number WITHOUT `+`, spaces, dashes,
/// or parentheses. Examples:
///   "+92 300 1234567" → "923001234567"
///   "0300 1234567"     → "923001234567" (if default_country = "92")
///   "(555) 123-4567"   → "5551234567"
pub fn normalize_phone(raw: &str, default_country: Option<&str>) -> Result<String, String> {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return Err("Phone number contains no digits.".to_string());
    }
    let normalized = if let Some(stripped) = digits.strip_prefix("00") {
        stripped.to_string()
    } else if digits.starts_with('0') && digits.len() > 1 {
        match default_country {
            Some(cc) => format!("{}{}", cc.trim_start_matches('+'), &digits[1..]),
            None => digits,
        }
    } else {
        digits
    };
    if normalized.len() < 8 || !normalized.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!("Normalized phone number '{}' is invalid.", normalized));
    }
    Ok(normalized)
}

// ── URL encoding ─────────────────────────────────────────────────────────────

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            b' ' => out.push_str("%20"),
            b'\n' => out.push_str("%0A"),
            b'\r' => out.push_str("%0D"),
            _ => {
                out.push_str(&format!("%{:02X}", *b));
            }
        }
    }
    out
}

fn wa_me_url(phone: &str, text: &str) -> String {
    format!("https://wa.me/{}?text={}", phone, url_encode(text))
}

// ── SEC-09: WhatsApp URL allow-list ──────────────────────────────────────────
//
// Before handing a URL to `opener().open_url()`, verify it is an HTTPS URL
// whose host is one of the two legitimate WhatsApp deep-link destinations:
//   - wa.me              — the standard click-to-chat short link.
//   - web.whatsapp.com   — WhatsApp Web (used for group sends).
//
// `opener:allow-open-url` (the only opener capability we grant) accepts
// ANY URL scheme including `file:`, `smb:`, `ftp:`, and custom URI
// handlers. Without this check, an attacker who can influence `phone`
// or `message` (e.g. a crafted patient record flowing into a
// notification) could direct the OS's default URL handler at an
// arbitrary resource — opening a local file, mounting an SMB share,
// or invoking a custom protocol handler that exfiltrates data.
fn validate_whatsapp_url(url: &str) -> Result<(), String> {
    // Parse with the `url` crate would be cleaner, but we don't pull it
    // in just for this; a lightweight manual parse is sufficient because
    // we generate the URL ourselves (so its shape is known).
    let lower = url.to_lowercase();
    if !lower.starts_with("https://") {
        return Err(format!(
            "Refused to open URL with disallowed scheme (only HTTPS is allowed): {}",
            url
        ));
    }
    let after_scheme = &url[8..]; // skip "https://"
    // Host is everything up to the first '/', '?', '#', or end-of-string.
    let host_end = after_scheme
        .find(['/', '?', '#'])
        .unwrap_or(after_scheme.len());
    let host = &after_scheme[..host_end];
    // Strip optional userinfo + port (defensive — we never generate these
    // but be strict in case the URL was tampered with before reaching us).
    let host = host.rsplit('@').next().unwrap_or(host);
    let host = host.split(':').next().unwrap_or(host);
    let host_lower = host.to_lowercase();
    if host_lower == "wa.me" || host_lower == "web.whatsapp.com" {
        Ok(())
    } else {
        Err(format!(
            "Refused to open URL with disallowed host (only wa.me and web.whatsapp.com are allowed): {}",
            url
        ))
    }
}

// ── WhatsApp Business Cloud API ──────────────────────────────────────────────

/// Configuration for the WhatsApp Business Cloud API.
/// Stored in the `whatsapp_config` table; loaded at send time.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WhatsAppBusinessConfig {
    pub access_token: String,
    pub phone_number_id: String,
    pub enabled: bool,
    /// "api" = send via Business Cloud API (fully automatic),
    /// "deep_link" = open wa.me in browser (user clicks Send).
    pub preferred_method: String,
}

/// Load the WhatsApp Business API config from the database.
/// Returns the full config row (even if credentials are missing) so the
/// caller can check `preferred_method` regardless of whether the API is
/// configured. Returns None only if no row exists at all.
///
/// CR-9: `whatsapp_config` is a singleton (id=1). Read the singleton row
/// directly instead of `ORDER BY id DESC LIMIT 1`, which previously
/// returned an arbitrary row from accumulated duplicates.
pub async fn load_whatsapp_config(pool: &sqlx::PgPool) -> Option<WhatsAppBusinessConfig> {
    let row = sqlx::query_as::<_, (Option<String>, Option<String>, bool, String)>(
        "SELECT access_token, phone_number_id, enabled, preferred_method \
         FROM whatsapp_config WHERE id = 1",
    )
    .fetch_optional(pool)
    .await
    .ok()??;

    let (token, phone_id, enabled, preferred_method) = row;
    Some(WhatsAppBusinessConfig {
        access_token: token.unwrap_or_default(),
        phone_number_id: phone_id.unwrap_or_default(),
        enabled,
        preferred_method,
    })
}

/// Returns true if the Business API is fully configured AND the user selected
/// it as their preferred method.
pub async fn should_use_business_api(pool: &sqlx::PgPool) -> bool {
    match load_whatsapp_config(pool).await {
        Some(cfg) => {
            cfg.enabled
                && cfg.preferred_method == "api"
                && !cfg.access_token.is_empty()
                && !cfg.phone_number_id.is_empty()
        }
        None => false,
    }
}

/// Send a WhatsApp message via the official Business Cloud API.
///
/// This is fully automatic — no WhatsApp client opens, no user interaction.
/// Requires a valid access token and phone_number_id from Meta Business.
///
/// API docs: https://developers.facebook.com/docs/whatsapp/cloud-api/messages
///
/// Note: WhatsApp Business API requires template messages for proactive
/// (non-session) messages. For appointment notifications, you'd need to
/// register a template like "appointment_confirmation" with Meta. For
/// simplicity, this implementation sends a text message; in production
/// you may need to use template messages depending on your Meta approval.
pub async fn send_via_business_api(
    config: &WhatsAppBusinessConfig,
    phone: &str,
    message: &str,
) -> Result<(), String> {
    let url = format!(
        "https://graph.facebook.com/v18.0/{}/messages",
        config.phone_number_id
    );

    let body = serde_json::json!({
        "messaging_product": "whatsapp",
        "recipient_type": "individual",
        "to": phone,
        "type": "text",
        "text": {
            "preview_url": false,
            "body": message
        }
    });

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", config.access_token))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Business API request failed: {}", e))?;

    if response.status().is_success() {
        Ok(())
    } else {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<no body>".to_string());
        Err(format!(
            "WhatsApp Business API returned {} ({}). \
             Check your access token and phone_number_id. \
             Note: proactive messages may require a pre-approved template.",
            status, body
        ))
    }
}

// ── Main send function ───────────────────────────────────────────────────────

/// Send a WhatsApp message using the user's preferred method.
///
/// The user selects their preferred method in Settings → WhatsApp:
///   - "api"       → Business Cloud API (fully automatic, no UI)
///   - "deep_link" → wa.me deep link (opens WhatsApp, user clicks Send)
///
/// If the preferred method is "api" but credentials are missing or the API
/// call fails, the function falls back to the deep link so the message still
/// goes out. Group messages always use the deep-link strategy.
///
/// CR-12 (SRS FR-0035, HIPAA, GDPR): before any non-group send that is not a
/// connectivity test, verify the recipient patient has explicitly consented
/// to WhatsApp notifications. If the recipient phone matches a patient in
/// the DB and that patient has no `whatsapp` consent row, or has one with
/// `granted = false`, the send is refused. Group messages and connectivity
/// tests (`notification_type = "test"`) bypass the consent gate because the
/// recipient is a group chat or the operator's own phone, not a patient.
pub async fn send_whatsapp(
    app_handle: &tauri::AppHandle,
    pool: &sqlx::PgPool,
    msg: WhatsAppMessage,
) -> Result<(), String> {
    let clinic_default_cc: Option<String> = std::env::var("HMS_DEFAULT_CC").ok();

    // ── CR-12 consent gate ───────────────────────────────────────────────
    // Refuse patient-facing sends without explicit opt-in consent. The gate
    // is a no-op for group sends and for connectivity tests.
    if !msg.is_group && msg.notification_type != "test" {
        check_patient_consent(pool, &msg.recipient, clinic_default_cc.as_deref()).await?;
    }

    // ── Strategy 1: Business API (user selected "api") ──
    // Only for non-group messages; groups can't use the simple text API.
    if !msg.is_group && should_use_business_api(pool).await {
        let config = load_whatsapp_config(pool).await.unwrap();
        let phone = normalize_phone(&msg.recipient, clinic_default_cc.as_deref())?;
        match send_via_business_api(&config, &phone, &msg.message).await {
            Ok(()) => {
                let _ = super::log::log_notification(pool, &msg, true).await;
                return Ok(());
            }
            Err(e) => {
                eprintln!("[HMS WA] Business API failed ({}), falling back to deep link", e);
                // Fall through to deep-link strategy
            }
        }
    }

    // ── Strategy 2: wa.me deep link (user selected "deep_link", or API fallback) ──
    let (url, strategy) = if msg.is_group {
        let url = "https://web.whatsapp.com/".to_string();
        (url, "group_web".to_string())
    } else {
        let phone = normalize_phone(&msg.recipient, clinic_default_cc.as_deref())?;
        let wa_me = wa_me_url(&phone, &msg.message);
        (wa_me, format!("wa_me:{}", phone))
    };

    // SEC-09: validate the URL scheme + host BEFORE handing it to the OS's
    // default-handler opener. The opener plugin honours the capability
    // `opener:allow-open-url` (we removed `opener:allow-open-path`), but
    // `open-url` itself accepts ANY URL scheme — `file:`, `smb:`, `ftp:`,
    // custom URI handlers, etc. An attacker who can control `msg.recipient`
    // or `msg.message` (e.g. via a crafted patient record that flows into
    // a notification) could otherwise direct the opener at an arbitrary
    // resource. We harden by allowing ONLY `https:` with hosts `wa.me` or
    // `web.whatsapp.com` — the only two legitimate WhatsApp deep-link
    // destinations.
    if let Err(e) = validate_whatsapp_url(&url) {
        let _ = super::log::log_notification(pool, &msg, false).await;
        return Err(e);
    }

    let open_result = app_handle
        .opener()
        .open_url(&url, None::<&str>)
        .map_err(|e| format!("Failed to open URL '{}': {}", url, e));

    let success = open_result.is_ok();
    let _ = open_result;

    let _ = super::log::log_notification(pool, &msg, success).await;

    if msg.is_group && success {
        let _ = app_handle.clipboard().write_text(&msg.message);
    }

    if success {
        Ok(())
    } else {
        Err(format!(
            "Could not open WhatsApp automatically. Strategy: {}. \
             Make sure WhatsApp Desktop is installed, or open {} manually.",
            strategy, url
        ))
    }
}

// ── Patient consent gate (CR-12, SRS FR-0035) ──────────────────────────────

/// Refuse the send if the recipient phone matches a patient in the DB and
/// that patient has not granted WhatsApp consent. Returns `Ok(())` if:
///   • the recipient phone does not match any patient (the recipient is not
///     a patient, so HIPAA consent does not apply — e.g. a doctor's phone
///     used for a manual ad-hoc send), OR
///   • the patient has a `patient_consent` row with `consent_type = 'whatsapp'`
///     and `granted = true`.
///
/// Returns `Err` with a clear, user-facing message otherwise.
///
/// Lookup is by phone-digit suffix (last 9 digits) to tolerate the various
/// local / international formats stored in `patients.phone` vs the
/// normalized `WhatsAppMessage.recipient`.
async fn check_patient_consent(
    pool: &sqlx::PgPool,
    raw_recipient: &str,
    default_country: Option<&str>,
) -> Result<(), String> {
    // Normalize the recipient the same way `send_whatsapp` does, so the
    // suffix we match on is comparable to the digits in `patients.phone`.
    // If normalization fails (e.g. group name slipped through), fail CLOSED
    // — refuse the send — because we cannot prove consent.
    let normalized = normalize_phone(raw_recipient, default_country)?;
    let digits: String = normalized.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 9 {
        // Very short numbers are unusual; we cannot safely identify a
        // patient, so fail closed.
        return Err(
            "Patient has not consented to WhatsApp notifications. \
             Update consent in the patient record first."
                .to_string(),
        );
    }
    let suffix_len = digits.len().min(9);
    let suffix = &digits[digits.len() - suffix_len..];
    let pattern = format!("%{}", suffix);

    // Look up the patient by phone suffix. If no patient matches, the
    // recipient is not a patient in our system — consent gate does not apply.
    // CR-11: only consider ACTIVE patients (deleted_at IS NULL). A soft-deleted
    // patient's stale consent record must NOT block a future patient who
    // happens to share the same phone-suffix pattern.
    let patient: Option<(i32,)> = sqlx::query_as(
        "SELECT id FROM patients WHERE phone LIKE $1 AND deleted_at IS NULL ORDER BY id DESC LIMIT 1",
    )
    .bind(&pattern)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Consent check (patient lookup) failed: {}", e))?;

    let patient_id = match patient {
        Some((id,)) => id,
        None => return Ok(()), // Recipient is not a registered patient.
    };

    // Look up the patient's `whatsapp` consent record. (patient_id,
    // consent_type) is UNIQUE, so LIMIT 1 is defensive.
    let row: Option<(bool,)> = sqlx::query_as(
        r#"SELECT granted FROM patient_consent
           WHERE patient_id = $1 AND consent_type = 'whatsapp'
           LIMIT 1"#,
    )
    .bind(patient_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Consent check failed: {}", e))?;

    match row {
        Some((granted,)) if granted => Ok(()),
        _ => Err(
            "Patient has not consented to WhatsApp notifications. \
             Update consent in the patient record first."
                .to_string(),
        ),
    }
}
