import type { ReactNode } from "react";
import { motion } from "motion/react";
import { type LucideIcon } from "lucide-react";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";

/* ============================================================
   VitalFlow HMS — Shared Design System Components
   The "homogeneity enforcement layer". Every page composes
   these primitives so spacing, typography, cards, tables,
   forms, and empty/loading states stay perfectly consistent
   across the entire application.
   ============================================================ */

// ── PageContainer ──────────────────────────────────────────────
/** Standard page wrapper. Every page's root element. Provides the
 *  canonical 40px vertical rhythm between top-level sections. */
export function PageContainer({
  children,
  className = "",
}: {
  children: ReactNode;
  className?: string;
}) {
  return <div className={`section-stack ${className}`}>{children}</div>;
}

// ── PageHeader ─────────────────────────────────────────────────
/** Standard page header: icon + title + description (left),
 *  actions (right). The icon sits in a soft primary-tinted tile. */
export function PageHeader({
  icon: Icon,
  title,
  description,
  actions,
}: {
  icon?: LucideIcon;
  title: string;
  description?: string;
  actions?: ReactNode;
}) {
  return (
    <div className="flex items-start justify-between gap-6 flex-wrap">
      <div className="flex items-start gap-4 min-w-0">
        {Icon && (
          <div className="h-12 w-12 rounded-[var(--radius-md)] bg-primary/10 flex items-center justify-center shrink-0 mt-0.5 ring-1 ring-inset ring-primary/10">
            <Icon className="h-5 w-5 text-primary" />
          </div>
        )}
        <div className="min-w-0">
          <h1 className="text-display-xl text-foreground truncate">{title}</h1>
          {description && (
            <p className="text-sm text-muted-foreground mt-1.5 leading-relaxed">
              {description}
            </p>
          )}
        </div>
      </div>
      {actions && (
        <div className="flex items-center gap-3 flex-wrap">{actions}</div>
      )}
    </div>
  );
}

// ── SectionHeader ──────────────────────────────────────────────
/** Lightweight section title (no card). For sub-grouping inside a
 *  page or card without a full SectionCard. */
export function SectionHeader({
  icon: Icon,
  title,
  description,
  action,
}: {
  icon?: LucideIcon;
  title: string;
  description?: string;
  action?: ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-4 flex-wrap">
      <div className="flex items-center gap-2.5 min-w-0">
        {Icon && <Icon className="h-4 w-4 text-primary shrink-0" />}
        <div className="min-w-0">
          <h3 className="text-display-md text-foreground truncate">{title}</h3>
          {description && (
            <p className="text-xs text-muted-foreground mt-0.5">{description}</p>
          )}
        </div>
      </div>
      {action}
    </div>
  );
}

// ── SectionCard ────────────────────────────────────────────────
/** Standard card with optional header (icon + title + action).
 *  The header strip uses a hairline bottom border and consistent
 *  padding; the body is left to the caller via `bodyClassName`. */
export function SectionCard({
  icon: Icon,
  title,
  description,
  action,
  children,
  className = "",
  bodyClassName = "",
  headerClassName = "",
}: {
  icon?: LucideIcon;
  title?: string;
  description?: string;
  action?: ReactNode;
  children: ReactNode;
  className?: string;
  bodyClassName?: string;
  headerClassName?: string;
}) {
  return (
    <Card
      className={`rounded-[var(--radius-md)] border-border shadow-sm ${className}`}
    >
      {(title || action) && (
        <div
          className={`flex items-center justify-between gap-4 px-6 py-4 border-b border-border ${headerClassName}`}
        >
          {title && (
            <div className="flex items-center gap-2.5 min-w-0">
              {Icon && <Icon className="h-4 w-4 text-primary shrink-0" />}
              <div className="min-w-0">
                <h3 className="text-display-md text-foreground truncate">
                  {title}
                </h3>
                {description && (
                  <p className="text-xs text-muted-foreground mt-0.5">
                    {description}
                  </p>
                )}
              </div>
            </div>
          )}
          {action}
        </div>
      )}
      <div className={bodyClassName}>{children}</div>
    </Card>
  );
}

