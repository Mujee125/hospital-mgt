# Agile Scrum Backlog — VitalFlow HMS

**Product:** VitalFlow HMS — Hospital Management System (Windows desktop)  
**Tech stack:** Tauri 2 · Rust (Axum, SQLx) · PostgreSQL 16 · React 19 · TypeScript · Tailwind CSS 4  
**Current release:** v0.4.0 (2026-09-09) — all 24 RCTF findings closed · 420 Rust tests + 119 frontend tests, all gates green  
**Last commit on `main`:** 533b641 (2026-09-30)  
**Working tree:** 14 modified files / 2 untracked (uncommitted in-progress work — see § Uncommitted Work below)  
**Today (environment):** 2026-11-05  
**Backlog owner:** Product Owner (engineering)  
**Backlog status:** Draft for Sprint Planning

---

## 1. Project status — where we are (till today)

| Area | State | Evidence |
|------|-------|----------|
| Release | v0.4.0 shipped 2026-09-09; no release since | `CHANGELOG.md` §v0.4.0 |
| Source branch | `main`, up to date with `origin/main` | `git status` / `git branch -a` |
| Last commit | 533b641, 2026-09-30 (~5 weeks ago) | `git log` |
| Uncommitted work | 14 modified files (~1,327 insertions), 2 untracked files (`src-tauri/src/client_seats.rs`, `src-tauri/tests/clear_config_tests.rs`) | `git diff --stat` / `git status` |
| Implemented modules | Patients, Doctors, Appointments, Queue, Encounters, IPD (wards/beds/admit/discharge), Laboratory, Billing, Inventory, Messaging, WhatsApp, Blood Bank, Pharmacy, Nursing, Radiology, Dashboard, Audit Log, Backup, Reports, Accounts (expense ledger), Users, Settings, Setup, Login | `docs/01-SRS.md` §4; `src-tauri/src/commands/` |
| Testing | 420 Rust tests (133 unit + 287 integration, 18 binaries) + 119 frontend tests (78 Vitest component + E2E smoke); cargo fmt/clippy, tsc, eslint, vitest all green | `TESTING.md`; `CHANGELOG.md` v0.4.0 |
| Docs | 10 authoritative docs in `/docs` reconciled to v0.2.0 codebase | `docs/README.md` |
| Known gaps | Batch 5 / Phase 2 follow-ups (DPAPI config, CI wiring, SAST/DAST, audit immutability, MFA, a11y, Batch 6 tests, release gate) | `CHANGELOG.md` "Known follow-ups"; `QA-GAP-ANALYSIS-2026-09-08.md` §5 "Deliberately deferred" |

**Bottom line:** The core product is functionally complete and secure (all RCTF findings closed). Remaining work is almost entirely *hardening, compliance, release engineering, accessibility, and the next test batch* — plus getting the uncommitted in-progress changes reviewed and committed/shipped.

---

## 2. Backlog conventions

- **ID:** `HMS-###` (stable across sprints)
- **Priority (MoSCoW):** `M` = Must (release blocker), `S` = Should (next sprint), `C` = Could (later), `W` = Won't (for now)
- **Story points:** Fibonacci (1, 2, 3, 5, 8, 13)
- **Status:** `To Do` / `In Progress` / `Done` / `Blocked`
- **Source of truth:** linked doc/file:line or finding ID
- **Definition of Done (DoD):** code reviewed + merged to `main`, tests added/updated, `cargo fmt` + `clippy -D warnings` + `cargo test` green, `tsc` + `eslint` + `vitest` green, CHANGELOG entry if user-visible, doc updated per `docs/README.md` §Documentation Improvement Policy.

---

## 3. Product backlog

### Epic E1 — Security & Compliance hardening (Batch 5 deferred)

| ID | User story | Pri | Pts | Status | Acceptance criteria | Source |
|----|------------|-----|-----|--------|---------------------|--------|
| HMS-101 | Encrypt `db_password` at rest in `config.json` via Windows DPAPI | M | 8 | To Do | `db_password` is DPAPI-encrypted on write, decrypted on read; falls back gracefully on non-Windows; covered by `config_tests`; secret never logged (`redact_log`) | CHANGELOG known follow-ups; QA-GAP R-003/R-032 |
| HMS-102 | Encrypt WhatsApp API token at rest | M | 5 | To Do | WhatsApp token stored encrypted; secret-store decision documented; covered by `whatsapp_authz_tests` | QA-GAP H7 deferred |
| HMS-103 | Least-privilege DB role (stop connecting as `postgres` superuser) | M | 13 | Blocked | Dedicated app DB role with least privileges; installer + pairing + config coordinated; tested against fresh PostgreSQL | QA-GAP C1 remainder deferred |
| HMS-104 | Audit log immutability (append-only trigger, M-08) | S | 5 | To Do | PostgreSQL trigger rejects UPDATE/DELETE on `audit_logs`; integration test proves immutability | QA-GAP M-08; CHANGELOG known follow-ups |
| HMS-105 | MFA for admin login | C | 8 | To Do | TOTP-based MFA enroll/verify for admin role; enforced at Settings; backup codes | CHANGELOG known follow-ups |
| HMS-106 | `clear_config` dead IPC command cleanup | S | 2 | To Do | Either register `clear_config` in `generate_handler![]` or remove the implementation + tests; no orphan IPC surface | CHANGELOG known follow-ups |

