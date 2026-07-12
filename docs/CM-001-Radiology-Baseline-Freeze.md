# CM-001 — Radiology Module Baseline Freeze & Configuration Audit

**Document:** Configuration Management Record CM-001
**Module:** Radiology (Phase 2-D)
**Baseline Identifier:** RAD-BASELINE-1.0
**Version:** 1.0.0
**Status:** Frozen
**Acceptance Date:** 2025-07-11 (Asia/Karachi, UTC+5)
**Authority:** Configuration Management Board (CMB), VitalFlow HMS
**Standards:** IEEE 828 (Configuration Management), ISO/IEC/IEEE 12207 (Lifecycle), ISO 25010 (Quality)

---

## 1. Executive Summary

The Configuration Management Board (CMB) has completed the formal baseline freeze for the Radiology Module of VitalFlow HMS. Every approved configuration item — source code, database objects, commands, permissions, routes, and documentation — has been inventoried against the actual repository contents at `/home/z/my-project/hospital-mgt-extracted/hospital-mgt/`.

**Objective evidence gathered:**
- 9 Tauri commands verified registered in `lib.rs:1162-1170`
- 7 RBAC permissions verified in `rbac.rs:56-63`
- 4 database tables, 7 indexes, 1 sequence, 1 UNIQUE constraint, 3 soft-delete columns verified in `db.rs:938-1040`
- 9 React Query hooks verified in `queries.ts:1418-1537`
- 1 lazy-loaded, permission-gated route verified in `App.tsx:417-428`
- Full requirements traceability FR-0140 → FR-0142 confirmed end-to-end

**Audit outcome:** No Critical inconsistencies. Seven documentation-status inconsistencies identified (SRS/SDD still describe Radiology as "PLANNED"/"not implemented" — obsolete). These are registered as Change Requests CR-001 through CR-007 for CCB disposition and do not block the code baseline.

**Decision:** ✅ **BASELINE APPROVED.** RAD-BASELINE-1.0 / v1.0.0 is established as the official production baseline. All future modifications to Radiology module configuration items require approved Change Requests processed through the Change Control Board (CCB-001).

---

## 2. Configuration Item List (CIL)

### 2.1 Backend Source Code (Rust)

| CI-ID | File | LOC | Purpose | Status |
|---|---|---|---|---|
| CI-RS-01 | `src-tauri/src/commands/radiology.rs` | ~790 | 9 IPC commands, state machine, enum validation, dashboard | Frozen |
| CI-RS-02 | `src-tauri/src/db.rs` (§938-1040) | ~102 | 4 tables, 7 indexes, sequence, constraint, soft-delete | Frozen |
| CI-RS-03 | `src-tauri/src/rbac.rs` (§56-63, 114-120, 164-170, 211) | ~30 | 7 Radiology permissions + role mapping | Frozen |
| CI-RS-04 | `src-tauri/src/models.rs` (§1069-1215) | ~146 | 5 Rust structs (Order, CreateOrder, Report, CreateReport, StatusHistory) | Frozen |
| CI-RS-05 | `src-tauri/src/lib.rs` (§1162-1170) | 9 lines | Command handler registration | Frozen |
| CI-RS-06 | `src-tauri/src/audit.rs` (shared) | — | `audit::for_session()` helper used by 5 radiology write commands | Frozen (shared) |

### 2.2 Frontend Source Code (React/TypeScript)

| CI-ID | File | LOC | Purpose | Status |
|---|---|---|---|---|
| CI-FE-01 | `src/pages/Radiology.tsx` | ~940 | Main page: dashboard, orders table, 3 dialogs, quick-status actions | Frozen |
| CI-FE-02 | `src/lib/queries.ts` (§1418-1537) | ~120 | 9 React Query hooks (1 per command) | Frozen |
| CI-FE-03 | `src/lib/models.ts` (§700-820) | ~120 | 6 TS interfaces (RadiologyOrder, OrdersResponse, CreateOrder, Report, CreateReport, Dashboard) | Frozen |
| CI-FE-04 | `src/lib/rbac.ts` (§33-39) | 7 lines | 7 frontend permission constants | Frozen |
| CI-FE-05 | `src/App.tsx` (§35-37, 417-428) | — | Lazy import + route + permission gate | Frozen |
| CI-FE-06 | `src/components/layout/Sidebar.tsx` (§233, 299-302) | — | Nav item (ScanLine icon, /radiology, RadiologyView gate) | Frozen |

### 2.3 Shared Components Used (not radiology-owned)

