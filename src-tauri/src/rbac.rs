//! Role-Based Access Control (ISO/IEC 27001 A.9 — access control).
//!
//! Design:
//! - A single canonical `Permission` enum is the source of truth for every
//!   authorisable action. The DB `permissions` table mirrors these keys; the
//!   seed function populates both. This prevents "stringly-typed" permission
//!   drift between Rust and the database.
//! - `require_permission` is the guard every protected command calls. It
//!   returns the current `Session` on success so commands can chain straight
//!   into audit logging, keeping call sites terse and consistent.
//! - Roles bundle permissions. The mapping is data-driven (role_permissions
//!   table) so administrators can adjust grants without a code change, but the
//!   default seed encodes least-privilege per NHS/HIPAA-aligned personas.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

/// Tauri-managed shared session state. One active desktop user per process.
pub type SessionState = Arc<Mutex<Option<Session>>>;

/// Every authorisable action in the system. Add new variants here and they are
/// automatically seeded into the `permissions` table by `auth::seed_defaults`.
///
/// Naming convention: `<Resource><Action>` in SCREAMING_SNAKE_CASE via the
/// `as_str()` representation, e.g. `PatientsCreate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    // Dashboard
    DashboardView,
    // Patients / EHR
    PatientsView,
    PatientsCreate,
    PatientsUpdate,
    PatientsDelete,
    PatientConsentManage,
    // Appointments
    AppointmentsView,
    AppointmentsCreate,
    AppointmentsUpdate,
    AppointmentsDelete,
    // Queue
    QueueView,
    QueueManage,
    // Doctors
    DoctorsView,
    DoctorsManage,
    // IPD
    IpdView,
    IpdManage,
    BedsManage,
    // Laboratory
    LabView,
    LabOrder,
    LabResultManage,
    LabCatalogManage,
    // Radiology
    RadiologyView,
    RadiologyCreate,
    RadiologyUpdate,
    RadiologyDelete,
    RadiologyReport,
    RadiologyVerify,
    RadiologyManage,
    // Blood Bank
    BloodBankView,
    BloodBankManage,
    BloodBankDonorManage,
    BloodBankCrossmatch,
    BloodBankIssue,
    BloodBankTransfuse,
    BloodBankDiscard,
    BloodBankVerify,
    // Billing
    BillingView,
    BillingCreate,
    BillingManage,
    PaymentsManage,
    // Inventory
    InventoryView,
    InventoryManage,
    // Users & RBAC
    UsersView,
    UsersManage,
    RolesManage,
    // Audit & reports
    AuditView,
    ReportsView,
    // Messaging (staff chat) — per SRS NFR-15, every protected command must be gated.
    MessagingView,
    MessagingSend,
    // WhatsApp (patient-facing comms) — separated from Messaging
    // because external communication has distinct authorization and PHI
    // implications. RCTF-IMPL-001 WP-1: gates the 5 WhatsApp commands that
    // were previously AUTH_ONLY (patient-role could invoke).
    WhatsAppSend,
    WhatsAppView,
    // System / settings
    SettingsManage,
    LicenseManage,
    BackupsManage,
}

