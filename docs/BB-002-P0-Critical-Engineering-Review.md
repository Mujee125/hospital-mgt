# BB-002 — Blood Bank Module P0 Critical Engineering Review
**Independent Verification & Validation (IV&V)**
**Phase 2-E · SRS FR-0145–FR-0149**
**Version 1.0**

---

## 1. Executive Summary

The Independent Engineering Review Board performed a complete adversarial review of the Blood Bank module implementation against the actual source code. The review attempted to break every clinical workflow, bypass every safety control, and verify every claim against objective evidence.

**Verdict: ⚠ Conditionally Ready — P0 Remediation Required**

**6 Critical (P0) findings, 5 High (P1) findings, 5 Medium (P2) findings, 3 Low (P3) findings.**

The module's architecture, RBAC scaffolding, audit logging, and traceability design are sound. However, **the core clinical safety enforcement is absent**: the `issue_blood` command — the single most dangerous operation in a blood bank — does not check blood expiry, donation screening status, or ABO/Rh compatibility before releasing a unit to a patient. Combined with the fact that `check_blood_compatibility` is itself broken (queries a non-existent `patients.rh_factor` column), the module in its current state **could issue expired, unscreened, incompatible blood to a patient**. This is a patient-safety blocker that must be remediated before any further phase.

The findings are concrete, evidence-based, and remediable without re-architecture. The state machine, concurrency-safe unit claims, RBAC, audit logging, and traceability table design are all correctly implemented — the gaps are in the *pre-condition checks* on the issue/transfusion path.

---

## 2. Architecture Review

| Aspect | Assessment | Evidence |
|---|---|---|
| Pattern reuse from RAD-BASELINE-1.0 | ✅ Sound | `validate_enum`, `sanitize_db_error`, `audit::for_session`, SEQUENCE numbers, state machine, soft-delete — all mirror Radiology |
| Command registration | ✅ Sound | All 32 commands registered in `lib.rs:1178-1208` |
| RBAC on every command | ✅ Sound | `rbac::require` on all 32 commands (verified by grep) |
| Transaction usage | ✅ Sound | All multi-step writes use `pool.begin()` + `tx.commit()` |
| Concurrency-safe unit claims | ✅ Sound | `UPDATE...WHERE status='available' RETURNING` (atomic) |
| Trust boundary | ✅ Sound | Frontend is not trusted; all validation server-side |
| Module LOC | ⚠ Reporting inaccuracy | Implementation report claims "~1500 LOC"; actual is **2728 LOC** (`wc -l`) |

**Architecture verdict:** The structural foundation is sound. The defects are in clinical pre-condition enforcement, not architecture.

---

## 3. Database Engineering Review

### 3.1 What is correct

| Check | Status | Evidence |
|---|---|---|
| Idempotent migrations | ✅ | All `CREATE TABLE IF NOT EXISTS` / `ADD COLUMN IF NOT EXISTS` |
| CHECK constraints on enums | ✅ | 27 CHECK constraints across 11 tables |
| Foreign keys with clinical policy | ✅ | `ON DELETE RESTRICT` for patient/donor; `CASCADE` for history/movements |
| Partial indexes | ✅ | 3 partial indexes (`idx_blood_units_available`, `idx_blood_donors_group`, `idx_blood_reservations_active`) |
| SEQUENCE for number generation | ✅ | 7 sequences, concurrency-safe |
| Soft-delete on donors + units | ✅ | `deleted_at`, `deleted_by_user_id`, `deleted_reason` columns |
| Status history table | ✅ | `blood_unit_status_history` with `related_record_type`/`related_record_id` |
| Inventory movement table | ✅ | `blood_inventory_movements` for chain-of-custody |
| Compatibility matrix seed | ✅ | 64 ISBT ABO/Rh pairings seeded (verified 8×8) |

### 3.2 Database findings

| ID | Severity | Finding | Evidence |
|---|---|---|---|
| DB-01 | Medium | Missing index on `blood_units.issued_to_patient_id` and `blood_units.transfused_to_patient_id` | `db.rs` — no index declared for these columns; patient-history queries by these FKs would seq-scan at scale |
| DB-02 | Low | Compatibility matrix seed uses `.ok()` to swallow insert errors | `db.rs:1392` — if a seed insert fails (e.g., transient lock), the matrix is silently incomplete; `check_blood_compatibility` returns `false` for missing rows (fail-closed, but data integrity unverified) |
| DB-03 | Low | `blood_units.reservation_id` FK added via `ALTER TABLE` after table creation | `db.rs:1223` — works but means the FK is not in the original CREATE TABLE; idempotent via `ADD CONSTRAINT IF NOT EXISTS` |

**Database verdict:** Schema is well-designed. No Critical or High database findings.

---

## 4. Backend Engineering Review

### 4.1 What is correct

| Check | Status | Evidence |
|---|---|---|
| RBAC on every command | ✅ | `rbac::require` on all 32 commands |
| Enum validation before DB access | ✅ | `validate_enum` on 12 enum types |
| Audit logging on write commands | ✅ | 16 of 16 write commands call `audit::for_session` |
| `sanitize_db_error` on all `.map_err()` | ✅ | Verified by grep — consistent usage |
| State machine | ✅ | `is_valid_unit_transition` enforces valid transitions; terminal states locked |
| Pagination | ✅ | All list commands use `LIMIT/OFFSET` + `COUNT(*)` |
| Parameterized queries (no injection) | ✅ | Dynamic WHERE uses `format!` for bind-index only; values via `.bind()` |
| No `unwrap()` in production paths | ✅ | Only 2 `unwrap()` calls (lines 2072, 2081), both guarded by `is_some()` check |

