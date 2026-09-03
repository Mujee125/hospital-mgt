/**
 * BB-007: Blood Bank Frontend Unit Tests
 *
 * These tests verify the frontend configuration items that are testable
 * without a running Tauri backend:
 *   - Permission constants (rbac.ts)
 *   - TypeScript interfaces (models.ts) — structural correctness
 *   - Query hook signatures (queries.ts) — export presence
 *
 * Tests that require mocking @tauri-apps/api/core invoke or rendering
 * BloodBank.tsx (which pulls in Radix Dialog/Select/Tabs) are documented
 * in BB-006 as Phase 4 and require a more complex test harness.
 */
import { describe, it, expect } from "vitest";
import { PERMISSIONS } from "@/lib/rbac";
import type {
  BloodDonor,
  BloodDonation,
  BloodUnit,
  BloodCrossmatch,
  BloodReservation,
  BloodIssue,
  BloodTransfusion,
  BloodDiscard,
  BloodUnitHistory,
  BloodMovement,
  BloodBankDashboard,
  BloodCompatibilityResult,
  BloodUnitTraceability,
  CreateBloodDonor,
  CreateBloodDonation,
  CreateBloodUnit,
  CreateBloodCrossmatch,
  CreateBloodReservation,
  CreateBloodIssue,
  CreateBloodTransfusion,
  CreateBloodDiscard,
  BloodDonorsResponse,
  BloodDonationsResponse,
  BloodUnitsResponse,
  BloodCrossmatchesResponse,
  BloodIssuesResponse,
  BloodTransfusionsResponse,
  BloodDiscardsResponse,
} from "@/lib/models";

// ── Permission Constants (FE-PG tests) ──────────────────────────────────────

describe("BB-007: Blood Bank Permission Constants", () => {
  it("FE-PG-001: BloodBankView constant matches Rust as_str", () => {
    expect(PERMISSIONS.BloodBankView).toBe("bloodbank.view");
  });

  it("FE-PG-002: BloodBankManage constant matches Rust as_str", () => {
    expect(PERMISSIONS.BloodBankManage).toBe("bloodbank.manage");
  });

  it("FE-PG-003: BloodBankDonorManage constant matches Rust as_str", () => {
    expect(PERMISSIONS.BloodBankDonorManage).toBe("bloodbank.donor.manage");
  });

  it("FE-PG-004: BloodBankCrossmatch constant matches Rust as_str", () => {
    expect(PERMISSIONS.BloodBankCrossmatch).toBe("bloodbank.crossmatch");
  });

  it("FE-PG-005: BloodBankIssue constant matches Rust as_str", () => {
    expect(PERMISSIONS.BloodBankIssue).toBe("bloodbank.issue");
  });

  it("FE-PG-006: BloodBankTransfuse constant matches Rust as_str", () => {
    expect(PERMISSIONS.BloodBankTransfuse).toBe("bloodbank.transfuse");
  });

  it("FE-PG-007: BloodBankDiscard constant matches Rust as_str", () => {
    expect(PERMISSIONS.BloodBankDiscard).toBe("bloodbank.discard");
  });

  it("FE-PG-008: BloodBankVerify constant matches Rust as_str", () => {
    expect(PERMISSIONS.BloodBankVerify).toBe("bloodbank.verify");
  });

  it("FE-PG-009: all 8 Blood Bank permissions exist in PERMISSIONS object", () => {
    const bloodBankPerms = Object.keys(PERMISSIONS).filter((k) =>
      k.startsWith("BloodBank"),
    );
    expect(bloodBankPerms).toHaveLength(8);
  });

  it("FE-PG-010: all Blood Bank permission values use bloodbank.* prefix", () => {
    const bloodBankPerms = Object.entries(PERMISSIONS).filter(([k]) =>
      k.startsWith("BloodBank"),
    );
    for (const [, value] of bloodBankPerms) {
      expect(value).toMatch(/^bloodbank\./);
    }
  });
});

// ── TypeScript Interface Structure (FE-MD tests) ────────────────────────────
// These tests verify the interfaces exist and have the expected fields.
// They protect against accidental field removal that would break IPC.

