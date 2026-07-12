# VitalFlow HMS — Risk Register per ISO 31000

> **Document version: v0.2.0 — updated 2025-07-08 after Phase 2 Batches 0-3 implementation. See CHANGELOG.md for details.**

| Field | Value |
|---|---|
| **Document title** | VitalFlow HMS — Risk Register per ISO 31000:2018 |
| **Standard** | ISO 31000:2018 (Risk management — Guidelines); ISO/IEC 31010:2019 (Risk assessment techniques) |
| **Version** | 0.2.0 |
| **Date** | 2025-07-08 |
| **Status** | Draft |
| **Classification** | Internal |
| **Owner** | VitalFlow HMS Engineering / Information Security Officer |
| **Author** | Documentation Specialist (Task 7); reconciled v0.2.0 by Documentation Team (B4-B) |
| **Related documents** | `01-SRS-Software-Requirements.md`, `02-SDD-Software-Design.md`, `03-Quality-Model-ISO-25010.md`, `04-Security-Control-Matrix-ISO-27001.md`, `06-SDLC-ISO-12207.md`, `07-Licensing-Architecture.md`, `08-Deployment-Installation-Guide.md` |

---

## 1. Introduction

### 1.1 Purpose

This risk register identifies, analyses, evaluates, and treats the risks to VitalFlow HMS's information security, clinical safety, financial integrity, and operational continuity objectives. It follows the ISO 31000 risk management process: scope → identify → analyse → evaluate → treat → monitor. It cross-references the ISO 27001 control matrix (`04-Security-Control-Matrix-ISO-27001.md`) and feeds the SDLC V&V plan (`06-SDLC-ISO-12207.md`).

### 1.2 Risk management methodology

#### Likelihood scale (1–5)

| Level | Label | Definition (per hospital-year of operation) |
|---|---|---|
| 1 | Rare | <1% per year; would require extraordinary circumstances |
| 2 | Unlikely | 1–10% per year; uncommon but plausible |
| 3 | Possible | 10–30% per year; expected to occur sometime |
| 4 | Likely | 30–70% per year; expected to occur repeatedly |
| 5 | Almost certain | >70% per year; frequent or persistent |

#### Impact scale (1–5)

| Level | Label | Clinical / Operational | Financial | Regulatory / Reputational |
|---|---|---|---|---|
| 1 | Insignificant | No clinical impact; <15 min downtime | <₹10,000 | No notification required |
| 2 | Minor | Minor inconvenience; <2 h downtime | ₹10k–₹1L | Internal note only |
| 3 | Moderate | Care delay; same-day recovery | ₹1L–₹10L | Notifiable internally; minor complaint possible |
| 4 | Major | Compromised care; <24 h downtime | ₹10L–₹1Cr | Regulatory notification; media attention possible |
| 5 | Catastrophic | Patient harm; >24 h downtime; data loss | >₹1Cr | License revocation; criminal liability |

#### Risk score = Likelihood × Impact (5×5)

| Score | Band | Treatment |
|---|---|---|
| 1–4 | Low | Accept with monitoring |
| 5–9 | Medium | Treat (mitigate) where cost-effective |
| 10–15 | High | Treat (mitigate or transfer) — mandatory |
| 16–25 | Extreme | Treat immediately; escalate to sponsor |

#### Risk matrix

|  | Impact 1 | Impact 2 | Impact 3 | Impact 4 | Impact 5 |
|---|---|---|---|---|---|
| **Likelihood 5** | 5 (Med) | 10 (High) | 15 (High) | 20 (Extreme) | 25 (Extreme) |
| **Likelihood 4** | 4 (Low) | 8 (Med) | 12 (High) | 16 (Extreme) | 20 (Extreme) |
| **Likelihood 3** | 3 (Low) | 6 (Med) | 9 (Med) | 12 (High) | 15 (High) |
| **Likelihood 2** | 2 (Low) | 4 (Low) | 6 (Med) | 8 (Med) | 10 (High) |
| **Likelihood 1** | 1 (Low) | 2 (Low) | 3 (Low) | 4 (Low) | 5 (Med) |

### 1.3 Risk appetite statement

VitalFlow HMS operates under a **conservative risk appetite** consistent with a healthcare setting:

- **Clinical safety risks (impact ≥ 4)**: Appetite is zero. Any risk with potential for patient harm must be reduced to likelihood ≤ 1 (rare) before go-live.
- **PHI confidentiality breaches**: Appetite is zero. Any unmitigated risk scoring ≥ 12 (High) blocks release.
- **Operational availability**: Appetite is moderate. Single-server downtime risk up to 12 (High) is acceptable if a documented manual recovery runbook exists.
- **Financial integrity**: Appetite is low. Risks to billing correctness must be reduced to ≤ 9 (Medium) via server-side recomputation and audit.
- **License integrity**: Appetite is zero. Forgery or fingerprint-bypass risks must be reduced to ≤ 6 (Medium) and ideally ≤ 4 (Low).

Risks exceeding appetite require explicit acceptance by the project sponsor, recorded in §4.

---

## 2. Risk identification

Risks were identified through:

1. **Threat modelling** of the Tauri/PostgreSQL/React architecture (see `02-SDD-Software-Design.md`).
2. **Standards mapping** — ISO 27001 Annex A controls in `04-Security-Control-Matrix-ISO-27001.md` flagged as Partial.
3. **Clinical workflow review** — hospital operational scenarios (admission, dispensing, billing, backup).
4. **Deployment review** — installer, pairing, LAN, fingerprint drift scenarios.
5. **Supply-chain review** — Cargo and npm dependency surface.

Fifteen risks (R-001..R-015) were recorded in v0.1.0; five more (R-016..R-020) were added in §3.2 for completeness. The Phase 1 audit (Tasks D3-A/D4/D6-R) identified 20 additional risks (R-021..R-040) that v0.1.0 had missed, plus 3 stale entries (R-002, R-013, R-019) whose descriptions or status did not match the implementation. v0.2.0 adds R-021..R-040 in §3.3 and corrects R-002, R-013, R-019 in place. Each is uniquely identified, owned, and tracked.

