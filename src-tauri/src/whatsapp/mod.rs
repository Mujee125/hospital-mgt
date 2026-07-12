/// WhatsApp messaging module.
///
/// Submodules:
///   automation  — visible WebView + JS injection to auto-send via WhatsApp Web
///   templates   — pure string builders for every message type
///   log         — whatsapp_notifications audit-log table
///   commands    — Tauri command entry points (pub mod so generate_handler!
///                 can reach __cmd__X macros via whatsapp::commands::fn_name)
///
/// IMPORTANT — Tauri command macro visibility rule:
///   `#[tauri::command]` generates hidden macros (__cmd__X, __tauri_command_name_X)
///   in the module where the fn is defined. These macros CANNOT be re-exported
///   with `pub use` into a parent module — generate_handler! must reference them
///   via their defining module's path.
///
///   Correct in lib.rs:   whatsapp::commands::send_whatsapp_notification
///   Wrong  in lib.rs:    whatsapp::send_whatsapp_notification   (even with pub use)
pub mod commands;           // pub so generate_handler! can reach its macros
mod automation;
mod log;
mod templates;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WhatsAppMessage {
    /// Phone number (international, no +) OR group name
    pub recipient: String,
    pub message: String,
    /// true = recipient is a group name, false = phone number
    pub is_group: bool,
    /// Linked appointment id for the audit log (optional)
    pub appointment_id: Option<i32>,
    /// Type label for the audit log e.g. "booked", "confirmed", "reminder"
    pub notification_type: String,
}

// Re-export non-command helpers so the rest of the app keeps using
// whatsapp::send_whatsapp, whatsapp::build_reminder_msg, etc. unchanged.
pub use automation::send_whatsapp;
pub use templates::{
    build_appointment_booked_msg,
    build_appointment_cancelled_msg,
    build_appointment_confirmed_msg,
    build_daily_digest_msg,
    build_reminder_msg,
};
// NOTE: get_notification_log and send_whatsapp_notification are intentionally
// NOT re-exported here. generate_handler! must use the full path
// whatsapp::commands::send_whatsapp_notification so it can find the
// __cmd__X macros in their defining module (whatsapp::commands).