impl Permission {
    /// Stable string key persisted in the `permissions` table. Never change
    /// existing keys (they are foreign-keyed); only append new ones.
    pub fn as_str(&self) -> &'static str {
        match self {
            Permission::DashboardView => "dashboard.view",
            Permission::PatientsView => "patients.view",
            Permission::PatientsCreate => "patients.create",
            Permission::PatientsUpdate => "patients.update",
            Permission::PatientsDelete => "patients.delete",
            Permission::PatientConsentManage => "patients.consent.manage",
            Permission::AppointmentsView => "appointments.view",
            Permission::AppointmentsCreate => "appointments.create",
            Permission::AppointmentsUpdate => "appointments.update",
            Permission::AppointmentsDelete => "appointments.delete",
            Permission::QueueView => "queue.view",
            Permission::QueueManage => "queue.manage",
            Permission::DoctorsView => "doctors.view",
            Permission::DoctorsManage => "doctors.manage",
            Permission::IpdView => "ipd.view",
            Permission::IpdManage => "ipd.manage",
            Permission::BedsManage => "beds.manage",
            Permission::LabView => "lab.view",
            Permission::LabOrder => "lab.order",
            Permission::LabResultManage => "lab.result.manage",
            Permission::LabCatalogManage => "lab.catalog.manage",
            Permission::RadiologyView => "radiology.view",
            Permission::RadiologyCreate => "radiology.create",
            Permission::RadiologyUpdate => "radiology.update",
            Permission::RadiologyDelete => "radiology.delete",
            Permission::RadiologyReport => "radiology.report",
            Permission::RadiologyVerify => "radiology.verify",
            Permission::RadiologyManage => "radiology.manage",
            Permission::BloodBankView => "bloodbank.view",
            Permission::BloodBankManage => "bloodbank.manage",
            Permission::BloodBankDonorManage => "bloodbank.donor.manage",
            Permission::BloodBankCrossmatch => "bloodbank.crossmatch",
            Permission::BloodBankIssue => "bloodbank.issue",
            Permission::BloodBankTransfuse => "bloodbank.transfuse",
            Permission::BloodBankDiscard => "bloodbank.discard",
            Permission::BloodBankVerify => "bloodbank.verify",
            Permission::BillingView => "billing.view",
            Permission::BillingCreate => "billing.create",
            Permission::BillingManage => "billing.manage",
            Permission::PaymentsManage => "payments.manage",
            Permission::InventoryView => "inventory.view",
            Permission::InventoryManage => "inventory.manage",
            Permission::UsersView => "users.view",
            Permission::UsersManage => "users.manage",
            Permission::RolesManage => "roles.manage",
            Permission::AuditView => "audit.view",
            Permission::ReportsView => "reports.view",
            Permission::MessagingView => "messaging.view",
            Permission::MessagingSend => "messaging.send",
            Permission::WhatsAppSend => "whatsapp.send",
            Permission::WhatsAppView => "whatsapp.view",
            Permission::SettingsManage => "settings.manage",
            Permission::LicenseManage => "license.manage",
            Permission::BackupsManage => "backups.manage",
        }
    }

    /// All variants, in declaration order. Used by the seeder.
    pub fn all() -> &'static [Permission] {
        &[
            Permission::DashboardView,
            Permission::PatientsView,
            Permission::PatientsCreate,
            Permission::PatientsUpdate,
            Permission::PatientsDelete,
            Permission::PatientConsentManage,
            Permission::AppointmentsView,
            Permission::AppointmentsCreate,
            Permission::AppointmentsUpdate,
            Permission::AppointmentsDelete,
            Permission::QueueView,
            Permission::QueueManage,
            Permission::DoctorsView,
            Permission::DoctorsManage,
            Permission::IpdView,
            Permission::IpdManage,
            Permission::BedsManage,
            Permission::LabView,
            Permission::LabOrder,
            Permission::LabResultManage,
            Permission::LabCatalogManage,
            Permission::RadiologyView,
            Permission::RadiologyCreate,
            Permission::RadiologyUpdate,
            Permission::RadiologyDelete,
            Permission::RadiologyReport,
            Permission::RadiologyVerify,
            Permission::RadiologyManage,
            Permission::BloodBankView,
            Permission::BloodBankManage,
            Permission::BloodBankDonorManage,
            Permission::BloodBankCrossmatch,
            Permission::BloodBankIssue,
            Permission::BloodBankTransfuse,
            Permission::BloodBankDiscard,
            Permission::BloodBankVerify,
            Permission::BillingView,
            Permission::BillingCreate,
            Permission::BillingManage,
            Permission::PaymentsManage,
            Permission::InventoryView,
            Permission::InventoryManage,
            Permission::UsersView,
            Permission::UsersManage,
            Permission::RolesManage,
            Permission::AuditView,
            Permission::ReportsView,
            Permission::MessagingView,
            Permission::MessagingSend,
            Permission::WhatsAppSend,
            Permission::WhatsAppView,
            Permission::SettingsManage,
            Permission::LicenseManage,
            Permission::BackupsManage,
        ]
    }
}

