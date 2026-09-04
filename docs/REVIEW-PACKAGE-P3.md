# Independent Review Package — VitalFlow HMS Authentication Remediation

**Prepared:** 2026-09-02 · **REVISED 2026-09-03** after the first independent pass
(REVIEW-FINDINGS-independent.md, findings F-1..F-6) **confirmed this package's original
scope tables were wrong.** The revision below is the corrected scope. Original author's
error, stated plainly: the WP-1..3 files were sitting **staged in the git index** while
my scoping commands ran unstaged-only diffs (`git diff` without `--cached`) against a
mixed tree — so the package described Set A as "already in the baseline" when it had
never been committed. The reviewer caught it by cloning the repo and grepping
`f2f669e:rbac.rs` (0 matches for `require_strong` / `WhatsAppSend`). Lesson now baked
into §1: **regenerate every diff table from the repo at review time; never hand-type one.**

**For:** an external reviewer with NO prior context (AI session or human)
**Repo:** `E:\hospital-mgt\hospital-mgt` (branch `main`, remote `github.com/Mujee125/hospital-mgt`)

Read this file top-to-bottom (~10 minutes), then you are reviewing, not researching.
Everything you need is in §2–§6. Do not trust this document's conclusions — that is
precisely what you exist to check. Every claim below is falsifiable against the cited
file:line or git command. **First pass findings and their dispositions are in §8.**

---

## §1 — What you are reviewing and why (CORRECTED SCOPE)

This hospital management system (Tauri v2 + Rust backend + React frontend + PostgreSQL,
Windows-only, one hospital per deployment) went through three remediation change-sets.
**All three were designed, implemented, and verified by the same AI session lineage.**
That is the governance failure you are here to repair (Handoff §13.6). Your job is NOT
to re-derive the work — it is to find the bypass, the edge case, and the omission the
author could not see in their own work.

### The true commit map (verified 2026-09-03 with `git show`/`git diff` per commit)

| Commit | Date | Contents | Review status |
|---|---|---|---|
| `2218ea5` | Jul 12 | Initial commit — pre-AERP application, no `require_strong`, no WhatsApp permissions, no `secrets.rs` | true baseline |
| `f2f669e` | Aug 18 | Mixed user commit — does **NOT contain Set A** (verified: `git show f2f669e:src-tauri/src/rbac.rs \| grep -c require_strong` → 0) | — |
| `abbc01b` | Sep 2 23:56 | **Set A: the ENTIRE WP-1..3 authorization surface** — `rbac.rs` (+143: `require_strong`, `token_hash`, WhatsApp permissions), `secrets.rs` (+190, whole file), all 34 guard call-sites across 7 `commands/*.rs` + `whatsapp/commands.rs` + `auth.rs`, frontend RBAC (`rbac.ts`, Login/Patients/Setup pages) | **ZERO independent review to date** — this, not Set B/C, is the largest unaudited change-set in the repo |
| `795b418` | Sep 3 | **Sets B+C**: Windows-verification fixes (VF-VERIF-001..004) + test round (cores, 3 new suites, 4 repaired legacy suites, `.bak`+version-gate fixes) | reviewed pass 1 (partially) and pass 2 (targeted) |
| `7f630e5` | Sep 3 | `.mimosa/` tool-state artifacts + this package's `.bak` edits | now untracked (F-3 remediated) |
| `d31d3b8` | Sep 3 | user commit (`.mimosa` deletion) | hygiene |
| `d77d09a` | Sep 4 | pass-1 F-4/F-5 fixes to `config.rs` (loopback pin, ACL-before-rename) | committed without package update — the pass-2 meta-finding |
| *(later)* | — | the repo MAY have moved again past this point. **This table is inherently stale.** | — |

### Scope — by DESIGN there are no hand-typed diff numbers in this package anymore.

Pass 2 of the review demonstrated why: both pass-1 and pass-2 of this package printed
diff statistics that were wrong the moment code (or even a `.mimosa` artifact) moved.
The stop-rule fired correctly both times — so now the rule IS the scope section:

