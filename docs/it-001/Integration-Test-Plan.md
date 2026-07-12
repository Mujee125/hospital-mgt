# Integration Test Plan

## 1. Test Scope

| Area | Tests | P0 Coverage |
|---|---|---|
| BE-02 Expiry rejection | IT-001 | ✅ |
| BE-03 Screening rejection | IT-002 | ✅ |
| BE-04 ABO/Rh compatibility | IT-003 | ✅ |
| BE-05 Scheduler auto-expiry | IT-004 | ✅ |
| BE-06 Quarantine workflow | IT-005, IT-006 | ✅ |
| BE-08 State machine | IT-007 | ✅ |
| BE-09 Return clears fields | IT-008 | ✅ |
| Traceability | IT-009 | ✅ |
| Migration idempotency | IT-010 | ✅ |
| CHECK constraints | IT-011 | ✅ |
| FK RESTRICT | IT-012 | ✅ |
| Compatibility matrix seed | IT-013 | ✅ |
| Sequence distinctness | IT-014 | ✅ |
| Soft delete | IT-015 | ✅ |

## 2. Concurrency Tests

| Test | Scenario |
|---|---|
| CC-001 | Double issue — only one succeeds |
| CC-002 | Double reservation — only one succeeds |
| CC-003 | Issue vs scheduler expiry |
| CC-004 | 10 parallel donations — all succeed |

## 3. Security Tests

| Test | Scenario |
|---|---|
| SEC-001 | CHECK rejects invalid status |
| SEC-002 | CHECK rejects invalid rh_factor |
| SEC-003 | SQL injection in search (parameterized) |
| SEC-004 | Soft-deleted unit inaccessible |
| SEC-005 | UNIQUE constraint on unit_number |
| SEC-006 | Volume=0 rejected |
| SEC-007 | Volume=1 accepted (boundary) |
| SEC-008 | patients.rh_factor CHECK |
| SEC-009 | patients.rh_factor NULL accepted |
| SEC-010 | FK nonexistent donor |
| SEC-011 | Volume=600 accepted (max boundary) |
| SEC-012 | Volume=601 rejected (over max) |

## 4. Scheduler Tests

| Test | Scenario |
|---|---|
| SCH-001 | Expire available unit |
| SCH-002 | Expire quarantined unit |
| SCH-003 | Transfused NOT touched (terminal) |
| SCH-004 | Discarded NOT touched (terminal) |
| SCH-005 | No-op when no expired units |
| SCH-006 | Multiple expiries in one UPDATE |
| SCH-007 | Idempotency (second run is no-op) |
| SCH-008 | Boundary: expiry at ~NOW() |
| SCH-009 | Future-dated unit NOT expired |

## 5. Execution Status

**ALL TESTS: NOT EXECUTED** — require PostgreSQL test DB + Rust toolchain.

## 6. Preconditions

1. Docker installed and running
2. Rust toolchain installed (`rustup`)
3. `docker-compose -f src-tauri/docker-compose.test.yml up -d`
4. `export DATABASE_URL=postgresql://hms_test:hms_test@localhost:5433/hms_test`
5. `cd src-tauri && cargo test`
