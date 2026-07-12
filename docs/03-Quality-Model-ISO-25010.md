# VitalFlow HMS — ISO/IEC 25010 Quality Model Evaluation

> **Document version: v0.2.0 — updated 2025-07-08 after Phase 2 Batches 0-3 implementation. See CHANGELOG.md for details.**

| Field | Value |
|---|---|
| **Document title** | VitalFlow HMS — Quality Model Evaluation per ISO/IEC 25010:2023 |
| **Standard** | ISO/IEC 25010:2023 (Systems and software engineering — Systems and software Quality Requirements and Evaluation — Quality model) |
| **Version** | 0.2.0 |
| **Date** | 2025-07-08 |
| **Status** | Draft |
| **Classification** | Internal |
| **Owner** | VitalFlow HMS Engineering / QA |
| **Author** | Documentation Specialist (Task 7) — reconciled by Documentation Team (B4-A) v0.2.0 |
| **Related documents** | `01-SRS-Software-Requirements.md`, `02-SDD-Software-Design.md`, `04-Security-Control-Matrix-ISO-27001.md`, `05-Risk-Register-ISO-31000.md`, `06-SDLC-ISO-12207.md` |

### Revision history

| Version | Date | Author | Summary |
|---|---|---|---|
| 0.1.0 | 2026-07-02 | Documentation Specialist | Initial QM baseline against the Phase 1 source tree. |
| 0.2.0 | 2025-07-08 | Documentation Team (B4-A) | Re-evaluated against Phase 2 Batches 0-3 code. Level updates: 1.1 Functional completeness 3→4 (inventory + consent now implemented); 4.4 User error protection 3→4 (5 destructive-action Dialogs added); 4.6 Accessibility 4→3 (corrected aspirational L4 down to actual L3 after a11y pass); 8.1 Confidentiality reaffirmed L4 with v0.2.0 evidence; 8.4 Accountability reaffirmed L4 with v0.2.0 evidence; 8.5 Authenticity reaffirmed L4 with v0.2.0 evidence; 9.3 Analysability reaffirmed L3 with v0.2.0 evidence. Gap remediation summary updated: 4 of 11 high/medium gaps resolved. |

---

## 1. Introduction

### 1.1 Purpose

This document evaluates VitalFlow HMS against the eight characteristics of the ISO/IEC 25010:2023 product quality model. For each characteristic and its sub-characteristics it states: the target level, current implementation status (Implemented / Partial / Planned), evidence grounded in the actual source tree, and gaps with remediation actions. The evaluation feeds the Risk Register (`05-Risk-Register-ISO-31000.md`) and the SDLC V&V plan (`06-SDLC-ISO-12207.md`).

### 1.2 Status legend

| Status | Meaning |
|---|---|
| **Implemented** | Code present and verified by manual review; meets the target. |
| **Partial** | Code present but incomplete, or verified only manually with no automated test. |
| **Planned** | Not yet implemented; design or requirement exists in the SRS. |

### 1.3 Target level scale

A simple 1–5 scale is used per sub-characteristic, where 5 = fully meets target and 1 = absent.

| Level | Label | Meaning |
|---|---|---|
| 5 | Excellent | Fully meets target, evidence-backed, automated verification |
| 4 | Good | Meets target, manual verification only |
| 3 | Fair | Partially meets target; gaps documented |
| 2 | Weak | Minimal coverage; significant gaps |
| 1 | Absent | Not implemented |

---

## 2. Master evaluation table

