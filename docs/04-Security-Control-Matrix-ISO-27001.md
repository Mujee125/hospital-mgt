# VitalFlow HMS — ISO/IEC 27001:2022 Security Control Matrix

> **Document version: v0.2.0 — updated 2025-07-08 after Phase 2 Batches 0-3 implementation. See CHANGELOG.md for details.**

| Field | Value |
|---|---|
| **Document title** | VitalFlow HMS — ISO/IEC 27001:2022 Annex A Control Matrix |
| **Standard** | ISO/IEC 27001:2022 (Information security, cybersecurity and privacy protection — Information security management systems — Requirements) Annex A |
| **Version** | 0.2.0 |
| **Date** | 2025-07-08 |
| **Status** | Draft |
| **Classification** | Internal |
| **Owner** | VitalFlow HMS Engineering / Information Security Officer |
| **Author** | Documentation Specialist (Task 7); reconciled v0.2.0 by Documentation Team (B4-B) |
| **Related documents** | `01-SRS-Software-Requirements.md`, `02-SDD-Software-Design.md`, `03-Quality-Model-ISO-25010.md`, `05-Risk-Register-ISO-31000.md`, `07-Licensing-Architecture.md`, `08-Deployment-Installation-Guide.md` |

---

## 1. ISMS scope statement

### 1.1 Scope

The VitalFlow HMS Information Security Management System (ISMS) covers the people, processes, and technology involved in the design, development, deployment, and operation of the VitalFlow HMS desktop application and its bundled PostgreSQL backend, for a single hospital per licensed deployment.

**In scope**:

