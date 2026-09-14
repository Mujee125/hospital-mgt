import { describe, it, expect } from "vitest";
import {
  parseSelection,
  joinSelection,
  mergeOptions,
  ALLERGY_PRESETS,
  CHRONIC_CONDITION_PRESETS,
  SPECIALIZATION_PRESETS,
  QUALIFICATION_PRESETS,
} from "./clinicalPresets";

describe("parseSelection", () => {
  it("splits a stored comma-separated value, trimming each item", () => {
    expect(parseSelection("Penicillin, Latex ,Soy")).toEqual([
      "Penicillin",
      "Latex",
      "Soy",
    ]);
  });

  it("drops empty entries without leaving holes", () => {
    expect(parseSelection(" , , ,")).toEqual([]);
    expect(parseSelection("A,,B, ")).toEqual(["A", "B"]);
  });

  it("returns [] for null/undefined/empty (no allergies recorded ≠ abnormal)", () => {
    expect(parseSelection(null)).toEqual([]);
    expect(parseSelection(undefined)).toEqual([]);
    expect(parseSelection("")).toEqual([]);
  });

  it("de-duplicates case-insensitively keeping the FIRST casing", () => {
    // A hand-typed "penicillin" stored under an existing "Penicillin"
    // collapses into one item — this is what keeps preset chips in sync
    // with legacy free-text values.
    expect(parseSelection("Penicillin, penicillin, PENICILLIN")).toEqual([
      "Penicillin",
    ]);
  });

  it("does not split on non-comma separators (spaces stay in-item)", () => {
    expect(parseSelection("Sulfa drugs")).toEqual(["Sulfa drugs"]);
    expect(parseSelection("Diabetes — Type 2")).toEqual(["Diabetes — Type 2"]);
  });
});

describe("joinSelection", () => {
  it("joins with the stored comma+space format", () => {
    expect(joinSelection(["Penicillin", "Latex"])).toBe("Penicillin, Latex");
  });

  it("round-trips through parseSelection", () => {
    const original = "Penicillin, Latex, Soy";
    expect(joinSelection(parseSelection(original))).toBe(original);
  });
});

describe("mergeOptions", () => {
  it("presets come first, then additional values, case-insensitively deduped", () => {
    expect(
      mergeOptions(["Cardiology", "ENT"], ["cardiology", "Neurology", ""]),
    ).toEqual(["Cardiology", "ENT", "Neurology"]);
  });

  it("handles undefined additional lists (DB query still loading)", () => {
    expect(mergeOptions(["A"], undefined, ["B"])).toEqual(["A", "B"]);
  });

  it("skips blank/whitespace options (Radix Select forbids empty values)", () => {
    expect(mergeOptions(["A"], ["  ", "B"])).toEqual(["A", "B"]);
  });
});

describe("preset lists (contract guards)", () => {
  it("no preset contains a comma (comma is the value separator)", () => {
    for (const list of [
      ALLERGY_PRESETS,
      CHRONIC_CONDITION_PRESETS,
      SPECIALIZATION_PRESETS,
      QUALIFICATION_PRESETS,
    ]) {
      for (const item of list) {
        expect(item).not.toMatch(/,/);
        expect(item.trim()).toBe(item);
      }
    }
  });

  it("presets are unique within each list", () => {
    for (const list of [
      ALLERGY_PRESETS,
      CHRONIC_CONDITION_PRESETS,
      SPECIALIZATION_PRESETS,
      QUALIFICATION_PRESETS,
    ]) {
      const keys = list.map((s) => s.toLowerCase());
      expect(new Set(keys).size).toBe(keys.length);
    }
  });

  it("covers the common clinical entries the forms rely on", () => {
    expect(ALLERGY_PRESETS).toContain("Penicillin");
    expect(ALLERGY_PRESETS).toContain("Latex");
    expect(CHRONIC_CONDITION_PRESETS).toContain("Hypertension");
    expect(SPECIALIZATION_PRESETS).toContain("Cardiology");
    expect(QUALIFICATION_PRESETS).toContain("MBBS");
  });
});