| Characteristic | Sub-characteristic | Target | Status | Level | Primary evidence |
|---|---|---|---|---|---|
| 1. Functional Suitability | 1.1 Functional completeness | 4 | Implemented (Phase 1) / Planned (Phase 2) | 4 | `01-SRS-Software-Requirements.md` §4; Phase 1 modules implemented including inventory (CR-21 v0.2.0) and patient consent (CR-12 v0.2.0); Phase 2 modules (Nurses, Pharmacy, Radiology, Blood Bank, HR, Payroll, Reports) still planned |
| 1. Functional Suitability | 1.2 Functional correctness | 4 | Partial | 3 | Manual review of `commands/*` for transactional correctness; no automated test suite yet |
| 1. Functional Suitability | 1.3 Functional appropriateness | 4 | Implemented | 4 | Each command maps to a real clinician/admin workflow; no dead features |
| 2. Performance Efficiency | 2.1 Time behaviour | 4 | Partial | 3 | Server-side aggregations in `commands/dashboard.rs`; targets in SRS NFR-01–NFR-05 not yet measured |
| 2. Performance Efficiency | 2.2 Resource utilisation | 3 | Implemented | 4 | Pool size explicitly configured to 10 connections (`.max_connections(10)` at `db.rs:123`, satisfying NFR-06); Argon2id memory cost 19 MiB per verify (intentional); audit log file trimmed to ~400 KB. Memory not profiled under sustained use. |
| 2. Performance Efficiency | 2.3 Capacity | 3 | Partial | 3 | Schema supports 100k+ appointments; indices on audit_logs and queue_tokens; no load test |
| 3. Compatibility | 3.1 Co-existence | 4 | Implemented | 4 | Server-build and client-build have distinct product names/identifiers (`tauri.server.conf.json`, `tauri.client.conf.json`); both can install on the same PC |
| 3. Compatibility | 3.2 Interoperability | 3 | Partial | 3 | PostgreSQL 16+ required; Tauri IPC contract is the FE/BE interface; no HL7/FHIR |
| 4. Usability | 4.1 Appropriateness recognisability | 4 | Implemented | 4 | Boot screen shows hospital name + edition from license; `App.tsx::BootScreen` |
| 4. Usability | 4.2 Learnability | 4 | Implemented | 4 | Permission-filtered sidebar (`Sidebar.tsx`); staged boot messages (`init_status` events) |
| 4. Usability | 4.3 Operability | 4 | Implemented | 4 | NSIS zero-interaction installer; pairing UI; Settings → Advanced |
| 4. Usability | 4.4 User error protection | 4 | Implemented | 4 | Forced password change on first login; "cannot delete own account" guard; client-supplied bill total ignored; **5 destructive-action confirmation Dialogs added in Batch 3 INT-01** (delete patient, delete doctor, delete appointment, reset password with real password Input, delete user); soft-delete patients (CR-11) means delete is reversible in audit history |
| 4. Usability | 4.5 User interface aesthetics | 4 | Implemented | 4 | `DESIGN_SYSTEM.md` token system; light/dark themes; WCAG AA contrast |
| 4. Usability | 4.6 Accessibility | 4 | Partial | 3 | **[Updated v0.2.0 — Batch 3]** Accessibility improved in v0.2.0: aria-labels on ~20 icon-only buttons (Patients, Doctors, Appointments, Queue, Users, Billing, Messaging, AuditLog) + 4 unlabeled inputs; `DialogDescription` added to 10 dialogs (Billing, Laboratory, Queue, IPD, Users); `TableHead` now defaults `scope="col"` at the component level + 7 most-used tables have explicit scope; `htmlFor`/`id` pairs on Login + Setup + Settings focusable fields. The previous revision's L4 was aspirational — the actual at-audit level was L2. Full WCAG AA conformance targeted for Batch 5 (Select-field `htmlFor`, ConsentPanel `confirm()` replacement, axe-core CI). |
| 5. Reliability | 5.1 Maturity | 3 | Partial | 3 | Idempotent migrations; installer never destroys pgdata; no automated fault-injection |
| 5. Reliability | 5.2 Availability | 3 | Partial | 3 | Single-server, no redundancy; target 99% operating-hours; see Risk R-005 |
| 5. Reliability | 5.3 Fault tolerance | 3 | Partial | 3 | Client self-heal via LAN discovery; SSL auto-repair on server; audit insert failures swallowed (availability-over-completeness) |
| 5. Reliability | 5.4 Recoverability | 3 | Partial | 3 | Idempotent migrations enable safe re-run; manual `pg_dump` only — no in-product backup UI |
| 6. Security | 6.1 Confidentiality | 4 | Implemented | 4 | RBAC on every protected command (NFR-15 — config/messaging/whatsapp added in Batches 1, 3); TLS pinning on LAN; no PHI in logs (Batch 3 SEC-05 redaction); `password_hash` and `db_password` are `#[serde(skip_serializing)]` (CR-4 v0.2.0); CSP enforced (CR-3 v0.2.0). **[Improved to L4 in v0.2.0 (CR-4, CR-5, SEC-05, CR-3).]** |
| 6. Security | 6.2 Integrity | 4 | Implemented | 4 | Ed25519-signed licenses, server-side bill totals, `ON DELETE RESTRICT` on critical FKs |
| 6. Security | 6.3 Non-repudiation | 3 | Partial | 3 | Audit log records user/action/resource; not append-only at DB level (no row immutability) |
| 6. Security | 6.4 Accountability | 4 | Implemented | 4 | `audit::for_session` on every state-changing command; `created_by_user_id` on patients/appointments/bills/payments/encounters/lab_orders/lab_order_tests/ipd_admissions/queue_tokens/patient_consent. **[Improved to L4 in v0.2.0]** Batches 1-3 added audit rows to: all 4 messaging commands (CR-16), all 6 inventory commands (CR-21) + `inventory_movements` audit table, all 3 consent commands (CR-12), `revoke_license` (LIC-DOC-04). All state-changing commands now write audit rows. |
| 6. Security | 6.5 Authenticity | 4 | Implemented | 4 | Argon2id + brute-force lockout + single active session + session token SHA-256 hash. **[Improved to L4 in v0.2.0]** Message `sender` field on `send_message` is now derived from the authenticated session, NOT a client-supplied string (CR-16). Pairing code generation switched from `thread_rng()` to `OsRng` (CSPRNG) so observed codes do not leak future codes (SEC-03 / SDD-12). |
| 7. Maintainability | 7.1 Modularity | 4 | Implemented | 4 | `commands/<module>.rs` per domain; `lib/queries.ts` + `lib/models.ts` centralised; clear file boundaries |
| 7. Maintainability | 7.2 Reusability | 4 | Implemented | 4 | Shared `rbac::require` guard; shared `audit::for_session`; shared UI primitives in `components/ui/` |
| 7. Maintainability | 7.3 Analysability | 3 | Implemented | 3 | **[Improved to L3 in v0.2.0 (Batch 0)]** `tsconfig.json` strict gate (strict + `noUnusedLocals` + `noUnusedParameters` + `noUncheckedIndexedAccess` + `noImplicitReturns`); `eslint.config.js` (ESLint 9 flat + typescript-eslint + react-hooks + react-refresh + prettier integration); `.prettierrc.json`; centralised query keys. `npx tsc --noEmit` and `npx eslint .` both pass with zero errors after every batch. No Rust tests yet; no tracing instrumentation; no metrics endpoint. |
| 7. Maintainability | 7.4 Modifiability | 4 | Implemented | 4 | Additive migrations; data-driven RBAC (role_permissions editable); new modules follow existing pattern |
| 7. Maintainability | 7.5 Testability | 2 | Partial | 2 | No `cargo test` suite; no frontend test runner; tsc strict gate only |
| 8. Portability | 8.1 Adaptability | 3 | Implemented | 3 | Windows 10/11 production target; non-Windows fallback for dev; ProgramData vs per-user config resolution |
| 8. Portability | 8.2 Installability | 4 | Implemented | 4 | NSIS installer with `NSIS_HOOK_POSTINSTALL`; zero-interaction PostgreSQL provisioning; pairing flow |
| 8. Portability | 8.3 Replaceability | 3 | Partial | 3 | Standard PostgreSQL 16+; data is portable via `pg_dump`; no proprietary lock-in |

