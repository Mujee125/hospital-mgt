# Local Test Execution Guide

## 1. Prerequisites

| Requirement | Install Command |
|---|---|
| Rust toolchain | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Docker | [docker.com](https://docker.com) |
| Node.js 20+ | [nodejs.org](https://nodejs.org) |
| cargo-llvm-cov (optional) | `cargo install cargo-llvm-cov` |

## 2. Frontend Tests (No Docker Required)

```bash
cd hospital-mgt-extracted/hospital-mgt
npm install
npx vitest run              # Run all tests
npx vitest run --coverage   # Run with coverage
npx tsc --noEmit            # Type check
npx eslint .                # Lint
```

**Expected result:** 107 tests pass (68 existing + 39 Blood Bank).

## 3. Rust Unit Tests (No Docker Required)

```bash
cd hospital-mgt-extracted/hospital-mgt/src-tauri
cargo test --lib
```

**Expected result:** 85 unit tests pass (state machine + enums + ABO/Rh).

## 4. Rust Integration Tests (Docker Required)

### Step 1: Start test database

```bash
cd hospital-mgt-extracted/hospital-mgt/src-tauri
docker-compose -f docker-compose.test.yml up -d
```

### Step 2: Wait for database to be ready

```bash
docker-compose -f docker-compose.test.yml exec postgres-test pg_isready -U hms_test
```

### Step 3: Set DATABASE_URL

```bash
export DATABASE_URL=postgresql://hms_test:hms_test@localhost:5433/hms_test
```

### Step 4: Run integration tests

```bash
cargo test --test integration_tests
cargo test --test concurrency_tests
cargo test --test ipc_security_tests
cargo test --test scheduler_tests
```

Or run all at once:

```bash
cargo test
```

### Step 5: Tear down

```bash
docker-compose -f docker-compose.test.yml down -v
```

## 5. Coverage (Optional)

### Rust coverage

```bash
cd src-tauri
export DATABASE_URL=postgresql://hms_test:hms_test@localhost:5433/hms_test
cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info
cargo llvm-cov --open   # Opens HTML report in browser
```

### Frontend coverage

```bash
npx vitest run --coverage
open coverage/index.html
```

## 6. Troubleshooting

### "DATABASE_URL must be set"
You forgot to export the env var. See Step 3 above.

### "Failed to connect to test database"
The Docker container may not be ready. Wait 10 seconds and retry, or check:
```bash
docker-compose -f docker-compose.test.yml ps
docker-compose -f docker-compose.test.yml logs postgres-test
```

### "Port 5433 already in use"
Another process is using port 5433. Either stop it or change the port mapping in `docker-compose.test.yml`.

### "cargo: command not found"
Install the Rust toolchain: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`

### Tests pass locally but fail in CI
Check that your local `DATABASE_URL` matches the CI service container credentials (`hms_test:hms_test`).