// ── StatCard / KpiCard ─────────────────────────────────────────
const STAT_COLORS: Record<string, { bg: string; icon: string }> = {
  primary: { bg: "hsl(var(--primary) / 0.10)", icon: "hsl(var(--primary))" },
  info: { bg: "hsl(var(--info) / 0.10)", icon: "hsl(var(--info))" },
  success: { bg: "hsl(var(--success) / 0.10)", icon: "hsl(var(--success))" },
  warning: { bg: "hsl(var(--warning) / 0.10)", icon: "hsl(var(--warning))" },
  accent: { bg: "hsl(var(--accent) / 0.10)", icon: "hsl(var(--accent))" },
  destructive: {
    bg: "hsl(var(--destructive) / 0.10)",
    icon: "hsl(var(--destructive))",
  },
  // legacy alias kept for backwards compatibility
  sky: { bg: "hsl(var(--info) / 0.10)", icon: "hsl(var(--info))" },
};

// Concrete default so `noUncheckedIndexedAccess` doesn't make every lookup
// return `T | undefined`. This is the single source of truth for the fallback.
const DEFAULT_STAT_COLOR: { bg: string; icon: string } = STAT_COLORS.primary!;

/** Standard KPI/stat card. Consistent across Dashboard and all module
 *  pages. Every instance renders at identical height: the label/icon
 *  row, the value, and the supporting-text slot are always present
 *  (the slot reserves its line-height even when `sub` is omitted) so a
 *  row of these never has one card taller than its neighbors. */
export function StatCard({
  icon: Icon,
  label,
  value,
  sub,
  trend,
  trendDirection,
  color = "primary",
  onClick,
}: {
  icon: LucideIcon;
  label: string;
  value: ReactNode;
  sub?: string;
  trend?: string;
  trendDirection?: "up" | "down" | "neutral";
  color?: keyof typeof STAT_COLORS;
  onClick?: () => void;
}) {
  const c = STAT_COLORS[color] ?? DEFAULT_STAT_COLOR;
  const trendColor =
    trendDirection === "down"
      ? "text-destructive"
      : trendDirection === "neutral"
        ? "text-muted-foreground"
        : "text-success";
  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.3, ease: [0.22, 1, 0.36, 1] }}
      className="h-full"
    >
      <Card
        className={`h-full flex flex-col rounded-[var(--radius-md)] border-border shadow-sm transition-all duration-200 ${onClick ? "cursor-pointer hover:shadow-md hover:border-primary/25 hover:-translate-y-0.5" : ""}`}
        onClick={onClick}
      >
        <CardContent className="p-6 flex flex-col flex-1">
          <div className="flex items-start justify-between mb-4">
            <span className="text-[11px] font-semibold text-muted-foreground uppercase tracking-wide">
              {label}
            </span>
            <div
              className="h-9 w-9 rounded-[var(--radius-sm)] flex items-center justify-center shrink-0"
              style={{ background: c.bg }}
            >
              <Icon className="h-4 w-4" style={{ color: c.icon }} />
            </div>
          </div>
          <div className="text-display-xl text-foreground tabular-nums leading-none">
            {value}
          </div>
          <div className="flex items-center justify-between mt-3 min-h-[1.1rem]">
            <span className="text-[11px] text-muted-foreground">
              {sub ?? "\u00A0"}
            </span>
            {trend && (
              <span className={`text-[11px] font-semibold ${trendColor}`}>
                {trend}
              </span>
            )}
          </div>
        </CardContent>
      </Card>
    </motion.div>
  );
}

/** KpiCard — alias of StatCard for semantic clarity in dashboards. */
export const KpiCard = StatCard;

// ── AnalyticsCard ──────────────────────────────────────────────
/** Card wrapper for charts / analytics panels. Title + optional
 *  description + action in the header, flexible body below. */