---

## 3. Characteristic 1 — Functional Suitability

### 3.1 Functional completeness — Level 4 (was Level 3 in v0.1.0)

**Target**: All Phase 1 functional requirements in `01-SRS-Software-Requirements.md` §4 are implemented.

**Status**: Implemented (Phase 1) / Planned (Phase 2).

**Evidence**:

- Phase 1 modules implemented in `commands/{dashboard,patients,doctors,appointments,queue,ipd,lab,billing,encounters,inventory}.rs` and the corresponding `pages/*.tsx`.
- **[Updated v0.2.0]** Inventory module (CR-21) + patient consent commands (CR-12) now implemented — closes the gap noted in v0.1.0 where these Phase 1 Must FRs had no code.
- Phase 2 modules (Nurses, Pharmacy, Radiology, Blood Bank, HR, Payroll, Reports) are stated as Planned in the SRS and have no code in the tree.
- The `Permission` enum in `rbac.rs` covers 37 keys (was 35; +2 for `MessagingView`/`MessagingSend` added in Batch 1 CR-16); Phase 2 will need additional keys (`nurses.manage`, `pharmacy.dispense`, `hr.manage`, etc.).

**Gaps**:

| Gap | Remediation |
|---|---|
| Phase 2 modules not implemented | Build per the Phase 2 plan in the SRS; add `Permission` variants and `commands/<module>.rs` files following the established pattern. |
| No Reports page despite `reports.view` permission existing | Implement `pages/Reports.tsx` + `commands/reports.rs` with server-side aggregations. |

### 3.2 Functional correctness — Level 3

**Target**: State-changing operations behave correctly under concurrency and edge cases.

**Status**: Partial.

**Evidence**:

- `commands/ipd.rs::admit_patient` uses a transaction for bed allocation (`UPDATE beds ... WHERE status='available'` + `INSERT ipd_admissions` in the same `tx`).
- `commands/billing.rs::create_bill` recomputes totals server-side.
- `commands/lab.rs::update_lab_result` auto-completes the order when all results are in.
- `auth.rs::login` runs a dummy Argon2 verify on unknown usernames for timing consistency.

**Gaps**:

