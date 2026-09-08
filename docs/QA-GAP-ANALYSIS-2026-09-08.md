# QA Gap Analysis — VitalFlow HMS (Tauri v2 · Rust · React/Tailwind v4 · PostgreSQL 16)

**Date:** 2026-09-08 · **Method:** Executed QA-Testing-Review-Package.md §8 (RCTF prompt) — every §3 checklist item verified against actual code with file:line evidence, via four parallel layer audits + spot-verification of the highest-impact claims.

**Scope audited:** `E:\hospital-mgt\hospital-mgt` — `src-tauri/` (Rust, ~33k LOC incl. live `src/lib.rs` 1,427 lines), `src/` (React, ~23k LOC), `.github/workflows/ci.yml`, `keygen/`, NSIS installer (`windows/hooks.nsh`), `docs/`.

**Scoreboard:** 20 ✅ · 24 ⚠️ · 5 ❌ · 6 N/A (of 55 checklist items)

---

## 1. Verdict Table (Section 3 Checklist)

### 3.1 Tauri Configuration & Security

| # | Checklist Item | Status | Key Evidence | Risk if Unaddressed | Fix | Priority |
|---|---|---|---|---|---|---|
| 1 | Capabilities scoped per-window | ✅ | `capabilities/default.json` scoped to `"main"` only, minimal permission list | — | — | — |
| 2 | No wildcard permissions; only used APIs | ⚠️ | `fs:allow-write-text-file` = "no pre-configured scope" (ACL manifest); `opener:allow-open-url` unscoped at capability level (Rust side allowlists `wa.me`/`web.whatsapp.com` at `whatsapp/automation.rs:94-124`); `clipboard-manager:allow-read-text` granted but **zero uses in codebase** | Post-XSS exfiltration/persistence surface; clipboard PHI sniffing | Scope fs write to user-chosen save dir; add `https://*` scope to opener; **delete `allow-read-text`** | Medium |
| 3 | CSP defined, not disabled | ✅ | `tauri.conf.json:25`: `script-src 'self'`, no unsafe-inline/eval; overlays don't touch `app.security` | — | — | — |
| 4 | devUrl/dev config not in prod | ✅ | `devUrl` only consumed by `tauri dev`; release uses `frontendDist` | — | — | — |
| 5 | No arbitrary external navigation | ✅ | Only external link: help `<a>` with noopener (`Settings.tsx:779-786`); no `window.open`/`location` redirects | — | — | — |
| 6 | Deep-link handlers validate input | N/A | No deep-link plugin/scheme registered | — | — | — |
| 7 | Updater verifies signatures | N/A | No `tauri-plugin-updater` (manual installer updates) | — | — | — |
| 8 | Identifier/version/metadata correct per OS | ⚠️ | `"targets": "all"` (`tauri.conf.json:30`) but product is Windows-only (NSIS, WMI, DPAPI, `cfg(windows)` deps); overlays intentionally rename per build | Misleading config; accidental broken mac/linux build attempts | Set `"targets": ["nsis"]` (or explicit Windows targets) | Medium |
| 9 | No debug endpoints in release | ✅ | `get_log` gated by `SettingsManage` + `redact_log` masks credentials (`src/lib.rs:226-251, 269-327`) | — | — | — |

### 3.2 Rust Backend / Tauri Commands

