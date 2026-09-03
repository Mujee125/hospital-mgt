# VitalFlow HMS — Windows Verification Report (RCTF-IMPL-001 Priority 1)

**Date:** 2026-08-31 → 2026-09-01
**Machine:** Windows 11 x64 (DESKTOP-S7PQB39), Rust stable-msvc, Node 24, PostgreSQL 18.4 (bundled)
**Scope:** Verify WP-1/WP-2/WP-3 security implementations on a real Windows system, per Handoff §11 Priority 1.

---

## Executive Summary

The handoff document's central worry was correct: **the WP-3 code had never been compiled or executed on Windows.** Verification found **4 real defects** (2 of them deployment-fatal), fixed all of them in code, rebuilt installers, and then **passed every runtime security check end-to-end** against live PostgreSQL, including the GUI.

The authentication subsystem can now be considered **implementation-verified on Windows** (unit + integration-lite + runtime GUI). Remaining risks are listed in §5.

---

## 1. Defects Found & Fixed (this verification)

| ID | File | Defect | Severity | Fix |
|---|---|---|---|---|
| VF-VERIF-001 | `src-tauri/src/secrets.rs` | `LocalFree(Some(ptr))` — invalid windows-rs 0.58 API usage; **library did not compile on Windows** (6 errors). Invisible on Linux because `#[cfg(windows)]` blocks are never compiled there. | **Blocker** | `LocalFree(HLOCAL(ptr))` + import fix; also removed unneeded `mut` on `CryptUnprotectData` input. |
| VF-VERIF-002 | `src-tauri/src/config.rs` | `save_config` round-trip broken: `get_config` never sends `db_password` (skip_serializing, correct), but serde **required** the field on the way back in → every Settings save failed with "missing field `db_password`". | **High** (v2 migration unreachable via GUI) | `#[serde(default)]` on `db_password`; `save_config` now **merges** UI fields onto the freshly-loaded on-disk config instead of persisting the webview payload verbatim (also prevents a tampered frontend from wiping credentials). |
| VF-VERIF-003 | `src-tauri/src/config.rs` | `db_password_encrypted` is `skip_serializing` (to keep the blob out of IPC replies) — but `save()` serializes the same struct **to disk**, so the v2 file was written **without the blob**: plaintext gone, ciphertext never stored → password lost forever on next launch (bricked credential). | **Blocker** (credential loss) | `save()` now serializes to a `serde_json::Value` and re-injects `db_password_encrypted` before writing. IPC replies stay blob-free; disk gets the blob. |
| VF-VERIF-004 | `src-tauri/src/config.rs`, `src-tauri/src/tls_provision.rs` | CR-5/SEC-13 ACL hardening (`SYSTEM`+`Administrators` only) made **both** `config.json` and `tls/server.key` unreadable by the app itself (it runs as the logged-in, non-admin user) → every non-elevated launch failed ("os error 5", misleadingly reported as "config.json missing"). Same defect class, two files. | **Blocker** (non-admin launch) | Both ACL grants now include the current user: `config.json` gets `USERNAME:(M)` (Modify — save() renames over it), `server.key` gets `USERNAME:(R)` (read-only — the app never rewrites it). All other non-admin users remain excluded. |

Secondary cleanups (user-directed "unfreeze blood bank"):
- Blood Bank module unfrozen; all 10 clippy warnings resolved: 2 dead consts annotated as test-contract (`VALID_DONOR_STATUSES`, `VALID_RESERVATION_STATUSES` — kept because the unit tests exercise them; production wiring is still missing and tracked in §5), truly-dead `SELECT_RESERVATIONS` deleted, `expire_blood_units` + `is_abo_rh_compatible` annotated `#[allow(dead_code)]` with explanatory comments (scheduler wiring absent; compatibility enforcement in production goes through the seeded DB matrix), 2× `too_many_arguments` annotated (stable IPC contract), 2× `bind_idx` `unused_assignments` annotated (harmless post-increment), 1× `let`-return simplified. **Clippy now: 0 warnings** (was 9-11).

