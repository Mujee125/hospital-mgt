# VitalFlow HMS — Software Design Description (SDD)

> **Document version: v0.2.0 — updated 2025-07-08 after Phase 2 Batches 0-3 implementation. See CHANGELOG.md for details.**

| Field | Value |
|---|---|
| **Document title** | VitalFlow HMS — Software Design Description |
| **Standard** | IEEE 1016-2009 (Standard for Information Technology — Systems Design — Software Design Descriptions) |
| **Version** | 0.2.0 |
| **Date** | 2025-07-08 |
| **Status** | Draft |
| **Classification** | Internal |
| **Owner** | VitalFlow HMS Engineering |
| **Author** | Documentation Specialist (Task 7) — reconciled by Documentation Team (B4-A) v0.2.0 |
| **Related documents** | `01-SRS-Software-Requirements.md`, `03-Quality-Model-ISO-25010.md`, `04-Security-Control-Matrix-ISO-27001.md`, `06-SDLC-ISO-12207.md`, `07-Licensing-Architecture.md`, `08-Deployment-Installation-Guide.md` |

### Revision history

| Version | Date | Author | Summary |
|---|---|---|---|
| 0.1.0 | 2026-07-02 | Documentation Specialist | Initial SDD baseline against the Phase 1 source tree. |
| 0.2.0 | 2025-07-08 | Documentation Team (B4-A) | Reconciled with Phase 2 Batches 0-3: SDD-01 (config RBAC), SDD-02 (messaging RBAC + audit + new perms), SDD-03 (IPD atomic — now matches code), SDD-04 (COMPANY_PUBLIC_KEY is real dev keypair, not all-zeros), SDD-05 (added `fingerprint` + `models` modules), SDD-06 (added `whatsapp_config` table), SDD-07 (lab_orders.patient_id RESTRICT), SDD-08 (appointments FKs RESTRICT + patients soft-delete), SDD-09 (consent commands now exist), SDD-10 (WhatsApp is free-text, not templated), SDD-11 (clear_config dead code noted), SDD-12 (pairing OsRng); added `inventory_movements` table; added `appointment_tz` column + scheduler timezone fix; added cooperative-shutdown flags for the 3 background tasks. |

---

## 1. Introduction

### 1.1 Purpose

This SDD documents the architectural and detailed design of VitalFlow HMS as implemented in the working tree at `/home/z/my-project/hospital-mgmt`. It is written to IEEE 1016-2009's recommended structure and is grounded in the actual source files; where design intent differs from the eventual implementation, the discrepancy is called out explicitly. It supports the SRS (`01-SRS-Software-Requirements.md`) and is consumed by the SDLC (`06-SDLC-ISO-12207.md`) and Quality Model (`03-Quality-Model-ISO-25010.md`) documents.

### 1.2 Scope

The design covers the Rust backend (`src-tauri/src/`), the React frontend (`src/`), the PostgreSQL schema (`db.rs`), the NSIS installer (`windows/hooks.nsh`), and the two-build (server/client) deployment model. Phase 1 (implemented) is described as such; Phase 2 (planned) design is sketched at the module level only.

### 1.3 Definitions

See `01-SRS-Software-Requirements.md` §2.3 for the canonical glossary. SDD-specific terms:

| Term | Definition |
|---|---|
| **AppState** | Tauri-managed application state: `Arc<Mutex<Option<Role>>>`, `PairingService`, `Arc<Mutex<Option<rbac::Session>>>`, `PgPool` |
| **Command** | A Rust function annotated `#[tauri::command]` and registered in `lib.rs::run()` via `generate_handler!` |
| **SessionState** | `Arc<Mutex<Option<rbac::Session>>>` — the type alias declared in `rbac.rs` and managed in AppState |

### 1.4 References

See `01-SRS-Software-Requirements.md` §2.4. IEEE 1016-2009 is the structural standard for this document; ISO/IEC/IEEE 42010 (architecture description) terminology is used where it improves clarity.

---

## 2. System architecture

### 2.1 Architectural style

VitalFlow HMS is a **two-tier desktop application with a host-local (or LAN-adjacent) relational database**, wrapped in a Tauri v2 shell. The architectural style is:

- **Backend**: a Rust monolith inside the Tauri binary, organised into modules per responsibility. The backend owns the PostgreSQL pool, the RBAC session, the audit pipeline, the licensing verifier, and the pairing/TLS provisioning. There is no separate daemon; the Tauri process is the only process (besides the bundled PostgreSQL service on the server-build).
- **Frontend**: a single-page React 19 application communicating with the backend solely through Tauri's `invoke()` IPC bridge. There is no HTTP server.
- **Data**: PostgreSQL is the only persistent store. Schema lives in `db.rs::run_migrations` (idempotent, additive migrations executed at every boot). No external caches, queues, or object stores.
- **Deployment**: one server-build PC + N client-build PCs on the same LAN, with TLS-pinned connections from clients to the server.

### 2.2 Architectural view — boot flow

The following ASCII diagram shows the boot sequence in `src/App.tsx` and `src-tauri/src/lib.rs`. Phases are Tauri-frontend phases; arrows show control flow, not data.

```
                       ┌──────────────────────────────┐
                       │ main.tsx                     │
                       │  QueryClientProvider         │
                       │   └── HashRouter             │
                       │        └── <App/>            │
                       └─────────────┬────────────────┘
                                     │
                                     v
                       ┌──────────────────────────────┐
                       │ App.tsx — phase=checkingSetup│
                       │  invoke("get_config")        │
                       └─────────────┬────────────────┘
                                     │
                  client-build && !setup_complete?
                       │                           │
                       v yes                       v no
            ┌────────────────────┐   ┌─────────────────────────────────┐
            │ phase=needsSetup   │   │ phase=verifyingLicense          │
            │ <Setup/>           │   │  invoke("verify_license")       │
            │  (pairing UI)      │   │  ── DB-FREE pre-boot gate ──    │
            └─────────┬──────────┘   └─────────────┬───────────────────┘
                      │                            │ fail
                      │ complete_pairing_and_connect            v
                      │                            ┌──────────────────────────┐
                      v                            │ phase=licenseError       │
            ┌────────────────────┐                 │  <License required/>     │
            │ phase=verifyingLicense                │  retry → back to verify  │
            └────────────────────┘                 └──────────────────────────┘
                      │ ok
                      v
            ┌────────────────────────────────────────────────────────────────┐
            │ phase=booting                                                   │
            │  invoke("initialize_database")                                  │
            │    ┌──────────────────────────────────────────────────────────┐ │
            │    │ src-tauri/src/lib.rs::initialize_database                │ │
            │    │   if cfg(server-build)  → initialize_as_server            │ │
            │    │   if cfg(client-build)  → initialize_as_client            │ │
            │    │   else                   → initialize_as_server_fallback  │ │
            │    │                                                          │ │
            │    │   resolve AppConfig (ProgramData\HMS\config.json)         │ │
            │    │   materialise pinned cert → sslrootcert path              │ │
            │    │   db::initialize(host,port,user,pw,db,ssl) → PgPool       │ │
            │    │   db::run_migrations(pool)  ← idempotent                  │ │
            │    │   auth::seed_defaults(pool) (roles, perms, admin)         │ │
            │    │   if Role::Server: scheduler::start_scheduler(...)        │ │
            │    │                                                          │ │
            │    │   emit("init_status", "Ready!")                           │ │
            │    │   return "server:<ip>" | "client:<ip>"                    │ │
            │    └──────────────────────────────────────────────────────────┘ │
            └────────────────────────────────┬───────────────────────────────┘
                                             │ ok
                                             v
                       ┌──────────────────────────────┐
                       │ phase=ready                   │
                       │  <AuthProvider><AuthGate/>   │
                       └─────────────┬────────────────┘
                                     │
                ┌────────────────────┼─────────────────────┐
                │                    │                     │
                v no session         v must_change_pw      v ok
        ┌────────────────┐  ┌────────────────────┐  ┌─────────────────────┐
        │ <Login/>       │  │ <ForceChangePwd/>  │  │ <AppShell>          │
        └────────────────┘  └────────────────────┘  │   <Sidebar/>        │
                                                      │   <Header/>         │
                                                      │   <Routes/>         │
                                                      └─────────────────────┘
```