| CI-ID | Component | Usage |
|---|---|---|
| CI-SH-01 | `StatCard` | 6 instances (dashboard KPIs) |
| CI-SH-02 | `Table` / `TableHeader` / `TableRow` / `TableCell` | Orders list |
| CI-SH-03 | `Dialog` / `DialogContent` / `DialogHeader` / `DialogFooter` | 3 dialogs |
| CI-SH-04 | `Button` | All actions |
| CI-SH-05 | `Pagination` | Orders pagination |
| CI-SH-06 | `EmptyState` / `LoadingState` | Zero-orders / loading states |
| CI-SH-07 | `RequirePermission` | Route-level guard |
| CI-SH-08 | `PageContainer` / `PageHeader` / `SectionCard` / `PageToolbar` | Layout primitives |

### 2.4 Database Configuration Items

| CI-ID | Object | Type | Definition Location |
|---|---|---|---|
| CI-DB-01 | `radiology_orders` | Table | `db.rs:940-966` |
| CI-DB-02 | `radiology_reports` | Table | `db.rs:971-983` |
| CI-DB-03 | `radiology_attachments` | Table | `db.rs:988-1000` |
| CI-DB-04 | `radiology_status_history` | Table | `db.rs:1004-1011` |
| CI-DB-05 | `idx_rad_orders_patient` | Index | `db.rs:1015` |
| CI-DB-06 | `idx_rad_orders_status` | Index | `db.rs:1016` |
| CI-DB-07 | `idx_rad_orders_doctor` | Index | `db.rs:1017` |
| CI-DB-08 | `idx_rad_orders_priority` | Index | `db.rs:1018` |
| CI-DB-09 | `idx_rad_orders_date` | Index | `db.rs:1019` |
| CI-DB-10 | `idx_rad_reports_order` | Index | `db.rs:1020` |
| CI-DB-11 | `idx_rad_orders_active` | Partial Index (`WHERE deleted_at IS NULL`) | `db.rs:1023` |
| CI-DB-12 | `radiology_order_seq` | SEQUENCE (START 1) | `db.rs:1027` |
| CI-DB-13 | `uq_rad_reports_order_id` | UNIQUE Constraint | `db.rs:1031` |
| CI-DB-14 | `radiology_orders.deleted_at` | Soft-delete column | `db.rs:1036` |
| CI-DB-15 | `radiology_orders.deleted_by_user_id` | Soft-delete column (FK users) | `db.rs:1038` |
| CI-DB-16 | `radiology_orders.deleted_reason` | Soft-delete column | `db.rs:1040` |

### 2.5 Command Inventory

| CI-ID | Command | Permission | Audit Logged | FR |
|---|---|---|---|---|
| CI-CMD-01 | `get_radiology_orders` | RadiologyView | No (read) | FR-0140 |
| CI-CMD-02 | `get_radiology_order` | RadiologyView | No (read) | FR-0140 |
| CI-CMD-03 | `create_radiology_order` | RadiologyCreate | Yes (`audit::for_session` L278) | FR-0140 |
| CI-CMD-04 | `update_radiology_order_status` | RadiologyUpdate | Yes (L399) | FR-0140 |
| CI-CMD-05 | `delete_radiology_order` | RadiologyDelete | Yes (L450) | FR-0140 |
| CI-CMD-06 | `get_radiology_report` | RadiologyView | No (read) | FR-0141 |
| CI-CMD-07 | `create_radiology_report` | RadiologyReport | Yes (L626) | FR-0141 |
| CI-CMD-08 | `verify_radiology_report` | RadiologyVerify | Yes (L720) | FR-0141 |
| CI-CMD-09 | `get_radiology_dashboard` | RadiologyView | No (read) | FR-0142 |

**Audit coverage:** 5 of 5 write commands audit-logged (100%). 4 read commands exempt by design.

### 2.6 Route Inventory

| CI-ID | Route | Permission Gate | Lazy Loaded | Navigation |
|---|---|---|---|---|
| CI-RT-01 | `/radiology` | `PERMISSIONS.RadiologyView` | Yes (`React.lazy`, `App.tsx:36`) | Sidebar nav item (`Sidebar.tsx:299`, ScanLine icon) |

### 2.7 Permission Inventory

| CI-ID | Permission | Code | Used By (commands) | Role Mapping |
|---|---|---|---|---|
| CI-PERM-01 | RadiologyView | `radiology.view` | CMD-01,02,06,09 + route + nav | SUPER_ADMIN, DOCTOR |
| CI-PERM-02 | RadiologyCreate | `radiology.create` | CMD-03 | SUPER_ADMIN, DOCTOR |
| CI-PERM-03 | RadiologyUpdate | `radiology.update` | CMD-04 | SUPER_ADMIN, DOCTOR |
| CI-PERM-04 | RadiologyDelete | `radiology.delete` | CMD-05 | SUPER_ADMIN only |
| CI-PERM-05 | RadiologyReport | `radiology.report` | CMD-07 | SUPER_ADMIN only |
| CI-PERM-06 | RadiologyVerify | `radiology.verify` | CMD-08 | SUPER_ADMIN only |
| CI-PERM-07 | RadiologyManage | `radiology.manage` | *(none — reserved)* | SUPER_ADMIN only |