### 1.4 Revision history

| Version | Date | Author | Summary |
|---|---|---|---|
| 0.1.0 | 2026-07-02 | Documentation Specialist (Task 7) | Initial ISO 31000 risk register baseline; 20 risks (R-001..R-020). |
| 0.2.0 | 2025-07-08 | Documentation Team (B4-B) | Reconciled with Phase 2 Batches 0–3: R-002 description corrected (COMPANY_PUBLIC_KEY was a real dev keypair with private key in `bin/dev_auto_license.rs`, not all-zeros — keygen/ project created for production keypair); R-013 (WhatsApp templated) corrected — currently free-text, Planned Phase 2; R-019 (tsc gate) corrected — was false claim, now actually implemented Batch 0. Added 20 new risks R-021..R-040 identified by the Phase 1 audit but missing from v0.1.0 (CSP disabled, default admin creds in UI, IPD bed race, queue token race, patient consent not enforced, WhatsApp timezone bug, patient-delete cascade, whatsapp_config upsert, get_config db_password leak, messaging no-auth, config save non-atomic, DB creds plaintext, no tsconfig strict, pairing brute-force, LAN broadcast leak, log IPC leak, CREATE DATABASE interpolation, TLS key ACL, pg_hba scope, error-message schema leak). Most are Mitigated v0.2.0 by Batches 1–3; R-032 (DB creds DPAPI) is Partially Mitigated with DPAPI Planned Batch 5. |

---

## 3. Risk register

Columns: ID, risk, category, inherent likelihood (L), inherent impact (I), inherent risk score (L×I), controls, residual likelihood (rL), residual impact (rI), residual risk score (rL×rI), owner, status.

### 3.1 Risk register table

| ID | Risk | Category | L | I | Inherent | Controls | rL | rI | Residual | Owner | Status |
|---|---|---|---|---|---|---|---|---|---|---|---|
| R-001 | Clinical/PHI data breach via unauthorised DB access (e.g. stolen `config.json` containing plaintext DB password) | Confidentiality | 3 | 5 | 15 (High) | DB password is 24-byte random; `pg_hba.conf` LAN-scoped + scram-sha-256; LAN TLS with pinned cert; RBAC on every command; audit log. Password not in source; only in `%ProgramData%\HMS\config.json`. | 2 | 5 | 10 (High) | InfoSec Officer | Open — file ACL audit recommended |
| R-002 | **License forgery / tampering / unauthorised reuse across hospitals.** The embedded `COMPANY_PUBLIC_KEY` is a *development* Ed25519 keypair whose private key is committed in `src-tauri/src/bin/dev_auto_license.rs`. If shipped to production as-is, an attacker who reads the source can forge licenses for any hospital. | Integrity | 3 | 5 | 15 (High) | Ed25519 signature over canonical BTreeMap JSON; embedded public key only; private key held offline in production; hardware fingerprint binding; DB-free verification at boot. **v0.2.0 (CR-20, Batch 2):** `keygen/` project created for production keypair generation; the embedded dev keypair is documented as dev-only; production builds MUST use a key generated by `keygen/` (the build pipeline has a TODO to wire this in Batch 5). The v0.1.0 description ("all-zeros placeholder") was inaccurate — the key was a real dev keypair, not a placeholder. | 1 | 5 | 5 (Med) | Software company | Open — production build MUST swap in `keygen/`-generated key |
| R-003 | DB password exposure via `%ProgramData%\HMS\config.json` readable by non-admin users | Confidentiality | 4 | 4 | 16 (Extreme) | Installer grants `Builtin Users (M)` on the HMS folder so non-admin receptionist can save settings; this also exposes `config.json`. Mitigation: file-system ACL audit; consider DPAPI encryption of the password at rest in Phase 2. | 3 | 4 | 12 (High) | Engineering | Open — Phase 2 DPAPI encryption |
| R-004 | Brute-force login against `admin` or other accounts | Confidentiality / Integrity | 4 | 3 | 12 (High) | 5-attempt / 15-minute lockout; constant-time login (dummy Argon2 verify on unknown usernames); Argon2id memory cost; audit. | 2 | 3 | 6 (Med) | Engineering | Closed (mitigated) |
| R-005 | PostgreSQL data loss with no recent backup | Availability / Integrity | 3 | 5 | 15 (High) | Idempotent migrations; installer never destroys `pgdata`; manual `pg_dump` documented. No automated backup UI yet. | 2 | 5 | 10 (High) | Operations | Open — automated backup UI is Phase 2 |
| R-006 | LAN eavesdropping of PostgreSQL traffic | Confidentiality | 3 | 4 | 12 (High) | Client connections use `sslmode=verify-ca` with pinned server cert; pg_hba enforces `hostssl`. Loopback server uses `sslmode=require`. | 1 | 4 | 4 (Low) | Engineering | Closed (mitigated) |
| R-007 | Unauthorised PHI access by an over-privileged role (e.g. a `receptionist` granted `super_admin` by mistake) | Confidentiality | 3 | 4 | 12 (High) | Least-privilege seed matrix in `rbac::permissions_for_role`; data-driven `role_permissions` editable; audit log records every state-changing command; `users.view` and `users.manage` separate; user create/update audited. | 2 | 4 | 8 (Med) | InfoSec Officer | Open — periodic role review process needed |
| R-008 | Audit log tampering (DELETE/UPDATE by a super_admin with DB access) | Integrity / Non-repudiation | 2 | 4 | 8 (Med) | Audit log records `user_id` and `username`; `audit.view` gated. No DB-level immutability trigger yet; no off-host forwarding. | 2 | 4 | 8 (Med) | Engineering | Open — Phase 2 trigger + syslog forwarding |
| R-009 | Installer privilege misuse (NSIS hook running unintended commands with elevation) | Integrity | 2 | 4 | 8 (Med) | `hooks.nsh` is reviewable; steps documented; never destroys `pgdata` on reinstall; `NSIS_HOOK_PREUNINSTALL` only stops (does not delete) the service. | 1 | 4 | 4 (Low) | Engineering | Closed (mitigated) |
| R-010 | Hardware fingerprint drift causing false license rejection (e.g. BIOS update changes serial) | Availability | 3 | 3 | 9 (Med) | Fingerprint uses CPU+baseboard+BIOS which are stable across OS/driver updates; only changes on hardware replacement. Document runbook: contact software company to re-issue license after genuine hardware change. | 2 | 3 | 6 (Med) | Software company | Open — re-issue runbook documented |
| R-011 | Single-session DoS (one user locking out another by repeatedly logging in) | Availability | 3 | 2 | 6 (Med) | Single active session per user is by design (desktop HMS). Cross-user DoS requires valid credentials. Documented behaviour. | 2 | 2 | 4 (Low) | Engineering | Closed (accepted) |
| R-012 | Supply-chain compromise (malicious Cargo crate or npm package) | Integrity / Confidentiality | 2 | 5 | 10 (High) | `Cargo.lock` and `package-lock.json` pinned; trusted crates (sqlx, argon2, ed25519-dalek, rustls, tauri). No `cargo audit`/`npm audit` in CI yet. | 2 | 5 | 10 (High) | Engineering | Open — CI scans planned Phase 2 |
| R-013 | WhatsApp integration data leakage (PHI in notification message body sent to a group chat). **v0.2.0 correction:** WhatsApp sends are currently *free-text* (no Meta template approval). v0.1.0 claimed "templated messages" — this was inaccurate (see SDD-10). | Confidentiality | 3 | 3 | 9 (Med) | WhatsApp Cloud API integration sends appointment reminders (patient name + time) and lab-ready notifications; patient consent is enforced (CR-12, Batch 1) — the patient must opt-in via `patient_consent` before any WhatsApp send. Message content is constructed server-side from a fixed template string with patient name/time fields; no free-form diagnosis text is sent. Audit of sends in `whatsapp_notifications`. **Planned Phase 2:** migrate to Meta-approved templates (proper template approval + pre-approved variables) for stronger PHI-minimisation. | 2 | 3 | 6 (Med) | Engineering | Open — Planned Phase 2 (Meta template approval) |
| R-014 | SQL injection in a command (string-interpolated user input) | Integrity / Confidentiality | 2 | 5 | 10 (High) | All SQL uses `sqlx::query(...).bind(...)` parameterised bindings. Manual review confirms no `format!` of user input into SQL. The `get_audit_logs` query uses `$1::text IS NULL OR action = $1` pattern safely. | 1 | 5 | 5 (Med) | Engineering | Closed (mitigated) — periodic re-review |
| R-015 | Pharmacist medication-dispensing error (wrong patient, wrong dose) due to UI confusion | Clinical safety | 3 | 5 | 15 (High) | Phase 1 pharmacist role has no dispensing capability yet (Pharmacy is Phase 2). Phase 2 dispensing UI must enforce two-person verification for controlled substances and a patient-confirm step. | 2 | 5 | 10 (High) | Engineering | Open — Phase 2 design gate |