export function AnalyticsCard({
  icon: Icon,
  title,
  description,
  action,
  children,
  className = "",
  bodyClassName = "",
}: {
  icon?: LucideIcon;
  title: string;
  description?: string;
  action?: ReactNode;
  children: ReactNode;
  className?: string;
  bodyClassName?: string;
}) {
  return (
    <Card
      className={`rounded-[var(--radius-md)] border-border shadow-sm ${className}`}
    >
      <div className="flex items-center justify-between gap-4 px-6 py-4 border-b border-border">
        <div className="flex items-center gap-2.5 min-w-0">
          {Icon && <Icon className="h-4 w-4 text-primary shrink-0" />}
          <div className="min-w-0">
            <h3 className="text-display-md text-foreground truncate">
              {title}
            </h3>
            {description && (
              <p className="text-xs text-muted-foreground mt-0.5">
                {description}
              </p>
            )}
          </div>
        </div>
        {action}
      </div>
      <div className={bodyClassName || "p-6"}>{children}</div>
    </Card>
  );
}

// ── InfoCard ───────────────────────────────────────────────────
/** Compact informational tile: icon + title + description.
 *  Used in legends, helper callouts, and quick summaries. */
export function InfoCard({
  icon: Icon,
  title,
  description,
  color = "primary",
  className = "",
}: {
  icon: LucideIcon;
  title: string;
  description?: string;
  color?: keyof typeof STAT_COLORS;
  className?: string;
}) {
  const c = STAT_COLORS[color] ?? DEFAULT_STAT_COLOR;
  return (
    <div
      className={`flex items-start gap-3 rounded-[var(--radius-md)] border border-border bg-card p-4 ${className}`}
    >
      <div
        className="h-9 w-9 rounded-[var(--radius-sm)] flex items-center justify-center shrink-0"
        style={{ background: c.bg }}
      >
        <Icon className="h-4 w-4" style={{ color: c.icon }} />
      </div>
      <div className="min-w-0">
        <p className="text-sm font-semibold text-foreground">{title}</p>
        {description && (
          <p className="text-xs text-muted-foreground mt-0.5 leading-relaxed">
            {description}
          </p>
        )}
      </div>
    </div>
  );
}

// ── QuickActionTile ────────────────────────────────────────────
/** Dashboard quick-access tile. Icon + label, hover lift. */
export function QuickActionTile({
  icon: Icon,
  label,
  description,
  onClick,
  color = "primary",
}: {
  icon: LucideIcon;
  label: string;
  description?: string;
  onClick?: () => void;
  color?: keyof typeof STAT_COLORS;
}) {
  const c = STAT_COLORS[color] ?? DEFAULT_STAT_COLOR;
  return (
    <motion.button
      type="button"
      onClick={onClick}
      whileHover={{ y: -2 }}
      whileTap={{ scale: 0.98 }}
      className="flex flex-col items-start gap-3 rounded-[var(--radius-md)] border border-border bg-card p-5 text-left shadow-sm transition-colors hover:border-primary/30 hover:shadow-md cursor-pointer"
    >
      <div
        className="h-10 w-10 rounded-[var(--radius-sm)] flex items-center justify-center"
        style={{ background: c.bg }}
      >
        <Icon className="h-5 w-5" style={{ color: c.icon }} />
      </div>
      <div>
        <p className="text-sm font-semibold text-foreground">{label}</p>
        {description && (
          <p className="text-xs text-muted-foreground mt-0.5">{description}</p>
        )}
      </div>
    </motion.button>
  );
}

// ── MetricBadge ────────────────────────────────────────────────
/** Small inline metric pill — e.g. "12 active", "3 urgent". */
export function MetricBadge({
  value,
  label,
  color = "primary",
}: {
  value: ReactNode;
  label?: string;
  color?: keyof typeof STAT_COLORS;
}) {
  const c = STAT_COLORS[color] ?? DEFAULT_STAT_COLOR;
  return (
    <span className="inline-flex items-center gap-1.5 rounded-full border border-border bg-card px-2.5 py-1 text-xs font-semibold">
      <span
        className="h-1.5 w-1.5 rounded-full"
        style={{ background: c.icon }}
      />
      <span className="text-foreground tabular-nums">{value}</span>
      {label && <span className="text-muted-foreground font-medium">{label}</span>}
    </span>
  );
}