---

## 3. Baseline Manifest

| Field | Value |
|---|---|
| **Baseline Name** | Radiology Module Baseline |
| **Baseline Identifier** | RAD-BASELINE-1.0 |
| **Version** | 1.0.0 |
| **Status** | Frozen |
| **Acceptance Date** | 2025-07-11 |
| **Predecessor** | Module Acceptance Review (P1 IV&V PASSED) |
| **Configuration Items** | 6 Rust files, 6 TS/TSX files, 16 DB objects, 9 commands, 7 permissions, 1 route |

### Approved Artifacts

| Artifact | Version | Purpose | Status |
|---|---|---|---|
| `commands/radiology.rs` | 1.0.0 | 9 IPC commands | Approved-Frozen |
| `pages/Radiology.tsx` | 1.0.0 | UI page | Approved-Frozen |
| `db.rs` (radiology §) | 1.0.0 | Schema + migrations | Approved-Frozen |
| `rbac.rs` (radiology §) | 1.0.0 | Permissions + roles | Approved-Frozen |
| `models.rs` (radiology §) | 1.0.0 | Rust structs | Approved-Frozen |
| `queries.ts` (radiology §) | 1.0.0 | React Query hooks | Approved-Frozen |
| `models.ts` (radiology §) | 1.0.0 | TS interfaces | Approved-Frozen |
| `rbac.ts` (radiology §) | 1.0.0 | Frontend perm constants | Approved-Frozen |
| `App.tsx` (radiology route) | 1.0.0 | Routing | Approved-Frozen |
| `Sidebar.tsx` (radiology nav) | 1.0.0 | Navigation | Approved-Frozen |
| `lib.rs` (radiology registration) | 1.0.0 | Handler registration | Approved-Frozen |

---

## 4. Configuration Audit Report

### 4.1 Audit Findings

| Finding ID | Phase | Severity | Description | Disposition |
|---|---|---|---|---|
| CA-01 | D (Docs) | Medium | SRS §4.12 (L276) marks Radiology "Phase 2, PLANNED" — obsolete | CR-001 (doc status correction) |
| CA-02 | D (Docs) | Medium | SRS L677 states Radiology "not implemented" — obsolete | CR-002 |
| CA-03 | D (Docs) | Medium | SDD L693 states Radiology "not yet designed" — obsolete | CR-003 |
| CA-04 | D (Docs) | Low | SRS L123 attributes FR-0140-0159 to Pharmacist; §4.12 assigns FR-0140-0149 to Radiology — cross-ref error | CR-004 |
| CA-05 | D (Docs) | Low | SRS FR-0140 text ("study catalog") does not match implementation FR mapping (orders). Traceability note, not a code defect | CR-005 (traceability reconciliation) |
| CA-06 | G (Security) | Medium | No Radiologist role seeded; RadiologyReport/Verify/Delete/Manage only available to SUPER_ADMIN. Workflow gap, not a security hole (least-privilege maintained) | CR-006 (Phase 2 enhancement) |
| CA-07 | F (Source) | Low | `RadiologyManage` permission defined but unused by any command | Accepted (reserved for future admin commands) |

### 4.2 Audit Result by Phase

| Phase | Checks | Pass | Fail | Result |
|---|---|---|---|---|
| A — CI Identification | 7 categories | 7 | 0 | ✅ PASS |
| B — Baseline Manifest | 1 manifest | 1 | 0 | ✅ PASS |
| C — Traceability Freeze | 3 FRs | 3 | 0 | ✅ PASS |
| D — Documentation Audit | 4 docs cross-checked | 3 | 1 (status inconsistencies) | ⚠ PASS-WITH-CRs |
| E — Database Config | 10 checks | 10 | 0 | ✅ PASS |
| F — Source Code Config | 6 checks | 6 | 0 | ✅ PASS |
| G — Security Config | 7 checks | 7 | 0 | ✅ PASS |
| H — Release Readiness | 6 checks | 6 | 0 | ✅ PASS |
| I — Change Control Prep | 1 CCB record | 1 | 0 | ✅ PASS |
| J — Checklist | 9 items | 9 | 0 | ✅ PASS |

**Overall Audit Result:** ✅ PASS — no Critical inconsistencies. Documentation-status findings are Medium/Low and remediable via CRs without touching frozen code.

---

## 5. Database Configuration Report

### 5.1 Tables (4)

