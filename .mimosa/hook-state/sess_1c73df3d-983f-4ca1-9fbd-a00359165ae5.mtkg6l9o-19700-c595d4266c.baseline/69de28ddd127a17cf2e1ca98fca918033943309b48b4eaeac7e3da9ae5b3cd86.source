/**
 * Reports (SRS §4.20, FR-0220–FR-0223 — Phase 2-A).
 *
 * Four operational report cards stacked vertically:
 *   1. Daily OPD Summary    — appointments by status, encounters, new
 *                              patients, top 5 doctors. (single date)
 *   2. IPD Census            — bed snapshot + ward-by-ward breakdown +
 *                              discharges today. (single date)
 *   3. Revenue               — billed vs collected vs outstanding, bill
 *                              count by status, revenue by billing type,
 *                              top 5 bill items. (date range)
 *   4. Lab Turnaround        — order volume, status breakdown, avg
 *                              turnaround hours, top 5 tests. (date range)
 *
 * Two date pickers at the top drive the four cards:
 *   - "As-of date"  → OPD + IPD cards (defaults to today).
 *   - "Date range"  → Revenue + Lab cards (defaults to last 30 days).
 *
 * Each card has its own "Export CSV" button. The export goes through the
 * generic `export_report_csv` Tauri command (which returns a CSV string
 * with a leading UTF-8 BOM), then the frontend wraps it in a Blob and
 * triggers a download via a temporary anchor element.
 *
 * All four read commands + the CSV exporter are RBAC-guarded server-side
 * by `Permission::ReportsView`; the route itself is also wrapped in
 * `<RequirePermission perm={ReportsView}>` (see App.tsx) so a user
 * without the permission never reaches this page. Read-only: no audit
 * row is written (per audit.rs design — reads are not audited). Money is
 * rendered with `formatMoney` (PKR) per project convention.
 */
