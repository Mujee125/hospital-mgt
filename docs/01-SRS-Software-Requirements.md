# VitalFlow HMS — Software Requirements Specification (SRS)

> **Document version: v0.2.0 — updated 2025-07-08 after Phase 2 Batches 0-3 implementation. See CHANGELOG.md for details.**

| Field | Value |
|---|---|
| **Document title** | VitalFlow HMS — Software Requirements Specification |
| **Standard** | ISO/IEC/IEEE 29148:2018 (Systems and software engineering — Life cycle processes — Requirements engineering) |
| **Version** | 0.2.0 |
| **Date** | 2025-07-08 |
| **Status** | Draft |
| **Classification** | Internal |
| **Owner** | VitalFlow HMS Engineering |
| **Author** | Documentation Specialist (Task 7) |
| **Related documents** | `02-SDD-Software-Design.md`, `03-Quality-Model-ISO-25010.md`, `04-Security-Control-Matrix-ISO-27001.md`, `05-Risk-Register-ISO-31000.md`, `06-SDLC-ISO-12207.md`, `07-Licensing-Architecture.md`, `08-Deployment-Installation-Guide.md` |

---

## 1. Document control

### 1.1 Revision history

| Version | Date | Author | Summary |
|---|---|---|---|
| 0.1.0 | 2026-07-02 | Documentation Specialist | Initial SRS baseline covering Phase 1 (implemented) and Phase 2 (planned) modules. |
| 0.2.0 | 2025-07-08 | Documentation Team (B4-A) | Reconciled with Phase 2 Batches 0-3 code changes: NFR-50 implemented (Batch 0); FR-0035 consent commands + WhatsApp consent gate (Batch 1 CR-12); FR-0180/0181/0182/0185 inventory commands + UI (Batch 1 CR-21); FR-0105 internal contradiction resolved via patient soft-delete + clinical FK RESTRICT (Batch 2 CR-11); NFR-15 RBAC on config/messaging/whatsapp (Batches 1, 3); revoke_license command (Batch 3 LIC-DOC-04); COMPANY_PUBLIC_KEY placeholder replaced with real dev keypair (Batch 2 CR-20). |

### 1.2 Approval matrix

| Role | Name | Signature | Date |
|---|---|---|---|
| Project sponsor | TBD | | |
| Lead architect | TBD | | |
| QA lead | TBD | | |
| Information security officer | TBD | | |
| Clinical lead | TBD | | |

### 1.3 Distribution

This document is classified **Internal**. Distribution is limited to VitalFlow HMS engineering, QA, compliance, and contracted hospital IT stakeholders. The on-disk source lives at `/home/z/my-project/hospital-mgmt/docs/01-SRS-Software-Requirements.md`.

---

## 2. Introduction

### 2.1 Purpose

This SRS specifies the requirements for **VitalFlow HMS** — a native Windows desktop Hospital Management System (HMS) built with Tauri v2, Rust, PostgreSQL, React 19, TypeScript, and Tailwind CSS. The document is the authoritative functional and non-functional baseline for Phase 1 (implemented in code) and Phase 2 (planned). It is written to satisfy the structure recommended by ISO/IEC/IEEE 29148:2018 §5.2 and is traceable to the Quality Model (ISO/IEC 25010), Security Control Matrix (ISO/IEC 27001), and Risk Register (ISO 31000) cross-referenced in §10.

### 2.2 Scope

VitalFlow HMS supports the operational, clinical, administrative, financial, and regulatory workflows of a single hospital per licensed deployment. The system is delivered as two Windows installers (server-build and client-build) sharing a common PostgreSQL backend on the server PC and pairing over a private LAN.

In scope:

- Single-hospital, hardware-bound, Ed25519-signed licensing model.
- Argon2id authentication, RBAC (8 roles, 35 permissions), audit logging, session management.
- Clinical modules: Patients/EHR, OPD, IPD, Laboratory, Radiology (Phase 2), Pharmacy (Phase 2), Blood Bank (Phase 2).
- Administrative modules: Appointments, Queue, Doctors, Nurses (Phase 2), Billing, Invoicing, Payments, Inventory, HR (Phase 2), Payroll (Phase 2), Reports, Admin/Settings.
- Multi-PC LAN topology with TLS-pinned PostgreSQL connections.
- Installer-driven PostgreSQL provisioning with zero operator interaction.

Out of scope for this revision:

- Multi-tenant / multi-hospital SaaS. The license model forbids it (see §6 and `07-Licensing-Architecture.md`).
- Mobile-native clients (the UI is responsive within the Tauri shell; no iOS/Android binaries).
- Public-internet exposure. The deployment model is LAN-only by design.
- HL7/FHIR integration with external systems. Reserved for a future revision.

### 2.3 Definitions, acronyms, and abbreviations

| Term | Definition |
|---|---|
| **HMS** | Hospital Management System |
| **EHR** | Electronic Health Record |
| **OPD** | Out-Patient Department |
| **IPD** | In-Patient Department |
| **MRN** | Medical Record Number |
| **PHI** | Protected Health Information |
| **RBAC** | Role-Based Access Control |
| **RBAC Permission** | A single authorisation key in the `Permission` enum (`src-tauri/src/rbac.rs`) |
| **Session** | An authenticated in-memory principal held in Tauri app state for a single desktop user |
| **Server-build** | Cargo feature `server-build` — installer bundles PostgreSQL, designates this PC as the DB host |
| **Client-build** | Cargo feature `client-build` — installer contains no PostgreSQL, connects to a paired server |
| **Pairing** | The short-lived, code-mediated credential exchange between a server and a client PC |
| **Hardware fingerprint** | SHA-256 over Windows WMI CPU+baseboard+BIOS identifiers, used to bind a license to a machine |
| **License file** | A signed JSON document (`license.json`) containing hospital identity, module entitlements, validity, fingerprint, and Ed25519 signature |
| **ProgramData** | `C:\ProgramData\HMS` — the machine-wide HMS data directory written by the elevated installer |
| **NSIS** | Nullsoft Scriptable Install System — produces the Windows `.exe` installer |
| **TLS pinning** | Client stores the server's self-signed certificate and pins to its SHA-256 fingerprint |
| **scram-sha-256** | PostgreSQL password challenge scheme mandated by `pg_hba.conf` |
| **ISMS** | Information Security Management System (per ISO/IEC 27001) |
| **SoA** | Statement of Applicability (ISO/IEC 27001 Annex A) |
| **PHC string** | Password Hashing Competition format — the Argon2id serialized hash format |

### 2.4 References

| Reference | Relevance |
|---|---|
| ISO/IEC/IEEE 29148:2018 | Structure of this SRS |
| ISO/IEC/IEEE 12207:2015 | SDLC processes — see `06-SDLC-ISO-12207.md` |
| ISO/IEC 25010:2023 | Quality model — see `03-Quality-Model-ISO-25010.md` |
| ISO/IEC 27001:2022 | ISMS controls — see `04-Security-Control-Matrix-ISO-27001.md` |
| ISO 31000:2018 | Risk management — see `05-Risk-Register-ISO-31000.md` |
| IEEE 1016-2009 | Software design — see `02-SDD-Software-Design.md` |
| OWASP ASVS v4.0 | Authentication and session control baselines |
| HIPAA "minimum necessary" principle | Reflected in role-to-permission map |
| NHS Digital Clinical Risk Management standard | Aligned where applicable for clinical safety |

---

## 3. Stakeholders and overview

### 3.1 Stakeholder identification

| Stakeholder | Interest | Concerns addressed by this SRS |
|---|---|---|
| Hospital administrator | Operational uptime, billing accuracy, license compliance | §4 functional, §6 licensing, §7 NFRs |
| Doctor | EHR availability, lab results, prescription correctness | FR-0030–FR-0049, FR-0080–FR-0099 |
| Nurse | Ward/bed state, queue, vitals | FR-0060–FR-0079, FR-0050–FR-0059 |
| Receptionist | Patient registration, appointments, queue | FR-0020–FR-0059 |
| Lab technician | Lab orders and results workflow | FR-0100–FR-0119 |
| Pharmacist | Inventory and dispensing (Phase 2 minimal) | FR-0140–FR-0159 |
| Billing clerk | Bills, payments, receipts | FR-0120–FR-0139 |
| Patient | Portal access to own records (Phase 2) | FR-0200–FR-0209 |
| Hospital IT | Installation, backup, recovery, LAN topology | `08-Deployment-Installation-Guide.md` |
| Software company (issuer) | License integrity, single-hospital enforcement | §6, `07-Licensing-Architecture.md` |
| Regulator / auditor | Evidence of access control, audit trail | `04-Security-Control-Matrix-ISO-27001.md` |
| VitalFlow engineering | Maintainable codebase, traceable requirements | This document, `02-SDD-Software-Design.md` |