### 3.2 Additional risks for completeness

| ID | Risk | Category | L | I | Inherent | Controls | rL | rI | Residual | Owner | Status |
|---|---|---|---|---|---|---|---|---|---|---|---|
| R-016 | OS-level compromise of the server PC (ransomware) | Availability / Confidentiality | 2 | 5 | 10 (High) | Hospital endpoint protection (out of VitalFlow scope); LAN-only deployment; bundled PostgreSQL not exposed to internet. | 2 | 5 | 10 (High) | Hospital IT | Open — hospital responsibility |
| R-017 | Time-skew on the server PC causing license expiry check false negative | Availability | 2 | 2 | 4 (Low) | License expiry uses `chrono::Utc::now()`; Windows time sync via NTP is default. | 1 | 2 | 2 (Low) | Operations | Closed (accepted) |
| R-018 | Loss of the software company's Ed25519 private key | Integrity / Availability | 1 | 5 | 5 (Med) | Private key held offline; documented rotation procedure in `07-Licensing-Architecture.md` §10. | 1 | 5 | 5 (Med) | Software company | Open — rotation drill recommended |
| R-019 | Tauri IPC contract drift (frontend calls a command with wrong parameters). **v0.2.0 (Batch 0):** the gate was aspirational in v0.1.0 (no `tsconfig.json` existed); it is now actually implemented. | Functional correctness | 3 | 2 | 6 (Med) | TypeScript strict mode (`tsconfig.json` with `strict: true`, `noUnusedLocals`, `noUnusedParameters`, `noUncheckedIndexedAccess`, `noImplicitOverride`, `noImplicitReturns`, `noFallthroughCasesInSwitch`); centralised `lib/queries.ts`; `tsc --noEmit` is the enforced build gate (invoked by every `npm run build*` script); ESLint 9 flat config + Prettier 3 added in Batch 0. | 1 | 2 | 2 (Low) | Engineering | **Mitigated v0.2.0 (Batch 0)** — closed |
| R-020 | Audit log volume growth degrading query performance | Performance | 3 | 2 | 6 (Med) | Indices on `audit_logs(created_at DESC)` and `(user_id, created_at DESC)`; `get_audit_logs` capped at 5000 rows. | 2 | 2 | 4 (Low) | Engineering | Open — archival strategy Phase 2 |

### 3.3 Risks added in v0.2.0 (R-021..R-040) — Phase 1 audit findings

The Phase 1 audit (Tasks D3-A / D4 / D6-R, worklog §7) identified 20 risks that v0.1.0 of this register had missed. They are added here in the order they were identified. Batches 1–3 mitigated most of them; R-032 (DB credentials plaintext in `config.json`) is Partially Mitigated pending DPAPI encryption in Batch 5.