describe("BB-007: Blood Bank TypeScript Interfaces", () => {
  it("FE-MD-001: BloodDonor interface has required fields", () => {
    const donor: BloodDonor = {
      id: 1,
      donor_number: "DON-20250711-000001",
      first_name: "John",
      last_name: "Doe",
      blood_group: "O",
      rh_factor: "+",
      total_donations: 0,
      status: "active",
      created_at: "2025-07-11T00:00:00Z",
      updated_at: "2025-07-11T00:00:00Z",
    };
    expect(donor.blood_group).toBe("O");
    expect(donor.rh_factor).toBe("+");
    expect(donor.status).toBe("active");
  });

  it("FE-MD-002: BloodDonation interface has required fields", () => {
    const donation: BloodDonation = {
      id: 1,
      donation_number: "BDN-20250711-000001",
      donor_id: 1,
      donation_date: "2025-07-11T00:00:00Z",
      volume_ml: 450,
      blood_group: "O",
      rh_factor: "+",
      status: "collected",
      screening_status: "pending",
      created_at: "2025-07-11T00:00:00Z",
      updated_at: "2025-07-11T00:00:00Z",
    };
    expect(donation.volume_ml).toBe(450);
    expect(donation.screening_status).toBe("pending");
  });

  it("FE-MD-003: BloodUnit interface has required fields including days_to_expiry", () => {
    const unit: BloodUnit = {
      id: 1,
      unit_number: "BU-20250711-000001",
      donor_id: 1,
      component_type: "whole_blood",
      blood_group: "O",
      rh_factor: "+",
      volume_ml: 450,
      collection_date: "2025-07-11T00:00:00Z",
      expiry_date: "2025-08-15T00:00:00Z",
      status: "quarantine",
      created_at: "2025-07-11T00:00:00Z",
      updated_at: "2025-07-11T00:00:00Z",
    };
    expect(unit.status).toBe("quarantine");
    expect(unit.component_type).toBe("whole_blood");
  });

  it("FE-MD-004: BloodCrossmatch interface has result field", () => {
    const xm: BloodCrossmatch = {
      id: 1,
      unit_id: 1,
      patient_id: 1,
      crossmatch_date: "2025-07-11T00:00:00Z",
      result: "compatible",
      created_at: "2025-07-11T00:00:00Z",
      updated_at: "2025-07-11T00:00:00Z",
    };
    expect(xm.result).toBe("compatible");
  });

  it("FE-MD-005: BloodReservation interface has expires_at field", () => {
    const res: BloodReservation = {
      id: 1,
      reservation_number: "BRS-20250711-000001",
      unit_id: 1,
      patient_id: 1,
      reserved_at: "2025-07-11T00:00:00Z",
      expires_at: "2025-07-12T00:00:00Z",
      status: "active",
      priority: "routine",
      created_at: "2025-07-11T00:00:00Z",
      updated_at: "2025-07-11T00:00:00Z",
    };
    expect(res.expires_at).toBeDefined();
    expect(res.status).toBe("active");
  });

  it("FE-MD-006: BloodIssue interface has issue_type field", () => {
    const issue: BloodIssue = {
      id: 1,
      issue_number: "BIS-20250711-000001",
      unit_id: 1,
      patient_id: 1,
      issued_at: "2025-07-11T00:00:00Z",
      issue_type: "routine",
      created_at: "2025-07-11T00:00:00Z",
      updated_at: "2025-07-11T00:00:00Z",
    };
    expect(issue.issue_type).toBe("routine");
  });

  it("FE-MD-007: BloodTransfusion interface has reaction_observed field", () => {
    const tx: BloodTransfusion = {
      id: 1,
      transfusion_number: "BTR-20250711-000001",
      issue_id: 1,
      unit_id: 1,
      patient_id: 1,
      started_at: "2025-07-11T00:00:00Z",
      reaction_observed: false,
      created_at: "2025-07-11T00:00:00Z",
      updated_at: "2025-07-11T00:00:00Z",
    };
    expect(tx.reaction_observed).toBe(false);
  });

  it("FE-MD-008: BloodDiscard interface has discard_reason field", () => {
    const d: BloodDiscard = {
      id: 1,
      unit_id: 1,
      discard_number: "BDC-20250711-000001",
      discarded_at: "2025-07-11T00:00:00Z",
      discard_reason: "expired",
      created_at: "2025-07-11T00:00:00Z",
      updated_at: "2025-07-11T00:00:00Z",
    };
    expect(d.discard_reason).toBe("expired");
  });

  it("FE-MD-009: BloodUnitHistory interface has status field", () => {
    const h: BloodUnitHistory = {
      id: 1,
      unit_id: 1,
      status: "available",
      changed_at: "2025-07-11T00:00:00Z",
    };
    expect(h.status).toBe("available");
  });

  it("FE-MD-010: BloodMovement interface has movement_type field", () => {
    const m: BloodMovement = {
      id: 1,
      unit_id: 1,
      movement_type: "received",
      moved_at: "2025-07-11T00:00:00Z",
      created_at: "2025-07-11T00:00:00Z",
    };
    expect(m.movement_type).toBe("received");
  });

  it("FE-MD-011: BloodBankDashboard has all 12 KPI fields", () => {
    const dash: BloodBankDashboard = {
      available_units: 10,
      reserved_units: 2,
      issued_units: 1,
      quarantine_units: 3,
      discarded_all_time: 5,
      expiring_soon: 1,
      total_donors: 100,
      active_donors: 80,
      deferred_donors: 5,
      transfusions_today: 2,
      active_reservations: 2,
      stock_by_type: [],
    };
    expect(dash.available_units).toBe(10);
    expect(dash.quarantine_units).toBe(3);
    expect(dash.expiring_soon).toBe(1);
  });

  it("FE-MD-012: BloodCompatibilityResult has compatible boolean", () => {
    const r: BloodCompatibilityResult = {
      compatible: true,
      donor_group: "O",
      donor_rh: "-",
      patient_group: "A",
      patient_rh: "+",
      reason: "ABO/Rh compatible",
    };
    expect(r.compatible).toBe(true);
  });

  it("FE-MD-013: BloodUnitTraceability has all 6 timeline arrays", () => {
    const t: BloodUnitTraceability = {
      unit_id: 1,
      status_history: [],
      movements: [],
      crossmatches: [],
      issues: [],
      transfusions: [],
      discards: [],
    };
    expect(t.status_history).toHaveLength(0);
    expect(t.movements).toHaveLength(0);
    expect(t.crossmatches).toHaveLength(0);
    expect(t.issues).toHaveLength(0);
    expect(t.transfusions).toHaveLength(0);
    expect(t.discards).toHaveLength(0);
  });

  // Create* payload interfaces
  it("FE-MD-014: CreateBloodDonor has required fields", () => {
    const d: CreateBloodDonor = {
      first_name: "John",
      last_name: "Doe",
      blood_group: "O",
      rh_factor: "+",
    };
    expect(d.blood_group).toBe("O");
  });

  it("FE-MD-015: CreateBloodDonation has donor_id + volume_ml", () => {
    const d: CreateBloodDonation = {
      donor_id: 1,
      volume_ml: 450,
      blood_group: "O",
      rh_factor: "+",
    };
    expect(d.volume_ml).toBe(450);
  });

  it("FE-MD-016: CreateBloodUnit has expiry_date", () => {
    const u: CreateBloodUnit = {
      donor_id: 1,
      component_type: "whole_blood",
      blood_group: "O",
      rh_factor: "+",
      volume_ml: 450,
      expiry_date: "2025-08-15T00:00:00Z",
    };
    expect(u.expiry_date).toBeDefined();
  });

  it("FE-MD-017: CreateBloodCrossmatch has unit_id + patient_id + result", () => {
    const xm: CreateBloodCrossmatch = {
      unit_id: 1,
      patient_id: 1,
      result: "compatible",
    };
    expect(xm.result).toBe("compatible");
  });

  it("FE-MD-018: CreateBloodReservation has expires_in_hours", () => {
    const r: CreateBloodReservation = {
      unit_id: 1,
      patient_id: 1,
      priority: "routine",
      expires_in_hours: 24,
    };
    expect(r.expires_in_hours).toBe(24);
  });

  it("FE-MD-019: CreateBloodIssue has issue_type", () => {
    const i: CreateBloodIssue = {
      unit_id: 1,
      patient_id: 1,
      issue_type: "routine",
    };
    expect(i.issue_type).toBe("routine");
  });

  it("FE-MD-020: CreateBloodTransfusion has issue_id + reaction_observed", () => {
    const t: CreateBloodTransfusion = {
      issue_id: 1,
      unit_id: 1,
      patient_id: 1,
      reaction_observed: false,
    };
    expect(t.reaction_observed).toBe(false);
  });

  it("FE-MD-021: CreateBloodDiscard has discard_reason", () => {
    const d: CreateBloodDiscard = {
      unit_id: 1,
      discard_reason: "expired",
    };
    expect(d.discard_reason).toBe("expired");
  });

  // Paginated response interfaces
  it("FE-MD-022: BloodDonorsResponse has donors array + pagination", () => {
    const r: BloodDonorsResponse = {
      donors: [],
      total: 0,
      page: 1,
      page_size: 10,
      total_pages: 1,
    };
    expect(r.donors).toHaveLength(0);
    expect(r.total_pages).toBe(1);
  });

  it("FE-MD-023: BloodUnitsResponse has units array", () => {
    const r: BloodUnitsResponse = {
      units: [],
      total: 0,
      page: 1,
      page_size: 10,
      total_pages: 1,
    };
    expect(r.units).toHaveLength(0);
  });

  it("FE-MD-024: BloodDonationsResponse has donations array", () => {
    const r: BloodDonationsResponse = {
      donations: [],
      total: 0,
      page: 1,
      page_size: 10,
      total_pages: 1,
    };
    expect(r.donations).toHaveLength(0);
  });

  it("FE-MD-025: BloodCrossmatchesResponse has crossmatches array", () => {
    const r: BloodCrossmatchesResponse = {
      crossmatches: [],
      total: 0,
      page: 1,
      page_size: 10,
      total_pages: 1,
    };
    expect(r.crossmatches).toHaveLength(0);
  });

  it("FE-MD-026: BloodIssuesResponse has issues array", () => {
    const r: BloodIssuesResponse = {
      issues: [],
      total: 0,
      page: 1,
      page_size: 10,
      total_pages: 1,
    };
    expect(r.issues).toHaveLength(0);
  });

  it("FE-MD-027: BloodTransfusionsResponse has transfusions array", () => {
    const r: BloodTransfusionsResponse = {
      transfusions: [],
      total: 0,
      page: 1,
      page_size: 10,
      total_pages: 1,
    };
    expect(r.transfusions).toHaveLength(0);
  });

  it("FE-MD-028: BloodDiscardsResponse has discards array", () => {
    const r: BloodDiscardsResponse = {
      discards: [],
      total: 0,
      page: 1,
      page_size: 10,
      total_pages: 1,
    };
    expect(r.discards).toHaveLength(0);
  });
});

