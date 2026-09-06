/**
 * Reports (SRS §4.20, FR-0220–FR-0223 — Phase 2-A + Phase 6.4).
 *
 * Fourteen operational report cards in five tabs:
 *   Core:      Daily OPD, IPD Census, Revenue, Lab Turnaround (Phase 2-A)
 *   Clinical:  Doctor Performance, Diagnosis Frequency (Phase 6.4)
 *   Financial: Daily Collection, Receivables Aging, Insurance Claims (6.4)
 *   Inventory: Pharmacy Consumption, Stock Status, Drug Expiry (6.4)
 *   Admin:     User Activity, Backup Status (6.4)
 *
 * Two date pickers at the top drive the dated cards:
 *   - "As-of date"  → OPD + IPD cards (defaults to today).
 *   - "Date range"  → all range reports (defaults to last 30 days).
 *
 * Each card has its own "Export CSV" button. The export goes through the
 * generic `export_report_csv` Tauri command (which returns a CSV string
 * with a leading UTF-8 BOM), then the frontend wraps it in a Blob and
 * triggers a download via a temporary anchor element.
 *
 * All read commands + the CSV exporter are RBAC-guarded server-side
 * by `Permission::ReportsView` (user activity additionally requires
 * AuditView server-side); the route itself is also wrapped in
 * `<RequirePermission perm={ReportsView}>` (see App.tsx) so a user
 * without the permission never reaches this page. Read-only: no audit
 * row is written (per audit.rs design — reads are not audited). Money is
 * rendered with `formatMoney` (PKR) per project convention.
 */