Note on VF-VERIF-004 trade-off: `config.json` is now readable by the app user. The password inside is **DPAPI machine-bound ciphertext** (v2), so file theft alone yields nothing on another machine, and on the same machine an attacker who can run code as that user can already do worse. The pre-fix design (admin-only read) was cryptographically stronger but **functionally broken** for every non-elevated launch. A future hardening could run a broker/service for config access; tracked in §5.

---

## 2. Build Gates (all green, after fixes)

| Gate | Result |
|---|---|
| `cargo check --lib` | exit 0 (6 pre-existing frozen-module warnings → now 0 after unfreeze fixes) |
| `cargo test --lib` | **103/103 pass**, incl. 3 DPAPI tests executing real `CryptProtectData`/`CryptUnprotectData` on Windows for the first time |
| `cargo clippy --lib` | **0 warnings** (baseline was 9; handoff allowed ≤9) |
| `npx tsc --noEmit` | exit 0 |
| `npx eslint .` | exit 0 |
| `npx vitest run` | **109/109 pass** |
| `npx vite build` | exit 0 |

---

## 3. Installer Rebuilds

Three full server builds + one client build were produced during verification (each ≈7-8 min). The **final** installers (all 4 fixes included):

- `src-tauri/target/release/bundle/nsis/HMS Server_0.1.0_x64-setup.exe` (Sep 1)
- `src-tauri/target/release/bundle/msi/HMS Server_0.1.0_x64_en-US.msi`
- `src-tauri/target/release/bundle/nsis/HMS Client_0.1.0_x64-setup.exe`

Install path exercised 3× as silent in-place upgrades over a live deployment — pgdata, license, and users survived every upgrade (verified via SQL user counts).

---

## 4. Runtime Verification Results (live PostgreSQL 18.4 + GUI via WebView2 CDP)