**Step 0 (mandatory, before anything else):**
```bash
git fetch origin && git log --oneline -8                  # what exists NOW?
git diff --stat 2218ea5..HEAD -- src-tauri/src src-tauri/tests src docs   # true scope
```
Then read the per-commit table above against what you see. Anything after the last
commit listed here is UNREVIEWED — treat it as a new change-set needing its own pass.
The set composition (what to look for in each file) lives in §2/§3 and remains valid;
only the commit hashes and counts drift.

**Reproduce the test suite before reading any code** (~30 min, gives you ground truth):
```bash
cd E:\hospital-mgt\hospital-mgt\src-tauri
# <pw> = the DPAPI-decrypted db password; see §6 note if you can't read it
HMS_TEST_DB_URL="postgresql://postgres:<pw>@127.0.0.1:5432/postgres?sslmode=require" \
  cargo test --features hms-integration-tests --tests -- --test-threads=1
```
As of the 2026-09-04 pass-2 fixes: **212 passing, authoritatively re-run 2026-09-04**
(110 lib incl. pass-2 regressions, 14 config, 20 WP-1, 28 WP-2, 40 legacy).
Re-derive the count from your own run — do not trust this number either. Gates: `cargo check --lib` 0 errors (both
feature paths), `cargo clippy --lib` 0 warnings, `npx tsc --noEmit` / `npx eslint .`
exit 0, `npx vitest run` passes.
The suite runs against an isolated `hospital_db_test` it creates itself; production
`hospital_db` is never touched (verified post-run; see §6 note 3).
Build-environment warning: do NOT build while VS Code's rust-analyzer is running —
it races cargo on the same target dir and produces phantom E0463/E0786 errors.

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
| (C-4) | Spec behaviors missing: no `.bak` on v1→v2 migration (WP3-I05), no unknown-version rejection (WP3-N04). | `config.rs:134-146` (>2 → None); `config.rs:226-262` (.bak only when disk is v1 AND no .bak exists) | **UPDATE 2026-09-02, during package prep: the `.bak` plaintext-ACL leak was CONFIRMED and FIXED.** Evidence: `C:\ProgramData\HMS` grants inherited `BUILTIN\Users:(OI)(CI)(M)` — a plain `fs::copy` .bak left the plaintext v1 DB password modifiable by every local user. Fix in `config.rs:240-261`: the .bak now gets the same icacls hardening as config.json (SYSTEM/Admins full + current-user read). **Your checks:** run `wp3_u10`, then `icacls` the .bak in the test temp dir; confirm a FAILED icacls doesn't block the save (best-effort is intentional — is that acceptable for a plaintext holder?); confirm the read-only user grant suffices for any legitimate .bak consumer. Also: the version gate `version > 2` — what about non-numeric `config_version` (as_u64 → None → treated as v1; right vs corrupt?)? |

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
| `auth.rs:745 update_user_core` | `update_user` (emits event when `is_active==Some(false) \|\| roles.is_some()` — **note the wrapper pre-computes `emits` before the call; verify no ordering bug**) | role sync, audit (pass-3 correction: there is NO cannot-target-self rule here — only `delete_user` has one; fidelity itself held) |
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
2. **The `.bak` plaintext leak (`config.rs:226-262`)** — **RESOLVED during package
   preparation**: the leak was confirmed real (the HMS dir's inherited `Users:(OI)(CI)(M)`
   ACE applies to new files, so the plain-`fs::copy` .bak left the plaintext v1 DB
   password modifiable by any local user) and fixed at `config.rs:240-261` with the
   same icacls hardening as config.json. Verify the fix end-to-end — the checks are in
   §2 (C-4). If the icacls ordering, quoting, or best-effort semantics are wrong
   anywhere, that is a genuine finding.
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
   **UPDATED (pass-3, P3-10):** the sanctioned admin path now CLOSES the revocation
   window — `update_user_core` deletes the target's session rows on any role change,
   forcing immediate re-login (test: `rev3_p3_10_role_change_sweeps_target_sessions`).
   Direct SQL edits by a DBA remain outside the app's control (a DBA can delete
   sessions by definition); that residual is accepted.
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
(b) the `.bak` fix at `config.rs:240-261` genuinely hardens the .bak on migration
    (the pre-fix plaintext leak was real — run `wp3_u10` and `icacls` the .bak);
