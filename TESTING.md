# Testing Strategy — VitalFlow HMS

## Overview

VitalFlow HMS uses a three-layer testing strategy aligned with the SDLC
(ISO 12207) and Quality Model (ISO 25010):

| Layer | Tool | Tests | What it covers |
|---|---|---|---|
| **Unit (Rust)** | `cargo test` | 47 | RBAC permissions, sanitize_db_error, redact_log, validate_db_identifier, SQL pattern regression for race-condition fixes |
| **Component (TS)** | Vitest + Testing Library | 78 | ErrorBoundary, Pagination, formatMoney, DoctorForm, RBAC permissions |
| **E2E** | Playwright | 3 | Golden path: app loads → sidebar visible → pages render |

## Running tests

### Frontend tests (Vitest)

```bash
# Run all unit/component tests (one shot)
npm test

# Watch mode (re-runs on file change)
npm run test:watch
```

### Rust unit tests

```bash
cd src-tauri
cargo test --features server-build --all-targets
```

### Windows-only prerequisite: the doctor-scoping integration suite

> **Read this before claiming a Windows test run is green.**
> `rctf_doctor_scoping_tests` **cannot start on Windows without the manual
> manifest step below.** Without it the process dies at load time and Cargo
> reports `exit code: 0xc0000139, STATUS_ENTRYPOINT_NOT_FOUND` — which is *not*
> a test failure, because zero tests execute.

#### Symptom

```
     Running tests\rctf_doctor_scoping_tests.rs (...deps\rctf_doctor_scoping_tests-<hash>.exe)
error: test failed, to rerun pass `--test rctf_doctor_scoping_tests`

Caused by:
  process didn't exit successfully: `...rctf_doctor_scoping_tests-<hash>.exe --test-threads=1`
  (exit code: 0xc0000139, STATUS_ENTRYPOINT_NOT_FOUND)
note: test exited abnormally; to see the full output pass --no-capture to the harness.
```

Windows may also pop: *"The procedure entry point TaskDialogIndirect could
not be located in the dynamic link library ...\<test>.exe"*.

#### Why it happens

1. `tests/rctf_doctor_scoping_tests.rs` transitively links the Tauri code that
   imports **`TaskDialogIndirect`** from **`comctl32.dll`**. It is the only test
   target that does — the other 18 integration binaries do not import
   `comctl32.dll` at all, which is why they run fine.
2. The **production `hospital-mgmt.exe` already embeds the correct manifest.**
   `tauri_build::build()` in `src-tauri/build.rs` writes a Windows resource that
   declares a dependency on `Microsoft.Windows.Common-Controls` version 6.0,
   plus the app icon and version info. The shipping app is correct.
3. **Cargo does not give `#[test]` integration targets that resource.** Tauri
   links its `.rc` into the *binary* targets only, so a test binary has no
   application manifest.
4. `TaskDialogIndirect` is exported **only** by the comctl32 **v6**
   side-by-side assembly, and the loader activates that assembly *exclusively*
   through the manifest dependency in step 2. With no manifest, Windows resolves
   `comctl32.dll` to the v5-compatible interface in `System32` (5.82 on this
   machine), which does **not** export the symbol — so the loader fails before
   `main` runs.

#### The prerequisite (Windows only)

Place a Common Controls v6 manifest next to the test executable. The
executable name carries a content hash, so glob for it:

```powershell
# 1. Remove STALE copies of this test binary. Cargo keeps previous
#    content-hashed builds in target\debug\deps, and if more than one
#    rctf_doctor_scoping_tests-*.exe is present a glob can pick the wrong
#    one -- you would then attach the manifest to a binary nothing runs.
cd src-tauri
Remove-Item target\debug\deps\rctf_doctor_scoping_tests-* -ErrorAction SilentlyContinue

# 2. Build the suite so exactly one test binary exists.
cargo test --features hms-integration-tests --test rctf_doctor_scoping_tests --no-run

# 3. Write the manifest beside that single binary.
$exe = (Get-ChildItem target\debug\deps -Filter 'rctf_doctor_scoping_tests-*.exe').FullName
@"
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity type="win32" name="hospital-mgmt" version="0.1.0.0" processorArchitecture="*"/>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
  </dependency>
</assembly>
"@ | Set-Content -Path "$exe.manifest"

# 4. Now run the suite (13 tests).
cargo test --features hms-integration-tests --test rctf_doctor_scoping_tests
```

The `<hash>` in the executable name changes on rebuild, so **re-create the
file after any rebuild** — and be aware that a plain `cargo test --tests` will
rebuild this target and may produce a *new* hash, leaving the manifest
stranded on the old binary. If you see `STATUS_ENTRYPOINT_NOT_FOUND` after a
full `--tests` run, redo steps 1-3. Delete the file when done:

```powershell
Remove-Item target\debug\deps\rctf_doctor_scoping_tests-*.exe.manifest
```

#### Scope and safety

* **Test binary only.** The file sits beside a *test* executable in
  `target/debug/deps/`. It does **not** modify `hospital_mgmt.exe`, the
  application manifest, `src-tauri/build.rs`, `Cargo.toml`, or any dependency.
* **No Windows system files are touched.** Nothing is written to
  `%ProgramData%`, `System32`, or the WinSxS store; Administrator rights are
  not required.