### 4.2 Backend findings

| ID | Severity | Finding | Evidence |
|---|---|---|---|
| BE-01 | **Critical** | `check_blood_compatibility` queries `SELECT blood_group, rh_factor FROM patients` but `patients` table has **no `rh_factor` column** | `blood_bank.rs:1383`; `db.rs:432` — patients has only `blood_group VARCHAR(8)`. This is a runtime SQL error. The compatibility-check command is **completely broken**. |
| BE-02 | **Critical** | `issue_blood` does not check `expiry_date > NOW()` | `blood_bank.rs:1818-1980` — the atomic claim checks `status IN ('available','reserved')` but never checks expiry. **Expired blood can be issued.** |
| BE-03 | **Critical** | `issue_blood` does not check the linked donation's `screening_status` | `blood_bank.rs:1818` — `create_blood_donation` (line 707) auto-creates the unit with `status='available'` and `screening_status='pending'`. The unit is issuable **before screening passes**. **Unscreened, potentially infectious blood can be issued.** |
| BE-04 | **Critical** | `issue_blood` does not enforce ABO/Rh compatibility | `blood_bank.rs:1818` — no call to `check_blood_compatibility` or the compatibility matrix. The `issue_type='uncrossmatched'` field is a label, not an enforcement. **Incompatible blood can be issued.** |
| BE-05 | **Critical** | No auto-expiry mechanism exists despite doc comment claiming "expired (auto)" | `blood_bank.rs:24` comment says "available/reserved/issued → expired (auto)" but `scheduler.rs` has no blood-bank expiry job. Units past `expiry_date` remain in their current status indefinitely. |
| BE-06 | **Critical** | `create_blood_donation` creates the blood unit as `'available'` immediately, before screening | `blood_bank.rs:707` — `INSERT INTO blood_units ... 'available'`. The unit should be created in `'quarantine'` status and only move to `'available'` after `update_blood_donation_screening` passes. |
| BE-07 | **High** | `create_blood_transfusion` does not verify `issue.patient_id == transfusion.patient_id` | `blood_bank.rs:2101` — checks `issue.unit_id == transfusion.unit_id` but NOT patient identity. A transfusion can be recorded for patient B under an issue that was for patient A. |
| BE-08 | **High** | `update_blood_unit_status` (issued→available) does not clear stale fields | `blood_bank.rs` — `stamp_clause` only adds timestamps for 'transfused'/'discarded'. Transitioning `issued → available` via the generic update leaves `issued_to_patient_id`, `issued_at`, and `reserved_for_patient_id` populated. An "available" unit falsely shows as issued/reserved. |
| BE-09 | **High** | `return_blood_unit` does not clear `reserved_for_patient_id` | `blood_bank.rs` — clears `issued_to_patient_id` and `issued_at` but not `reserved_for_patient_id` or `reservation_id`. A returned unit that was previously reserved still shows as reserved. |
| BE-10 | **High** | `return_blood_unit` succeeds after a cancelled transfusion | If `create_blood_transfusion` is called with `outcome='cancelled'`, the unit stays 'issued' (not moved to 'transfused'). `return_blood_unit` then returns it as "unused" — but a transfusion was attempted. The transfusion record exists in `blood_transfusions` but the unit returns to 'available' as if nothing happened. Traceability gap. |
| BE-11 | **High** | Reservation expiry not enforced | `create_blood_reservation` sets `expires_at`, but `issue_blood` does not check `expires_at > NOW()` on the linked reservation. An expired reservation can still be fulfilled. |
| BE-12 | Medium | No donor eligibility interval check | `create_blood_donation` does not enforce minimum interval between donations (typically 8-12 weeks per national guidelines). A donor could donate daily. |
| BE-13 | Medium | No hemoglobin minimum enforcement | `create_blood_donation` records `hemoglobin_level` but does not enforce a minimum (≥12.5 g/dL per standards). Donation proceeds regardless. |
| BE-14 | Low | `from_f64_retain` produces `Option<Option<Decimal>>` | `blood_bank.rs:470,471,707,710,2191,2192` — `.map(rust_decimal::Decimal::from_f64_retain)` on `Option<f64>` yields `Option<Option<Decimal>>`. Functions correctly via sqlx nested-Option Encode impl (both `None` and `Some(None)` → NULL), but should use `.and_then()` for clarity. |
| BE-15 | Low | `cargo check` not verified | No Rust toolchain in implementation environment. All Rust code is unverified for compilation. Must run `cargo check --features server-build` + `cargo clippy -- -D warnings` before P0 closure. |

---

## 5. Frontend Engineering Review

### 5.1 What is correct

| Check | Status | Evidence |
|---|---|---|
| React Query hooks (1 per command) | ✅ | 30 hooks in `queries.ts:1569-1991` |
| Cache invalidation | ✅ | All mutations invalidate `["bloodbank"]` prefix |
| No optimistic updates | ✅ | All mutations use server-confirmed success (safer for clinical data) |
| Lazy-loaded route | ✅ | `App.tsx:42` — `React.lazy` + Suspense |
| Permission-gated UI | ✅ | `has(PERMISSIONS.BloodBank*)` on all action buttons |
| Loading/empty states | ✅ | `LoadingState` + `EmptyState` on all tabs |
| Server-side pagination | ✅ | All tabs use `Pagination` with `totalItems`/`rowsPerPage` |
| TypeScript strict | ✅ | `tsc --noEmit` passes with 0 errors |

