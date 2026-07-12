# E2E Tests — VitalFlow HMS

## Overview

The E2E tests use [Playwright](https://playwright.dev/) to test the frontend
UI flows. Tauri `invoke` calls are stubbed via `page.addInitScript` so the
tests run against the Vite dev server without a real PostgreSQL backend.

## Running E2E tests

```bash
# Install Playwright browsers (first time only)
npx playwright install --with-deps chromium

# Run all E2E tests
npm run test:e2e

# Run with interactive UI mode
npm run test:e2e:ui

# Run a specific test file
npx playwright test e2e/smoke.spec.ts
```

## Test strategy

| Layer | Tool | What it covers |
|---|---|---|
| **Unit** | `cargo test` (Rust) | RBAC, sanitize_db_error, redact_log, validate_db_identifier, SQL pattern regression |
| **Component** | Vitest + Testing Library | ErrorBoundary, Pagination, formatMoney, DoctorForm, RBAC permissions |
| **E2E** | Playwright | Golden path: app loads → sidebar visible → pages render |

## Mocking Tauri

The `stubTauriInvoke` helper in `smoke.spec.ts` intercepts all
`window.__TAURI_INTERNALS__.invoke` calls and returns mock data. This lets
the frontend render without a real database or Tauri runtime.

### What's mocked:
- `get_config` → returns a server-mode config with `setup_complete: true`
- `verify_license` → returns a valid license
- `initialize_database` → returns `"server:127.0.0.1"`
- `login` → returns a super_admin session with all permissions
- `get_dashboard_kpis` → returns mock KPI data
- `get_doctors`, `get_patients`, etc. → return empty arrays

### What's NOT mocked:
- Real PostgreSQL queries (would need a test DB)
- Real TLS/pairing (would need a test server)
- Real file system operations

## Future: True desktop E2E with tauri-driver

For true end-to-end tests that exercise the full Tauri stack (Rust backend +
PostgreSQL + IPC), use [`tauri-driver`](https://docs.rs/tauri-driver) +
WebDriverIO/Playwright WebDriver protocol:

1. Install `tauri-driver` on the CI machine
2. Build the app in test mode: `cargo tauri build --features server-build`
3. Launch `tauri-driver` + the app binary
4. Connect Playwright via WebDriver protocol to `http://localhost:4444`
5. Run tests that exercise real IPC + DB

This is planned for a future testing phase once the CI environment has
PostgreSQL + Tauri build support.