/// Canonical role names seeded into the `roles` table.
pub const ROLE_SUPER_ADMIN: &str = "super_admin";
pub const ROLE_DOCTOR: &str = "doctor";
pub const ROLE_NURSE: &str = "nurse";
pub const ROLE_RECEPTIONIST: &str = "receptionist";
pub const ROLE_LAB_TECH: &str = "lab_technician";
pub const ROLE_PHARMACIST: &str = "pharmacist";
pub const ROLE_BILLING: &str = "billing_clerk";
pub const ROLE_PATIENT: &str = "patient";

/// Returns the seed permission set for a given role. Encodes least-privilege:
/// each persona gets only what it needs (HIPAA "minimum necessary" principle).
pub fn permissions_for_role(role: &str) -> Vec<Permission> {
    use Permission::*;
    match role {
        ROLE_SUPER_ADMIN => Permission::all().to_vec(),
        ROLE_DOCTOR => vec![
            DashboardView, PatientsView, PatientsCreate, PatientsUpdate,
            AppointmentsView, AppointmentsUpdate, QueueView,
            DoctorsView, IpdView, IpdManage, LabView, LabOrder, LabResultManage,
            RadiologyView, RadiologyCreate, RadiologyUpdate,
            BloodBankView, BloodBankCrossmatch, BloodBankIssue, BloodBankTransfuse,
            BillingView, InventoryView, PatientConsentManage, AuditView, ReportsView,
            MessagingView, MessagingSend,
            WhatsAppSend, WhatsAppView,
        ],
        ROLE_NURSE => vec![
            DashboardView, PatientsView, PatientsUpdate, AppointmentsView,
            QueueView, QueueManage, IpdView, IpdManage, BedsManage,
            LabView, InventoryView, ReportsView,
            BloodBankView, BloodBankTransfuse,
            MessagingView, MessagingSend,
            WhatsAppSend, WhatsAppView,
        ],
        ROLE_RECEPTIONIST => vec![
            DashboardView, PatientsView, PatientsCreate, PatientsUpdate,
            AppointmentsView, AppointmentsCreate, AppointmentsUpdate,
            QueueView, QueueManage, DoctorsView, BillingView, BillingCreate,
            MessagingView, MessagingSend,
            WhatsAppSend, WhatsAppView,
        ],
        ROLE_LAB_TECH => vec![
            DashboardView, PatientsView, LabView, LabOrder, LabResultManage,
            LabCatalogManage, InventoryView,
            BloodBankView, BloodBankDonorManage, BloodBankCrossmatch,
            MessagingView, MessagingSend,
        ],
        ROLE_PHARMACIST => vec![
            DashboardView, InventoryView, InventoryManage, BillingView, PatientsView,
            MessagingView, MessagingSend,
        ],
        ROLE_BILLING => vec![
            DashboardView, BillingView, BillingCreate, BillingManage, PaymentsManage,
            PatientsView, AppointmentsView, ReportsView,
            MessagingView, MessagingSend,
            WhatsAppView,
        ],
        ROLE_PATIENT => vec![DashboardView],
        _ => vec![],
    }
}

/// The set of role names the seeder creates.
pub fn seed_roles() -> Vec<(&'static str, &'static str)> {
    vec![
        (ROLE_SUPER_ADMIN,    "Full system access — break-glass only"),
        (ROLE_DOCTOR,         "Clinical care: EHR, prescriptions, rounds, lab orders"),
        (ROLE_NURSE,          "Ward & IPD care, queue, vitals"),
        (ROLE_RECEPTIONIST,   "Front desk: registration, appointments, queue"),
        (ROLE_LAB_TECH,       "Laboratory orders and results"),
        (ROLE_PHARMACIST,     "Pharmacy inventory and dispensing"),
        (ROLE_BILLING,        "Billing, invoices, payments"),
        (ROLE_PATIENT,        "Patient portal — own records only"),
    ]
}