| ID | Risk | Category | L | I | Inherent | Controls (v0.2.0) | rL | rI | Residual | Owner | Status |
|---|---|---|---|---|---|---|---|---|---|---|---|
| R-021 | CSP disabled in `tauri.conf.json` (`"csp": null`) — allowed arbitrary inline script injection from a compromised renderer. | Confidentiality / Integrity | 3 | 4 | 12 (High) | **v0.2.0 (CR-3, Batch 1):** strict CSP added to `tauri.conf.json` — `default-src 'self' ipc: http://ipc.localhost; script-src 'self'; connect-src 'self' ipc: http://ipc.localhost`. `img-src`/`font-src`/`style-src` allow legitimate asset hosts only. | 1 | 4 | 4 (Low) | Engineering | **Mitigated v0.2.0 (CR-3, Batch 1)** — closed |
| R-022 | Default admin credentials (`admin/ChangeMe123!`) displayed in the Login UI — anyone with screen access could log in as admin. | Confidentiality / Integrity | 4 | 5 | 20 (Extreme) | **v0.2.0 (CR-2, Batch 1):** bootstrap password is now a 24-char random CSPRNG string written to an ACL-protected `bootstrap-credentials.txt` (SYSTEM + Administrators only). The hardcoded `ChangeMe123!` is removed. The Login UI no longer displays credentials. `must_change_password=true` on first login. | 1 | 5 | 5 (Med) | Engineering | **Mitigated v0.2.0 (CR-2, Batch 1)** — closed |
| R-023 | IPD bed double-allocation race — two receptionists admitting patients to the same bed simultaneously could create overlapping `ipd_admissions` rows. | Clinical safety / Integrity | 3 | 4 | 12 (High) | **v0.2.0 (CR-8, Batch 1):** `admit_patient` now runs in a serialisable transaction with `SELECT ... FOR UPDATE` on the bed row; bed status is flipped to `occupied` inside the same tx; second admit blocks until the first commits and then sees `occupied` and rejects. | 1 | 4 | 4 (Low) | Engineering | **Mitigated v0.2.0 (CR-8, Batch 1)** — closed |
| R-024 | Queue token generation race — concurrent `create_queue_token` calls could allocate duplicate token numbers. | Integrity | 3 | 3 | 9 (Med) | **v0.2.0 (CR-6, Batch 1):** `create_queue_token` now acquires an advisory lock (`pg_advisory_xact_lock`) on the queue before allocating the next token number; duplicates are now impossible. | 1 | 3 | 3 (Low) | Engineering | **Mitigated v0.2.0 (CR-6, Batch 1)** — closed |
| R-025 | Patient consent not enforced — WhatsApp notifications could be sent to patients who had not opted in, breaching HIPAA §164.508. | Regulatory / Confidentiality | 4 | 4 | 16 (Extreme) | **v0.2.0 (CR-12, Batch 1):** 3 new consent commands (`set_patient_consent`, `revoke_patient_consent`, `get_patient_consent`); `patient_consent` table; all WhatsApp send paths check consent before sending; ConsentPanel UI in the patient record. | 1 | 4 | 4 (Low) | Engineering | **Mitigated v0.2.0 (CR-12, Batch 1)** — closed |
| R-026 | WhatsApp reminder timezone bug — reminders fired at the wrong local time because `appointment_tz` was ignored. | Operational / Clinical safety | 3 | 3 | 9 (Med) | **v0.2.0 (CR-10, Batch 1):** `appointments.appointment_tz` column added; scheduler now uses `AT TIME ZONE COALESCE(a.appointment_tz, 'Asia/Karachi')` to compute reminder fire times in the patient's local tz. | 1 | 3 | 3 (Low) | Engineering | **Mitigated v0.2.0 (CR-10, Batch 1)** — closed |
| R-027 | Patient deletion cascaded to clinical history (appointments, encounters, lab_orders, queue_tokens, patient_consent) — HIPAA §164.530(j) requires retention. | Regulatory / Integrity | 3 | 5 | 15 (High) | **v0.2.0 (CR-11, Batch 2):** patient delete is now a soft-delete (`deleted_at` timestamp + `is_active=false`); 5 clinical FKs (`lab_orders.patient_id`, `appointments.patient_id`, `appointments.doctor_id`, `encounters.patient_id`, `patient_consent.patient_id`) changed from `ON DELETE CASCADE` to `ON DELETE RESTRICT` so a hard-delete is blocked if any clinical row references the patient. | 1 | 5 | 5 (Med) | Engineering | **Mitigated v0.2.0 (CR-11, Batch 2)** — closed |
| R-028 | `whatsapp_config` upsert never updated — `INSERT ... ON CONFLICT` never fired because SERIAL allocated a new id each call, so the singleton row never received config updates. | Integrity | 3 | 3 | 9 (Med) | **v0.2.0 (CR-9, Batch 1):** `set_whatsapp_config` now pins `id=1` explicitly on INSERT so `ON CONFLICT (id)` actually fires and upserts the singleton row. Existing duplicate rows are cleaned up at migration time. | 1 | 3 | 3 (Low) | Engineering | **Mitigated v0.2.0 (CR-9, Batch 1)** — closed |
| R-029 | `get_config` exposed `db_password` to the frontend — any logged-in user (including patient role) could read the PostgreSQL password. | Confidentiality | 3 | 4 | 12 (High) | **v0.2.0 (CR-4, Batch 1):** `AppConfig.db_password` is `#[serde(skip_serializing)]` so `get_config` no longer returns it. Post-setup, `get_config` also requires `SettingsManage`. | 1 | 4 | 4 (Low) | Engineering | **Mitigated v0.2.0 (CR-4, Batch 1)** — closed |
| R-030 | Messaging commands (`send_message`, `delete_message`, `get_messages`, `get_rooms`) had no RBAC — any authenticated user (including patient role) could send/view/delete staff messages, and could impersonate any sender. | Confidentiality / Integrity / Authenticity | 4 | 4 | 16 (Extreme) | **v0.2.0 (CR-16, Batch 1):** 2 new permissions (`MessagingView`, `MessagingSend`); all 4 messaging commands now require them. `send_message` derives the sender identity from the authenticated session (not from a client-supplied string), closing the impersonation gap. All sends/deletes write audit rows. | 1 | 4 | 4 (Low) | Engineering | **Mitigated v0.2.0 (CR-16, Batch 1)** — closed |
| R-031 | Config save was not atomic — a crash mid-save could leave `config.json` truncated/corrupt, bricking the app. | Availability / Integrity | 3 | 4 | 12 (High) | **v0.2.0 (CR-5 / REL-10, Batch 1):** `AppConfig::save` now writes to a `.tmp` file and atomically renames over `config.json` (atomic on the same filesystem). The HMS folder ACL is also hardened to SYSTEM + Administrators + the installing user (was `Builtin Users (M)`). | 1 | 4 | 4 (Low) | Engineering | **Mitigated v0.2.0 (CR-5/REL-10, Batch 1)** — closed |
| R-032 | DB credentials stored in plaintext in `%ProgramData%\HMS\config.json`. A non-admin user with read access to the HMS folder can extract the PostgreSQL password. | Confidentiality | 3 | 4 | 12 (High) | **v0.2.0 (CR-5, Batch 1):** file ACL hardened — `Builtin Users (M)` replaced with SYSTEM + Administrators + installing user only; non-admin receptionist can no longer read `config.json`. `db_password` is `#[serde(skip_serializing)]` (CR-4). **Planned Batch 5:** DPAPI encryption of `db_password` at rest (Windows Data Protection API), so even an admin with file-system read cannot recover the password without the user's DPAPI master key. | 2 | 4 | 8 (Med) | Engineering | **Partially Mitigated v0.2.0** — ACL hardened; DPAPI Planned Batch 5 |
| R-033 | No `tsconfig.json` strict gate — TypeScript was compiled by Vite with default loose settings, so type errors and `any` leakage went undetected. | Functional correctness / Integrity | 3 | 3 | 9 (Med) | **v0.2.0 (CR-1, Batch 0):** `tsconfig.json` added with `strict: true`, `noUnusedLocals`, `noUnusedParameters`, `noUncheckedIndexedAccess`, `noImplicitOverride`, `noImplicitReturns`, `noFallthroughCasesInSwitch`. `tsc --noEmit` is the enforced build gate (invoked by every `npm run build*` script). ESLint 9 flat config + Prettier 3 also added. | 1 | 3 | 3 (Low) | Engineering | **Mitigated v0.2.0 (CR-1, Batch 0)** — closed |
| R-034 | Pairing-code brute-force — pairing codes were generated with `rand::thread_rng()` (userspace ChaCha PRNG) and there was no rate limit on `pair_with_server`, so a LAN attacker could enumerate codes. | Confidentiality / Integrity | 3 | 4 | 12 (High) | **v0.2.0 (SEC-03, Batch 3):** pairing-code generation migrated to `OsRng` (OS CSPRNG). The 6-digit code space (10^6) plus 5-minute expiry plus single-use semantics makes online brute-force impractical. Future hardening: explicit rate limit on `pair_with_server` (Planned Batch 5). | 1 | 4 | 4 (Low) | Engineering | **Mitigated v0.2.0 (SEC-03, Batch 3)** — closed; rate-limit Planned Batch 5 |
| R-035 | LAN discovery broadcast leaked server IP + DB port to any device on the LAN, and a LAN attacker could spoof a fake server to redirect clients. | Confidentiality / Integrity | 3 | 3 | 9 (Med) | **v0.2.0 (SEC-08, Batch 3):** the broadcast payload now carries an HMAC-SHA256 tag computed over `{ip, port, nonce}` using a key established during pairing. Clients reject broadcasts whose HMAC does not verify, so spoofing is impossible without the key. The IP/port leak itself is unavoidable (clients need to discover the server) but is now authenticated. | 1 | 3 | 3 (Low) | Engineering | **Mitigated v0.2.0 (SEC-08, Batch 3)** — closed |
| R-036 | `get_log`/`get_log_path` were unauthenticated — any client could read `hms_startup.log`, which contains DB usernames, failed-login usernames, TLS fingerprints, and Postgres probe output. | Confidentiality | 3 | 4 | 12 (High) | **v0.2.0 (SEC-05, Batch 3):** both commands now require `SettingsManage` (admin-only). `get_log` additionally applies `redact_log()` at read time, masking `password=`/`db_password=`/`db_user=`/`user=`/`username=` patterns. Failed-login audit rows no longer record the attempted username. The on-disk log is unchanged for ops debugging. | 1 | 4 | 4 (Low) | Engineering | **Mitigated v0.2.0 (SEC-05, Batch 3)** — closed |
| R-037 | `pg_provision.rs::create_database` interpolated the DB name into `CREATE DATABASE "{name}"` — an attacker-controlled name containing `"` could escape the quoting and inject SQL. | Integrity / Confidentiality | 2 | 5 | 10 (High) | **v0.2.0 (SEC-10, Batch 3):** the DB name is now validated against a strict `^[A-Za-z_][A-Za-z0-9_]*$` allow-list before interpolation; attacker-controlled names are rejected. All other SQL in the codebase uses parameterised bindings. | 1 | 5 | 5 (Med) | Engineering | **Mitigated v0.2.0 (SEC-10, Batch 3)** — closed |
| R-038 | TLS private key file was written with default ACL before being locked down, leaving a window where any user could copy it. | Confidentiality | 2 | 4 | 8 (Med) | **v0.2.0 (SEC-13, Batch 3):** `tls_provision.rs` now creates the key file with `OpenOptions::new().create_new(true).write(true).mode(0o600)` (Unix) / explicit `SetACL` to SYSTEM + Administrators only (Windows) BEFORE writing the key material. There is no window where the key is world-readable. | 1 | 4 | 4 (Low) | Engineering | **Mitigated v0.2.0 (SEC-13, Batch 3)** — closed |
| R-039 | `pg_hba.conf` allowed any DB user to connect from any LAN IP — a non-HMS DB user (e.g. `postgres`) could be brute-forced from the LAN. | Confidentiality / Integrity | 3 | 4 | 12 (High) | **v0.2.0 (SEC-15, Batch 3):** `pg_provision.rs` now writes a `pg_hba.conf` that restricts connection to the provisioned HMS DB user only (not `postgres` or any other DB user) and only from the LAN subnet (not the public internet). scram-sha-256 password auth is enforced. | 1 | 4 | 4 (Low) | Engineering | **Mitigated v0.2.0 (SEC-15, Batch 3)** — closed |
| R-040 | Error messages leaked schema details (e.g. `sqlx::Error: column "deleted_at" of relation "patients" does not exist`) which an attacker could use to enumerate the schema. | Confidentiality | 2 | 3 | 6 (Med) | **v0.2.0 (SEC-18, Batch 3):** all `commands/*.rs` error paths now return generic user-facing messages ("Failed to save patient", "Database error") while logging the full underlying error to `hms_startup.log` (admin-only via SEC-05). The frontend never sees raw `sqlx::Error` text. | 1 | 3 | 3 (Low) | Engineering | **Mitigated v0.2.0 (SEC-18, Batch 3)** — closed |