### Epic E2 — CI/CD, release engineering & supply chain

| ID | User story | Pri | Pts | Status | Acceptance criteria | Source |
|----|------------|-----|-----|--------|---------------------|--------|
| HMS-201 | Wire CI to run `typecheck` + `lint` + `cargo check` + `cargo audit` + `npm audit` | M | 5 | To Do | `.github/workflows/ci.yml` job runs all five gates on PR + main; failures block merge | SDLC-DOC-01; CHANGELOG known follow-ups |
| HMS-202 | Add SAST + DAST + dependency scanning to CI | S | 8 | To Do | SAST (Rust/TS scanner), DAST skeleton, dependency scan (`cargo audit` + `npm audit`) configured; results reviewed | CHANGELOG known follow-ups |
| HMS-203 | Authenticode code-signing for Windows installers | C | 13 | Blocked | Requires purchased certificate; both NSIS installers signed; signature verified on install | QA-GAP H6 deferred |
| HMS-204 | Rebuild both installers (NSIS hook changed) + fresh-machine install test | M | 5 | To Do | Server + client NSIS installers rebuilt; clean Windows VM install + first-run + pairing smoke test passes | QA-GAP release gate deferred |
| HMS-205 | Fix Windows test manifest for `rctf_doctor_scoping_tests` (no manual step) | S | 5 | To Do | Test binary gets Common Controls v6 manifest automatically; zero manual manifest copy needed on Windows | TESTING.md §Windows prerequisite; QA-GAP item 7 |


### Epic E3 — Data protection, reliability & operations

| ID | User story | Pri | Pts | Status | Acceptance criteria | Source |
|----|------------|-----|-----|--------|---------------------|--------|
| HMS-301 | Backup encryption-at-rest + offsite/retention docs (M-04) | S | 5 | To Do | AES-256-GCM encrypted backup archive; retention policy documented; restore from encrypted backup tested | QA-GAP M4 deferred |
| HMS-302 | DB-level double-booking EXCLUDE constraint (backstop for F-23) | S | 5 | To Do | PostgreSQL EXCLUDE constraint prevents overlapping ACTIVE appointments for same doctor; migration added; app-level check retained | QA-GAP H3 remainder deferred |
| HMS-303 | `ts-rs`/`specta` type generation between Rust and TS | C | 8 | To Do | Shared types generated at build; no hand-duplicated command payloads | QA-GAP H9 deferred |

### Epic E4 — Accessibility (WCAG 2.2 AA)

| ID | User story | Pri | Pts | Status | Acceptance criteria | Source |
|----|------------|-----|-----|--------|---------------------|--------|
| HMS-401 | `<SkipLink>` component for keyboard users | C | 3 | To Do | Skip-to-main link present on all pages, visible on focus | 09-UI-UX spec; CHANGELOG a11y |
| HMS-402 | Select-field `htmlFor` wiring on Billing/Queue/IPD/Users/Laboratory forms | C | 3 | To Do | Every `<select>` has associated `<label htmlFor>`; axe/form tests pass | CHANGELOG a11y |
| HMS-403 | More `aria-label`s on KPI onClick wrappers | C | 2 | To Do | KPI cards expose accessible names | CHANGELOG a11y |
| HMS-404 | axe-core in CI | C | 5 | To Do | axe-core accessibility lint runs in CI on build output; violations fail build | CHANGELOG a11y |


### Epic E5 — Testing & quality (Batch 6)

| ID | User story | Pri | Pts | Status | Acceptance criteria | Source |
|----|------------|-----|-----|--------|---------------------|--------|
| HMS-501 | True desktop E2E with `tauri-driver` + real Postgres + real IPC | S | 13 | To Do | E2E suite (not stubbed invoke) runs against real Tauri binary + PostgreSQL; covers login → clinical flow → billing | TESTING.md "What's NOT covered" #1 |
| HMS-502 | IPC integration tests (RBAC + input validation vs real Postgres) | S | 13 | To Do | Each Tauri command tested for RBAC + validation against real test DB via `sqlx::test` / test container | TESTING.md "What's NOT covered" #2 |
| HMS-503 | Playwright E2E integrated in CI | S | 8 | To Do | Chromium binary in CI; Playwright smoke suite runs on PR; longer timeout configured | TESTING.md "What's NOT covered" #6 |
| HMS-504 | Batch 6 unit/integration/E2E test expansion | S | 20 | To Do | Additional coverage for accounts, pharmacy, blood bank, scheduler; target >500 Rust tests | CHANGELOG known follow-ups |
| HMS-505 | Queue token state-machine guard test (waiting→in-progress→completed) | S | 3 | To Do | `set_token_status` enforces transitions; B6-A finding closed | TESTING.md "What's NOT covered" #4 |
| HMS-506 | Frontend `PERMISSIONS` vs Rust `Permission` reconciliation (35 vs 37) | S | 3 | To Do | Permission sets aligned; drift test added | TESTING.md "What's NOT covered" #5 |

