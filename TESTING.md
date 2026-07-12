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