- The Tauri v2 desktop application (server-build and client-build installers).
- The bundled PostgreSQL 16+ database, configuration files (`%ProgramData%\HMS\`), TLS material, and license file.
- The LAN over which clients connect to the server.
- The software company's offline license-issuing process (Ed25519 key management).
- VitalFlow engineering workstations used to build and sign releases.

**Out of scope** (this revision):

- The hospital's own corporate IT infrastructure (network, Active Directory, endpoint protection) — governed by the hospital's ISMS, not VitalFlow's.
- Public-internet-facing services — by design VitalFlow HMS is LAN-only.
- Third-party PACS/RIS/LIS integrations — Phase 2.

### 1.2 Statement of Applicability (summary)

Of the 93 Annex A controls in ISO/IEC 27001:2022, this SoA summarises the 22 controls most directly applicable to a single-hospital desktop HMS deployment. The remaining controls are addressed at the hospital's organisational level (e.g. A.5.1 policies, A.5.10 acceptable use, A.6 people controls, A.7 physical controls) and are noted as "Organisational — hospital" where relevant.

| Control cluster | Controls in scope for VitalFlow HMS | Controls deferred to hospital ISMS |
|---|---|---|
| A.5 Organizational | A.5.15, A.5.17, A.5.23, A.5.24, A.5.25, A.5.26, A.5.27, A.5.28, A.5.30 | A.5.1, A.5.2, A.5.7, A.5.8, A.5.10, A.5.13, A.5.14 |
| A.6 People | (limited — engineering staffing) | A.6.1–A.6.6 (hospital HR) |
| A.7 Physical | A.7.7 (clear desk, applied to license media) | A.7.1–A.7.14 (hospital premises) |
| A.8 Technological | A.8.2, A.8.3, A.8.5, A.8.12, A.8.13, A.8.14, A.8.16, A.8.23, A.8.24, A.8.25 | A.8.1 (user endpoint devices — hospital), A.8.6–A.8.11 (largely N/A for desktop), A.8.15 (logging — partially covered by A.8.16), A.8.17–A.8.22 (mostly N/A), A.8.26–A.8.37 (mostly N/A or organisational) |

### 1.3 Status legend

| Status | Meaning |
|---|---|
| **Implemented** | Control fully in place and evidence-backed. |
| **Partial** | Control in place but with documented gaps. |
| **Planned** | Control not yet implemented; remediation scheduled. |
| **N/A** | Control not applicable to VitalFlow HMS scope. |

> **v0.2.0 convention:** where a control was a Phase 1 *aspirational* claim that the implementation did not actually meet, but Batches 0–3 have since closed the gap, the implementation column is annotated with the marker **[Implemented v0.2.0]** (or **[Improved v0.2.0]** / **[Mitigated v0.2.0]** as appropriate) and a batch reference (Batch 0 / 1 / 2 / 3). Where the gap remains open, the marker **[Planned Phase 2]** / **[Planned Batch 5]** is used.

### 1.4 Revision history

| Version | Date | Author | Summary |
|---|---|---|---|
| 0.1.0 | 2026-07-02 | Documentation Specialist (Task 7) | Initial ISO 27001:2022 Annex A control matrix baseline. |
| 0.2.0 | 2025-07-08 | Documentation Team (B4-B) | Reconciled with Phase 2 Batches 0–3 code changes: M-01 RBAC universality actually delivered (CR-4/CR-16/SEC-05 + 2 new messaging permissions, 35→37); M-02 audit universality actually delivered (messaging ×4, config save/repair/clear, consent ×3, inventory ×6, license revoke — all now write audit rows); M-03 tsconfig strict gate implemented (Batch 0); M-04 OsRng for pairing codes (SEC-03 Batch 3); M-05 log access RBAC + redaction (SEC-05 Batch 3); M-06 CSP enforced (CR-3 Batch 1); M-07 random bootstrap password + ACL-protected credentials file (CR-2 Batch 1); M-08 audit append-only trigger still Planned Phase 2 (universality improved). |

---

## 2. Annex A control matrix

### 2.1 A.5 Organizational controls

| Control | Title | VitalFlow implementation | Status | Notes |
|---|---|---|---|---|
| **A.5.15** | Access control | RBAC enforced on every protected Tauri command via `rbac::require(&session, Permission::XxxYyy)`. 8 seeded roles (super_admin, doctor, nurse, receptionist, lab_technician, pharmacist, billing_clerk, patient) with least-privilege grant matrix in `rbac::permissions_for_role`. Frontend mirrors via `lib/rbac.ts` and permission-filtered sidebar; backend re-checks on every call. Data-driven `role_permissions` table allows policy updates without code change. Single active session per user. **[Implemented v0.2.0]** — Phase 1 audit found 7 protected commands missing the RBAC gate (messaging ×4, config ×2, whatsapp_to_patient ×1); Batches 1 & 3 closed the gap. Evidence: `config.rs::get_config/save_config/repair_server_config/clear_config` require `SettingsManage`; `messaging.rs::send_message/delete_message` require `MessagingSend` and `get_messages/get_rooms` require `MessagingView`; `whatsapp/commands.rs::set_whatsapp_config/test_whatsapp_api` require `SettingsManage`; `lib.rs::get_log/get_log_path` require `SettingsManage` (SEC-05). Two new permission keys were added: `MessagingView` (`messaging.view`) and `MessagingSend` (`messaging.send`) — total 35 → 37 Permission variants. | Implemented | Cross-ref: SRS FR-0020–FR-0029, NFR-15. Risk R-030. |
| **A.5.16** | Identity management | User accounts in `users` table; created/updated/deleted via `auth::{create_user, update_user, delete_user}` gated by `users.manage`. Bootstrap admin seeded once with forced password change. No backdoor account. | Implemented | Lifecycle (leaver process) is the hospital's HR responsibility. |
| **A.5.17** | Authentication information | Passwords stored as Argon2id PHC strings (m=19456 KiB, t=2, p=1) via `argon2` 0.5. Plaintext never persisted. Session tokens are 32 random bytes, base64url-encoded; only the SHA-256 hash is persisted in `sessions.token_hash`. Password reset forces `must_change_password=true` and invalidates all sessions for the user. **[Improved v0.2.0]** — Batch 1 CR-2: bootstrap admin password is now randomly generated (24-char CSPRNG over an unambiguous alphabet) and written to an ACL-protected `bootstrap-credentials.txt` (SYSTEM + Administrators only) at first-run seed. The Phase 1 hardcoded `admin/ChangeMe123!` pair is removed; the Login UI no longer displays credentials. | Implemented | Cross-ref: SRS NFR-10–NFR-13. Risk R-022. |
| **A.5.23** | Information transfer | LAN PostgreSQL connections use TLS (`sslmode=verify-ca`) with pinned server cert (`db.rs::build_url`). Pairing exchange is TLS-protected (rustls). LAN discovery broadcast contains only server IP + DB port (no credentials). No data leaves the LAN by design. **[Improved v0.2.0]** — Batch 1 CR-3: strict CSP added to `tauri.conf.json` — `default-src 'self' ipc: http://ipc.localhost; script-src 'self'; connect-src 'self' ipc: http://ipc.localhost` (relaxed only for `img-src`/`font-src`/`style-src` for legitimate asset hosts). Batch 3 SEC-08: LAN discovery broadcast now carries an HMAC-SHA256 tag over the `{ip, port, nonce}` payload so a LAN attacker cannot spoof a server (the HMAC key is established during pairing). | Implemented | Public-internet transfer is out of scope (LAN-only deployment). Risk R-035. |
| **A.5.24** | Information security incident management planning | Process documented in `05-Risk-Register-ISO-31000.md` §6 (incident response runbook). Severity matrix maps to escalation. | Partial | Incident response drills not yet conducted. |
| **A.5.25** | Assessment and decision on information security events | Risk register (`05-Risk-Register-ISO-31000.md`) defines likelihood×impact 5×5; incident classification aligns. | Partial | No dedicated incident response team named. |
| **A.5.26** | Response to information security incidents | Audit log (`audit_logs` table) is the primary forensic source. Login failures with reasons (`unknown_user`, `bad_password`, `locked`, `inactive`) are recorded. License verification failures are surfaced on the license gate. | Partial | No automated alerting (e.g. SIEM forwarding). |
| **A.5.27** | Learning from information security incidents | Post-incident review template included in `05-Risk-Register-ISO-31000.md` §6.4. | Partial | No incidents recorded yet (system not in production). |
| **A.5.28** | Collection of evidence | Audit log rows include `user_id`, `username`, `action`, `resource`, `resource_id`, `details` (JSONB), `created_at`. App log (`hms_startup.log`) captures boot-time diagnostics. **[Improved v0.2.0]** — Audit universality expanded (see A.8.16 / M-02): messaging send/delete, config save/repair/clear, consent set/revoke, inventory adjust, license install/revoke now all write audit rows. Append-only DB trigger is still Planned Phase 2 (see A.8.3 / M-08). | Partial | Audit log not yet append-only at DB level — see A.8.3. |
| **A.5.30** | ICT readiness for information security | DB migrations are idempotent (re-runnable). Client self-heal via LAN discovery. Server auto-repairs broken SSL. Manual `pg_dump` backup documented in `08-Deployment-Installation-Guide.md`. | Partial | No automated backup; no warm standby. |

### 2.2 A.6 People controls

| Control | Title | VitalFlow implementation | Status | Notes |
|---|---|---|---|---|
| **A.6.1–A.6.6** | Screening, terms of employment, etc. | Engineering staffing is the software company's HR responsibility. Hospital end-user screening is the hospital's HR responsibility. | N/A | Out of VitalFlow HMS scope. |
| **A.6.7** | Remote working | License-issuing private key is held offline by the software company; remote access to it is governed by the company's own ISMS. | Partial | Documented in `07-Licensing-Architecture.md` §10. |

### 2.3 A.7 Physical controls

| Control | Title | VitalFlow implementation | Status | Notes |
|---|---|---|---|---|
| **A.7.1–A.7.6, A.7.8–A.7.14** | Physical security perimeters, entry, etc. | Hospital premises — hospital ISMS responsibility. | N/A | |
| **A.7.7** | Clear desk and clear screen | License-issuing private key media kept in locked storage when not in use (per `07-Licensing-Architecture.md` §10). App auto-locks not implemented (desktop HMS — single user, assumed attended). | Partial | Consider an idle-session lock in Phase 2. |

### 2.4 A.8 Technological controls

| Control | Title | VitalFlow implementation | Status | Notes |
|---|---|---|---|---|
| **A.8.1** | User endpoint devices | Hospital-managed PCs. VitalFlow requires Windows 10/11 with .NET runtime for WMI. | N/A | Hospital ISMS. |
| **A.8.2** | Privileged access rights | `super_admin` role has all 37 permissions; used only for break-glass. `RolesManage`, `UsersManage`, `SettingsManage`, `LicenseManage`, `BackupsManage` permissions are gated separately and granted only to `super_admin`. **[Updated v0.2.0]** — Two new permissions (`MessagingView`, `MessagingSend`) added in Batch 1 CR-16; total 35 → 37. | Implemented | Cross-ref: `rbac.rs::permissions_for_role`. |
| **A.8.3** | Information access restriction | RBAC on every command. DTOs omit `password_hash` (`#[serde(skip_serializing)]`). `ON DELETE RESTRICT` on critical FKs prevents accidental data loss. **[Improved v0.2.0]** — `AppConfig.db_password` is `#[serde(skip_serializing)]` so `get_config` (CR-4) no longer returns it to the frontend. Audit universality improved (see A.8.16 / M-02). Append-only trigger still Planned Phase 2 (M-08). | Partial | Audit log rows are not yet immutable at DB level — add a trigger rejecting `UPDATE`/`DELETE` on `audit_logs` in Phase 2 (M-08). |
| **A.8.5** | Secure authentication | Argon2id (OWASP-recommended minimum), 5-attempt/15-min brute-force lockout, single active session, 32-byte random tokens with SHA-256 hash at rest, constant-time login (dummy Argon2 verify on unknown usernames), 12-hour session expiry, server-side re-validation on `me`. | Implemented | No MFA yet (planned for `super_admin`/`billing_clerk` in Phase 2). |
| **A.8.12** | Data leakage prevention | LAN-only deployment. No PHI in `hms_startup.log`. `password_hash` never sent to frontend. No public-internet egress. **[Improved v0.2.0]** — Batch 3 SEC-05: `get_log`/`get_log_path` are RBAC-gated behind `SettingsManage` (admin-only); `redact_log()` masks `password=`/`db_password=`/`db_user=`/`user=`/`username=` patterns at read time (on-disk log unchanged for ops); failed-login audit rows no longer record the attempted username (only `reason: unknown_user`/`bad_password`). `get_config` (CR-4) skips `db_password` from serialization. | Partial | No DLP at OS level (hospital responsibility); consider blocking USB mass storage on hospital PCs. Risk R-032 (config.json plaintext — DPAPI Planned Batch 5). |
| **A.8.13** | Information backup | Manual `pg_dump` documented in `08-Deployment-Installation-Guide.md`. Idempotent migrations enable safe re-run. License file is regenerable by the software company from the hardware fingerprint. | Partial | No automated backup UI; no PITR documentation. See Risk R-005. |
| **A.8.14** | Redundancy of information processing facilities | Single server PC, no warm standby. Target availability 99% operating hours. | Partial | Single-server model is a documented constraint; warm standby considered for Phase 3. |
| **A.8.16** | Monitoring activities | `audit_logs` table records every state-changing command with user, action, resource, resource_id, JSONB details, timestamp. App log captures boot diagnostics. Login/logout audited. `get_audit_logs` queryable in-app by `audit.view`. **[Implemented v0.2.0]** — Phase 1 audit found that messaging send/delete, config save/repair/clear, consent set/revoke, inventory adjust, and license revoke wrote ZERO audit rows. Batches 1 & 3 closed the gap: all now write audit rows. `get_log`/`get_log_path` (lib.rs) are RBAC-gated behind `SettingsManage` (SEC-05) so admin-only access. | Implemented | Read commands intentionally not row-level audited (volume); no SIEM forwarding. Risk R-008, R-020. |
| **A.8.23** | Web filtering | N/A — desktop application, no general web browsing. WhatsApp integration is outbound-only to a configured endpoint; no general internet access from the app. | N/A | Hospital may enforce web filtering at the network level. |
| **A.8.24** | Use of cryptography | Argon2id for passwords, SHA-256 for session tokens and hardware fingerprint, Ed25519 for license signatures, rustls (ring provider) for TLS, rcgen for self-signed cert generation. All keys generated via `OsRng` (cryptographically secure). No custom crypto. **[Implemented v0.2.0]** — Batch 3 SEC-03: pairing-code generation migrated from `rand::thread_rng()` (userspace PRNG, predictable to an attacker who observed prior codes) to `OsRng` (OS CSPRNG). All security-critical randomness (session tokens, Argon2 salts, bootstrap password, pairing codes) now uses OsRng. Batch 3 SEC-08: HMAC-SHA256 added to LAN discovery broadcast for authenticity. | Implemented | License private key held offline by software company (see `07-Licensing-Architecture.md` §10). Risk R-002, R-034. |
| **A.8.25** | Secure development life cycle | TypeScript strict mode gate (`tsc --noEmit`). Manual Rust review. Idempotent DB migrations. Centralised models/queries. RBAC + audit pattern enforced per command. See `06-SDLC-ISO-12207.md`. **[Implemented v0.2.0 (Batch 0)]** — `tsconfig.json` now exists with `strict: true`, `noUnusedLocals`, `noUnusedParameters`, `noUncheckedIndexedAccess`, `noImplicitOverride`, `noImplicitReturns`, `noFallthroughCasesInSwitch`. `tsc --noEmit` is the enforced build gate (every `npm run build*` script invokes it). ESLint 9 flat config (`eslint.config.js`) + Prettier 3 (`.prettierrc.json`) added. The Phase 1 doc claim was aspirational; it is now reality. | Partial | No `cargo test` yet; no SAST/DAST; no dependency-vuln scan in CI. `cargo audit` + `npm audit` Planned Batch 5. |
| **A.8.28** | Secure coding | See A.8.25. SQL injection mitigated by `sqlx::query` parameterised bindings (no string interpolation of user input into SQL). No `format!` into SQL for user-controlled values. **[Implemented v0.2.0]** — Batch 3 SEC-10: the one pre-existing identifier-interpolation site (`CREATE DATABASE "{name}"` in `pg_provision.rs`) now validates the identifier against a strict `^[A-Za-z_][A-Za-z0-9_]*$` allow-list before interpolation, rejecting attacker-controlled names that could escape quoting. All other SQL uses parameterised bindings. | Implemented | Manual review confirms no string-interpolated SQL with user input. Risk R-014, R-037. |
| **A.8.29** | Security testing in development and acceptance | Manual code review; tsc strict gate. **[Implemented v0.2.0 (Batch 0)]** — `tsc --noEmit` + ESLint 9 + Prettier 3 are the enforced Phase 1 quality gates (see §3.6 / SDLC §5.1). No penetration test; no fuzzing; no SAST/DAST. **[Planned Batch 5]** — CI runner + `cargo audit`/`npm audit`. | Partial | No penetration test; no fuzzing; no SAST/DAST. Planned Phase 2. |
| **A.8.30** | Outsourced development | N/A — development is in-house at the software company. | N/A | |
| **A.8.31** | Separation of development, test and production environments | Production = single-hospital deployment on Windows. Development = non-Windows fallback (license fingerprint uses hostname+OS — explicitly not a production fingerprint). | Partial | No formal staging environment; test on Windows VM before production deploy. |

---

## 3. Detailed implementation notes

### 3.1 A.5.15 Access control — implementation detail

**RBAC architecture**:

- Source of truth: `rbac.rs::Permission` enum (**37 variants** as of v0.2.0 — was 35; +`MessagingView`, +`MessagingSend`). Stable string keys via `as_str()`.
- DB mirror: `permissions`, `roles`, `role_permissions`, `user_roles` tables seeded by `auth::seed_defaults`.
- Runtime guard: `rbac::require(&session_state, Permission::X)` — returns the cloned `Session` on success or an `"Access denied: ..."` error string. Every protected command begins with this call.
- Frontend mirror: `src/lib/rbac.ts` exports the same string keys. `<RequirePermission>` and the permission-filtered sidebar (`Sidebar.tsx`) hide inaccessible UI. Frontend hiding is UX only; backend re-checks.

**v0.2.0 RBAC universality closure (M-01)** — Phase 1 audit found 7 protected Tauri commands missing the `rbac::require` gate. All are now gated:

| Module | Command | Required permission | Batch |
|---|---|---|---|
| `config.rs` | `get_config` (post-setup) | `SettingsManage` | 1 (CR-4) |
| `config.rs` | `save_config` (post-setup) | `SettingsManage` | 1 (CR-4) |
| `config.rs` | `repair_server_config` (post-setup) | `SettingsManage` | 1 (CR-4) |
| `config.rs` | `clear_config` | `SettingsManage` | 1 (CR-4) |
| `messaging.rs` | `send_message` | `MessagingSend` | 1 (CR-16) |
| `messaging.rs` | `delete_message` | `MessagingSend` | 1 (CR-16) |
| `messaging.rs` | `get_messages` | `MessagingView` | 1 (CR-16) |
| `messaging.rs` | `get_rooms` | `MessagingView` | 1 (CR-16) |
| `whatsapp/commands.rs` | `set_whatsapp_config` | `SettingsManage` | 1 (CR-4) |
| `whatsapp/commands.rs` | `test_whatsapp_api` | `SettingsManage` | 1 (CR-4) |
| `lib.rs` | `get_log` | `SettingsManage` | 3 (SEC-05) |
| `lib.rs` | `get_log_path` | `SettingsManage` | 3 (SEC-05) |

`messaging.rs::send_message` and `delete_message` additionally derive the sender identity from the authenticated session (not from a client-supplied string), closing an impersonation gap.

**Least-privilege role matrix** (excerpt — full matrix in `02-SDD-Software-Design.md` §5.2 and `rbac.rs::permissions_for_role`):

| Role | Notable permissions | Notable exclusions |
|---|---|---|
| super_admin | All 37 | — |
| doctor | PatientsView/Create/Update, AppointmentsView/Update, IpdView/Manage, LabView/Order/ResultManage, PatientConsentManage, AuditView, ReportsView | No billing create, no users manage, no settings manage |
| nurse | PatientsView/Update, QueueView/Manage, IpdView/Manage, BedsManage | No billing, no lab result manage |
| receptionist | PatientsView/Create/Update, AppointmentsView/Create/Update, QueueView/Manage, BillingView/Create | No IPD, no lab, no users |
| lab_technician | LabView/Order/ResultManage/CatalogManage, InventoryView | No patient create/update, no billing |
| pharmacist | InventoryView/Manage, BillingView, PatientsView | No patient create/update, no appointments |
| billing_clerk | BillingView/Create/Manage, PaymentsManage, ReportsView | No clinical access, no users manage |
| patient | DashboardView only | Portal — Phase 2 |

### 3.2 A.5.17 Authentication information — implementation detail

| Aspect | Implementation |
|---|---|
| Password hashing | `argon2::Argon2::new(Algorithm::Argon2id, Version::V0x13, Params::new(19_456, 2, 1, None))` |
| Salt | `SaltString::generate(&mut OsRng)` — 128-bit cryptographically random |
| Verification | `PasswordVerifier::verify_password(argon2, plain, &PasswordHash::from_str(phc))` |
| Session token | 32 bytes from `RandOsRng.fill_bytes`, base64url-encoded |
| Token-at-rest | SHA-256 hex of the raw token, persisted in `sessions.token_hash` (PRIMARY KEY) |
| Single active session | `DELETE FROM sessions WHERE user_id = $1` on successful login |
| Expiry | 12 hours (`SESSION_HOURS = 12`); `me` re-validates against DB |
| Lockout | `MAX_FAILED_ATTEMPTS = 5`, `LOCKOUT_MINUTES = 15` |
| Reset | `reset_user_password` sets `must_change_password=true` and `DELETE FROM sessions WHERE user_id = $1` |
| Bootstrap | **v0.2.0 (CR-2):** `admin` / `<24-char random CSPRNG password>` with `must_change_password=true`, seeded once when no users exist. Password is written to `%ProgramData%\HMS\bootstrap-credentials.txt` (ACL: SYSTEM + Administrators only). The Phase 1 hardcoded `admin/ChangeMe123!` is removed. |

### 3.3 A.8.5 Secure authentication — implementation detail

The login flow in `auth.rs::login` is hardened against the OWASP ASVS V2 threats:

1. **Username enumeration**: on unknown username, run a dummy Argon2 verify against a fixed PHC string before returning the generic `"Invalid username or password."` error. This flattens the timing side-channel.
2. **Brute force**: increment `failed_login_count` on each bad password; set `locked_until = now + 15min` when count ≥ 5. Reset on success.
3. **Account enumeration via lockout difference**: locked accounts return the same generic message family as bad-password accounts to avoid leaking lock state.
4. **Session fixation**: prior sessions are deleted on login (single active session).
5. **Session token disclosure**: raw token never persisted; only SHA-256 hash at rest.
6. **Session expiry**: 12-hour `expires_at`; `me` re-checks.
7. **Audit**: every login attempt (success or failure with reason) is recorded in `audit_logs`.

### 3.4 A.8.16 Monitoring activities — implementation detail

| Event | Audited? | Action string | Resource |
|---|---|---|---|
| Login success | Yes | `login_success` | `auth` |
| Login failure (unknown user) | Yes | `login_failed` (details: `{"reason":"unknown_user"}`) | `auth` |
| Login failure (bad password) | Yes | `login_failed` (details: `{"reason":"bad_password","attempts":N}`) | `auth` |
| Login failure (inactive) | Yes | `login_failed` (details: `{"reason":"inactive"}`) | `auth` |
| Login failure (locked) | Yes | `login_failed` (details: `{"reason":"locked","locked_until":...}`) | `auth` |
| Logout | Yes | `logout` | `auth` |
| Password change (self) | Yes | `password_change` | `auth` |
| Password reset (admin) | Yes | `password_reset` | `users` |
| User create/update/delete | Yes | `user_create` / `user_update` / `user_delete` | `users` |
| Patient create/update/delete | Yes | `patient_create` / `patient_update` / `patient_delete` | `patients` |
| Appointment create/update/delete/status | Yes | `appointment_create` / `_update` / `_delete` / `_status` | `appointments` |
| Queue token create/call/status | Yes | `queue_token_create` / `_call` / `_status` | `queue` |
| IPD admit/discharge | Yes | `ipd_admit` / `ipd_discharge` | `ipd` |
| Lab order create | Yes | `lab_order_create` | `lab` |
| Lab result update | Yes | `lab_result_update` | `lab` |
| Bill create | Yes | `bill_create` | `billing` |
| Payment record | Yes | `payment_record` | `billing` |
| License install | Yes (if session) | `license_install` | `license` |
| **Message send** (v0.2.0) | **Yes** | `message_send` | `messages` |
| **Message delete** (v0.2.0) | **Yes** | `message_delete` | `messages` |
| **Config save** (v0.2.0) | **Yes** | `config_save` | `config` |
| **Config repair** (v0.2.0) | **Yes** | `config_repair` | `config` |
| **Config clear** (v0.2.0) | **Yes** | `config_clear` | `config` |
| **Patient consent set** (v0.2.0) | **Yes** | `consent_set` | `patients` |
| **Patient consent revoke** (v0.2.0) | **Yes** | `consent_revoke` | `patients` |
| **Inventory adjust** (v0.2.0) | **Yes** | `inventory_adjust` | `inventory` |
| **License revoke** (v0.2.0) | **Yes** | `license_revoke` | `license` |
| **WhatsApp config update** (v0.2.0) | **Yes** | `whatsapp_config_update` | `whatsapp` |
| **WhatsApp send to patient** (v0.2.0) | **Yes** | `whatsapp_send` | `whatsapp` |
| Patient read | No | — | — (volume; intentional) |
| Audit log read | No | — | — (would self-referentially explode volume) |

**v0.2.0 audit-universality closure (M-02)** — Phase 1 audit found messaging ×2 + config ×3 + consent ×2 + inventory adjust + license revoke wrote ZERO audit rows. All now write rows (Batch 1 CR-4/CR-12/CR-16/CR-21 + Batch 3 LIC-DOC-04).

`get_audit_logs` is gated by `audit.view` and supports filtering by action/resource with a 5000-row clamp. `get_log`/`get_log_path` are gated by `SettingsManage` (SEC-05, Batch 3) and `get_log` applies `redact_log()` at read time.

### 3.5 A.8.24 Cryptography — implementation detail

| Purpose | Algorithm | Parameters | Library |
|---|---|---|---|
| Password hashing | Argon2id | m=19456 KiB, t=2, p=1 (OWASP 2023 minimum) | `argon2` 0.5 |
| Session token at rest | SHA-256 | n/a | `sha2` 0.10 |
| Hardware fingerprint | SHA-256 | domain-separation prefix `vitalflow-hms-fp-v1\0` | `sha2` 0.10 |
| License signature | Ed25519 | deterministic, 64-byte signature | `ed25519-dalek` 2 |
| License canonical form | Compact JSON via BTreeMap | sorted keys, no whitespace | `serde_json` 1 |
| TLS | TLS 1.3 (rustls ring provider) | self-signed cert (rcgen) pinned by SHA-256 fingerprint | `rustls` 0.23, `tokio-rustls` 0.26 |
| Random number generation | OS CSPRNG | `OsRng` | `rand` 0.8, `rand_core` 0.6 |
| PostgreSQL password auth | scram-sha-256 | per `pg_hba.conf` | PostgreSQL 16+ |
| LAN discovery authenticity (v0.2.0) | HMAC-SHA256 | keyed over `{ip, port, nonce}` payload | `hmac` 0.12, `sha2` 0.10 |

No custom cryptography is used. All keys are generated via `OsRng`.

**v0.2.0 (SEC-03 / M-04):** Pairing-code generation was migrated from `rand::thread_rng()` (userspace ChaCha PRNG, predictable to an attacker who observed prior codes) to `OsRng` (OS CSPRNG via `RtlGenRandom` on Windows / `getrandom` on Linux). All security-critical randomness — session tokens, Argon2 salts, bootstrap password, pairing codes — now uses `OsRng`.

**v0.2.0 (SEC-08):** LAN discovery broadcast (`discovery.rs`) carries an HMAC-SHA256 tag computed over the `{ip, port, nonce}` payload using a key established during pairing. A LAN attacker cannot spoof a fake server because they do not know the HMAC key.

**v0.2.0 (SEC-13 / SEC-15):** TLS private key file ACL is hardened (SYSTEM + Administrators only) before the key is written. `pg_hba.conf` is restricted to the provisioned HMS DB user (not any DB user from the LAN).

### 3.6 A.8.25 / A.8.28 Secure development — implementation detail

| Practice | Status | Evidence |
|---|---|---|
| Parameterised SQL | Implemented | `sqlx::query("...$1...").bind(x)` throughout `commands/*`; no string interpolation of user input into SQL. v0.2.0 (SEC-10): the lone identifier-interpolation site (`CREATE DATABASE "{name}"`) is now guarded by an `^[A-Za-z_][A-Za-z0-9_]*$` allow-list. |
| Input validation | Implemented | Username non-empty, password ≥8 chars, bill items validated server-side. v0.2.0 (IPC-09): WhatsApp `send_whatsapp_to_patient` validates patient existence + message length. v0.2.0 (IPC-07): payment NaN guard. |
| Output DTO sanitisation | Implemented | `password_hash` is `#[serde(skip_serializing)]`. v0.2.0 (CR-4): `AppConfig.db_password` is also `#[serde(skip_serializing)]` so `get_config` no longer returns it. |
| TS strict mode | **Implemented v0.2.0 (Batch 0)** | `tsconfig.json` (was missing in Phase 1) with `strict: true`, `noUnusedLocals`, `noUnusedParameters`, `noUncheckedIndexedAccess`, `noImplicitOverride`, `noImplicitReturns`, `noFallthroughCasesInSwitch`. `tsc --noEmit` is the build gate (invoked by every `npm run build*` script). Verified 0 errors on the current tree. |
| ESLint | **Implemented v0.2.0 (Batch 0)** | ESLint 9 flat config (`eslint.config.js`) with `typescript-eslint`, `eslint-plugin-react-hooks`, `eslint-plugin-react-refresh`, `eslint-config-prettier`. `npm run lint` script wired. |
| Prettier | **Implemented v0.2.0 (Batch 0)** | Prettier 3 (`.prettierrc.json`); `npm run format`/`format:check` scripts wired. |
| Centralised models | Implemented | `src/lib/models.ts` is the single source for TS shapes |
| Centralised queries | Implemented | `src/lib/queries.ts` with shared `qk` keys |
| RBAC pattern enforcement | **Implemented v0.2.0 (Batch 1/3)** | Every protected command begins with `rbac::require` — see §3.1 for the v0.2.0 closure of 12 commands (M-01). |
| Audit pattern enforcement | **Implemented v0.2.0 (Batch 1/3)** | Every state-changing command ends with `audit::for_session` — see §3.4 for the v0.2.0 closure of 10 commands (M-02). |
| Dependency pinning | Implemented | `Cargo.lock` and `package-lock.json` committed |
| SAST | Planned | Not yet integrated. **Planned Batch 5.** |
| DAST | Planned | Not yet integrated (limited applicability — no HTTP server). |
| Dependency vulnerability scan | Planned | `cargo audit` / `npm audit` not yet in CI. **Planned Batch 5.** |
| Code review | Implemented (manual) | No PR tooling configured in this revision. CI runner is Planned Batch 5. |

### 3.7 A.8.13 Backup — implementation detail

**Current state**:

- Manual `pg_dump` is the only backup mechanism. Documented in `08-Deployment-Installation-Guide.md` §8.
- Idempotent migrations enable safe re-run after restoring a dump.
- The license file is regenerable by the software company from the hardware fingerprint (i.e. re-issuing is the recovery path for a lost license file).

**Phase 2 plan**:

- Settings → Backup panel wrapping `pg_dump` with one-click invocation.
- Scheduled backup via Windows Task Scheduler template.
- Restore path in the installer ("restore from backup").
- WAL archiving documentation for PITR.

---

## 4. Risk cross-reference

| ISO 27001 control | Risk register entries (in `05-Risk-Register-ISO-31000.md`) |
|---|---|
| A.5.15 Access control | R-007 (over-privileged role), R-014 (SQL injection) |
| A.5.17 Authentication info | R-004 (brute force), R-003 (DB password exposure) |
| A.5.23 Information transfer | R-006 (LAN eavesdropping) |
| A.5.24–A.5.28 Incident management | R-001 (PHI breach), R-008 (audit tampering) |
| A.5.30 ICT readiness | R-005 (DB loss/no backup) |
| A.8.2 Privileged access | R-007 (over-privileged role), R-009 (installer privilege misuse) |
| A.8.3 Information access restriction | R-001 (PHI breach), R-008 (audit tampering) |
| A.8.5 Secure authentication | R-004 (brute force) |
| A.8.12 Data leakage | R-013 (WhatsApp data leakage) |
| A.8.13 Backup | R-005 (DB loss/no backup) |
| A.8.14 Redundancy | R-011 (single-session DoS), R-005 (DB loss) |
| A.8.16 Monitoring | R-001 (PHI breach), R-008 (audit tampering) |
| A.8.23 Web filtering | N/A (LAN-only desktop) |
| A.8.24 Cryptography | R-002 (license forgery/tampering), R-010 (fingerprint drift) |
| A.8.25 Secure development | R-012 (supply chain), R-014 (SQL injection), R-015 (dispensing error) |

---

## 5. Statement of Applicability — formal summary

Pursuant to ISO/IEC 27001:2022 §6.1.3 d), the VitalFlow HMS Statement of Applicability is summarised below. The full SoA (with justification for inclusion/exclusion of each of the 93 Annex A controls) is maintained as a controlled document by the Information Security Officer.