### 5.2 Frontend findings

| ID | Severity | Finding | Evidence |
|---|---|---|---|
| FE-01 | Medium | `stock_by_type` fetched but never rendered | `BloodBank.tsx` — `useBloodBankDashboard` returns `stock_by_type` (stock-by-blood-type grid) but `DashboardGrid` component does not render it. Clinically useful data is fetched and discarded. |
| FE-02 | Medium | `CreateTransfusionDialog` loads only first 100 issues | `BloodBank.tsx` — `useBloodIssues(undefined, undefined, 1, 100)` then filters `!returned_at`. At scale, the relevant issue may not be in the first 100. No search/filter. The operator may be unable to find the issue to record a transfusion against. |
| FE-03 | Medium | `CreateUnitDialog` loads only first 100 active donors | `BloodBank.tsx` — `useBloodDonors(undefined, undefined, "active", 1, 100)`. At 100k donors, a donor not in the first 100 cannot be selected. No type-ahead search. |
| FE-04 | Low | No dashboard auto-refresh | `useBloodBankDashboard` has no `refetchInterval`. In a multi-user environment, the dashboard can be stale. |
| FE-05 | Low | `PRIORITIES_LIST` constant declared but unused | `BloodBank.tsx` — removed during type-check fixes but the `ISSUE_TYPES` constant is used; no regression, just a cleanup note. |

---

## 6. Clinical Safety Review

The review board attempted every unsafe workflow listed in the prompt. Results:

### 6.1 Unsafe Workflow Test Results

| Workflow Attempt | Result | Evidence / Finding |
|---|---|---|
| Reserve the same unit twice | ✅ Blocked | Atomic `UPDATE...WHERE status='available' RETURNING` — second reservation finds status='reserved', fails |
| Issue the same unit twice | ✅ Blocked | Atomic claim sets status='issued'; second issue finds status not in ('available','reserved') |
| Transfuse the same unit twice | ✅ Blocked | After transfusion, status='transfused' (terminal); second transfusion rejected |
| Discard after transfusion | ✅ Blocked | State machine: 'transfused' is terminal, `is_valid_unit_transition` returns false |
| Return after transfusion | ✅ Blocked | `return_blood_unit` checks status='issued'; 'transfused' rejected |
| **Issue expired blood** | ❌ **NOT BLOCKED** | `issue_blood` does not check `expiry_date` (BE-02). **Expired blood can be issued.** |
| **Issue unscreened blood** | ❌ **NOT BLOCKED** | Unit created as 'available' before screening (BE-06); `issue_blood` doesn't check screening_status (BE-03). **Unscreened blood can be issued.** |
| **Issue incompatible blood** | ❌ **NOT BLOCKED** | `issue_blood` does not check ABO/Rh compatibility (BE-04). **Incompatible blood can be issued.** |
| Issue while quarantined | ✅ Blocked | Quarantine status excluded from `issue_blood` claim (`status IN ('available','reserved')`) |
| **Issue without compatibility check** | ❌ **NOT BLOCKED** | `check_blood_compatibility` is advisory only; `issue_blood` does not call it. AND the check itself is broken (BE-01). |
| Screening failure bypass quarantine | ✅ Blocked | `update_blood_donation_screening` with 'failed' moves unit to 'quarantine' |
| **Issue after expiry (reserved unit)** | ❌ **NOT BLOCKED** | No auto-expiry (BE-05); `issue_blood` doesn't check expiry on reserved units either |
| Transfusion to wrong patient | ❌ **NOT BLOCKED** | `create_blood_transfusion` doesn't verify `issue.patient_id == transfusion.patient_id` (BE-07) |
| Traceability broken | ✅ Not broken | `blood_unit_status_history` + `blood_inventory_movements` use `ON DELETE CASCADE` from `blood_units`; history survives |
| Audit log bypass | ✅ Not broken | All 16 write commands call `audit::for_session` |
| History disappears | ✅ Not broken | Soft-delete preserves history; CASCADE on history tables |
| **Return "unused" blood that was transfusion-attempted** | ❌ **NOT BLOCKED** | Cancelled transfusion leaves unit 'issued'; `return_blood_unit` returns it as unused (BE-10) |

### 6.2 Clinical Safety Verdict

**3 of the most dangerous workflows are NOT blocked:**

1. **Expired blood can be issued** — no expiry check in `issue_blood`, no auto-expiry job
2. **Unscreened blood can be issued** — unit is 'available' before screening passes; no screening check in `issue_blood`
3. **Incompatible blood can be issued** — no ABO/Rh enforcement in `issue_blood`; the compatibility check command is itself broken

These are **P0 Critical patient-safety defects**. In a real hospital, any one of these could cause a fatality. All three must be remediated before the module can proceed.

---

## 7. Security Review

