# Changelog — VitalFlow HMS

All notable changes to the VitalFlow Hospital Management System are documented here. Dates are in Asia/Karachi timezone (UTC+5).

This changelog is the canonical entry point for understanding what changed between releases. For full engineering detail, see the per-batch entries in `/home/z/my-project/worklog.md` (project root). For per-document revision detail, see the "Revision history" subsection at the top of each document in `/docs`.

---

## v0.2.0 — 2025-07-08 (Phase 2 Batches 0-3 + Batch 4 documentation reconciliation)

This release closes 50+ findings from the Phase 1 enterprise audit (RCTF prompt). The codebase is now considered feature-complete for Phase 1, with security hardening, accessibility improvements, and full documentation reconciliation in place.

### Batch 0 — Build Unblock (CR-1)

The Phase 1 audit found that the project did not actually build — `tsconfig.json`, `vite.config.ts`, ESLint, and Prettier configs were missing or stubbed. Batch 0 restored the toolchain so `npm run build` and `cargo check` both succeed.

- **CR-1 (partial):** Restored `tsconfig.json` with strict mode per SRS NFR-50 / Security Matrix A.8.25. Strict flags enabled: `strict`, `noUnusedLocals`, `noUnusedParameters`, `noUncheckedIndexedAccess`, `noImplicitOverride`, `noImplicitReturns`, `noFallthroughCasesInSwitch`, `forceConsistentCasingInFileNames`, `skipLibCheck`.
- Created `vite.config.ts` with the `@/*` path alias + `@vitejs/plugin-react` + `@tailwindcss/vite` plugins.
- Created ESLint 9 flat config (`eslint.config.js`) with TypeScript + React + React Hooks + Refresh rulesets.
- Created Prettier config (`.prettierrc.json`).
- Added `typecheck` / `lint` / `format` / `format:check` npm scripts.
- Fixed 15 strict-mode TypeScript errors (mostly `noUncheckedIndexedAccess` indexing fixes + null checks).
- Fixed 3 missing image imports in `Login.tsx` / `Setup.tsx` / `App.tsx` (Vite was failing to resolve the assets).
- Verified `npm run typecheck` reports 0 errors and `npm run lint` reports 0 errors after the fixes.

### Batch 1 — Critical Security & Patient Safety (12 fixes)

- **CR-2:** Random bootstrap admin password. The hardcoded `admin / ChangeMe123!` pair is removed. The app now generates a 24-character CSPRNG password (alphabet excludes `0/O/1/l/I` for transcription safety) on first DB init and writes it to `C:\ProgramData\HMS\bootstrap-credentials.txt` (ACL-restricted to `SYSTEM` + `Administrators`). The installing admin reads the file, logs in, and is forced to change the password.
- **CR-3:** Strict Content Security Policy (CSP) in `tauri.conf.json`. Was `"csp": null` (disabled); now: `default-src 'self' ipc: http://ipc.localhost; img-src 'self' data: blob: https:; font-src 'self' https://fonts.gstatic.com; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; script-src 'self'; connect-src 'self' ipc: http://ipc.localhost`.
- **CR-4:** RBAC on config commands. `get_config` now requires `SettingsManage` (was unauthenticated). `db_password` is tagged `#[serde(skip_serializing)]` on the `Config` struct so a `get_config` IPC call never returns the password to the frontend. `save_config` / `repair_server_config` / `clear_config` all require `SettingsManage`.
- **CR-5:** `config.json` ACL hardening. `Config::save` runs `icacls "<path>" /inheritance:r /grant:r SYSTEM:F /grant:r Administrators:F` after every atomic write. The file is now readable only by `SYSTEM` + `Administrators` (was: `Builtin Users` modify).
- **CR-6:** Queue token race condition. `call_next` now uses an atomic `SELECT ... FOR UPDATE` on the queue row to prevent two receptionists from issuing the same token.
- **CR-7:** IPD admit race condition. `admit_patient` now atomically claims a bed (row-level lock + conditional update) so two admissions can't land on the same bed.
- **CR-8:** `complete_pairing_and_connect` race. Pairing redemption now holds the `PairingService` mutex across the consume + credential-return boundary.
- **CR-9:** `whatsapp_config` upsert fix. The config table is now a singleton (id=1, CHECK constraint, UNIQUE on id); `set_whatsapp_config` is a proper upsert instead of a duplicate-row insert.
- **CR-10:** Scheduler timezone fix. Reminders now compute `AT TIME ZONE COALESCE(a.appointment_tz, 'Asia/Karachi')` so a patient in Karachi with a 09:00 appointment gets the reminder at the right local time (was: server-local time, which on a UTC-set server would fire 5 hours early).
- **CR-12:** Patient consent commands + WhatsApp consent gate. Three new Tauri commands (`set_patient_consent`, `revoke_patient_consent`, `get_patient_consent`); WhatsApp send now refuses if the patient has not consented (`WhatsApp consent not granted for this patient`).
- **CR-16:** RBAC on messaging commands. All 4 messaging commands (`send_message`, `get_messages`, `get_message_thread`, `delete_message`) now require `MessagingView` (read) or `MessagingSend` (write). Sender is derived from the session, not the request body. Audit rows are written for send + delete.
- **CR-21:** Inventory commands. Six new Tauri commands: `list_inventory_items`, `get_inventory_item`, `create_inventory_item`, `update_inventory_item`, `adjust_inventory`, `delete_inventory_item`. The `adjust_inventory` command writes both an `inventory_movements` row and an `audit_logs` row.
- **CR-22:** `pg_hba.conf` LAN `hostssl`. The installer's `hooks.nsh` now writes `host` for loopback (so the app can bootstrap before TLS exists) and `hostssl` for LAN clients (so no DB traffic ever crosses the network in plaintext). Was inconsistent — installer wrote `host`, spec said `hostssl`.