| Cluster | Included | Excluded (with reason) |
|---|---|---|
| A.5 Organizational | A.5.15, A.5.16, A.5.17, A.5.23, A.5.24, A.5.25, A.5.26, A.5.27, A.5.28, A.5.30 | A.5.1–A.5.14, A.5.18–A.5.22, A.5.29, A.5.31–A.5.37 (organisational — hospital ISMS) |
| A.6 People | A.6.7 (partial) | A.6.1–A.6.6, A.6.8 (hospital HR) |
| A.7 Physical | A.7.7 (partial) | A.7.1–A.7.6, A.7.8–A.7.14 (hospital premises) |
| A.8 Technological | A.8.2, A.8.3, A.8.5, A.8.12, A.8.13, A.8.14, A.8.16, A.8.23 (N/A noted), A.8.24, A.8.25, A.8.28, A.8.29, A.8.31 | A.8.1, A.8.4, A.8.6–A.8.11, A.8.15, A.8.17–A.8.22, A.8.26, A.8.27, A.8.30, A.8.32–A.8.37 (largely N/A for LAN-only desktop HMS; addressed by hospital IT where relevant) |

---

## 6. Continuous improvement

| Item | Action | Owner | Target | Status |
|---|---|---|---|---|
| Audit log immutability | Add DB trigger rejecting UPDATE/DELETE on `audit_logs` | Engineering | Phase 2 | Open (M-08) — universality improved v0.2.0 (M-02) |
| MFA | TOTP for `super_admin`/`billing_clerk` | Engineering | Phase 2 | Open |
| SAST/DAST | Integrate into CI | Engineering | Phase 2 (Batch 5) | Open |
| Dependency scanning | `cargo audit` + `npm audit` in CI | Engineering | Phase 2 (Batch 5) | Open |
| Automated backup | In-product `pg_dump` UI + scheduler | Engineering | Phase 2 | Open |
| DPAPI encryption of `config.json` DB password | Encrypt `db_password` at rest on Windows via DPAPI | Engineering | Batch 5 | Open (R-032) — file ACL hardened v0.2.0 (CR-5) as interim |
| Warm standby | Document restore-to-standby runbook | Engineering | Phase 3 | Open |
| SIEM forwarding | Audit log shipper to hospital SIEM | Engineering | Phase 3 | Open |
| WhatsApp templated messages | Move from free-text to Meta-approved templates | Engineering | Phase 2 | Open (R-013) — currently free-text per SDD-10 |
| Clear-config dead IPC command | Register or delete `clear_config` (currently in `config.rs` but not in `generate_handler![]`) | Engineering | Batch 5 | Open |