| Attack Vector | Result | Evidence |
|---|---|---|
| RBAC bypass | ✅ Blocked | `rbac::require` on all 32 commands; no command is unguarded |
| Soft-delete bypass | ✅ Blocked | All list commands filter `deleted_at IS NULL`; `get_blood_unit` checks it |
| Direct IPC (no session) | ✅ Blocked | `rbac::require` returns "not signed in" error if `None` |
| Enum injection | ✅ Blocked | `validate_enum` before DB access on all enum inputs |
| Permission escalation | ✅ Blocked | 8 distinct permissions; issue requires `BloodBankIssue`, transfuse requires `BloodBankTransfuse`, discard requires `BloodBankDiscard` |
| SQL injection | ✅ Blocked | All queries parameterized; dynamic WHERE uses `format!` for bind-index integers only |
| Replay | ⚠ Not applicable | Desktop app, single-session; no replay surface |
| Audit bypass | ✅ Blocked | Audit logging is in code path, not bypassable via IPC |
| Invalid state injection | ❌ **Partially blocked** | State machine blocks invalid transitions, BUT `update_blood_unit_status` allows `issued→available` without clearing stale fields (BE-08) — data integrity issue, not a security bypass |
| Concurrent requests | ✅ Blocked | Atomic unit claims use `UPDATE...RETURNING`; `FOR UPDATE` on status checks |

**Security verdict:** No security bypass found. The gaps are clinical-safety (pre-condition checks), not access-control.

---

## 8. Performance Review

| Operation | Assessment | Evidence |
|---|---|---|
| Dashboard | ✅ Good | Conditional aggregation (`COUNT(*) FILTER`) for inventory KPIs; 2 round-trips |
| Inventory list | ✅ Good | Partial index `idx_blood_units_available`; `LIMIT/OFFSET` |
| Donor search | ✅ Good | ILIKE with `idx_blood_donors_name` + `idx_blood_donors_number` |
| Crossmatch list | ✅ Good | Indexed on `unit_id` and `patient_id` |
| Atomic reservation | ✅ Good | Single `UPDATE...RETURNING` — O(1) lock |
| Traceability query | ⚠ Acceptable | 6 indexed queries by `unit_id`; could be consolidated but acceptable |
| Pagination (page 100) | ✅ Good | `LIMIT 10 OFFSET 990` + partial index |
| `COUNT(*)` on list queries | ⚠ Note | Each list query runs a separate `COUNT(*)`; at 1M units this is a seq-scan on the filtered set. Acceptable with partial indexes but will slow at extreme scale. Consider estimated counts for very large datasets. |
| Frontend dropdown loading | ⚠ Medium | `CreateUnitDialog` and `CreateTransfusionDialog` load 100 records into dropdowns (FE-02, FE-03) — no infinite-scroll/type-ahead |

**Performance verdict:** Acceptable for 1M units / 100k donors with current indexes. The `COUNT(*)` on every list query is the main scaling concern.

---

## 9. Maintainability Review

| Aspect | Assessment | Evidence |
|---|---|---|
| Magic strings | ✅ Good | All enums in `const` arrays; `COMPONENT_LABELS` map in frontend |
| Duplicate logic | ✅ Good | `validate_enum`, `record_unit_event`, `record_movement` are reusable helpers |
| Code organization | ✅ Good | Clear section comments per FR; consistent with Radiology |
| Naming | ✅ Good | `blood_*` tables, `BloodBank*` permissions, `useBloodBank*` hooks |
| Module structure | ✅ Good | Single `blood_bank.rs` file; mirrors `radiology.rs` |
| Documentation | ✅ Good | Module doc comment explains workflow + state machine |
| Technical debt | ⚠ | `from_f64_retain` double-Option (BE-14); LOC reporting inaccuracy (2728 vs claimed 1500) |
| Function length | ⚠ | `issue_blood` (~160 lines), `create_blood_transfusion` (~130 lines) — long but readable |

**Maintainability verdict:** Good. Consistent with the frozen Radiology baseline.

---

## 10. Requirements Traceability Review

| Requirement | DB | Backend | Frontend | RBAC | Audit | Verification |
|---|---|---|---|---|---|---|
| FR-0145 Blood Inventory | ✅ `blood_units` | ✅ 7 commands | ✅ Inventory tab | ✅ BloodBankView/Manage | ✅ | ⚠ Implementation complete; P0 gaps in issue safety |
| FR-0146 Donor Registry | ✅ `blood_donors` + `blood_donations` | ✅ 7 commands | ✅ Donors tab | ✅ BloodBankDonorManage | ✅ | ⚠ Missing donor eligibility interval (BE-12) |
| FR-0147 Cross-Matching | ✅ `blood_crossmatch_results` + `blood_reservations` + `blood_compatibility_matrix` | ✅ 6 commands | ✅ Cross-Match tab | ✅ BloodBankCrossmatch/Verify | ✅ | ❌ `check_blood_compatibility` broken (BE-01); compatibility not enforced at issue (BE-04) |
| FR-0148 Blood Issue / Transfusion | ✅ `blood_issues` + `blood_transfusions` | ✅ 6 commands | ✅ Issues + Transfusions tabs | ✅ BloodBankIssue/Transfuse | ✅ | ❌ No expiry/screening/compatibility check (BE-02,03,04); patient mismatch not verified (BE-07) |
| FR-0149 Blood Traceability | ✅ `blood_unit_status_history` + `blood_inventory_movements` + `blood_discards` | ✅ 6 commands | ✅ Traceability dialog | ✅ BloodBankView | ✅ | ✅ Traceability design sound; gap is BE-10 (return after cancelled transfusion) |

**Traceability verdict:** All 5 FRs map to complete implementation stacks. The defects are in pre-condition enforcement, not missing functionality.

---

## 11. Risk Matrix

