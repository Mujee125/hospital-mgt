/**
 * Canonical TypeScript shapes for every backend model, matching the Rust
 * structs in src-tauri/src/models.rs field-for-field. Previously these
 * were redefined ad hoc in each page (Patients.tsx, Doctors.tsx,
 * Appointments.tsx, Dashboard.tsx), which had already drifted — one copy
 * of AppointmentWithDetails was missing created_at/updated_at, which
 * caused a real bug earlier. Import from here instead of redefining.
 */

export interface Patient {
  id: number;
  first_name: string;
  last_name: string;
  email: string | null;
  phone: string;
  date_of_birth: string;
  gender: string;
  address: string | null;
  created_at: string;
}

export interface Doctor {
  id: number;
  first_name: string;
  last_name: string;
  email: string | null;
  phone: string;
  specialization: string;
  qualification: string;
  available_from: string; // "HH:MM:SS"
  available_to: string;
  is_active: boolean;
  created_at: string;
}

export interface Appointment {
  id: number;
  patient_id: number;
  doctor_id: number;
  appointment_date: string;
  appointment_time: string;
  duration_minutes: number;
  status: string;
  reason: string | null;
  notes: string | null;
  created_at: string;
  updated_at: string;
}

export interface AppointmentWithDetails {
  id: number;
  patient_id: number;
  doctor_id: number;
  appointment_date: string;
  appointment_time: string;
  duration_minutes: number;
  status: string;
  reason: string | null;
  notes: string | null;
  created_at: string;
  updated_at: string;
  patient_first_name: string;
  patient_last_name: string;
  doctor_first_name: string;
  doctor_last_name: string;
  doctor_specialization: string;
}

export interface AppointmentStats {
  total: number;
  scheduled: number;
  confirmed: number;
  completed: number;
  cancelled: number;
  no_show: number;
}

export interface ChatMessage {
  id: string;
  sender: string;
  content: string;
  room: string;
  created_at: string;
}

// ── EHR-expanded patient ─────────────────────────────────────────────────
export interface PatientEhr {
  id: number;
  first_name: string;
  last_name: string;
  email: string | null;
  phone: string;
  date_of_birth: string;
  gender: string;
  address: string | null;
  created_at: string;
  mrn: string | null;
  blood_group: string | null;
  allergies: string | null;
  chronic_conditions: string | null;
  emergency_contact_name: string | null;
  emergency_contact_phone: string | null;
  insurance_provider: string | null;
  insurance_policy_number: string | null;
  status: string;
  created_by_user_id: number | null;
}

export interface CreatePatientEhr {
  first_name: string;
  last_name: string;
  email: string | null;
  phone: string;
  date_of_birth: string;
  gender: string;
  address: string | null;
  mrn: string | null;
  blood_group: string | null;
  allergies: string | null;
  chronic_conditions: string | null;
  emergency_contact_name: string | null;
  emergency_contact_phone: string | null;
  insurance_provider: string | null;
  insurance_policy_number: string | null;
}

export interface UpdatePatientEhr extends CreatePatientEhr {
  id: number;
  status: string;
}

// ── Encounters ───────────────────────────────────────────────────────────
export interface Encounter {
  id: number;
  patient_id: number;
  doctor_id: number | null;
  visit_type: string;
  visit_date: string;
  chief_complaint: string | null;
  diagnosis: string | null;
  notes: string | null;
  created_by_user_id: number | null;
  created_at: string;
  patient_name: string | null;
}

// ── Queue ────────────────────────────────────────────────────────────────
export interface QueueToken {
  id: number;
  patient_id: number;
  department_id: number | null;
  doctor_id: number | null;
  token_number: number;
  status: string;
  priority: number;
  issued_at: string;
  called_at: string | null;
  completed_at: string | null;
  patient_name: string | null;
  doctor_name: string | null;
  department_name: string | null;
}

// ── IPD ──────────────────────────────────────────────────────────────────
export interface Ward {
  id: number;
  name: string;
  code: string;
  floor: string | null;
  gender_restriction: string | null;
  is_active: boolean;
  created_at: string;
}

