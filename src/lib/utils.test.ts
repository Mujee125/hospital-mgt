import { describe, it, expect } from "vitest";
import {
  CURRENCY,
  CURRENCY_SYMBOL,
  formatMoney,
  patientMrn,
  formatDob,
  patientDescriptor,
} from "@/lib/utils";

describe("CURRENCY constants", () => {
  it("CURRENCY is PKR", () => {
    expect(CURRENCY).toBe("PKR");
  });

  it("CURRENCY_SYMBOL is Rs", () => {
    expect(CURRENCY_SYMBOL).toBe("Rs");
  });
});

describe("formatMoney", () => {
  it("formats a whole number", () => {
    expect(formatMoney(500)).toBe("Rs 500");
  });

  it("formats a decimal number", () => {
    expect(formatMoney(1500.5)).toBe("Rs 1,500.5");
  });

  it("formats a string number", () => {
    expect(formatMoney("123.45")).toBe("Rs 123.45");
  });

  it("formats zero", () => {
    expect(formatMoney(0)).toBe("Rs 0");
  });

  it("formats null as dash", () => {
    expect(formatMoney(null)).toBe("—");
  });

  it("formats NaN as dash", () => {
    expect(formatMoney(NaN)).toBe("—");
  });

  it("formats Infinity as dash", () => {
    expect(formatMoney(Infinity)).toBe("—");
  });

  it("formats negative Infinity as dash", () => {
    expect(formatMoney(-Infinity)).toBe("—");
  });

  it("formats undefined as dash", () => {
    expect(formatMoney(undefined as unknown as null)).toBe("—");
  });

  it("formats a large number with commas", () => {
    const result = formatMoney(1234567);
    expect(result).toContain("Rs");
    expect(result).toContain("1,234,567");
  });

  it("formats a string with decimal", () => {
    expect(formatMoney("99.99")).toBe("Rs 99.99");
  });

  it("formats an empty string as dash", () => {
    expect(formatMoney("")).toBe("—");
  });
});

// ── RCTF-FULL-SYSTEM-2026-09-09 F-02: patient-identity helpers ─────────────

describe("patientMrn (two-identifier display)", () => {
  it("returns the MRN when present", () => {
    expect(patientMrn("MRN-000123")).toBe("MRN-000123");
  });

  it("trims whitespace", () => {
    expect(patientMrn("  MRN-000123  ")).toBe("MRN-000123");
  });

  it("returns 'no MRN' for null/undefined/empty — never a blank picker entry", () => {
    expect(patientMrn(null)).toBe("no MRN");
    expect(patientMrn(undefined)).toBe("no MRN");
    expect(patientMrn("")).toBe("no MRN");
    expect(patientMrn("   ")).toBe("no MRN");
  });
});

describe("formatDob", () => {
  it("formats an ISO date as 'DD Mon YYYY'", () => {
    expect(formatDob("1990-05-12")).toBe("12 May 1990");
  });

  it("returns 'DOB unknown' for null/undefined", () => {
    expect(formatDob(null)).toBe("DOB unknown");
    expect(formatDob(undefined)).toBe("DOB unknown");
  });

  it("returns the raw string when unparsable (never 'Invalid Date')", () => {
    expect(formatDob("not-a-date")).toBe("not-a-date");
  });
});

describe("patientDescriptor (picker line)", () => {
  const base = {
    first_name: "Ali",
    last_name: "Raza",
    phone: "03001234567",
  };

  it("renders name · MRN · DOB · phone for a fully-identified patient", () => {
    const d = patientDescriptor({
      ...base,
      mrn: "MRN-000123",
      date_of_birth: "1990-05-12",
    });
    expect(d).toBe("Ali Raza · MRN MRN-000123 · DOB 12 May 1990 · 03001234567");
  });

  it("falls back to 'no MRN' when MRN is absent — the second identifier slot stays visible", () => {
    const d = patientDescriptor({ ...base, mrn: null });
    expect(d).toBe("Ali Raza · no MRN · 03001234567");
  });

  it("omits the DOB segment entirely when unknown (no dangling separators)", () => {
    const d = patientDescriptor({
      ...base,
      mrn: "MRN-000123",
      date_of_birth: null,
    });
    expect(d).toBe("Ali Raza · MRN MRN-000123 · 03001234567");
  });

  it("handles a patient with neither MRN nor DOB (minimum viable descriptor)", () => {
    const d = patientDescriptor({ ...base, mrn: "", date_of_birth: "" });
    expect(d).toBe("Ali Raza · no MRN · 03001234567");
  });
});