// ── Session (held in Tauri app state) ─────────────────────────────────────────

/// The authenticated principal, held in `AppState` for the lifetime of a
/// desktop session. Desktop HMS = one active user per process, so an in-memory
/// session is appropriate and avoids transmitting tokens on every command.
#[derive(Debug, Clone)]
pub struct Session {
    pub user_id: i32,
    pub username: String,
    pub full_name: String,
    pub roles: Vec<String>,
    pub permissions: HashSet<String>,
    // RCTF-IMPL-001 WP-2.1: SHA-256 hash of the session token. Used by `me`
    // and (in WP-2.2) `require_strong` to validate that the in-memory session
    // still exists in the DB. Without this, `me` filtered by `user_id` only —
    // a cross-PC login (which deletes the prior session row) was not detected.
    pub token_hash: String,
}

impl Session {
    pub fn has(&self, perm: Permission) -> bool {
        self.permissions.contains(perm.as_str())
    }
}

/// Guard: returns the session or an "access denied" error string.
/// Every protected command begins with:
///   `let session = rbac::require(&session_state, Permission::XxxYyy)?;`
///
/// `&session_state` (a `tauri::State<SessionState>`) deref-coerces to
/// `&SessionState` here because `State<T>: Deref<Target = T>`.
pub fn require(state: &SessionState, perm: Permission) -> Result<Session, String> {
    // REL-02: recover from mutex poisoning instead of panicking. A poisoned
    // mutex means a previous thread panicked while holding the lock; the data
    // it guards may be in an inconsistent state, but for our session cache
    // the worst case is a stale/missing session — better to serve the
    // (possibly-stale) cached state than to permanently lock out ALL
    // authentication for the running process.
    let guard = state.lock().unwrap_or_else(|e| e.into_inner());
    match guard.as_ref() {
        Some(s) if s.has(perm) => Ok(s.clone()),
        Some(_) => Err(format!(
            "Access denied: this action requires the '{}' permission.",
            perm.as_str()
        )),
        None => Err("Access denied: you are not signed in.".to_string()),
    }
}

/// Guard for commands that only need an authenticated user (no specific
/// permission), e.g. `me`, `change_password`, `logout`.
pub fn require_session(state: &SessionState) -> Result<Session, String> {
    // REL-02: same poisoning-recovery rationale as `require` above.
    let guard = state.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .clone()
        .ok_or_else(|| "Access denied: you are not signed in.".to_string())
}


/// RCTF-IMPL-001 WP-2.2: Strong guard for high-risk commands.
///
/// Like `require`, but also validates the session against the DB:
/// - Checks that the session's `token_hash` still exists in the `sessions` table
/// - Checks that `expires_at > NOW()`
/// - Checks that `users.is_active = TRUE`
///
/// If any check fails, clears the in-memory session and returns an error.
/// This catches: cross-PC login (token deleted), account deactivation,
/// role/permission changes, and password resets.
///
/// Used on ~22 high-risk state-changing commands (PHI-accessing).
pub async fn require_strong(
    state: &SessionState,
    pool: &sqlx::PgPool,
    perm: Permission,
) -> Result<Session, String> {
    // Step 1: in-memory permission check (same as require)
    let session = require(state, perm)?;

    // Step 2: DB-backed session validity check
    let valid: Option<(i32,)> = sqlx::query_as(
        "SELECT 1 FROM sessions s
         JOIN users u ON u.id = s.user_id
         WHERE s.token_hash = $1
           AND s.expires_at > NOW()
           AND u.is_active = TRUE",
    )
    .bind(&session.token_hash)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Session validation query failed: {}", e))?;

    if valid.is_none() {
        // Session invalidated (cross-PC login, deactivation, role change,
        // password reset, or expiry). Clear in-memory state so future
        // calls fail fast at the in-memory check instead of hitting DB.
        *state.lock().unwrap_or_else(|e| e.into_inner()) = None;
        return Err("Session invalidated. Please sign in again.".to_string());
    }

    Ok(session)
}