| Risk | Severity | Likelihood | Impact | Risk Score |
|---|---|---|---|---|
| Expired blood issued/transfused | Critical | High (no check) | Catastrophic (patient death) | **Extreme** |
| Unscreened blood issued/transfused | Critical | High (unit available pre-screening) | Catastrophic (infection transmission) | **Extreme** |
| Incompatible blood issued/transfused | Critical | Medium (operator may notice) | Catastrophic (hemolytic reaction) | **Extreme** |
| Transfusion recorded for wrong patient | High | Low (requires manual ID entry) | Severe (wrong-patient transfusion) | **High** |
| Stale issued-to/reserved-for data after return | High | High (every return) | Moderate (data integrity, reporting errors) | **High** |
| Reservation expiry not enforced | High | Medium | Moderate (unit held unnecessarily) | **Medium** |
| No donor eligibility interval | Medium | Medium | Moderate (donor harm) | **Medium** |
| `cargo check` unverified | Medium | Unknown | Unknown (may not compile) | **Medium** |

---

## 12. Engineering Findings Register

| ID | Severity | Title | Section |
|---|---|---|---|
| BE-01 | P0 Critical | `check_blood_compatibility` queries non-existent `patients.rh_factor` | Backend |
| BE-02 | P0 Critical | `issue_blood` does not check expiry_date | Backend |
| BE-03 | P0 Critical | `issue_blood` does not check donation screening_status | Backend |
| BE-04 | P0 Critical | `issue_blood` does not enforce ABO/Rh compatibility | Backend |
| BE-05 | P0 Critical | No auto-expiry mechanism (doc claims "auto" but none exists) | Backend |
| BE-06 | P0 Critical | Unit created as 'available' before screening passes | Backend |
| BE-07 | P1 High | `create_blood_transfusion` does not verify patient identity | Backend |
| BE-08 | P1 High | `update_blood_unit_status` leaves stale fields on issued→available | Backend |
| BE-09 | P1 High | `return_blood_unit` does not clear `reserved_for_patient_id` | Backend |
| BE-10 | P1 High | `return_blood_unit` succeeds after cancelled transfusion | Backend |
| BE-11 | P1 High | Reservation expiry not enforced | Backend |
| BE-12 | P2 Medium | No donor eligibility interval check | Backend |
| BE-13 | P2 Medium | No hemoglobin minimum enforcement | Backend |
| FE-01 | P2 Medium | `stock_by_type` fetched but not rendered | Frontend |
| FE-02 | P2 Medium | `CreateTransfusionDialog` loads only first 100 issues | Frontend |
| FE-03 | P2 Medium | `CreateUnitDialog` loads only first 100 donors | Frontend |
| DB-01 | P2 Medium | Missing index on `issued_to_patient_id`/`transfused_to_patient_id` | Database |
| BE-14 | P3 Low | `from_f64_retain` double-Option | Backend |
| BE-15 | P3 Low | `cargo check` not verified | Backend |
| FE-04 | P3 Low | No dashboard auto-refresh | Frontend |
| DB-02 | P3 Low | Compatibility matrix seed swallows errors | Database |
| DB-03 | P3 Low | `reservation_id` FK added post-creation | Database |

---

## 13. P0 Findings (Critical)

### BE-01 — `check_blood_compatibility` queries non-existent `patients.rh_factor` column

- **Severity:** P0 Critical
- **Title:** Compatibility-check command is broken at runtime
- **Description:** `check_blood_compatibility` executes `SELECT blood_group, rh_factor FROM patients WHERE id = $1`. The `patients` table has a `blood_group` column (`db.rs:432`, `VARCHAR(8)`) but **no `rh_factor` column**. This query will fail with a PostgreSQL error at runtime.
- **Technical Root Cause:** The command assumed `patients` has the same `rh_factor` column as `blood_donors`/`blood_units`. The patients EHR expansion (`db.rs:425-447`) added `blood_group` but never added `rh_factor`.
- **Clinical Risk:** The compatibility check is the operator's primary tool for verifying ABO/Rh safety before issue. A broken check forces operators to issue without verification — or worse, they see the error and proceed anyway.
- **Evidence:** `blood_bank.rs:1383` (`SELECT blood_group, rh_factor FROM patients`); `db.rs:432` (patients only has `blood_group`)
- **Recommended Remediation:** Either (a) add `rh_factor VARCHAR(5)` to `patients` via migration, or (b) change the query to derive patient Rh from a separate field, or (c) store patient blood type as a single `blood_type VARCHAR(5)` column (e.g., "O+", "A-"). Option (a) is simplest and most consistent.
- **Regression Risk:** Low — additive migration.
- **Affected Files:** `src-tauri/src/commands/blood_bank.rs:1383`, `src-tauri/src/db.rs` (patients table)

### BE-02 — `issue_blood` does not check `expiry_date`

- **Severity:** P0 Critical
- **Title:** Expired blood can be issued
- **Description:** The `issue_blood` atomic claim checks `status IN ('available','reserved')` but never checks `expiry_date > NOW()`. A unit past its expiry date (which may still be 'available' because there is no auto-expiry job — see BE-05) can be issued and transfused.
- **Technical Root Cause:** Missing pre-condition check in the claim query.
- **Clinical Risk:** Transfusion of expired blood can cause sepsis, hemolysis, or loss of efficacy. Blood components have strict expiry (35 days for whole blood, 5 days for platelets, 1 year for FFP). This is a never-event.
- **Evidence:** `blood_bank.rs:1835-1843` — the `UPDATE...WHERE status IN ('available','reserved') AND deleted_at IS NULL` clause has no expiry predicate.
- **Recommended Remediation:** Add `AND expiry_date > NOW()` to the claim query. Additionally, add a scheduled job (BE-05) to auto-expire units.
- **Regression Risk:** Low — additive predicate.
- **Affected Files:** `src-tauri/src/commands/blood_bank.rs:1835`