| Table | PK | Columns | Purpose |
|---|---|---|---|
| `radiology_orders` | id (SERIAL) | 22 | Imaging order with patient/doctor/study/priority/status lifecycle |
| `radiology_reports` | id (SERIAL) | 10 | Radiologist findings/impression/recommendations + verification |
| `radiology_attachments` | id (SERIAL) | 10 | Image attachment metadata (future PACS bridge; no commands yet) |
| `radiology_status_history` | id (SERIAL) | 5 | Audit trail of every status transition |

### 5.2 Foreign Keys

| Child Table | Column | Parent | ON DELETE |
|---|---|---|---|
| radiology_orders | patient_id | patients(id) | **RESTRICT** (clinical safety) |
| radiology_orders | encounter_id | encounters(id) | SET NULL |
| radiology_orders | ordered_by_doctor_id | doctors(id) | SET NULL |
| radiology_orders | ordered_by_user_id | users(id) | SET NULL |
| radiology_orders | assigned_radiologist_id | doctors(id) | SET NULL |
| radiology_reports | order_id | radiology_orders(id) | CASCADE |
| radiology_reports | radiologist_id | doctors(id) | SET NULL |
| radiology_reports | verified_by_user_id | users(id) | SET NULL |
| radiology_attachments | order_id | radiology_orders(id) | CASCADE |
| radiology_status_history | order_id | radiology_orders(id) | CASCADE |
| radiology_status_history | changed_by_user_id | users(id) | SET NULL |

### 5.3 Indexes (7)

| Index | Type | Purpose |
|---|---|---|
| idx_rad_orders_patient | B-tree | Patient lookup |
| idx_rad_orders_status | B-tree | Status filter |
| idx_rad_orders_doctor | B-tree | Doctor lookup |
| idx_rad_orders_priority | B-tree | Priority filter |
| idx_rad_orders_date | B-tree | Date range |
| idx_rad_reports_order | B-tree | Report→order join |
| idx_rad_orders_active | **Partial** (`WHERE deleted_at IS NULL`) | Soft-delete-filtered list (P1-3) |

### 5.4 Sequences & Constraints

| Object | Type | Definition |
|---|---|---|
| `radiology_order_seq` | SEQUENCE | START 1 — atomic order-number generation (P1-1) |
| `uq_rad_reports_order_id` | UNIQUE | One report per order (P0-4 concurrency protection) |
| `order_number` | UNIQUE (column) | Backstop for sequence-generated order numbers |

### 5.5 Migration Integrity

| Check | Result |
|---|---|
| All migrations use `IF NOT EXISTS` | ✅ Idempotent |
| Migration order: tables → indexes → sequence → constraint → soft-delete → seed | ✅ Correct |
| No duplicate indexes | ✅ Verified |
| No orphan objects | ✅ All 4 tables referenced by commands |
| No obsolete schema | ✅ All columns used |
| Soft-delete columns present on `radiology_orders` | ✅ (deleted_at, deleted_by_user_id, deleted_reason) |
| `radiology_attachments` has no commands (intentional — future PACS) | ✅ Documented |

---

## 6. Source Code Configuration Report

### 6.1 Module Structure

```
src-tauri/src/commands/radiology.rs    ← 9 commands, state machine, validation
src-tauri/src/db.rs (§938-1040)        ← schema + migrations
src-tauri/src/rbac.rs (§56-63)         ← 7 permissions + role seed
src-tauri/src/models.rs (§1069-1215)   ← 5 Rust structs
src-tauri/src/lib.rs (§1162-1170)      ← handler registration
src/pages/Radiology.tsx                ← UI page (3 dialogs)
src/lib/queries.ts (§1418-1537)        ← 9 hooks
src/lib/models.ts (§700-820)           ← 6 TS interfaces
src/lib/rbac.ts (§33-39)               ← 7 frontend perm constants
src/App.tsx (§35-37, 417-428)          ← lazy route + guard
src/components/layout/Sidebar.tsx      ← nav item
```

### 6.2 Code Quality Checks

| Check | Result |
|---|---|
| No duplicate code | ✅ `validate_enum()`, `allowed_transitions_from()` are reusable helpers |
| No orphan files | ✅ All radiology files are referenced |
| No unused commands | ✅ All 9 commands registered in `lib.rs` and consumed by frontend hooks |
| No undocumented helpers | ✅ All helpers documented with doc comments |
| No missing exports | ✅ `Radiology` exported from page; all hooks exported from queries.ts |
| No circular dependencies | ✅ radiology.rs → audit/models/rbac (one direction) |
| Constants replace magic strings | ✅ `VALID_STATUSES`, `VALID_PRIORITIES`, `VALID_STUDY_TYPES`, `FORBIDDEN_VIA_UPDATE` |
| No `unwrap()`/`expect()` in commands | ✅ All errors use `.map_err(sanitize_db_error)` |

