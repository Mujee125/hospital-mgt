/**
 * Radiology — orders, reports, and verification workflow
 * (Phase 2-D, SRS FR-0140–FR-0142).
 *
 * Page pattern matches Laboratory.tsx (orders list + results dialog) and
 * Appointments.tsx (quick status buttons per row). The page composes the
 * shared layout primitives — PageContainer, PageHeader, SectionCard,
 * PageToolbar, Table, EmptyState, LoadingState, StatusBadge, StatCard,
 * FormField, Pagination — so it is visually homogeneous with the rest of
 * the app.
 *
 * Three dialogs:
 *   1. New order dialog — patient/doctor/study-type/priority/contrast/etc.
 *   2. Report dialog — findings/impression/recommendations/critical flag.
 *      Read-only once a report exists and is verified; otherwise shows a
 *      verify button if a report exists but is unverified.
 *   3. Delete confirmation — replaces window.confirm() with a shadcn Dialog.
 *
 * Quick-status buttons per row mirror Appointments.tsx: Schedule / Start /
 * Complete / Cancel, gated by the current row status (terminal states hide
 * the relevant buttons). The Report action is only visible for orders in
 * a status that has produced imaging (completed or beyond).
 */
import { useState } from "react";
import {
  ScanLine,
  Plus,
  Loader2,
  AlertTriangle,
  CheckCircle2,
  ShieldCheck,
  FileText,
  Ban,
  Search,
  Trash2,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
  DialogClose,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import {
  useRadiologyOrders,
  useRadiologyDashboard,
  useCreateRadiologyOrder,
  useUpdateRadiologyOrderStatus,
  useDeleteRadiologyOrder,
  useRadiologyReport,
  useCreateRadiologyReport,
  useVerifyRadiologyReport,
  usePatientsEhr,
  useDoctors,
} from "@/lib/queries";
import { useAuth } from "@/lib/auth";
import { PERMISSIONS } from "@/lib/rbac";
import type { RadiologyOrder, CreateRadiologyOrder } from "@/lib/models";
import {
  PageContainer,
  PageHeader,
  SectionCard,
  EmptyState,
  StatusBadge,
  LoadingState,
  PageToolbar,
  StatCard,
  FormField,
  Pagination,
} from "@/components/layout/shared";

// ── Constants ──────────────────────────────────────────────────────────────
//
// Catalogue values kept in one place so the new-order dialog, the filter
// dropdowns, and the badge renderer never disagree. Mirrors the
// `LabCatalogManage` convention — the values are not enforced server-side
// (the backend stores them as free-form VARCHAR) but the UI constrains
// them to the canonical list per the RAD-2 spec wording.
const STUDY_TYPES = [
  "X-Ray",
  "CT Scan",
  "MRI",
  "Ultrasound",
  "Mammography",
  "Fluoroscopy",
  "DEXA",
  "Other",
] as const;

const PRIORITIES = ["routine", "urgent", "emergency", "stat"] as const;

// Terminal statuses — once an order is in one of these, the relevant quick
// action buttons are hidden (the operator cannot, e.g., "Complete" a
// verified order from the row).
const TERMINAL_STATUSES = new Set(["verified", "cancelled"]);

/** Priority → StatCard color token + human label, used by the badge renderer. */
const PRIORITY_STYLE: Record<string, { color: string; label: string }> = {
  routine: { color: "var(--muted-foreground)", label: "Routine" },
  urgent: { color: "var(--status-no-show)", label: "Urgent" },
  emergency: { color: "var(--status-cancelled)", label: "Emergency" },
  stat: { color: "var(--destructive)", label: "STAT" },
};

function PriorityBadge({ priority }: { priority: string }) {
  const style = PRIORITY_STYLE[priority] ?? PRIORITY_STYLE.routine!;
  return (
    <span
      className="status-badge uppercase tracking-wide"
      style={{ background: `hsl(${style.color} / 0.10)`, color: `hsl(${style.color})` }}
    >
      <span className="h-1.5 w-1.5 rounded-full" style={{ background: `hsl(${style.color})` }} />
      {style.label}
    </span>
  );
}

// ── Page ───────────────────────────────────────────────────────────────────

export function Radiology() {
  const { has } = useAuth();
  const { data: dashboard } = useRadiologyDashboard();
  const { data: ordersResp, isLoading } = useRadiologyOrders(
    undefined, undefined, 1, 10,
  );
  const { data: patients = [] } = usePatientsEhr();
  const { data: doctors = [] } = useDoctors();
  const createOrder = useCreateRadiologyOrder();
  const updateStatus = useUpdateRadiologyOrderStatus();
  const deleteOrder = useDeleteRadiologyOrder();

  const [search, setSearch] = useState("");
  const [statusFilter, setStatusFilter] = useState<string>("all");
  const [priorityFilter, setPriorityFilter] = useState<string>("all");

  const [orderDialogOpen, setOrderDialogOpen] = useState(false);
  const [reportOrderId, setReportOrderId] = useState<number | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<RadiologyOrder | null>(null);

  // P0-6: Server-side pagination state — page number is sent to the backend
  // via useRadiologyOrders hook params. The local state drives the query key.
  const [, setPage] = useState(1);
  const [, setRowsPerPage] = useState(10);

  const canCreate = has(PERMISSIONS.RadiologyCreate);
  const canUpdate = has(PERMISSIONS.RadiologyUpdate);
  const canDelete = has(PERMISSIONS.RadiologyDelete);

  // P0-6: Server-side pagination — orders come pre-paginated from the backend.
  // Client-side search filters the current page only (the backend returns
  // status/priority-filtered + paginated results; search is a client-side
  // convenience on the current page).
  const orders = ordersResp?.orders ?? [];
  const totalCount = ordersResp?.total ?? 0;
  const currentPage = ordersResp?.page ?? 1;

  const filtered = orders.filter((o) => {
    const q = search.toLowerCase();
    const matchesSearch =
      !q ||
      (o.patient_name ?? "").toLowerCase().includes(q) ||
      o.order_number.toLowerCase().includes(q) ||
      o.study_type.toLowerCase().includes(q);
    return matchesSearch;
  });

  const handleQuickStatus = (id: number, status: string) => {
    updateStatus.mutate({ id, status });
  };

  const confirmDelete = () => {
    if (!deleteTarget) return;
    deleteOrder.mutate(
      { id: deleteTarget.id, reason: "Deleted by user" },
      { onSettled: () => setDeleteTarget(null) },
    );
  };

  return (
    <PageContainer>
      <PageHeader
        icon={ScanLine}
        title="Radiology"
        description="Imaging orders, reports & verification"
        actions={
          canCreate && (
            <Button onClick={() => setOrderDialogOpen(true)}>
              <Plus className="h-4 w-4" /> New radiology order
            </Button>
          )
        }
      />

      {/* ── KPI grid ───────────────────────────────────────────────
          Six counts surfaced by `get_radiology_dashboard`: today's studies,
          pending reports, emergency cases, completed today, cancelled
          (all-time), and reports awaiting verification. The grid uses the
          same `grid-cols-2 lg:grid-cols-4 items-stretch` pattern as the
          Dashboard so cards render at identical height. */}
      <div className="grid grid-cols-2 lg:grid-cols-3 xl:grid-cols-6 gap-4 items-stretch">
        <StatCard
          icon={ScanLine}
          label="Studies today"
          value={dashboard?.studies_today ?? "—"}
          color="primary"
        />
        <StatCard
          icon={FileText}
          label="Pending reports"
          value={dashboard?.pending_reports ?? "—"}
          sub="Awaiting imaging / report"
          color="info"
        />
        <StatCard
          icon={AlertTriangle}
          label="Emergency cases"
          value={dashboard?.emergency_cases ?? "—"}
          sub="Active emergencies"
          color="destructive"
        />
        <StatCard
          icon={CheckCircle2}
          label="Completed today"
          value={dashboard?.completed_today ?? "—"}
          sub="Imaging finished"
          color="success"
        />
        <StatCard
          icon={Ban}
          label="Cancelled"
          value={dashboard?.cancelled ?? "—"}
          sub="All-time"
          color="warning"
        />
        <StatCard
          icon={ShieldCheck}
          label="Verification pending"
          value={dashboard?.verification_pending ?? "—"}
          sub="Reports not yet verified"
          color="accent"
        />
      </div>

      <SectionCard>
        {isLoading ? (
          <LoadingState rows={6} />
        ) : orders.length === 0 ? (
          <EmptyState
            icon={ScanLine}
            title="No radiology orders"
            description="Create a radiology order to begin the imaging workflow."
            action={
              canCreate && (
                <Button size="sm" onClick={() => setOrderDialogOpen(true)}>
                  <Plus className="h-3.5 w-3.5" /> New radiology order
                </Button>
              )
            }
          />
        ) : (
          <>
            <PageToolbar>
              <div className="relative w-full max-w-xs">
                <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground pointer-events-none" />
                <Input
                  placeholder="Search patient, order #, or study…"
                  value={search}
                  onChange={(e) => {
                    setSearch(e.target.value);
                    setPage(1);
                  }}
                  className="pl-9"
                />
              </div>
              <Select
                value={statusFilter}
                onValueChange={(v) => {
                  setStatusFilter(v);
                  setPage(1);
                }}
              >
                <SelectTrigger className="w-[170px]">
                  <SelectValue placeholder="All statuses" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">All statuses</SelectItem>
                  <SelectItem value="ordered">Ordered</SelectItem>
                  <SelectItem value="scheduled">Scheduled</SelectItem>
                  <SelectItem value="in-progress">In progress</SelectItem>
                  <SelectItem value="completed">Completed</SelectItem>
                  <SelectItem value="reported">Reported</SelectItem>
                  <SelectItem value="verified">Verified</SelectItem>
                  <SelectItem value="cancelled">Cancelled</SelectItem>
                </SelectContent>
              </Select>
              <Select
                value={priorityFilter}
                onValueChange={(v) => {
                  setPriorityFilter(v);
                  setPage(1);
                }}
              >
                <SelectTrigger className="w-[170px]">
                  <SelectValue placeholder="All priorities" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">All priorities</SelectItem>
                  {PRIORITIES.map((p) => (
                    <SelectItem key={p} value={p} className="capitalize">
                      {PRIORITY_STYLE[p]?.label ?? p}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <span className="text-xs text-muted-foreground ml-auto tabular-nums">
                {filtered.length} of {totalCount} orders
              </span>
            </PageToolbar>

            <Table>
              <TableHeader>
                <TableRow className="hover:bg-transparent">
                  <TableHead>Order #</TableHead>
                  <TableHead>Patient</TableHead>
                  <TableHead>Study</TableHead>
                  <TableHead>Priority</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Ordered at</TableHead>
                  <TableHead>Quick actions</TableHead>
                  <TableHead className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {filtered.map((o) => {
                  const terminal = TERMINAL_STATUSES.has(o.status);
                  // The Report action is available once imaging is done
                  // (completed) and stays available through the reported /
                  // verified lifecycle so the operator can review the
                  // filed report at any time. Pre-completion orders have
                  // nothing to report against yet.
                  const canAccessReport =
                    o.status === "completed" ||
                    o.status === "reported" ||
                    o.status === "verified";
                  return (
                    <TableRow key={o.id}>
                      <TableCell className="font-mono text-xs">
                        {o.order_number}
                      </TableCell>
                      <TableCell className="font-medium">
                        {o.patient_name ?? "—"}
                        {o.body_part && (
                          <span className="block text-[10px] text-muted-foreground">
                            {o.body_part}
                            {o.contrast_required ? " · contrast" : ""}
                          </span>
                        )}
                      </TableCell>
                      <TableCell>
                        <div className="text-sm">{o.study_type}</div>
                        {o.radiologist_name && (
                          <div className="text-[10px] text-muted-foreground">
                            Dr. {o.radiologist_name}
                          </div>
                        )}
                      </TableCell>
                      <TableCell>
                        <PriorityBadge priority={o.priority} />
                      </TableCell>
                      <TableCell>
                        <StatusBadge status={o.status} />
                      </TableCell>
                      <TableCell className="text-xs text-muted-foreground">
                        {new Date(o.ordered_at).toLocaleString()}
                      </TableCell>
                      <TableCell>
                        <div className="flex flex-wrap gap-1">
                          {canUpdate && o.status === "ordered" && (
                            <Button
                              variant="outline"
                              size="sm"
                              className="h-7 px-2.5 text-[11px] font-semibold"
                              onClick={() => handleQuickStatus(o.id, "scheduled")}
                              disabled={updateStatus.isPending}
                            >
                              Schedule
                            </Button>
                          )}
                          {canUpdate && o.status === "scheduled" && (
                            <Button
                              variant="outline"
                              size="sm"
                              className="h-7 px-2.5 text-[11px] font-semibold text-status-confirmed border-status-confirmed/30 hover:bg-status-confirmed/10"
                              onClick={() => handleQuickStatus(o.id, "in-progress")}
                              disabled={updateStatus.isPending}
                            >
                              Start
                            </Button>
                          )}
                          {canUpdate && o.status === "in-progress" && (
                            <Button
                              variant="outline"
                              size="sm"
                              className="h-7 px-2.5 text-[11px] font-semibold text-status-confirmed border-status-confirmed/30 hover:bg-status-confirmed/10"
                              onClick={() => handleQuickStatus(o.id, "completed")}
                              disabled={updateStatus.isPending}
                            >
                              Complete
                            </Button>
                          )}
                          {canUpdate && !terminal && (
                            <Button
                              variant="outline"
                              size="sm"
                              className="h-7 px-2.5 text-[11px] font-semibold text-status-cancelled border-status-cancelled/30 hover:bg-status-cancelled/10"
                              onClick={() => handleQuickStatus(o.id, "cancelled")}
                              disabled={updateStatus.isPending}
                            >
                              Cancel
                            </Button>
                          )}
                        </div>
                      </TableCell>
                      <TableCell className="text-right">
                        <div className="flex justify-end gap-1">
                          {canAccessReport && (
                            <Button
                              variant="ghost"
                              size="sm"
                              className="h-8 px-2.5"
                              onClick={() => setReportOrderId(o.id)}
                            >
                              {o.status === "reported" || o.status === "verified"
                                ? "View report"
                                : "Report"}
                            </Button>
                          )}
                          {canDelete && (
                            <Button
                              variant="ghost"
                              size="icon"
                              className="h-8 w-8 text-destructive hover:text-destructive hover:bg-destructive/10"
                              title="Delete order"
                              aria-label={`Delete radiology order ${o.order_number}`}
                              onClick={() => setDeleteTarget(o)}
                              disabled={deleteOrder.isPending}
                            >
                              <Trash2 className="h-4 w-4" />
                            </Button>
                          )}
                        </div>
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
            <Pagination
              totalItems={totalCount}
              page={currentPage}
              rowsPerPage={ordersResp?.page_size ?? 10}
              onPageChange={setPage}
              onRowsPerPageChange={(r) => {
                setRowsPerPage(r);
                setPage(1);
              }}
            />
          </>
        )}
      </SectionCard>

      {/* New order dialog */}
      <NewOrderDialog
        open={orderDialogOpen}
        onOpenChange={setOrderDialogOpen}
        patients={patients}
        doctors={doctors}
        isPending={createOrder.isPending}
        onSubmit={async (payload) => {
          await createOrder.mutateAsync(payload);
          setOrderDialogOpen(false);
        }}
      />

      {/* Report dialog */}
      {reportOrderId != null && (
        <ReportDialog
          orderId={reportOrderId}
          onClose={() => setReportOrderId(null)}
          canReport={has(PERMISSIONS.RadiologyReport)}
          canVerify={has(PERMISSIONS.RadiologyVerify)}
        />
      )}

      {/* Delete confirmation */}
      <Dialog open={deleteTarget !== null} onOpenChange={(o) => !o && setDeleteTarget(null)}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>Delete radiology order?</DialogTitle>
            <DialogDescription>This action cannot be undone.</DialogDescription>
          </DialogHeader>
          {deleteTarget && (
            <p className="text-sm text-muted-foreground leading-relaxed">
              Permanently delete order{" "}
              <span className="font-semibold text-foreground font-mono">
                {deleteTarget.order_number}
              </span>{" "}
              for{" "}
              <span className="font-semibold text-foreground">
                {deleteTarget.patient_name ?? "—"}
              </span>{" "}
              ({deleteTarget.study_type})? All linked reports and status history will be removed.
            </p>
          )}
          <DialogFooter>
            <DialogClose asChild>
              <Button variant="outline">Cancel</Button>
            </DialogClose>
            <Button
              variant="destructive"
              onClick={confirmDelete}
              disabled={deleteOrder.isPending}
            >
              {deleteOrder.isPending ? "Deleting…" : "Delete order"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </PageContainer>
  );
}

// ── New Order dialog ───────────────────────────────────────────────────────

interface NewOrderDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  patients: { id: number; first_name: string; last_name: string; phone: string }[];
  doctors: { id: number; first_name: string; last_name: string; is_active: boolean }[];
  isPending: boolean;
  onSubmit: (payload: CreateRadiologyOrder) => Promise<void>;
}

const EMPTY_FORM = {
  patientId: null as number | null,
  doctorId: null as number | null,
  studyType: "X-Ray" as string,
  priority: "routine" as string,
  contrastRequired: false,
  bodyPart: "",
  clinicalIndication: "",
  department: "",
  radiologistId: null as number | null,
  instructions: "",
  expectedDate: "",
};

function NewOrderDialog({
  open,
  onOpenChange,
  patients,
  doctors,
  isPending,
  onSubmit,
}: NewOrderDialogProps) {
  const [form, setForm] = useState(EMPTY_FORM);
  const activeRadiologists = doctors.filter((d) => d.is_active);
  const activeDoctors = doctors.filter((d) => d.is_active);

  const set = <K extends keyof typeof form>(key: K, value: (typeof form)[K]) =>
    setForm((f) => ({ ...f, [key]: value }));

  const submit = async () => {
    if (!form.patientId) return;
    const payload: CreateRadiologyOrder = {
      patient_id: form.patientId,
      ordered_by_doctor_id: form.doctorId,
      department: form.department.trim() || null,
      clinical_indication: form.clinicalIndication.trim() || null,
      priority: form.priority,
      study_type: form.studyType,
      contrast_required: form.contrastRequired,
      body_part: form.bodyPart.trim() || null,
      instructions: form.instructions.trim() || null,
      assigned_radiologist_id: form.radiologistId,
      expected_date: form.expectedDate || null,
    };
    await onSubmit(payload);
    setForm(EMPTY_FORM);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>New radiology order</DialogTitle>
          <DialogDescription>
            Select the patient and study parameters. Required: patient, study type, and priority.
          </DialogDescription>
        </DialogHeader>
        <div className="grid grid-cols-2 gap-4 py-2">
          <FormField label="Patient" required className="col-span-2">
            <Select
              value={form.patientId?.toString() ?? ""}
              onValueChange={(v) => set("patientId", Number(v))}
            >
              <SelectTrigger>
                <SelectValue placeholder="Select patient" />
              </SelectTrigger>
              <SelectContent>
                {patients.map((p) => (
                  <SelectItem key={p.id} value={p.id.toString()}>
                    {p.first_name} {p.last_name} · {p.phone}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </FormField>

          <FormField label="Ordering doctor">
            <Select
              value={form.doctorId?.toString() ?? "none"}
              onValueChange={(v) => set("doctorId", v === "none" ? null : Number(v))}
            >
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="none">—</SelectItem>
                {activeDoctors.map((d) => (
                  <SelectItem key={d.id} value={d.id.toString()}>
                    Dr. {d.first_name} {d.last_name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </FormField>

          <FormField label="Department">
            <Input
              placeholder="e.g. Cardiology, ER"
              value={form.department}
              onChange={(e) => set("department", e.target.value)}
            />
          </FormField>

          <FormField label="Study type" required>
            <Select value={form.studyType} onValueChange={(v) => set("studyType", v)}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {STUDY_TYPES.map((t) => (
                  <SelectItem key={t} value={t}>
                    {t}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </FormField>

          <FormField label="Priority" required>
            <Select value={form.priority} onValueChange={(v) => set("priority", v)}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {PRIORITIES.map((p) => (
                  <SelectItem key={p} value={p} className="capitalize">
                    {PRIORITY_STYLE[p]?.label ?? p}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </FormField>

          <FormField label="Body part">
            <Input
              placeholder="e.g. Chest, Left knee, Abdomen"
              value={form.bodyPart}
              onChange={(e) => set("bodyPart", e.target.value)}
            />
          </FormField>

          <FormField label="Assigned radiologist">
            <Select
              value={form.radiologistId?.toString() ?? "none"}
              onValueChange={(v) => set("radiologistId", v === "none" ? null : Number(v))}
            >
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="none">—</SelectItem>
                {activeRadiologists.map((d) => (
                  <SelectItem key={d.id} value={d.id.toString()}>
                    Dr. {d.first_name} {d.last_name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </FormField>

          <FormField label="Expected date">
            <Input
              type="date"
              value={form.expectedDate}
              onChange={(e) => set("expectedDate", e.target.value)}
            />
          </FormField>

          <FormField label="Contrast" className="col-span-2">
            <label className="flex items-center gap-2 cursor-pointer text-sm">
              <input
                type="checkbox"
                checked={form.contrastRequired}
                onChange={(e) => set("contrastRequired", e.target.checked)}
                className="h-4 w-4 accent-primary"
              />
              <span>Contrast agent required</span>
            </label>
          </FormField>

          <FormField label="Clinical indication" className="col-span-2">
            <Textarea
              placeholder="Reason for the study, symptoms, suspected diagnosis…"
              rows={3}
              value={form.clinicalIndication}
              onChange={(e) => set("clinicalIndication", e.target.value)}
            />
          </FormField>

          <FormField label="Instructions" className="col-span-2">
            <Textarea
              placeholder="Patient prep, positioning, technique notes…"
              rows={2}
              value={form.instructions}
              onChange={(e) => set("instructions", e.target.value)}
            />
          </FormField>
        </div>
        <DialogFooter>
          <DialogClose asChild>
            <Button variant="outline">Cancel</Button>
          </DialogClose>
          <Button
            disabled={!form.patientId || isPending}
            onClick={submit}
          >
            {isPending ? (
              <>
                <Loader2 className="h-4 w-4 animate-spin" /> Placing…
              </>
            ) : (
              "Place order"
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// ── Report dialog ──────────────────────────────────────────────────────────

interface ReportDialogProps {
  orderId: number;
  onClose: () => void;
  canReport: boolean;
  canVerify: boolean;
}

function ReportDialog({ orderId, onClose, canReport, canVerify }: ReportDialogProps) {
  const { data: report, isLoading } = useRadiologyReport(orderId);
  const createReport = useCreateRadiologyReport();
  const verifyReport = useVerifyRadiologyReport();

  const [findings, setFindings] = useState("");
  const [impression, setImpression] = useState("");
  const [recommendations, setRecommendations] = useState("");
  const [critical, setCritical] = useState(false);

  const reportExists = !!report;
  const verified = !!report?.verified_at;

  const submit = async () => {
    await createReport.mutateAsync({
      order_id: orderId,
      findings: findings.trim() || null,
      impression: impression.trim() || null,
      recommendations: recommendations.trim() || null,
      critical_finding: critical,
    });
    // The order's status flips to 'reported' server-side; the query
    // invalidation in useCreateRadiologyReport refetches this dialog.
  };

  const verify = async () => {
    if (!report) return;
    await verifyReport.mutateAsync(report.id);
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>Radiology report — order #{orderId}</DialogTitle>
          <DialogDescription>
            {reportExists
              ? verified
                ? "Verified report — read only."
                : "Report filed — awaiting verification."
              : "Enter the radiologist's findings, impression, and recommendations."}
          </DialogDescription>
        </DialogHeader>

        {isLoading ? (
          <LoadingState rows={4} />
        ) : reportExists ? (
          <div className="space-y-4 py-2">
            {report.critical_finding && (
              <div className="flex items-center gap-2 rounded-[var(--radius-sm)] border border-destructive/30 bg-destructive/8 px-3 py-2 text-xs font-semibold text-destructive">
                <AlertTriangle className="h-4 w-4" />
                Critical finding flagged
              </div>
            )}
            <ReportField label="Findings" value={report.findings} />
            <ReportField label="Impression" value={report.impression} />
            <ReportField label="Recommendations" value={report.recommendations} />
            <div className="flex items-center justify-between text-xs text-muted-foreground pt-2 border-t border-border">
              <span>
                Filed {new Date(report.report_date).toLocaleString()}
              </span>
              {verified && report.verified_at && (
                <span className="flex items-center gap-1.5 text-success font-semibold">
                  <ShieldCheck className="h-3.5 w-3.5" />
                  Verified {new Date(report.verified_at).toLocaleString()}
                </span>
              )}
            </div>
          </div>
        ) : canReport ? (
          <div className="space-y-4 py-2">
            <FormField label="Findings">
              <Textarea
                placeholder="Describe the imaging observations…"
                rows={4}
                value={findings}
                onChange={(e) => setFindings(e.target.value)}
              />
            </FormField>
            <FormField label="Impression">
              <Textarea
                placeholder="Radiologist's interpretation / differential…"
                rows={3}
                value={impression}
                onChange={(e) => setImpression(e.target.value)}
              />
            </FormField>
            <FormField label="Recommendations">
              <Textarea
                placeholder="Follow-up, further imaging, referral…"
                rows={2}
                value={recommendations}
                onChange={(e) => setRecommendations(e.target.value)}
              />
            </FormField>
            <label className="flex items-center gap-2 cursor-pointer text-sm">
              <input
                type="checkbox"
                checked={critical}
                onChange={(e) => setCritical(e.target.checked)}
                className="h-4 w-4 accent-destructive"
              />
              <span className="flex items-center gap-1.5">
                <AlertTriangle className="h-3.5 w-3.5 text-destructive" />
                Flag as critical finding (requires urgent clinician follow-up)
              </span>
            </label>
          </div>
        ) : (
          <div className="py-6 text-center text-sm text-muted-foreground">
            You do not have permission to file a report for this order.
          </div>
        )}

        <DialogFooter>
          <DialogClose asChild>
            <Button variant="outline">Close</Button>
          </DialogClose>
          {reportExists && !verified && canVerify && (
            <Button
              onClick={verify}
              disabled={verifyReport.isPending}
              className="gap-2"
            >
              {verifyReport.isPending ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <ShieldCheck className="h-4 w-4" />
              )}
              Verify report
            </Button>
          )}
          {!reportExists && canReport && (
            <Button
              onClick={submit}
              disabled={createReport.isPending}
              className="gap-2"
            >
              {createReport.isPending ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <FileText className="h-4 w-4" />
              )}
              File report
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function ReportField({ label, value }: { label: string; value: string | null | undefined }) {
  return (
    <div className="space-y-1">
      <div className="text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
        {label}
      </div>
      <div className="text-sm text-foreground whitespace-pre-wrap leading-relaxed">
        {value && value.trim().length > 0 ? value : "—"}
      </div>
    </div>
  );
}