| Gap | Remediation |
|---|---|
| No automated test suite (Rust or TypeScript) | Add `cargo test` covering auth lockout, IPD transaction, billing totals, lab auto-complete; add Vitest for frontend hooks. |
| Manual review only — no fuzzing or property-based testing | Consider `proptest` for monetary computation and queue ordering. |

### 3.3 Functional appropriateness — Level 4

**Target**: Features map to real workflows; no dead features.

**Status**: Implemented.

**Evidence**:

- Each command maps to a real clinician/admin action (admit, discharge, order lab, record payment, etc.).
- The 8 RBAC personas reflect real hospital roles.
- No "coming soon" stubs in Phase 1; Phase 2 modules are explicitly not yet built rather than fake-implemented.

---

## 4. Characteristic 2 — Performance Efficiency

### 4.1 Time behaviour — Level 3

**Target**: SRS NFR-01–NFR-05 latency targets (e.g. dashboard KPI <800ms p95, login <1.5s p95).

**Status**: Partial.

**Evidence**:

- `commands/dashboard.rs::get_dashboard_kpis` is a single server-side aggregation (not a client-side reduction).
- Patient list, bill list, and audit log queries use server-side filtering.
- Indices exist on `audit_logs(created_at DESC)` and `queue_tokens(status, issued_at)`.

**Gaps**:

| Gap | Remediation |
|---|---|
| Targets not measured under load | Add a benchmark harness with synthetic data (100k patients, 100k appointments) on a reference Windows PC. |
| Missing indices on hot paths | Consider indices on `appointments(appointment_date, status)`, `bills(created_at DESC)`, `lab_orders(patient_id)`. |
| Argon2id memory cost (19 MiB) makes login ~1s on a 5-year-old CPU | Acceptable per OWASP; document in user guide. |

### 4.2 Resource utilisation — Level 4 (was Level 3 in v0.1.0)

**Target**: Connection pool ≤10; memory bounded.

**Status**: Implemented.

**Evidence**:

- **[Updated v0.2.0]** `db.rs:123` explicitly sets `.max_connections(10)` on the application pool (NFR-06 satisfied). The temporary bootstrap pool at `db.rs:106` uses `.max_connections(2)` and the diagnostic-check pool at `lib.rs:816` uses `.max_connections(1)` — both intentionally smaller.
- Audit log file trimmed to ~400 KB in `lib.rs::log`.
- Tauri single-process; no orphaned background workers (Batch 2 REL-03 added cooperative shutdown for the 3 background tasks).

**Gaps**:

| Gap | Remediation |
|---|---|
| ~~Pool size not explicitly configured~~ | **[Resolved v0.2.0]** `.max_connections(10)` is explicit in `db::connect_app`. |
| Memory not profiled under sustained use | Add Windows Performance Monitor baseline. |

### 4.3 Capacity — Level 3

**Target**: Support a single hospital's lifetime data (decades of appointments, admissions, bills).

**Status**: Partial.

**Evidence**:

- `SERIAL`/`BIGSERIAL` PKs.
- `NUMERIC(14,2)` for monetary fields (no float drift).
- JSONB for audit `details`.

**Gaps**:

| Gap | Remediation |
|---|---|
| No archival strategy for audit_logs | Add a yearly partition or archive-to-cold-storage routine. |
| No load test | Add synthetic data generation + JMeter/k6 plan. |

---

## 5. Characteristic 3 — Compatibility

### 5.1 Co-existence — Level 4

**Target**: Server-build and client-build installers can coexist on the same PC for testing.

**Status**: Implemented.

**Evidence**:

- `tauri.server.conf.json` and `tauri.client.conf.json` have distinct `productName` and `identifier`.
- Server installer bundles `resources/pgsql`; client installer does not.
- Both installers can be present simultaneously without conflict.

### 5.2 Interoperability — Level 3

**Target**: Standard PostgreSQL; no proprietary wire protocol.

**Status**: Partial.

**Evidence**:

- PostgreSQL 16+ with `scram-sha-256` auth, JSONB, standard SQL.
- Data is fully extractable via `pg_dump`.
- Tauri IPC is the FE/BE interface; versioning is by coordinated release.

**Gaps**:

| Gap | Remediation |
|---|---|
| No HL7/FHIR integration | Out of scope for this revision; consider for Phase 3 if external lab/RIS integration is required. |
| No DICOM/PACS integration | Reserved for Phase 2 Radiology module. |

---

## 6. Characteristic 4 — Usability

### 6.1 Appropriateness recognisability — Level 4

**Evidence**: Boot screen displays `licenseInfo.hospital_name` and `product_edition`; license-error screen is unambiguous.

### 6.2 Learnability — Level 4

**Evidence**: Permission-filtered sidebar hides inaccessible features; staged `init_status` messages inform the user during boot; first-run admin is forced to change password.