### 3.2 System overview

VitalFlow HMS is a Tauri v2 desktop application. The Rust core (`src-tauri/src/`) performs all database, security, licensing, and provisioning work; the React 19 frontend (`src/`) renders the UI and calls Rust commands over the Tauri IPC bridge. PostgreSQL is the only persistent store. A server-build installer provisions PostgreSQL as a Windows Service; client-build installers pair to it over the LAN with TLS-pinned connections.

---

## 4. Functional requirements

Requirements use the numbering convention `FR-NNNN` and are grouped by module. Each requirement row states: ID, description, priority (Must/Should/Could per MoSCoW), phase, and the principal implementing file(s). **Phase 1 = IMPLEMENTED in the current source tree. Phase 2 = PLANNED, code not yet present.**

Priority legend: **M** = Must (release-blocking), **S** = Should, **C** = Could.

### 4.1 Dashboard (FR-0010–FR-0019) — Phase 1, IMPLEMENTED

| ID | Requirement | Priority | Phase | Implementation |
|---|---|---|---|---|
| FR-0010 | The system shall render a role-aware dashboard with KPI cards (today's appointments, active IPD admissions, pending lab results, outstanding bills) computed from the authenticated user's permissions. | M | 1 | `commands/dashboard.rs`, `pages/Dashboard.tsx` |
| FR-0011 | The dashboard shall display today's appointment schedule with quick-link navigation to the Appointments page. | M | 1 | `pages/Dashboard.tsx` |
| FR-0012 | The dashboard shall display an appointment-status mix chart that renders only when at least one appointment exists. | S | 1 | `pages/Dashboard.tsx` (Recharts) |
| FR-0013 | The dashboard shall expose LAN/server status information that is hidden from day-to-day staff and surfaced only in Settings → Advanced. | M | 1 | `App.tsx`, `pages/Settings.tsx` |
| FR-0014 | KPI queries shall be server-side aggregations returned by `get_dashboard_kpis`, not client-side reductions of full result sets. | M | 1 | `commands/dashboard.rs` |
| FR-0015 | Dashboard access shall require the `dashboard.view` permission. | M | 1 | `rbac.rs` (`Permission::DashboardView`) |

### 4.2 Authentication, RBAC, and Session management (FR-0020–FR-0029) — Phase 1, IMPLEMENTED

| ID | Requirement | Priority | Phase | Implementation |
|---|---|---|---|---|
| FR-0020 | The system shall authenticate users against the `users` table using Argon2id (m=19456 KiB, t=2, p=1) PHC-string hashes; plaintext passwords shall never be persisted. | M | 1 | `auth.rs::hash_password`, `auth.rs::verify_password` |
| FR-0021 | The system shall lock an account for 15 minutes after 5 consecutive failed logins; the counter shall reset on a successful login. | M | 1 | `auth.rs::login` (`MAX_FAILED_ATTEMPTS`, `LOCKOUT_MINUTES`) |
| FR-0022 | Login shall be audited (success and failure with reason) to `audit_logs`. | M | 1 | `auth.rs::login`, `audit.rs::record` |
| FR-0023 | Login shall be constant-time against unknown usernames by running a dummy Argon2 verify before rejecting, to mitigate user-enumeration timing. | M | 1 | `auth.rs::login` |
| FR-0024 | Sessions shall be single-active per user; a new login shall delete prior `sessions` rows for that user. | M | 1 | `auth.rs::login` (`DELETE FROM sessions WHERE user_id = $1`) |
| FR-0025 | Session tokens shall be 32 random bytes, base64url-encoded; only the SHA-256 hash shall be persisted in `sessions.token_hash`. The raw token shall never be written to disk. | M | 1 | `auth.rs::random_token`, `auth.rs::hash_token` |
| FR-0026 | Sessions shall expire after 12 hours; `me` shall re-verify the session against the DB on each call. | M | 1 | `auth.rs::me`, `SESSION_HOURS = 12` |
| FR-0027 | The system shall seed a bootstrap `admin` user with password `ChangeMe123!` and `must_change_password = TRUE` on first migration; no other backdoor account shall exist. | M | 1 | `auth.rs::seed_defaults` |
| FR-0028 | Password changes shall require the current password, enforce an 8-character minimum on the new password, and audit the event. | M | 1 | `auth.rs::change_password` |
| FR-0029 | User management commands (create/update/delete/reset-password) shall require `users.manage`; administrators shall not be able to delete their own account. | M | 1 | `auth.rs::{create_user, update_user, delete_user, reset_user_password}` |

### 4.3 Patients / EHR (FR-0030–FR-0049) — Phase 1, IMPLEMENTED

| ID | Requirement | Priority | Phase | Implementation |
|---|---|---|---|---|
| FR-0030 | The system shall maintain a patient registry with EHR columns: MRN, blood group, allergies, chronic conditions, emergency contact, insurance provider/policy, status, `created_by_user_id`. | M | 1 | `db.rs` (patients ALTER additions), `commands/patients.rs` |
| FR-0031 | The system shall record a unique MRN per patient and surface uniqueness violations as user-facing errors. | M | 1 | `db.rs` (`mrn VARCHAR(20) UNIQUE`) |
| FR-0032 | Patient create/update/delete shall be RBAC-guarded by `patients.create`/`patients.update`/`patients.delete` respectively. | M | 1 | `commands/patients.rs`, `rbac.rs` |
| FR-0033 | Every patient write shall record `created_by_user_id` from the session for provenance. | M | 1 | `commands/patients.rs` |
| FR-0034 | Patient reads shall require `patients.view`; reads shall not be row-level audited (volume), but patient create/update/delete shall be audited. | M | 1 | `commands/patients.rs`, `audit.rs` |
| FR-0035 | The system shall maintain `patient_consent` rows keyed by patient + consent_type; consent changes shall require `patients.consent.manage` and shall be audited. **[Implemented v0.2.0 — Batch 1 CR-12 added 3 consent commands (`get_patient_consent`, `set_patient_consent`, `revoke_patient_consent`) in `commands/patients.rs`; the WhatsApp automation gate now refuses to send to a patient with revoked or missing `whatsapp` consent.]** | M | 1 | `db.rs` (`patient_consent`), `rbac.rs` (`Permission::PatientConsentManage`), `commands/patients.rs` (CR-12 v0.2.0) |
| FR-0036 | The system shall support encounters (visit records) linked to a patient and optionally a doctor, with chief complaint, diagnosis, notes, visit_type, and visit_date. | M | 1 | `commands/encounters.rs`, `db.rs` (`encounters`) |
| FR-0037 | The patient list page shall display skeletons during load and an empty-state CTA when no patients exist. | S | 1 | `pages/Patients.tsx` |
| FR-0038 | Patient search shall be client-side against the fetched list, matching the backend's actual capability (no server-side search parameter exists). | S | 1 | `pages/Patients.tsx` |
| FR-0039 | The patient DTO returned to the frontend shall never include `password_hash` or any field not declared in `models.rs::Patient`. | M | 1 | `models.rs`, `commands/patients.rs` |
| FR-0040 | Deletion of a patient with existing `ipd_admissions` or `bills` rows shall be prevented by `ON DELETE RESTRICT` to preserve financial/clinical history. **[Updated v0.2.0 — Batch 2 CR-11 expanded `ON DELETE RESTRICT` to all 5 clinical `patient_id` FKs (`appointments`, `patient_consent`, `encounters`, `queue_tokens`, `lab_orders`) and added patient soft-delete columns (`patients.deleted_at TIMESTAMPTZ`, `patients.is_active BOOLEAN`) for HIPAA §164.530(j) 6-year PHI retention. UI delete now sets `deleted_at = NOW(), is_active = FALSE` instead of hard-deleting.]** | M | 1 | `db.rs` (FK constraints), `commands/patients.rs::delete_patient` (soft-delete) |

### 4.4 OPD — Outpatient department workflow (FR-0050–FR-0059) — Phase 1, IMPLEMENTED (via Appointments + Queue + Encounters)

| ID | Requirement | Priority | Phase | Implementation |
|---|---|---|---|---|
| FR-0050 | The system shall support an OPD visit flow: appointment → check-in (queue token) → encounter → lab/bill. | M | 1 | `pages/Appointments.tsx`, `pages/Queue.tsx`, `commands/encounters.rs` |
| FR-0051 | Encounters shall default `visit_type = 'opd'` and be creatable from the patient record. | M | 1 | `commands/encounters.rs` |
| FR-0052 | The OPD encounter shall support recording chief_complaint, diagnosis, and free-text notes. | M | 1 | `commands/encounters.rs` |
| FR-0053 | OPD encounters shall be linkable to lab orders (`lab_orders.encounter_id`). | M | 1 | `db.rs` (`lab_orders.encounter_id`) |
| FR-0054 | OPD encounters shall be linkable to bills (`bills.encounter_id`, `bills.bill_type = 'opd'`). | M | 1 | `db.rs` (`bills`) |
| FR-0055 | Receptionist shall be able to create appointments and queue tokens without elevated permissions beyond their role grant. | M | 1 | `rbac.rs` (`ROLE_RECEPTIONIST`) |
| FR-0056 | Doctors shall be able to update appointment status and create encounters without requiring receptionist permissions. | M | 1 | `rbac.rs` (`ROLE_DOCTOR`) |

### 4.5 IPD — In-patient department (FR-0060–FR-0079) — Phase 1, IMPLEMENTED

| ID | Requirement | Priority | Phase | Implementation |
|---|---|---|---|---|
| FR-0060 | The system shall maintain `wards` (name, code, floor, gender_restriction) and `beds` (ward_id, bed_number, status, is_icu, daily_rate). | M | 1 | `db.rs`, `commands/ipd.rs` |
| FR-0061 | `beds` shall be unique per `(ward_id, bed_number)`. | M | 1 | `db.rs` (`UNIQUE`) |
| FR-0062 | Admission (`admit_patient`) shall atomically allocate a bed by setting `beds.status = 'occupied'` and inserting `ipd_admissions` in the same transaction; on conflict the operation shall roll back. | M | 1 | `commands/ipd.rs::admit_patient` |
| FR-0063 | Discharge (`discharge_patient`) shall free the bed (`status = 'available'`), set `discharge_date` and `discharge_summary`, and mark `ipd_admissions.status = 'discharged'`. | M | 1 | `commands/ipd.rs::discharge_patient` |
| FR-0064 | Admission shall require `ipd.manage`; bed/ward management shall require `beds.manage`; IPD views shall require `ipd.view`. | M | 1 | `rbac.rs` |
| FR-0065 | IPD admissions shall support `admission_type` (routine/emergency/day-care) and an `attending_doctor_id` distinct from `doctor_id`. | S | 1 | `db.rs` (`ipd_admissions`) |
| FR-0066 | Nurses shall have IPD manage and beds manage permissions; receptionists shall not. | M | 1 | `rbac.rs` (`ROLE_NURSE` vs `ROLE_RECEPTIONIST`) |
| FR-0067 | IPD admissions shall be billable: a bill with `bill_type = 'ipd'` may reference `ipd_admission_id`. | M | 1 | `db.rs` (`bills.ipd_admission_id`) |
| FR-0068 | Bed daily_rate shall be NUMERIC(12,2); the system shall not silently truncate monetary values. | M | 1 | `db.rs` |

### 4.6 Appointments (FR-0080–FR-0099) — Phase 1, IMPLEMENTED

| ID | Requirement | Priority | Phase | Implementation |
|---|---|---|---|---|
| FR-0080 | The system shall maintain `appointments` with patient_id, doctor_id, date, time, duration_minutes, status, reason, notes. | M | 1 | `db.rs`, `commands/appointments.rs` |
| FR-0081 | Appointment status shall be one of `scheduled`, `confirmed`, `completed`, `cancelled`, `no-show`. | M | 1 | `commands/appointments.rs::update_appointment_status` |
| FR-0082 | Appointment create/update/delete shall be RBAC-guarded and audited; `created_by_user_id` shall be recorded. | M | 1 | `commands/appointments.rs` |
| FR-0083 | The system shall provide `get_today_appointments` and `get_appointment_stats` for the dashboard. | M | 1 | `commands/appointments.rs` |
| FR-0084 | Appointment receipts shall be printable on 80mm thermal paper via `@media print` CSS. | S | 1 | `components/Receipt.tsx` |
| FR-0085 | Appointment quick-status buttons (Confirm/Complete/Cancel) shall call `update_appointment_status` and invalidate the React Query cache for appointments. | M | 1 | `pages/Appointments.tsx`, `lib/queries.ts` |
| FR-0086 | An appointment may be linked to a `queue_token_id` to bridge scheduling and queue. | S | 1 | `db.rs` (`appointments.queue_token_id`) |
| FR-0087 | WhatsApp notifications shall be sent on appointment creation/confirmation if a WhatsApp group is configured and the patient has not revoked WhatsApp consent. **[Updated v0.2.0 — Batch 1 CR-12 added the WhatsApp consent gate: `whatsapp/automation.rs` now checks `patient_consent` for the `whatsapp` consent_type and refuses to send if revoked or missing.]** | S | 1 | `whatsapp/automation.rs`, `scheduler.rs`, `commands/patients.rs` (consent commands) |

### 4.7 Queue (FR-0090–FR-0099) — Phase 1, IMPLEMENTED

| ID | Requirement | Priority | Phase | Implementation |
|---|---|---|---|---|
| FR-0090 | The system shall maintain `queue_tokens` (patient_id, department_id, doctor_id, token_number, status, priority, issued_at, called_at, completed_at). | M | 1 | `db.rs`, `commands/queue.rs` |
| FR-0091 | `call_next_token` shall select the next waiting token by priority desc, issued_at asc, mark it `called`, and audit the action. | M | 1 | `commands/queue.rs::call_next_token` |
| FR-0092 | `set_token_status` shall transition a token between waiting/called/completed/skipped. | M | 1 | `commands/queue.rs::set_token_status` |
| FR-0093 | Token numbers shall be unique per day per department (implemented as a sequential counter; cross-day reset is a Should). | S | 1 | `commands/queue.rs` |
| FR-0094 | Queue management shall require `queue.manage`; viewing shall require `queue.view`. | M | 1 | `rbac.rs` |
| FR-0095 | Receptionists and nurses shall have `queue.manage`; doctors shall have `queue.view` only. | M | 1 | `rbac.rs` (`ROLE_RECEPTIONIST`, `ROLE_NURSE`, `ROLE_DOCTOR`) |

### 4.8 Doctors (FR-0100–FR-0109) — Phase 1, IMPLEMENTED

| ID | Requirement | Priority | Phase | Implementation |
|---|---|---|---|---|
| FR-0100 | The system shall maintain `doctors` (first/last name, email, phone, specialization, qualification, available_from/to, is_active). | M | 1 | `db.rs`, `commands/doctors.rs` |
| FR-0101 | The doctor list page shall compute a client-side "Available now / Off duty / Inactive" indicator from current time vs `available_from/to`. | S | 1 | `pages/Doctors.tsx` |
| FR-0102 | Doctor create/update/delete shall require `doctors.manage`. | M | 1 | `commands/doctors.rs`, `rbac.rs` |
| FR-0103 | `get_specializations` shall return the distinct set of specializations for filter dropdowns. | S | 1 | `commands/doctors.rs::get_specializations` |
| FR-0104 | A doctor may be the head of a `department` (`departments.head_doctor_id`). | S | 1 | `db.rs` |
| FR-0105 | Deleting a doctor shall not cascade-delete appointments; `appointments.doctor_id` FK is `ON DELETE RESTRICT` (changed in Batch 2 CR-11 from CASCADE) — administrators must reassign the doctor's appointments before deletion. `departments.head_doctor_id` remains `ON DELETE SET NULL`. **[Updated v0.2.0 — internal contradiction resolved: the previous revision claimed `appointments.doctor_id ... ON DELETE CASCADE` while simultaneously stating "shall not cascade-delete". Both `appointments.doctor_id` and `appointments.patient_id` are now RESTRICT. Patient deletion is soft-delete via `patients.deleted_at` + `patients.is_active=FALSE`; clinical FKs are RESTRICT for HIPAA §164.530(j) 6-year PHI retention.]** | M | 1 | `db.rs` (FK constraints) |

### 4.9 Nurses (FR-0110–FR-0119) — Phase 2, PLANNED

| ID | Requirement | Priority | Phase | Implementation |
|---|---|---|---|---|
| FR-0110 | The system shall maintain a `nurses` registry (first/last name, registration number, ward assignment, shift, contact). | M | 2 | _planned_ |
| FR-0111 | Nurses shall be assignable to wards (`nurse_ward_assignments`). | S | 2 | _planned_ |
| FR-0112 | Nurses shall be able to record vitals per encounter (`vitals` table: BP, pulse, temp, SpO2, respiratory rate, recorded_at, recorded_by). | M | 2 | _planned_ |
| FR-0113 | Nurse management shall require a `nurses.manage` permission (new in Phase 2 RBAC extension). | M | 2 | _planned_ |

### 4.10 Pharmacy (FR-0120–FR-0129) — Phase 2, PLANNED (inventory scaffolding exists in Phase 1)

| ID | Requirement | Priority | Phase | Implementation |
|---|---|---|---|---|
| FR-0120 | The system shall maintain a medication catalog with brand, generic, form, strength, controlled-substance schedule. | M | 2 | _planned_; `inventory_items.category = 'medication'` in Phase 1 |
| FR-0121 | The system shall support prescription generation from encounters with dose, route, frequency, duration. | M | 2 | _planned_ |
| FR-0122 | Dispensing shall decrement `inventory_items.stock_quantity` and create an audited dispensing record. | M | 2 | _planned_ |
| FR-0123 | Controlled-substance dispensing shall require two-person verification (electronic signature by a second pharmacist). | S | 2 | _planned_ |
| FR-0124 | Pharmacists shall be limited to `inventory.view`, `inventory.manage`, `billing.view`, `patients.view` in Phase 1 (already implemented); Phase 2 adds `pharmacy.dispense`. | M | 1→2 | `rbac.rs::ROLE_PHARMACIST` |

### 4.11 Laboratory (FR-0130–FR-0139) — Phase 1, IMPLEMENTED

| ID | Requirement | Priority | Phase | Implementation |
|---|---|---|---|---|
| FR-0130 | The system shall maintain `lab_test_catalog` (name, code, category, sample_type, normal_range, unit, price). | M | 1 | `db.rs`, `commands/lab.rs` |
| FR-0131 | The system shall support `lab_orders` (patient_id, encounter_id, ordered_by_doctor_id, ordered_by_user_id, status) and `lab_order_tests` per ordered test. | M | 1 | `db.rs`, `commands/lab.rs` |
| FR-0132 | Entering the last pending result on an order shall auto-transition the order to `completed`. | M | 1 | `commands/lab.rs::update_lab_result` |
| FR-0133 | Each result shall capture value, unit, abnormal_flag, notes, completed_at, completed_by_user_id. | M | 1 | `db.rs` (`lab_order_tests`) |
| FR-0134 | Lab ordering shall require `lab.order`; result entry shall require `lab.result.manage`; catalog management shall require `lab.catalog.manage`. | M | 1 | `rbac.rs` |
| FR-0135 | The lab technician role shall not include patient create/update/delete. | M | 1 | `rbac.rs::ROLE_LAB_TECH` |

### 4.12 Radiology (FR-0140–FR-0149) — Phase 2, PLANNED

| ID | Requirement | Priority | Phase | Implementation |
|---|---|---|---|---|
| FR-0140 | The system shall maintain a radiology study catalog (modality, body part, contrast, price). | M | 2 | _planned_ |
| FR-0141 | The system shall support radiology orders analogous to lab orders, with study status workflow (ordered/scheduled/in-progress/reported/verified). | M | 2 | _planned_ |
| FR-0142 | Radiology reports shall be versioned; the verified-by radiologist shall be recorded. | M | 2 | _planned_ |
| FR-0143 | DICOM image storage is out of scope; integration with a PACS via worklist (DICOM MWL) is a Could. | C | 2 | _planned_ |

### 4.13 Billing (FR-0150–FR-0159) — Phase 1, IMPLEMENTED

| ID | Requirement | Priority | Phase | Implementation |
|---|---|---|---|---|
| FR-0150 | The system shall maintain `bills` (bill_number unique, bill_type, total_amount, discount, tax, net_amount, status) and `bill_items` (item_type, description, quantity, unit_price, total, reference_id). | M | 1 | `db.rs`, `commands/billing.rs` |
| FR-0151 | The server shall recompute `net_amount` from items + discount + tax; the client-supplied total shall be ignored. | M | 1 | `commands/billing.rs::create_bill` |
| FR-0152 | Money fields shall be NUMERIC(14,2); round-trip via `rust_decimal::Decimal`. | M | 1 | `models.rs`, `commands/billing.rs` |
| FR-0153 | Recording a payment (`payments` row) shall roll up to `bills.status`: `paid` if sum(payments) ≥ net_amount, else `partial`. | M | 1 | `commands/billing.rs::record_payment` |
| FR-0154 | Bill create shall require `billing.create`; payment recording shall require `payments.manage`. | M | 1 | `rbac.rs` |
| FR-0155 | The `bill_number` shall be generated server-side; clients shall not supply it. | M | 1 | `commands/billing.rs` |
| FR-0156 | Bill items may reference an encounter, lab order, or IPD admission via `reference_id` (loose reference; no FK). | S | 1 | `db.rs` (`bill_items.reference_id`) |

### 4.14 Invoicing (FR-0160–FR-0164) — Phase 2, PLANNED (Phase 1 bills serve as invoices)

| ID | Requirement | Priority | Phase | Implementation |
|---|---|---|---|---|
| FR-0160 | The system shall support invoice generation from one or more bills (consolidated invoicing for insurance claims). | M | 2 | _planned_ |
| FR-0161 | Invoices shall carry GST/tax breakdown by line item. | S | 2 | _planned_ |
| FR-0162 | Invoices shall be printable as A4 PDF with hospital letterhead. | S | 2 | _planned_; `printpdf` already a dependency |
| FR-0163 | Credit notes shall be supported against paid invoices. | C | 2 | _planned_ |
| FR-0164 | The Phase 1 `bills` table doubles as the invoice of record; Phase 2 introduces `invoices` as a presentation layer. | M | 1→2 | `db.rs` |

### 4.15 Payments (FR-0170–FR-0179) — Phase 1, IMPLEMENTED

| ID | Requirement | Priority | Phase | Implementation |
|---|---|---|---|---|
| FR-0170 | The system shall maintain `payments` (bill_id, amount, payment_method, reference_number, paid_at, received_by_user_id). | M | 1 | `db.rs`, `commands/billing.rs::record_payment` |
| FR-0171 | payment_method shall be one of cash, card, upi, insurance, bank_transfer. | M | 1 | `commands/billing.rs` |
| FR-0172 | Payments shall require `payments.manage`. | M | 1 | `rbac.rs` |
| FR-0173 | Deleting a bill shall be restricted if payments exist (`ON DELETE RESTRICT` on `payments.bill_id`). | M | 1 | `db.rs` |
| FR-0174 | The `received_by_user_id` shall be set from the session for cashier provenance. | M | 1 | `commands/billing.rs` |

### 4.16 Inventory (FR-0180–FR-0189) — Phase 1, IMPLEMENTED (basic)

| ID | Requirement | Priority | Phase | Implementation |
|---|---|---|---|---|
| FR-0180 | The system shall maintain `inventory_items` (name, sku, category, unit, stock_quantity, reorder_level, expiry_date, batch_number, unit_cost). **[Implemented v0.2.0 — Batch 1 CR-21 added `commands/inventory.rs` with 6 commands and `pages/Inventory.tsx`; sidebar entry gated by `InventoryView`.]** | M | 1 | `db.rs`, `commands/inventory.rs` (CR-21 v0.2.0), `pages/Inventory.tsx` |
| FR-0181 | Inventory management shall require `inventory.manage`; viewing shall require `inventory.view`. **[Implemented v0.2.0 — Batch 1 CR-21.]** | M | 1 | `rbac.rs`, `commands/inventory.rs` (CR-21 v0.2.0) |
| FR-0182 | Stock adjustments (receipts, issues, returns) shall be recorded in an `inventory_movements` audit table (Phase 2). **[Implemented v0.2.0 — Batch 1 CR-21 created the `inventory_movements` table (item_id, quantity_change, reason, created_by_user_id, created_at) and `adjust_inventory` writes a movement row on every adjustment.]** | M | 1 (moved from Phase 2) | `db.rs` (`inventory_movements`), `commands/inventory.rs::adjust_inventory` (CR-21 v0.2.0) |
| FR-0183 | The system shall surface low-stock alerts when `stock_quantity ≤ reorder_level`. | S | 2 | _planned_ |
| FR-0184 | The system shall surface near-expiry alerts based on `expiry_date`. | S | 2 | _planned_ |
| FR-0185 | Inventory adjustments shall be audited. **[Implemented v0.2.0 — Batch 1 CR-21: `adjust_inventory` writes an `audit::for_session` row plus an `inventory_movements` row for every stock change.]** | M | 1 | `commands/inventory.rs::adjust_inventory` (CR-21 v0.2.0) |

### 4.17 Blood Bank (FR-0190–FR-0199) — Phase 2, PLANNED

| ID | Requirement | Priority | Phase | Implementation |
|---|---|---|---|---|
| FR-0190 | The system shall maintain a blood inventory (blood_group, component, volume_ml, collection_date, expiry_date, donor_id). | M | 2 | _planned_ |
| FR-0191 | Cross-match records shall link a blood unit to a patient and an IPD admission. | M | 2 | _planned_ |
| FR-0192 | Discarded/expired units shall be retained with status for traceability. | M | 2 | _planned_ |

### 4.18 HR — Human Resources (FR-0200–FR-0209) — Phase 2, PLANNED

| ID | Requirement | Priority | Phase | Implementation |
|---|---|---|---|---|
| FR-0200 | The system shall maintain an `employees` registry (separate from `users`) covering both clinical and non-clinical staff. | M | 2 | _planned_ |
| FR-0201 | Employee records shall include designation, department, joining_date, employment_type, contact. | M | 2 | _planned_ |
| FR-0202 | HR management shall require a new `hr.manage` permission. | M | 2 | _planned_ |

### 4.19 Payroll (FR-0210–FR-0219) — Phase 2, PLANNED

| ID | Requirement | Priority | Phase | Implementation |
|---|---|---|---|---|
| FR-0210 | The system shall maintain `payroll_runs` (period, gross, deductions, net, status). | M | 2 | _planned_ |
| FR-0211 | Payslips shall be generated per employee per run and printable as PDF. | S | 2 | _planned_ |
| FR-0212 | Payroll shall integrate with attendance and leave (Phase 2 HR module). | S | 2 | _planned_ |

### 4.20 Reports (FR-0220–FR-0229) — Phase 2, PLANNED (Phase 1 KPIs exist)

| ID | Requirement | Priority | Phase | Implementation |
|---|---|---|---|---|
| FR-0220 | The system shall provide operational reports: daily OPD summary, IPD census, revenue, lab turnaround. | M | 2 | _planned_ |
| FR-0221 | Reports shall be exportable to CSV and PDF. | S | 2 | _planned_ |
| FR-0222 | Reports access shall require `reports.view`. | M | 1 (perm exists) / 2 (reports built) | `rbac.rs::Permission::ReportsView` |
| FR-0223 | The audit log shall be queryable in-app by users holding `audit.view`. | M | 1 | `audit.rs::get_audit_logs`, `pages/AuditLog.tsx` |

### 4.21 Admin / Settings (FR-0230–FR-0239) — Phase 1, IMPLEMENTED

| ID | Requirement | Priority | Phase | Implementation |
|---|---|---|---|---|
| FR-0230 | The system shall persist machine-wide configuration in `%ProgramData%\HMS\config.json` written by the installer and readable/writable by all interactive users. | M | 1 | `config.rs`, `windows/hooks.nsh` |
| FR-0231 | The system shall provide a Settings page exposing clinic name, WhatsApp group, and an Advanced section for server IP / TLS fingerprint. | M | 1 | `pages/Settings.tsx` |
| FR-0232 | Settings management shall require `settings.manage`. | M | 1 | `rbac.rs::Permission::SettingsManage` |
| FR-0233 | The system shall provide a "Connect a New Client PC" panel (server-build only) that generates a short-lived pairing code with countdown. | M | 1 | `pages/Settings.tsx`, `pairing.rs` |
| FR-0234 | The system shall display license info (hospital name, edition, modules, expiry, fingerprint match) on Settings → License, gated by `settings.manage` or `license.manage`. | M | 1 | `pages/Settings.tsx`, `license.rs::get_license_info` |
| FR-0235 | The system shall expose the local machine's hardware fingerprint via `get_hardware_fingerprint` for license issuance. | M | 1 | `license.rs::get_hardware_fingerprint` |
| FR-0236 | The system shall expose the embedded company public-key fingerprint via `get_license_public_key_fingerprint` for verification. | S | 1 | `license.rs::get_license_public_key_fingerprint` |
| FR-0237 | The system shall support license installation via `install_license` (file picker), which verifies signature + fingerprint before persisting. | M | 1 | `license.rs::install_license` |
| FR-0238 | The system shall refuse to boot past the license gate when the license is missing, unsigned, expired, or fingerprint-mismatched. | M | 1 | `license.rs::verify_license`, `App.tsx` |
| FR-0239 | The audit log page shall support filtering by action and resource and pagination up to 5000 rows. | M | 1 | `audit.rs::get_audit_logs`, `pages/AuditLog.tsx` |

### 4.22 Licensing — detailed requirements (FR-0240–FR-0259) — Phase 1, IMPLEMENTED

See §6 of this document and `07-Licensing-Architecture.md` for the full licensing model. The functional requirements below are the SRS-facing summary.

| ID | Requirement | Priority | Phase | Implementation |
|---|---|---|---|---|
| FR-0240 | A license shall bind to exactly one hospital identity (hospital_id, hospital_name) and exactly one deployment_id. | M | 1 | `license.rs::LicenseFile` |
| FR-0241 | A license shall bind to exactly one hardware fingerprint (the designated server PC). | M | 1 | `license.rs` |
| FR-0242 | The license signature algorithm shall be Ed25519; the embedded verification key shall be the only key shipped with the app. | M | 1 | `license.rs::COMPANY_PUBLIC_KEY` |
| FR-0243 | The canonical signing representation shall be compact JSON over a `BTreeMap` (sorted keys), excluding the `signature` field. | M | 1 | `license.rs::LicenseFile::canonical_bytes` |
| FR-0244 | License verification shall be DB-free and run before any database connection is opened. | M | 1 | `license.rs::verify_license` |
| FR-0245 | License verification shall be performed on install, on first activation, on every startup, on upgrade, and on renewal. | M | 1 | `App.tsx` boot flow, `license.rs` |
| FR-0246 | The system shall reject forged licenses (signature verification failure) with a clear error and refuse to proceed. | M | 1 | `license.rs::verify_signature` |
| FR-0247 | The system shall reject licenses whose hardware fingerprint does not match this machine. | M | 1 | `license.rs::verify_license_file` |
| FR-0248 | The system shall reject expired licenses (if `expiration_date` is in the past). | M | 1 | `license.rs::verify_license_file` |
| FR-0249 | The system shall permit perpetual licenses (`expiration_date = null`) while still enforcing `maintenance_until` for upgrade entitlement. | S | 1 | `license.rs` |
| FR-0250 | The persisted `license_state` row (id = 1) shall record `license_json`, `hardware_fingerprint`, `installed_at`, `last_verified_at`, `verification_status`. | M | 1 | `db.rs` (`license_state`), `license.rs::persist_verification` |
| FR-0251 | The license shall enumerate `enabled_modules`; future Phase 2 modules shall check entitlement at runtime. | S | 1→2 | `license.rs::LicenseFile::enabled_modules` |
| FR-0252 | The license shall carry `software_version_min` / `software_version_max` so an out-of-band software upgrade can be blocked by the issuer. | S | 1 | `license.rs::LicenseFile` |
| FR-0253 | A deployment shall not share its database, signing keys, license file, or hospital identity with any other deployment. | M | 1 | Architectural constraint — see §6 |
| FR-0254 | The system shall support revoking a previously-installed license via `revoke_license`. Revocation shall delete the on-disk `license.json`, clear the in-memory `LicenseInfo`, write an audit row (`action=license_revoke`), and force the boot flow back to the license-gate phase on the next `verify_license` call. Revocation shall require `license.manage`. **[Implemented v0.2.0 — Batch 3 LIC-DOC-04.]** | M | 1 | `license.rs::revoke_license` (LIC-DOC-04 v0.2.0) |

### 4.23 IPC command inventory (v0.2.0)

A consolidated list of Tauri IPC commands exposed by the Rust backend, grouped by module. Commands marked **[new v0.2.0]** were added in Phase 2 Batches 1-3. The full handler registration lives in `src-tauri/src/lib.rs::run()` via `tauri::generate_handler![]`.

| Module | Commands |
|---|---|
| Config | `get_config` (RBAC-gated once `setup_complete` since CR-4 v0.2.0; `db_password` is `#[serde(skip_serializing)]`), `save_config`, `get_local_ip`, `test_server_connection`, `repair_server_config`, `get_config_path` |
| Discovery | `get_server_role` |
| Pairing | `generate_pairing_code`, `get_pairing_status`, `redeem_pairing_code`, `verify_pairing` |
| Core init | `initialize_database`, `check_db_connection`, `complete_pairing_and_connect`, `get_log_path`, `get_log` |
| Auth & RBAC | `login`, `logout`, `me`, `change_password`, `list_users`, `create_user`, `update_user`, `delete_user`, `reset_user_password`, `list_roles`, `list_user_roles` |
| Audit | `get_audit_logs` |
| Licensing | `verify_license`, `get_license_info`, `install_license`, `revoke_license` **[new v0.2.0 — Batch 3 LIC-DOC-04]**, `get_hardware_fingerprint`, `get_license_public_key_fingerprint`, `get_install_fingerprint` |
| Dashboard | `get_dashboard_kpis` |
| Patients | `create_patient`, `get_patients`, `get_patient`, `update_patient`, `delete_patient` (soft-delete since CR-11 v0.2.0), `get_patient_consent` **[new v0.2.0 — CR-12]**, `set_patient_consent` **[new v0.2.0 — CR-12]**, `revoke_patient_consent` **[new v0.2.0 — CR-12]** |
| Doctors | `create_doctor`, `get_doctors`, `get_doctor`, `update_doctor`, `delete_doctor`, `get_specializations` |
| Appointments | `create_appointment`, `get_appointments`, `get_appointment`, `update_appointment`, `update_appointment_status`, `delete_appointment`, `get_today_appointments`, `get_appointment_stats` |
| Encounters | `get_encounters`, `create_encounter` |
| Queue | `get_queue`, `create_queue_token`, `call_next_token`, `set_token_status` |
| IPD | `get_wards`, `create_ward`, `get_beds`, `create_bed`, `get_admissions`, `admit_patient`, `discharge_patient` |
| Laboratory | `get_lab_catalog`, `create_lab_test`, `get_lab_orders`, `create_lab_order`, `get_lab_order_tests`, `update_lab_result` |
| Billing | `get_bills`, `get_bill`, `get_bill_items`, `create_bill`, `record_payment`, `get_payments` |
| Inventory | `get_inventory_items` **[new v0.2.0 — CR-21]**, `get_inventory_item` **[new v0.2.0 — CR-21]**, `create_inventory_item` **[new v0.2.0 — CR-21]**, `update_inventory_item` **[new v0.2.0 — CR-21]**, `adjust_inventory` **[new v0.2.0 — CR-21]**, `get_inventory_movements` **[new v0.2.0 — CR-21]** |
| Messaging | `send_message` (RBAC-gated `MessagingSend` + sender from session since CR-16 v0.2.0), `get_messages` (RBAC `MessagingView`), `delete_message`, `get_rooms` |
| WhatsApp | `send_whatsapp_notification`, `send_whatsapp_to_patient`, `send_whatsapp_test`, `get_notification_log`, `get_whatsapp_config`, `set_whatsapp_config`, `test_whatsapp_api` |

**Known IPC gap (Planned for Batch 5 cleanup):** `clear_config` is implemented in `config.rs` but is NOT registered in `tauri::generate_handler![]`, so it cannot be invoked from the frontend. Either register it or delete the dead function in Batch 5.

---

## 5. External interface requirements

### 5.1 User interfaces

The UI is a React 19 SPA rendered inside the Tauri v2 webview. Routing uses `HashRouter` (required for Tauri's production custom protocol, which has no SPA fallback). State management uses TanStack React Query v5 with centralised query keys. Animation uses `motion` (the maintained successor to `framer-motion`). The design system is documented in `DESIGN_SYSTEM.md`. WCAG 2.1 AA is targeted via accessible tokens, focus rings, and `prefers-reduced-motion` respect.

### 5.2 Hardware interfaces

| Interface | Use | Notes |
|---|---|---|
| Windows WMI (`Win32_Processor`, `Win32_BaseBoard`, `Win32_BIOS`) | Hardware fingerprint | `wmi` crate, gated to `target_os = "windows"` |
| LAN (TCP 5432 PostgreSQL, UDP discovery broadcast, TCP pairing listener) | Multi-PC topology | `discovery.rs`, `pairing.rs` |
| Thermal printer (80mm) | Appointment/bill receipts | Browser `window.print()` via `@media print` CSS |
| Standard A4 printer | Invoices (Phase 2) | `printpdf` already a dependency |

### 5.3 Software interfaces

| Interface | Direction | Notes |
|---|---|---|
| PostgreSQL 16+ (bundled) | Backend | `sqlx` 0.8 with `tls-rustls`; `scram-sha-256` auth |
| Windows Service Control Manager | Backend | PostgreSQL registered as `HMS-PostgreSQL` auto-start service |
| NSIS installer | Build / deploy | `windows/hooks.nsh` for `NSIS_HOOK_POSTINSTALL`/`NSIS_HOOK_PREUNINSTALL` |
| WhatsApp Web (via automation) | Outbound notifications | `whatsapp/automation.rs`; no Business API used |
| Tauri IPC bridge | Frontend ↔ Rust | `invoke()` from React, `#[tauri::command]` in Rust |

### 5.4 Communications interfaces

| Channel | Protocol | Encryption | Notes |
|---|---|---|---|
| Server ↔ PostgreSQL (loopback) | TCP 5432 | `sslmode=require` | Acceptable loopback; cert not validated |
| Client ↔ PostgreSQL (LAN) | TCP 5432 | `sslmode=verify-ca` + pinned server cert | Defeats sniffing and server impersonation |
| Server ↔ Client pairing | TCP listener (pairing port) | TLS (rustls) | Short-lived; 6-char code, 10-minute expiry |
| LAN discovery | UDP broadcast | None (plaintext presence beacon only) | Contains only server IP + port, no credentials |

---

## 6. Licensing requirements (detailed)

This section consolidates the licensing model. The full architectural treatment is in `07-Licensing-Architecture.md`.

### 6.1 Single-hospital license

- **R-6.1** Each license file is bound to exactly one `hospital_id` and `hospital_name`. The system shall display these on the boot screen and on Settings → License.
- **R-6.2** Each license is bound to exactly one `deployment_id` (a UUID-like identifier the issuer assigns per hospital site).
- **R-6.3** A license is bound to exactly one hardware fingerprint (the designated server PC). Client PCs in the same deployment do not have separate licenses — they pair to the server, which holds the license.
- **R-6.4** A deployment shall not share its PostgreSQL database, signing keys, license file, or hospital identity with any other deployment. The database name defaults to `hms` and is not configurable to a shared cluster.

### 6.2 Hardware-bound

- **R-6.5** The hardware fingerprint shall be `SHA-256` over the byte string `b"vitalflow-hms-fp-v1\0" || cpu_id || b"\0" || board_sn || b"\0" || bios_sn`, where the three components are obtained from WMI on Windows.
- **R-6.6** The fingerprint shall be hex-encoded (64 chars) and stable across OS updates, driver changes, and reboots. It shall change if the CPU or motherboard is replaced, which is the desired license-rejection trigger.
- **R-6.7** A non-Windows fallback fingerprint (over hostname + OS) exists for development only and shall not be used in any production deployment.

### 6.3 Signed license

- **R-6.8** The license file is JSON. Every field except `signature` is included in the canonical signing representation.
- **R-6.9** Canonicalisation is `serde_json::to_vec` over a `BTreeMap<&str, Value>` populated with each non-signature field, producing sorted-key compact JSON. Both signer and verifier use this exact construction.
- **R-6.10** The signature is Ed25519 over the canonical bytes, base64-encoded, stored in `LicenseFile.signature`.
- **R-6.11** The application embeds only the company public key (`COMPANY_PUBLIC_KEY`, 32 bytes). The matching private key is held offline by the software company and never distributed.
- **R-6.12** ~~Until a real key is provisioned at build time, `COMPANY_PUBLIC_KEY` is all-zeros and shall reject every signature by design — forcing explicit key provisioning before any license can verify.~~ **[Updated v0.2.0]** Batch 2 (CR-20) replaced the all-zeros placeholder with a real development Ed25519 keypair generated by the new `keygen/` project (see `10-Licensing-Workflow-Guide.md`). The embedded `COMPANY_PUBLIC_KEY` (32 bytes at `license.rs:49-54`) now accepts signatures from the matching dev private key. Production deployments MUST replace the dev keypair with a production keypair before ship; the dev private key (`src-tauri/src/bin/dev_auto_license.rs`) MUST NOT ship to production.

### 6.4 Verification sequence

See `07-Licensing-Architecture.md` §5 for the full sequence and rejection-case matrix.

### 6.5 License file fields

| Field | Type | Purpose |
|---|---|---|
| `license_id` | string | Issuer-assigned unique ID |
| `hospital_id` | string | Stable hospital identifier |
| `hospital_name` | string | Human-readable hospital name (shown on boot) |
| `deployment_id` | string | Issuer-assigned deployment UUID |
| `hardware_fingerprint` | string (hex 64) | Target machine fingerprint |
| `license_version` | string | License format version |
| `product_edition` | string | Edition tag (e.g. `Enterprise`) |
| `enabled_modules` | string[] | Module entitlement list |
| `issue_date` | ISO-8601 | Issue timestamp |
| `expiration_date` | ISO-8601 or null | Hard expiry; null = perpetual |
| `maintenance_until` | ISO-8601 | Update entitlement window |
| `software_version_min` | string | Minimum app version this license permits |
| `software_version_max` | string | Maximum app version this license permits |
| `signature` | base64 | Ed25519 signature over canonical bytes of all other fields |

---

## 7. Non-functional requirements

NFRs use the numbering `NFR-NN` and are mapped to ISO/IEC 25010 characteristics in `03-Quality-Model-ISO-25010.md`.

### 7.1 Performance efficiency

| ID | Requirement | Target |
|---|---|---|
| NFR-01 | Dashboard KPI query (`get_dashboard_kpis`) shall return within 800 ms p95 on a 5-year-old server PC with up to 100k appointments, 50k patients, 10k IPD admissions. | p95 < 800 ms |
| NFR-02 | Patient list (`get_patients`) shall return within 1.0 s p95 for up to 50k rows. | p95 < 1.0 s |
| NFR-03 | Bill creation (`create_bill`) including server-side total recomputation shall complete within 500 ms p95. | p95 < 500 ms |
| NFR-04 | Login (Argon2id verify + session write + audit insert) shall complete within 1.5 s p95. Argon2id parameters are intentionally costly. | p95 < 1.5 s |
| NFR-05 | License verification (`verify_license`) shall complete within 500 ms p95 including WMI queries. | p95 < 500 ms |
| NFR-06 | The connection pool default max connections shall not exceed 10 to avoid starving the bundled PostgreSQL. | ≤ 10 |

### 7.2 Security

| ID | Requirement |
|---|---|
| NFR-10 | Passwords shall be Argon2id (m=19456 KiB, t=2, p=1) per OWASP 2023 minimum. |
| NFR-11 | Account lockout after 5 failed attempts for 15 minutes. |
| NFR-12 | Single active session per user. |
| NFR-13 | Session tokens 32 random bytes, SHA-256 hashed at rest. |
| NFR-14 | Every state-changing command shall write an audit log entry. |
| NFR-15 | Every protected command shall enforce RBAC via `rbac::require`. **[Implemented v0.2.0 — Batch 1 (CR-4) added `rbac::require(SettingsManage)` to `get_config` once `setup_complete`; (CR-16) added `rbac::require(MessagingView/MessagingSend)` to all 4 messaging commands with sender derived from session; Batch 3 (SEC-05) added RBAC to log-reading commands and redacted PHI from log output. Two new permissions `MessagingView` and `MessagingSend` were added to `rbac.rs`. WhatsApp commands remain RBAC-gated via `whatsapp.manage`; `db_password` is `#[serde(skip_serializing)]` so it is no longer exposed to any frontend caller.]** |
| NFR-16 | License verification shall be cryptographically signed (Ed25519) and DB-free. |
| NFR-17 | LAN PostgreSQL connections shall use TLS with pinned server certificate (`sslmode=verify-ca`). |
| NFR-18 | `pg_hba.conf` shall allow only loopback + private LAN ranges with `scram-sha-256`. |
| NFR-19 | The PostgreSQL `postgres` superuser password shall be 24 random bytes generated via `RandomNumberGenerator`, never NSIS PRNG. |
| NFR-20 | No hardcoded backdoor account shall exist. |
| NFR-21 | No PHI shall be written to application log files (`hms_startup.log`). |
| NFR-22 | The frontend shall never receive `password_hash` from any command. |

### 7.3 Reliability

| ID | Requirement |
|---|---|
| NFR-30 | The installer shall never destroy an existing PostgreSQL data directory on reinstall (idempotent migrations, `IF NOT EXISTS`). |
| NFR-31 | Idempotent migrations (`CREATE TABLE IF NOT EXISTS`, `ADD COLUMN IF NOT EXISTS`) shall be safe to re-run. |
| NFR-32 | Audit insert failures shall be swallowed (logged to stderr) and shall never block a clinical operation — availability over completeness. |
| NFR-33 | The client-build shall self-heal from a stale server IP via LAN broadcast discovery and persist the recovered IP. |
| NFR-34 | The server-build shall detect and auto-repair broken SSL configuration in `pg_hba.conf` / `postgresql.conf` at startup. |
| NFR-35 | Target uptime: 99% during hospital operating hours (single-server, no redundancy — see `05-Risk-Register-ISO-31000.md` R-005). |

### 7.4 Usability

| ID | Requirement |
|---|---|
| NFR-40 | WCAG 2.1 AA contrast for all text and status colours against both light and dark themes. |
| NFR-41 | `prefers-reduced-motion: reduce` shall be respected globally (CSS and Motion). |
| NFR-42 | All icon-only buttons shall have `title` attributes and, where intent is non-obvious, `aria-label`. |
| NFR-43 | The boot flow shall display staged progress messages (`init_status` events) so the user understands what the app is doing. |
| NFR-44 | The first-run admin shall be forced to change password before reaching the shell. |
| NFR-45 | The sidebar shall be permission-filtered: a user shall never see a nav item they cannot access. |

### 7.5 Maintainability

| ID | Requirement |
|---|---|
| NFR-50 | TypeScript strict mode with `noUnusedLocals`/`noUnusedParameters` shall pass with zero errors (`tsc --noEmit`). **[Implemented v0.2.0 — Batch 0 created `tsconfig.json` (strict + `noUncheckedIndexedAccess` + `noImplicitReturns` + `forceConsistentCasingInFileNames`), `vite.config.ts` (with `@/*` path alias), `eslint.config.js` (ESLint 9 flat config + typescript-eslint + react-hooks + react-refresh + prettier integration), `.prettierrc.json`. `npx tsc --noEmit` and `npx eslint .` both pass with zero errors after every batch (B0/B1/B2/B3).]** |
| NFR-51 | All TypeScript shapes mirroring Rust structs shall be defined once in `src/lib/models.ts` and imported, never redefined per page. |
| NFR-52 | All React Query hooks shall be defined in `src/lib/queries.ts` with centralised query keys (`qk`). |
| NFR-53 | Permission keys shall be defined once in `rbac.rs::Permission::as_str` and mirrored in `src/lib/rbac.ts`. |
| NFR-54 | SQL migrations shall be additive (no destructive `DROP`); schema changes shall be reviewable in `db.rs::run_migrations`. |
| NFR-55 | New modules shall follow the existing pattern: `commands/<module>.rs` with `rbac::require` + `audit::for_session` at each write. |

### 7.6 Portability

| ID | Requirement |
|---|---|
| NFR-60 | Production target is Windows 10/11 x64 only. Non-Windows builds are dev-only. |
| NFR-61 | Windows-only dependencies (`wmi`) shall be gated with `#[cfg(target_os = "windows")]` so non-Windows builds compile. |
| NFR-62 | The server vs client build distinction shall be compile-time (`server-build` / `client-build` Cargo features), not runtime. |
| NFR-63 | Configuration shall resolve from `%ProgramData%\HMS\config.json` first, falling back to per-user app data for dev mode. |

### 7.7 Compatibility

| ID | Requirement |
|---|---|
| NFR-70 | Co-existence: server-build and client-build installers shall have distinct product names and identifiers so both can coexist on a single test PC. |
| NFR-71 | Interoperability: PostgreSQL 13+ is required; the bundled version is 16+ for `scram-sha-256` default and JSONB features. |
| NFR-72 | The Tauri IPC contract (command names + DTO shapes) is the interface between frontend and backend; breaking changes require coordinated releases. |

---

## 8. Constraints

| ID | Constraint |
|---|---|
| C-01 | The deployment model is LAN-only; the system shall not be exposed to the public internet. |
| C-02 | The application shall be a Tauri v2 desktop binary; no migration to Electron, Next.js, or web-only. |
| C-03 | One PostgreSQL cluster per deployment; no shared cluster across hospitals. |
| C-04 | One active desktop session per process (single-user app at any moment). |
| C-05 | The license private key shall be held offline by the software company; the application binary contains only the public key. |
| C-06 | The bundled PostgreSQL binaries shall be shipped inside the installer (no runtime download). |
| C-07 | The installer shall not request elevation from the running HMS app; provisioning happens once, at install time, while elevated. |
| C-08 | The codebase shall not introduce a separate ORM (e.g. SeaORM, Diesel) — `sqlx` is the data layer. |
| C-09 | Frontend routing shall use `HashRouter`, not `BrowserRouter`, due to Tauri's production custom protocol lacking SPA fallback. |
| C-10 | Animation library shall be `motion` (the maintained successor to `framer-motion`). |

---

## 9. Assumptions and dependencies

| ID | Assumption / Dependency |
|---|---|
| A-01 | The hospital operates a single LAN segment with no routing between client PCs and the server PC. |
| A-02 | Windows Firewall on the server PC is configured to allow inbound TCP 5432 from the LAN. |
| A-03 | The software company issues Ed25519-signed license files offline using a key generated and rotated per the procedure in `07-Licensing-Architecture.md` §10. |
| A-04 | The bundled PostgreSQL binaries are supplied by the engineering team into `src-tauri/resources/pgsql` before the server installer is built (see `SETUP_POSTGRES_BINARIES.md`). |
| A-05 | The hospital's IT designates one PC as the server; this PC remains powered on during operating hours. |
| A-06 | Windows 10/11 x64 with .NET runtime present (required for WMI access). |
| A-07 | Audit log retention is sufficient for the hospital's regulatory window; archival is operational, not in-product (Phase 2 will add archival). |

---

## 10. Traceability

### 10.1 Requirements to standards

| Standard | Clause | Mapped VitalFlow requirements |
|---|---|---|
| ISO/IEC/IEEE 29148 | §5.2 SRS structure | This document, §1–§10 |
| ISO/IEC 25010 | Functional suitability | FR-0010–FR-0253 |
| ISO/IEC 25010 | Performance efficiency | NFR-01–NFR-06 |
| ISO/IEC 25010 | Security | NFR-10–NFR-22, FR-0020–FR-0029, FR-0240–FR-0253 |
| ISO/IEC 25010 | Reliability | NFR-30–NFR-35 |
| ISO/IEC 25010 | Usability | NFR-40–NFR-45 |
| ISO/IEC 25010 | Maintainability | NFR-50–NFR-55 |
| ISO/IEC 25010 | Portability | NFR-60–NFR-63 |
| ISO/IEC 25010 | Compatibility | NFR-70–NFR-72 |
| ISO/IEC 27001:2022 | A.5.15 Access control | FR-0020–FR-0029, NFR-15 |
| ISO/IEC 27001:2022 | A.5.17 Authentication info | NFR-10–NFR-13, FR-0027–FR-0028 |
| ISO/IEC 27001:2022 | A.8.5 Secure authentication | NFR-10–NFR-13 |
| ISO/IEC 27001:2022 | A.8.16 Monitoring | FR-0022, NFR-14 |
| ISO/IEC 27001:2022 | A.8.24 Cryptography | NFR-10, NFR-16, NFR-17 |
| ISO 31000 | Risk treatment | `05-Risk-Register-ISO-31000.md` |
| IEEE 1016 | Design description | `02-SDD-Software-Design.md` |
| ISO/IEC/IEEE 12207 | Life cycle | `06-SDLC-ISO-12207.md` |

### 10.2 Requirements to implementation

A representative subset; the full mapping is the responsibility of the QA lead during the Phase 2 verification cycle.

| Requirement | Implementation file(s) | Verification |
|---|---|---|
| FR-0020 | `auth.rs::hash_password`, `auth.rs::verify_password` | Manual review; `cargo test` planned (see `06-SDLC-ISO-12207.md`) |
| FR-0021 | `auth.rs::login` (lockout branch) | Manual review |
| FR-0024 | `auth.rs::login` (`DELETE FROM sessions WHERE user_id = $1`) | Manual review |
| FR-0062 | `commands/ipd.rs::admit_patient` (transaction) | Manual review |
| FR-0132 | `commands/lab.rs::update_lab_result` (auto-complete) | Manual review |
| FR-0151 | `commands/billing.rs::create_bill` (server-side total) | Manual review |
| FR-0240 | `license.rs::LicenseFile` | Manual review |
| FR-0244 | `license.rs::verify_license` (DB-free) | Manual review |
| NFR-10 | `auth.rs::argon2` (Params::new(19_456, 2, 1, None)) | Manual review |
| NFR-17 | `db.rs::build_url` (client `sslmode=verify-ca`) | Manual review |

### 10.3 Open items / caveats

| Item | Status | Owner |
|---|---|---|
| ~~`COMPANY_PUBLIC_KEY` is all-zeros placeholder~~ | **[Updated v0.2.0]** Resolved — Batch 2 (CR-20) replaced the all-zeros placeholder with a real development Ed25519 keypair generated by the new `keygen/` project. Production deployments MUST still swap the dev keypair for a production keypair before ship; the dev private key (`src-tauri/src/bin/dev_auto_license.rs`) MUST NOT ship to production. | Software company |
| No `cargo test` suite exists yet | Open — planned for Phase 2 SDLC | Engineering |
| Phase 2 modules (Nurses, Pharmacy, Radiology, Blood Bank, HR, Payroll, Reports) are not implemented | Planned | Engineering |
| No automated backup UI | Open — manual `pg_dump` only | Operations |
| ~~No `npx tsc --noEmit` regression CI~~ | **[Updated v0.2.0]** Resolved — Batch 0 created `tsconfig.json` (strict gate) + `eslint.config.js` + `.prettierrc.json`. `npx tsc --noEmit` and `npx eslint .` pass with zero errors after every batch. A formal CI pipeline that runs the gate on every commit is still Planned Phase 2. | Engineering |
| `clear_config` dead IPC command | Open — implemented in `config.rs` but not registered in `generate_handler![]` (see §4.23) | Engineering (Batch 5 cleanup) |

---

_End of `01-SRS-Software-Requirements.md`. Cross-reference `02-SDD-Software-Design.md` for design, `03-Quality-Model-ISO-25010.md` for quality evaluation, and `07-Licensing-Architecture.md` for licensing detail._