### Batch 2 — Critical UX, Reliability & Licensing (9 fixes)

- **CR-11:** Patient soft-delete + clinical FK `ON DELETE RESTRICT`. Patients are now soft-deleted (`deleted_at` timestamp + `is_active = false`) instead of hard-deleted, satisfying HIPAA §164.530(j) data-retention requirements. Foreign keys on `lab_orders.patient_id`, `appointments.patient_id`, `appointments.doctor_id`, `patient_consent.patient_id`, `queue_tokens.patient_id` are changed from `CASCADE` to `RESTRICT` so a deletion cannot cascade-destroy clinical history. Hard-delete is refused if any dependent row exists.
- **CR-13:** Tailwind v4 `@theme inline` token registration. The `:root` CSS variables for `--info`, `--status-scheduled/confirmed/completed/cancelled/no-show`, `--success-foreground`, `--warning-foreground` were NOT registered in the `@theme inline` block, so Tailwind v4 did not generate the corresponding utilities — Badge variants using these tokens silently rendered unstyled. All tokens are now registered.
- **CR-14:** Top-level React Error Boundary. The previous implementation had no top-level error boundary, so an uncaught render error would leave the user with a blank white screen. The new `ErrorBoundary` component at the App root catches, logs, and shows a recovery UI.
- **CR-15:** `AtomicBool` init-flag reset. The DB-initialisation guard is reset on app shutdown so a restart doesn't get stuck in "already initialised" state.
- **CR-18:** Re-skin to UI/UX spec palette. The running implementation was Mayo-navy; the spec mandated sky-blue (`#0EA5E9`) + teal (`#14B8A6`) + Inter. `src/index.css` is rewritten with the VitalFlow palette; `Sidebar`, `Header`, `Login`, `Setup`, `Dashboard` all updated. The comment `/* CR-18: re-skinned from Mayo navy to spec palette. */` is at `src/index.css:11`.
- **CR-19:** Settings → License panel. New `LicensePanel` component in `Settings.tsx`. Surfaces hospital name, edition, license ID, issue/expiry/maintenance dates, status badge (valid/grace/expired/fingerprint_mismatch/revoked), fingerprint match indicator, enabled modules list. Two operator actions: **Install license** (file picker → `install_license`) and **Revoke license** (`revoke_license` with confirmation). Plus a **Show fingerprint** action (`get_install_fingerprint`).
- **CR-20:** `keygen/` project. New standalone Rust crate at the repository root with 3 binaries:
  - `gen_keys` — generates the company Ed25519 keypair (4 files: `private_key.pem/.bin` + `public_key.pem/.bin`). Prints the public key as a Rust array literal ready to paste into `license.rs::COMPANY_PUBLIC_KEY`. Refuses to overwrite by default.
  - `sign_license` — signs a customer license payload JSON with the private key. Self-verifies after signing.
  - `get_fingerprint` — computes a machine's hardware fingerprint (Windows WMI; `--insecure-dev-fallback` for non-Windows dev).
  - Plus `keygen/README.md` (operator runbook) + `keygen/.gitignore` (ignores `*.pem`, `*.bin`, `private_key*`, `*.license`).
  - This closes the v0.1.0 audit finding that the embedded `COMPANY_PUBLIC_KEY` was a real committed dev keypair (not the documented "all-zeros placeholder") — the keygen project gives the software company a real workflow for production key management.
