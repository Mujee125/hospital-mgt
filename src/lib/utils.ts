import { type ClassValue, clsx } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

// ── Money / currency helpers ─────────────────────────────────────────────
//
// VitalFlow is deployed to Pakistani hospitals — every money figure shown
// to the user is in Pakistani Rupees (PKR). The previous implementation
// (Billing.tsx, Dashboard.tsx) hard-coded `currency: "USD"` and an `en-US`
// locale, producing wrong currency symbols ($ instead of Rs) and wrong
// grouping. Pages should call `formatMoney(amount)` instead of
// `Intl.NumberFormat("en-US", { style: "currency", currency: "USD" })`.
//
// The backend stores money as `NUMERIC(14,2)` and round-trips via
// `rust_decimal::Decimal`. With the `serde-with-float` Cargo feature
// (see src-tauri/Cargo.toml), the JSON serialises as an f64 number, so
// `amount` typically arrives as `number`. We also accept `string` so the
// helper keeps working if/when the backend flips to `serde-with-str`
// (see TYPE-04 / FIXME in models.ts).
export const CURRENCY = "PKR" as const;
export const CURRENCY_SYMBOL = "Rs" as const;

/**
 * Format a money amount for display. Handles both `number` (current
 * backend serialization) and `string` (future-proof if backend switches
 * to serde-with-str). Returns an em-dash for non-finite values so the
 * UI never renders `NaN`/`Infinity` to the operator.
 */
export function formatMoney(amount: number | string | null | undefined): string {
  if (amount === null || amount === undefined) return "—";
  const n = typeof amount === "string" ? parseFloat(amount) : amount;
  if (!isFinite(n)) return "—";
  return `${CURRENCY_SYMBOL} ${n.toLocaleString("en-PK", {
    minimumFractionDigits: 0,
    maximumFractionDigits: 2,
  })}`;
}

/**
 * Alias of `formatMoney` for code that reads more naturally with the
 * "currency" verb (e.g. `formatCurrency(revenueToday)`).
 */
export const formatCurrency = formatMoney;

// ── Patient-identity helpers (RCTF-FULL-SYSTEM-2026-09-09 F-02) ──────────
//
// Wrong-patient selection is the classic clinical error mode: two
// homonymous relatives sharing a family phone were indistinguishable in
// every patient picker. Every picker and patient-facing action surface must
// render a SECOND identifier (MRN) plus the date of birth alongside the
// name, per the two-identifier patient-safety convention.

/** Human label for a patient: "MRN-SYN-000001" or "no MRN". */
export function patientMrn(mrn: string | null | undefined): string {
  const trimmed = (mrn ?? "").trim();
  return trimmed.length > 0 ? trimmed : "no MRN";
}

/** YYYY-MM-DD → "12 May 1990" (or the raw string if unparsable). */
export function formatDob(dob: string | null | undefined): string {
  if (!dob) return "DOB unknown";
  const d = new Date(dob);
  if (isNaN(d.getTime())) return dob;
  return d.toLocaleDateString("en-GB", {
    day: "2-digit",
    month: "short",
    year: "numeric",
  });
}

/**
 * One-line patient descriptor for pickers and action dialogs:
 * "Ali Raza · MRN MRN-000123 · DOB 12 May 1990 · 03001234567".
 * MRN and DOB are the two-identifier guard; phone stays for lookup
 * convenience.
 */
export function patientDescriptor(p: {
  first_name: string;
  last_name: string;
  mrn?: string | null;
  date_of_birth?: string | null;
  phone: string;
}): string {
  const mrn = (p.mrn ?? "").trim();
  const idPart = mrn ? `MRN ${mrn}` : "no MRN";
  const dobPart = p.date_of_birth ? `DOB ${formatDob(p.date_of_birth)}` : "";
  return [`${p.first_name} ${p.last_name}`, idPart, dobPart, p.phone]
    .filter(Boolean)
    .join(" · ");
}