(c) every core extraction is behaviorally identical to its `f2f669e` predecessor;
(d) the ACL grants cannot be abused via a forged `USERNAME`;
(e) no test or harness path can write to `hospital_db`.

---

## §8 — First-pass findings & dispositions (ADDRESSED 2026-09-03)

The first independent pass (Claude, cloned repo, did not take this package's tables at
face value — exactly as instructed) returned F-1..F-6. Dispositions below; **the
reviewer's own "what I'd do next" list items 1–2 are done, and their open items are
folded into the re-review scope.**

| ID | Severity | Verdict | Disposition |
|---|---|---|---|
| F-1 | P0 | **CONFIRMED** (verified locally: `git show f2f669e:rbac.rs \| grep -c require_strong` → 0; rbac.rs only touched by `2218ea5` and `abbc01b`) | Package §1 rewritten with the true commit map. `require_strong` + WP-1..3 promoted from "recommended" to **primary review scope** (it IS the scope now). Root cause documented in the revision header: staged-index vs unstaged-diff scoping error. |
| F-2 | P1 | **CONFIRMED** (real `f2f669e..HEAD` = 183 files; the ~66k "extra" lines are the F-3 artifacts) | §1 now mandates regenerating the diff-stat from the repo at review time, with a stop-and-rescope rule if numbers disagree. No hand-typed stats remain in the package. |
| F-3 | P2 | **CONFIRMED, remediated** | `.mimosa/` + `**/.mimosa/` added to `.gitignore`; 144 files untracked via `git rm -r --cached`. **All `task-review-*.json` and finding-ledger artifacts verified EMPTY** (`"run_status": "inconclusive"`, 0 findings, 0 files scanned — the hook never established a baseline), so the "unreviewed second findings set" concern resolves to nothing. History purge remains optional (the files contain no secrets — reviewer grepped; only tool telemetry). |
| F-4 | P2 | Accepted with caveat | **Partially hardened:** `save_config` now pins `db_host` to loopback during the pre-`setup_complete` window on the server build (SRS: first-run setup talks to the locally provisioned PostgreSQL). The authenticated/steady-state whitelist was already safe (passwords excluded from merge). Residual: the client-build pre-setup window remains open by design (client Setup must accept a server IP) — flagged as a documented trade-off, not silently accepted. |
| F-5 | P3 | **Accepted & fixed** | ACL now applied to the TEMP file **before** the rename (`config.rs` save_to_inner): the live config.json never carries inherited ACEs, not even briefly. Config suite 14/14 re-passed; both feature paths compile. |
| F-6 | info | Count confirmed (34) | Reviewer correctly notes the count is syntactic, not semantic. **Their suggested spot-check (right permission per command) is now §4 item 4's explicit task** — still open. |

**Attestation status from first pass:** (a) mostly-true-with-caveat (F-4; now hardened), (b) code-level-yes/runtime-unverified (**runtime verification exists: 2026-09-02 on real Windows — migrated .bak icacls showed only `user:(R)/Administrators:(F)/SYSTEM:(F)`, documented in worklog 2026-09-02**), (d) no-abuse-vector/moderate-confidence, (c) and (e) **open**.

**Re-review scope (second pass):** §3 core-extraction fidelity, §4 items 4–7
(guard-site semantics, L03 staleness, `login_core` race, harness DB isolation),
**plus the newly promoted Set A** (`rbac.rs`, `secrets.rs`, guard sites — the P0),
using the corrected `2218ea5..HEAD` diff from §1.

---

## §9 — Second-pass findings & dispositions (ADDRESSED 2026-09-04)

Pass 2 (fresh external reviewer, cloned repo, targeted §4 pass) returned one meta-finding
and two code findings. Both code findings **confirmed and fixed**; the meta-finding
triggered the §1 redesign above.

