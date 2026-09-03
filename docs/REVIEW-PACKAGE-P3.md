# Independent Review Package — VitalFlow HMS Authentication Remediation

**Prepared:** 2026-09-02 · **For:** an external reviewer with NO prior context (AI session or human)
**Repo:** `E:\hospital-mgt\hospital-mgt` (branch `main`)

Read this file top-to-bottom (~10 minutes), then you are reviewing, not researching.
Everything you need is in §2–§6. Do not trust this document's conclusions — that is
precisely what you exist to check. Every claim below is falsifiable against the cited
file:line.

---

## §1 — What you are reviewing and why

This hospital management system (Tauri v2 + Rust backend + React frontend + PostgreSQL,
Windows-only, one hospital per deployment) went through three remediation change-sets.
**All three were designed, implemented, and verified by the same AI session lineage.**
That is the governance failure you are here to repair (Handoff §13.6). Your job is NOT
to re-derive the work — it is to find the bypass, the edge case, and the omission the
author could not see in their own work.

The three change-sets (all in `main`'s working tree):

| Set | What | Authored | Baseline |
|---|---|---|---|
| **A** | WP-1..WP-3 security fixes (WhatsApp RBAC, token-hash sessions, `require_strong`, DPAPI config encryption) | Jul 14–16 (upstream, before this machine) | **already committed** in `f2f669e` "changes are commit on 18-08-26" |
| **B** | Windows verification round: 4 defect fixes (VF-VERIF-001..004) | Aug 31–Sep 1 (this session) | uncommitted working tree |
| **C** | AERP Part G test round: 3 new suites + 4 repaired legacy suites + 4 more production fixes + core extractions | Sep 2 (this session) | uncommitted working tree |

**Diff scope, exactly:** Sets B+C are the working tree vs `f2f669e`:

```
git -C E:\hospital-mgt\hospital-mgt diff --stat          # Set B+C, 14 files, +812/−211
```

```
 src-tauri/Cargo.toml                   |  11 +
 src-tauri/src/auth.rs                  | 215 ++++++++++------
 src-tauri/src/commands/patients.rs    |  25 ++-
 src-tauri/src/config.rs                | 172 +++++++++++++--
 src-tauri/src/db.rs                    |   7 +
 src-tauri/src/lib.rs                   |  42 +++-
 src-tauri/src/tls_provision.rs         |  18 +-
 src-tauri/src/whatsapp/automation.rs   |   2 +-
 src-tauri/src/whatsapp/commands.rs     |   81 ++++----
 src-tauri/src/whatsapp/mod.rs          |   3 +
 src-tauri/tests/common/mod.rs          | 380 ++++++++++++++++++++-----
 src-tauri/tests/concurrency_tests.rs   |   4 +-
 src-tauri/tests/integration_tests.rs   |   2 +-
 src-tauri/tests/ipc_security_tests.rs  |  61 ++++--
```

Untracked new files (Set C):
```
 src-tauri/tests/config_tests.rs        (348 lines)
 src-tauri/tests/session_tests.rs       (879 lines)
 src-tauri/tests/whatsapp_authz_tests.rs (577 lines)
 docs/VERIFICATION-REPORT-WP1-WP3.md
```

To review Set A too (recommended — it was never independently reviewed either):
it is the diff `2218ea5 → f2f669e`, concentrated in `rbac.rs`, `auth.rs`,
`whatsapp/commands.rs`, `config.rs`, `secrets.rs` (new file), plus 34 guard call-site
changes across `commands/*.rs`. The Handoff (`documentation/VitalFlow-HMS-Project-Handoff.md`
§4 "Files Modified by Implementation") tabulates it.

**Reproduce the test suite before reading any code** (30 min, gives you ground truth):
```bash
cd E:\hospital-mgt\hospital-mgt\src-tauri
# <pw> = the DPAPI-decrypted db password; see §6 note if you can't read it
HMS_TEST_DB_URL="postgresql://postgres:<pw>@127.0.0.1:5432/postgres?sslmode=require" \
  cargo test --features hms-integration-tests --tests -- --test-threads=1
```
Expected: 205 passing (103 lib + 14 config + 20 WP-1 + 28 WP-2 + 15/12/9/4 legacy).
Gates: `cargo check --lib` 0 errors, `cargo clippy --lib` 0 warnings,
`npx tsc --noEmit` / `npx eslint .` exit 0, `npx vitest run` 109 pass.
The suite runs against an isolated `hospital_db_test` it creates itself; production
`hospital_db` is never touched (verified post-run; see §6 note 3).

---

## §2 — Finding-ID map

Two rounds of author-found defects. For each: what was wrong, where the fix is,
and **what the fix might have broken** (your attack surface).

### Round B — Windows verification (Aug 31–Sep 1) — `docs/VERIFICATION-REPORT-WP1-WP3.md`

| ID | Defect | Fix site | What to scrutinize |
|---|---|---|---|
| VF-VERIF-001 | WP-3 DPAPI code **did not compile on Windows**: `LocalFree(Some(ptr))` invalid for windows-rs 0.58. Invisible on Linux (cfg-gated). | `secrets.rs:98,132` → `LocalFree(HLOCAL(...))` | Any other windows-rs misuse in `secrets.rs`; memory freeing on error paths; whether `CryptUnprotectData` input aliasing (`&input` on ciphertext bytes) is sound. |
| VF-VERIF-002 | `save_config` IPC round-trip failed: `get_config` never sends `db_password` (skip_serializing) but deserialization required it → every Settings save errored. | `config.rs:26-27` `#[serde(default)]`; `config.rs:351` `save_config` now MERGES payload onto loaded-from-disk config instead of persisting webview payload | **HIGHEST-RISK SITE.** The merge: which fields does the webview get to overwrite? Can a tampered frontend set `db_password` non-empty (bypassing disk value)? Can it flip `setup_complete`/`pinned_*`? Is `existing` re-loaded BEFORE the guard check (TOCTOU)? What happens when no on-disk config exists (first-run path) — does `unwrap_or_else(|| config.clone())` resurrect the untrusted payload? |
| VF-VERIF-003 | v2 config written **without the encrypted blob** (skip_serializing applied to disk write) → password destroyed on next launch. | `config.rs:206-211` re-injects `db_password_encrypted` into the serialized JSON | Is the re-injection conditional correct (`is_some()`)? Can `to_value` drop or rename fields vs `to_string_pretty` path? Is there any OTHER write path that still serializes the struct directly? |
| VF-VERIF-004 | ACL hardening (`SYSTEM`+`Administrators` only) locked the app out of its own files (app runs as non-admin user): `config.json` and `tls/server.key` unreadable → every launch dead. | `config.rs:278-287` grants `USERNAME:(M)` on config.json; `tls_provision.rs:137-143` grants `USERNAME:(R)` on server.key | **ACL TRADE-OFF.** The current user now has Modify on config.json / Read on server.key. Is `USERNAME` env trustworthy (can a process spawn with forged USERNAME)? What if `USERNAME` contains spaces (icacls arg quoting)? Does the grant survive the `/inheritance:r` ordering? Is the key read grant acceptable given key is only TLS material? Compare `pg_provision.rs` — does it have the SAME bug for the postgres private key (untouched this round — check!). |
| (B-extra) | TLS key read failure at startup reported misleadingly as "config.json missing"; `load()` returned None on unreadable file → startup wrote a DEFAULT config over the real one (clobber path!). | `lib.rs` startup error handling (view `git diff src/lib.rs` for the guard), config save ACL fix above | Does any code path still write a default config when load fails? The clobber was prevented only by the ACL blocking the rename — is that still the only thing preventing it? |

### Round C — test round (Sep 2) — worklog entry 2026-09-02

| ID | Defect | Fix site | What to scrutinize |
|---|---|---|---|
| (C-1) | **`patients.rh_factor` column missing from migrations** while `blood_bank.rs:1999` SELECTs it → `create_blood_issue` crashes on ANY deployment. Found by first-ever legacy-suite execution. | `db.rs:439` adds `("rh_factor", "VARCHAR(5)")` to the patients ALTER loop | Why did nothing catch this for weeks (answer: the legacy tests never compiled)? Does anything else reference patients.rh_factor (grep) — CHECK constraints, indexes? Should it be CHECK-constrained ('+','-') like blood_units (gap documented, not fixed)? |
| (C-2) | Config `save()` temp-file race: fixed name `config.json.tmp` → concurrent saves collided (os error 2). | `config.rs:247-263` unique temp names (pid+nanos+atomic counter) | Is the rename still atomic? Can stale `config.<uniq>.json.tmp` files accumulate (leak on rename failure only cleans its own)? Is `with_file_name` correct vs `with_extension` for paths without extensions? |
| (C-3) | Argon2 (~100ms, memory-hard) ran on tokio workers → PoolTimedOut cascades under concurrent logins. | `auth.rs:73-84` `hash_password_async`/`verify_password_async` (spawn_blocking); all async call sites migrated (`auth.rs` login_core dummy-verify + real verify, change_password, create_user, reset core, bootstrap seed) | **Fidelity:** did every async caller actually migrate (grep `hash_password(`/`verify_password(` in async fns — line 318 `seed_defaults` bootstrap is `hash_password_async` now; any stragglers)? Is the timing-flattening dummy-verify still present (enumeration resistance)? Did wrapper behavior change at all (diff the wrappers against `f2f669e`)? |
| (C-4) | Spec behaviors missing: no `.bak` on v1→v2 migration (WP3-I05), no unknown-version rejection (WP3-N04). | `config.rs:134-146` (>2 → None); `config.rs:226-240` (.bak only when disk is v1 AND no .bak exists) | The `.bak` contains the **plaintext v1 password** — is its ACL hardened (it is a plain `fs::copy` — CHECK: the .bak likely inherits default ACLs → plaintext password readable by local users?! Verify `icacls` on a migrated .bak). The version gate: `version > 2` — what about non-numeric `config_version` (as_u64 → None → treated as v1; is that right vs corrupt?)? |

---

## §3 — The core extractions (fidelity review)

To make the logic testable without a Wry AppHandle (tauri::test's mock runtime is broken
on this machine — see §5), Set C extracted logic into `*_core` functions and left thin
Tauri wrappers. **For each pair, diff the core against the old monolithic command
(`git show f2f669e:src-tauri/src/auth.rs` etc.) and verify ONLY the AppHandle concerns
were moved out.** Any behavioral drift here is a silent authz change.

| Core | Wrapper | Fidelity checklist |
|---|---|---|
| `auth.rs:413 login_core` | `login` (+ emits `session_invalidated` on success, WARN log on failure — **verify the event fires on exactly the same conditions as before**, incl. user_id) | lockout counter, dummy-verify timing, single-session DELETE, audit rows, session-state write |
| `auth.rs:601 me_core` | `me` (pure pass-through) | token_hash SQL, expiry check, state-clear on invalid |
| `auth.rs:745 update_user_core` | `update_user` (emits event when `is_active==Some(false) \|\| roles.is_some()` — **note the wrapper pre-computes `emits` before the call; verify no ordering bug**) | role sync, audit, cannot-target-self rules |
| `auth.rs:821 reset_user_password_core` | `reset_user_password` (emits on success) | hash update, must_change flag, session DELETE, audit |
| `commands/patients.rs:92 create_patient_core` | `create_patient` (pure pass-through) | require_strong guard, audit |
| `whatsapp/commands.rs:98 send_to_patient_checks` | inline call inside `send_whatsapp_to_patient` (guard runs BEFORE checks — **verify guard order preserved**) | patient-lookup, soft-delete filter, 1000-char cap |

Also feature-gated visibility (review for zero production impact):
`lib.rs` (20 occurrences of `hms-integration-tests` cfg-pub), `whatsapp/mod.rs:3`,
`whatsapp/automation.rs` `check_patient_consent` → pub, `auth.rs` `load_session` → pub,
`config.rs` `load_from`/`save_to`/`config_path_for_tests` (cfg'd to the feature).
**Check:** none of these are `#[cfg(test)]` (which would NOT apply to integration
tests); all are the cargo feature; confirm `cargo build` (no features) compiles identically.

---

## §4 — High-risk areas, in priority order

1. **`save_config` merge (`config.rs:351`)** — the one place where untrusted webview
   input touches the credential-bearing config. Enumerate every field the payload can
   overwrite; try to construct a payload that (a) plants a `db_password`, (b) unsets
   `setup_complete` to re-open first-run flows, (c) swaps `pinned_server_fingerprint`
   (TLS pinning bypass?). The test suite covers the happy path; adversarial payloads are yours.
2. **The `.bak` plaintext leak (`config.rs:226-240`)** — my own §2 table flags this as
   unverified: `fs::copy` does not ACL-harden the destination. On Windows the .bak may
   inherit the HMS dir's Users-modify ACEs → last-known plaintext DB password readable
   by any local user. Verify with icacls on a real migrated file; if confirmed, this is
   a finding (severity: medium-high; fix: ACL the .bak like config.json).
3. **ACL grants (`config.rs:278-287`, `tls_provision.rs:137-143`)** — including the
   questions in the VF-VERIF-004 row. Also grep for OTHER ACL sites
   (`auth.rs` bootstrap-credentials writer, `pg_provision.rs`) — the same lockout bug
   class may exist there (bootstrap-credentials.txt is written with SYSTEM+Admins only —
   the app writes it ONCE during seed on an empty DB... but as which user? Is it ever
   re-read by the app itself?).
4. **Guard inventory (`tests/session_tests.rs` `wp2_i11`)** — the test pins 34
   require_strong sites by counting `require_strong(` occurrences per file. That is
   **syntactic, not semantic**: it can't tell whether a site guards the RIGHT command,
   and a string in a comment counts. Spot-check 5 random sites in
   `commands/{radiology,lab,pharmacy}.rs` and confirm the guarded command is actually
   high-risk per AERP C.2.2, and check for commands that SHOULD be strong but use plain
   `require` (the AERP originally said 22; the decision log widened to 34 — was the
   widening correct or did it miss some?).
5. **The L03 limitation pin (`session_tests.rs` `wp2_i04`)** — permission *revocation*
   does not propagate until re-login/`me`. This is now a deliberate, pinned decision.
   Challenge it: is "up to 12h stale permissions on high-risk commands" acceptable for
   a hospital? What did AERP Part F actually promise?
6. **Session-fixation / single-session race (`login_core`)** — `DELETE sessions WHERE
   user_id` then `INSERT` is not transactional. Under the two-login race (WP2-C01
   passes: exactly 1 row survives) — but is there an interleaving where BOTH logins
   succeed and the LAST insert wins with both tokens valid? Trace it; the test only
   counts rows, not token validity.
7. **Test-harness safety (`tests/common/mod.rs`)** — it DROPs/CREATEs `hospital_db_test`,
   redirects `ProgramData` to temp (a global `set_var`! — audit for any reader racing
   the OnceCell init), and holds the decrypted DB password in an env var. Confirm no
   path can target `hospital_db` (grep the URL construction; the db name is hardcoded
   `hospital_db_test` — verify).

---

## §5 — Deviations the tests deliberately pinned (do not "fix" without a decision)

These are **documented behavior choices**, not bugs. If you disagree, that's a review
finding — not a test-failure to repair:

1. **require_strong checks session validity, NOT permissions** (WP2-I04/L03): revocation
   of a *permission* lands on the next `me`/re-login; *deactivation*/*password reset*/
   *cross-PC login* are caught immediately. Rationale: per-command permission re-query
   would defeat the in-memory cache (AERP G.2.7 note).
2. **Low-risk commands are in-memory-only** (WP2-I06): a deactivated user's READ commands
   keep working until the session clears. Two-tier design.
3. **34 (not 22) require_strong sites** — decision-log widening, "regex caught 12 extra."
   WP2-I11 pins 34 syntactically.
4. **54 (not 55) Permission variants** — the Handoff's "55" was a miscount; 54 is correct
   (WP1-I11 asserts DB == enum exactly).
5. **LAN tests are same-process approximations** — true cross-PC needs 2 machines
   (client installer built and staged; hardware-gated). The approximation is honest:
   a "client PC session" is just a session validated server-side.
6. **WP1-I06/I09 assert guard-passage, not delivery** — the OS opener can't run in a DB
   test; the full GUI path was verified live 2026-08-31 (report §4).
7. **WP3-P01 cross-machine theft is approximated by blob corruption** — same observable
   (DPAPI key mismatch → decrypt fail), no second machine needed.
8. **Serial test execution is mandatory** (`--test-threads=1`) — mutating tests share
   one test DB; parallel runs poison each other (discovered empirically).
9. **Pool-per-test, never a shared static pool** — `#[tokio::test]` gives each test its
   own runtime; a shared pool leaves zombie connections on dead reactors (the session's
   hardest-won lesson; documented in `tests/common/mod.rs` header).
10. **`patients.rh_factor` has NO CHECK constraint** — SEC-008 spec wanted one; test
    documents the gap for the P5 schema review rather than silently passing (it passes
    today only via VARCHAR(5) length rejection of 'positive').
11. **Known-open items you should NOT spend time re-finding** (already tracked):
    license keypair still the dev keypair (no installer accepts a true production
    license); `expire_blood_units` never called by the scheduler;
    `session_invalidated` has no frontend listener; `service.rs` dead code; PHASE1_AUDIT_REPORT.md retired.

---

## §6 — Practical notes

1. **Getting the DB password (needed only for running the suite):** it is DPAPI-encrypted in
   `C:\ProgramData\HMS\config.json` (v2). Decrypt as the SAME user the app runs as:
   ```powershell
   Add-Type -AssemblyName System.Security
   [Text.Encoding]::UTF8.GetString([Security.Cryptography.ProtectedData]::Unprotect(
     [Convert]::FromBase64String((Get-Content C:\ProgramData\HMS\config.json | ConvertFrom-Json).db_password_encrypted),
     $null, [Security.Cryptography.DataProtectionScope]::LocalMachine))
   ```
   PostgreSQL is a Windows service `HMS-PostgreSQL` on 127.0.0.1:5432, SSL required.
   psql is at `C:\ProgramData\HMS\pgsql\bin\psql.exe`.
2. **Do not** run tests against `hospital_db` (production). The harness hardcodes
   `hospital_db_test` for all writes; still, review `tests/common/mod.rs:35-41` yourself.
3. **Known operational disclosure (pre-existing, flagged not fixed):**
   `C:\ProgramData\HMS\bootstrap-credentials.txt` was overwritten by the first pre-redirect
   test run on Sep 2 (stale install-time artifact; its header says delete after first login).
   The production `hospital_db` itself was verified untouched (1 user, 54 permissions).
4. **Source-of-truth docs:** `docs/VERIFICATION-REPORT-WP1-WP3.md` (Round B evidence),
   `E:\hospital-mgt\worklog.md` (both rounds' entries), the Handoff itself, and
   `documentation/aerp/Part-G-test-engineering-package.md` (the 99-test spec the
   suites implement — the spec-vs-test deviation list IS §5 above).
5. **Suggested review order** (risk-weighted): §4 items 1 → 2 → 3 → one core-extraction
   fidelity diff per pair (§3) → §4 items 4-7 → read all of §5 and challenge each pin.
   Budget: a focused pass is 3-4 hours; a full pass incl. Set A is a day.

## §7 — Review output contract

Produce findings as: `ID | severity (P0-P3) | file:line | claim | evidence (command/repro) | suggested fix`.
For each, state whether it invalidates a claim in `docs/VERIFICATION-REPORT-WP1-WP3.md`
or this package. **Explicitly attest or refute** these five, in one line each:
(a) the `save_config` merge cannot plant a password from the webview;
(b) the `.bak` file is not world-readable after migration;
(c) every core extraction is behaviorally identical to its `f2f669e` predecessor;
(d) the ACL grants cannot be abused via a forged `USERNAME`;
(e) no test or harness path can write to `hospital_db`.

---

*End of review package. Good hunting — the author genuinely wants you to find something.*
