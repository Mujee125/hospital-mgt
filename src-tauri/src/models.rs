use chrono::{NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Patient ──────────────────────────────────────────────

// Used as a Tauri command return type via #[tauri::command] — clippy
// doesn't see macro-expanded usage, so flag as allowed dead code.
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct Patient {
    pub id: i32,
    pub first_name: String,
    pub last_name: String,
    #[serde(default)]
    pub email: Option<String>,
    pub phone: String,
    pub date_of_birth: NaiveDate,
    pub gender: String,
    #[serde(default)]
    pub address: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// Used as a Tauri command parameter type via #[tauri::command] — clippy
// doesn't see macro-expanded usage, so flag as allowed dead code.
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct CreatePatient {
    pub first_name: String,
    pub last_name: String,
    #[serde(default)]
    pub email: Option<String>,
    pub phone: String,
    pub date_of_birth: String,
    pub gender: String,
    #[serde(default)]
    pub address: Option<String>,
}

// Used as a Tauri command parameter type via #[tauri::command] — clippy
// doesn't see macro-expanded usage, so flag as allowed dead code.
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdatePatient {
    pub id: i32,
    pub first_name: String,
    pub last_name: String,
    #[serde(default)]
    pub email: Option<String>,
    pub phone: String,
    pub date_of_birth: String,
    pub gender: String,
    #[serde(default)]
    pub address: Option<String>,
}

// ── Doctor ───────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct Doctor {
    pub id: i32,
    pub first_name: String,
    pub last_name: String,
    #[serde(default)]
    pub email: Option<String>,
    pub phone: String,
    pub specialization: String,
    pub qualification: String,
    pub available_from: NaiveTime,
    pub available_to: NaiveTime,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateDoctor {
    pub first_name: String,
    pub last_name: String,
    #[serde(default)]
    pub email: Option<String>,
    pub phone: String,
    pub specialization: String,
    pub qualification: String,
    pub available_from: String,
    pub available_to: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateDoctor {
    pub id: i32,
    pub first_name: String,
    pub last_name: String,
    #[serde(default)]
    pub email: Option<String>,
    pub phone: String,
    pub specialization: String,
    pub qualification: String,
    pub available_from: String,
    pub available_to: String,
    pub is_active: bool,
}

// ── Appointment ──────────────────────────────────────────

// Used as a Tauri command return type via #[tauri::command] — clippy
// doesn't see macro-expanded usage, so flag as allowed dead code.
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct Appointment {
    pub id: i32,
    pub patient_id: i32,
    pub doctor_id: i32,
    pub appointment_date: NaiveDate,
    pub appointment_time: NaiveTime,
    pub duration_minutes: i32,
    pub status: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateAppointment {
    pub patient_id: i32,
    pub doctor_id: i32,
    pub appointment_date: String,
    pub appointment_time: String,
    #[serde(default)]
    pub duration_minutes: Option<i32>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateAppointment {
    pub id: i32,
    pub patient_id: i32,
    pub doctor_id: i32,
    pub appointment_date: String,
    pub appointment_time: String,
    pub duration_minutes: i32,
    pub status: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct AppointmentWithDetails {
    pub id: i32,
    pub patient_id: i32,
    pub doctor_id: i32,
    pub appointment_date: NaiveDate,
    pub appointment_time: NaiveTime,
    pub duration_minutes: i32,
    pub status: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub patient_first_name: String,
    pub patient_last_name: String,
    pub doctor_first_name: String,
    pub doctor_last_name: String,
    pub doctor_specialization: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppointmentStats {
    pub total: i64,
    pub scheduled: i64,
    pub confirmed: i64,
    pub completed: i64,
    pub cancelled: i64,
    pub no_show: i64,
}

// ── Chat messages ─────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct ChatMessage {
    pub id: Uuid,
    pub sender: String,
    pub content: String,
    pub room: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendMessage {
    pub sender: String,
    pub content: String,
    pub room: String,
}

// ── Patient (EHR-expanded) ───────────────────────────────────────────────
//
// Superset of the original Patient: every original field is retained (so the
// existing `Patient` struct above still maps the base table), plus the EHR
// columns added by the migration. `sqlx::FromRow` deserialises by column name,
// so missing nullable columns yield `None` gracefully.

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct PatientEhr {
    pub id: i32,
    pub first_name: String,
    pub last_name: String,
    #[serde(default)]
    pub email: Option<String>,
    pub phone: String,
    pub date_of_birth: NaiveDate,
    pub gender: String,
    #[serde(default)]
    pub address: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    // EHR additions
    #[serde(default)]
    pub mrn: Option<String>,
    #[serde(default)]
    pub blood_group: Option<String>,
    #[serde(default)]
    pub allergies: Option<String>,
    #[serde(default)]
    pub chronic_conditions: Option<String>,
    #[serde(default)]
    pub emergency_contact_name: Option<String>,
    #[serde(default)]
    pub emergency_contact_phone: Option<String>,
    #[serde(default)]
    pub insurance_provider: Option<String>,
    #[serde(default)]
    pub insurance_policy_number: Option<String>,
    pub status: String,
    #[serde(default)]
    pub created_by_user_id: Option<i32>,
    // CR-11: soft-delete columns. `is_active` mirrors `deleted_at IS NULL`
    // for easy boolean filtering; `deleted_at` is the authoritative marker
    // (NULL = active, non-NULL = soft-deleted). Soft-deleted patients are
    // hidden from `get_patients` / `get_patient` (which filter
    // `deleted_at IS NULL`) but their clinical history (encounters, bills,
    // lab orders, queue tokens, appointments) is retained for HIPAA
    // §164.530(j) 6-year PHI retention.
    #[serde(default = "default_true")]
    pub is_active: bool,
    #[serde(default)]
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

fn default_true() -> bool { true }

#[derive(Debug, Serialize, Deserialize)]
pub struct CreatePatientEhr {
    pub first_name: String,
    pub last_name: String,
    #[serde(default)]
    pub email: Option<String>,
    pub phone: String,
    pub date_of_birth: String,
    pub gender: String,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub mrn: Option<String>,
    #[serde(default)]
    pub blood_group: Option<String>,
    #[serde(default)]
    pub allergies: Option<String>,
    #[serde(default)]
    pub chronic_conditions: Option<String>,
    #[serde(default)]
    pub emergency_contact_name: Option<String>,
    #[serde(default)]
    pub emergency_contact_phone: Option<String>,
    #[serde(default)]
    pub insurance_provider: Option<String>,
    #[serde(default)]
    pub insurance_policy_number: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdatePatientEhr {
    pub id: i32,
    pub first_name: String,
    pub last_name: String,
    #[serde(default)]
    pub email: Option<String>,
    pub phone: String,
    pub date_of_birth: String,
    pub gender: String,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub mrn: Option<String>,
    #[serde(default)]
    pub blood_group: Option<String>,
    #[serde(default)]
    pub allergies: Option<String>,
    #[serde(default)]
    pub chronic_conditions: Option<String>,
    #[serde(default)]
    pub emergency_contact_name: Option<String>,
    #[serde(default)]
    pub emergency_contact_phone: Option<String>,
    #[serde(default)]
    pub insurance_provider: Option<String>,
    #[serde(default)]
    pub insurance_policy_number: Option<String>,
    pub status: String,
}

// ── Patient consent (CR-12, SRS FR-0035) ─────────────────────────────────
//
// Tracks the patient's explicit consent for each category of PHI use
// (e.g. "whatsapp", "marketing", "research"). The `whatsapp` consent type
// gates outbound WhatsApp messages — see `whatsapp::automation::send_whatsapp`.
// (patient_id, consent_type) is UNIQUE in the DB so each patient has at most
// one row per consent type, enabling clean upserts.

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct PatientConsent {
    pub id: i32,
    pub patient_id: i32,
    pub consent_type: String,
    pub granted: bool,
    pub granted_at: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub granted_by_user_id: Option<i32>,
    #[serde(default)]
    pub notes: Option<String>,
}

// ── Encounter / visit ────────────────────────────────────────────────────

// Used as a Tauri command return type via #[tauri::command] — clippy
// doesn't see macro-expanded usage, so flag as allowed dead code.
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct Encounter {
    pub id: i32,
    pub patient_id: i32,
    #[serde(default)]
    pub doctor_id: Option<i32>,
    pub visit_type: String,
    pub visit_date: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub chief_complaint: Option<String>,
    #[serde(default)]
    pub diagnosis: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub created_by_user_id: Option<i32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateEncounter {
    pub patient_id: i32,
    #[serde(default)]
    pub doctor_id: Option<i32>,
    #[serde(default)]
    pub visit_type: Option<String>,
    #[serde(default)]
    pub chief_complaint: Option<String>,
    #[serde(default)]
    pub diagnosis: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

// ── Queue ────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct QueueToken {
    pub id: i32,
    pub patient_id: i32,
    #[serde(default)]
    pub department_id: Option<i32>,
    #[serde(default)]
    pub doctor_id: Option<i32>,
    pub token_number: i32,
    pub status: String,
    pub priority: i16,
    pub issued_at: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub called_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub patient_name: Option<String>,
    #[serde(default)]
    pub doctor_name: Option<String>,
    #[serde(default)]
    pub department_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateQueueToken {
    pub patient_id: i32,
    #[serde(default)]
    pub department_id: Option<i32>,
    #[serde(default)]
    pub doctor_id: Option<i32>,
    #[serde(default)]
    pub priority: Option<i16>,
}

// ── IPD ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct Ward {
    pub id: i32,
    pub name: String,
    pub code: String,
    #[serde(default)]
    pub floor: Option<String>,
    #[serde(default)]
    pub gender_restriction: Option<String>,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct Bed {
    pub id: i32,
    pub ward_id: i32,
    pub bed_number: String,
    pub status: String,
    pub is_icu: bool,
    pub daily_rate: rust_decimal::Decimal,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct IpdAdmission {
    pub id: i32,
    pub patient_id: i32,
    #[serde(default)]
    pub doctor_id: Option<i32>,
    pub ward_id: i32,
    pub bed_id: i32,
    pub admission_date: chrono::DateTime<chrono::Utc>,
    pub admission_type: String,
    #[serde(default)]
    pub admitting_diagnosis: Option<String>,
    #[serde(default)]
    pub attending_doctor_id: Option<i32>,
    pub status: String,
    #[serde(default)]
    pub discharge_date: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub discharge_summary: Option<String>,
    #[serde(default)]
    pub created_by_user_id: Option<i32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    // Joined
    #[serde(default)]
    pub patient_name: Option<String>,
    #[serde(default)]
    pub doctor_name: Option<String>,
    #[serde(default)]
    pub ward_name: Option<String>,
    #[serde(default)]
    pub bed_number: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateIpdAdmission {
    pub patient_id: i32,
    #[serde(default)]
    pub doctor_id: Option<i32>,
    pub ward_id: i32,
    pub bed_id: i32,
    #[serde(default)]
    pub admission_type: Option<String>,
    #[serde(default)]
    pub admitting_diagnosis: Option<String>,
    #[serde(default)]
    pub attending_doctor_id: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DischargeIpd {
    pub id: i32,
    #[serde(default)]
    pub discharge_summary: Option<String>,
}

// ── Laboratory ───────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct LabTestCatalog {
    pub id: i32,
    pub name: String,
    pub code: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub sample_type: Option<String>,
    #[serde(default)]
    pub normal_range: Option<String>,
    #[serde(default)]
    pub unit: Option<String>,
    pub price: rust_decimal::Decimal,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct LabOrder {
    pub id: i32,
    pub patient_id: i32,
    #[serde(default)]
    pub encounter_id: Option<i32>,
    #[serde(default)]
    pub ordered_by_doctor_id: Option<i32>,
    #[serde(default)]
    pub ordered_by_user_id: Option<i32>,
    pub status: String,
    pub ordered_at: chrono::DateTime<chrono::Utc>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub patient_name: Option<String>,
    #[serde(default)]
    pub doctor_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct LabOrderTest {
    pub id: i32,
    pub lab_order_id: i32,
    pub test_catalog_id: i32,
    #[serde(default)]
    pub result_value: Option<String>,
    #[serde(default)]
    pub result_unit: Option<String>,
    #[serde(default)]
    pub result_abnormal_flag: Option<String>,
    #[serde(default)]
    pub result_notes: Option<String>,
    #[serde(default)]
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub completed_by_user_id: Option<i32>,
    #[serde(default)]
    pub test_name: Option<String>,
    #[serde(default)]
    pub test_code: Option<String>,
    #[serde(default)]
    pub normal_range: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateLabOrder {
    pub patient_id: i32,
    #[serde(default)]
    pub encounter_id: Option<i32>,
    #[serde(default)]
    pub ordered_by_doctor_id: Option<i32>,
    pub test_catalog_ids: Vec<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateLabResult {
    pub id: i32,
    #[serde(default)]
    pub result_value: Option<String>,
    #[serde(default)]
    pub result_unit: Option<String>,
    #[serde(default)]
    pub result_abnormal_flag: Option<String>,
    #[serde(default)]
    pub result_notes: Option<String>,
}

// ── Billing ──────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct Bill {
    pub id: i32,
    pub patient_id: i32,
    #[serde(default)]
    pub encounter_id: Option<i32>,
    #[serde(default)]
    pub ipd_admission_id: Option<i32>,
    pub bill_number: String,
    pub bill_type: String,
    pub total_amount: rust_decimal::Decimal,
    pub discount: rust_decimal::Decimal,
    pub tax: rust_decimal::Decimal,
    pub net_amount: rust_decimal::Decimal,
    pub status: String,
    #[serde(default)]
    pub created_by_user_id: Option<i32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub patient_name: Option<String>,
    #[serde(default)]
    pub amount_paid: Option<rust_decimal::Decimal>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct BillItem {
    pub id: i32,
    pub bill_id: i32,
    pub item_type: String,
    pub description: String,
    pub quantity: rust_decimal::Decimal,
    pub unit_price: rust_decimal::Decimal,
    pub total: rust_decimal::Decimal,
    #[serde(default)]
    pub reference_id: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BillItemInput {
    pub item_type: String,
    pub description: String,
    pub quantity: f64,
    pub unit_price: f64,
    #[serde(default)]
    pub reference_id: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateBill {
    pub patient_id: i32,
    #[serde(default)]
    pub encounter_id: Option<i32>,
    #[serde(default)]
    pub ipd_admission_id: Option<i32>,
    #[serde(default)]
    pub bill_type: Option<String>,
    #[serde(default)]
    pub discount: Option<f64>,
    #[serde(default)]
    pub tax: Option<f64>,
    pub items: Vec<BillItemInput>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct Payment {
    pub id: i32,
    pub bill_id: i32,
    pub amount: rust_decimal::Decimal,
    pub payment_method: String,
    #[serde(default)]
    pub reference_number: Option<String>,
    pub paid_at: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub received_by_user_id: Option<i32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreatePayment {
    pub bill_id: i32,
    pub amount: f64,
    #[serde(default)]
    pub payment_method: Option<String>,
    #[serde(default)]
    pub reference_number: Option<String>,
}

// ── Inventory (CR-21, SRS FR-0180/0181/0185) ──────────────────────────────
//
// Stock is stored as NUMERIC(14,2) and round-tripped via `rust_decimal`,
// matching the billing pattern. All stock changes go through the
// `adjust_inventory` command, which atomically updates `stock_quantity` and
// writes a row to `inventory_movements` with the resulting balance snapshot.

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct InventoryItem {
    pub id: i32,
    pub name: String,
    #[serde(default)]
    pub sku: Option<String>,
    pub category: String,
    #[serde(default)]
    pub unit: Option<String>,
    pub stock_quantity: rust_decimal::Decimal,
    pub reorder_level: rust_decimal::Decimal,
    #[serde(default)]
    pub expiry_date: Option<NaiveDate>,
    #[serde(default)]
    pub batch_number: Option<String>,
    pub unit_cost: rust_decimal::Decimal,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateInventoryItem {
    pub name: String,
    #[serde(default)]
    pub sku: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub stock_quantity: Option<f64>,
    #[serde(default)]
    pub reorder_level: Option<f64>,
    #[serde(default)]
    pub expiry_date: Option<String>,
    #[serde(default)]
    pub batch_number: Option<String>,
    #[serde(default)]
    pub unit_cost: Option<f64>,
    #[serde(default)]
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateInventoryItem {
    pub id: i32,
    pub name: String,
    #[serde(default)]
    pub sku: Option<String>,
    pub category: String,
    #[serde(default)]
    pub unit: Option<String>,
    pub stock_quantity: f64,
    pub reorder_level: f64,
    #[serde(default)]
    pub expiry_date: Option<String>,
    #[serde(default)]
    pub batch_number: Option<String>,
    pub unit_cost: f64,
    pub is_active: bool,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct InventoryMovement {
    pub id: i32,
    pub item_id: i32,
    pub quantity_change: rust_decimal::Decimal,
    pub reason: String,
    pub balance_after: rust_decimal::Decimal,
    #[serde(default)]
    pub reference_id: Option<i32>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub created_by_user_id: Option<i32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// ── Dashboard KPIs ───────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardKpis {
    pub patients_total: i64,
    pub appointments_today: i64,
    pub appointments_scheduled: i64,
    pub appointments_completed: i64,
    pub queue_waiting: i64,
    pub queue_in_progress: i64,
    pub ipd_admitted: i64,
    pub beds_available: i64,
    pub beds_total: i64,
    pub revenue_today: f64,
    pub revenue_month: f64,
    pub pending_lab_orders: i64,
    pub staff_on_duty: i64,
}

// ── Reports (SRS §4.20 — Phase 2) ────────────────────────────────────────
//
// These structs are the typed return values of the five report commands in
// `commands/reports.rs`. They are NOT mapped 1:1 to a single DB table — each
// aggregates rows from several existing tables (bills, payments, patients,
// appointments, lab_orders, ipd_admissions, beds) on the server side and
// ships the rolled-up result to the frontend, which renders charts/CSV.
//
// `#[allow(dead_code)]` is added defensively on every struct: clippy doesn't
// see `#[tauri::command]` macro-expanded usage, and these structs are only
// ever constructed inside their respective report commands (which themselves
// are referenced by `generate_handler!`).

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct RevenueByDate {
    pub date: String,
    pub billed: f64,
    pub paid: f64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct RevenueReport {
    pub total_billed: f64,
    pub total_paid: f64,
    pub total_outstanding: f64,
    pub bill_count: i64,
    pub payment_count: i64,
    pub by_date: Vec<RevenueByDate>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct PatientDemographicsBucket {
    pub label: String,
    pub count: i64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct PatientDemographicsReport {
    pub total_patients: i64,
    pub by_gender: Vec<PatientDemographicsBucket>,
    pub by_age_group: Vec<PatientDemographicsBucket>,
    pub new_registrations_in_range: i64,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct AppointmentByBucket {
    pub label: String,
    pub count: i64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct AppointmentReport {
    pub total: i64,
    pub by_status: Vec<AppointmentByBucket>,
    pub by_doctor: Vec<AppointmentByBucket>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct LabReport {
    pub total_orders: i64,
    pub total_tests: i64,
    pub by_status: Vec<AppointmentByBucket>,
    pub by_test: Vec<AppointmentByBucket>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct IpdWardUtilization {
    pub ward_id: i32,
    pub ward_name: String,
    pub total_beds: i64,
    pub occupied_beds: i64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct IpdOccupancyReport {
    pub current_admissions: i64,
    pub total_beds: i64,
    pub occupied_beds: i64,
    pub available_beds: i64,
    pub occupancy_rate: f64,
    pub average_stay_hours: f64,
    pub by_ward: Vec<IpdWardUtilization>,
}

// ── Backup & Restore (SRS §9 A-07 — Phase 2) ────────────────────────────────
//
// Returned by `list_backups` and `create_backup`. The fields are populated
// from filesystem metadata (filename, path, size, creation time); there is
// no `backups` table — backups are `.sql` files (PostgreSQL custom-format
// archives produced by `pg_dump -Fc`) on disk under
// `%ProgramData%\HMS\backups\`.
//
// `path` is the absolute filesystem path to the backup file — surfaced to
// the frontend so the Settings → Backup section can show the operator where
// the file lives (and so `create_backup`'s success toast can include the
// path per the spec wording "Return the backup file path"). It is NEVER
// accepted as an input parameter on `restore_backup` / `delete_backup` —
// those take a bare `backup_filename` and join it to the backups directory
// after path-traversal validation, so a malicious frontend cannot trick
// the backend into overwriting/deleting files outside the backups dir.
//
// `#[allow(dead_code)]` because clippy doesn't see `#[tauri::command]`
// macro-expanded usage (same rationale as the other Tauri-command-return
// structs above).
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct BackupInfo {
    pub filename: String,
    pub path: String,
    pub size_bytes: u64,
    pub created_at: String,
}

// ── Pharmacy (Phase 2-C, SRS FR-0120–FR-0124) ──────────────────────────────
//
// Three Rust structs mirror the three pharmacy tables created in
// `db.rs::run_migrations`:
//   - `Medication`           ← `medications`           (FR-0120 catalog)
//   - `Prescription`         ← `prescriptions`         (FR-0121 header)
//   - `PrescriptionItem`     ← `prescription_items`    (FR-0121 line items)
//
// `Prescription` carries joined patient/doctor names (LEFT JOINed in the
// SELECT query, same pattern as `LabOrder`) so the frontend list view can
// render a row without a second round-trip per prescription.
//
// `CreatePrescriptionItem` is the wire shape for one medication line in
// `create_prescription`'s `items: Vec<CreatePrescriptionItem>` parameter;
// the backend snapshots `medication_name` + `is_controlled` from the
// referenced `medication_id` at insert time so the prescription remains
// readable even if the medication is later soft-deleted.
//
// `#[allow(dead_code)]` because clippy doesn't see `#[tauri::command]`
// macro-expanded usage (same rationale as the other Tauri-command-return
// structs above).

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct Medication {
    pub id: i32,
    pub brand_name: String,
    pub generic_name: String,
    pub form: String,
    pub strength: String,
    pub schedule: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub manufacturer: Option<String>,
    pub reorder_level: i32,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateMedication {
    pub brand_name: String,
    pub generic_name: String,
    #[serde(default)]
    pub form: Option<String>,
    pub strength: String,
    #[serde(default)]
    pub schedule: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub manufacturer: Option<String>,
    #[serde(default)]
    pub reorder_level: Option<i32>,
    #[serde(default)]
    pub is_active: Option<bool>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateMedication {
    pub id: i32,
    pub brand_name: String,
    pub generic_name: String,
    pub form: String,
    pub strength: String,
    pub schedule: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub manufacturer: Option<String>,
    pub reorder_level: i32,
    pub is_active: bool,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct Prescription {
    pub id: i32,
    pub patient_id: i32,
    #[serde(default)]
    pub doctor_id: Option<i32>,
    #[serde(default)]
    pub encounter_id: Option<i32>,
    #[serde(default)]
    pub prescribed_by_user_id: Option<i32>,
    pub status: String,
    #[serde(default)]
    pub notes: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    // Joined fields (LEFT JOIN patients + doctors), nullable like LabOrder.
    #[serde(default)]
    pub patient_name: Option<String>,
    #[serde(default)]
    pub doctor_name: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct PrescriptionItem {
    pub id: i32,
    pub prescription_id: i32,
    #[serde(default)]
    pub medication_id: Option<i32>,
    pub medication_name: String,
    pub dose: String,
    pub route: String,
    pub frequency: String,
    #[serde(default)]
    pub duration: Option<String>,
    pub quantity: i32,
    pub is_controlled: bool,
    pub dispensed: bool,
    #[serde(default)]
    pub dispensed_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub dispensed_by_user_id: Option<i32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreatePrescriptionItem {
    pub medication_id: Option<i32>,
    pub medication_name: String,
    pub dose: String,
    #[serde(default)]
    pub route: Option<String>,
    pub frequency: String,
    #[serde(default)]
    pub duration: Option<String>,
    #[serde(default)]
    pub quantity: Option<i32>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreatePrescription {
    pub patient_id: i32,
    #[serde(default)]
    pub doctor_id: Option<i32>,
    #[serde(default)]
    pub encounter_id: Option<i32>,
    pub items: Vec<CreatePrescriptionItem>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// Returned by `get_prescription(id)` — the prescription header plus its
/// line items in one round-trip.
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct PrescriptionWithItems {
    #[serde(flatten)]
    pub prescription: Prescription,
    pub items: Vec<PrescriptionItem>,
}

// ── Radiology (Phase 2-D, SRS FR-0140–FR-0142) ──────────────────────────────
//
// Three Rust structs mirror the four radiology tables created in
// `db.rs::run_migrations` (the `radiology_attachments` table is intentionally
// NOT modelled here — Phase 2-D does not surface attachments in any command;
// they are managed out-of-band by the PACS bridge stub):
//   - `RadiologyOrder`         ← `radiology_orders`           (FR-0140 order)
//   - `RadiologyReport`        ← `radiology_reports`          (FR-0141 report)
//   - `RadiologyStatusHistory` ← `radiology_status_history`   (audit-trail)
//
// `RadiologyOrder` carries joined patient / doctor / radiologist names via
// LEFT JOINs (same pattern as `LabOrder` + `Prescription`) so the frontend
// list view renders a row without a second round-trip per order.
//
// `#[allow(dead_code)]` because clippy doesn't see `#[tauri::command]`
// macro-expanded usage (same rationale as the other Tauri-command-return
// structs above).

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct RadiologyOrder {
    pub id: i32,
    pub patient_id: i32,
    #[serde(default)]
    pub encounter_id: Option<i32>,
    #[serde(default)]
    pub ordered_by_doctor_id: Option<i32>,
    #[serde(default)]
    pub ordered_by_user_id: Option<i32>,
    pub order_number: String,
    #[serde(default)]
    pub department: Option<String>,
    #[serde(default)]
    pub clinical_indication: Option<String>,
    #[serde(default)]
    pub symptoms: Option<String>,
    #[serde(default)]
    pub diagnosis: Option<String>,
    pub priority: String,
    pub study_type: String,
    pub contrast_required: bool,
    #[serde(default)]
    pub body_part: Option<String>,
    #[serde(default)]
    pub instructions: Option<String>,
    pub status: String,
    #[serde(default)]
    pub assigned_radiologist_id: Option<i32>,
    #[serde(default)]
    pub assigned_technician: Option<String>,
    #[serde(default)]
    pub expected_date: Option<NaiveDate>,
    pub ordered_at: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub scheduled_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub reported_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub verified_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    // P0-5: soft-delete fields
    #[serde(default)]
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    // Joined fields (LEFT JOIN patients + doctors × 2 for ordering doctor +
    // assigned radiologist), nullable like LabOrder/Prescription.
    #[serde(default)]
    pub patient_name: Option<String>,
    #[serde(default)]
    pub doctor_name: Option<String>,
    #[serde(default)]
    pub radiologist_name: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateRadiologyOrder {
    pub patient_id: i32,
    #[serde(default)]
    pub encounter_id: Option<i32>,
    #[serde(default)]
    pub ordered_by_doctor_id: Option<i32>,
    #[serde(default)]
    pub department: Option<String>,
    #[serde(default)]
    pub clinical_indication: Option<String>,
    #[serde(default)]
    pub symptoms: Option<String>,
    #[serde(default)]
    pub diagnosis: Option<String>,
    pub priority: String,
    pub study_type: String,
    pub contrast_required: bool,
    #[serde(default)]
    pub body_part: Option<String>,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub assigned_radiologist_id: Option<i32>,
    #[serde(default)]
    pub assigned_technician: Option<String>,
    #[serde(default)]
    pub expected_date: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct RadiologyReport {
    pub id: i32,
    pub order_id: i32,
    #[serde(default)]
    pub findings: Option<String>,
    #[serde(default)]
    pub impression: Option<String>,
    #[serde(default)]
    pub recommendations: Option<String>,
    pub critical_finding: bool,
    #[serde(default)]
    pub radiologist_id: Option<i32>,
    #[serde(default)]
    pub verified_by_user_id: Option<i32>,
    pub report_date: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub verified_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateRadiologyReport {
    pub order_id: i32,
    #[serde(default)]
    pub findings: Option<String>,
    #[serde(default)]
    pub impression: Option<String>,
    #[serde(default)]
    pub recommendations: Option<String>,
    #[serde(default)]
    pub critical_finding: Option<bool>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct RadiologyStatusHistory {
    pub id: i32,
    pub order_id: i32,
    pub status: String,
    #[serde(default)]
    pub changed_by_user_id: Option<i32>,
    pub changed_at: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub notes: Option<String>,
}

// ── Blood Bank (Phase 2-E, SRS FR-0145–FR-0149) ──────────────────────────────
//
// Rust structs mirror the blood-bank tables created in `db.rs::run_migrations`.
// One struct per table that is read back to the frontend; write payloads use
// dedicated `Create*` structs (mirrors the radiology convention).
//
//   - `BloodDonor`        ← `blood_donors`            (FR-0146)
//   - `BloodDonation`     ← `blood_donations`         (FR-0146)
//   - `BloodUnit`         ← `blood_units`             (FR-0145)
//   - `BloodCrossmatch`   ← `blood_crossmatch_results`(FR-0147)
//   - `BloodReservation`  ← `blood_reservations`      (FR-0147)
//   - `BloodIssue`        ← `blood_issues`            (FR-0148)
//   - `BloodTransfusion`  ← `blood_transfusions`      (FR-0148)
//   - `BloodDiscard`      ← `blood_discards`          (FR-0149)
//   - `BloodUnitHistory`  ← `blood_unit_status_history`(FR-0149)
//   - `BloodMovement`     ← `blood_inventory_movements`(FR-0149)

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct BloodDonor {
    pub id: i32,
    pub donor_number: String,
    #[serde(default)]
    pub patient_id: Option<i32>,
    pub first_name: String,
    pub last_name: String,
    #[serde(default)]
    pub date_of_birth: Option<NaiveDate>,
    #[serde(default)]
    pub gender: Option<String>,
    pub blood_group: String,
    pub rh_factor: String,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub weight_kg: Option<rust_decimal::Decimal>,
    #[serde(default)]
    pub height_cm: Option<rust_decimal::Decimal>,
    #[serde(default)]
    pub last_donation_date: Option<NaiveDate>,
    pub total_donations: i32,
    pub status: String,
    #[serde(default)]
    pub medically_deferred_until: Option<NaiveDate>,
    #[serde(default)]
    pub defer_reason: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub created_by_user_id: Option<i32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    // Joined fields
    #[serde(default)]
    pub patient_name: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateBloodDonor {
    #[serde(default)]
    pub patient_id: Option<i32>,
    pub first_name: String,
    pub last_name: String,
    #[serde(default)]
    pub date_of_birth: Option<String>,
    #[serde(default)]
    pub gender: Option<String>,
    pub blood_group: String,
    pub rh_factor: String,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub weight_kg: Option<f64>,
    #[serde(default)]
    pub height_cm: Option<f64>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct BloodDonation {
    pub id: i32,
    pub donation_number: String,
    pub donor_id: i32,
    pub donation_date: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub collection_site: Option<String>,
    #[serde(default)]
    pub collected_by_user_id: Option<i32>,
    pub volume_ml: i32,
    pub blood_group: String,
    pub rh_factor: String,
    #[serde(default)]
    pub bag_type: Option<String>,
    pub status: String,
    pub screening_status: String,
    #[serde(default)]
    pub screening_notes: Option<String>,
    #[serde(default)]
    pub screened_by_user_id: Option<i32>,
    #[serde(default)]
    pub screened_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub hemoglobin_level: Option<rust_decimal::Decimal>,
    #[serde(default)]
    pub blood_pressure: Option<String>,
    #[serde(default)]
    pub pulse: Option<i32>,
    #[serde(default)]
    pub temperature_c: Option<rust_decimal::Decimal>,
    #[serde(default)]
    pub notes: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    // Joined fields
    #[serde(default)]
    pub donor_name: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateBloodDonation {
    pub donor_id: i32,
    #[serde(default)]
    pub collection_site: Option<String>,
    pub volume_ml: i32,
    pub blood_group: String,
    pub rh_factor: String,
    #[serde(default)]
    pub bag_type: Option<String>,
    #[serde(default)]
    pub hemoglobin_level: Option<f64>,
    #[serde(default)]
    pub blood_pressure: Option<String>,
    #[serde(default)]
    pub pulse: Option<i32>,
    #[serde(default)]
    pub temperature_c: Option<f64>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct BloodUnit {
    pub id: i32,
    pub unit_number: String,
    #[serde(default)]
    pub donation_id: Option<i32>,
    pub donor_id: i32,
    pub component_type: String,
    pub blood_group: String,
    pub rh_factor: String,
    pub volume_ml: i32,
    pub collection_date: chrono::DateTime<chrono::Utc>,
    pub expiry_date: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub storage_temperature: Option<String>,
    #[serde(default)]
    pub storage_location: Option<String>,
    pub status: String,
    #[serde(default)]
    pub reserved_for_patient_id: Option<i32>,
    #[serde(default)]
    pub reservation_id: Option<i32>,
    #[serde(default)]
    pub issued_to_patient_id: Option<i32>,
    #[serde(default)]
    pub issued_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub transfused_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub transfused_to_patient_id: Option<i32>,
    #[serde(default)]
    pub discarded_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub discard_reason: Option<String>,
    #[serde(default)]
    pub created_by_user_id: Option<i32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    // Joined fields
    #[serde(default)]
    pub donor_name: Option<String>,
    #[serde(default)]
    pub patient_name: Option<String>,
    #[serde(default)]
    pub days_to_expiry: Option<i32>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateBloodUnit {
    pub donor_id: i32,
    #[serde(default)]
    pub donation_id: Option<i32>,
    pub component_type: String,
    pub blood_group: String,
    pub rh_factor: String,
    pub volume_ml: i32,
    #[serde(default)]
    pub storage_temperature: Option<String>,
    #[serde(default)]
    pub storage_location: Option<String>,
    pub expiry_date: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct BloodCrossmatch {
    pub id: i32,
    pub unit_id: i32,
    pub patient_id: i32,
    #[serde(default)]
    pub doctor_id: Option<i32>,
    #[serde(default)]
    pub requested_by_user_id: Option<i32>,
    pub crossmatch_date: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub method: Option<String>,
    pub result: String,
    #[serde(default)]
    pub reaction_grade: Option<i32>,
    #[serde(default)]
    pub incubation_time_min: Option<i32>,
    #[serde(default)]
    pub ahg_phase: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub performed_by_user_id: Option<i32>,
    #[serde(default)]
    pub verified_by_user_id: Option<i32>,
    #[serde(default)]
    pub verified_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    // Joined fields
    #[serde(default)]
    pub unit_number: Option<String>,
    #[serde(default)]
    pub patient_name: Option<String>,
    #[serde(default)]
    pub doctor_name: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateBloodCrossmatch {
    pub unit_id: i32,
    pub patient_id: i32,
    #[serde(default)]
    pub doctor_id: Option<i32>,
    #[serde(default)]
    pub method: Option<String>,
    pub result: String,
    #[serde(default)]
    pub reaction_grade: Option<i32>,
    #[serde(default)]
    pub incubation_time_min: Option<i32>,
    #[serde(default)]
    pub ahg_phase: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct BloodReservation {
    pub id: i32,
    pub reservation_number: String,
    pub unit_id: i32,
    pub patient_id: i32,
    #[serde(default)]
    pub doctor_id: Option<i32>,
    #[serde(default)]
    pub requested_by_user_id: Option<i32>,
    #[serde(default)]
    pub crossmatch_id: Option<i32>,
    pub reserved_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub fulfilled_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub cancelled_at: Option<chrono::DateTime<chrono::Utc>>,
    pub status: String,
    pub priority: String,
    #[serde(default)]
    pub clinical_indication: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    // Joined fields
    #[serde(default)]
    pub unit_number: Option<String>,
    #[serde(default)]
    pub patient_name: Option<String>,
    #[serde(default)]
    pub doctor_name: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateBloodReservation {
    pub unit_id: i32,
    pub patient_id: i32,
    #[serde(default)]
    pub doctor_id: Option<i32>,
    #[serde(default)]
    pub crossmatch_id: Option<i32>,
    pub priority: String,
    pub expires_in_hours: i32,
    #[serde(default)]
    pub clinical_indication: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct BloodIssue {
    pub id: i32,
    pub issue_number: String,
    pub unit_id: i32,
    pub patient_id: i32,
    #[serde(default)]
    pub reservation_id: Option<i32>,
    #[serde(default)]
    pub crossmatch_id: Option<i32>,
    #[serde(default)]
    pub doctor_id: Option<i32>,
    #[serde(default)]
    pub issued_by_user_id: Option<i32>,
    pub issued_at: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub issued_to_location: Option<String>,
    pub issue_type: String,
    #[serde(default)]
    pub clinical_indication: Option<String>,
    #[serde(default)]
    pub special_instructions: Option<String>,
    #[serde(default)]
    pub returned_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub return_reason: Option<String>,
    #[serde(default)]
    pub received_by_user_id: Option<i32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    // Joined fields
    #[serde(default)]
    pub unit_number: Option<String>,
    #[serde(default)]
    pub patient_name: Option<String>,
    #[serde(default)]
    pub doctor_name: Option<String>,
    #[serde(default)]
    pub issued_by_name: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateBloodIssue {
    pub unit_id: i32,
    pub patient_id: i32,
    #[serde(default)]
    pub reservation_id: Option<i32>,
    #[serde(default)]
    pub crossmatch_id: Option<i32>,
    #[serde(default)]
    pub doctor_id: Option<i32>,
    #[serde(default)]
    pub issued_to_location: Option<String>,
    pub issue_type: String,
    #[serde(default)]
    pub clinical_indication: Option<String>,
    #[serde(default)]
    pub special_instructions: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct BloodTransfusion {
    pub id: i32,
    pub transfusion_number: String,
    pub issue_id: i32,
    pub unit_id: i32,
    pub patient_id: i32,
    #[serde(default)]
    pub doctor_id: Option<i32>,
    #[serde(default)]
    pub nurse_id: Option<i32>,
    pub started_at: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub volume_transfused_ml: Option<i32>,
    #[serde(default)]
    pub pre_transfusion_bp: Option<String>,
    #[serde(default)]
    pub post_transfusion_bp: Option<String>,
    #[serde(default)]
    pub pre_transfusion_temp: Option<rust_decimal::Decimal>,
    #[serde(default)]
    pub post_transfusion_temp: Option<rust_decimal::Decimal>,
    #[serde(default)]
    pub pre_transfusion_pulse: Option<i32>,
    #[serde(default)]
    pub post_transfusion_pulse: Option<i32>,
    pub reaction_observed: bool,
    #[serde(default)]
    pub reaction_type: Option<String>,
    #[serde(default)]
    pub reaction_severity: Option<String>,
    #[serde(default)]
    pub reaction_notes: Option<String>,
    #[serde(default)]
    pub outcome: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    // Joined fields
    #[serde(default)]
    pub unit_number: Option<String>,
    #[serde(default)]
    pub patient_name: Option<String>,
    #[serde(default)]
    pub doctor_name: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateBloodTransfusion {
    pub issue_id: i32,
    pub unit_id: i32,
    pub patient_id: i32,
    #[serde(default)]
    pub doctor_id: Option<i32>,
    #[serde(default)]
    pub nurse_id: Option<i32>,
    #[serde(default)]
    pub volume_transfused_ml: Option<i32>,
    #[serde(default)]
    pub pre_transfusion_bp: Option<String>,
    #[serde(default)]
    pub post_transfusion_bp: Option<String>,
    #[serde(default)]
    pub pre_transfusion_temp: Option<f64>,
    #[serde(default)]
    pub post_transfusion_temp: Option<f64>,
    #[serde(default)]
    pub pre_transfusion_pulse: Option<i32>,
    #[serde(default)]
    pub post_transfusion_pulse: Option<i32>,
    pub reaction_observed: bool,
    #[serde(default)]
    pub reaction_type: Option<String>,
    #[serde(default)]
    pub reaction_severity: Option<String>,
    #[serde(default)]
    pub reaction_notes: Option<String>,
    #[serde(default)]
    pub outcome: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct BloodDiscard {
    pub id: i32,
    pub unit_id: i32,
    pub discard_number: String,
    pub discarded_at: chrono::DateTime<chrono::Utc>,
    pub discard_reason: String,
    #[serde(default)]
    pub discard_notes: Option<String>,
    #[serde(default)]
    pub discarded_by_user_id: Option<i32>,
    #[serde(default)]
    pub authorized_by_user_id: Option<i32>,
    #[serde(default)]
    pub disposal_method: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    // Joined fields
    #[serde(default)]
    pub unit_number: Option<String>,
    #[serde(default)]
    pub discarded_by_name: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateBloodDiscard {
    pub unit_id: i32,
    pub discard_reason: String,
    #[serde(default)]
    pub discard_notes: Option<String>,
    #[serde(default)]
    pub disposal_method: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct BloodUnitHistory {
    pub id: i32,
    pub unit_id: i32,
    pub status: String,
    #[serde(default)]
    pub changed_by_user_id: Option<i32>,
    pub changed_at: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub related_record_type: Option<String>,
    #[serde(default)]
    pub related_record_id: Option<i32>,
    // Joined fields
    #[serde(default)]
    pub changed_by_name: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct BloodMovement {
    pub id: i32,
    pub unit_id: i32,
    pub movement_type: String,
    #[serde(default)]
    pub from_location: Option<String>,
    #[serde(default)]
    pub to_location: Option<String>,
    #[serde(default)]
    pub moved_by_user_id: Option<i32>,
    pub moved_at: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub related_record_type: Option<String>,
    #[serde(default)]
    pub related_record_id: Option<i32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    // Joined fields
    #[serde(default)]
    pub unit_number: Option<String>,
    #[serde(default)]
    pub moved_by_name: Option<String>,
}