### Epic E6 — Product / clinical features

| ID | User story | Pri | Pts | Status | Acceptance criteria | Source |
|----|------------|-----|-----|--------|---------------------|--------|
| HMS-601 | WhatsApp templated messages (pending Meta template approval) | C | 8 | To Do | Template-based WhatsApp sends behind feature flag; free-text retained as fallback | CHANGELOG R-013 deferred |
| HMS-602 | Accounts expense ledger — verify UI Expenses tab + CSV export complete | S | 3 | To Do | `get_accounts_summary`, `export_report_csv` type `accounts_summary`, Billing page Expenses tab end-to-end tested | SRS §4.24 |
| HMS-603 | `validate_db_identifier` / `sanitize_db_error` regression coverage (SQL pattern) | S | 2 | To Do | Add regression tests for race-condition SQL patterns | TESTING.md Rust unit coverage |


---

## 4. Uncommitted work in the working tree (triage first)

14 modified files (~1,327 insertions) + 2 untracked files are **not yet on `main`**. Before planning new sprints, triage these:

- `src-tauri/src/{config,db,lib,license,pairing,pg_provision,tls_provision}.rs` — large Rust changes (likely DPAPI config / pairing / TLS hardening — in-progress Batch 5)
- `src-tauri/tests/{ipc_posture_tests,clear_config_tests}.rs` + untracked `clear_config_tests.rs` — IPC posture / clear_config tests
- `src-tauri/src/client_seats.rs` (untracked) — new file; purpose to confirm
- `docs/01-SRS-Software-Requirements.md`, `docs/02-SDD-Software-Design.md` — doc deltas to reconcile
- `src/App.tsx`, `src/pages/Setup.tsx`, `package.json`, `package-lock.json` — frontend/setup changes

**Recommended first action:** review `git diff`, commit or revert, then branch `release/v0.5.0` (or `batch-5-hardening`) and pull HMS-101/HMS-104/HMS-201/HMS-204 into Sprint 1.

---

## 5. Suggested Sprint 1 — "v0.5.0 Hardening & Release Gate" (2 weeks)

**Goal:** Ship v0.5.0 with the uncommitted security work committed, CI gates wired, installers rebuilt + install-tested, and the highest-priority compliance items closed.

| Story | ID | Pri | Pts | Owner |
|-------|-----|-----|-----|-------|
| Triage + commit uncommitted changes | — | M | 3 | Dev lead |
| DPAPI encrypt `config.json` `db_password` | HMS-101 | M | 8 | Backend |
| Wire CI gates (typecheck/lint/cargo check/cargo audit/npm audit) | HMS-201 | M | 5 | DevOps |
| Rebuild installers + fresh-machine install test | HMS-204 | M | 5 | Release |
| Audit log immutability trigger | HMS-104 | S | 5 | Backend |
| `clear_config` IPC cleanup | HMS-106 | S | 2 | Backend |
| Fix Windows test manifest (remove manual step) | HMS-205 | S | 5 | QA |
| CHANGELOG + docs reconciliation | — | S | 3 | Tech writer |

**Sprint 1 capacity:** ~41 points (adjust to team velocity).  
**Definition of Done for Sprint 1:** v0.5.0 tag cut; both installers rebuilt and install-tested on clean Windows; CI gate job green on `main`; all merged stories meet DoD.

---

## 6. Suggested Sprint 2+ roadmap (indicative)

| Sprint | Focus | Candidate stories |
|--------|-------|-------------------|
| Sprint 2 | Testing & quality (Batch 6) | HMS-501, HMS-502, HMS-503, HMS-504, HMS-505, HMS-506 |
| Sprint 3 | Data protection & operations | HMS-102, HMS-301, HMS-302, HMS-303 (least-privilege HMS-103 remains blocked until cert/installer coordination) |
| Sprint 4 | Accessibility | HMS-401, HMS-402, HMS-403, HMS-404 |
| Sprint 5 | Product features & MFA | HMS-601, HMS-602, HMS-603, HMS-105 |
| Ongoing | Supply chain | HMS-203 (blocked — needs purchased Authenticode cert) |

---

## 7. Roles & ceremonies (lightweight Scrum)

- **Product Owner:** owns this backlog, prioritizes, accepts stories at Sprint Review
- **Scrum Master / Dev Lead:** runs Sprint Planning, Daily Stand-up (15 min), Sprint Review, Retrospective; maintains board
- **Developers:** Rust backend, React frontend, Release/DevOps, QA
- **Cadence:** 2-week sprints; sprint starts Monday

**Suggested board columns:** Backlog → Ready → In Progress → Review → Done

---

## 8. Immediate next steps (this week)

1. Review `git diff` on the 14 modified files; decide commit vs revert; confirm purpose of untracked `client_seats.rs`.
2. Create branch `batch-5-hardening` from `main`; commit triaged work.
3. Pull HMS-101, HMS-201, HMS-204, HMS-104 into Sprint 1; run Sprint Planning.
4. Add this file to the repo and update `CHANGELOG.md` with the v0.5.0 planning note.