export interface Bed {
  id: number;
  ward_id: number;
  bed_number: string;
  status: string;
  is_icu: boolean;
  // FIXME(TYPE-04): money as float — backend should use serde-with-str.
  // See src-tauri/Cargo.toml `rust_decimal = { features = ["serde-with-float"] }`.
  // The DB stores NUMERIC(14,2); rust_decimal serialises as f64 here, which
  // opens the door to IEEE-754 rounding errors at the JS layer. Treat as
  // opaque at the data layer; only convert via `formatMoney` at display.
  daily_rate: number;
  created_at: string;
}

export interface IpdAdmission {
  id: number;
  patient_id: number;
  doctor_id: number | null;
  ward_id: number;
  bed_id: number;
  admission_date: string;
  admission_type: string;
  admitting_diagnosis: string | null;
  attending_doctor_id: number | null;
  status: string;
  discharge_date: string | null;
  discharge_summary: string | null;
  created_by_user_id: number | null;
  created_at: string;
  updated_at: string;
  patient_name: string | null;
  doctor_name: string | null;
  ward_name: string | null;
  bed_number: string | null;
}

// ── Laboratory ───────────────────────────────────────────────────────────
export interface LabTestCatalog {
  id: number;
  name: string;
  code: string;
  category: string | null;
  sample_type: string | null;
  normal_range: string | null;
  unit: string | null;
  // FIXME(TYPE-04): money as float — backend should use serde-with-str.
  price: number;
  is_active: boolean;
  created_at: string;
}

export interface LabOrder {
  id: number;
  patient_id: number;
  encounter_id: number | null;
  ordered_by_doctor_id: number | null;
  ordered_by_user_id: number | null;
  status: string;
  ordered_at: string;
  created_at: string;
  patient_name: string | null;
  doctor_name: string | null;
}

export interface LabOrderTest {
  id: number;
  lab_order_id: number;
  test_catalog_id: number;
  result_value: string | null;
  result_unit: string | null;
  result_abnormal_flag: string | null;
  result_notes: string | null;
  completed_at: string | null;
  completed_by_user_id: number | null;
  test_name: string | null;
  test_code: string | null;
  normal_range: string | null;
}

// ── Billing ──────────────────────────────────────────────────────────────
// FIXME(TYPE-04): all money fields below arrive as f64 because the backend's
// `rust_decimal` is configured with `serde-with-float` (see src-tauri/
// Cargo.toml). The DB stores NUMERIC(14,2); the f64 hop loses precision in
// JS. Treat these as opaque at the data layer; only convert via
// `formatMoney` (src/lib/utils.ts) at the display layer. The proper fix is
// a backend change: `serde-with-float` → `serde-with-str` (one Cargo.toml
// line), which makes Decimal serialise as "123.45" — then re-type these as
// `string`. Tracked as Batch 4 follow-up.
export interface Bill {
  id: number;
  patient_id: number;
  encounter_id: number | null;
  ipd_admission_id: number | null;
  bill_number: string;
  bill_type: string;
  total_amount: number;
  discount: number;
  tax: number;
  net_amount: number;
  status: string;
  created_by_user_id: number | null;
  created_at: string;
  updated_at: string;
  patient_name: string | null;
  amount_paid: number | null;
}

export interface BillItem {
  id: number;
  bill_id: number;
  item_type: string;
  description: string;
  quantity: number;
  // FIXME(TYPE-04): money as float — backend should use serde-with-str.
  unit_price: number;
  total: number;
  reference_id: number | null;
}

export interface Payment {
  id: number;
  bill_id: number;
  // FIXME(TYPE-04): money as float — backend should use serde-with-str.
  amount: number;
  payment_method: string;
  reference_number: string | null;
  paid_at: string;
  received_by_user_id: number | null;
  created_at: string;
}

// ── Dashboard ────────────────────────────────────────────────────────────
export interface DashboardKpis {
  patients_total: number;
  appointments_today: number;
  appointments_scheduled: number;
  appointments_completed: number;
  queue_waiting: number;
  queue_in_progress: number;
  ipd_admitted: number;
  beds_available: number;
  beds_total: number;
  // FIXME(TYPE-04): money as float — backend should use serde-with-str.
  revenue_today: number;
  revenue_month: number;
  pending_lab_orders: number;
  staff_on_duty: number;
}

