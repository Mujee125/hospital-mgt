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
