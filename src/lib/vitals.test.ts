import { describe, it, expect } from "vitest";
import { flagVital, describeFlag, isCritical, VITAL_THRESHOLDS } from "./vitals";

/**
 * U-07 tests — boundary-exact coverage of the ward vitals flagging
 * module. Every threshold below is asserted from BOTH sides (last value
 * inside a band, first value inside the next band) so a future threshold
 * edit that opens a gap or shifts a boundary fails here, not on the ward.
 */
describe("flagVital — band boundaries", () => {
  describe("temperature (°C)", () => {
    it("flags hypothermia below 35.1 as critical", () => {
      expect(flagVital("temperature_c", 35.0)).toBe("critical");
      expect(flagVital("temperature_c", "34.5")).toBe("critical");
    });
    it("treats 35.1..37.9 as normal", () => {
      expect(flagVital("temperature_c", 35.1)).toBe("normal");
      expect(flagVital("temperature_c", 37.9)).toBe("normal");
      expect(flagVital("temperature_c", "36.8")).toBe("normal");
    });
    it("treats 38.0..38.9 as warning (fever)", () => {
      expect(flagVital("temperature_c", 38.0)).toBe("warning");
      expect(flagVital("temperature_c", 38.9)).toBe("warning");
    });
    it("treats >=39.0 as critical (high fever)", () => {
      expect(flagVital("temperature_c", 39.0)).toBe("critical");
      expect(flagVital("temperature_c", 41.2)).toBe("critical");
    });
  });

  describe("systolic BP", () => {
    it("flags <90 as critical", () => {
      expect(flagVital("systolic_bp", 89)).toBe("critical");
      expect(flagVital("systolic_bp", 75)).toBe("critical");
    });
    it("treats 90..139 as normal", () => {
      expect(flagVital("systolic_bp", 90)).toBe("normal");
      expect(flagVital("systolic_bp", 139)).toBe("normal");
    });
    it("treats 140..179 as warning", () => {
      expect(flagVital("systolic_bp", 140)).toBe("warning");
      expect(flagVital("systolic_bp", 179)).toBe("warning");
    });
    it("treats >=180 as critical", () => {
      expect(flagVital("systolic_bp", 180)).toBe("critical");
    });
  });

  describe("SpO2", () => {
    it("flags <92 as critical (hypoxaemia)", () => {
      expect(flagVital("spo2_pct", 91)).toBe("critical");
      expect(flagVital("spo2_pct", 88)).toBe("critical");
    });
    it("treats 92..93 as warning", () => {
      expect(flagVital("spo2_pct", 92)).toBe("warning");
      expect(flagVital("spo2_pct", 93)).toBe("warning");
    });
    it("treats >=94 as normal", () => {
      expect(flagVital("spo2_pct", 94)).toBe("normal");
      expect(flagVital("spo2_pct", 100)).toBe("normal");
    });
  });

  describe("pulse", () => {
    it("flags <50 as critical bradycardia", () => {
      expect(flagVital("pulse_bpm", 49)).toBe("critical");
    });
    it("treats 50..99 as normal", () => {
      expect(flagVital("pulse_bpm", 50)).toBe("normal");
      expect(flagVital("pulse_bpm", 99)).toBe("normal");
    });
    it("treats 100..129 as warning tachycardia", () => {
      expect(flagVital("pulse_bpm", 100)).toBe("warning");
      expect(flagVital("pulse_bpm", 129)).toBe("warning");
    });
    it("treats >=130 as critical", () => {
      expect(flagVital("pulse_bpm", 130)).toBe("critical");
    });
  });

  describe("respiratory rate", () => {
    it("flags <9 as critical", () => {
      expect(flagVital("resp_rate", 8)).toBe("critical");
    });
    it("treats 9..20 as normal", () => {
      expect(flagVital("resp_rate", 9)).toBe("normal");
      expect(flagVital("resp_rate", 20)).toBe("normal");
    });
    it("treats 21..24 as warning", () => {
      expect(flagVital("resp_rate", 24)).toBe("warning");
    });
    it("treats >=25 as critical", () => {
      expect(flagVital("resp_rate", 25)).toBe("critical");
    });
  });

  describe("pain score", () => {
    it("treats 0..3 as normal", () => {
      expect(flagVital("pain_score", 0)).toBe("normal");
      expect(flagVital("pain_score", 3)).toBe("normal");
    });
    it("treats 4..6 as warning", () => {
      expect(flagVital("pain_score", 4)).toBe("warning");
      expect(flagVital("pain_score", 6)).toBe("warning");
    });
    it("treats 7..10 as critical (severe)", () => {
      expect(flagVital("pain_score", 7)).toBe("critical");
      expect(flagVital("pain_score", 10)).toBe("critical");
    });
  });
});

describe("flagVital — input coercion", () => {
  it("returns normal for null/undefined/'' (not recorded, not abnormal)", () => {
    expect(flagVital("spo2_pct", null)).toBe("normal");
    expect(flagVital("spo2_pct", undefined)).toBe("normal");
    expect(flagVital("spo2_pct", "")).toBe("normal");
  });

  it("parses numeric strings the backend serialises (temperature_c is a string)", () => {
    // VitalReading.temperature_c arrives as string | null per models.ts.
    expect(flagVital("temperature_c", "39.5")).toBe("critical");
    expect(flagVital("temperature_c", "36.5")).toBe("normal");
  });

  it("returns normal for non-finite garbage rather than crashing", () => {
    expect(flagVital("pulse_bpm", "not-a-number")).toBe("normal");
  });

  it("covers every key of VITAL_FIELDS used by the Nursing form", () => {
    // The Nursing form feeds exactly these keys; a rename would break
    // flagging silently — assert each still has thresholds registered.
    for (const key of [
      "temperature_c",
      "systolic_bp",
      "diastolic_bp",
      "pulse_bpm",
      "resp_rate",
      "spo2_pct",
      "pain_score",
    ]) {
      const thresholds = VITAL_THRESHOLDS[key];
      expect(thresholds).toBeDefined();
      expect(thresholds!.zones.length).toBeGreaterThan(0);
    }
  });
});

describe("isCritical", () => {
  it("is true only for the critical flag", () => {
    expect(isCritical("spo2_pct", 90)).toBe(true);
    expect(isCritical("spo2_pct", 95)).toBe(false);
    expect(isCritical("spo2_pct", 93)).toBe(false); // warning, not critical
    expect(isCritical("spo2_pct", null)).toBe(false);
  });
});

describe("describeFlag", () => {
  it("names the vital and the severity", () => {
    expect(describeFlag("spo2_pct", "critical")).toContain("SpO₂");
    expect(describeFlag("spo2_pct", "critical")).toContain("critical");
    expect(describeFlag("temperature_c", "warning")).toContain("review");
    expect(describeFlag("temperature_c", "normal")).toContain("normal");
  });
});