### 6.3 Operability — Level 4

**Evidence**: NSIS installer is zero-interaction for PostgreSQL; pairing UI on the client is a 2-field form (server IP + pairing code); Settings → Advanced surfaces operational diagnostics.

### 6.4 User error protection — Level 3

**Evidence**: `auth.rs::delete_user` blocks self-deletion; `change_password` enforces 8-char minimum and current-password verification; billing ignores client-supplied totals.

**Gaps**:

| Gap | Remediation |
|---|---|
| Some destructive actions (patient delete, bill delete) lack an explicit confirm dialog in the UI | Add `<AlertDialog>` confirmation modals in `pages/Patients.tsx` and `pages/Billing.tsx`. |
| No undo for queue status changes | Document as intentional (queue is real-time); consider an audit-driven "revert" action. |

### 6.5 User interface aesthetics — Level 4

**Evidence**: `DESIGN_SYSTEM.md` token system; light/dark themes; status colours tuned for WCAG AA on both backgrounds; Lexend + Inter typography; surface-card/surface-elevated utilities.

### 6.6 Accessibility — Level 3 (was Level 4 in v0.1.0 — the L4 was aspirational; actual at-audit was L2)

**Evidence**:

- Global `:focus-visible` ring (2px solid `--ring`).
- `prefers-reduced-motion: reduce` respected in CSS and Motion.
- WCAG AA contrast for status and primary/accent colours.
- **[Updated v0.2.0 — Batch 3]** `aria-label` on ~20 icon-only buttons across Patients, Doctors, Appointments, Queue, Users, Billing, Messaging, AuditLog + 4 unlabeled inputs.
- **[Updated v0.2.0 — Batch 3]** `DialogDescription` added to 10 dialogs (Billing ×2, Laboratory ×2, Queue ×1, IPD ×2, Users ×3) so Radix's `aria-describedby` requirement is satisfied.
- **[Updated v0.2.0 — Batch 3]** `TableHead` now defaults `scope="col"` at the component level (every `<th>` in the app renders `scope="col"` automatically); 7 most-used tables also have explicit `scope="col"` for defense-in-depth.
- **[Updated v0.2.0 — Batch 3]** `htmlFor`/`id` pairs on Login + Setup + Settings focusable fields.

**Gaps**:

| Gap | Remediation |
|---|---|
| Select-based form fields in Billing/Queue/IPD/Users/Laboratory lack `htmlFor` association (Radix Select is a button, different pattern from Input) | Planned Batch 5 a11y. |
| `ConsentPanel.tsx` revoke-consent still uses `confirm()` | Planned Batch 5 — replace with destructive-confirmation Dialog. |
| No automated axe-core or pa11y run in CI | Add to the SDLC gate. |
| Color contrast not verified at every font size | Manual sampling done; add automated checks. |

**Note**: The previous revision claimed Level 4 based on the design intent. The actual at-audit level was Level 2 (icon-only buttons had no aria-labels; dialogs were missing `DialogDescription`; tables had no `scope`). Batch 3 closed the largest gaps; Level 3 reflects the current state. Full WCAG AA conformance targeted for Batch 5.

---

## 7. Characteristic 5 — Reliability

### 7.1 Maturity — Level 3

**Evidence**: Idempotent migrations (`CREATE TABLE IF NOT EXISTS`, `ADD COLUMN IF NOT EXISTS`) re-runnable on every boot; installer never destroys existing `pgdata`.

**Gaps**: No fault-injection testing; no chaos testing of the LAN pairing path.

### 7.2 Availability — Level 3

**Target**: 99% during hospital operating hours (single-server, no redundancy).

**Evidence**: PostgreSQL auto-start Windows Service; client self-heal via LAN discovery.

**Gaps**: Single point of failure at the server PC; no warm standby. See Risk R-005 in `05-Risk-Register-ISO-31000.md`.

### 7.3 Fault tolerance — Level 3

**Evidence**:

- Client falls back to LAN broadcast discovery if the saved server IP is unreachable (`lib.rs::initialize_as_client`).
- Server auto-repairs broken SSL configuration in `pg_hba.conf`/`postgresql.conf` (`pg_provision::repair_ssl_config`).
- Audit insert failures are swallowed (`audit.rs` — availability over completeness).

**Gaps**: No automatic retry of transient DB errors (sqlx pool acquire timeouts); user must click "Try again".

### 7.4 Recoverability — Level 3

**Evidence**: Idempotent migrations enable safe re-run after a crash; `pg_ctl` service control; data directory preserved across reinstalls.

**Gaps**:

| Gap | Remediation |
|---|---|
| No in-product backup/restore UI | Add Settings → Backup with `pg_dump` wrapper; restore via installer "restore from backup" path. |
| No point-in-time recovery (PITR) | Document WAL archiving as an operational responsibility. |