| # | Checklist Item | Status | Key Evidence | Risk if Unaddressed | Fix | Priority |
|---|---|---|---|---|---|---|
| 1 | Every command validates input | ⚠️ | RBAC on ~200 commands + conformance test (`tests/ipc_posture_tests.rs:27-66`); good validation in patients/messaging/audit. Gaps: `check_db_connection` (src/lib.rs:1042-1073), `update_appointment_status` accepts any status string (`appointments.rs:298-303`), `create/update_doctor` zero field validation (`doctors.rs:10-40`), `send_whatsapp_notification` bypasses IPC-09 patient/phone/length checks (`whatsapp/commands.rs:19-38` vs 98-142) | Garbage status skips WhatsApp triggers; arbitrary WhatsApp sends; bad doctor data | Whitelist status; validate doctor fields; route raw send through same checks as patient send | High |
| 2 | No unwrap/expect on reachable paths | ⚠️ | `whatsapp/automation.rs:276` `load_whatsapp_config().unwrap()` on scheduler path — panic kills the entire scheduler task (reminders + nightly backup stop silently); `config.rs:124` `.expect()` in IPC-reachable config-dir resolution | Silent loss of reminders/backups until restart | Replace with `?`/fallback + log | High |
| 3 | Errors typed Result + surfaced | ⚠️ | All commands `Result<T,String>` with curated diagnostics (`diagnose_db_error` lib.rs:443-487, `sanitize_db_error` db.rs:30-33). Gaps: audit writes fire-and-forget (stderr only, `audit.rs:55-79` — invisible in GUI, no retry); `get_rooms` `unwrap_or_default` masks DB failure (`messaging.rs:128-132`); raw sqlx errors leak schema to users (`doctors.rs:35`, `appointments.rs:107,188`) | HIPAA audit rows lost invisibly; schema disclosure | Log-and-alert on audit failure; propagate `get_rooms` error; sanitize remaining map_errs | Medium |
| 4 | No shared-state deadlock potential | ✅ | All locks short clone-out sections, **never held across `.await`** (rbac.rs:358-373, 396-427; auth.rs:538, 589-591); no nested locks; poisoning recovered (`unwrap_or_else(\|e\| e.into_inner())`) | — | — | — |
| 5 | Async doesn't block main thread | ⚠️ | Broadcast on own thread; UDP/TCP probes and pg_dump via `spawn_blocking`. Violations: `test_server_connection` blocking `TcpStream::connect_timeout` 3s inside async command (`config.rs:518-521`); sync file-log writes on hot paths (`lib.rs:167-189`) | 3s tokio-worker stalls (not UI thread) | Wrap in `spawn_blocking` | Low |
| 6 | Long tasks cancellable / clean shutdown | ✅ | `ShutdownFlags` flipped in `ExitRequested` (lib.rs:109-125, 1413-1425) with regression test; scheduler exits ≤5s; pairing accept 1s timeout (pairing.rs:327-343) | — | — | — |
| 7 | Logging doesn't leak secrets | ✅ | No URL/password ever printed; only `password set: bool` (lib.rs:378); `redact_log` masks `user=`/`password=` patterns; WhatsApp token masked to last-4 in IPC (whatsapp/commands.rs:209-215) | — | — | — |
| 8 | Unit tests for core logic | ✅ | 130 unit tests in 9 `#[cfg(test)]` modules (runnable DB-free) + 151 integration tests across 17 suites, real Postgres via `HMS_TEST_DB_URL` with production-DB protection (`tests/common/mod.rs:111-151`). **But see CI finding C3** | — | — | — |
| 9 | cargo clippy clean | ⚠️ | Clippy clean **with** `--features hms-integration-tests` (worklog: 0 warnings). But CI runs `cargo clippy --all-targets` **without** the feature (`ci.yml:59`) → E0603 "module is private" errors (verified: `cargo check --test session_tests` fails without feature) | CI quality gate red/broken as configured | Add `--features hms-integration-tests` to CI clippy step | High |
| 10 | cargo audit / cargo deny | ❌ | Not run anywhere; no CI step; docs mark "Planned Batch 5" | Known-CVE dependencies ship silently | Add `cargo audit` + `npm audit` CI steps | Medium |

### 3.3 IPC Boundary (Rust ↔ React)