// ── EmptyState ─────────────────────────────────────────────────
/** Standard empty state. Consistent across all list/table pages. */
export function EmptyState({
  icon: Icon,
  title,
  description,
  action,
}: {
  icon: LucideIcon;
  title: string;
  description?: string;
  action?: ReactNode;
}) {
  return (
    <div className="flex flex-col items-center justify-center py-16 px-8 text-center">
      <div className="h-16 w-16 rounded-[var(--radius-lg)] bg-muted flex items-center justify-center mb-5">
        <Icon className="h-7 w-7 text-muted-foreground" />
      </div>
      <h3 className="text-display-md text-foreground mb-1.5">{title}</h3>
      {description && (
        <p className="text-sm text-muted-foreground max-w-sm mb-5 leading-relaxed">
          {description}
        </p>
      )}
      {action}
    </div>
  );
}

// ── LoadingState ───────────────────────────────────────────────
/** Standard loading skeleton. `rows` controls the number of bars;
 *  `variant` switches between table-row and card-grid skeletons. */
export function LoadingState({
  rows = 5,
  variant = "rows",
}: {
  rows?: number;
  variant?: "rows" | "cards";
}) {
  if (variant === "cards") {
    return (
      <div className="p-6 grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4">
        {Array.from({ length: rows }).map((_, i) => (
          <div key={i} className="rounded-[var(--radius-md)] border border-border p-6 space-y-3">
            <div className="skeleton h-4 w-24" />
            <div className="skeleton h-8 w-16" />
            <div className="skeleton h-3 w-32" />
          </div>
        ))}
      </div>
    );
  }
  return (
    <div className="p-6 space-y-3">
      {Array.from({ length: rows }).map((_, i) => (
        <div key={i} className="skeleton h-11 w-full" />
      ))}
    </div>
  );
}

// ── StatusBadge ────────────────────────────────────────────────
const STATUS_COLORS: Record<string, string> = {
  // Appointments
  scheduled: "var(--status-scheduled)",
  confirmed: "var(--status-confirmed)",
  completed: "var(--status-completed)",
  cancelled: "var(--status-cancelled)",
  "no-show": "var(--status-no-show)",
  // Queue
  waiting: "var(--status-scheduled)",
  "in-progress": "var(--status-confirmed)",
  skipped: "var(--status-cancelled)",
  // Beds
  available: "var(--status-confirmed)",
  occupied: "var(--status-cancelled)",
  maintenance: "var(--status-no-show)",
  cleaning: "var(--status-scheduled)",
  // Lab
  ordered: "var(--status-scheduled)",
  collected: "var(--status-no-show)",
  // Billing
  draft: "var(--muted-foreground)",
  unpaid: "var(--status-cancelled)",
  partial: "var(--status-no-show)",
  paid: "var(--status-confirmed)",
  // IPD
  admitted: "var(--status-scheduled)",
  discharged: "var(--status-completed)",
  // Users
  active: "var(--status-confirmed)",
  inactive: "var(--muted-foreground)",
};

/** Standard status badge. Consistent across ALL pages. */
export function StatusBadge({ status }: { status: string }) {
  const token =
    STATUS_COLORS[status.toLowerCase()] ?? "var(--muted-foreground)";
  return (
    <span
      className="status-badge capitalize"
      style={{ background: `hsl(${token} / 0.10)`, color: `hsl(${token})` }}
    >
      <span
        className="h-1.5 w-1.5 rounded-full"
        style={{ background: `hsl(${token})` }}
      />
      {status.replace(/-/g, " ")}
    </span>
  );
}

// ── PageToolbar / TableToolbar ─────────────────────────────────
/** Standard toolbar for filter/search row above tables. */
export function PageToolbar({
  children,
  className = "",
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={`flex items-center gap-4 flex-wrap px-6 py-3.5 border-b border-border bg-muted/30 ${className}`}
    >
      {children}
    </div>
  );
}

/** Alias of PageToolbar for semantic clarity above tables. */
export const TableToolbar = PageToolbar;

// ── ActionBar ──────────────────────────────────────────────────
/** Sticky action row for dialog/form footers. Right-aligned
 *  actions with a hairline top border. */