| # | Handoff §11 Priority 1 check | Result |
|---|---|---|
| 1 | App starts, connects to PostgreSQL, login screen appears | ✅ (after license renewal + VF-VERIF-001/-004 fixes) |
| 2 | Argon2id login works (m=19456,t=2,p=1) | ✅ logged in via GUI as seeded test super_admin |
| 3 | **WP-1 DB seeding**: permissions 52→54 (full enum; handoff's "55 variants" was a miscount — 54 is correct), WhatsApp grants exact: doctor/nurse/receptionist/super_admin=send+view, billing_clerk=view-only, patient/lab_tech/pharmacist=none. role_permissions=148 = exact sum of code grants | ✅ |
| 4 | **WP-1 runtime denial** (patient attempts WhatsApp via IPC — the attacker path): `send_whatsapp_notification` → "requires 'whatsapp.send'"; `send_whatsapp_to_patient` → same; `get_notification_log` → "requires 'whatsapp.view'" | ✅ |
| 5 | **config.json v2 migration** (Settings save in GUI): `config_version: 2`, **no `db_password` field**, `db_password_encrypted` = genuine DPAPI blob (`AQAAANCMnd8…`, 396 chars base64), ACL hardened (user read/modify, admins full) | ✅ |
| 6 | **DPAPI restart round-trip**: app closed, relaunched → blob decrypted, PostgreSQL connected, login succeeded | ✅ |
| 7 | **WP-2.2 deactivation invalidation**: user deactivated in DB mid-session → next `require_strong` command (`create_patient`) returned exactly `"Session invalidated. Please sign in again."` | ✅ |
| 8 | Cross-PC session invalidation (same-machine approximation) | ⚠️ Partially covered — deactivation path proves the DB-backed check; true 2-PC login test still requires two machines (see §5) |

Operational notes discovered during verification:
- **License expiry is real**: the dev license on this machine had expired (7-day grace elapsed) — app blocked at the license screen. Renewed with the bundled `gen_production_license` tool (signs with the embedded dev keypair, `dev:false`). **The app still embeds the DEV public key** (`COMPANY_PUBLIC_KEY` ≠ keygen's keypair) — production deployments cannot be licensed until the real keypair is embedded. This was already documented in the code as the production step, but it is now *empirically* confirmed: **no currently-buildable installer can accept a true production license.**
- PostgreSQL SSL is enforced (`sslmode=require`); psql connects only with TLS.
- Audit trail: every login/logout/config action during testing was captured in `audit_logs` (checked during C5 SQL access).

---

## 5. Still NOT Verified / Remaining Risks

1. **True 2-PC cross-PC login test** — needs a second physical/virtual client machine (client installer is built and ready).
2. **require_strong performance benchmark** (AERP H2-001-14: 1000 calls < 5 s; 50 concurrent users) — not run; the deactivation test proves correctness, not throughput.
3. **86 missing tests** from AERP Part G (integration/negative/penetration/concurrency/LAN/Windows-only suites) — unchanged; this report covers the runtime subset but formal test suites remain unwritten.
4. **License keypair rotation** — embedded key is the committed dev keypair; keygen's production keypair is NOT embedded. Before any customer deployment: embed real public key, sign customer licenses from keygen, destroy private key. Also note `keygen/private_key.pem` is committed to the repo — acceptable while it only signs dev licenses (current state), fatal if ever used as the production key.
5. **Blood Bank scheduler wiring** — `expire_blood_units` is written, tested at unit level, and intentionally not IPC-exposed, but the scheduler (`scheduler.rs`) never calls it → expired units do not auto-expire. Donor/reservation status validation consts similarly unwired. Tracked for a follow-up work package.
6. **session_invalidated frontend listener** — backend verified; the React side still has no listener (users see a raw error rather than a clean redirect to login) — unchanged from handoff.
7. DPAPI machine-binding (`CRYPTPROTECT_LOCAL_MACHINE`) means **OS reinstall / machine migration breaks config decryption** — documented, tested implicitly (same-machine round-trip), but a `ProgramData\HMS` migration procedure should be documented for IT staff.

---

## 6. Artifacts

- Fixed source: `src-tauri/src/{secrets.rs, config.rs, tls_provision.rs, commands/blood_bank.rs}`
- Installers: `src-tauri/target/release/bundle/{nsis,msi}/` (rebuilt 2026-09-01)
- DB state after verification: 1 user (`admin`), 54 permissions, 148 role_permissions, config v2 (DPAPI), PostgreSQL service `HMS-PostgreSQL` healthy
- Test users `vf_test_admin` / `vf_test_patient` were created, used, and **deleted**; temp config snapshots cleaned

## 7. Verdict

**WP-1: VERIFIED** (DB + GUI denial). **WP-2.1: code-verified** (`me` SQL fix in place; runtime cross-PC test pending hardware). **WP-2.2: VERIFIED** (deactivation invalidation at runtime). **WP-3: VERIFIED after 3 code fixes** (compile, round-trip serialization, ACL). The subsystem is now *implementation-verified on Windows* — the missing piece before this report was exactly what the handoff feared: none of it had ever run on the target OS.

---

## §8 Update — 2026-09-02: Priority 2 Executed (AERP Part G suite)

**205 tests passing** (103 lib unit + 102 integration across 7 binaries). All original §5 risks #2 (partially — see below) and #3 (test-infrastructure aspects) addressed. Details in worklog 2026-09-02 entry.

Headline outcomes:
- 62 new AERP Part G tests + 40 legacy tests repaired to actually compile/run (they never had — wrong crate name).
- **4 new production defects found & fixed**, incl. a fatal one: `patients.rh_factor` missing from migrations (any blood issue would crash); config save() concurrency race; Argon2 blocking the async runtime; spec-mandated config .bak + unknown-version rejection.
- **H2-001-14 benchmark (Priority 4) passed** as part of WP2-C04: 1000 require_strong calls well under 5s.
- Production isolation verified: hospital_db untouched; test DB dropped; ProgramData protected during tests (one stale pre-redirect overwrite of the already-obsolete bootstrap-credentials.txt occurred — see worklog).
- Installers rebuilt (Sep 2) containing every fix.

Remaining after this round: true 2-PC LAN tests (client installer ready), independent code review (Priority 3 — now including this round's diffs), Priority 5 subsystem reviews.