### 2.3 Architectural view — license verification flow

The license verification flow is the security precondition for opening the database. It is intentionally DB-free: it cannot depend on the pool it gates.

```
                       ┌──────────────────────────────────────┐
                       │ App.tsx phase=verifyingLicense       │
                       │   invoke("verify_license")           │
                       └─────────────────┬────────────────────┘
                                         │
                                         v
        ┌──────────────────────────────────────────────────────────────────┐
        │ license.rs::verify_license(app_handle)                          │
        │   path = license_file_path() → C:\ProgramData\HMS\license.json  │
        │   info  = verify_license_file(path)?                            │
        └─────────────────┬────────────────────────────────────────────────┘
                          │
                          v
        ┌──────────────────────────────────────────────────────────────────┐
        │ verify_license_file(path)                                        │
        │                                                                  │
        │  1. read file → String                                           │
        │  2. serde_json::from_str → LicenseFile                           │
        │  3. license.verify_signature():                                  │
        │       decode base64 → 64-byte sig                                │
        │       VerifyingKey::from_bytes(COMPANY_PUBLIC_KEY)               │
        │       vk.verify(canonical_bytes(), sig)                          │
        │       ── FAIL → Err("forged/corrupted")                          │
        │  4. compute_hardware_fingerprint() (WMI on Windows)              │
        │  5. fingerprint_matches = license.hardware_fingerprint == actual │
        │  6. if expiration_date < now → status = "expired"                │
        │  7. if !fingerprint_matches && status == "valid"                 │
        │        → status = "fingerprint_mismatch"                         │
        │  8. return LicenseInfo                                           │
        └─────────────────┬────────────────────────────────────────────────┘
                          │
                          v
        ┌──────────────────────────────────────────────────────────────────┐
        │ verify_license (cont.)                                           │
        │   if info.status != "valid"                                      │
        │     return Err(match status {                                    │
        │       "expired"                => "license expired",             │
        │       "fingerprint_mismatch"   => "bound to a different PC",     │
        │       _                        => "verification failed",         │
        │     })                                                           │
        │   else return Ok(info)                                           │
        └─────────────────┬────────────────────────────────────────────────┘
                          │
              ok          │           err
              ┌───────────┘           └─────────────┐
              v                                     v
        bootApp(cfg)                      phase=licenseError
        (open DB pool)                    (block boot; show retry)
```

Rejection-case matrix:

| Condition | Detected at | Result |
|---|---|---|
| File missing | `verify_license_file` step 1 | `Err("License file not readable")` → licenseError |
| JSON malformed | step 2 | `Err("not valid JSON")` → licenseError |
| Signature invalid / forged / corrupted | step 3 | `Err("signature verification FAILED")` → licenseError |
| ~~Public key all-zeros (placeholder)~~ | step 3 | **[Updated v0.2.0]** No longer applicable — Batch 2 (CR-20) replaced the all-zeros placeholder with a real development Ed25519 keypair generated by the new `keygen/` project. The embedded `COMPANY_PUBLIC_KEY` (32 bytes, `license.rs:56-60`) is `0x09, 0xbb, 0xa3, 0x04, …` and accepts signatures from the matching dev private key in `src-tauri/src/bin/dev_auto_license.rs`. Production deployments MUST replace the dev keypair with a production keypair before ship; the dev private key MUST NOT ship to production. |
| Hardware fingerprint mismatch | step 5 | status `fingerprint_mismatch` → licenseError |
| Hard expiry in the past | step 6 | status `expired` → licenseError |
| All checks pass | — | `Ok(LicenseInfo)` → bootApp |

### 2.4 Architectural view — request handling

Every frontend action ultimately calls a Tauri command. The uniform guard pattern is:

```rust
#[tauri::command]
pub async fn some_write(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    request: SomeRequest,
) -> Result<SomeResponse, String> {
    let session = rbac::require(&session, Permission::SomeResourceSomeAction)?;
    // ... business logic, parameterised SQL via sqlx::query ...
    audit::for_session(pool.inner(), &session, "some_action", "some_resource",
        Some(&id.to_string()), Some(serde_json::json!({"key": "value"}))).await;
    Ok(response)
}
```

This three-step pattern (RBAC → SQL → audit) is the spine of every state-changing command in `commands/`.

---

## 3. Component decomposition

### 3.1 Rust backend modules