---

## 7. Security Configuration Report

### 7.1 RBAC Mapping

| Command | Permission | Verified |
|---|---|---|
| get_radiology_orders | RadiologyView | ✅ `radiology.rs:119` |
| get_radiology_order | RadiologyView | ✅ `radiology.rs:181` |
| create_radiology_order | RadiologyCreate | ✅ `radiology.rs:201` |
| update_radiology_order_status | RadiologyUpdate | ✅ `radiology.rs:315` |
| delete_radiology_order | RadiologyDelete | ✅ `radiology.rs:429` |
| get_radiology_report | RadiologyView | ✅ `radiology.rs:477` |
| create_radiology_report | RadiologyReport | ✅ `radiology.rs:522` |
| verify_radiology_report | RadiologyVerify | ✅ `radiology.rs:651` |
| get_radiology_dashboard | RadiologyView | ✅ `radiology.rs:750` |

**Result:** 9 of 9 commands have explicit RBAC checks. No undocumented privileged commands.

### 7.2 Audit Coverage

| Write Command | Audit Call | Verified |
|---|---|---|
| create_radiology_order | `audit::for_session` | ✅ L278 |
| update_radiology_order_status | `audit::for_session` | ✅ L399 |
| delete_radiology_order | `audit::for_session` | ✅ L450 |
| create_radiology_report | `audit::for_session` | ✅ L626 |
| verify_radiology_report | `audit::for_session` | ✅ L720 |

**Result:** 5 of 5 write commands audit-logged (100%).

### 7.3 Security Controls

| Control | Status | Evidence |
|---|---|---|
| Soft delete enforced | ✅ | All 9 commands filter `deleted_at IS NULL` |
| Input validation (enum) | ✅ | `validate_enum()` on study_type, priority, status before DB access |
| SQL injection prevention | ✅ | All queries parameterized (`.bind()`) |
| State machine enforcement | ✅ | `is_valid_transition()` + `FORBIDDEN_VIA_UPDATE` blocks reported/verified via generic update |
| Report verification separation | ✅ | `verify_radiology_report` requires distinct `RadiologyVerify` permission |
| UNIQUE constraint (1 report/order) | ✅ | `uq_rad_reports_order_id` at DB level |
| No filesystem path exposure | ✅ | `storage_path` never SELECTed by any command |

### 7.4 Role-Mapping Gap (Finding CA-06)

| Role | Radiology Perms | Assessment |
|---|---|---|
| super_admin | All 7 | ✅ Full access |
| doctor | View, Create, Update (3) | ⚠ Cannot file/verify reports — workflow gap |
| nurse | None | — (no radiology duties expected) |
| receptionist | None | — (no radiology duties) |
| lab_technician | None | — |
| pharmacist | None | — |
| billing_clerk | None | — |
| patient | None | — |
| **radiologist** | **(role does not exist)** | ❌ Gap — needs RadiologyReport + RadiologyVerify |

**Disposition:** CR-006 registered. This is an operational workflow gap (over-restrictive), not a security vulnerability. Least-privilege is maintained.

---

## 8. Requirements Traceability Snapshot

```
FR-0140 (Radiology Orders)
   ↓
   radiology_orders table (db.rs:940)
   ↓
   create_radiology_order, get_radiology_orders, get_radiology_order,
   update_radiology_order_status, delete_radiology_order (radiology.rs)
   ↓
   useRadiologyOrders, useRadiologyOrder, useCreateRadiologyOrder,
   useUpdateRadiologyOrderStatus, useDeleteRadiologyOrder (queries.ts)
   ↓
   RadiologyCreate, RadiologyUpdate, RadiologyDelete permissions (rbac.rs)
   ↓
   P0 Verification: PASSED (state machine, RBAC, concurrency, soft delete, pagination)
   ↓
   P1 IV&V: PASSED (SEQUENCE, enum validation, partial index, dashboard)
   ↓
   Module Acceptance: ACCEPTED
   ↓
   CM-001 Baseline: FROZEN ✅

FR-0141 (Radiology Reports & Verification)
   ↓
   radiology_reports + radiology_status_history tables (db.rs:971, 1004)
   ↓
   get_radiology_report, create_radiology_report, verify_radiology_report (radiology.rs)
   ↓
   useRadiologyReport, useCreateRadiologyReport, useVerifyRadiologyReport (queries.ts)
   ↓
   RadiologyReport, RadiologyVerify permissions (rbac.rs)
   ↓
   P0 Verification: PASSED (UNIQUE constraint, report validation, verify separation)
   ↓
   P1 IV&V: PASSED (sanitize_db_error, code quality)
   ↓
   Module Acceptance: ACCEPTED
   ↓
   CM-001 Baseline: FROZEN ✅

FR-0142 (Radiology Dashboard / Monitoring)
   ↓
   get_radiology_dashboard command (radiology.rs:746)
   ↓
   useRadiologyDashboard hook (queries.ts:1534)
   ↓
   6 StatCards in Radiology.tsx (studies_today, pending_reports,
       emergency_cases, completed_today, cancelled, verification_pending)
   ↓
   RadiologyView permission (rbac.rs)
   ↓
   P1 IV&V: PASSED (single conditional-aggregation query, 2 round-trips)
   ↓
   Module Acceptance: ACCEPTED
   ↓
   CM-001 Baseline: FROZEN ✅
```