- **REL-02:** Mutex poisoning recovery. The DB pool, pairing service, and session state `Arc<Mutex<...>>` wrappers now recover from mutex poisoning (a thread panicked while holding the lock) instead of crashing the app. The poisoned data is replaced with a fresh default and an error is logged.
- **REL-03:** Graceful shutdown. New `ShutdownFlags` struct holds `Arc<AtomicBool>` for each background task (broadcast, pairing, scheduler). The Tauri `Builder::build()?.run(callback)` callback intercepts `RunEvent::ExitRequested` and flips the flags, so background tasks stop cleanly on app close instead of being killed mid-operation.

### Batch 3 — High-Severity Cleanup (30+ fixes)

#### Security hardening (SEC series)

- **SEC-03:** Pairing code CSPRNG + brute-force protection. Pairing codes now use `OsRng` (was `thread_rng()`, which is not a CSPRNG on all platforms). Max uses reduced from 10 to 3. Per-IP lockout: after 3 failed attempts within 5 minutes, the source IP is locked out for 15 minutes.
- **SEC-05:** Log redaction + RBAC. `get_log` and `get_log_path` now require `SettingsManage`. `redact_log()` masks `password`, `db_password`, `db_user`, `user`, `username` patterns in log lines before returning them. Failed-login usernames are no longer logged.
- **SEC-08:** LAN discovery broadcast HMAC. The `discovery::broadcast` UDP packet is now HMAC-SHA256-signed with the server's TLS fingerprint as the key. Paired clients verify the HMAC; pre-pairing clients accept any well-formed broadcast (TOFU). Replay protection: broadcasts older than 120 seconds are rejected. Broadcast interval reduced from 5s to 30s (~6x less traffic).
- **SEC-09:** Opener capability reduction. `capabilities/default.json` was granting `opener:allow-open-url`, `opener:allow-open-path`, clipboard read/write — broad surface. Reduced to the minimum set the UI actually uses. (`tauri-plugin-shell` was flagged by the auditor but was never present in `Cargo.toml` — false positive; documented as such.)
- **SEC-10:** SQL identifier validation. `CREATE DATABASE <name>` was using string interpolation; now uses an identifier allow-list (regex `^[a-zA-Z0-9_]+$` + length cap).
- **SEC-13:** TLS key file ACL. `C:\ProgramData\HMS\tls\server.key` is now ACL-restricted to `SYSTEM` + `Administrators` only (was `Builtin Users` modify).
- **SEC-15:** `pg_hba.conf` user restriction. The `pg_hba.conf` rules now scope the `postgres` superuser to loopback only (LAN clients use a dedicated `hms_app` role with limited privileges — Planned Batch 5 to fully wire this; the current rule already restricts `postgres` to loopback).
- **SEC-18:** Generic error messages. Authentication error messages no longer distinguish "user not found" from "wrong password" — both return `Invalid username or password`. Prevents username enumeration.

#### Functional correctness (FUN / IPC / ARCH / STATE / TYP)

- **FUN-09:** Discharge unpaid-bill check. `discharge_patient` now refuses discharge if the patient has unpaid bills (`OUTSTANDING_BALANCE` error). Forces the billing clerk to settle or write off before discharge.
- **IPC-07:** NaN payment guard. `record_payment` now rejects `amount = NaN` / `Infinity` / negative values. Was: accepted and stored, producing weird dashboard totals.
- **IPC-09:** WhatsApp patient-existence + length validation. `send_whatsapp` now refuses if the patient ID doesn't exist or the message is empty / > 4096 chars. Was: silently sent garbage or crashed.
- **ARCH-03:** PatientForm EHR fields. The patient create/edit form now includes the EHR fields (allergies, chronic conditions, blood group, emergency contact) that the SRS required but the form was missing.
- **STATE-02:** Query-key collision. Two distinct queries were sharing the same TanStack Query key prefix, causing cross-invalidation (editing a patient invalidated the appointments list). Query keys are now uniquely prefixed.
- **TYP-04 / QUAL-07:** Currency USD → PKR. The previous implementation was formatting money as `USD $XX.XX`; the SRS requires PKR (`Rs XX.XX`). All `Intl.NumberFormat('en-US', { currency: 'USD' })` calls changed to `Intl.NumberFormat('en-PK', { currency: 'PKR' })`.

#### Design system (DS)

- **DS-01:** Dead `App.css` deleted. The legacy `App.css` (leftover from Create React App scaffold) was overriding Tailwind utilities. Deleted.
- **DS-03:** Dead CSS deleted. Several unused CSS classes (`.btn-old`, `.card-old`, etc.) removed.
- **DS-04:** Dynamic branding. `App.tsx` now reads `licenseInfo?.hospital_name` for the header / login hero / force-change-password / boot footer branding, with `"VitalFlow HMS"` / `"Hospital Management System"` fallbacks. The previous static "Rasheed Medical Center" branding is retired.

#### Interaction (INT)