/// Like `require`, but allows pre-login access (no session).
///
/// Returns `Ok(None)` when there is no session (boot/setup screen before
/// login), or `Ok(Some(session))` when a session exists and has the
/// required permission, or `Err(...)` when a session exists but lacks it.
///
/// Used by config + log commands that are needed during the boot flow
/// (before any user logs in) but should be admin-only once a user IS
/// logged in. The security trade-off is safe because:
///   - `get_config` never serializes `db_password` (skip_serializing)
///   - `get_log` redacts sensitive patterns at read time
///   - `save_config` / `repair_server_config` / `clear_config` are only
///     callable pre-login during first-run Setup; post-login they require
///     SettingsManage.
pub fn require_if_session(
    state: &SessionState,
    perm: Permission,
) -> Result<Option<Session>, String> {
    let guard = state.lock().unwrap_or_else(|e| e.into_inner());
    match guard.as_ref() {
        None => Ok(None), // Pre-login: allow (boot/setup screen)
        Some(s) if s.has(perm) => Ok(Some(s.clone())), // Authorized: allow
        Some(_) => Err(format!(
            "Access denied: this action requires the '{}' permission.",
            perm.as_str()
        )),
    }
}


#[cfg(test)]
mod wp1_tests {
    use super::*;

    #[test]
    fn test_whatsapp_send_permission_exists() {
        assert_eq!(Permission::WhatsAppSend.as_str(), "whatsapp.send");
    }

    #[test]
    fn test_whatsapp_view_permission_exists() {
        assert_eq!(Permission::WhatsAppView.as_str(), "whatsapp.view");
    }

    #[test]
    fn test_whatsapp_permissions_in_all() {
        let all = Permission::all();
        assert!(all.contains(&Permission::WhatsAppSend));
        assert!(all.contains(&Permission::WhatsAppView));
    }

    #[test]
    fn test_doctor_has_whatsapp_send_and_view() {
        let perms = permissions_for_role(ROLE_DOCTOR);
        assert!(perms.contains(&Permission::WhatsAppSend));
        assert!(perms.contains(&Permission::WhatsAppView));
    }

    #[test]
    fn test_nurse_has_whatsapp_send_and_view() {
        let perms = permissions_for_role(ROLE_NURSE);
        assert!(perms.contains(&Permission::WhatsAppSend));
        assert!(perms.contains(&Permission::WhatsAppView));
    }

    #[test]
    fn test_receptionist_has_whatsapp_send_and_view() {
        let perms = permissions_for_role(ROLE_RECEPTIONIST);
        assert!(perms.contains(&Permission::WhatsAppSend));
        assert!(perms.contains(&Permission::WhatsAppView));
    }

    #[test]
    fn test_billing_clerk_has_view_only() {
        let perms = permissions_for_role(ROLE_BILLING);
        assert!(!perms.contains(&Permission::WhatsAppSend));
        assert!(perms.contains(&Permission::WhatsAppView));
    }

    #[test]
    fn test_patient_lacks_whatsapp_permissions() {
        let perms = permissions_for_role(ROLE_PATIENT);
        assert!(!perms.contains(&Permission::WhatsAppSend));
        assert!(!perms.contains(&Permission::WhatsAppView));
    }

    #[test]
    fn test_lab_tech_lacks_whatsapp_permissions() {
        let perms = permissions_for_role(ROLE_LAB_TECH);
        assert!(!perms.contains(&Permission::WhatsAppSend));
        assert!(!perms.contains(&Permission::WhatsAppView));
    }

    #[test]
    fn test_pharmacist_lacks_whatsapp_permissions() {
        let perms = permissions_for_role(ROLE_PHARMACIST);
        assert!(!perms.contains(&Permission::WhatsAppSend));
        assert!(!perms.contains(&Permission::WhatsAppView));
    }

    #[test]
    fn test_super_admin_has_all_whatsapp_permissions() {
        let perms = permissions_for_role(ROLE_SUPER_ADMIN);
        assert!(perms.contains(&Permission::WhatsAppSend));
        assert!(perms.contains(&Permission::WhatsAppView));
    }
}
