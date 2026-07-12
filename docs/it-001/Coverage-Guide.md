# Coverage Guide

## 1. Rust Coverage

### Tool: cargo-llvm-cov

```bash
# Install
cargo install cargo-llvm-cov

# Run with LCOV output
cd src-tauri
cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info

# View HTML report
cargo llvm-cov --open
```

### CI Integration
The GitHub Actions workflow (`ci.yml`) installs cargo-llvm-cov and runs coverage automatically. The `lcov.info` artifact is uploaded for integration with code-coverage services (Codecov, Coveralls).

## 2. Frontend Coverage

### Tool: Vitest v8 provider

Configured in `vitest.config.ts`:
```typescript
coverage: {
  provider: "v8",
  reporter: ["text", "html", "lcov", "cobertura"],
  reportsDirectory: "./coverage",
  include: ["src/pages/BloodBank.tsx", "src/lib/queries.ts", ...],
}
```

### Run locally
```bash
npx vitest run --coverage
open coverage/index.html
```

## 3. Current Coverage Status

**NOT MEASURED** — no coverage tool has been executed in this environment.

| Layer | Tool | Status |
|---|---|---|
| Rust line coverage | cargo-llvm-cov | NOT MEASURED |
| Rust branch coverage | cargo-llvm-cov | NOT MEASURED |
| Frontend line coverage | vitest --coverage | NOT MEASURED |
| Frontend branch coverage | vitest --coverage | NOT MEASURED |

## 4. Target Thresholds (Future)

| Layer | Current Target | Enforcement |
|---|---|---|
| Rust overall | 70% lines | P1 goal (not yet enforced) |
| State machine | 100% lines | P0 (achieved by 24 unit tests) |
| Enum validation | 100% lines | P0 (achieved by 33 unit tests) |
| ABO compatibility | 95% lines | P0 (achieved by 14 unit tests) |
| issue_blood | 80% lines | P1 (requires integration tests) |
| Frontend overall | 70% lines | P1 goal |

## 5. Mutation Testing (Future)

**Tool:** cargo-mutants (not yet installed)

```bash
# Install
cargo install cargo-mutants

# Run
cd src-tauri
cargo mutants
```

**Status:** NOT EXECUTED. Mutation testing is documented as a P2 goal in the technical debt register.