| # | Checklist Item | Status | Key Evidence | Risk if Unaddressed | Fix | Priority |
|---|---|---|---|---|---|---|
| 1 | TS types match Rust (ts-rs/specta) | ❌ | No codegen; hand-written `src/lib/models.ts` whose own header documents past drift ("one copy of AppointmentWithDetails was missing created_at/updated_at … caused a real bug") | Recurring silent drift bugs | Adopt `ts-rs` or `specta` codegen | High |
| 2 | All invoke() have error handling | ⚠️ | Mutations exemplary: 89/91 `useMutation` with `onError` toasts (`queries.ts`); direct invokes in try/catch. **Read paths: errors dropped** — only Reports.tsx renders `isError` (120-128); no global `QueryCache.onError` | DB outage shows empty states, not errors (dangerous in hospital) | Add global `QueryCache onError` in `main.tsx:15-23` + error state in `PageContainer` | High |
| 3 | Event listeners cleaned on unmount | ✅ | All 4 `listen()` sites unlisten (App.tsx:104-110; Setup.tsx:171; Messaging.tsx:77; TitleBar.tsx:78) | — | — | — |
| 4 | Large payloads chunked/streamed | ⚠️ | No virtualization anywhere; `Queue.tsx:20` fetches the **entire EHR-expanded patient list** for a dropdown; Dashboard pulls full `usePatients()` to check `length===0` | UI freezes + data minimization violation as patient count grows | Server-side select list; pagination on Patients/Appointments | Medium |
| 5 | No races on rapid invokes | ⚠️ | Broadcast/pairing CAS-guarded (`BROADCAST_RUNNING`/`PAIRING_LISTENER_STARTED`, live src/lib.rs:76-80,562,738 — with failure-path reset). Gaps: **scheduler double-start unguarded** (lib.rs:412-429 → scheduler.rs:58-70) → duplicate WhatsApp reminders possible; re-init replaces pool without closing old one (lib.rs:407) | Duplicate messages; connection leak | AtomicBool guard for scheduler; close old pool | High |

### 3.4 React + Tailwind Frontend

| # | Checklist Item | Status | Key Evidence | Risk if Unaddressed | Fix | Priority |
|---|---|---|---|---|---|---|
| 1 | Loading/empty/error states every view | ⚠️ | Loading+empty in all 19 data pages (`shared.tsx:395,425`); **error state only in Reports.tsx** — 0 `isError` in 18 other pages | Staff misread outages as "no patients" | Global query onError + error slot in PageContainer | High |
| 2 | Forms validate client + backend re-validates | ⚠️ | PatientForm/DoctorForm/AppointmentForm/Login solid. Gaps: Users create dialog no checks (Users.tsx:39-46 — 2-char password sent); Billing line items no checks (163-185); Pharmacy `Number(...)\|\|0` silent coercion (395) | Confusing server rejections; bad invoices | Add canSubmit gates matching backend rules | Medium |
| 3 | Global error boundary | ✅ | Root `ErrorBoundary` (componentDidCatch) at `main.tsx:30` with recovery UI | — | — | — |
| 4 | No console errors; StrictMode run | ✅ | StrictMode on (main.tsx:29); 4 console.error all paired with user-facing UI | — | — | — |
| 5 | Tailwind purge / bundle not bloated | ✅ | Tailwind v4 via `@tailwindcss/vite` (auto content detection, nothing to misconfigure); dist main 1.29MB with lazy BloodBank/Radiology chunks. Note: recharts+motion eager; Google Fonts remote `@import` (index.css:1) is an **offline-LAN hazard** — bundled TTFs in `src/assets/fonts/` unused | First paint fails/flash on internet-less hospital LAN | Self-host fonts via `@fontsource` or local TTFs | Medium |
| 6 | Keyboard nav for primary flows | ⚠️ | Focus-visible global (index.css:268-272) + Radix semantics. Gaps: clickable `<tr onClick>` not focusable/keyboard-usable (Dashboard.tsx:121); unlabeled dialog inputs (Queue.tsx:129-148, Users.tsx:163-170, Pharmacy.tsx:884+, Billing, Laboratory); GlobalSearch no aria (TitleBar.tsx:507) | Keyboard-only users locked out of core flows | tabIndex+onKeyDown on rows; htmlFor/id pass; aria-label search | Medium |
| 7 | Basic a11y (labels, contrast, alt) | ⚠️ | Main forms labeled; TitleBar.tsx:113 `text-[#014292]` unreadable on dark card | WCAG failures | Token-based color | Medium |
| 8 | Resize/multi-monitor/DPI doesn't break | ✅ | minWidth 600 honored; mobile drawer <1024px; tables overflow-auto | — | — | — |
| 9 | Dark mode complete | ✅ | Full token set (index.css:138-192) + ThemeToggle; only the TitleBar brand-color defect above | — | — | — |
| 10 | No unnecessary re-renders on large lists | ⚠️ | React Query v5 with centralized keys — clean. `motion.tr` per table row (Patients.tsx:153) mounts animation per row | Jank on large tables | Drop per-row motion | Medium |