| ID | Severity | Verdict | Disposition |
|---|---|---|---|
| Meta | — | **Accepted** — the package's own stop-rule fired: rev 2's hand-typed stats (claimed ~26 files +1,843/−1,534; actual 33 files +3,488/−810) were stale because the pass-1 F-4/F-5 fixes were committed (`d77d09a`) after rev 2 was written. Symptom fixed twice, cause reproduced twice. | **§1 redesigned: no hand-typed diff numbers remain in this package.** Scope is now the per-commit table (which self-declares its own staleness) + the mandatory regenerate-first rule. |
| F-1 (P2-1) | **P0** | **Confirmed** — `save_config` / `repair_server_config` / `clear_config` failed OPEN whenever `SessionState == None` (the post-logout state) on a configured machine: no permission check, no audit row. A tampered webview could unset `setup_complete` or swap the pinned TLS cert; `repair_server_config` could additionally replace the DB credential. No test covered it. | **Fixed:** new fail-closed gate `rbac::require_config_mutation` (rbac.rs) wired into all three commands. Once `setup_complete == true`, config mutation REQUIRES a signed-in `SettingsManage` session — no-session is denied outright. The unauthenticated path exists ONLY while setup has never completed (genuine first run). Recovery path preserved: an operator with file-admin rights can delete the corrupt config.json (ACL-hardened against non-admins) to re-enter first-run setup. Regression tests added (fail-closed configured + no-session → deny; first-run + no-session → allow; configured + session-without-permission → deny; first-run + signed-in low-priv user → deny). |
| F-2 (P2-2) | **P1** | **Confirmed** — `create_prescription` was guarded by `PatientsCreate`, which both doctor AND receptionist hold → **receptionists could write prescriptions**. The §4.4 "syntactic not semantic" warning, concretely instantiated. | **Fixed:** new `PrescriptionsCreate` permission (`prescriptions.create`), granted to doctor + super_admin only; guard swapped in `pharmacy.rs`; frontend `rbac.ts` mirrored; regression tests (receptionist/nurse/pharmacist lack it; doctor has it). |

**Attestation deltas from pass 2:** (a) is now the *right* question answered correctly —
the merge whitelist was always safe; the AUTH GATE was not, and now fails closed;
(c) `login_core` spot-checked (lockout/dummy-verify/session-DELETE intact) — full
six-pair fidelity diff still open; (d), (e) still open from pass 1.

**Remaining open for pass 3:** Set A semantic review (§8 re-review scope), the full
core-extraction fidelity diff (§3), §4 items 4–7 semantics, forged-`USERNAME` depth-test
((d)), and harness URL-construction grep ((e)). Also: no integration test yet pins
`create_prescription` end-to-end under a receptionist session — the fix is unit- and
guard-tested, but the command-level wiring is only covered by the wp2_i11 source
inventory and manual inspection.

---

## §10 — Third-pass findings & dispositions (ADDRESSED 2026-09-04)

Pass 3 (fresh external reviewer, fresh clone, kernel-verification discipline: checked
whether the "fixed" symbols exist at PUBLISHED HEAD before reviewing anything else).
Outcome: the publication failure was the headline; two genuine bypasses were found in
the pass-2 fix itself; Set A (the long-unaudited authorization core) was finally
deep-reviewed and **held up sound**.

**THE PUBLICATION RULE (new, permanent):** no disposition in this package may say
"Fixed" unless the symbol is reachable from `origin/main`. A fix that exists only as
working-tree changes is NOT fixed — it is a promise. **P3-1 applies to this very
section until the maintainer commits and pushes the pass-3 fix-set.**

