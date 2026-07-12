# IT-001 — Enterprise Integration Testing Infrastructure

## What This Is

A complete, reusable test infrastructure for VitalFlow HMS that makes every clinical module automatically verifiable. Built for the Blood Bank module (BB-BASELINE-1.0) but designed for all future modules.

## Quick Start

### Frontend tests (no Docker needed)
```bash
npm install
npx vitest run
```

### Rust unit tests (no Docker needed)
```bash
cd src-tauri
cargo test --lib
```

### Rust integration tests (Docker needed)
```bash
cd src-tauri
docker-compose -f docker-compose.test.yml up -d
export DATABASE_URL=postgresql://hms_test:hms_test@localhost:5433/hms_test
cargo test
docker-compose -f docker-compose.test.yml down -v
```

## Files in This Package

| File | Description |
|---|---|
| `IT-001-Test-Infrastructure.md` | Main infrastructure document |
| `Integration-Test-Plan.md` | Test plan with all test cases |
| `Coverage-Guide.md` | How to measure coverage |
| `CI-CD-Guide.md` | GitHub Actions pipeline |
| `Local-Test-Execution.md` | Step-by-step local execution |
| `docker-compose.test.yml` | Test PostgreSQL environment |
| `ci.yml` | GitHub Actions workflow |
| `Cargo.toml` | Dev-dependencies added |
| `tests/` | All test source files |

## Test Count

| Layer | Tests | Status |
|---|---|---|
| Rust unit (BB-007) | 85 | NOT EXECUTED (no toolchain) |
| Rust integration | 15 | NOT EXECUTED |
| Rust concurrency | 4 | NOT EXECUTED |
| Rust security | 12 | NOT EXECUTED |
| Rust scheduler | 9 | NOT EXECUTED |
| Frontend (BB-007) | 39 | ✅ PASS |
| **Total** | **164** | **39 executed, 125 not executed** |

## Honest Reporting

No test results were invented. The 39 frontend tests were executed and passed. The 125 Rust tests are written as source code but require a Rust toolchain + Docker to execute. See each test file's header comment for status.