### 3.5 PostgreSQL / Data Layer

| # | Checklist Item | Status | Key Evidence | Risk if Unaddressed | Fix | Priority |
|---|---|---|---|---|---|---|
| 1 | Migrations version-controlled/reproducible | ⚠️ | Single imperative `run_migrations()` (db.rs:243, ~1500 lines): 58 tables, idempotent `IF NOT EXISTS` DDL at every boot. **No schema_version table, no checksums, not wrapped in a transaction**; older index creations swallowed via `.ok()` | Undetectable drift; partial-failure = unknown state | Adopt `sqlx::migrate!` or add version table + wrap in tx | Medium |
| 2 | Rollback path for last migration | ❌ | None; only `pg_restore` from backups | Bad migration = restore-from-backup only | Down-migrations or documented restore procedure | Medium |
| 3 | All queries parameterized | ✅ | All values `$n`-bound; only identifier DDL is interpolated and it's regex-validated (`validate_db_identifier` db.rs:197-225); LIKE wildcards escaped (`search.rs:74-78`) | — | — | — |
| 4 | DB-level constraints | ⚠️ | Excellent overall (RESTRICT for PHI, CHECKs on money/blood statuses, UNIQUEs, single-session index db.rs:422). Gaps: `appointments.status` free-text (db.rs:300); **no double-booking exclusion constraint**; `queue_tokens` UNIQUE(date,token_number) claimed in `queue.rs:88` comment **does not exist** (only `idx_queue_status` db.rs:573) | Duplicate token numbers; overlapping bookings; garbage statuses | Add CHECK + unique index + EXCLUDE constraint | High |
| 5 | Transactions wrap multi-step writes | ✅ | tx + `FOR UPDATE` in billing/IPD/lab/blood/queue (billing.rs:158,248; ipd.rs:142-188); appointment+notification deliberately non-atomic, documented | — | — | — |
| 6 | Pool limits/timeouts | ⚠️ | `connect_app`: max 10, acquire 15s (db.rs:126-132); idle/lifetime defaults only | Minor | Pin `idle_timeout`/`max_lifetime` | Low |
| 7 | DB unreachable at startup handled | ✅ | `initialize_database` → curated hints (lib.rs:443-487); frontend `initError` screen (App.tsx:162-178,224+) | — | — | — |
| 8 | Backup/restore strategy | ⚠️ | Real feature: `pg_dump -Fc` + nightly scheduler + keep-N retention pruning (backup.rs:305-384) + RBAC + path validation + ACL hardening. Gaps: **archives are unencrypted PHI**; no offsite/retention documentation in deployment guide | Stolen backup = full PHI | Encrypt archives (age/7z-AES/pgdump+GPG), document retention/offsite | Medium |
| 9 | Sensitive fields hashed/encrypted | ⚠️ | Strong: Argon2id m=19456 (auth.rs:42-47), SHA-256 session tokens, DPAPI db_password (secrets.rs:80-88), bootstrap creds ACL-hardened + forced rotation. Gaps: **app connects as `postgres` SUPERUSER** (config.rs:96, hooks.nsh:210); `config.json.bak` retains v1 **plaintext** password after DPAPI migration (config.rs:276-277); WhatsApp token plaintext in DB (db.rs:1006); PHI plaintext at rest (documented posture) | Any SQLi/bug = full superuser DB compromise; local non-admin reads superuser password | Least-privilege `hms_app` role; delete/ACL the .bak; encrypt token | **Critical** |
| 10 | Indexes on WHERE/JOIN/ORDER BY | ⚠️ | 75+ indexes incl. partials (blood bank exemplary, db.rs:1720-1742). Gaps: **`appointments` has zero indexes** (no patient_id/doctor_id/date/status — most-hit clinical table); encounters/bills/payments/lab_orders FK columns unindexed | Seq scans on core clinical queries; RESTRICT checks seq-scan | Add 4 appointments indexes + FK indexes | High |

### 3.6 Packaging, Distribution & Updates