- **INT-01:** `window.confirm()` → shadcn Dialogs. Five destructive-action confirmations in Billing / Queue / IPD / Users / Laboratory were using `window.confirm()` (unstyled, blocks the main thread, no a11y). Replaced with shadcn `Dialog` components with proper `DialogDescription` + focus management.

#### Accessibility (A11Y)

- **A11Y-01:** `aria-label`s on icon-only buttons. Header buttons, table action buttons, and KPI onClick wrappers now carry `aria-label` attributes. (Some KPI onClick wrappers still need labels — Batch 5.)
- **A11Y-03:** `DialogDescription` on every Dialog. Every shadcn `Dialog` now has a `DialogDescription` so screen readers announce context before focus lands inside.
- **A11Y-04:** Table `scope="col"` default. `TableHead` in `src/components/ui/table.tsx` defaults to `scope="col"` so screen readers announce every column header correctly without each caller having to pass it.

#### Licensing (LIC-DOC)

- **LIC-DOC-04:** License revocation. New `revoke_license` Tauri command, gated behind `LicenseManage`. Removes the on-disk `license.json`, marks `license_state.verification_status = 'revoked'`, writes an audit row.
- **LIC-DOC-06:** SDD §5.4 revocation flow now matches reality. The v0.1.0 SDD documented a revocation flow that didn't exist; the flow now exists (above) and the SDD is updated to match.
- **LIC-DOC-07:** 7-day grace period. `LICENSE_GRACE_PERIOD_DAYS = 7`. A license past its `expiration_date` but within 7 days gets `status = "grace"` (app continues with warning); after 7 days, `status = "expired"` (app refuses to boot).
- **LIC-DOC-08:** License transfer flow. Documented as the operational sequence: revoke on old machine + `install_license` on new machine (using `keygen/get_fingerprint` to get the new hardware fingerprint). No dedicated `transfer_license` command — by design.

### Documentation (Batch 4)

The Phase 1 audit's most-critical documentation finding was DOC-01: the RCTF prompt states all authoritative docs live in `/docs`, but **no `/docs` folder existed** in the project. Batch 4 created the folder and reconciled all 10 documents with the codebase.

- **DOC-01 (resolved):** Created `/docs` folder; created all 10 documents (SRS, SDD, Quality Model, Security Matrix, Risk Register, SDLC, Licensing Architecture, Deployment Guide, UI/UX Spec, Licensing Workflow Guide). Each carries a v0.2.0 version banner referencing this CHANGELOG.
- **Batch 4-A:** Updated SRS (11 edits), SDD (15 edits), Quality Model (17 edits).
- **Batch 4-B:** Updated Security Matrix (20 edits), Risk Register (11 edits), SDLC (12 edits).
- **Batch 4-C:** Updated Licensing Architecture (8 edits), Deployment Guide (10 edits), UI/UX Spec (9 edits), Licensing Workflow Guide (full rewrite). Created this CHANGELOG and the docs/README.md index.

### Known follow-ups (Batch 5 / Phase 2)

These items are documented as "Planned Batch 5" or "Planned Phase 2" across the docs and are NOT yet implemented:

- DPAPI encryption at rest for `config.json` `db_password` (R-003 / R-032).
- CI runner wiring `npm run typecheck` + `npm run lint` + `cargo check` + `cargo audit` + `npm audit` (SDLC-DOC-01).
- SAST + DAST + dependency scanning.
- Audit log immutability (append-only trigger, M-08).
- MFA for admin login.
- WhatsApp templated messages (currently free-text; pending Meta template approval, R-013).
- `clear_config` dead IPC command cleanup (in `config.rs` but not in `generate_handler![]`).
- Select-field `htmlFor` wiring on Billing / Queue / IPD / Users / Laboratory forms (a11y follow-up).
- More `aria-label`s on KPI onClick wrappers (a11y follow-up).
- `<SkipLink>` component (a11y).
- axe-core in CI (a11y).
- Unit / integration / E2E test suites (Batch 6).

---

## v0.1.0 — 2025-07-03 (Initial audited state)

- Pre-audit state. The codebase built under specific developer-machine conditions but failed on a clean checkout (missing `tsconfig.json`, `vite.config.ts`, ESLint, Prettier).
- No `/docs` folder existed (RCTF prompt required all authoritative docs to live there).
- 50+ audit findings across security, functional correctness, reliability, accessibility, and documentation.
- See `PHASE1_AUDIT_REPORT.md` (project root) for the full audit findings.
- See per-batch entries in `/home/z/my-project/worklog.md` for the implementation detail of each fix.

---

_End of `CHANGELOG.md`. For per-document revision detail see the "Revision history" subsection at the top of each document in `/docs`. For the full implementation worklog see `/home/z/my-project/worklog.md`._