### BE-03 — `issue_blood` does not check donation screening_status

- **Severity:** P0 Critical
- **Title:** Unscreened blood can be issued
- **Description:** `create_blood_donation` creates the blood unit with `status='available'` and the donation with `screening_status='pending'` (BE-06). `issue_blood` does not join to `blood_donations` to verify `screening_status='passed'`. A unit from a donation that has not passed infectious-disease screening can be issued.
- **Technical Root Cause:** Two compounding defects: (1) unit is 'available' before screening, (2) issue doesn't check screening.
- **Clinical Risk:** Transfusion of unscreened blood can transmit HIV, Hepatitis B/C, Syphilis, Malaria. This is a catastrophic patient-safety failure.
- **Evidence:** `blood_bank.rs:707` (unit created as 'available'); `blood_bank.rs:1835` (issue claim has no screening join)
- **Recommended Remediation:** (1) Fix BE-06: create unit as 'quarantine', move to 'available' only after screening passes. (2) As defense-in-depth, add a screening check in `issue_blood` via JOIN to `blood_donations`.
- **Regression Risk:** Medium — changes the donation→unit status flow; existing test data may need adjustment.
- **Affected Files:** `src-tauri/src/commands/blood_bank.rs:707, 1835`

### BE-04 — `issue_blood` does not enforce ABO/Rh compatibility

- **Severity:** P0 Critical
- **Title:** Incompatible blood can be issued
- **Description:** `issue_blood` has no ABO/Rh compatibility check. The `issue_type` field includes 'uncrossmatched' but this is a label the operator sets — it is not enforced. For `issue_type='routine'`, there is no requirement that a compatible crossmatch exists. The `check_blood_compatibility` command exists but is advisory (and broken — BE-01).
- **Technical Root Cause:** Missing pre-condition: for non-emergency issues, the system should require either a verified compatible crossmatch or an explicit override with documented justification.
- **Clinical Risk:** ABO-incompatible transfusion causes acute intravascular hemolysis — fatal in ~10% of cases. This is the most dangerous transfusion error.
- **Evidence:** `blood_bank.rs:1818-1980` — no compatibility check anywhere in the issue path.
- **Recommended Remediation:** For `issue_type IN ('routine')`: require a `crossmatch_id` linked to a `blood_crossmatch_results` row with `result='compatible'` AND `verified_at IS NOT NULL`. For `issue_type='emergency'`/`'uncrossmatched'`: require an explicit `clinical_indication` (non-empty) and log a critical audit event. For `issue_type='autologous'`: verify unit donor_id matches patient's own donor record (if applicable).
- **Regression Risk:** Medium — changes issue pre-conditions; may require frontend form updates.
- **Affected Files:** `src-tauri/src/commands/blood_bank.rs:1818`

### BE-05 — No auto-expiry mechanism

- **Severity:** P0 Critical
- **Title:** Expired units remain available indefinitely
- **Description:** The module doc comment (`blood_bank.rs:24`) states "available/reserved/issued → expired (auto)" but no scheduled job exists to auto-expire units. `scheduler.rs` handles WhatsApp reminders and daily digests only — no blood-bank expiry task. Units past `expiry_date` remain in their current status until manually changed.
- **Technical Root Cause:** Missing scheduled task.
- **Clinical Risk:** Compounds BE-02 — expired units not only *can* be issued, they *will* be issued because the system never flags them as expired.
- **Evidence:** `scheduler.rs` — no blood-bank references (verified by grep).
- **Recommended Remediation:** Add a scheduled task (hourly) that runs `UPDATE blood_units SET status='expired', updated_at=NOW() WHERE expiry_date <= NOW() AND status IN ('available','reserved','issued','quarantine') AND deleted_at IS NULL`. Record a `blood_unit_status_history` entry for each. This mirrors the pattern in `scheduler.rs`.
- **Regression Risk:** Low — additive scheduled task.
- **Affected Files:** `src-tauri/src/scheduler.rs`, `src-tauri/src/commands/blood_bank.rs` (new helper)

### BE-06 — Unit created as 'available' before screening passes

- **Severity:** P0 Critical
- **Title:** Unscreened blood enters available inventory immediately
- **Description:** `create_blood_donation` auto-creates the blood unit with `status='available'` (`blood_bank.rs:707`). The donation's `screening_status` is `'pending'`. Until `update_blood_donation_screening` is called with 'passed', the unit is nonetheless available for reservation, crossmatch, and issue.
- **Technical Root Cause:** The donation→unit creation should use `status='quarantine'`, transitioning to 'available' only when screening passes.
- **Clinical Risk:** See BE-03 — unscreened blood can be issued.
- **Evidence:** `blood_bank.rs:707` — `INSERT INTO blood_units ... 'available'`
- **Recommended Remediation:** Change the INSERT to use `'quarantine'`. In `update_blood_donation_screening`, when `screening_status='passed'`, move the unit from 'quarantine' to 'available' (state machine already allows `quarantine → available`).
- **Regression Risk:** Medium — changes donation flow; `update_blood_donation_screening` must be updated to transition the unit.
- **Affected Files:** `src-tauri/src/commands/blood_bank.rs:707`, `update_blood_donation_screening`