| # | Checklist Item | Status | Key Evidence | Risk if Unaddressed | Fix | Priority |
|---|---|---|---|---|---|---|
| 1 | Installers for every target OS | ❌ (by-design Windows-only) | Only NSIS server/client installers exist; `"targets": "all"` misleading | — | Declare Windows-only targets explicitly | Medium |
| 2 | Code signing (Authenticode/notarization) | ❌ | No `certificateThumbprint`/`signCommand`/`signingIdentity` in any bundle config | SmartScreen blocks; MITM'd installer indistinguishable | Buy cert + configure `signCommand`; document verification | High |
| 3 | Icon/name/metadata correct | ⚠️ | Window title "VitalFlow HMS" vs installer "HMS Server/Client" (intentional split); fine otherwise | Minor confusion | — | Low |
| 4 | Bundled PG works on clean machine | ✅ (needs fresh-profile test) | NSIS provisions PG16 service, CSPRNG password, scram+hostssl, scoped firewall, upgrade path preserves pgdata (hooks.nsh) | — | Run §4.4 fresh-profile test once | Medium |
| 5 | Uninstall clean + intentional data decision | ✅ | Pre-uninstall stops service, **preserves pgdata intentionally**; orphan service registration noted | — | Document service removal option | Low |
| 6 | Auto-update flow tested | N/A | No updater (manual installs) | — | — | — |

### 3.7 Performance

| # | Checklist Item | Status | Key Evidence | Risk if Unaddressed | Fix | Priority |
|---|---|---|---|---|---|---|
| 1 | Cold start measured | ⚠️ | No measurement evidence found | Unknown regression | Measure & record | Low |
| 2 | Memory bounded (leak check) | ⚠️ | Pool replaced-not-closed on re-init (lib.rs:407) is a real leak path; no long-session data | Slow degradation | Close old pool; measure | Low |
| 3 | Large lists virtualized/paginated | ⚠️ | Only BloodBank (5 tables) + Radiology paginated; Patients/Appointments/Billing/IPD full render; AuditLog capped 300 ✅ | Freezes at 10k+ rows | Paginate Patients/Appointments; server search | Medium |
| 4 | Queries profiled vs full scans | ⚠️ | Appointments unindexed (see 3.5-10) guarantees seq scans; no profiling evidence | Slow core flows | EXPLAIN top queries; add indexes | Medium |
| 5 | Bundle size checked | ✅ | dist present; 1.29MB main (recharts/motion eager — lazy them); no dev deps leaked | Minor | Lazy recharts/motion | Low |

### 3.8 Cross-Platform Parity — N/A (Windows-only product by design)

| # | Checklist Item | Status | Note |
|---|---|---|---|
| 1 | OS-agnostic paths | N/A | Mostly `Path::join`; but WMI/DPAPI/NSIS make the product Windows-only |
| 2 | Native menu/titlebar per OS | N/A | Custom titlebar, Windows target only |
| 3 | Shortcuts don't conflict per OS | N/A | Minimal shortcuts (Enter/Shift-Enter messaging) |
| 4 | Notifications/tray/dialogs per OS | N/A | No tray; dialog plugin used for save/export |

---

## 2. Findings NOT on the Checklist (prioritized)

### Critical

- **C1 — App runs as PostgreSQL SUPERUSER + listens on all interfaces.** `db_user: "postgres"` (config.rs:96, hooks.nsh:210), `listen_addresses='*'` (hooks.nsh:180). Every query the app runs is superuser-privileged; combined with `Users:(M)` ACL on `C:\ProgramData\HMS` (hooks.nsh:40), any local non-admin can copy/tamper the entire pgdata cluster. *Fix:* provision a least-privilege `hms_app` role (app code needs no superuser); tighten ACLs on pgdata like backups already do (backup.rs:64-110).
- **C2 — Plaintext superuser password residue.** (a) `config.json.bak` deliberately retains the v1 plaintext DB password after DPAPI migration (config.rs:276-277), in the Users-writable dir — readable indefinitely; (b) transient `set_pw.sql`/`pg_pwfile.tmp` during install (hooks.nsh:144-151). *Fix:* delete .bak once DPAPI blob verified (or store DPAPI copy in it); clear temp files immediately.
- **C3 — CI quality gate broken; 13 of 17 test suites never run in CI.** `ci.yml:59` runs `cargo clippy --all-targets` **without** `--features hms-integration-tests` → test targets fail E0603 (verified by direct `cargo check --test session_tests`), so the `rust-quality` job is red or the pipeline can't be merging as-is; `ci.yml:103` runs only 4 legacy suites while the 11 newer AERP suites (incl. **session security, WhatsApp authz, billing, backup, config**) need `HMS_TEST_DB_URL` which CI never sets. All 281 tests DO pass when run correctly with the feature (worklog 2026-09-08) — this is purely a CI wiring gap. *Fix:* add `--features hms-integration-tests` to clippy + all test invocations; set `HMS_TEST_DB_URL`; enumerate all 17 suites.