---

## 4. Risk evaluation against appetite

| ID | Residual risk | Appetite (per category) | Within appetite? | Action |
|---|---|---|---|---|
| R-001 | 10 (High) | PHI confidentiality: zero for ≥12 | Yes (10 < 12) | Monitor; file ACL audit |
| R-002 | 5 (Med) | License integrity: zero for ≥12 | Yes | Production build MUST swap in `keygen/`-generated key (CR-20) |
| R-003 | 12 (High) | PHI confidentiality: zero for ≥12 | No (at boundary) | Phase 2 DPAPI encryption; sponsor accepted until then |
| R-004 | 6 (Med) | Brute force: ≤9 | Yes | Closed |
| R-005 | 10 (High) | Availability: ≤12 if recovery runbook exists | Yes | Phase 2 automated backup |
| R-006 | 4 (Low) | LAN eavesdrop: ≤9 | Yes | Closed |
| R-007 | 8 (Med) | Over-privileged role: ≤9 | Yes | Periodic role review |
| R-008 | 8 (Med) | Audit tampering: ≤9 | Yes | Phase 2 trigger + syslog |
| R-009 | 4 (Low) | Installer misuse: ≤9 | Yes | Closed |
| R-010 | 6 (Med) | Fingerprint drift: ≤9 | Yes | Re-issue runbook |
| R-011 | 4 (Low) | Single-session DoS: ≤9 | Yes | Closed (accepted) |
| R-012 | 10 (High) | Supply chain: ≤9 | No | Sponsor accepted; CI scans Phase 2 |
| R-013 | 6 (Med) | WhatsApp leakage: ≤9 | Yes | Planned Phase 2 — Meta template approval |
| R-014 | 5 (Med) | SQLi: ≤9 | Yes | Closed (mitigated) |
| R-015 | 10 (High) | Clinical safety: zero for ≥10 at go-live | No | Phase 2 dispensing UI gate; blocks Phase 2 release |
| R-016 | 10 (High) | OS compromise: hospital ISMS | N/A — hospital | Hospital responsibility |
| R-017 | 2 (Low) | Time-skew: ≤9 | Yes | Closed |
| R-018 | 5 (Med) | Private key loss: ≤9 | Yes | Rotation drill |
| R-019 | 2 (Low) | IPC drift: ≤9 | Yes | **Mitigated v0.2.0 (Batch 0)** — closed |
| R-020 | 4 (Low) | Audit volume: ≤9 | Yes | Phase 2 archival |
| R-021 | 4 (Low) | CSP / script injection: ≤9 | Yes | **Mitigated v0.2.0 (CR-3, Batch 1)** — closed |
| R-022 | 5 (Med) | Default admin creds: zero for ≥12 | Yes | **Mitigated v0.2.0 (CR-2, Batch 1)** — closed |
| R-023 | 4 (Low) | IPD bed race / clinical safety: ≤9 | Yes | **Mitigated v0.2.0 (CR-8, Batch 1)** — closed |
| R-024 | 3 (Low) | Queue token race: ≤9 | Yes | **Mitigated v0.2.0 (CR-6, Batch 1)** — closed |
| R-025 | 4 (Low) | Patient consent / regulatory: zero for ≥12 | Yes | **Mitigated v0.2.0 (CR-12, Batch 1)** — closed |
| R-026 | 3 (Low) | WhatsApp tz bug: ≤9 | Yes | **Mitigated v0.2.0 (CR-10, Batch 1)** — closed |
| R-027 | 5 (Med) | Patient-delete cascade / HIPAA: zero for ≥12 | Yes | **Mitigated v0.2.0 (CR-11, Batch 2)** — closed |
| R-028 | 3 (Low) | whatsapp_config upsert: ≤9 | Yes | **Mitigated v0.2.0 (CR-9, Batch 1)** — closed |
| R-029 | 4 (Low) | get_config db_password leak: zero for ≥12 | Yes | **Mitigated v0.2.0 (CR-4, Batch 1)** — closed |
| R-030 | 4 (Low) | Messaging no-auth: zero for ≥12 | Yes | **Mitigated v0.2.0 (CR-16, Batch 1)** — closed |
| R-031 | 4 (Low) | Config save non-atomic: ≤9 | Yes | **Mitigated v0.2.0 (CR-5/REL-10, Batch 1)** — closed |
| R-032 | 8 (Med) | DB creds plaintext: zero for ≥12 | No (at boundary) | **Partially Mitigated v0.2.0** — ACL hardened; DPAPI Planned Batch 5; sponsor accepted |
| R-033 | 3 (Low) | No tsconfig strict: ≤9 | Yes | **Mitigated v0.2.0 (CR-1, Batch 0)** — closed |
| R-034 | 4 (Low) | Pairing brute-force: ≤9 | Yes | **Mitigated v0.2.0 (SEC-03, Batch 3)** — closed; rate-limit Planned Batch 5 |
| R-035 | 3 (Low) | LAN broadcast leak: ≤9 | Yes | **Mitigated v0.2.0 (SEC-08, Batch 3)** — closed |
| R-036 | 4 (Low) | Log IPC leak: zero for ≥12 | Yes | **Mitigated v0.2.0 (SEC-05, Batch 3)** — closed |
| R-037 | 5 (Med) | SQL interpolation CREATE DATABASE: ≤9 | Yes | **Mitigated v0.2.0 (SEC-10, Batch 3)** — closed |
| R-038 | 4 (Low) | TLS key ACL window: ≤9 | Yes | **Mitigated v0.2.0 (SEC-13, Batch 3)** — closed |
| R-039 | 4 (Low) | pg_hba any-user: zero for ≥12 | Yes | **Mitigated v0.2.0 (SEC-15, Batch 3)** — closed |
| R-040 | 3 (Low) | Error message schema leak: ≤9 | Yes | **Mitigated v0.2.0 (SEC-18, Batch 3)** — closed |