---

## 14. P1 Findings (High)

### BE-07 — `create_blood_transfusion` does not verify patient identity

- **Severity:** P1 High
- **Description:** `create_blood_transfusion` checks `issue.unit_id == transfusion.unit_id` but does NOT check `issue.patient_id == transfusion.patient_id`. An operator could record a transfusion against patient B using an issue record for patient A.
- **Evidence:** `blood_bank.rs:2101-2160` — only unit_id is verified.
- **Recommended Remediation:** Add `if issue_patient_id != transfusion.patient_id { return Err(...) }` after fetching the issue's patient_id.
- **Affected Files:** `src-tauri/src/commands/blood_bank.rs:2138`

### BE-08 — `update_blood_unit_status` leaves stale fields on issued→available

- **Severity:** P1 High
- **Description:** The generic `update_blood_unit_status` command allows `issued → available` (the "return" transition). But the `stamp_clause` only adds timestamps for 'transfused' and 'discarded'. It does not clear `issued_to_patient_id`, `issued_at`, or `reserved_for_patient_id`. After this transition, an "available" unit falsely shows as issued to / reserved for a patient.
- **Evidence:** `blood_bank.rs` — `stamp_clause` match has no arm for 'available' that clears fields.
- **Recommended Remediation:** When `new_status == 'available'`, clear `issued_to_patient_id`, `issued_at`, `reserved_for_patient_id`, `reservation_id`. Or: remove `issued → available` and `reserved → available` from the generic command and require the dedicated `return_blood_unit` / `cancel_blood_reservation` commands (which clear fields properly).
- **Affected Files:** `src-tauri/src/commands/blood_bank.rs` (`update_blood_unit_status`)

### BE-09 — `return_blood_unit` does not clear `reserved_for_patient_id`

- **Severity:** P1 High
- **Description:** `return_blood_unit` clears `issued_to_patient_id` and `issued_at` but not `reserved_for_patient_id` or `reservation_id`. A returned unit that was previously reserved still shows as reserved for a patient, even though it's back in available inventory.
- **Evidence:** `blood_bank.rs` — `UPDATE blood_units SET status='available', issued_to_patient_id=NULL, issued_at=NULL` — no `reserved_for_patient_id=NULL`.
- **Recommended Remediation:** Add `reserved_for_patient_id = NULL, reservation_id = NULL` to the UPDATE.
- **Affected Files:** `src-tauri/src/commands/blood_bank.rs` (`return_blood_unit`)

### BE-10 — `return_blood_unit` succeeds after cancelled transfusion

- **Severity:** P1 High
- **Description:** If `create_blood_transfusion` is called with `outcome='cancelled'`, the unit stays 'issued' (not moved to 'transfused'). `return_blood_unit` then returns it as "unused" — but a transfusion was attempted (and cancelled). The transfusion record exists in `blood_transfusions` but the unit returns to 'available' as if nothing happened. This breaks traceability: a unit that touched a patient's line is back in the available pool.
- **Evidence:** `blood_bank.rs` — `create_blood_transfusion` only moves unit to 'transfused' when `outcome == 'completed' || outcome == 'reaction'`. For 'cancelled'/'incomplete', unit stays 'issued'.
- **Recommended Remediation:** Either (a) a cancelled/incomplete transfusion should move the unit to 'quarantine' (not back to 'available' without inspection), or (b) `return_blood_unit` should check for existing transfusion records on the issue and refuse/flag the return.
- **Affected Files:** `src-tauri/src/commands/blood_bank.rs` (`create_blood_transfusion`, `return_blood_unit`)

### BE-11 — Reservation expiry not enforced

- **Severity:** P1 High
- **Description:** `create_blood_reservation` sets `expires_at`, but `issue_blood` does not check `expires_at > NOW()` on the linked reservation. An expired reservation can still be fulfilled, defeating the purpose of the expiry.
- **Evidence:** `blood_bank.rs:1835` — issue claim checks unit status, not reservation expiry.
- **Recommended Remediation:** When fulfilling a reservation in `issue_blood`, check `expires_at > NOW()`. If expired, either reject or auto-cancel the reservation and require a fresh one.
- **Affected Files:** `src-tauri/src/commands/blood_bank.rs:1835`

---

## 15. P2 Findings (Medium)

| ID | Title | Remediation |
|---|---|---|
| BE-12 | No donor eligibility interval check | Enforce minimum 56 days (8 weeks) between donations for the same donor; check `last_donation_date` |
| BE-13 | No hemoglobin minimum enforcement | Reject donation if `hemoglobin_level < 12.5` (g/dL) for males, `< 12.0` for females |
| FE-01 | `stock_by_type` fetched but not rendered | Add a stock-grid section to `DashboardGrid` showing available units by blood type × component |
| FE-02 | `CreateTransfusionDialog` loads only first 100 issues | Add a search input or filter by patient; use debounced search query |
| FE-03 | `CreateUnitDialog` loads only first 100 donors | Add type-ahead donor search with debounced query |
| DB-01 | Missing index on `issued_to_patient_id`/`transfused_to_patient_id` | Add partial indexes for patient-history queries |

---

## 16. P3 Findings (Low)