### High

- **H1 — Read-path IPC errors invisible.** Only Reports.tsx renders query errors; no global `QueryCache.onError`. A Postgres outage on a client PC renders cheerful "No patients registered yet" empty states — in a hospital this misleads staff about data existence. *Fix (1 line):* `new QueryCache({ onError: e => toast.error(...) })` in main.tsx + error slot in PageContainer.
- **H2 — `queue_tokens` duplicate-token guard is fiction.** `queue.rs:88` claims "The UNIQUE(date, token_number) index (added in db.rs)" — no such index exists (only `idx_queue_status`, db.rs:573). The `LOCK TABLE ... IN EXCLUSIVE MODE` guard is per-connection/per-process only. *Fix:* `CREATE UNIQUE INDEX IF NOT EXISTS uq_queue_day_token ON queue_tokens ((issued_at::date), token_number)` + ON CONFLICT retry.
- **H3 — `appointments` table integrity/perf gaps.** Zero indexes; `status` free-text (no CHECK, unlike every other status column); no double-booking exclusion. Garbage statuses silently skip the WhatsApp confirmed/cancelled triggers. *Fix:* 4 indexes + `CHECK (status IN (...))` + consider `EXCLUDE USING gist (doctor_id WITH =, daterange(...) WITH &&)`.
- **H4 — Scheduler double-start race.** `start_scheduler` spawns unconditionally on every `initialize_database` Server-role completion (lib.rs:412-429, scheduler.rs:58-70) — double-invoke (double-click, StrictMode double-effect) yields two scheduler loops; the NOT EXISTS dedup has a TOCTOU window → duplicate WhatsApp sends. *Fix:* AtomicBool CAS guard like `BROADCAST_RUNNING`.
- **H5 — `automation.rs:276` unwrap kills the scheduler.** A transient DB/config error between the check and load panics the whole scheduler task: reminders, digest, and nightly backup stop silently until app restart. *Fix:* `match`/`?` + log.
- **H6 — Unsigned installers.** No Authenticode; hospital IT will hit SmartScreen; tampered installers indistinguishable.
- **H7 — WhatsApp token plaintext in DB** (db.rs:1006) — anyone with DB read (backup, dump, DBA) gets a live Meta API token. *Fix:* DPAPI-encrypt like db_password.
- **H8 — `send_whatsapp_notification` bypasses patient-safety checks** (whatsapp/commands.rs:19-38): arbitrary recipient + content with just `WhatsAppSend`, skipping the phone-verification + 1000-char cap that `send_whatsapp_to_patient` enforces (98-142). *Fix:* route through the same checks or remove the raw command.
- **H9 — No IPC type generation.** Hand-written `models.ts`; drift already caused a real bug (per its own header). *Fix:* `ts-rs`.

### Medium