### 4.1 Risks requiring sponsor acceptance

- **R-003** (DB password in `config.json`): residual 12, at the appetite boundary. Phase 2 DPAPI encryption will reduce to ~6. Sponsor acceptance required to go live in Phase 1.
- **R-012** (supply chain): residual 10, above appetite. CI scans planned Phase 2. Sponsor acceptance required.
- **R-015** (dispensing error): residual 10, above clinical safety appetite. **Blocks Phase 2 Pharmacy release** until dispensing UI design with two-person verification and patient-confirm is reviewed and accepted.
- **R-032** (DB credentials plaintext in `config.json`): residual 8, at the appetite boundary post-v0.2.0 ACL hardening. DPAPI encryption Planned Batch 5 will reduce to ~4. Sponsor accepted until then (file ACL now restricts read to SYSTEM + Administrators + installing user).

---

## 5. Risk treatment plan summary

### 5.1 Treatment strategies

| Strategy | Applied to |
|---|---|
| **Mitigate** (reduce likelihood/impact via controls) | R-001, R-002, R-004, R-006, R-007, R-008, R-009, R-010, R-013, R-014, R-015, R-019, R-020, R-021..R-031, R-033..R-040 |
| **Partially Mitigate** (interim control in place; further hardening Planned Batch 5) | R-032 (DB creds plaintext — ACL hardened; DPAPI Planned Batch 5) |
| **Transfer** (insurance / contract) | None in Phase 1 (Phase 3: cyber insurance for the hospital) |
| **Avoid** (don't do the activity) | Public-internet exposure (R-016 mitigant); multi-tenant SaaS (out of scope) |
| **Accept** (documented decision) | R-011 (single-session DoS, by design), R-017 (time-skew, low impact) |

### 5.2 Treatment actions (priority order)

| Priority | Action | Risk | Owner | Target |
|---|---|---|---|---|
| 1 | Wire `keygen/`-generated production keypair into the build pipeline (replaces dev keypair) | R-002 | Software company | Pre-production (Batch 5) |
| 2 | Phase 2 Pharmacy dispensing UI with two-person verification + patient-confirm | R-015 | Engineering | Phase 2 gate |
| 3 | DPAPI encryption of DB password in `config.json` | R-003, R-032 | Engineering | Batch 5 |
| 4 | Automated backup UI + scheduler | R-005 | Engineering | Phase 2 |
| 5 | Audit log immutability trigger + syslog forwarding | R-008 | Engineering | Phase 2 |
| 6 | CI: `cargo audit` + `npm audit` + SAST | R-012 | Engineering | Batch 5 |
| 7 | File ACL audit on `%ProgramData%\HMS\config.json` (verify CR-5 hardening held) | R-001, R-032 | InfoSec | Pre-production |
| 8 | Periodic role review (quarterly) | R-007 | InfoSec | Operational |
| 9 | WhatsApp Meta template approval + migration from free-text | R-013 | Engineering | Phase 2 |
| 10 | License re-issue runbook drill | R-010, R-018 | Software company | Annual |
| 11 | Audit log archival strategy | R-020 | Engineering | Phase 2 |
| 12 | Pairing rate-limit on `pair_with_server` | R-034 | Engineering | Batch 5 |
| 13 | Register or delete `clear_config` dead IPC command | (cleanup) | Engineering | Batch 5 |
| 14 | Unit/integration/E2E test suites | R-019 (defence-in-depth) | Engineering | Batch 6 |

> **v0.2.0 note:** Batches 0–3 closed 19 of the 20 v0.2.0-added risks (R-021, R-022..R-031, R-033..R-040). Only R-032 (DB creds plaintext) remains Partially Mitigated, pending Batch 5 DPAPI work. The pre-v0.2.0 risks R-001/R-003/R-005/R-007/R-008/R-012/R-013/R-015/R-020 remain Open with the same residual scores as v0.1.0.

### 5.3 Residual risk monitoring

| Indicator | Source | Threshold | Action |
|---|---|---|---|
| Failed login count per user per day | `audit_logs` (`action=login_failed`) | >20 → investigate | InfoSec |
| License verification failures on boot | App log | Any → escalate | Engineering |
| Audit log insert failures | stderr log | Any → investigate | Engineering |
| DB connection acquire timeout | App log | >5% → review pool config | Engineering |
| `pgdata` directory size growth | OS | >2× baseline → review archival | Operations |
| Role grant changes | `audit_logs` (`action=user_update`) | All → review quarterly | InfoSec |

---

## 6. Incident response runbook (summary)

### 6.1 Severity classification

| Severity | Definition | Examples | Response time |
|---|---|---|---|
| SEV-1 | Patient safety or catastrophic data loss | R-015 realised; R-005 with no backup; R-001 with confirmed exfiltration | Immediate; war room |
| SEV-2 | Major operational impact or confirmed PHI breach | R-001 confirmed; R-008 confirmed tampering | <4 h |
| SEV-3 | Moderate impact; contained | R-004 lockout storm; R-010 false rejection | <24 h |
| SEV-4 | Minor; no operational impact | R-017 time-skew | Next business day |

### 6.2 Response process

1. **Detect** — alert from audit log review, user report, or monitoring indicator.
2. **Triage** — assign severity per §6.1.
3. **Contain** — disable compromised account (`update_user is_active=false`), revoke sessions (`DELETE FROM sessions WHERE user_id=$1`), or stop the service (`sc stop HMS-PostgreSQL`).
4. **Investigate** — pull `audit_logs` for the affected user/resource/timeframe; review `hms_startup.log`.
5. **Eradicate** — patch the underlying vulnerability (per §5.2 treatment actions).
6. **Recover** — restore from backup (R-005 runbook in `08-Deployment-Installation-Guide.md` §8); re-issue license if fingerprint changed (R-010 runbook in `07-Licensing-Architecture.md` §10).
7. **Notify** — internal stakeholders; regulator if required (per hospital's regulatory regime).
8. **Post-incident review** — within 5 business days; update this register; track remediation to closure.

### 6.3 Forensic preservation

- Snapshot `audit_logs` (`pg_dump -t audit_logs`).
- Copy `%APPDATA%\<bundle-id>\Logs\hms_startup.log`.
- Copy `%ProgramData%\HMS\config.json`, `license.json`, `tls\` (do not modify).
- Preserve Windows event logs (`wevtutil epl System system.evtx`).
- Record all collection in an evidence chain-of-custody log.

### 6.4 Post-incident review template

| Field | Entry |
|---|---|
| Incident ID | INC-YYYY-NN |
| Severity | SEV-N |
| Detected (UTC) | |
| Contained (UTC) | |
| Eradicated (UTC) | |
| Recovered (UTC) | |
| Root cause | |
| Risks implicated | R-NNN |
| Controls that failed | |
| New/updated controls | |
| Register update required? | Yes/No |
| Owner to closure | |
| Target closure date | |

---

## 7. Cross-references

| Risk | ISO 27001 control | SRS requirement | Quality model sub-characteristic |
|---|---|---|---|
| R-001 | A.5.15, A.8.3, A.8.16 | NFR-15, NFR-22 | 6.1 Confidentiality |
| R-002 | A.8.24 | FR-0242, FR-0243, FR-0246 | 6.2 Integrity |
| R-003 | A.5.17, A.8.3 | NFR-19 | 6.1 Confidentiality |
| R-004 | A.8.5 | FR-0021, NFR-11 | 6.5 Authenticity |
| R-005 | A.8.13, A.5.30 | NFR-30, NFR-35 | 5.4 Recoverability |
| R-006 | A.5.23, A.8.24 | NFR-17 | 6.1 Confidentiality |
| R-007 | A.5.15, A.8.2 | NFR-15 | 6.1 Confidentiality |
| R-008 | A.8.3, A.8.16 | NFR-14 | 6.3 Non-repudiation |
| R-009 | A.8.25 | C-07 | 6.2 Integrity |
| R-010 | A.8.24 | FR-0247 | 5.1 Maturity |
| R-011 | A.8.14 | C-04 | 5.2 Availability |
| R-012 | A.8.25 | NFR-55 | 7.5 Testability |
| R-013 | A.8.12, A.5.23 | FR-0087 | 6.1 Confidentiality |
| R-014 | A.8.25, A.8.28 | NFR-15 | 6.2 Integrity |
| R-015 | A.5.15 | FR-0123 | 1.2 Functional correctness |
| R-019 | A.8.25 | NFR-50 | 1.2 Functional correctness |
| R-021 | A.5.23 (information transfer / CSP) | NFR-50 | 6.1 Confidentiality |
| R-022 | A.5.17, A.8.5 (bootstrap admin) | FR-0021, NFR-11 | 6.5 Authenticity |
| R-023 | A.8.28 (secure coding — race) | FR-0144 | 1.2 Functional correctness |
| R-024 | A.8.28 (secure coding — race) | FR-0072 | 1.2 Functional correctness |
| R-025 | A.5.15, A.8.3 (consent enforcement) | FR-0035, FR-0087 | 6.1 Confidentiality |
| R-026 | A.8.25 (correctness — tz) | FR-0087 | 1.2 Functional correctness |
| R-027 | A.8.3, A.5.28 (clinical FK RESTRICT) | FR-0040 | 6.2 Integrity |
| R-028 | A.8.28 (upsert correctness) | FR-0087 | 1.2 Functional correctness |
| R-029 | A.5.15, A.8.3 (db_password DTO) | NFR-15, NFR-19 | 6.1 Confidentiality |
| R-030 | A.5.15, A.8.16 (messaging RBAC + audit) | NFR-15 | 6.5 Authenticity |
| R-031 | A.8.25, A.8.28 (atomic save) | (operational) | 5.4 Recoverability |
| R-032 | A.5.17, A.8.3, A.8.12 (DPAPI) | NFR-19 | 6.1 Confidentiality |
| R-033 | A.8.25 (TS strict gate) | NFR-50 | 7.5 Testability |
| R-034 | A.8.24 (OsRng), A.8.5 (rate limit) | FR-0021 | 6.5 Authenticity |
| R-035 | A.5.23 (LAN broadcast authenticity) | NFR-17 | 6.1 Confidentiality |
| R-036 | A.5.15, A.8.12 (log access RBAC + redact) | NFR-15, NFR-22 | 6.1 Confidentiality |
| R-037 | A.8.28 (SQL identifier validation) | NFR-15 | 6.2 Integrity |
| R-038 | A.5.23, A.8.24 (TLS key ACL) | NFR-17 | 6.1 Confidentiality |
| R-039 | A.5.23, A.8.3 (pg_hba scoping) | NFR-17, NFR-19 | 6.1 Confidentiality |
| R-040 | A.8.25 (generic error messages) | (operational) | 6.1 Confidentiality |

---

_End of `05-Risk-Register-ISO-31000.md`. Cross-reference `04-Security-Control-Matrix-ISO-27001.md` for control mapping, `01-SRS-Software-Requirements.md` for requirement traceability, and `08-Deployment-Installation-Guide.md` §8 for the backup/recovery runbook._