import { useState } from "react";
import { toast } from "sonner";
import {
  BarChart3, Calendar, BedDouble, DollarSign, FlaskConical,
  Download, Loader2, Stethoscope, ClipboardList, Pill, Package,
  CalendarClock, Hourglass, Landmark, Activity, DatabaseBackup, AlertTriangle,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from "@/components/ui/table";
import {
  PageContainer, PageHeader, SectionCard, StatCard, EmptyState,
} from "@/components/layout/shared";
import {
  useDailyOpdReport, useIpdCensusReport, useRevenueReport,
  useLabTurnaroundReport, useExportReportCsv,
  useDoctorPerformanceReport, useDiagnosisFrequencyReport,
  usePharmacyConsumptionReport, useDrugExpiryReport,
  useDailyCollectionReport, useReceivablesAgingReport,
  useInsuranceClaimsReport, useStockStatusReport,
  useUserActivityReport, useBackupStatusReport,
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
  reportType:
    | "daily_opd" | "ipd_census" | "revenue" | "lab_turnaround"
    | "doctor_performance" | "diagnosis_frequency" | "pharmacy_consumption"
    | "drug_expiry" | "daily_collection" | "receivables_aging"
    | "insurance_claims" | "stock_status" | "user_activity" | "backup_status";
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
        description="Daily OPD, IPD census, revenue, lab turnaround, doctor performance, diagnoses, pharmacy, expiry, collections, aging, claims, stock, user activity & backup status. Export any report to CSV."
      />

      {/* Date pickers bar — two pickers drive the dated cards. */}
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
            label="From (range reports)"
            value={fromDate}
            onChange={setFromDate}
          />
          <DatePicker
            id="report-to-date"
            label="To (range reports)"
            value={toDate}
            onChange={setToDate}
          />
        </div>
      </SectionCard>

      <Tabs defaultValue="core">
        <TabsList className="flex-wrap h-auto">
          <TabsTrigger value="core">Core</TabsTrigger>
          <TabsTrigger value="clinical">Clinical</TabsTrigger>
          <TabsTrigger value="financial">Financial</TabsTrigger>
          <TabsTrigger value="inventory">Inventory</TabsTrigger>
          <TabsTrigger value="admin">Admin</TabsTrigger>
        </TabsList>

        <TabsContent value="core" className="pt-4 space-y-6">
          <DailyOpdCard asOfDate={asOfDate} />
          <IpdCensusCard asOfDate={asOfDate} />
          <RevenueCard fromDate={fromDate} toDate={toDate} />
          <LabTurnaroundCard fromDate={fromDate} toDate={toDate} />
        </TabsContent>

        <TabsContent value="clinical" className="pt-4 space-y-6">
          <DoctorPerformanceCard fromDate={fromDate} toDate={toDate} />
          <DiagnosisFrequencyCard fromDate={fromDate} toDate={toDate} />
        </TabsContent>

        <TabsContent value="financial" className="pt-4 space-y-6">
          <DailyCollectionCard fromDate={fromDate} toDate={toDate} />
          <ReceivablesAgingCard />
          <InsuranceClaimsCard />
        </TabsContent>

        <TabsContent value="inventory" className="pt-4 space-y-6">
          <PharmacyConsumptionCard fromDate={fromDate} toDate={toDate} />
          <StockStatusCard />
          <DrugExpiryCard />
        </TabsContent>

        <TabsContent value="admin" className="pt-4 space-y-6">
          <UserActivityCard fromDate={fromDate} toDate={toDate} />
          <BackupStatusCard />
        </TabsContent>
      </Tabs>
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

// ── Phase 6.4 cards (SRS §4.20 gap list) ───────────────────────────────────
//
// Compact variants of the Phase 2-A card pattern: stat row + one table +
// export button. All money via formatMoney; empty datasets render an
// inline muted note instead of a full EmptyState to keep tab switching fast.

function DoctorPerformanceCard({ fromDate, toDate }: { fromDate: string; toDate: string }) {
  const { data, isLoading, isError, error } = useDoctorPerformanceReport(fromDate, toDate);
  return (
    <SectionCard
      icon={Stethoscope}
      title="Doctor Performance"
      description={`Appointments, encounters, lab orders and prescriptions per active doctor, ${fromDate} → ${toDate}.`}
      action={
        <ExportCsvButton
          reportType="doctor_performance"
          params={{ from_date: fromDate, to_date: toDate }}
          filename={`doctor-performance_${fromDate}_${toDate}.csv`}
          disabled={!data}
        />
      }
      bodyClassName="p-6"
    >
      {isLoading ? (
        <ReportLoading label="Aggregating doctor activity…" />
      ) : isError ? (
        <ReportError message={String(error)} />
      ) : !data ? null : data.doctors.length === 0 ? (
        <p className="text-xs text-muted-foreground">No active doctors.</p>
      ) : (
        <Table>
          <TableHeader>
            <TableRow className="border-border hover:bg-transparent">
              <TableHead>Doctor</TableHead>
              <TableHead className="text-right">Appointments</TableHead>
              <TableHead className="text-right">Encounters</TableHead>
              <TableHead className="text-right">Lab orders</TableHead>
              <TableHead className="text-right">Prescriptions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {data.doctors.map((d) => (
              <TableRow key={d.doctor_id}>
                <TableCell className="font-medium">{d.doctor_name}</TableCell>
                <TableCell className="text-right">{d.appointments}</TableCell>
                <TableCell className="text-right">{d.encounters}</TableCell>
                <TableCell className="text-right">{d.lab_orders}</TableCell>
                <TableCell className="text-right">{d.prescriptions}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
    </SectionCard>
  );
}

function DiagnosisFrequencyCard({ fromDate, toDate }: { fromDate: string; toDate: string }) {
  const { data, isLoading, isError, error } = useDiagnosisFrequencyReport(fromDate, toDate);
  return (
    <SectionCard
      icon={ClipboardList}
      title="Diagnosis Frequency"
      description={`${data?.total_encounters ?? 0} diagnosed encounters, ${fromDate} → ${toDate}. Top 20 diagnoses.`}
      action={
        <ExportCsvButton
          reportType="diagnosis_frequency"
          params={{ from_date: fromDate, to_date: toDate }}
          filename={`diagnosis-frequency_${fromDate}_${toDate}.csv`}
          disabled={!data}
        />
      }
      bodyClassName="p-6"
    >
      {isLoading ? (
        <ReportLoading label="Grouping diagnoses…" />
      ) : isError ? (
        <ReportError message={String(error)} />
      ) : !data ? null : data.top_diagnoses.length === 0 ? (
        <p className="text-xs text-muted-foreground">No diagnosed encounters in this range.</p>
      ) : (
        <Table>
          <TableHeader>
            <TableRow className="border-border hover:bg-transparent">
              <TableHead>Diagnosis</TableHead>
              <TableHead className="text-right">Encounters</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {data.top_diagnoses.map((d) => (
              <TableRow key={d.diagnosis}>
                <TableCell className="font-medium">{d.diagnosis}</TableCell>
                <TableCell className="text-right">{d.encounter_count}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
    </SectionCard>
  );
}

function DailyCollectionCard({ fromDate, toDate }: { fromDate: string; toDate: string }) {
  const { data, isLoading, isError, error } = useDailyCollectionReport(fromDate, toDate);
  return (
    <SectionCard
      icon={CalendarClock}
      title="Daily Collection"
      description={`Payments and refunds per day, ${fromDate} → ${toDate}. Net = collected − refunded.`}
      action={
        <ExportCsvButton
          reportType="daily_collection"
          params={{ from_date: fromDate, to_date: toDate }}
          filename={`daily-collection_${fromDate}_${toDate}.csv`}
          disabled={!data}
        />
      }
      bodyClassName="p-6"
    >
      {isLoading ? (
        <ReportLoading label="Aggregating collections…" />
      ) : isError ? (
        <ReportError message={String(error)} />
      ) : !data ? null : (
        <>
          <div className="grid grid-cols-2 lg:grid-cols-3 gap-4 mb-4">
            <StatCard icon={DollarSign} label="Total collected" value={formatMoney(data.total_collected)} sub="Range total" color="success" />
            <StatCard icon={DollarSign} label="Total refunded" value={formatMoney(data.total_refunded)} sub="Range total" color="warning" />
            <StatCard icon={DollarSign} label="Net collection" value={formatMoney(data.total_collected - data.total_refunded)} sub="Collected − refunded" color="primary" />
          </div>
          <Table>
            <TableHeader>
              <TableRow className="border-border hover:bg-transparent">
                <TableHead>Date</TableHead>
                <TableHead className="text-right">Payments</TableHead>
                <TableHead className="text-right">Collected</TableHead>
                <TableHead className="text-right">Refunded</TableHead>
                <TableHead className="text-right">Net</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {data.by_day.map((d) => (
                <TableRow key={d.date}>
                  <TableCell className="font-mono text-xs">{d.date}</TableCell>
                  <TableCell className="text-right">{d.payments}</TableCell>
                  <TableCell className="text-right">{formatMoney(d.collected)}</TableCell>
                  <TableCell className="text-right text-muted-foreground">{d.refunded > 0 ? `−${formatMoney(d.refunded)}` : "—"}</TableCell>
                  <TableCell className="text-right font-medium">{formatMoney(d.net)}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </>
      )}
    </SectionCard>
  );
}

function ReceivablesAgingCard() {
  const { data, isLoading, isError, error } = useReceivablesAgingReport();
  return (
    <SectionCard
      icon={Hourglass}
      title="Receivables Aging"
      description={`Open bills bucketed by outstanding age as of ${data?.as_of_date ?? "today"}.`}
      action={
        <ExportCsvButton
          reportType="receivables_aging"
          params={{}}
          filename="receivables-aging.csv"
          disabled={!data}
        />
      }
      bodyClassName="p-6"
    >
      {isLoading ? (
        <ReportLoading label="Bucketing receivables…" />
      ) : isError ? (
        <ReportError message={String(error)} />
      ) : !data ? null : data.buckets.length === 0 ? (
        <p className="text-xs text-muted-foreground">No outstanding receivables — every open bill is settled.</p>
      ) : (
        <>
          <div className="grid grid-cols-2 lg:grid-cols-3 gap-4 mb-4">
            <StatCard icon={Hourglass} label="Outstanding" value={formatMoney(data.total_outstanding)} sub={`${data.total_open_bills} open bills`} color="warning" />
          </div>
          <Table>
            <TableHeader>
              <TableRow className="border-border hover:bg-transparent">
                <TableHead>Age bucket</TableHead>
                <TableHead className="text-right">Bills</TableHead>
                <TableHead className="text-right">Outstanding</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {data.buckets.map((b) => (
                <TableRow key={b.bucket}>
                  <TableCell className="font-medium">{b.bucket}</TableCell>
                  <TableCell className="text-right">{b.bill_count}</TableCell>
                  <TableCell className="text-right font-medium">{formatMoney(b.outstanding)}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </>
      )}
    </SectionCard>
  );
}

function InsuranceClaimsCard() {
  const { data, isLoading, isError, error } = useInsuranceClaimsReport();
  return (
    <SectionCard
      icon={Landmark}
      title="Insurance / TPA Claims"
      description={`Claim pipeline by status as of ${data?.as_of_date ?? "today"} (Phase 6.3 claims module).`}
      action={
        <ExportCsvButton
          reportType="insurance_claims"
          params={{}}
          filename="insurance-claims.csv"
          disabled={!data}
        />
      }
      bodyClassName="p-6"
    >
      {isLoading ? (
        <ReportLoading label="Aggregating claims…" />
      ) : isError ? (
        <ReportError message={String(error)} />
      ) : !data ? null : data.by_status.length === 0 ? (
        <p className="text-xs text-muted-foreground">No insurance claims recorded yet.</p>
      ) : (
        <>
          <div className="grid grid-cols-3 gap-4 mb-4">
            <StatCard icon={Landmark} label="Total claims" value={data.total_claims} sub="All statuses" color="primary" />
            <StatCard icon={DollarSign} label="Claimed" value={formatMoney(data.total_claimed)} sub="Requested from insurers" color="info" />
            <StatCard icon={DollarSign} label="Approved" value={formatMoney(data.total_approved)} sub="Approved / partially approved" color="success" />
          </div>
          <Table>
            <TableHeader>
              <TableRow className="border-border hover:bg-transparent">
                <TableHead>Status</TableHead>
                <TableHead className="text-right">Claims</TableHead>
                <TableHead className="text-right">Claimed</TableHead>
                <TableHead className="text-right">Approved</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {data.by_status.map((s) => (
                <TableRow key={s.status}>
                  <TableCell className="font-medium capitalize">{s.status.replace("_", " ")}</TableCell>
                  <TableCell className="text-right">{s.claims}</TableCell>
                  <TableCell className="text-right">{formatMoney(s.claimed_amount)}</TableCell>
                  <TableCell className="text-right">{formatMoney(s.approved_amount)}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </>
      )}
    </SectionCard>
  );
}

function PharmacyConsumptionCard({ fromDate, toDate }: { fromDate: string; toDate: string }) {
  const { data, isLoading, isError, error } = usePharmacyConsumptionReport(fromDate, toDate);
  return (
    <SectionCard
      icon={Pill}
      title="Pharmacy Consumption"
      description={`${data?.total_dispensed_items ?? 0} dispensed items, ${fromDate} → ${toDate}. Top 20 medications.`}
      action={
        <ExportCsvButton
          reportType="pharmacy_consumption"
          params={{ from_date: fromDate, to_date: toDate }}
          filename={`pharmacy-consumption_${fromDate}_${toDate}.csv`}
          disabled={!data}
        />
      }
      bodyClassName="p-6"
    >
      {isLoading ? (
        <ReportLoading label="Aggregating dispensing…" />
      ) : isError ? (
        <ReportError message={String(error)} />
      ) : !data ? null : data.top_medications.length === 0 ? (
        <p className="text-xs text-muted-foreground">No dispensed medication in this range.</p>
      ) : (
        <Table>
          <TableHeader>
            <TableRow className="border-border hover:bg-transparent">
              <TableHead>Medication</TableHead>
              <TableHead className="text-right">Times dispensed</TableHead>
              <TableHead className="text-right">Total quantity</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {data.top_medications.map((m) => (
              <TableRow key={m.medication_name}>
                <TableCell className="font-medium">{m.medication_name}</TableCell>
                <TableCell className="text-right">{m.times_dispensed}</TableCell>
                <TableCell className="text-right">{m.total_quantity}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
    </SectionCard>
  );
}

function StockStatusCard() {
  const { data, isLoading, isError, error } = useStockStatusReport();
  return (
    <SectionCard
      icon={Package}
      title="Stock Status"
      description={`Inventory value and low-stock items as of ${data?.as_of_date ?? "today"}.`}
      action={
        <ExportCsvButton
          reportType="stock_status"
          params={{}}
          filename="stock-status.csv"
          disabled={!data}
        />
      }
      bodyClassName="p-6"
    >
      {isLoading ? (
        <ReportLoading label="Counting stock…" />
      ) : isError ? (
        <ReportError message={String(error)} />
      ) : !data ? null : (
        <>
          <div className="grid grid-cols-3 gap-4 mb-4">
            <StatCard icon={Package} label="Active items" value={data.active_items} sub="All categories" color="primary" />
            <StatCard icon={DollarSign} label="Stock value" value={formatMoney(data.total_stock_value)} sub="At unit cost" color="info" />
            <StatCard icon={AlertTriangle} label="Low-stock items" value={data.low_stock_count} sub="At/below reorder level" color={data.low_stock_count > 0 ? "warning" : "success"} />
          </div>
          {data.low_stock_items.length === 0 ? (
            <p className="text-xs text-muted-foreground">No items at or below their reorder level.</p>
          ) : (
            <Table>
              <TableHeader>
                <TableRow className="border-border hover:bg-transparent">
                  <TableHead>Item</TableHead>
                  <TableHead className="text-right">Stock</TableHead>
                  <TableHead className="text-right">Reorder level</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {data.low_stock_items.map((i) => (
                  <TableRow key={i.name}>
                    <TableCell className="font-medium">{i.name}</TableCell>
                    <TableCell className="text-right text-destructive">{i.stock_quantity}</TableCell>
                    <TableCell className="text-right">{i.reorder_level}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </>
      )}
    </SectionCard>
  );
}

function DrugExpiryCard() {
  const { data, isLoading, isError, error } = useDrugExpiryReport();
  return (
    <SectionCard
      icon={AlertTriangle}
      title="Drug Expiry"
      description="Expired and soon-expiring medication stock (≤180 days), with value at risk."
      action={
        <ExportCsvButton
          reportType="drug_expiry"
          params={{}}
          filename="drug-expiry.csv"
          disabled={!data}
        />
      }
      bodyClassName="p-6"
    >
      {isLoading ? (
        <ReportLoading label="Scanning expiry dates…" />
      ) : isError ? (
        <ReportError message={String(error)} />
      ) : !data ? null : (
        <>
          <div className="grid grid-cols-2 lg:grid-cols-4 gap-4 mb-4">
            <StatCard icon={AlertTriangle} label="Expired" value={data.expired_count} sub={formatMoney(data.expired_stock_value)} color={data.expired_count > 0 ? "destructive" : "success"} />
            <StatCard icon={AlertTriangle} label="Expiring ≤90 days" value={data.expiring_90_days} sub={formatMoney(data.expiring_90_stock_value)} color="warning" />
            <StatCard icon={Calendar} label="Expiring ≤180 days" value={data.expiring_180_days} sub="After the 90-day window" color="info" />
          </div>
          {data.items.length === 0 ? (
            <p className="text-xs text-muted-foreground">No medication expiring within 180 days.</p>
          ) : (
            <Table>
              <TableHeader>
                <TableRow className="border-border hover:bg-transparent">
                  <TableHead>Item</TableHead>
                  <TableHead>Batch</TableHead>
                  <TableHead className="text-right">Stock</TableHead>
                  <TableHead>Expiry</TableHead>
                  <TableHead className="text-right">Days left</TableHead>
                  <TableHead className="text-right">Stock value</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {data.items.map((i, idx) => (
                  <TableRow key={`${i.name}-${idx}`}>
                    <TableCell className="font-medium">{i.name}</TableCell>
                    <TableCell className="font-mono text-xs">{i.batch_number ?? "—"}</TableCell>
                    <TableCell className="text-right">{i.stock_quantity}</TableCell>
                    <TableCell className="font-mono text-xs">{i.expiry_date ?? "—"}</TableCell>
                    <TableCell className={`text-right font-medium ${(i.days_until_expiry ?? 0) < 0 ? "text-destructive" : ""}`}>
                      {(i.days_until_expiry ?? 0) < 0 ? `expired ${Math.abs(i.days_until_expiry ?? 0)}d ago` : i.days_until_expiry}
                    </TableCell>
                    <TableCell className="text-right">{formatMoney(i.stock_value)}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </>
      )}
    </SectionCard>
  );
}

function UserActivityCard({ fromDate, toDate }: { fromDate: string; toDate: string }) {
  const { data, isLoading, isError, error } = useUserActivityReport(fromDate, toDate);
  return (
    <SectionCard
      icon={Activity}
      title="User Activity"
      description={`${data?.total_actions ?? 0} audited actions by staff, ${fromDate} → ${toDate}. Requires the audit-view permission (enforced server-side).`}
      action={
        <ExportCsvButton
          reportType="user_activity"
          params={{ from_date: fromDate, to_date: toDate }}
          filename={`user-activity_${fromDate}_${toDate}.csv`}
          disabled={!data}
        />
      }
      bodyClassName="p-6"
    >
      {isLoading ? (
        <ReportLoading label="Aggregating audit activity…" />
      ) : isError ? (
        <ReportError message={String(error)} />
      ) : !data ? null : data.by_user.length === 0 ? (
        <p className="text-xs text-muted-foreground">No audited activity in this range (or you lack the audit-view permission).</p>
      ) : (
        <Table>
          <TableHeader>
            <TableRow className="border-border hover:bg-transparent">
              <TableHead>User</TableHead>
              <TableHead className="text-right">Actions</TableHead>
              <TableHead>Last action (UTC)</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {data.by_user.map((u) => (
              <TableRow key={u.username}>
                <TableCell className="font-medium">{u.full_name ?? u.username}</TableCell>
                <TableCell className="text-right">{u.action_count}</TableCell>
                <TableCell className="text-xs text-muted-foreground">{u.last_action_at ?? "—"}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
    </SectionCard>
  );
}

function BackupStatusCard() {
  const { data, isLoading, isError, error } = useBackupStatusReport();
  return (
    <SectionCard
      icon={DatabaseBackup}
      title="Backup Status"
      description="Database backup files on this machine. Create and restore backups on the Backup page."
      action={
        <ExportCsvButton
          reportType="backup_status"
          params={{}}
          filename="backup-status.csv"
          disabled={!data}
        />
      }
      bodyClassName="p-6"
    >
      {isLoading ? (
        <ReportLoading label="Checking backup files…" />
      ) : isError ? (
        <ReportError message={String(error)} />
      ) : !data ? null : data.backup_count === 0 ? (
        <p className="text-xs text-muted-foreground">No backup files found yet — run a backup from the Backup page.</p>
      ) : (
        <>
          <div className="grid grid-cols-3 gap-4 mb-4">
            <StatCard icon={DatabaseBackup} label="Backups" value={data.backup_count} sub="Files in store" color="primary" />
            <StatCard
              icon={Calendar}
              label="Latest backup"
              value={data.latest_backup_age_days === 0 ? "Today" : `${data.latest_backup_age_days} day(s) ago`}
              sub="Age of newest file"
              color={(data.latest_backup_age_days ?? 999) > 1 ? "warning" : "success"}
            />
            <StatCard icon={Package} label="Total size" value={`${(data.total_size_bytes / 1_048_576).toFixed(1)} MB`} sub="All backup files" color="info" />
          </div>
          <Table>
            <TableHeader>
              <TableRow className="border-border hover:bg-transparent">
                <TableHead>File</TableHead>
                <TableHead className="text-right">Size</TableHead>
                <TableHead className="text-right">Age (days)</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {data.files.map((f) => (
                <TableRow key={f.filename}>
                  <TableCell className="font-mono text-xs">{f.filename}</TableCell>
                  <TableCell className="text-right">{(f.size_bytes / 1_048_576).toFixed(1)} MB</TableCell>
                  <TableCell className="text-right">{f.age_days}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </>
      )}
    </SectionCard>
  );
}