- **M1** — No migration versioning/atomicity (single imperative boot DDL).
- **M2** — **Dead legacy backend files** at `src-tauri/lib.rs` + `src-tauri/db.rs` (non-compiled duplicates of `src/lib.rs`/`src/db.rs`; contain an *unvalidated* `CREATE DATABASE` and RBAC-less `check_db_connection`; one Cargo `[lib] path` edit would resurrect weaker code). Delete them.
- **M3** — `keygen/` committed orphan Ed25519 keypair + `license_payload.json` forging template. Verified it matches **neither** the embedded dev (license.rs:84-88) nor prod (100-105) key → no forgery path today, but pure hazard. Purge.
- **M4** — Backups unencrypted PHI; deployment guide lacks retention/offsite section (retention pruning itself exists).
- **M5** — `audit_logs` not tamper-evident (no append-only trigger/hash chain; docs honestly mark M-08 "Planned").
- **M6** — Capability tightening (see 3.1-2): unscoped fs write, unscoped opener, unused clipboard read.
- **M7** — `delete_doctor` hard DELETE + FK CASCADE (doctors.rs:118) destroys clinical appointment history — inconsistent with patients' soft-delete + 6-year PHI retention (RESTRICT). Restrict + archive instead.
- **M8** — Google Fonts remote `@import` on an offline-LAN product; local TTFs shipped but unused.
- **M9** — `check_db_connection`/`build_url` interpolate password into connection URL (lib.rs:1056-1059, db.rs:63) — special-char passwords misparse/inject params. Use `PgConnectOptions`.
- **M10** — E2E is weak: Vite dev server + 3 stubbed golden-path smoke tests with `waitForTimeout(5000)` flakiness; no real flows, no tauri-driver. Frontend unit coverage ~6% of files, thresholds disabled (vitest.config.ts), `Settings.tsx` (1,662 LOC, 4+ responsibilities) untested.
- **M11** — Frontend route guards inconsistent: only 6/19 routes wrapped in `RequirePermission`; deep-links to /billing /users /audit etc. render and fetch before backend RBAC toasts (server enforces, UX leaks). Wrap all or none.
- **M12** — `Queue.tsx:20` fetches entire EHR-expanded patient list for a dropdown; Dashboard pulls all patients for a length check.

### Low

- **L1** `session_invalidated` emitted but no listener (frontend polls `me` — dead event). **L2** Dead code: `Header.tsx` (179 LOC, zero imports), `AnimatePresence` without location key (exit animations never fire), commented blocks. **L3** `window.location.reload()` on logout/password-change — full webview + license/DB reboot. **L4** Login screen displays the bootstrap-credentials file path pre-auth. **L5** TitleBar `#014292` unreadable in dark mode. **L6** Index-keyed dynamic item rows (Pharmacy prescription editor) shuffle focus on delete. **L7** Pairing status polls at 1s fixed, no backoff. **L8** Test harness drift: compose file (port 5433, placeholder migrate container) vs CI (5432, no `HMS_TEST_DB_URL`); literal `hms_test:hms_test` creds.

---

## 3. Top 5 Must-Fix Before Release