// ── Audit ────────────────────────────────────────────────────────────────
export interface AuditLog {
  id: number;
  user_id: number | null;
  username: string | null;
  action: string;
  resource: string;
  resource_id: string | null;
  details: unknown;
  ip: string | null;
  created_at: string;
}

// ── Licensing ────────────────────────────────────────────────────────────
export interface LicenseInfo {
  license_id: string;
  hospital_id: string;
  hospital_name: string;
  deployment_id: string;
  product_edition: string;
  enabled_modules: string[];
  issue_date: string;
  expiration_date: string | null;
  maintenance_until: string;
  hardware_fingerprint: string;
  fingerprint_matches: boolean;
  status: string;
}

// ── User management ──────────────────────────────────────────────────────
export interface UserProfile {
  id: number;
  username: string;
  full_name: string;
  email: string | null;
  is_active: boolean;
  must_change_password: boolean;
  last_login_at: string | null;
}

// ── Patient consent (CR-12, SRS FR-0035) ─────────────────────────────────
//
// Tracks the patient's explicit consent for each category of PHI use
// (e.g. "whatsapp", "marketing", "research"). The `whatsapp` consent type
// gates outbound WhatsApp messages — see
// `src-tauri/src/whatsapp/automation.rs::send_whatsapp`.
// `(patient_id, consent_type)` is UNIQUE in the DB so each patient has at
// most one row per consent type, enabling clean upserts.
export interface PatientConsent {
  id: number;
  patient_id: number;
  consent_type: string;
  granted: boolean;
  granted_at: string;
  granted_by_user_id: number | null;
  notes: string | null;
}

// ── Inventory (CR-21, SRS FR-0180/0181/0185) ─────────────────────────────
//
// Stock is stored as NUMERIC(14,2) and round-tripped via `rust_decimal`,
// matching the billing pattern. All stock changes go through
// `adjust_inventory`, which atomically updates `stock_quantity` and writes a
// row to `inventory_movements` with the resulting balance snapshot.