export function ActionBar({
  children,
  className = "",
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={`flex items-center justify-end gap-2 pt-4 mt-2 border-t border-border ${className}`}
    >
      {children}
    </div>
  );
}

// ── FormSection ────────────────────────────────────────────────
/** Titled group of form fields. Optional icon + description. */
export function FormSection({
  icon: Icon,
  title,
  description,
  children,
  className = "",
}: {
  icon?: LucideIcon;
  title?: string;
  description?: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <div className={className}>
      {(title || description) && (
        <div className="flex items-center gap-2.5 mb-4">
          {Icon && <Icon className="h-4 w-4 text-primary shrink-0" />}
          <div>
            {title && (
              <h4 className="text-display-sm text-foreground">{title}</h4>
            )}
            {description && (
              <p className="text-xs text-muted-foreground mt-0.5">
                {description}
              </p>
            )}
          </div>
        </div>
      )}
      <div className="form-stack">{children}</div>
    </div>
  );
}

// ── FormField ──────────────────────────────────────────────────
/** Label + control + optional hint/error. The canonical form
 *  field wrapper — equal label spacing, consistent rhythm. */
export function FormField({
  label,
  htmlFor,
  required,
  hint,
  error,
  children,
  className = "",
}: {
  label: string;
  htmlFor?: string;
  required?: boolean;
  hint?: string;
  error?: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <div className={`flex flex-col gap-1.5 ${className}`}>
      <label
        htmlFor={htmlFor}
        className="text-xs font-semibold uppercase tracking-wide text-foreground"
      >
        {label}
        {required && <span className="text-destructive ml-0.5">*</span>}
      </label>
      {children}
      {hint && !error && (
        <p className="text-xs text-muted-foreground leading-relaxed">{hint}</p>
      )}
      {error && (
        <p className="text-xs text-destructive leading-relaxed">{error}</p>
      )}
    </div>
  );
}

// ── Pagination ─────────────────────────────────────────────────────────────
/** Reusable pagination control for list/table pages.
 *  Shows "Showing X–Y of Z" + rows-per-page selector + Previous/Next buttons. */
export function Pagination({
  totalItems,
  page,
  rowsPerPage,
  onPageChange,
  onRowsPerPageChange,
}: {
  totalItems: number;
  page: number;
  rowsPerPage: number;
  onPageChange: (page: number) => void;
  onRowsPerPageChange: (rows: number) => void;
}) {
  const totalPages = Math.max(1, Math.ceil(totalItems / rowsPerPage));
  const start = totalItems === 0 ? 0 : (page - 1) * rowsPerPage + 1;
  const end = Math.min(page * rowsPerPage, totalItems);

  return (
    <div
      className="flex items-center justify-between gap-4 flex-wrap px-6 py-3 border-t border-border"
      role="navigation"
      aria-label="Pagination"
    >
      <div className="flex items-center gap-3">
        <span className="text-xs text-muted-foreground">
          Showing {start}–{end} of {totalItems}
        </span>
        <div className="flex items-center gap-2">
          <label className="text-xs text-muted-foreground" htmlFor="rows-per-page">
            Rows:
          </label>
          <select
            id="rows-per-page"
            value={rowsPerPage}
            onChange={(e) => onRowsPerPageChange(Number(e.target.value))}
            className="text-xs border border-border rounded-[var(--radius-sm)] px-2 py-1 bg-card"
          >
            <option value={10}>10</option>
            <option value={25}>25</option>
            <option value={50}>50</option>
          </select>
        </div>
      </div>
      <div className="flex items-center gap-2">
        <Button
          variant="outline"
          size="sm"
          disabled={page <= 1}
          onClick={() => onPageChange(page - 1)}
        >
          Previous
        </Button>
        <span className="text-xs text-muted-foreground">
          Page {page} of {totalPages}
        </span>
        <Button
          variant="outline"
          size="sm"
          disabled={page >= totalPages}
          onClick={() => onPageChange(page + 1)}
        >
          Next
        </Button>
      </div>
    </div>
  );
}