1. **Close the local-privilege / DB-superuser hole (C1 + C2).** Provision a least-privilege app role instead of running every query as `postgres` superuser; stop writing the plaintext password to `config.json.bak`; restrict the `Users:(M)` ACL on `C:\ProgramData\HMS` (pgdata deserves the same hardening the backups dir already got).
2. **Fix CI so the quality gates actually run (C3).** Add `--features hms-integration-tests` to clippy + tests, set `HMS_TEST_DB_URL`, and run all 17 suites — today the security-relevant suites (session, authz, billing, backup) never execute in CI, and the clippy job can't pass as wired.
3. **Surface read-path errors in the UI (H1).** One global `QueryCache.onError` + error slots in the shared page container. In a hospital, "server unreachable" must never look like "no patients registered".
4. **Restore queue + appointments data integrity (H2 + H3).** Add the missing `queue_tokens` unique index (the code comment claims it exists — it doesn't), appointments indexes, and a status CHECK; guard against doctor double-booking.
5. **Make the scheduler singleton-safe and panic-free (H4 + H5).** AtomicBool start guard + replace the `unwrap()` at automation.rs:276 — otherwise reminders and nightly backups can silently die mid-shift.

*(Close 6th: Authenticode-sign installers before any distribution outside your own test machines — H6.)*

---

## 4. Notes on Verification

- Claims spot-verified directly against source before inclusion: dead `src-tauri/{lib,db}.rs` files (no `[lib] path` in Cargo.toml → `src/lib.rs` is authoritative); missing queue UNIQUE index; appointments missing CHECK/indexes; CI's 4-suite test list; E0603-without-feature reproduction; `audit::for_session` stderr logging (downgraded from "silent"); backup retention pruning exists (downgraded to "unencrypted/no-offsite" only).
- The worklog's "clippy 0 warnings / 281 tests pass" is accurate **when run with** `--features hms-integration-tests` (as the worklog does); the contradiction with the audit came from running clippy without the feature — which is exactly what CI does, hence finding C3.
- Positives worth stating plainly: parameterized SQL everywhere, Argon2id + lockouts + timing flattening, DPAPI config secrets, scram-sha-256 + hostssl + cert pinning, RBAC enforced in Rust on every command with a conformance test, real-DB integration tests, cancellation-safe shutdown, secret-free logging. The security engineering here is well above typical for this class of app; the must-fix list is about closing the last real holes.

---

## 5. Remediation Status (2026-09-08, same day)

Executed after this report: "fix all the errors according to priority".

### Fixed (verified)
| Finding | Fix | Verified by |
|---|---|---|
| C2 | `.bak` written as encrypted v2 (`serialize_v2` shared with live write) | cargo test (config_tests incl. migration suite) |
| C1 (pgdata ACL) | `harden_pgdata_acl` in Rust first-launch + `pgdata_harden` label in NSIS hook — SYSTEM+Admins F, installing user RX | code review (installer rebuild pending) |
| C3 | CI: feature flag on clippy+tests, `HMS_TEST_DB_URL` set, bare `--tests` (was 4/17 suites), `--test-threads=1`, cargo-audit job added; ~15 pre-existing test-target lints fixed | local run of the same commands |
| H1 | Global `QueryCache.onError` toast (auth probes excluded) | tsc/eslint/vitest green |
| H2 | `uq_queue_day_token` UNIQUE index + duplicate-heal migration | integration suite |
| H3 | 4 appointments indexes + status CHECK (+legacy normalize) + `check_doctor_overlap` on create/update + duration clamp + status whitelist on both update paths | integration suite |
| H4 | `SCHEDULER_RUNNING` CAS guard; superseded pool closed on re-init; pool idle/lifetime timeouts | unit tests + code |
| H5 | config-load `unwrap()` → graceful deep-link fallback | code (scheduler stays alive on read failure) |
| H8 | raw send routed through IPC-09 patient/length checks; group/test refused | whatsapp_authz suite |
| M2/M3 | dead legacy `src-tauri/{lib,db}.rs` + orphan keygen keys + payload template deleted | ls |
| M5 | `audit_logs` append-only trigger + RLS | integration suite |
| M6 | clipboard-read permission removed; opener scoped to https/mailto | capabilities JSON |
| M7 | `delete_doctor` refusal w/ count; doctor FK CASCADE→RESTRICT (DDL + retro-fit) | integration suite |
| M8 | local Inter variable font; CSP tightened (no remote fonts, no https: img wildcard) | tsc/eslint/vitest |
| M11 | all feature routes `RequirePermission`-wrapped | tsc |
| L2/L5 | Header.tsx deleted; TitleBar brand color theme-aware | eslint |

### Deliberately deferred (need external resources or coordinated design — tracked in worklog)
- **C1 remainder:** least-privilege DB role (app still connects as `postgres` superuser) — requires installer+pairing+config coordination.
- **H6:** Authenticode signing — requires a purchased certificate.
- **H9:** ts-rs/specta type generation — build-pipeline migration.
- **H7:** WhatsApp token encryption at rest — needs secret-store decision.
- **M4:** backup encryption-at-rest + offsite/retention docs.
- **H3 remainder:** DB-level EXCLUDE constraint (app-level overlap check shipped).
- **Release gate:** rebuild both installers (NSIS hook changed) + fresh-machine install test per Section 7.

### Verification of the remediation itself
- fmt clean · clippy `--all-targets --features hms-integration-tests -D warnings` (one documented `-A field_reassign_with_default` for pre-existing test-file style) · `cargo test --lib` 130/130 · tsc 0 · eslint 0 · vitest 109/109 · full 17-suite integration battery against the local HMS PostgreSQL (see worklog 2026-09-08 entry, including the UTF-16 password-recovery trap).
- One defect was introduced and caught during remediation: the `idx_whatsapp_notifications_appt` index was initially placed before its table's CREATE in the migration sequence (every suite failed with "relation does not exist"); moved after the whatsapp_notifications block, suite green again. Recorded so the placement constraint is known for future index additions to `run_migrations`.