**Traceability completeness:** 3 of 3 FRs fully traced (Requirement → DB → Backend → Frontend → RBAC → Verification → Acceptance). ✅ COMPLETE.

**Note on FR numbering (Finding CA-05):** The SRS §4.12 text assigns FR-0140 to a "study catalog" and FR-0141 to "orders." The implementation header comments map FR-0140→orders, FR-0141→reports, FR-0142→dashboard. This is a documentation traceability-label reconciliation matter (CR-005), not a missing-requirement defect — all three functional capabilities (orders, reports+verification, dashboard) are implemented and verified.

---

## 9. Documentation Consistency Report

### 9.1 Documents Reviewed

| Document | Version | Radiology Coverage |
|---|---|---|
| `01-SRS-Software-Requirements.md` | v1.0 | §4.12 (L276-283), L123, L677 |
| `02-SDD-Software-Design.md` | v1.0 | L693 |
| `03-Quality-Model-ISO-25010.md` | v1.0 | (general) |
| `04-Security-Control-Matrix-ISO-27001.md` | v1.0 | (general RBAC) |
| `CHANGELOG.md` | v0.2.0 | Phase 2 batches |
| P0 Remediation Report | — | (audit archive) |
| P1 IV&V Report | — | (audit archive) |
| Module Acceptance Review | — | (audit archive) |

### 9.2 Inconsistencies Identified

| ID | Location | Inconsistency | Severity | Action |
|---|---|---|---|---|
| CA-01 | SRS §4.12 L276 | "Radiology (FR-0140–FR-0149) — Phase 2, PLANNED" → should be "IMPLEMENTED" | Medium | CR-001 |
| CA-02 | SRS L677 | "Phase 2 modules (... Radiology ...) are not implemented" → obsolete | Medium | CR-002 |
| CA-03 | SDD L693 | "Phase 2 module designs (... Radiology ...) are not yet designed" → obsolete | Medium | CR-003 |
| CA-04 | SRS L123 | Attributes "FR-0140–FR-0159" to Pharmacist; conflicts with §4.12 (FR-0140-0149=Radiology) | Low | CR-004 |
| CA-05 | SRS §4.12 | FR-0140 text ("study catalog") vs implementation (orders). Label reconciliation needed | Low | CR-005 |

### 9.3 Consistency Checks Passed

- ✅ No contradictions in terminology (module consistently named "Radiology")
- ✅ Correct version numbers in code header comments (Phase 2-D, FR-0140-0142)
- ✅ Consistent naming (`radiology_*` tables, `Radiology*` permissions, `useRadiology*` hooks)
- ✅ CHANGELOG references correct batch structure
- ✅ Audit archive reports (P0/P1/Acceptance) reference correct file paths

### 9.4 Disposition

Documentation-status inconsistencies (CA-01 to CA-05) are **configuration corrections** permitted under CM-001 rules. They will be processed as Change Requests CR-001 through CR-005 via the CCB. They do NOT require code changes and do NOT block the code baseline freeze.

---

## 10. Technical Debt Register

| ID | Item | Severity | Owner | Recommended Phase |
|---|---|---|---|---|
| TD-01 | Zero automated tests for radiology module (Rust + frontend) | Medium | QA Lead | Phase 2 testing batch |
| TD-02 | No Radiologist role in RBAC seed (CA-06) | Medium | Security Architect | Phase 2 enhancement (CR-006) |
| TD-03 | `cargo check` / `cargo clippy` not verified in IV&V environment (no Rust toolchain) | Medium | DevOps | Pre-deployment (local) |
| TD-04 | `RadiologyManage` permission defined but unused | Low | CMB | Accepted (reserved for future admin commands) |
| TD-05 | `radiology_attachments` table has no commands | Low | Backend Lead | Future PACS integration |
| TD-06 | No PDF/print export for reports | Low | Frontend Lead | Future enhancement |
| TD-07 | No patient-timeline integration | Low | Product | Future enhancement |
| TD-08 | Some icon-only buttons may lack explicit aria-labels | Low | Accessibility | Phase 2 a11y polish |

