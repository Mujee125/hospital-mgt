# CI/CD Guide

## 1. Pipeline Overview

File: `.github/workflows/ci.yml`

### Triggers
- Push to `main` or `develop`
- Pull request to `main` or `develop`

### Jobs

| Job | Purpose | Requires DB | Runner |
|---|---|---|---|
| `rust-quality` | fmt + clippy + unit tests | No | ubuntu-latest |
| `rust-integration` | Integration + concurrency + scheduler + security | Yes (PostgreSQL service) | ubuntu-latest |
| `rust-coverage` | cargo-llvm-cov | Yes (PostgreSQL service) | ubuntu-latest |
| `frontend-quality` | tsc + eslint + vitest --coverage + vite build | No | ubuntu-latest |
| `ci-success` | Branch-protection gate (all above must pass) | No | ubuntu-latest |

## 2. PostgreSQL Service Container

The `rust-integration` and `rust-coverage` jobs use a GitHub Actions service container:

```yaml
services:
  postgres:
    image: postgres:16-alpine
    env:
      POSTGRES_USER: hms_test
      POSTGRES_PASSWORD: hms_test
      POSTGRES_DB: hms_test
    ports:
      - 5432:5432
    options: >-
      --health-cmd "pg_isready -U hms_test"
      --health-interval 5s
      --health-timeout 5s
      --health-retries 10
```

## 3. Artifacts Uploaded

| Artifact | Content | Retention |
|---|---|---|
| `rust-coverage-lcov` | `lcov.info` | 30 days |
| `frontend-coverage` | `coverage/` directory | 30 days |
| `dist` | Vite build output | 30 days |

## 4. Local CI Simulation

To run the same checks locally before pushing:

```bash
# Rust
cd src-tauri
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --lib
# Integration tests (requires Docker)
docker-compose -f docker-compose.test.yml up -d
export DATABASE_URL=postgresql://hms_test:hms_test@localhost:5433/hms_test
cargo test --test integration_tests --test concurrency_tests --test ipc_security_tests --test scheduler_tests
docker-compose -f docker-compose.test.yml down -v

# Frontend
cd ..
npx tsc --noEmit
npx eslint .
npx vitest run --coverage
npx vite build
```

## 5. Failure Handling

- Any job failure blocks the pull request merge (via `ci-success` gate)
- Failed jobs upload logs automatically (GitHub Actions UI)
- Coverage artifacts are uploaded even on failure (for debugging)