| Module | File(s) | Responsibility | Phase |
|---|---|---|---|
| `config` | `src-tauri/src/config.rs` | Load/save `%ProgramData%\HMS\config.json`; resolve machine-wide vs per-user paths; materialise pinned cert. **[Updated v0.2.0 — SDD-01 / CR-4]** `get_config` now requires `SettingsManage` once `setup_complete` is true (during first-run setup it remains open so the wizard can read pairing state); `db_password` is `#[serde(skip_serializing)]` so it is never returned to the frontend. `clear_config` is implemented in this module but is NOT registered in `tauri::generate_handler![]` — see SDD-11. | 1 |
| `db` | `src-tauri/src/db.rs` | Build Postgres URL with correct `sslmode`; connect root pool; `ensure_database`; `run_migrations`; `initialize` | 1 |
| `discovery` | `src-tauri/src/discovery.rs` | LAN broadcast/listen; `local_lan_ip()`; `is_reachable(host, port)`; `detect_server()`. **[Updated v0.2.0 — Batch 3 SEC-08]** Broadcast payload now carries an HMAC tag derived from a deployment-shared secret so a rogue device on the LAN cannot forge a server-presence beacon. The broadcast loop also takes an `Arc<AtomicBool>` running flag and shuts down cooperatively on `RunEvent::ExitRequested` (REL-03). | 1 |
| `pairing` | `src-tauri/src/pairing.rs` | `PairingService` (in-memory code store); `generate_pairing_code`, `redeem_pairing_code`, `verify_pairing`; TLS-protected TCP listener. **[Updated v0.2.0 — SDD-12 / SEC-03]** Pairing code generation now uses `OsRng` (CSPRNG, reads from the OS RNG on every call) instead of `thread_rng()` (userspace PRNG seeded once) so an attacker who has observed prior codes cannot predict future ones. The listener also takes an `Arc<AtomicBool>` running flag (REL-03) and shuts down cooperatively on `RunEvent::ExitRequested`. Pairing codes are rate-limited and burnt after `MAX_REDEEM_ATTEMPTS` failed verifications. | 1 |
| `tls_provision` | `src-tauri/src/tls_provision.rs` | Generate self-signed cert for the server's LAN IP via `rcgen`; persist to `%ProgramData%\HMS\tls\`; expose SHA-256 fingerprint | 1 |
| `pg_provision` | `src-tauri/src/pg_provision.rs` (server-build only) | Health-check the bundled PostgreSQL Windows Service; first-time SSL enablement; SSL repair; `pg_hba.conf` enforcement | 1 |
| `scheduler` | `src-tauri/src/scheduler.rs` | Background tokio task for WhatsApp reminders and other periodic jobs (server-build only). **[Updated v0.2.0]** (1) Timezone fix — the appointment-due query now uses `AT TIME ZONE COALESCE(a.appointment_tz, 'Asia/Karachi')` instead of the previous `AT TIME ZONE 'UTC'`, so reminders fire at the wall-clock time the receptionist entered (clinic is Asia/Karachi, UTC+5). The new `appointments.appointment_tz TEXT DEFAULT 'Asia/Karachi'` column backs this. (2) Cooperative shutdown — the scheduler loop takes an `Arc<AtomicBool>` running flag from `ShutdownFlags` and exits within ~5 s of `RunEvent::ExitRequested` (Batch 2 REL-03). | 1 |
| `messaging` | `src-tauri/src/messaging.rs` | In-app chat (`messages` table, 3 fixed rooms); emits `new_message` Tauri event. **[Updated v0.2.0 — SDD-02 / CR-16]** All 4 messaging commands (`send_message`, `get_messages`, `delete_message`, `get_rooms`) now enforce RBAC via two new permissions: `MessagingView` (read) and `MessagingSend` (write). The message `sender` is derived from the authenticated session (`session.user_id` + `session.full_name`), NOT from a client-supplied string. `send_message` and `delete_message` write audit rows via `audit::for_session`. | 1 |
| `whatsapp` | `src-tauri/src/whatsapp/{mod,commands,automation,templates,log}.rs` | WhatsApp Cloud API integration (server-build only), notification log. **[Updated v0.2.0 — SDD-10 / CR-9 / CR-12 / IPC-09]** (1) Sends use **free-text messages**, NOT Meta-approved templates. Template-based sending (per Meta's pre-approval process) is Planned Phase 2. (2) `whatsapp_config` is now a singleton (id=1, `CHECK(id=1)`, `UNIQUE(id)`) — the upsert path collapses legacy duplicate rows. (3) `whatsapp/automation.rs` now refuses to send to a patient with revoked or missing `whatsapp` consent (CR-12). (4) `send_whatsapp_to_patient` validates patient existence + message length ≤ 4096 chars before sending (IPC-09). (5) `templates.rs` exists but contains helpers for message formatting, not Meta-template dispatch. | 1 |
| `auth` | `src-tauri/src/auth.rs` | Argon2id hashing; login/logout/me/change_password; user CRUD; `seed_defaults` (roles, permissions, bootstrap admin) | 1 |
| `rbac` | `src-tauri/src/rbac.rs` | `Permission` enum (**37 keys** as of v0.2.0 — 35 original + `MessagingView` + `MessagingSend` added in Batch 1 CR-16); `Session`; `require`/`require_session` guards; `permissions_for_role` (8 personas). The 8 personas are: `super_admin`, `doctor`, `nurse`, `receptionist`, `lab_technician`, `pharmacist`, `billing_clerk`, `patient`. | 1 |
| `audit` | `src-tauri/src/audit.rs` | `record`/`for_session` helpers; `get_audit_logs` query command | 1 |
| `license` | `src-tauri/src/license.rs` | `LicenseFile` struct; canonical bytes; signature verification; `verify_license` (DB-free); `install_license`; `get_license_info`; `get_hardware_fingerprint`; `get_license_public_key_fingerprint`; `get_install_fingerprint`. **[Updated v0.2.0]** Added `revoke_license` (Batch 3 LIC-DOC-04) — deletes the on-disk `license.json`, clears in-memory `LicenseInfo`, writes an audit row, and forces the boot flow back to the license-gate phase on the next `verify_license` call. Gated by `Permission::LicenseManage`. Added 7-day grace period (LIC-DOC-07) so an expired license does not cause a hard outage at midnight — the UI surfaces a renewal warning during grace. License transfer = revoke + re-install on the new machine (LIC-DOC-08). The embedded `COMPANY_PUBLIC_KEY` is a real 32-byte development keypair (not all-zeros) generated by the `keygen/` project (see SDD-04 / CR-20). | 1 |
| `commands/dashboard` | `src-tauri/src/commands/dashboard.rs` | `get_dashboard_kpis` — single server-side aggregation query | 1 |
| `commands/patients` | `src-tauri/src/commands/patients.rs` | Patient CRUD with EHR fields; consent commands; RBAC + audit. **[Updated v0.2.0 — SDD-09 / CR-12 / CR-11]** Added 3 consent commands: `get_patient_consent`, `set_patient_consent`, `revoke_patient_consent` (gated by `Permission::PatientConsentManage`, all audited). `delete_patient` is now a soft-delete: sets `deleted_at = NOW(), is_active = FALSE` instead of hard-deleting (Batch 2 CR-11, for HIPAA §164.530(j) 6-year PHI retention). Patient list/detail queries filter `deleted_at IS NULL` so soft-deleted patients don't appear in normal workflows but remain queryable for historical/audit purposes. | 1 |
| `commands/doctors` | `src-tauri/src/commands/doctors.rs` | Doctor CRUD; `get_specializations` | 1 |
| `commands/appointments` | `src-tauri/src/commands/appointments.rs` | Appointment CRUD; status update; today's list; stats | 1 |
| `commands/queue` | `src-tauri/src/commands/queue.rs` | `get_queue`, `create_queue_token`, `call_next_token`, `set_token_status` | 1 |
| `commands/ipd` | `src-tauri/src/commands/ipd.rs` | Wards/beds CRUD; `admit_patient` (transactional); `discharge_patient` | 1 |
| `commands/lab` | `src-tauri/src/commands/lab.rs` | Catalog CRUD; orders; `update_lab_result` (auto-completes order) | 1 |
| `commands/billing` | `src-tauri/src/commands/billing.rs` | Bills (server-side total); items; `record_payment` (status rollup); payment history | 1 |
| `commands/encounters` | `src-tauri/src/commands/encounters.rs` | Encounter CRUD; visit_type; linkage to lab orders and bills | 1 |
| `commands/inventory` | `src-tauri/src/commands/inventory.rs` | **[new v0.2.0 — SDD-05 / CR-21]** Inventory item CRUD + `adjust_inventory` (writes both an `audit::for_session` row and an `inventory_movements` row) + `get_inventory_movements` (history query). All commands RBAC-gated by `InventoryView` (read) or `InventoryManage` (write). 6 commands total: `get_inventory_items`, `get_inventory_item`, `create_inventory_item`, `update_inventory_item`, `adjust_inventory`, `get_inventory_movements`. | 1 |
| `fingerprint` | `src-tauri/src/fingerprint.rs` | **[new v0.2.0 — SDD-05]** Hardware fingerprint computation. On Windows: SHA-256 over `b"vitalflow-hms-fp-v1\0" || cpu_id || b"\0" || board_sn || b"\0" || bios_sn` collected via WMI (`Win32_Processor`, `Win32_BaseBoard`, `Win32_BIOS`). Non-Windows fallback: SHA-256 over hostname+OS for dev only (must NOT be used in production). Declared `pub` so the `dev_auto_license` binary can use it. | 1 |
| `models` | `src-tauri/src/models.rs` | **[new v0.2.0 — SDD-05]** Shared DTOs (request/response structs) used by the command modules. Centralises the `Patient`/`PatientEhr`/`Bill`/`Appointment`/`InventoryItem`/`InventoryMovement`/`Consent`/`Message`/etc. shapes so the `commands/*` modules don't each redefine them. | 1 |
| `lib` (entry) | `src-tauri/src/lib.rs` | Tauri builder; AppState wiring; `initialize_database`; `complete_pairing_and_connect`; log helpers; `diagnose_db_error`. **[Updated v0.2.0]** Added `ShutdownFlags` struct (Batch 2 REL-03) holding three `Arc<AtomicBool>` running flags for the broadcast, pairing, and scheduler tasks. The `Builder::run()` call was switched to `Builder::build()?.run(callback)` so the `RunEvent::ExitRequested` handler can flip all three flags to false and the background tasks exit cooperatively within ~5 s instead of being torn down mid-query. | 1 |

### 3.2 React frontend decomposition

| Area | File(s) | Responsibility |
|---|---|---|
| Boot flow | `src/App.tsx` | Phase state machine: `checkingSetup` → `needsSetup` → `verifyingLicense` → `licenseError` / `booting` → `initError` → `ready` (AuthGate → Login / ForceChangePassword / AppShell) |
| Auth context | `src/lib/auth.tsx` | `AuthProvider`; `useAuth()` hook; calls `me` on mount; holds `session` state |
| Shell | `src/components/layout/AppShell.tsx`, `Sidebar.tsx`, `Header.tsx`, `ThemeToggle.tsx` | Persistent layout: collapsible desktop sidebar + mobile drawer, header with profile dropdown, theme toggle |
| Permission gate | `src/components/auth/RequirePermission.tsx` | Declarative `<RequirePermission perm="patients.create">` wrapper |
| RBAC constants | `src/lib/rbac.ts` | Mirror of `Permission::as_str()` keys for frontend filtering |
| Data layer | `src/lib/queries.ts` | All `useQuery`/`useMutation` hooks; centralised `qk` query keys; `onSuccess` invalidation |
| Models | `src/lib/models.ts` | Single source of truth for TS shapes mirroring Rust structs |
| Pages | `src/pages/*.tsx` | Dashboard, Patients, Doctors, Appointments, Queue, IPD, Laboratory, Billing, Messaging, AuditLog, Users, Settings, Setup, Login |
| Forms | `src/components/forms/{Patient,Doctor,Appointment}Form.tsx` | React-Query-mutation-backed forms |
| UI primitives | `src/components/ui/*.tsx` | Button, Card, Dialog, Input, Select, Table, Tabs, Badge, Avatar, DropdownMenu, Label, Separator, Textarea (shadcn-style) |
| Receipt | `src/components/Receipt.tsx` | 80mm thermal-print CSS via `@media print` |
| Styling | `src/index.css` | Tailwind v4 `@theme inline` tokens (light/dark), status colours, surface utilities, accessibility focus rings |

### 3.3 Installer / provisioning decomposition

| Component | File | Responsibility |
|---|---|---|
| Server installer config | `src-tauri/tauri.server.conf.json` | `installMode: perMachine`; bundles `resources/pgsql`; `installerHooks` → `windows/hooks.nsh` |
| Client installer config | `src-tauri/tauri.client.conf.json` | No Postgres resources; no elevation; distinct product name/identifier |
| Post-install hook | `src-tauri/windows/hooks.nsh` | `NSIS_HOOK_POSTINSTALL`: create `C:\ProgramData\HMS` + ACL; provision PostgreSQL (initdb, scram-sha-256, pg_hba LAN scoping, Windows Service registration, start); write `config.json`. `NSIS_HOOK_PREUNINSTALL`: stop (do not delete) the service |
| Tauri capabilities | `src-tauri/capabilities/default.json` | IPC capability grants for the frontend |

---

## 4. Data design

### 4.1 Schema overview

All schema is defined in `db.rs::run_migrations` as idempotent `CREATE TABLE IF NOT EXISTS` / `ADD COLUMN IF NOT EXISTS` statements. The migration is re-run on every boot. The schema is grouped logically; the table list below is exhaustive (Phase 1).

| # | Table | Group | Purpose |
|---|---|---|---|
| 1 | `patients` | Patient EHR | Patient registry with EHR columns (MRN, blood_group, allergies, chronic_conditions, emergency_contact, insurance, status, created_by_user_id). **[Updated v0.2.0 — CR-11]** Added `deleted_at TIMESTAMPTZ` (NULL = active, non-NULL = soft-deleted) and `is_active BOOLEAN NOT NULL DEFAULT TRUE` (mirrors `deleted_at IS NULL` for easy boolean filtering). Patient deletion is now soft-delete; clinical FKs are `ON DELETE RESTRICT` for HIPAA §164.530(j) 6-year PHI retention. |
| 2 | `patient_consent` | Patient EHR | Per-patient consent flags (consent_type, granted, granted_by_user_id) |
| 3 | `encounters` | Patient EHR | Visit records (visit_type, chief_complaint, diagnosis, notes, doctor_id, created_by_user_id) |
| 4 | `doctors` | Clinical | Doctor registry (specialization, qualification, availability window) |
| 5 | `departments` | Clinical | Departments (code, head_doctor_id) |
| 6 | `appointments` | Scheduling | Appointments with status lifecycle; `created_by_user_id`, `queue_token_id`. **[Updated v0.2.0]** Added `appointment_tz TEXT DEFAULT 'Asia/Karachi'` (clinic timezone) — the scheduler uses `AT TIME ZONE COALESCE(a.appointment_tz, 'Asia/Karachi')` instead of `AT TIME ZONE 'UTC'` so reminders fire at the wall-clock time the receptionist entered. Both `appointments.doctor_id` and `appointments.patient_id` FKs are `ON DELETE RESTRICT` (changed in CR-11) so deletion is blocked until the appointment is reassigned/cancelled. |
| 7 | `queue_tokens` | Scheduling | OPD queue tokens with priority, status, issued/called/completed timestamps |
| 8 | `wards` | IPD | Wards (code, floor, gender_restriction) |
| 9 | `beds` | IPD | Beds (unique per ward; status, is_icu, daily_rate) |
| 10 | `ipd_admissions` | IPD | Admissions (patient, ward, bed, doctor, attending_doctor, discharge_date/summary) |
| 11 | `lab_test_catalog` | Laboratory | Test definitions (code, sample_type, normal_range, unit, price) |
| 12 | `lab_orders` | Laboratory | Orders (patient, encounter, ordering doctor/user, status) |
| 13 | `lab_order_tests` | Laboratory | Per-test rows (result_value, abnormal_flag, completed_at/by) |
| 14 | `bills` | Billing | Bills (bill_number unique, bill_type, totals, status) |
| 15 | `bill_items` | Billing | Line items (item_type, qty, unit_price, total, reference_id) |
| 16 | `payments` | Billing | Payments (method, reference_number, received_by_user_id) |
| 17 | `inventory_items` | Inventory | Stock items (sku, category, batch, expiry, reorder_level, unit_cost). **[Updated v0.2.0 — CR-21]** Now has 6 management commands in `commands/inventory.rs`. |
| 17a | `inventory_movements` | Inventory | **[new v0.2.0 — CR-21]** Append-only audit table for every stock change (item_id, quantity_change signed int, reason TEXT, created_by_user_id, created_at). Written by `adjust_inventory` alongside the `audit_logs` row. |
| 18 | `users` | Identity | User accounts (Argon2id hash, must_change_password, lockout fields) |
| 19 | `roles` | Identity | Role names + descriptions (8 seeded) |
| 20 | `permissions` | Identity | Permission keys (35 seeded) |
| 21 | `role_permissions` | Identity | Many-to-many role↔permission |
| 22 | `user_roles` | Identity | Many-to-many user↔role |
| 23 | `sessions` | Identity | Session token hashes (SHA-256), user_id, expires_at |
| 24 | `audit_logs` | Security | Append-only audit trail (user, action, resource, details JSONB) |
| 25 | `messages` | Comms | In-app chat (sender, room, content) — 3 fixed rooms |
| 26 | `whatsapp_notifications` | Comms | Notification log (appointment_id, type, recipient, success) |
| 27 | `settings` | System | Key-value settings store |
| 27a | `whatsapp_config` | Comms | **[new v0.2.0 — SDD-06 / CR-9]** Singleton config row (id=1, `CHECK(id=1)`, `UNIQUE(id)`) for the WhatsApp Cloud API integration: `access_token`, `phone_number_id`, `enabled`, `preferred_method`, `updated_at`. The upsert path in `whatsapp/commands.rs::set_whatsapp_config` collapses legacy duplicate rows to the single most-recent row before applying the new values so the singleton invariant holds. |
| 28 | `license_state` | System | Single-row table (id=1) holding the persisted license JSON + verification status |

### 4.2 Key integrity constraints

| Constraint | Implementation |
|---|---|
| Patient MRN uniqueness | `patients.mrn VARCHAR(20) UNIQUE` |
| Bed uniqueness per ward | `UNIQUE (ward_id, bed_number)` |
| Department code uniqueness | `departments.code VARCHAR(20) NOT NULL UNIQUE` |
| Ward code uniqueness | `wards.code VARCHAR(20) NOT NULL UNIQUE` |
| Lab catalog code uniqueness | `lab_test_catalog.code VARCHAR(30) NOT NULL UNIQUE` |
| Bill number uniqueness | `bills.bill_number VARCHAR(40) NOT NULL UNIQUE` |
| Inventory SKU uniqueness | `inventory_items.sku VARCHAR(40) UNIQUE` |
| Username uniqueness | `users.username VARCHAR(60) NOT NULL UNIQUE` |
| Role name uniqueness | `roles.name VARCHAR(60) NOT NULL UNIQUE` |
| Permission key uniqueness | `permissions.key VARCHAR(80) NOT NULL UNIQUE` |
| Session token hash as PK | `sessions.token_hash TEXT PRIMARY KEY` |
| Settings key as PK | `settings.key TEXT PRIMARY KEY` |
| Bill deletion blocked by payments | `payments.bill_id ... ON DELETE RESTRICT` |
| Patient deletion blocked by IPD/bills | `ipd_admissions.patient_id ... ON DELETE RESTRICT`, `bills.patient_id ... ON DELETE RESTRICT` |
| ~~Patient deletion cascades to appointments/encounters/consent/queue~~ | **[Updated v0.2.0 — CR-11]** Replaced — these 5 clinical `patient_id` FKs (`appointments`, `patient_consent`, `encounters`, `queue_tokens`, `lab_orders`) are now `ON DELETE RESTRICT` for HIPAA §164.530(j) 6-year PHI retention. Patient deletion is soft-delete (`deleted_at` + `is_active=FALSE`); hard-delete is blocked by any clinical FK. |
| Lab order patient deletion blocked | `lab_orders.patient_id ... ON DELETE RESTRICT` (changed in CR-11 v0.2.0) |
| Appointment doctor deletion blocked | `appointments.doctor_id ... ON DELETE RESTRICT` (changed in CR-11 v0.2.0) |
| Appointment patient deletion blocked | `appointments.patient_id ... ON DELETE RESTRICT` (changed in CR-11 v0.2.0) |
| WhatsApp config singleton | `whatsapp_config CHECK(id=1)`, `UNIQUE(id)` (CR-9 v0.2.0) |

### 4.3 Indices

| Index | Table | Columns |
|---|---|---|
| `idx_audit_logs_created_at` | `audit_logs` | `created_at DESC` |
| `idx_audit_logs_user` | `audit_logs` | `user_id, created_at DESC` |
| `idx_queue_status` | `queue_tokens` | `status, issued_at` |

### 4.4 Data flow — write path

```
UI form ──► useMutation (lib/queries.ts) ──► invoke("create_patient", {...})
                                                      │
                                                      v
                            commands/patients.rs::create_patient
                                                      │
                              ┌───────────────────────┼───────────────────────┐
                              │                       │                       │
                              v                       v                       v
                  rbac::require(session,        sqlx::query("INSERT      audit::for_session(
                    Permission::PatientsCreate)    INTO patients ...")       "patient_create",
                                                      bind(...)               "patients",
                                                      execute(pool)           Some(&id),
                                                                              json!({...}))
                              │
                              v
                          Ok(id) ──► returned to frontend
                                              │
                                              v
                          useMutation.onSuccess → queryClient.invalidateQueries(qk.patients)
                                              │
                                              v
                          useQuery('patients') refetches → UI updates
```

### 4.5 Storage locations

| Artifact | Path | Owner | ACL |
|---|---|---|---|
| Configuration | `C:\ProgramData\HMS\config.json` | Installer writes; app reads/writes | `Builtin Users (M)` granted by `icacls` in `hooks.nsh` |
| License file | `C:\ProgramData\HMS\license.json` | Installer/operator drops; app reads; `install_license` writes | Inherits HMS folder ACL |
| PostgreSQL binaries | `C:\ProgramData\HMS\pgsql\bin\` | Installer copies from bundle | Inherited |
| PostgreSQL data | `C:\ProgramData\HMS\pgdata\` | `initdb` (installer) | Locked down by `initdb` defaults |
| TLS material | `C:\ProgramData\HMS\tls\server.crt`, `server.key` | `tls_provision::ensure_tls_material` | Inherited |
| Application log | `%APPDATA%\<bundle-id>\Logs\hms_startup.log` | `lib.rs::log` | Per-user |
| Pairing code (transient) | In-memory in `PairingService` | `pairing.rs` | n/a |

---

## 5. Security design

### 5.1 Authentication flow

1. Frontend `Login.tsx` collects `username` + `password` and calls `invoke("login", { request })`.
2. `auth.rs::login` queries `users` by username. On miss, runs a **dummy Argon2 verify** against a fixed PHC string to flatten timing, then returns `"Invalid username or password."` and audits `login_failed` with `reason=unknown_user`.
3. On hit: checks `is_active`, then `locked_until`. On bad password: increments `failed_login_count`; if ≥ `MAX_FAILED_ATTEMPTS` (5), sets `locked_until = now + 15min`. Audits `login_failed` with `reason=bad_password` and attempts count.
4. On success: resets `failed_login_count` to 0, clears `locked_until`, sets `last_login_at = NOW()`, deletes prior `sessions` rows for this user (single active session), generates a 32-byte random token, hashes it with SHA-256, inserts `sessions` row with 12-hour expiry, loads `Session { user_id, username, full_name, roles, permissions }` into `AppState`, audits `login_success`.
5. Returns `LoginResponse { user, roles, permissions, must_change_password }` to the frontend. The raw token is **never** persisted to disk and **never** sent back to the frontend (it lives only in `AppState`).

### 5.2 RBAC enforcement

- **Canonical source of truth**: `rbac.rs::Permission` enum. Each variant maps to a stable string key (`Permission::as_str`) that is persisted in the `permissions` table and referenced by `role_permissions` and `user_roles`. **[Updated v0.2.0 — CR-16]** Two new variants added: `MessagingView` (`messaging.view`) and `MessagingSend` (`messaging.send`). Total: 37 keys (was 35 in v0.1.0).
- **Roles**: 8 personas seeded by `seed_roles()`: `super_admin`, `doctor`, `nurse`, `receptionist`, `lab_technician`, `pharmacist`, `billing_clerk`, `patient`. Least-privilege grant matrix is in `permissions_for_role()`. `MessagingView` + `MessagingSend` are granted to `super_admin`, `doctor`, `nurse`, `receptionist`, `billing_clerk`, `lab_technician` (i.e. all staff except `patient`).
- **Enforcement**: every protected command begins with `let session = rbac::require(&session_state, Permission::XxxYyy)?;`. The guard returns either the cloned `Session` (for downstream audit) or an `"Access denied: ..."` error string.
- **Frontend mirroring**: `src/lib/rbac.ts` exports the same string keys; `<RequirePermission>` and the permission-filtered sidebar use them to hide UI the user cannot access. Frontend hiding is a UX concern only — every backend command re-checks.
- **Policy updates**: `seed_defaults` re-syncs role→permission grants on every app start, so policy changes in `permissions_for_role()` propagate without code change to the DB. Direct DB edits to `role_permissions` are also honoured (data-driven).

### 5.3 Audit logging

- Every state-changing command writes a single audit row via `audit::for_session(pool, &session, action, resource, resource_id, details)`.
- Read commands are **not** row-level audited (volume would be untenable and would itself leak PHI access patterns). Login/logout and explicit PHI exports are the auditable events.
- Audit insert failures are swallowed (logged to stderr) so a logging fault can never block a clinical operation. This is a deliberate availability-over-completeness trade-off documented in `audit.rs`.
- `get_audit_logs` is RBAC-gated by `audit.view` and supports filtering by action/resource with a 5000-row clamp.

### 5.4 Licensing verification sequence

See §2.3 above and `07-Licensing-Architecture.md` §5 for the full sequence and rejection matrix.

### 5.5 TLS / SSL

- **Server (loopback)**: `sslmode=require`. No cert validation (acceptable for loopback; no network hop). The server self-heals broken SSL config in `pg_hba.conf`/`postgresql.conf` via `pg_provision::repair_ssl_config`.
- **Client (LAN)**: `sslmode=verify-ca` with the pinned server cert materialised to a temp file. The pinned cert is exchanged during pairing via the TLS-protected pairing listener. If the server's cert changes, the client refuses to connect and surfaces a "Re-pair with the server" hint (`lib.rs::diagnose_db_error`).
- **Pairing listener**: rustls-protected TCP listener on the server, using the same self-signed cert. The 6-character pairing code is exchanged for real DB credentials over this channel.
- **Discovery broadcast**: UDP, contains server IP + DB port + **HMAC tag** (Batch 3 SEC-08). The HMAC is keyed by a deployment-shared secret so a rogue device on the LAN cannot forge a server-presence beacon and redirect clients to a malicious PostgreSQL. No credentials are transmitted; the HMAC only authenticates the beacon's authenticity, not confidentiality.

### 5.6 Cryptographic primitives

| Purpose | Primitive | Crate |
|---|---|
| Password hashing | Argon2id (m=19456 KiB, t=2, p=1) | `argon2` 0.5 |
| Session token hashing | SHA-256 | `sha2` 0.10 |
| Hardware fingerprint | SHA-256 | `sha2` 0.10 |
| License signature | Ed25519 | `ed25519-dalek` 2 |
| Base64 codec | URL_SAFE_NO_PAD (tokens) / STANDARD (license sig) | `base64` 0.22 |
| Hex codec | fingerprint + token hash | `hex` 0.4 |
| TLS | rustls 0.23 (ring provider) | `rustls`, `tokio-rustls` |
| Cert generation | rcgen 0.13 | `rcgen` |
| Random bytes | `OsRng` (rand 0.8 + rand_core 0.6) | `rand`, `rand_core` |

**[Updated v0.2.0 — SDD-12 / SEC-03]** Pairing code generation now uses `OsRng` (CSPRNG; reads from the OS RNG on every call) instead of the previous `thread_rng()` (userspace PRNG seeded once at thread start). This means an attacker who has observed prior pairing codes cannot predict future codes even if they have compromised the process memory. `OsRng` is also used for session tokens, the postgres superuser password (24 bytes), and the Argon2id salt. The licensing HMAC for the LAN broadcast payload (SEC-08) uses `Hmac<Sha256>` from the `hmac` crate keyed by a deployment-shared secret.

### 5.7 Background task lifecycle (v0.2.0)

**[new v0.2.0 — REL-03]** Three background tokio tasks run on the server-build only: (1) the LAN broadcast loop (`discovery.rs`), (2) the pairing TCP listener (`pairing.rs`), (3) the WhatsApp reminder scheduler (`scheduler.rs`). Each task takes an `Arc<AtomicBool>` running flag from the `ShutdownFlags` struct managed in Tauri app state. The flags default to `true` at startup.

The Tauri builder was switched from `Builder::run(context)` (no event callback) to `Builder::build(context)?.run(callback)` so a `RunEvent::ExitRequested` handler can flip all three flags to `false`:

```rust
.run(|app, event| {
    if let tauri::RunEvent::ExitRequested { .. } = event {
        if let Some(flags) = app.try_state::<ShutdownFlags>() {
            flags.broadcast.store(false, Ordering::Relaxed);
            flags.pairing.store(false, Ordering::Relaxed);
            flags.scheduler.store(false, Ordering::Relaxed);
        }
    }
});
```

The three task loops check their flag at the top of every iteration (broadcast loop every ~1 s, pairing listener per `accept()` with a short timeout, scheduler every ~60 s) and exit cleanly when it flips to false. This prevents the tasks from being torn down mid-query or mid-TLS-handshake when the pool/socket closes — previously a source of scary `BrokenPipe` log noise on shutdown. The static `BROADCAST_RUNNING` / `PAIRING_LISTENER_STARTED` `AtomicBool` gates remain (they prevent double-start); they are SEPARATE from the `ShutdownFlags` Arcs (which gate cooperative shutdown).

---

## 6. Deployment design

### 6.1 Two-build model

The role of a given PC (server vs client) is fixed at **compile time** via Cargo features:

```toml
[features]
default = []
server-build = []
client-build = []
```

- `cargo build --features server-build` produces the server binary; `tauri.server.conf.json` bundles PostgreSQL and sets `installMode: perMachine`.
- `cargo build --features client-build` produces the client binary; `tauri.client.conf.json` contains no PostgreSQL resources and has a distinct product name/identifier so both can coexist on a test PC.

`lib.rs::initialize_database` branches at compile time:

```rust
#[cfg(feature = "server-build")]
let role = initialize_as_server(&app_handle).await?;

#[cfg(feature = "client-build")]
let role = initialize_as_client(&app_handle).await?;

#[cfg(not(any(feature = "server-build", feature = "client-build")))]
let role = initialize_as_server_fallback(&app_handle).await?;
```

### 6.2 Installer hook (NSIS) — `windows/hooks.nsh`

The hook runs once during installation, while still elevated (`NSIS_HOOK_POSTINSTALL`). Steps:

1. `SetShellVarContext all` so `$APPDATA` resolves to `C:\ProgramData`.
2. `CreateDirectory "$APPDATA\HMS"`; `icacls "$APPDATA\HMS" /grant *S-1-5-32-545:(OI)(CI)M /T` to grant Builtin Users modify rights (locale-independent well-known SID).
3. Skip provisioning if `$APPDATA\HMS\pgdata\PG_VERSION` exists (upgrade safety — never destroys patient data).
4. Copy bundled PostgreSQL binaries from `$INSTDIR\pgsql\` to `$APPDATA\HMS\pgsql\`.
5. Generate a 24-byte random password via a temp PowerShell script using `[System.Security.Cryptography.RandomNumberGenerator]` (NOT NSIS's weak PRNG).
6. Run `initdb` with `--auth=scram-sha-256` and the generated password.
7. Write `pg_hba.conf` scoped to loopback + private LAN ranges only, with `scram-sha-256` auth method.
8. Register PostgreSQL as a Windows Service (`pg_ctl register -S auto`) named `HMS-PostgreSQL`.
9. Start the service.
10. Write `C:\ProgramData\HMS\config.json` with the generated credentials, DB host=127.0.0.1, DB port, DB name=hms, `setup_complete=true`.

`NSIS_HOOK_PREUNINSTALL` stops (does **not** delete) the service so patient data survives a reinstall/upgrade.

### 6.3 ProgramData layout

```
C:\ProgramData\HMS\
├── config.json              ← written by installer, read/written by app
├── license.json             ← dropped by operator / written by install_license
├── pgsql\                   ← bundled PostgreSQL binaries (server-build only)
│   └── bin\
├── pgdata\                  ← PostgreSQL data directory (server-build only)
│   ├── PG_VERSION
│   ├── postgresql.conf
│   ├── pg_hba.conf
│   └── ...
└── tls\
    ├── server.crt           ← self-signed cert for LAN IP (rcgen)
    └── server.key           ← matching private key
```

### 6.4 LAN pairing flow

```
Server PC (server-build)              Client PC (client-build)
─────────────────────────             ─────────────────────────
1. App boots; runs verify_license
   + initialize_as_server.
2. TLS material generated
   (rcgen self-signed for LAN IP).
3. Pairing TCP listener starts
   on pairing::PAIRING_PORT.
4. Receptionist opens
   Settings → "Connect a New Client PC"
   → generate_pairing_code.
   ┌────────────────────────────┐
   │ code "AB12CD" displayed    │
   │ (10-min expiry, capped use)│
   └─────────────┬──────────────┘
                 │ operator reads code aloud / types into client
                 v
                                 5. First-run Setup screen on client:
                                    enter server IP + pairing code.
                                 6. redeem_pairing_code(ip, code)
                                    ── TLS-protected TCP to server ──►
                                    ◄── returns real DB creds + pinned cert PEM ──
                                 7. complete_pairing_and_connect():
                                    verify_pairing (materialise cert,
                                    open real DB connection, set
                                    setup_complete=true).
                                 8. initialize_database() →
                                    client connects to server's PG
                                    with sslmode=verify-ca + pinned cert.
```

### 6.5 Multi-PC topology

```
Hospital LAN (192.168.x.x)
├── Server PC (server-build installer)
│     ├── HMS-PostgreSQL Windows Service (auto-start)
│     ├── Pairing listener (TLS)
│     ├── LAN discovery broadcast
│     └── HMS app (receptionist login)
├── Client PC 1 (client-build installer)  ──► paired via code ──► Server PG (TLS)
├── Client PC 2 (client-build installer)  ──► paired via code ──► Server PG (TLS)
└── Client PC N (client-build installer)  ──► paired via code ──► Server PG (TLS)
```

---

## 7. Design decisions and rationale

| Decision | Chosen | Alternatives considered | Rationale |
|---|---|---|---|
| Routing | `HashRouter` | `BrowserRouter` | Tauri production custom protocol has no SPA fallback; `HashRouter` works without server-side rewrite. Documented in `DESIGN_SYSTEM.md` §2.1. |
| Animation library | `motion` (package name; imported as `motion/react`) | `framer-motion` | `motion` is the actively maintained successor; same API. Using the legacy name would pull a deprecated package. |
| Data layer | `sqlx` (compile-time-checked SQL where possible; raw queries elsewhere) | SeaORM, Diesel | `sqlx` is async-native, pairs cleanly with Tokio + Tauri, and doesn't impose a DSL that fights PostgreSQL-specific features (JSONB, `ON CONFLICT`, `RETURNING`). |
| Single-session model | One active desktop user per process, session held in `AppState` | Per-request token validation (web model) | Desktop HMS = single concurrent user; an in-memory session avoids transmitting tokens on every command and avoids the complexity of token-in-header plumbing through Tauri IPC. |
| License signing | Ed25519 over canonical BTreeMap JSON | RSA, ECDSA | Ed25519 is fast, has small keys (32B public, 64B sig), is deterministic, and is widely supported by `ed25519-dalek`. RSA keys are larger and slower; ECDSA requires careful nonce handling. |
| Canonical signing form | Compact JSON via `BTreeMap<&str, Value>` (sorted keys), excluding `signature` | Full JSON with default field order | BTreeMap enforces sorted keys deterministically; `serde_json::to_vec` produces compact output. Both signer and verifier use identical construction, guaranteeing byte-equivalence. |
| Hardware fingerprint | SHA-256 over CPU ProcessorId + baseboard serial + BIOS serial via WMI | MAC address, disk serial, Windows MachineGuid | CPU+baseboard+BIOS survive OS updates and routine driver changes; MAC changes with NIC swap; disk serial changes on disk replacement; MachineGuid can be reset by sysprep. |
| Build split | Compile-time Cargo features (`server-build`, `client-build`) | Runtime role detection via boot-time race | Compile-time split avoids the runtime race entirely; the binary knows its role. Distinct installers also let both coexist on a test PC. |
| PostgreSQL provisioning | Once, at install time, while NSIS is already elevated | Runtime provisioning by the app (UAC every launch); `pg_embed` (runtime download) | Runtime provisioning would require UAC on every launch (poor UX, larger attack surface). `pg_embed` requires internet on first launch and pins an old crate version. Install-time provisioning avoids both. |
| Bootstrap admin | `admin` / `ChangeMe123!` with `must_change_password=true` | Random one-time password printed by installer | Forced change on first login is simpler and aligns with the "no backdoor" principle. The credential is documented; the installer does not need to display it. |
| Audit failure policy | Swallow + log to stderr | Fail-closed (block the operation) | A logging fault must not block a clinical operation. Availability over completeness for audit is the documented trade-off in `audit.rs`. |
| Frontend data layer | Centralised React Query hooks in `lib/queries.ts` with shared `qk` keys | Per-page `useState`/`useEffect` | Centralisation surfaces silent parameter drift (e.g. dead `search`/`specialization` params the backend never accepted) and gives one source of truth for cache invalidation. |
| License persistence | Single-row table `license_state` (id=1) + on-disk `license.json` | DB-only | The on-disk file is the source of truth (DB-free verification reads it); the DB row mirrors last-known state for the Settings UI without re-running signature verification. |
| ~~`COMPANY_PUBLIC_KEY` placeholder~~ | **[Updated v0.2.0 — CR-20]** Was: all-zeros (rejects every signature) until build-time provisioning. Now: a real **development** Ed25519 keypair committed to the repo, generated by the new `keygen/` project. The dev keypair can only sign `dev=true` licenses, which release builds reject. Production deployments MUST regenerate a separate production keypair with `cargo run --bin gen_keys` in the `keygen/` project, replace `COMPANY_PUBLIC_KEY` in `license.rs`, and destroy the production private key after signing customer licenses. | A real production key committed to the repo | The all-zeros placeholder forced explicit key provisioning but blocked all dev-time license verification. The dev keypair unblocks dev-time verification while still preventing a "works on my machine" trap with a production key. |

---

## 8. Detailed design — selected subsystems

### 8.1 IPD admission transaction (`commands/ipd.rs::admit_patient`)

The bed allocation must be atomic with the admission insert. The pattern:

```rust
let mut tx = pool.begin().await?;
sqlx::query("UPDATE beds SET status = 'occupied' WHERE id = $1 AND status = 'available'")
    .bind(bed_id).execute(&mut *tx).await?;
let row: (i32,) = sqlx::query_as("INSERT INTO ipd_admissions (...) VALUES (...) RETURNING id")
    .bind(...).fetch_one(&mut *tx).await?;
tx.commit().await?;
```

If the bed is no longer available (concurrent admission), the `UPDATE` affects 0 rows; the transaction is rolled back. This is the canonical pattern for any "allocate-then-insert" workflow.

**[Implemented v0.2.0 — Batch 1 CR-8]** `commands/ipd.rs::admit_patient` now follows this exact pattern. The previous revision described the intended pattern; the implementation now matches. The `discharge_patient` command additionally enforces FUN-09 (no discharge if outstanding unpaid bills for the admission) before freeing the bed.

### 8.2 Billing server-side total (`commands/billing.rs::create_bill`)

The client-supplied bill total is **ignored**. The server recomputes:

```rust
let total: Decimal = items.iter().map(|i| i.unit_price * i.quantity).sum();
let net = (total - request.discount + request.tax).max(dec(0));
```

`net_amount` is then persisted alongside the items. This prevents a tampered client payload from inflating or deflating a bill.

### 8.3 Lab order auto-completion (`commands/lab.rs::update_lab_result`)

After inserting/updating a `lab_order_tests` row, the command counts remaining incomplete tests on the parent order:

```rust
let pending: (i64,) = sqlx::query_as(
    "SELECT COUNT(*) FROM lab_order_tests WHERE lab_order_id = $1 AND completed_at IS NULL"
).bind(order_id).fetch_one(pool).await?;
if pending.0 == 0 {
    sqlx::query("UPDATE lab_orders SET status = 'completed' WHERE id = $1")
        .bind(order_id).execute(pool).await?;
}
```

### 8.4 License canonical bytes (`license.rs::LicenseFile::canonical_bytes`)

```rust
let mut map: BTreeMap<&str, serde_json::Value> = BTreeMap::new();
map.insert("license_id", serde_json::json!(self.license_id));
// ... all non-signature fields ...
serde_json::to_vec(&map).expect("canonical serialization is infallible")
```

`BTreeMap` guarantees sorted keys; `serde_json::to_vec` produces compact JSON (no whitespace). The signer (offline, in the software company's issuing tool) and the verifier (in-app) use byte-identical construction.

---

## 9. Traceability to requirements

| Design element | Requirements covered | Reference |
|---|---|---|
| `auth.rs` (Argon2id, lockout, single session, SHA-256 token hash) | FR-0020–FR-0029, NFR-10–NFR-13 | §5.1 |
| `rbac.rs` (Permission enum, require guard, 8 roles) | FR-0020–FR-0029, NFR-15 | §5.2 |
| `audit.rs` (record + get_audit_logs) | FR-0022, NFR-14, FR-0239 | §5.3 |
| `license.rs` (DB-free verify, Ed25519, canonical bytes) | FR-0240–FR-0253, NFR-16 | §2.3, §5.4 |
| `db.rs` (sslmode=verify-ca on client, sslmode=require on server) | NFR-17, NFR-18 | §5.5 |
| `pg_provision.rs` + `windows/hooks.nsh` (initdb, scram-sha-256, pg_hba, Windows Service) | NFR-18, NFR-19, FR-0230, C-06, C-07 | §6.2 |
| `commands/ipd.rs::admit_patient` (transactional bed allocation) | FR-0062 | §8.1 |
| `commands/billing.rs::create_bill` (server-side total) | FR-0151 | §8.2 |
| `commands/lab.rs::update_lab_result` (auto-completion) | FR-0132 | §8.3 |
| `pairing.rs` + `tls_provision.rs` (TLS-protected pairing, cert pinning) | NFR-17, FR-0233 | §6.4 |
| `App.tsx` boot state machine (license gate before DB) | FR-0238, FR-0245 | §2.2 |
| `config.rs` (ProgramData resolution) | FR-0230, NFR-63 | §6.3 |

---

## 10. Open design issues

| Issue | Status | Owner |
|---|---|---|
| ~~`COMPANY_PUBLIC_KEY` is all-zeros placeholder — must be provisioned at build time~~ | **[Updated v0.2.0 — CR-20]** Resolved — replaced with a real development Ed25519 keypair generated by the new `keygen/` project. Production deployments MUST still swap the dev keypair for a production keypair before ship. The dev private key in `src-tauri/src/bin/dev_auto_license.rs` MUST NOT ship to production. | Software company |
| `clear_config` is implemented in `config.rs` but NOT registered in `tauri::generate_handler![]` — dead code that callers cannot invoke (SDD-11) | Open — Planned for Batch 5 cleanup | Engineering |
| No automated backup UI in Settings — operator must run `pg_dump` manually | Open | Engineering (Phase 2) |
| `service.rs` exists in source tree but is not declared as a module — confirmed dead code | Open | Engineering cleanup |
| `cargo test` suite not yet implemented | Open | Engineering (Phase 2) |
| Phase 2 module designs (Nurses, Pharmacy, Radiology, Blood Bank, HR, Payroll, Reports) are not yet designed | Planned | Engineering |
| WhatsApp sends are free-text, not Meta-approved templates (SDD-10) | Open — Planned Phase 2 (requires Meta template approval workflow) | Engineering |
| Select-based form fields in Billing/Queue/IPD/Users/Laboratory lack `htmlFor` association (Radix Select is a button, different pattern from Input) | Open — Planned Batch 5 a11y | Engineering |

---

_End of `02-SDD-Software-Design.md`. Cross-reference `01-SRS-Software-Requirements.md` for the requirements baseline, `07-Licensing-Architecture.md` for licensing detail, and `08-Deployment-Installation-Guide.md` for installation procedures._