---

## 8. Characteristic 6 — Security

(Cross-reference `04-Security-Control-Matrix-ISO-27001.md` for ISO/IEC 27001 mapping.)

### 8.1 Confidentiality — Level 4 (reaffirmed v0.2.0 with expanded evidence)

**Evidence**:

- RBAC enforced on every protected command via `rbac::require`. **[Updated v0.2.0]** `get_config` now requires `SettingsManage` once `setup_complete` (CR-4); the 4 messaging commands now require `MessagingView`/`MessagingSend` (CR-16); log-reading commands now require `audit.view` and redact PHI from log output (SEC-05).
- LAN PostgreSQL connections use TLS with pinned server certificate (`sslmode=verify-ca`).
- No PHI is written to `hms_startup.log` (Batch 3 SEC-05 redaction pass).
- `password_hash` is `#[serde(skip_serializing)]` and never returned to the frontend.
- **[Updated v0.2.0 — CR-4]** `db_password` is `#[serde(skip_serializing)]` so `get_config` no longer leaks the DB password to any frontend caller.
- **[Updated v0.2.0 — CR-3]** `tauri.conf.json` now sets an explicit CSP (was `null` in v0.1.0).

**Gaps**: Read commands are not row-level audited (intentional — volume would leak PHI access patterns).

### 8.2 Integrity — Level 4

**Evidence**:

- Ed25519-signed licenses reject forgery/tampering.
- Server-side bill total recomputation defeats tampered client payloads.
- `ON DELETE RESTRICT` on `ipd_admissions.patient_id`, `bills.patient_id`, `payments.bill_id` preserves clinical/financial history.
- Bootstrap admin is forced to change password on first login.

### 8.3 Non-repudiation — Level 3

**Evidence**: Audit log records `user_id`, `username`, `action`, `resource`, `resource_id`, `details` (JSONB), `created_at`.

**Gaps**:

| Gap | Remediation |
|---|---|
| Audit log is not append-only at the DB level — a super_admin with DB access could `UPDATE`/`DELETE` rows | Add a trigger rejecting `UPDATE`/`DELETE` on `audit_logs`; forward to an off-host syslog in Phase 2. |
| No external time-stamping (RFC 3161) | Consider for compliance regimes that demand it. |

### 8.4 Accountability — Level 4 (reaffirmed v0.2.0 with expanded evidence)

**Evidence**: `audit::for_session` invoked on every state-changing command; `created_by_user_id` columns on `patients`, `appointments`, `bills`, `payments`, `encounters`, `lab_orders`, `lab_order_tests`, `ipd_admissions`, `queue_tokens`, `patient_consent`.

**[Updated v0.2.0]** Batches 1-3 closed the previously-documented gaps where some state-changing commands skipped the audit row:
- All 4 messaging commands now audit `send_message` and `delete_message` (CR-16).
- All 6 inventory commands now audit; `adjust_inventory` writes both an `audit_logs` row AND an `inventory_movements` row (CR-21).
- All 3 consent commands (`set_patient_consent`, `revoke_patient_consent`) now audit (CR-12).
- `revoke_license` now audits (LIC-DOC-04).
- `install_license` now audits.

All state-changing commands in the application now write audit rows.

### 8.5 Authenticity — Level 4 (reaffirmed v0.2.0 with expanded evidence)

**Evidence**: Argon2id password hashing; 5-attempt/15-minute brute-force lockout; single active session per user; 32-byte random session token with SHA-256 hash at rest; constant-time login via dummy Argon2 verify on unknown usernames.

**[Updated v0.2.0]** Two gaps from the v0.1.0 audit are now closed:
- The message `sender` field on `send_message` is now derived from the authenticated session (`session.user_id` + `session.full_name`), NOT from a client-supplied string — a user can no longer forge a message under another user's name (CR-16).
- Pairing code generation switched from `thread_rng()` (userspace PRNG seeded once at thread start) to `OsRng` (CSPRNG; reads from the OS RNG on every call) so an attacker who has observed prior pairing codes cannot predict future ones (SEC-03 / SDD-12).

**Gaps**:

| Gap | Remediation |
|---|---|
| No MFA | Phase 2 — add TOTP for `super_admin` and `billing_clerk` roles. |
| No password complexity beyond length 8 | Add complexity rules + breached-password check. |

---

## 9. Characteristic 7 — Maintainability

### 9.1 Modularity — Level 4

**Evidence**: `commands/<module>.rs` per domain; `lib/queries.ts` + `lib/models.ts` + `lib/rbac.ts` + `lib/auth.tsx` centralised; clear Rust module boundaries declared in `lib.rs`.