* **The production database is never involved** by this step.
* The manifest is regenerated on every clean build, so a fresh checkout is
  unaffected — it is a per-developer-machine step, not a committed artifact.
  Nothing is written into the repository.

#### CI is unaffected

Every job in `.github/workflows/ci.yml` runs on **`ubuntu-latest`**, where
`comctl32.dll` and the manifest do not exist at all — the issue is strictly
Windows-local. Linux CI compiles and runs this suite with no extra step.

#### Why this is documented rather than automated

An automatic fix was attempted and deliberately reverted: adding `winres` to
`build.rs` collides with the resource `tauri_build` already embeds, and the
linker rejects the result —

```
CVTRES : fatal error CVT1100: duplicate resource.  type:VERSION, name:1, language:0x0409
LINK  : fatal error LNK1123: failure during conversion to COFF
```

Because a Cargo build script has no way to target *only* test targets, any
package-wide manifest change also lands on the production binaries. Rather than
risk the shipping executable to save a local convenience, the prerequisite is
documented instead. See "What's NOT covered" below for the tracked follow-up.

### E2E tests (Playwright)
```bash
# Install browsers (first time only)
npx playwright install --with-deps chromium

# Run E2E tests
npm run test:e2e

# Interactive UI mode
npm run test:e2e:ui
```

## CI integration

The CI pipeline (`.github/workflows/ci.yml`) runs on every push + PR to `main`:

| Job | Steps |
|---|---|
| **frontend** | `npm install` → `typecheck` → `lint` → `vitest run` → `npm audit` → `vite build` |
| **backend** | Tauri Linux deps → `cargo check --features server-build` → `cargo clippy -D warnings` → `cargo test` → `cargo audit` |
| **keygen** | `cargo check --all-targets` |
| **security** | `npm audit --omit=dev --audit-level=high` + `cargo audit` (parallel, `continue-on-error`) |

## What's covered

### Rust unit tests (47 tests)

| File | Tests | Coverage |
|---|---|---|
| `rbac.rs` | 17 | permissions_for_role (4 roles), require/require_session/require_if_session (6 branches), Permission enum invariants |
| `lib.rs` | 13 | redact_log (password/db_user/user/username/db_password + boundary + IPs + case) |
| `db.rs` | 6 | sanitize_db_error (SEC-18), validate_db_identifier (SEC-10) |
| `commands/queue.rs` | 5 | CR-6 LOCK TABLE atomic, CR-7 FOR UPDATE OF q, CR-8 param count, status state machine |
| `commands/ipd.rs` | 6 | SDD §8.1 conditional UPDATE, double-admission prevention, discharge-frees-bed, FUN-09 unpaid-bills guard |

### Frontend component tests (78 tests)

| File | Tests | Coverage |
|---|---|---|
| `ErrorBoundary.test.tsx` | 7 | Normal render, error catch, error reference ID, reload/continue buttons |
| `shared.test.tsx` (Pagination) | 14 | Page navigation, rows-per-page, disabled states, item counts |
| `utils.test.ts` (formatMoney) | 26 | Number/string/null/NaN/Infinity inputs, PKR formatting |
| `rbac.test.ts` | 17 | PERMISSIONS keys, ROLE_LABELS, permission checks |
| `DoctorForm.test.tsx` | 14 | Field rendering, validation, submit, loading state |

### E2E smoke tests (3 tests)

| Test | What it verifies |
|---|---|
| `app loads and shows login screen` | App title + initial render |
| `sidebar shows all navigation items` | Boot flow → sidebar nav items visible |
| `error boundary catches render errors` | No white screen on error |

## What's NOT covered (future plans)

1. **True desktop E2E with tauri-driver** — The current E2E tests stub Tauri
   `invoke` calls. True end-to-end tests (real Postgres + real IPC) require
   `tauri-driver` + WebDriver protocol on the CI machine.

2. **IPC integration tests** — Tests that verify each Tauri command's RBAC
   + input validation against a real PostgreSQL test database. Would use
   `sqlx::test` or a test container.

3. **DPAPI encryption for config.json** — The password field is ACL-hardened
   but not DPAPI-encrypted (deferred from Batch 5). Needs a Windows test
   environment.

4. **`set_token_status` state-machine guard** — The queue token status
   transition doesn't enforce waiting→in-progress→completed (B6-A finding).
   Needs a hardening pass + test.

5. **Frontend `PERMISSIONS` vs Rust `Permission` drift** — Frontend has 35
   entries, Rust has 37 (B6-B finding). Needs reconciliation.

6. **Playwright E2E in CI** — Currently local-only; CI integration requires
   Chromium browser binary (~200 MB) + longer timeout.

7. **Automatic Windows manifest for test binaries** —
   `rctf_doctor_scoping_tests` currently needs the manual
   Common Controls v6 manifest step documented under
   "Windows-only prerequisite: the doctor-scoping integration suite" above.
   A proper fix is to make the test target receive the same manifest the
   application already embeds, without disturbing the production binary. The
   obvious route (`winres` in `build.rs`) was tried and reverted: it collides
   with the resource `tauri_build` already emits
   (`CVT1100: duplicate resource ... VERSION/1`), and Cargo provides no build-script
   hook to scope an embed to test targets only. Needs a Tauri-level or
   Cargo-level answer. Not a shipping risk — it affects local Windows test
   runs only, and Linux CI is unaffected.