import { useState } from "react";
import { toast } from "sonner";
import {
  BarChart3, Calendar, BedDouble, DollarSign, FlaskConical,
  Download, Loader2,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from "@/components/ui/table";
import {
  PageContainer, PageHeader, SectionCard, StatCard, EmptyState,
} from "@/components/layout/shared";
import {
  useDailyOpdReport, useIpdCensusReport, useRevenueReport,
  useLabTurnaroundReport, useExportReportCsv,
} from "@/lib/queries";
import { formatMoney } from "@/lib/utils";

// ── CSV download helper (Tauri native desktop) ─────────────────────────────
//
// The backend `export_report_csv` command returns a CSV string with a
// leading UTF-8 BOM (`\uFEFF`) so Excel detects UTF-8. We use Tauri's
// native save dialog to let the user choose where to save the file,
// then write it via the Tauri filesystem plugin. This is the correct
// approach for a Tauri desktop app — browser Blob/anchor downloads don't
// work reliably in the Tauri webview.
async function downloadCsvString(filename: string, csv: string): Promise<void> {
  const { save } = await import("@tauri-apps/plugin-dialog");
  const { writeTextFile } = await import("@tauri-apps/plugin-fs");

  const filePath = await save({
    defaultPath: filename,
    filters: [{ name: "CSV Files", extensions: ["csv"] }],
  });

  if (!filePath) {
    // User cancelled the save dialog — not an error, just no action.
    return;
  }

  await writeTextFile(filePath, csv);
}

// ── Date inputs ────────────────────────────────────────────────────────────

function DatePicker({
  id, label, value, onChange,
}: {
  id: string;
  label: string;
  value: string;
  onChange: (v: string) => void;
}) {
  return (
    <div className="flex flex-col gap-1.5">
      <label
        htmlFor={id}
        className="text-[11px] font-semibold uppercase tracking-wide text-muted-foreground"
      >
        {label}
      </label>
      <Input
        id={id}
        type="date"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="h-10 w-[170px]"
      />
    </div>
  );
}

// ── Loading + error wrappers (shared across all four cards) ────────────────

function ReportLoading({ label }: { label: string }) {
  return (
    <div className="flex flex-col items-center justify-center py-12 gap-3">
      <Loader2 className="h-5 w-5 text-primary animate-spin" />
      <p className="text-sm text-muted-foreground">{label}</p>
    </div>
  );
}

function ReportError({ message }: { message: string }) {
  return (
    <EmptyState
      icon={BarChart3}
      title="Report unavailable"
      description={message}
    />
  );
}

// ── Export button ───────────────────────────────────────────────────────────
//
// The "Export CSV" button is disabled while the export mutation is pending
// (so the operator can't queue duplicate downloads) and shows a spinner.
// The mutation calls `export_report_csv` which dispatches server-side.
function ExportCsvButton({
  reportType, params, filename, disabled,
}: {
  reportType: "daily_opd" | "ipd_census" | "revenue" | "lab_turnaround";
  params: Record<string, unknown>;
  filename: string;
  disabled?: boolean;
}) {
  const exportMut = useExportReportCsv();
  const [isSaving, setIsSaving] = useState(false);

  const handleExport = async () => {
    exportMut.mutate(
      { reportType, params },
      {
        onSuccess: async (csv) => {
          setIsSaving(true);
          try {
            await downloadCsvString(filename, csv);
            toast.success(`Exported to ${filename}`);
          } catch (err) {
            toast.error(`Failed to save file: ${String(err)}`);
          } finally {
            setIsSaving(false);
          }
        },
      },
    );
  };
  return (
    <Button
      variant="outline"
      size="sm"
      onClick={handleExport}
      disabled={disabled || exportMut.isPending || isSaving}
      aria-label={`Export ${reportType.replace(/_/g, " ")} as CSV`}
    >
      {exportMut.isPending || isSaving ? (
        <Loader2 className="h-4 w-4 animate-spin" />
      ) : (
        <Download className="h-4 w-4" />
      )}
      <span className="ml-1.5">Export CSV</span>
    </Button>
  );
}

// ── Main page ──────────────────────────────────────────────────────────────
export function Reports() {
  // "As-of date" for the OPD + IPD cards. Defaults to today (local date,
  // sliced to YYYY-MM-DD). Empty string means "today" on the backend too.
  const today = new Date();
  const todayStr = today.toISOString().slice(0, 10);

  // "Date range" for the Revenue + Lab cards. Defaults to the last 30
  // days. Both endpoints are inclusive on the backend.
  const thirtyDaysAgo = new Date(today);
  thirtyDaysAgo.setDate(today.getDate() - 30);
  const defaultFrom = thirtyDaysAgo.toISOString().slice(0, 10);
  const defaultTo = todayStr;

  const [asOfDate, setAsOfDate] = useState(todayStr);
  const [fromDate, setFromDate] = useState(defaultFrom);
  const [toDate, setToDate] = useState(defaultTo);

  return (
    <PageContainer>
      <PageHeader
        icon={BarChart3}
        title="Operational Reports"
        description="Daily OPD, IPD census, revenue, and lab turnaround. Export any report to CSV for offline review or spreadsheet import."
      />

      {/* Date pickers bar — two pickers drive all four cards. */}
      <SectionCard bodyClassName="p-4 sm:p-5">
        <div className="flex flex-wrap items-end gap-6">
          <DatePicker
            id="report-as-of-date"
            label="As-of date (OPD + IPD)"
            value={asOfDate}
            onChange={setAsOfDate}
          />
          <div className="h-8 w-px bg-border hidden sm:block" />
          <DatePicker
            id="report-from-date"
            label="From (Revenue + Lab)"
            value={fromDate}
            onChange={setFromDate}
          />
          <DatePicker
            id="report-to-date"
            label="To (Revenue + Lab)"
            value={toDate}
            onChange={setToDate}
          />
        </div>
      </SectionCard>

      {/* 1. Daily OPD Summary */}
      <DailyOpdCard asOfDate={asOfDate} />

      {/* 2. IPD Census */}
      <IpdCensusCard asOfDate={asOfDate} />

      {/* 3. Revenue */}
      <RevenueCard fromDate={fromDate} toDate={toDate} />

      {/* 4. Lab Turnaround */}
      <LabTurnaroundCard fromDate={fromDate} toDate={toDate} />
    </PageContainer>
  );
}

// ── 1. Daily OPD Summary card ──────────────────────────────────────────────
function DailyOpdCard({ asOfDate }: { asOfDate: string }) {
  // Pass empty string as null so the backend defaults to today when the
  // user clears the input.
  const effectiveDate = asOfDate.length > 0 ? asOfDate : null;
  const { data, isLoading, isError, error } = useDailyOpdReport(effectiveDate);

  return (
    <SectionCard
      icon={Calendar}
      title="Daily OPD Summary"
      description={`Outpatient activity for ${data?.date ?? asOfDate ?? "today"}: appointments by status, encounters, new patients, and top doctors.`}
      action={
        <ExportCsvButton
          reportType="daily_opd"
          params={effectiveDate ? { date: effectiveDate } : {}}
          filename={`daily-opd_${data?.date ?? "today"}.csv`}
          disabled={!data}
        />
      }
      bodyClassName="p-6 space-y-6"
    >
      {isLoading ? (
        <ReportLoading label="Aggregating daily OPD activity…" />
      ) : isError ? (
        <ReportError message={String(error)} />
      ) : !data ? null : (
        <>
          <div className="grid grid-cols-2 lg:grid-cols-4 gap-4 items-stretch">
            <StatCard
              icon={Calendar}
              label="Total appointments"
              value={data.total_appointments}
              sub={`On ${data.date}`}
              color="primary"
            />
            <StatCard
              icon={Calendar}
              label="Encounters / visits"
              value={data.total_encounters}
              sub="Same day"
              color="info"
            />
            <StatCard
              icon={Calendar}
              label="New patients"
              value={data.new_patients}
              sub="Registered that day"
              color="success"
            />
            <StatCard
              icon={Calendar}
              label="Appointment statuses"
              value={data.appointments_by_status.length}
              sub="Distinct buckets"
              color="accent"
            />
          </div>

          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            <div>
              <h4 className="text-display-sm text-foreground mb-2">
                Appointments by status
              </h4>
              {data.appointments_by_status.length === 0 ? (
                <p className="text-xs text-muted-foreground">
                  No appointments for this day.
                </p>
              ) : (
                <Table>
                  <TableHeader>
                    <TableRow className="border-border hover:bg-transparent">
                      <TableHead>Status</TableHead>
                      <TableHead className="text-right">Count</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {data.appointments_by_status.map((s) => (
                      <TableRow key={s.status}>
                        <TableCell className="font-medium capitalize">
                          {s.status.replace(/-/g, " ")}
                        </TableCell>
                        <TableCell className="text-right tabular-nums">
                          {s.count}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              )}
            </div>

            <div>
              <h4 className="text-display-sm text-foreground mb-2">
                Top 5 doctors by appointments
              </h4>
              {data.top_doctors.length === 0 ? (
                <p className="text-xs text-muted-foreground">
                  No appointments for this day.
                </p>
              ) : (
                <Table>
                  <TableHeader>
                    <TableRow className="border-border hover:bg-transparent">
                      <TableHead>#</TableHead>
                      <TableHead>Doctor</TableHead>
                      <TableHead className="text-right">Appointments</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {data.top_doctors.map((d, i) => (
                      <TableRow key={`${d.doctor_name}-${i}`}>
                        <TableCell className="tabular-nums text-muted-foreground">
                          {i + 1}
                        </TableCell>
                        <TableCell className="font-medium">
                          {d.doctor_name}
                        </TableCell>
                        <TableCell className="text-right tabular-nums">
                          {d.appointment_count}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              )}
            </div>
          </div>
        </>
      )}
    </SectionCard>
  );
}

// ── 2. IPD Census card ─────────────────────────────────────────────────────
function IpdCensusCard({ asOfDate }: { asOfDate: string }) {
  const effectiveDate = asOfDate.length > 0 ? asOfDate : null;
  const { data, isLoading, isError, error } = useIpdCensusReport(effectiveDate);

  return (
    <SectionCard
      icon={BedDouble}
      title="IPD Census"
      description={`In-patient bed snapshot${data ? ` for ${data.date}` : ""}: total/available/occupied/maintenance beds, current admissions, discharges today, and per-ward breakdown.`}
      action={
        <ExportCsvButton
          reportType="ipd_census"
          params={effectiveDate ? { date: effectiveDate } : {}}
          filename={`ipd-census_${data?.date ?? "today"}.csv`}
          disabled={!data}
        />
      }
      bodyClassName="p-6 space-y-6"
    >
      {isLoading ? (
        <ReportLoading label="Aggregating IPD census…" />
      ) : isError ? (
        <ReportError message={String(error)} />
      ) : !data ? null : (
        <>
          <div className="grid grid-cols-2 lg:grid-cols-4 gap-4 items-stretch">
            <StatCard
              icon={BedDouble}
              label="Total beds"
              value={data.total_beds}
              sub="All statuses"
              color="primary"
            />
            <StatCard
              icon={BedDouble}
              label="Available"
              value={data.available_beds}
              sub="Ready for admission"
              color="success"
            />
            <StatCard
              icon={BedDouble}
              label="Occupied"
              value={data.occupied_beds}
              sub="Currently in use"
              color="warning"
            />
            <StatCard
              icon={BedDouble}
              label="Maintenance"
              value={data.maintenance_beds}
              sub="Out of service"
              color="destructive"
            />
          </div>

          <div className="grid grid-cols-2 lg:grid-cols-3 gap-4 items-stretch">
            <StatCard
              icon={BedDouble}
              label="Current admissions"
              value={data.current_admissions}
              sub="Status = admitted"
              color="info"
            />
            <StatCard
              icon={BedDouble}
              label="Discharges today"
              value={data.discharges_today}
              sub={`On ${data.date}`}
              color="accent"
            />
            {data.total_beds > 0 && (
              <StatCard
                icon={BedDouble}
                label="Occupancy rate"
                value={`${((data.occupied_beds / data.total_beds) * 100).toFixed(1)}%`}
                sub="Occupied / total"
                color={
                  data.occupied_beds / data.total_beds >= 0.9
                    ? "destructive"
                    : "primary"
                }
              />
            )}
          </div>

          <div>
            <h4 className="text-display-sm text-foreground mb-2">
              Ward-by-ward breakdown
            </h4>
            {data.by_ward.length === 0 ? (
              <EmptyState
                icon={BedDouble}
                title="No wards configured"
                description="Create wards and beds in the IPD module to see census data."
              />
            ) : (
              <Table>
                <TableHeader>
                  <TableRow className="border-border hover:bg-transparent">
                    <TableHead>Ward</TableHead>
                    <TableHead className="text-right">Total beds</TableHead>
                    <TableHead className="text-right">Occupied</TableHead>
                    <TableHead className="text-right">Available</TableHead>
                    <TableHead className="text-right">Utilization</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {data.by_ward.map((w) => {
                    const util =
                      w.total_beds > 0
                        ? (w.occupied_beds / w.total_beds) * 100
                        : 0;
                    return (
                      <TableRow key={w.ward_id}>
                        <TableCell className="font-medium">
                          {w.ward_name}
                        </TableCell>
                        <TableCell className="text-right tabular-nums">
                          {w.total_beds}
                        </TableCell>
                        <TableCell className="text-right tabular-nums">
                          {w.occupied_beds}
                        </TableCell>
                        <TableCell className="text-right tabular-nums">
                          {w.available_beds}
                        </TableCell>
                        <TableCell className="text-right tabular-nums">
                          {util.toFixed(1)}%
                        </TableCell>
                      </TableRow>
                    );
                  })}
                </TableBody>
              </Table>
            )}
          </div>
        </>
      )}
    </SectionCard>
  );
}

// ── 3. Revenue card ────────────────────────────────────────────────────────
function RevenueCard({
  fromDate, toDate,
}: {
  fromDate: string;
  toDate: string;
}) {
  const { data, isLoading, isError, error } = useRevenueReport(fromDate, toDate);

  return (
    <SectionCard
      icon={DollarSign}
      title="Revenue"
      description={`Bills and payments from ${fromDate} to ${toDate}: billed vs collected vs outstanding, bill count by status, revenue by billing type, and top bill items.`}
      action={
        <ExportCsvButton
          reportType="revenue"
          params={{ from_date: fromDate, to_date: toDate }}
          filename={`revenue_${fromDate}_to_${toDate}.csv`}
          disabled={!data}
        />
      }
      bodyClassName="p-6 space-y-6"
    >
      {isLoading ? (
        <ReportLoading label="Aggregating bills and payments…" />
      ) : isError ? (
        <ReportError message={String(error)} />
      ) : !data ? null : (
        <>
          <div className="grid grid-cols-1 lg:grid-cols-3 gap-4 items-stretch">
            <StatCard
              icon={DollarSign}
              label="Total billed"
              value={formatMoney(data.total_billed)}
              sub={`${fromDate} to ${toDate}`}
              color="primary"
            />
            <StatCard
              icon={DollarSign}
              label="Total collected"
              value={formatMoney(data.total_collected)}
              sub="Sum of payments"
              color="success"
            />
            <StatCard
              icon={DollarSign}
              label="Outstanding"
              value={formatMoney(data.total_outstanding)}
              sub="Billed − collected"
              color="destructive"
            />
          </div>

          <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
            <div>
              <h4 className="text-display-sm text-foreground mb-2">
                Bill count by status
              </h4>
              {data.bill_count_by_status.length === 0 ? (
                <p className="text-xs text-muted-foreground">
                  No bills in this range.
                </p>
              ) : (
                <Table>
                  <TableHeader>
                    <TableRow className="border-border hover:bg-transparent">
                      <TableHead>Status</TableHead>
                      <TableHead className="text-right">Bills</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {data.bill_count_by_status.map((s) => (
                      <TableRow key={s.status}>
                        <TableCell className="font-medium capitalize">
                          {s.status}
                        </TableCell>
                        <TableCell className="text-right tabular-nums">
                          {s.count}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              )}
            </div>

            <div>
              <h4 className="text-display-sm text-foreground mb-2">
                Revenue by billing type
              </h4>
              {data.revenue_by_type.length === 0 ? (
                <p className="text-xs text-muted-foreground">
                  No bills in this range.
                </p>
              ) : (
                <Table>
                  <TableHeader>
                    <TableRow className="border-border hover:bg-transparent">
                      <TableHead>Type</TableHead>
                      <TableHead className="text-right">Revenue</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {data.revenue_by_type.map((t) => (
                      <TableRow key={t.bill_type}>
                        <TableCell className="font-medium uppercase">
                          {t.bill_type}
                        </TableCell>
                        <TableCell className="text-right tabular-nums">
                          {formatMoney(t.total)}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              )}
            </div>

            <div>
              <h4 className="text-display-sm text-foreground mb-2">
                Top 5 bill items by revenue
              </h4>
              {data.top_bill_items.length === 0 ? (
                <p className="text-xs text-muted-foreground">
                  No bill items in this range.
                </p>
              ) : (
                <Table>
                  <TableHeader>
                    <TableRow className="border-border hover:bg-transparent">
                      <TableHead>#</TableHead>
                      <TableHead>Description</TableHead>
                      <TableHead className="text-right">Revenue</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {data.top_bill_items.map((it, i) => (
                      <TableRow key={`${it.description}-${i}`}>
                        <TableCell className="tabular-nums text-muted-foreground">
                          {i + 1}
                        </TableCell>
                        <TableCell className="font-medium">
                          {it.description}
                        </TableCell>
                        <TableCell className="text-right tabular-nums">
                          {formatMoney(it.revenue)}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              )}
            </div>
          </div>
        </>
      )}
    </SectionCard>
  );
}

// ── 4. Lab Turnaround card ─────────────────────────────────────────────────
function LabTurnaroundCard({
  fromDate, toDate,
}: {
  fromDate: string;
  toDate: string;
}) {
  const { data, isLoading, isError, error } = useLabTurnaroundReport(fromDate, toDate);

  return (
    <SectionCard
      icon={FlaskConical}
      title="Lab Turnaround"
      description={`Lab orders from ${fromDate} to ${toDate}: total orders, status breakdown, average turnaround (ordered → last result completed), and top tests.`}
      action={
        <ExportCsvButton
          reportType="lab_turnaround"
          params={{ from_date: fromDate, to_date: toDate }}
          filename={`lab-turnaround_${fromDate}_to_${toDate}.csv`}
          disabled={!data}
        />
      }
      bodyClassName="p-6 space-y-6"
    >
      {isLoading ? (
        <ReportLoading label="Aggregating lab turnaround…" />
      ) : isError ? (
        <ReportError message={String(error)} />
      ) : !data ? null : (
        <>
          <div className="grid grid-cols-1 lg:grid-cols-3 gap-4 items-stretch">
            <StatCard
              icon={FlaskConical}
              label="Total lab orders"
              value={data.total_orders}
              sub={`${fromDate} to ${toDate}`}
              color="primary"
            />
            <StatCard
              icon={FlaskConical}
              label="Avg turnaround"
              value={`${data.average_turnaround_hours.toFixed(1)} h`}
              sub="Ordered → last result"
              color="info"
            />
            <StatCard
              icon={FlaskConical}
              label="Order statuses"
              value={data.orders_by_status.length}
              sub="Distinct buckets"
              color="accent"
            />
          </div>

          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            <div>
              <h4 className="text-display-sm text-foreground mb-2">
                Orders by status
              </h4>
              {data.orders_by_status.length === 0 ? (
                <p className="text-xs text-muted-foreground">
                  No lab orders in this range.
                </p>
              ) : (
                <Table>
                  <TableHeader>
                    <TableRow className="border-border hover:bg-transparent">
                      <TableHead>Status</TableHead>
                      <TableHead className="text-right">Orders</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {data.orders_by_status.map((s) => (
                      <TableRow key={s.status}>
                        <TableCell className="font-medium capitalize">
                          {s.status}
                        </TableCell>
                        <TableCell className="text-right tabular-nums">
                          {s.count}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              )}
            </div>

            <div>
              <h4 className="text-display-sm text-foreground mb-2">
                Top 5 most ordered tests
              </h4>
              {data.top_tests.length === 0 ? (
                <p className="text-xs text-muted-foreground">
                  No lab tests ordered in this range.
                </p>
              ) : (
                <Table>
                  <TableHeader>
                    <TableRow className="border-border hover:bg-transparent">
                      <TableHead>#</TableHead>
                      <TableHead>Test</TableHead>
                      <TableHead className="text-right">Orders</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {data.top_tests.map((t, i) => (
                      <TableRow key={`${t.test_name}-${i}`}>
                        <TableCell className="tabular-nums text-muted-foreground">
                          {i + 1}
                        </TableCell>
                        <TableCell className="font-medium">
                          {t.test_name}
                        </TableCell>
                        <TableCell className="text-right tabular-nums">
                          {t.order_count}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              )}
            </div>
          </div>
        </>
      )}
    </SectionCard>
  );
}