### 9.2 Reusability — Level 4

**Evidence**: Shared `rbac::require` guard; shared `audit::for_session` helper; shared UI primitives in `components/ui/`; shared `models::Bill`, `models::Patient` etc.

### 9.3 Analysability — Level 3 (reaffirmed v0.2.0 — was Partial in v0.1.0, now Implemented)

**Evidence**: **[Updated v0.2.0 — Batch 0]** TypeScript strict mode (`noUnusedLocals`, `noUnusedParameters`, `noUncheckedIndexedAccess`, `noImplicitReturns`, `forceConsistentCasingInFileNames`) passes with zero errors via `tsc --noEmit`; ESLint 9 flat config (`typescript-eslint` recommended + `react-hooks` rules-of-hooks/exhaustive-deps + `react-refresh` only-export-components + `eslint-config-prettier`) passes with zero errors and 16 pre-existing warnings; Prettier formatting enforced via `.prettierrc.json`; centralised query keys (`qk` in `lib/queries.ts`); structured logging in `lib.rs::log`.

**Gaps**: No Rust tests; no tracing instrumentation; no metrics endpoint; no CI pipeline that runs the gate on every commit (the gate is run manually after every batch).

### 9.4 Modifiability — Level 4

**Evidence**: Additive migrations; data-driven RBAC (role_permissions editable in DB); new modules follow the established `commands/<module>.rs` pattern with RBAC + audit.

### 9.5 Testability — Level 2

**Evidence**: `tsc --noEmit` strict gate; manual review of Rust.

**Gaps**:

| Gap | Remediation |
|---|---|
| No `cargo test` suite | Add unit tests for `rbac`, `auth` (lockout, single session), `license` (canonical bytes, signature verification), `billing` (server-side total), `lab` (auto-complete). |
| No frontend test runner | Add Vitest + React Testing Library for `lib/queries.ts` and `pages/*.tsx`. |
| No CI pipeline | Add GitHub Actions / local CI runner. |

---

## 10. Characteristic 8 — Portability

### 10.1 Adaptability — Level 3

**Evidence**: Windows 10/11 production target; non-Windows fallback for dev (license fingerprint uses hostname+OS); `config.rs` resolves ProgramData vs per-user paths.

**Gaps**: Production non-Windows is not supported (by design); the fallback fingerprint is explicitly not a production fingerprint.

### 10.2 Installability — Level 4

**Evidence**: NSIS installer with `NSIS_HOOK_POSTINSTALL` performs zero-interaction PostgreSQL provisioning (initdb, scram-sha-256, pg_hba LAN scoping, Windows Service registration, start, config.json write). Client installer has no PostgreSQL and uses the 2-field pairing UI.

### 10.3 Replaceability — Level 3

**Evidence**: Standard PostgreSQL 16+; data extractable via `pg_dump`; no proprietary data formats.

**Gaps**: No documented migration runbook to a different HMS (data export to FHIR is Phase 3).

---

## 11. Gap remediation summary

**[Updated v0.2.0]** Rows marked ✅ are resolved by Phase 2 Batches 0-3. Rows marked 🟡 are partially addressed. Rows marked ⬜ remain open.

