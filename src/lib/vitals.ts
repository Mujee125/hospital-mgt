/**
 * Vital-sign abnormality flagging (RCTF UX-2026-09-13 U-07).
 *
 * The Nursing vitals table previously rendered raw numbers with NO
 * abnormality indication — a nurse scanning 30 rows across 8 admitted
 * patients had to recall every threshold mentally. The backend enforces
 * plausibility (F-22: is the VALUE physically possible?) but nothing
 * marked CLINICAL abnormality (is this reading dangerous?).
 *
 * Thresholds follow widely-taught adult ward standards (NEWS2-adjacent).
 * Each kind is a list of ascending, gap-free zones — a value matches the
 * first zone whose upper bound (exclusive) exceeds it — so no decimal
 * value can slip between bands. These are presentation-layer flags
 * only: never a substitute for clinical judgment, and the backend
 * remains the enforcement boundary for what may be STORED. Color is
 * always paired with an icon + text (never color alone, WCAG 1.4.1).
 *
 * Keep the outer bounds in sync with the backend plausibility guard
 * (record_vitals_core): a flag here must never claim a value is normal
 * that the backend refuses, or vice versa.
 */

export type VitalFlag = "critical" | "warning" | "normal";

export interface VitalZone {
  /** Zone applies when value < lt (strictly less than). */
  lt: number;
  flag: VitalFlag;
}

export const VITAL_THRESHOLDS: Record<string, { label: string; zones: VitalZone[] }> = {
  temperature_c: {
    label: "Temperature (°C)",
    zones: [
      { lt: 35.1, flag: "critical" }, // hypothermia
      { lt: 38.0, flag: "normal" },
      { lt: 39.0, flag: "warning" }, // fever
      { lt: Infinity, flag: "critical" }, // high fever
    ],
  },
  systolic_bp: {
    label: "Systolic BP",
    zones: [
      { lt: 90, flag: "critical" },
      { lt: 140, flag: "normal" },
      { lt: 180, flag: "warning" }, // hypertension stage
      { lt: Infinity, flag: "critical" }, // severe
    ],
  },
  diastolic_bp: {
    label: "Diastolic BP",
    zones: [
      { lt: 60, flag: "critical" },
      { lt: 90, flag: "normal" },
      { lt: 110, flag: "warning" },
      { lt: Infinity, flag: "critical" },
    ],
  },
  pulse_bpm: {
    label: "Pulse (bpm)",
    zones: [
      { lt: 50, flag: "critical" }, // bradycardia
      { lt: 100, flag: "normal" },
      { lt: 130, flag: "warning" }, // tachycardia
      { lt: Infinity, flag: "critical" },
    ],
  },
  resp_rate: {
    label: "Resp. rate",
    zones: [
      { lt: 9, flag: "critical" },
      { lt: 21, flag: "normal" },
      { lt: 25, flag: "warning" },
      { lt: Infinity, flag: "critical" },
    ],
  },
  spo2_pct: {
    label: "SpO₂ (%)",
    zones: [
      { lt: 92, flag: "critical" }, // hypoxaemia
      { lt: 94, flag: "warning" },
      { lt: Infinity, flag: "normal" },
    ],
  },
  pain_score: {
    label: "Pain score (0–10)",
    zones: [
      { lt: 4, flag: "normal" },
      { lt: 7, flag: "warning" },
      { lt: Infinity, flag: "critical" }, // severe
    ],
  },
} as const;

export type VitalKind = keyof typeof VITAL_THRESHOLDS;

/** Flag one vital reading. Returns "normal" for null/undefined/"" (not
 *  recorded — not abnormal), matching the table's "—" rendering. */
export function flagVital(
  kind: VitalKind,
  value: number | string | null | undefined,
): VitalFlag {
  if (value === null || value === undefined || value === "") return "normal";
  const n = typeof value === "string" ? parseFloat(value) : value;
  if (!isFinite(n)) return "normal";
  const zones = VITAL_THRESHOLDS[kind]?.zones;
  if (!zones || zones.length === 0) return "normal";
  for (const z of zones) {
    if (n < z.lt) return z.flag;
  }
  return zones[zones.length - 1]!.flag;
}

/** Human explanation for the flag chip's title/aria-label. */
export function describeFlag(kind: VitalKind, flag: VitalFlag): string {
  const label = VITAL_THRESHOLDS[kind]?.label ?? String(kind);
  switch (flag) {
    case "critical":
      return `${label}: critical range — escalate per ward protocol`;
    case "warning":
      return `${label}: outside normal range — review`;
    default:
      return `${label}: within normal range`;
  }
}

/** Does this reading need the red escalation treatment? (Table-cell
 *  highlighting + row-level escalation cues.) */
export function isCritical(
  kind: VitalKind,
  value: number | string | null | undefined,
): boolean {
  return flagVital(kind, value) === "critical";
}