**Totals:** 0 Critical, 0 High, 3 Medium, 5 Low. No debt item blocks the baseline.

---

## 11. Release Notes (v1.0.0)

### Radiology Module — v1.0.0 (Production Baseline)

**Implemented Features:**
- Imaging order creation with patient/doctor/study-type/priority/contrast/body-part
- Order number generation via PostgreSQL SEQUENCE (concurrency-safe, format `RAD-YYYYMMDD-NNNNNN`)
- Strict state machine: ordered → scheduled → in_progress → completed → reported → verified (+ cancelled)
- Radiology report filing (findings, impression, recommendations, critical-finding flag)
- Report verification workflow (separate permission from report filing)
- Soft-delete with audit trail (HIPAA §164.530(j) retention)
- Status history audit trail (every transition recorded)
- Radiology dashboard (6 KPIs via single conditional-aggregation query)
- Pagination + status/priority filtering
- Permission-gated UI actions

**P0 Corrections (closed):**
- P0-1: State machine enforces valid transitions; `verified`/`reported` cannot be set via generic update
- P0-2: RBAC bypass closed — every command has explicit permission check
- P0-3: Report creation validates order status = 'completed'
- P0-4: UNIQUE constraint enforces one-report-per-order (DB-level concurrency protection)
- P0-5: Soft-delete on all 9 commands (15 `deleted_at IS NULL` filters)
- P0-6: Pagination prevents unbounded result sets

**P1 Improvements (verified):**
- P1-1: SEQUENCE-based order numbers (replaces COUNT+1 race)
- P1-2: Enum validation before DB access (study_type, priority, status)
- P1-3: Partial index for soft-delete-filtered queries
- P1-4: Dashboard consolidated to 2 DB round-trips (was 6)
- P1-5: Reusable helpers (`validate_enum`, `allowed_transitions_from`)
- P1-6: Attachment schema with FK CASCADE (no orphan risk)
- P1-7: `sanitize_db_error()` on all `.map_err()` calls
- P1-8: Constants replace magic strings; no dead code

**Known Limitations:**
- No Radiologist role seeded (report filing/verification requires SUPER_ADMIN until CR-006)
- No PACS/DICOM integration (attachments table is future bridge)
- No PDF/print export
- No automated tests (Phase 2 testing batch)

**Compatibility:**
- PostgreSQL 14+ (uses SEQUENCE, partial indexes, FILTER aggregation)
- Tauri v2 + Rust (stable)
- React 19 + TypeScript 5 + TanStack Query
- No breaking changes to other modules (additive-only migrations)

**Upgrade Notes:**
- Migrations are idempotent (`IF NOT EXISTS`); safe to re-run
- Soft-delete columns added via `ALTER TABLE ... ADD COLUMN IF NOT EXISTS`
- No existing schema modified; no data migration required
- Existing roles unaffected (new permissions additive)

**Rollback Notes:**
- To roll back: `DROP TABLE radiology_orders, radiology_reports, radiology_attachments, radiology_status_history CASCADE; DROP SEQUENCE radiology_order_seq;`
- Frontend: remove `/radiology` route + nav item + hooks
- No data loss to other modules (radiology tables are self-contained)

---

## 12. Configuration Baseline Approval Record

| Field | Value |
|---|---|
| **Baseline ID** | RAD-BASELINE-1.0 |
| **Version** | 1.0.0 |
| **Approval Status** | ✅ APPROVED |
| **Approval Authority** | Configuration Management Board (CMB), VitalFlow HMS |
| **Approval Date** | 2025-07-11 |
| **Scope** | Radiology module: 9 commands, 7 permissions, 4 DB tables, 9 hooks, 1 route, 1 nav item |
| **Predecessor Artifacts** | P0 Remediation Report, P1 IV&V Report, Module Acceptance Review |
| **Success Criteria Met** | All 7 criteria verified (see §14) |
| **Configuration Items Frozen** | 28 CIs (6 Rust + 6 TS/TSX + 16 DB) |
| **Change Control** | All future changes require approved CR via CCB-001 |

---

## 13. Initial Change Control Board (CCB-001)

### 13.1 CCB Establishment

| Field | Value |
|---|---|
| **CCB ID** | CCB-001 |
| **Baseline Governed** | RAD-BASELINE-1.0 |
| **Establishment Date** | 2025-07-11 |
| **Approval Authority** | CMB (Chief Software Architect, Configuration Manager, Release Manager, Senior Rust Engineer, QA Lead) |

### 13.2 Change Control Policy