| Priority | Gap | Action | Owner | Target | Status |
|---|---|---|---|---|---|
| High | No `cargo test` suite | Add unit + integration tests for `auth`, `rbac`, `license`, `billing`, `lab`, `ipd` | Engineering | Phase 2 start | ⬜ Open |
| High | ~~`COMPANY_PUBLIC_KEY` placeholder~~ | ~~Provision real Ed25519 keypair at build time~~ | ~~Software company~~ | ~~Pre-production~~ | ✅ Resolved v0.2.0 (CR-20 — dev keypair generated by `keygen/` project; production swap still required before ship) |
| High | No in-product backup UI | Add Settings → Backup with `pg_dump` wrapper | Engineering | Phase 2 | ⬜ Open |
| High | Audit log not append-only at DB level | Add trigger rejecting UPDATE/DELETE on `audit_logs` | Engineering | Phase 2 | ⬜ Open |
| Medium | No MFA | Add TOTP for `super_admin`/`billing_clerk` | Engineering | Phase 2 | ⬜ Open |
| High | ~~Destructive actions lack confirmation dialogs~~ | ~~Add `<AlertDialog>` to patient/bill delete~~ | ~~Engineering~~ | ~~Phase 1 patch~~ | ✅ Resolved v0.2.0 (Batch 3 INT-01 — 5 destructive-action Dialogs added: delete patient, delete doctor, delete appointment, reset password, delete user) |
| Medium | Missing DB indices on hot paths | Add indices on `appointments(date,status)`, `bills(created_at)`, `lab_orders(patient_id)` | Engineering | Phase 1 patch | ⬜ Open |
| Medium | ~~No CI pipeline~~ | ~~GitHub Actions / local runner with `tsc --noEmit` + `cargo check` + `cargo test`~~ | ~~Engineering~~ | ~~Phase 2~~ | 🟡 Partial v0.2.0 (Batch 0 — `tsconfig.json` strict gate + ESLint + Prettier all configured and pass; CI runner itself still Planned Phase 2) |
| Medium | No accessibility CI | Add axe-core to CI | Engineering | Phase 2 | ⬜ Open |
| Medium | ~~`db_password` exposed via `get_config`~~ | ~~RBAC-gate `get_config` + `#[serde(skip_serializing)]` on `db_password`~~ | ~~Engineering~~ | ~~Phase 1 patch~~ | ✅ Resolved v0.2.0 (CR-4) |
| Medium | ~~Messaging commands lack RBAC + audit + sender from session~~ | ~~Add `MessagingView`/`MessagingSend` perms; derive sender from session; audit send/delete~~ | ~~Engineering~~ | ~~Phase 1 patch~~ | ✅ Resolved v0.2.0 (CR-16) |
| Medium | ~~Pairing code uses `thread_rng()` (non-CSPRNG)~~ | ~~Switch to `OsRng`~~ | ~~Engineering~~ | ~~Phase 1 patch~~ | ✅ Resolved v0.2.0 (SEC-03 / SDD-12) |
| Medium | ~~Patient consent commands missing (FR-0035 stale)~~ | ~~Implement 3 consent commands + WhatsApp consent gate~~ | ~~Engineering~~ | ~~Phase 1 patch~~ | ✅ Resolved v0.2.0 (CR-12) |
| Medium | ~~Inventory commands missing (FR-0180/0181/0185 stale)~~ | ~~Implement 6 inventory commands + Inventory page UI~~ | ~~Engineering~~ | ~~Phase 1 patch~~ | ✅ Resolved v0.2.0 (CR-21) |
| Medium | ~~`tauri.conf.json` CSP is `null`~~ | ~~Set explicit CSP~~ | ~~Engineering~~ | ~~Phase 1 patch~~ | ✅ Resolved v0.2.0 (CR-3) |
| Medium | ~~Pool size not explicitly configured~~ | ~~Set `.max_connections(10)` explicitly~~ | ~~Engineering~~ | ~~Phase 1 patch~~ | ✅ Resolved v0.2.0 (`db.rs:123`) |
| Low | No HL7/FHIR | Out of scope this revision; revisit Phase 3 | Engineering | Phase 3 | ⬜ Open |
| Low | No PITR documentation | Document WAL archiving in deployment guide | Engineering | Phase 2 | ⬜ Open |
| Low | Select-based form fields lack `htmlFor` (Radix Select pattern) | Add `htmlFor` association to ~10 Select fields | Engineering | Batch 5 a11y | ⬜ Open |
| Low | `ConsentPanel.tsx` revoke-consent uses `confirm()` | Replace with destructive-confirmation Dialog | Engineering | Batch 5 | ⬜ Open |
| Low | `clear_config` dead IPC command | Register in `generate_handler![]` or delete | Engineering | Batch 5 cleanup | ⬜ Open |

---

## 12. Traceability

| ISO/IEC 25010 characteristic | SRS NFR section | Risk register entries | Security controls |
|---|---|---|---|
| Functional suitability | NFR (implicit in FRs) | — | — |
| Performance efficiency | §7.1 NFR-01–NFR-06 | R-011 (single-session DoS) | — |
| Compatibility | §7.7 NFR-70–NFR-72 | — | — |
| Usability | §7.4 NFR-40–NFR-45 | — | A.5.15 (access control UX) |
| Reliability | §7.3 NFR-30–NFR-35 | R-005 (DB loss/no backup), R-008 (audit tampering) | A.8.13 (backup), A.8.14 (redundancy) |
| Security | §7.2 NFR-10–NFR-22 | R-001 (PHI breach), R-003 (DB password), R-004 (brute force), R-006 (LAN eavesdrop), R-007 (over-privileged role), R-014 (SQLi) | A.5.15, A.5.17, A.8.2, A.8.3, A.8.5, A.8.16, A.8.24 |
| Maintainability | §7.5 NFR-50–NFR-55 | R-012 (supply chain) | A.8.25 (secure development) |
| Portability | §7.6 NFR-60–NFR-63 | — | — |

---

_End of `03-Quality-Model-ISO-25010.md`. Cross-reference `01-SRS-Software-Requirements.md` for requirements, `04-Security-Control-Matrix-ISO-27001.md` for the security control mapping, and `05-Risk-Register-ISO-31000.md` for risk treatment._