export interface InventoryItem {
  id: number;
  name: string;
  sku: string | null;
  category: string;
  unit: string | null;
  // FIXME(TYPE-04): money/quantity as float — backend should use
  // serde-with-str. Stock is NUMERIC(14,2) on the DB side.
  stock_quantity: number;
  reorder_level: number;
  expiry_date: string | null;
  batch_number: string | null;
  unit_cost: number;
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateInventoryItem {
  name: string;
  sku?: string | null;
  category?: string | null;
  unit?: string | null;
  stock_quantity?: number | null;
  reorder_level?: number | null;
  expiry_date?: string | null;
  batch_number?: string | null;
  unit_cost?: number | null;
  is_active?: boolean | null;
}

export interface UpdateInventoryItem {
  id: number;
  name: string;
  sku?: string | null;
  category: string;
  unit?: string | null;
  stock_quantity: number;
  reorder_level: number;
  expiry_date?: string | null;
  batch_number?: string | null;
  unit_cost: number;
  is_active: boolean;
}

export interface InventoryMovement {
  id: number;
  item_id: number;
  // FIXME(TYPE-04): quantity/balance as float — backend should use
  // serde-with-str.
  quantity_change: number;
  reason: string;
  balance_after: number;
  reference_id: number | null;
  notes: string | null;
  created_by_user_id: number | null;
  created_at: string;
}

// ── WhatsApp notification audit log ───────────────────────────────────────
// Matches the JSON object built in `src-tauri/src/whatsapp/log.rs::fetch_notification_log`
// (id, appointment_id, notification_type, recipient, message, sent_at, success).
export interface NotificationLogEntry {
  id: number;
  appointment_id: number | null;
  notification_type: string;
  recipient: string;
  message: string;
  sent_at: string;
  success: boolean;
}

// ── Reports (SRS §4.20, FR-0220–FR-0223 — Phase 2-A) ───────────────────────
//
// Typed return shapes of the five Reports-module Tauri commands in
// `src-tauri/src/commands/reports.rs`. Each struct is an aggregation of
// existing tables (appointments, encounters, patients, beds, wards,
// ipd_admissions, bills, bill_items, payments, lab_orders,
// lab_order_tests, lab_test_catalog) computed server-side — there is no
// `reports` table; these are read-only views. Match the Rust structs
// defined locally in `commands/reports.rs` field-for-field.
//
// Money fields (`total_billed`, `total_collected`, `total_outstanding`,
// `revenue`, `total` in `RevenueByTypeRow`/`TopBillItemRow`) arrive as
// `number` (f64) via serde. Use `formatMoney` to render them.
//
// `date` / `from_date` / `to_date` are `YYYY-MM-DD` ISO-8601 date strings
// (no time component); when the backend received `null` for the optional
// `date` parameter it substitutes today's UTC date.

// ── 1. Daily OPD report ─────────────────────────────────────────────────────
export interface DailyOpdStatusCount {
  status: string;
  count: number;
}

export interface DailyOpdDoctorCount {
  doctor_name: string;
  appointment_count: number;
}

export interface DailyOpdReport {
  date: string;
  total_appointments: number;
  appointments_by_status: DailyOpdStatusCount[];
  total_encounters: number;
  new_patients: number;
  top_doctors: DailyOpdDoctorCount[];
}

// ── 2. IPD census report ────────────────────────────────────────────────────
export interface IpdCensusWardRow {
  ward_id: number;
  ward_name: string;
  total_beds: number;
  occupied_beds: number;
  available_beds: number;
}

export interface IpdCensusReport {
  date: string;
  total_beds: number;
  available_beds: number;
  occupied_beds: number;
  maintenance_beds: number;
  current_admissions: number;
  discharges_today: number;
  by_ward: IpdCensusWardRow[];
}

// ── 3. Revenue report ───────────────────────────────────────────────────────
export interface BillStatusCount {
  status: string;
  count: number;
}

export interface RevenueByTypeRow {
  bill_type: string;
  total: number;
}

export interface TopBillItemRow {
  description: string;
  revenue: number;
}

export interface RevenueReport {
  from_date: string;
  to_date: string;
  total_billed: number;
  total_collected: number;
  total_outstanding: number;
  bill_count_by_status: BillStatusCount[];
  revenue_by_type: RevenueByTypeRow[];
  top_bill_items: TopBillItemRow[];
}

// ── 4. Lab turnaround report ────────────────────────────────────────────────
export interface LabOrderStatusCount {
  status: string;
  count: number;
}

export interface TopLabTestRow {
  test_name: string;
  order_count: number;
}

export interface LabTurnaroundReport {
  from_date: string;
  to_date: string;
  total_orders: number;
  orders_by_status: LabOrderStatusCount[];
  average_turnaround_hours: number;
  top_tests: TopLabTestRow[];
}

// ── Backup & Restore (SRS §9 A-07 — Phase 2) ───────────────────────────────
//
// Returned by `list_backups` and `create_backup`. All four fields are
// populated server-side from filesystem metadata — there is no `backups`
// table. `filename` is the bare file name (no path). `path` is the absolute
// filesystem path to the file (e.g. `C:\ProgramData\HMS\backups\hospital_db_20250115_120000.sql`)
// — surfaced so the Settings → Backup section can show the operator where the
// file lives and so `create_backup`'s success toast can include the path. The
// `path` field is NEVER accepted as an input on `restore_backup` /
// `delete_backup` — those commands take a bare `backupFilename` (snake_cased
// to `backup_filename` over the Tauri IPC boundary) and join it to the
// backups directory after path-traversal validation, so a malicious frontend
// cannot trick the backend into overwriting/deleting files outside the
// backups dir.
//
// `created_at` is a UTC ISO-ish string ("YYYY-MM-DD HH:MM:SS UTC") produced
// by chrono's `format!` on the Rust side, NOT an ISO-8601 timestamp — treat
// it as a display string, not a machine-parsable instant.
export interface BackupInfo {
  filename: string;
  path: string;
  size_bytes: number;
  created_at: string;
}

// ── Pharmacy (Phase 2-C, SRS FR-0120–FR-0124) ──────────────────────────────
//
// TS shapes for the three pharmacy tables created in
// `src-tauri/src/db.rs::run_migrations`. Match the Rust structs in
// `src-tauri/src/models.rs` field-for-field. The `Medication.schedule`
// values mirror the DB VARCHAR(20): 'non-controlled' | 'schedule-II' |
// 'schedule-III' | 'schedule-IV' | 'schedule-V'. Any schedule other than
// 'non-controlled' is treated as controlled for the FR-0123 confirmation
// gate — see `src/pages/Pharmacy.tsx`.

export interface Medication {
  id: number;
  brand_name: string;
  generic_name: string;
  form: string;
  strength: string;
  schedule: string;
  category: string | null;
  manufacturer: string | null;
  reorder_level: number;
  is_active: boolean;
  created_at: string;
}

export interface CreateMedication {
  brand_name: string;
  generic_name: string;
  form?: string | null;
  strength: string;
  schedule?: string | null;
  category?: string | null;
  manufacturer?: string | null;
  reorder_level?: number | null;
  is_active?: boolean | null;
}

export interface UpdateMedication {
  id: number;
  brand_name: string;
  generic_name: string;
  form: string;
  strength: string;
  schedule: string;
  category?: string | null;
  manufacturer?: string | null;
  reorder_level: number;
  is_active: boolean;
}

export interface Prescription {
  id: number;
  patient_id: number;
  doctor_id: number | null;
  encounter_id: number | null;
  prescribed_by_user_id: number | null;
  status: string;
  notes: string | null;
  created_at: string;
  // Joined fields (LEFT JOIN patients + doctors), nullable like LabOrder.
  patient_name: string | null;
  doctor_name: string | null;
}

export interface PrescriptionItem {
  id: number;
  prescription_id: number;
  medication_id: number | null;
  medication_name: string;
  dose: string;
  route: string;
  frequency: string;
  duration: string | null;
  quantity: number;
  is_controlled: boolean;
  dispensed: boolean;
  dispensed_at: string | null;
  dispensed_by_user_id: number | null;
  created_at: string;
}

export interface CreatePrescriptionItem {
  medication_id: number | null;
  medication_name: string;
  dose: string;
  route?: string | null;
  frequency: string;
  duration?: string | null;
  quantity?: number | null;
}

export interface CreatePrescription {
  patient_id: number;
  doctor_id?: number | null;
  encounter_id?: number | null;
  items: CreatePrescriptionItem[];
  notes?: string | null;
}

/**
 * Returned by `get_prescription(id)` — the prescription header (flattened
 * via serde `#[serde(flatten)]` on the Rust side) plus its line items in
 * one round-trip. All `Prescription` fields sit at the top level; `items`
 * is the array of `PrescriptionItem`.
 */
export interface PrescriptionWithItems extends Prescription {
  items: PrescriptionItem[];
}

// ── Radiology (Phase 2-D, SRS FR-0140–FR-0142) ────────────────────────────
//
// TS shapes for the radiology workflow tables created in
// `src-tauri/src/db.rs::run_migrations`. Match the Rust structs in
// `src-tauri/src/models.rs` field-for-field.
//
// `RadiologyOrder` carries joined patient / doctor / radiologist names via
// LEFT JOINs (same pattern as LabOrder + Prescription) so the frontend's
// worklist renders a row without a second round-trip per order.
//
// `expected_date` is a `chrono::NaiveDate` on the Rust side (no time
// component) — it serialises to an ISO `YYYY-MM-DD` string. The other
// `*_at` timestamps are `chrono::DateTime<Utc>` and arrive as ISO-8601
// strings. Treat all of them as opaque display strings; use `new Date(...)`
// at the render layer.
//
// Status workflow (single source of truth = the Rust command):
//   ordered → scheduled → in_progress → completed → reported → verified
//                                                              ↘ cancelled
// Priority values: 'routine' | 'urgent' | 'emergency' | 'stat'.
// Study types: X-Ray, CT Scan, MRI, Ultrasound, Mammography, Fluoroscopy,
// DEXA, Other (catalogue defined in the frontend; not enforced server-side).
export interface RadiologyOrder {
  id: number;
  patient_id: number;
  encounter_id: number | null;
  ordered_by_doctor_id: number | null;
  ordered_by_user_id: number | null;
  order_number: string;
  department: string | null;
  clinical_indication: string | null;
  symptoms: string | null;
  diagnosis: string | null;
  priority: string;
  study_type: string;
  contrast_required: boolean;
  body_part: string | null;
  instructions: string | null;
  status: string;
  assigned_radiologist_id: number | null;
  assigned_technician: string | null;
  expected_date: string | null;
  ordered_at: string;
  scheduled_at: string | null;
  completed_at: string | null;
  reported_at: string | null;
  verified_at: string | null;
  created_at: string;
  updated_at: string;
  deleted_at: string | null;
  patient_name: string | null;
  doctor_name: string | null;
  radiologist_name: string | null;
}

export interface RadiologyOrdersResponse {
  orders: RadiologyOrder[];
  total: number;
  page: number;
  page_size: number;
  total_pages: number;
}

export interface CreateRadiologyOrder {
  patient_id: number;
  encounter_id?: number | null;
  ordered_by_doctor_id?: number | null;
  department?: string | null;
  clinical_indication?: string | null;
  symptoms?: string | null;
  diagnosis?: string | null;
  priority: string;
  study_type: string;
  contrast_required: boolean;
  body_part?: string | null;
  instructions?: string | null;
  assigned_radiologist_id?: number | null;
  assigned_technician?: string | null;
  expected_date?: string | null;
}

export interface RadiologyReport {
  id: number;
  order_id: number;
  findings: string | null;
  impression: string | null;
  recommendations: string | null;
  critical_finding: boolean;
  radiologist_id: number | null;
  verified_by_user_id: number | null;
  report_date: string;
  verified_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateRadiologyReport {
  order_id: number;
  findings?: string | null;
  impression?: string | null;
  recommendations?: string | null;
  critical_finding?: boolean;
}

/**
 * Radiology dashboard KPIs — returned by `get_radiology_dashboard` as a
 * flat JSON object. Mirrors the inline `serde_json::json!` construction in
 * `commands/radiology.rs::get_radiology_dashboard` field-for-field.
 */
export interface RadiologyDashboard {
  studies_today: number;
  pending_reports: number;
  emergency_cases: number;
  completed_today: number;
  cancelled: number;
  verification_pending: number;
}

// ── Blood Bank (Phase 2-E, SRS FR-0145–FR-0149) ────────────────────────────
//
// TS shapes for the blood-bank workflow tables created in
// `src-tauri/src/db.rs::run_migrations`. One interface per Rust struct in
// `models.rs` (field-for-field). Write payloads use dedicated `Create*`
// interfaces (mirrors the radiology convention).

export interface BloodDonor {
  id: number;
  donor_number: string;
  patient_id?: number | null;
  first_name: string;
  last_name: string;
  date_of_birth?: string | null;
  gender?: string | null;
  blood_group: string;
  rh_factor: string;
  phone?: string | null;
  email?: string | null;
  address?: string | null;
  weight_kg?: string | null;
  height_cm?: string | null;
  last_donation_date?: string | null;
  total_donations: number;
  status: string;
  medically_deferred_until?: string | null;
  defer_reason?: string | null;
  notes?: string | null;
  created_by_user_id?: number | null;
  created_at: string;
  updated_at: string;
  deleted_at?: string | null;
  patient_name?: string | null;
}

export interface CreateBloodDonor {
  patient_id?: number | null;
  first_name: string;
  last_name: string;
  date_of_birth?: string;
  gender?: string;
  blood_group: string;
  rh_factor: string;
  phone?: string;
  email?: string;
  address?: string;
  weight_kg?: number;
  height_cm?: number;
  notes?: string;
}

export interface BloodDonation {
  id: number;
  donation_number: string;
  donor_id: number;
  donation_date: string;
  collection_site?: string | null;
  collected_by_user_id?: number | null;
  volume_ml: number;
  blood_group: string;
  rh_factor: string;
  bag_type?: string | null;
  status: string;
  screening_status: string;
  screening_notes?: string | null;
  screened_by_user_id?: number | null;
  screened_at?: string | null;
  hemoglobin_level?: string | null;
  blood_pressure?: string | null;
  pulse?: number | null;
  temperature_c?: string | null;
  notes?: string | null;
  created_at: string;
  updated_at: string;
  donor_name?: string | null;
}

export interface CreateBloodDonation {
  donor_id: number;
  collection_site?: string;
  volume_ml: number;
  blood_group: string;
  rh_factor: string;
  bag_type?: string;
  hemoglobin_level?: number;
  blood_pressure?: string;
  pulse?: number;
  temperature_c?: number;
  notes?: string;
}

export interface BloodUnit {
  id: number;
  unit_number: string;
  donation_id?: number | null;
  donor_id: number;
  component_type: string;
  blood_group: string;
  rh_factor: string;
  volume_ml: number;
  collection_date: string;
  expiry_date: string;
  storage_temperature?: string | null;
  storage_location?: string | null;
  status: string;
  reserved_for_patient_id?: number | null;
  reservation_id?: number | null;
  issued_to_patient_id?: number | null;
  issued_at?: string | null;
  transfused_at?: string | null;
  transfused_to_patient_id?: number | null;
  discarded_at?: string | null;
  discard_reason?: string | null;
  created_by_user_id?: number | null;
  created_at: string;
  updated_at: string;
  deleted_at?: string | null;
  donor_name?: string | null;
  patient_name?: string | null;
  days_to_expiry?: number | null;
}

export interface CreateBloodUnit {
  donor_id: number;
  donation_id?: number | null;
  component_type: string;
  blood_group: string;
  rh_factor: string;
  volume_ml: number;
  storage_temperature?: string;
  storage_location?: string;
  expiry_date: string;
}

export interface BloodCrossmatch {
  id: number;
  unit_id: number;
  patient_id: number;
  doctor_id?: number | null;
  requested_by_user_id?: number | null;
  crossmatch_date: string;
  method?: string | null;
  result: string;
  reaction_grade?: number | null;
  incubation_time_min?: number | null;
  ahg_phase?: string | null;
  notes?: string | null;
  performed_by_user_id?: number | null;
  verified_by_user_id?: number | null;
  verified_at?: string | null;
  created_at: string;
  updated_at: string;
  unit_number?: string | null;
  patient_name?: string | null;
  doctor_name?: string | null;
}

export interface CreateBloodCrossmatch {
  unit_id: number;
  patient_id: number;
  doctor_id?: number | null;
  method?: string;
  result: string;
  reaction_grade?: number | null;
  incubation_time_min?: number | null;
  ahg_phase?: string;
  notes?: string;
}

export interface BloodReservation {
  id: number;
  reservation_number: string;
  unit_id: number;
  patient_id: number;
  doctor_id?: number | null;
  requested_by_user_id?: number | null;
  crossmatch_id?: number | null;
  reserved_at: string;
  expires_at: string;
  fulfilled_at?: string | null;
  cancelled_at?: string | null;
  status: string;
  priority: string;
  clinical_indication?: string | null;
  notes?: string | null;
  created_at: string;
  updated_at: string;
  unit_number?: string | null;
  patient_name?: string | null;
  doctor_name?: string | null;
}

export interface CreateBloodReservation {
  unit_id: number;
  patient_id: number;
  doctor_id?: number | null;
  crossmatch_id?: number | null;
  priority: string;
  expires_in_hours: number;
  clinical_indication?: string;
  notes?: string;
}

export interface BloodIssue {
  id: number;
  issue_number: string;
  unit_id: number;
  patient_id: number;
  reservation_id?: number | null;
  crossmatch_id?: number | null;
  doctor_id?: number | null;
  issued_by_user_id?: number | null;
  issued_at: string;
  issued_to_location?: string | null;
  issue_type: string;
  clinical_indication?: string | null;
  special_instructions?: string | null;
  returned_at?: string | null;
  return_reason?: string | null;
  received_by_user_id?: number | null;
  created_at: string;
  updated_at: string;
  unit_number?: string | null;
  patient_name?: string | null;
  doctor_name?: string | null;
  issued_by_name?: string | null;
}

export interface CreateBloodIssue {
  unit_id: number;
  patient_id: number;
  reservation_id?: number | null;
  crossmatch_id?: number | null;
  doctor_id?: number | null;
  issued_to_location?: string;
  issue_type: string;
  clinical_indication?: string;
  special_instructions?: string;
}

export interface BloodTransfusion {
  id: number;
  transfusion_number: string;
  issue_id: number;
  unit_id: number;
  patient_id: number;
  doctor_id?: number | null;
  nurse_id?: number | null;
  started_at: string;
  completed_at?: string | null;
  volume_transfused_ml?: number | null;
  pre_transfusion_bp?: string | null;
  post_transfusion_bp?: string | null;
  pre_transfusion_temp?: string | null;
  post_transfusion_temp?: string | null;
  pre_transfusion_pulse?: number | null;
  post_transfusion_pulse?: number | null;
  reaction_observed: boolean;
  reaction_type?: string | null;
  reaction_severity?: string | null;
  reaction_notes?: string | null;
  outcome?: string | null;
  notes?: string | null;
  created_at: string;
  updated_at: string;
  unit_number?: string | null;
  patient_name?: string | null;
  doctor_name?: string | null;
}

export interface CreateBloodTransfusion {
  issue_id: number;
  unit_id: number;
  patient_id: number;
  doctor_id?: number | null;
  nurse_id?: number | null;
  volume_transfused_ml?: number | null;
  pre_transfusion_bp?: string;
  post_transfusion_bp?: string;
  pre_transfusion_temp?: number;
  post_transfusion_temp?: number;
  pre_transfusion_pulse?: number;
  post_transfusion_pulse?: number;
  reaction_observed: boolean;
  reaction_type?: string;
  reaction_severity?: string;
  reaction_notes?: string;
  outcome?: string;
  notes?: string;
}

export interface BloodDiscard {
  id: number;
  unit_id: number;
  discard_number: string;
  discarded_at: string;
  discard_reason: string;
  discard_notes?: string | null;
  discarded_by_user_id?: number | null;
  authorized_by_user_id?: number | null;
  disposal_method?: string | null;
  created_at: string;
  updated_at: string;
  unit_number?: string | null;
  discarded_by_name?: string | null;
}

export interface CreateBloodDiscard {
  unit_id: number;
  discard_reason: string;
  discard_notes?: string;
  disposal_method?: string;
}

export interface BloodUnitHistory {
  id: number;
  unit_id: number;
  status: string;
  changed_by_user_id?: number | null;
  changed_at: string;
  notes?: string | null;
  related_record_type?: string | null;
  related_record_id?: number | null;
  changed_by_name?: string | null;
}

export interface BloodMovement {
  id: number;
  unit_id: number;
  movement_type: string;
  from_location?: string | null;
  to_location?: string | null;
  moved_by_user_id?: number | null;
  moved_at: string;
  reason?: string | null;
  related_record_type?: string | null;
  related_record_id?: number | null;
  created_at: string;
  unit_number?: string | null;
  moved_by_name?: string | null;
}

export interface BloodBankDashboard {
  available_units: number;
  reserved_units: number;
  issued_units: number;
  quarantine_units: number;
  discarded_all_time: number;
  expiring_soon: number;
  total_donors: number;
  active_donors: number;
  deferred_donors: number;
  transfusions_today: number;
  active_reservations: number;
  stock_by_type: Array<{
    blood_group: string;
    rh_factor: string;
    component_type: string;
    count: number;
  }>;
}

export interface BloodBankStatistics {
  months: number;
  donations_per_month: Array<[string, number]>;
  transfusions_per_month: Array<[string, number]>;
  discards_per_month: Array<[string, number]>;
  reactions_per_month: Array<[string, number]>;
}

export interface BloodCompatibilityResult {
  compatible: boolean;
  donor_group: string;
  donor_rh: string;
  patient_group: string;
  patient_rh: string;
  reason: string;
}

export interface BloodUnitTraceability {
  unit_id: number;
  status_history: BloodUnitHistory[];
  movements: BloodMovement[];
  crossmatches: BloodCrossmatch[];
  issues: BloodIssue[];
  transfusions: BloodTransfusion[];
  discards: BloodDiscard[];
}

export interface PaginatedResponse {
  total: number;
  page: number;
  page_size: number;
  total_pages: number;
}

export interface BloodDonorsResponse extends PaginatedResponse {
  donors: BloodDonor[];
}

export interface BloodDonationsResponse extends PaginatedResponse {
  donations: BloodDonation[];
}

export interface BloodUnitsResponse extends PaginatedResponse {
  units: BloodUnit[];
}

export interface BloodCrossmatchesResponse extends PaginatedResponse {
  crossmatches: BloodCrossmatch[];
}

export interface BloodIssuesResponse extends PaginatedResponse {
  issues: BloodIssue[];
}

export interface BloodTransfusionsResponse extends PaginatedResponse {
  transfusions: BloodTransfusion[];
}

export interface BloodDiscardsResponse extends PaginatedResponse {
  discards: BloodDiscard[];
}