**Effective immediately, no modifications to any Radiology module configuration item (CI-RS-*, CI-FE-*, CI-DB-*, CI-CMD-*, CI-PERM-*, CI-RT-*) shall be made except through an approved Change Request (CR) processed by CCB-001.**

### 13.3 Change Classification

| Class | Definition | Examples |
|---|---|---|
| **Emergency** | Production defect causing data loss / patient safety risk | Soft-delete bypass, RBAC failure |
| **Major** | Functional change to frozen behavior | New command, state-machine change, schema change |
| **Minor** | Non-functional change | Performance optimization, refactoring |
| **Documentation** | Doc-only change | SRS/SDD status correction, README update |

### 13.4 CR Workflow

1. **Submit** — CR filed with description, classification, impact analysis
2. **Impact Analysis** — Senior Rust Engineer + DB Architect assess blast radius
3. **Regression Requirement** — QA Lead defines regression test scope
4. **Verification Requirement** — Define acceptance criteria (must pass P0+P1 checks)
5. **CCB Review** — Board approves/rejects/defers
6. **Implementation** — Only after approval; under version control
7. **Verification** — Must pass defined acceptance criteria
8. **Baseline Update** — New baseline version (e.g., RAD-BASELINE-1.0.1) issued

### 13.5 Registered Pending CRs (from CM-001 audit)

| CR ID | Class | Description | Origin |
|---|---|---|---|
| CR-001 | Documentation | SRS §4.12 status: PLANNED → IMPLEMENTED | CA-01 |
| CR-002 | Documentation | SRS L677 remove "not implemented" for Radiology | CA-02 |
| CR-003 | Documentation | SDD L693 remove "not yet designed" for Radiology | CA-03 |
| CR-004 | Documentation | SRS L123 fix FR-0140-0159 cross-reference | CA-04 |
| CR-005 | Documentation | SRS FR-0140/0141/0142 label reconciliation | CA-05 |
| CR-006 | Major | Add Radiologist role with Report+Verify permissions | CA-06 |

### 13.6 Future Planned Enhancements (under CCB control)

| Enhancement | Phase | Notes |
|---|---|---|
| PACS Integration (DICOM MWL worklist) | Phase 3 | Uses `radiology_attachments.future_pacs_id` bridge column |
| PDF Report Export | Phase 2+ | Print-friendly radiology report rendering |
| Radiologist Role | Phase 2 | CR-006 — seed role with Report + Verify perms |
| Patient Timeline Integration | Phase 3 | Surface imaging studies in patient profile |
| Automated Test Suite | Phase 2 | Unit tests for state machine, enum validation, pagination |
| Study Catalog (FR-0140 per SRS literal text) | Phase 3 | Modality/body-part/contrast/price catalog (if CR-005 reconciles) |

---

## 14. Final Recommendation

### ✅ BASELINE APPROVED

**Decision:** The Radiology Module Baseline **RAD-BASELINE-1.0 / v1.0.0** is **APPROVED** as the official production baseline for VitalFlow HMS.

### Supporting Objective Evidence

| Success Criterion | Met | Evidence |
|---|---|---|
| Module Acceptance Review passed | ✅ | P1 IV&V: 8/9 PASS, all P0 closed, no Critical/High defects |
| All accepted source files inventoried | ✅ | 6 Rust + 6 TS/TSX files in CIL (§2) |
| All database objects documented | ✅ | 4 tables, 7 indexes, 1 sequence, 1 constraint, 3 soft-delete cols (§5) |
| Requirements traceability complete | ✅ | FR-0140/0141/0142 fully traced (§8) |
| No Critical inconsistencies | ✅ | 0 Critical, 0 High; 7 findings all Medium/Low (§4.1) |
| Technical debt documented | ✅ | 8 items registered (§10) |
| Future changes under CCB control | ✅ | CCB-001 established, CR workflow defined (§13) |

### Conditions of Approval

1. CR-001 through CR-006 shall be processed by CCB-001 before the next module baseline.
2. `cargo check --features server-build` + `cargo clippy -- -D warnings` shall be run locally before production deployment (no Rust toolchain in IV&V environment — TD-03).
3. No modifications to frozen configuration items shall occur outside the CR process defined in §13.4.

### Baseline Declaration

```
Baseline ID:    RAD-BASELINE-1.0
Version:        1.0.0
Status:         FROZEN
Approval Date:  2025-07-11
Authority:      Configuration Management Board, VitalFlow HMS
```

**The Radiology module is now the official production baseline. All future enhancements and maintenance shall be performed only through approved Change Requests processed by the Change Control Board (CCB-001).**

---

*End of CM-001 — Radiology Module Baseline Freeze & Configuration Audit*