---

_End of `04-Security-Control-Matrix-ISO-27001.md`. Cross-reference `05-Risk-Register-ISO-31000.md` for the risk register, `02-SDD-Software-Design.md` §5 for security design, and `07-Licensing-Architecture.md` for the cryptographic license model._

---

## Appendix A — v0.2.0 control-by-control audit closure summary (M-01 .. M-08)

The Phase 1 audit identified 8 control cells in this matrix where the *status* or *evidence* in v0.1.0 was inaccurate relative to the actual implementation. Batches 0–3 remediated most of them. The closure record:

| # | Control | v0.1.0 claim | v0.2.0 reality | Batch(es) |
|---|---|---|---|---|
| **M-01** | A.5.15 / A.8.3 (RBAC universality) | Implemented — false; 7 commands lacked the gate | Implemented — all protected Tauri commands now enforce RBAC | 1, 3 |
| **M-02** | A.8.16 (audit universality) | Implemented — false; 10 commands wrote no audit row | Implemented — all state-changing commands now write audit rows | 1, 3 |
| **M-03** | A.8.25 (TS strict gate) | Implemented — false; no `tsconfig.json` existed | Implemented — `tsconfig.json` strict + ESLint 9 + Prettier 3 are the build gate | 0 |
| **M-04** | A.8.24 (OsRng for keys) | Implemented — inaccurate; pairing used `thread_rng()` | Implemented — all security-critical randomness uses `OsRng` (session tokens, salts, bootstrap password, pairing codes) | 3 |
| **M-05** | A.8.12 (no PHI in logs) | Partial — under-documented | Improved — log access RBAC-gated; sensitive patterns redacted; failed-login usernames masked | 3 |
| **M-06** | A.5.23 (TLS + cert pinning) | Implemented — accurate | Implemented + CSP enforced (`tauri.conf.json`) + HMAC-SHA256 on LAN discovery broadcast | 1, 3 |
| **M-07** | A.5.17 / A.8.5 (bootstrap admin) | Implemented — accurate re: `must_change_password` | Improved — random 24-char CSPRNG bootstrap password + ACL-protected `bootstrap-credentials.txt`; hardcoded `ChangeMe123!` removed; Login UI no longer displays credentials | 1 |
| **M-08** | A.8.3 / A.5.28 (audit append-only) | Partial — Phase 2 trigger planned | Partial — still Planned Phase 2; v0.2.0 audit universality improved (M-02) | — (Phase 2) |