| ID | Severity | Verdict | Disposition |
|---|---|---|---|
| P3-1 | P0 (process) | Confirmed | Pass-2 fixes were uncommitted working-tree changes while §9 said "Fixed." Fix-set now includes pass-3 changes; the maintainer MUST commit+push (commands provided in the worklog entry). This rule added to the package. |
| P3-2 | P0 (live at HEAD) | Confirmed | Pass-2 P0 re-confirmed at published HEAD; fixed in working tree since pass-2, hardened further by P3-4; becomes live for cloners the moment the fix-set is pushed. |
| P3-3 | P1 (live at HEAD) | Confirmed | Pass-2 P1 re-confirmed at published HEAD; same publication status as P3-2. |
| P3-4 | P1 (new) | **Confirmed & fixed** | The pass-2 gate treated an unreadable/corrupt config as "first run" (load→None→unwrap_or(false)), re-opening the fail-open hole one level down. **Fix: tri-state `ConfigDiskState` (Missing/Corrupt/Active) — Corrupt now fails closed** (requires an authenticated SettingsManage session). Gate grants via `ConfigMutationGrant` enum (removes the sentinel-session hack, per P3-6). Unit tests cover all disk-state × session-state combinations. |
| P3-5 | P1 (new) | **Confirmed & fixed** | Loopback pin computed its window from `merged` — which IS the raw payload when no disk config exists, so a payload of `{setup_complete: true, db_host: remote}` skipped the pin. **Fix: window now derived from disk state only** (`!matches!(disk, Active{setup_complete:true})`); Missing and Corrupt both enforce the pin. |
| P3-7 | P2 (new) | **Confirmed & fixed** | Single-session invariant was application-only; concurrent DELETE+INSERT could leave two valid tokens. **Fix: schema-enforced** — migration dedupes then creates `UNIQUE INDEX idx_sessions_single_user ON sessions(user_id)`; `login_core` now rotates the session with one atomic `INSERT … ON CONFLICT (user_id) DO UPDATE`. Tests: `rev3_p3_7_unique_user_session_schema_enforced`, `rev3_p3_7_upsert_replaces_prior_token`. |
| P3-8 | P2 (new) | **Confirmed & fixed** | Harness `test_db_url()` dropped `?sslmode=require`. Query string now preserved in the URL rewrite. |
| P3-9 | P2 (new) | **Confirmed & fixed** | Stale "contains the plaintext DB password" comment on the config ACL (the v2 blob is DPAPI LOCAL_MACHINE — decryptable by any local process; the ACL's real value is tamper-integrity; the `.bak` ACL is the confidentiality-load-bearing one). Comment corrected; package updated here. |
| P3-10 | P2 | **Confirmed & fixed** | L03 window closed on the sanctioned path: `update_user_core` deletes the target's sessions on role change (test: `rev3_p3_10_role_change_sweeps_target_sessions`). DBA-level SQL edits remain out of scope (documented). §5 deviation #1 updated. |
| P3-11 | P3 | **Confirmed & fixed** | `Pharmacy.tsx canPrescribe` now checks `PrescriptionsCreate`. |
| P3-12 | P3 | Confirmed | §3 fidelity row corrected (no self-target rule in `update_user_core`); fidelity itself held. |
| P3-6 | P2 (nits) | Addressed | Gate redesigned with `ConfigMutationGrant` enum: sentinel `user_id != 0` and fabricated "system-setup" session eliminated; single mutex read. |

**Set A verdict (the reason this pass existed): SOUND.** 54-permission model, roles
matrix, all 34 guard sites re-counted and semantically spot-checked (one wrong pairing
— the known P1), DPAPI implementation correct with no further windows-rs misuse, error
paths sound, harness isolation real. **§7 attestations: (a) PASS at the merge layer,
FAIL at the auth layer at published HEAD (now fixed in working tree — publish it!);
(b) PASS; (c) PASS — all six pairs faithful modulo four documented intended deltas;
(d) PASS (with two non-exploitable caveats documented); (e) PASS (sslmode caveat fixed).**

**Open for pass 4 (shrinking):** verify the pushed repo matches this document (P3-1
closure), a command-level receptionist-cannot-prescribe IPC test, and the standing
hardware-gated items (2-PC LAN, DPAPI sysprep-class).

---

*End of review package (rev 4, 2026-09-04). Good hunting — the author genuinely wants you to find something.*