| ID | Title | Remediation |
|---|---|---|
| BE-14 | `from_f64_retain` produces `Option<Option<Decimal>>` | Use `.and_then(rust_decimal::Decimal::from_f64_retain)` for clarity |
| BE-15 | `cargo check` not verified | Must run `cargo check --features server-build` + `cargo clippy -- -D warnings` locally before P0 closure |
| FE-04 | No dashboard auto-refresh | Add `refetchInterval: 30_000` to `useBloodBankDashboard` |
| DB-02 | Compatibility matrix seed swallows errors | Log seed-insert failures instead of `.ok()` |
| DB-03 | `reservation_id` FK added post-creation | Cosmetic — move into CREATE TABLE (low priority) |

---

## 17. Technical Debt Register

| ID | Item | Severity | Phase |
|---|---|---|---|
| TD-BB-01 | No automated tests for blood bank module | Medium | P1 hardening / testing batch |
| TD-BB-02 | `cargo check` unverified in implementation environment | Medium | Pre-P0-closure |
| TD-BB-03 | No auto-expiry scheduled job | Critical (P0) | P0 remediation |
| TD-BB-04 | No barcode/scanner integration for unit numbers | Low | Future enhancement |
| TD-BB-05 | No PDF/print for transfusion records | Low | Future enhancement |
| TD-BB-06 | No blood-bank-specific role (e.g., "blood_bank_technician") | Low | Future enhancement |
| TD-BB-07 | `COUNT(*)` on every list query (scaling concern at 1M+) | Low | P1 hardening |

---

## 18. Regression Risk Assessment

| Module | Status | Evidence |
|---|---|---|
| Patients | ✅ Unchanged | No modifications (but BE-01 remediation will add `rh_factor` column — additive migration) |
| Doctors | ✅ Unchanged | No modifications |
| Appointments | ✅ Unchanged | No modifications |
| Laboratory | ✅ Unchanged | No modifications |
| Radiology (RAD-BASELINE-1.0) | ✅ Unchanged | Frozen baseline respected — no modifications |
| Billing | ✅ Unchanged | No modifications |
| Pharmacy | ✅ Unchanged | No modifications |
| Reports | ✅ Unchanged | No modifications |
| Backup | ✅ Unchanged | No modifications |
| Authentication | ✅ Unchanged | No modifications |
| RBAC | ✅ Additive only | 8 new permissions appended |
| Audit | ✅ Unchanged | Uses existing `audit::for_session` |
| Navigation/Routing | ✅ Additive only | 1 new route + 1 new nav item |
| Shared components | ✅ Unchanged | No modifications |
| Frontend tests | ✅ All pass | 68/68 vitest tests pass |

**Regression risk:** Low. All P0 remediations are additive (new checks, new scheduled job, new migration column). No existing module needs modification.

---

## 19. Production Readiness Assessment (ISO 25010)

| Characteristic | Score | Assessment |
|---|---|---|
| Functional Suitability | 70/100 | All FRs implemented but core safety enforcement (expiry/screening/compatibility) absent |
| Performance Efficiency | 88/100 | Good index design; COUNT(*) concern at extreme scale |
| Compatibility | 95/100 | Additive only; no changes to existing modules |
| Usability | 82/100 | Good UI; dropdown-search gaps (FE-02, FE-03); missing stock grid (FE-01) |
| Reliability | 60/100 | State machine sound; but data-integrity bugs (BE-08, BE-09, BE-10) and no auto-expiry |
| Security | 90/100 | RBAC solid; no bypass found; gaps are clinical not access-control |
| Maintainability | 88/100 | Good patterns; consistent with Radiology baseline |
| Portability | 95/100 | Windows-native; no platform-specific code |
| **Clinical Safety** | **35/100** | **3 of the most dangerous workflows not blocked — expired/unscreened/incompatible blood can be issued** |

**Overall: ~65/100 — NOT production-ready. P0 remediation required.**

---

## 20. Final Recommendation

### ⚠ Conditionally Ready — P0 Remediation Required

**Objective Evidence:**

The Blood Bank module's architecture, RBAC, audit logging, traceability design, and concurrency controls are sound and consistent with the frozen Radiology baseline (RAD-BASELINE-1.0). The frontend builds cleanly (tsc 0 errors, eslint 0 errors, vite build succeeds, 68/68 tests pass).

**However, 6 Critical (P0) patient-safety defects must be remediated before the module can proceed to P1 hardening:**

1. **BE-01:** `check_blood_compatibility` is broken (queries non-existent `patients.rh_factor`)
2. **BE-02:** `issue_blood` does not check expiry — expired blood can be issued
3. **BE-03:** `issue_blood` does not check screening status — unscreened blood can be issued
4. **BE-04:** `issue_blood` does not enforce ABO/Rh compatibility — incompatible blood can be issued
5. **BE-05:** No auto-expiry mechanism — expired units remain available indefinitely
6. **BE-06:** Units are 'available' before screening passes — unscreened blood enters inventory

**The core issue:** `issue_blood` — the single most dangerous command in a blood bank — has no clinical pre-condition checks. In its current state, the module could issue expired, unscreened, ABO-incompatible blood to a patient. This is a never-event in transfusion medicine.

**Conditions for P0 closure:**
- All 6 P0 findings remediated
- `cargo check --features server-build` + `cargo clippy -- -D warnings` pass locally
- Re-run IV&V to verify remediation

**The 5 P1 (High) findings (BE-07 through BE-11) should be remediated in the same P0 batch** as they are low-effort and directly related to clinical safety (patient identity verification, data integrity, reservation expiry).

**Do NOT proceed to P1 hardening until P0 findings are closed and verified.**

---

*End of BB-002 — Blood Bank Module P0 Critical Engineering Review*