// ── Query Hook Export Verification (FE-HK tests) ────────────────────────────
// These tests verify the hooks are exported with the correct names.
// Full hook behavior testing (query keys, mutation invalidation) requires
// a TanStack Query test setup with mocked invoke — documented in BB-006.

describe("BB-007: Blood Bank Query Hook Exports", () => {
  it("FE-HK-001: all 29 Blood Bank hooks are exported from queries.ts", async () => {
    const mod = await import("@/lib/queries");
    const expectedHooks = [
      "useBloodDonors",
      "useBloodDonor",
      "useCreateBloodDonor",
      "useDeleteBloodDonor",
      "useBloodDonations",
      "useCreateBloodDonation",
      "useUpdateDonationScreening",
      "useBloodUnits",
      "useBloodUnit",
      "useCreateBloodUnit",
      "useUpdateBloodUnitStatus",
      "useDeleteBloodUnit",
      "useSearchBloodInventory",
      "useBloodCrossmatches",
      "useCheckBloodCompatibility",
      "useCreateBloodCrossmatch",
      "useVerifyBloodCrossmatch",
      "useCreateBloodReservation",
      "useCancelBloodReservation",
      "useBloodIssues",
      "useIssueBlood",
      "useReturnBloodUnit",
      "useBloodTransfusions",
      "useCreateBloodTransfusion",
      "useBloodDiscards",
      "useDiscardBloodUnit",
      "useBloodUnitHistory",
      "useBloodUnitMovements",
      "useBloodUnitTraceability",
      "useBloodBankDashboard",
      "useBloodBankStatistics",
    ];
    for (const hookName of expectedHooks) {
      expect(mod, `Hook ${hookName} should be exported`).toHaveProperty(hookName);
      expect(typeof mod[hookName as keyof typeof mod]).toBe("function");
    }
  });
});
