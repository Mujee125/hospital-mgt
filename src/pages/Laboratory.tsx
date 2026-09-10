/**
 * Laboratory — test orders & results with the Phase 6.2 workflow:
 *   ordered → sampled (barcode printed) → resulted (tech entry) →
 *   approved (released by lab in-charge / doctor).
 *
 * Critical-value protocol: a result flagged critical raises an in-app
 * alert banner and CANNOT be approved until the approver acknowledges
 * that the ordering doctor has been contacted (backend-enforced via
 * chk_lot_critical_release + the approve command's guard).
 */
import { useState } from "react";
import { FlaskConical, Plus, Loader2, CheckCircle2, AlertTriangle, Syringe, ShieldCheck } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter, DialogClose } from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useLabOrders, useLabCatalog, useCreateLabOrder, useLabOrderTests, useUpdateLabResult, useCollectLabSample, useApproveLabResult, usePatientsEhr, useDoctors } from "@/lib/queries";
import { useAuth } from "@/lib/auth";
import { PERMISSIONS } from "@/lib/rbac";
import { formatMoney, patientDescriptor } from "@/lib/utils";
import { PageContainer, PageHeader, SectionCard, EmptyState, ErrorState, StatusBadge, LoadingState, PageToolbar } from "@/components/layout/shared";

const STATUS_FILTERS = [
  { value: "all", label: "All orders" },
  { value: "ordered", label: "Awaiting sample" },
  { value: "sampled", label: "Sampled — awaiting results" },
  { value: "resulted", label: "Resulted — awaiting approval" },
  { value: "approved", label: "Approved / released" },
] as const;

export function Laboratory() {
  const { has } = useAuth();
  const [statusFilter, setStatusFilter] = useState<string>("all");
  const { data: allOrders = [], isLoading, isError, refetch, isFetching } = useLabOrders();
  // Legacy rows (pre-6.2 'completed'/'pending' from old flow or seeds)
  // still display; the workflow filter only matches the 6.2 vocabulary.
  const orders = statusFilter === "all" ? allOrders : allOrders.filter((o) => o.status === statusFilter);
  const { data: catalog = [] } = useLabCatalog();
  const { data: patients = [] } = usePatientsEhr();
  const { data: doctors = [] } = useDoctors();
  const createOrder = useCreateLabOrder();
  const collect = useCollectLabSample();

  const [orderOpen, setOrderOpen] = useState(false);
  const [resultOrderId, setResultOrderId] = useState<number | null>(null);
  const [barcodeOrder, setBarcodeOrder] = useState<{ id: number; barcode: string } | null>(null);
  const [form, setForm] = useState({ patientId: null as number | null, doctorId: null as number | null, testIds: [] as number[] });

  const submit = async () => {
    if (!form.patientId || form.testIds.length === 0) return;
    await createOrder.mutateAsync({ patient_id: form.patientId, ordered_by_doctor_id: form.doctorId, test_catalog_ids: form.testIds });
    setOrderOpen(false);
    setForm({ patientId: null, doctorId: null, testIds: [] });
  };

  const toggleTest = (id: number) =>
    setForm((f) => ({ ...f, testIds: f.testIds.includes(id) ? f.testIds.filter((t) => t !== id) : [...f.testIds, id] }));

  const doCollect = async (orderId: number) => {
    const barcode = await collect.mutateAsync(orderId);
    setBarcodeOrder({ id: orderId, barcode });
  };

  return (
    <PageContainer>
      <PageHeader
        icon={FlaskConical}
        title="Laboratory"
        description="Orders, sample collection, results & approval"
        actions={has(PERMISSIONS.LabOrder) && (
          <Button onClick={() => setOrderOpen(true)}><Plus className="h-4 w-4" /> New lab order</Button>
        )}
      />

      <SectionCard>
        {isLoading ? (
          <LoadingState rows={5} />
        ) : isError ? (
          <ErrorState onRetry={() => void refetch()} retrying={isFetching} />
        ) : orders.length === 0 ? (
          <EmptyState icon={FlaskConical} title="No lab orders" description={statusFilter === "all" ? "Create a lab order to get started." : `No orders in the '${statusFilter}' stage.`} />
        ) : (
          <>
            <PageToolbar>
              <Select value={statusFilter} onValueChange={setStatusFilter}>
                <SelectTrigger className="w-[240px]"><SelectValue /></SelectTrigger>
                <SelectContent>
                  {STATUS_FILTERS.map((f) => <SelectItem key={f.value} value={f.value}>{f.label}</SelectItem>)}
                </SelectContent>
              </Select>
              <span className="text-xs text-muted-foreground ml-auto">
                {allOrders.filter((o) => o.status === "ordered").length} to collect · {allOrders.filter((o) => o.status === "resulted").length} to approve
              </span>
            </PageToolbar>
            <Table>
              <TableHeader>
                <TableRow className="border-border hover:bg-transparent">
                  <TableHead>Order #</TableHead>
                  <TableHead>Patient</TableHead>
                  <TableHead>Ordered by</TableHead>
                  <TableHead>Ordered at</TableHead>
                  <TableHead>Barcode</TableHead>
                  <TableHead className="text-right">Status</TableHead>
                  <TableHead className="text-right">Action</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {orders.map((o) => (
                  <TableRow key={o.id}>
                    <TableCell className="font-mono">#{o.id}</TableCell>
                    <TableCell className="font-medium">{o.patient_name ?? "—"}</TableCell>
                    <TableCell className="text-muted-foreground">{o.doctor_name ?? "—"}</TableCell>
                    <TableCell className="text-xs text-muted-foreground">{new Date(o.ordered_at).toLocaleString()}</TableCell>
                    <TableCell className="font-mono text-xs">{o.sample_barcode ?? "—"}</TableCell>
                    <TableCell className="text-right"><StatusBadge status={o.status} /></TableCell>
                    <TableCell className="text-right">
                      {o.status === "ordered" && has(PERMISSIONS.LabResultManage) ? (
                        <Button size="sm" variant="outline" disabled={collect.isPending} onClick={() => doCollect(o.id)}>
                          <Syringe className="h-3.5 w-3.5" /> Collect sample
                        </Button>
                      ) : (
                        <Button size="sm" variant="ghost" onClick={() => setResultOrderId(o.id)}>
                          {["resulted", "approved"].includes(o.status) && has(PERMISSIONS.LabApprove) ? "Review / approve" : "View"}
                        </Button>
                      )}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </>
        )}
      </SectionCard>

      {/* New order dialog */}
      <Dialog open={orderOpen} onOpenChange={setOrderOpen}>
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle>New lab order</DialogTitle>
            <DialogDescription>Select a patient and one or more catalog tests, then submit to place the order.</DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-2">
            <div className="space-y-1.5">
              <Label>Patient</Label>
              <Select value={form.patientId?.toString() ?? ""} onValueChange={(v) => setForm({ ...form, patientId: Number(v) })}>
                <SelectTrigger><SelectValue placeholder="Select patient" /></SelectTrigger>
                <SelectContent>{patients.map((p) => <SelectItem key={p.id} value={p.id.toString()}>{patientDescriptor(p)}</SelectItem>)}</SelectContent>
              </Select>
            </div>
            <div className="space-y-1.5">
              <Label>Ordering doctor (optional)</Label>
              <Select value={form.doctorId?.toString() ?? "none"} onValueChange={(v) => setForm({ ...form, doctorId: v === "none" ? null : Number(v) })}>
                <SelectTrigger><SelectValue /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="none">—</SelectItem>
                  {doctors.filter((d) => d.is_active).map((d) => <SelectItem key={d.id} value={d.id.toString()}>Dr. {d.first_name} {d.last_name}</SelectItem>)}
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-1.5">
              <Label>Tests</Label>
              <div className="border border-border rounded-lg max-h-56 overflow-y-auto divide-y">
                {catalog.map((t) => (
                  <label key={t.id} className="flex items-center gap-3 p-2.5 cursor-pointer hover:bg-muted/50">
                    <input type="checkbox" checked={form.testIds.includes(t.id)} onChange={() => toggleTest(t.id)} className="h-4 w-4 accent-primary" />
                    <div className="flex-1 min-w-0">
                      <div className="text-sm font-medium truncate">{t.name}</div>
                      <div className="text-[10px] text-muted-foreground">{t.code} · {t.category ?? "—"}</div>
                    </div>
                    <span className="text-xs text-muted-foreground">{formatMoney(t.price)}</span>
                  </label>
                ))}
              </div>
            </div>
          </div>
          <DialogFooter>
            <DialogClose asChild><Button variant="outline">Cancel</Button></DialogClose>
            <Button disabled={!form.patientId || form.testIds.length === 0 || createOrder.isPending} onClick={submit}>
              {createOrder.isPending ? "Placing…" : `Order ${form.testIds.length} test(s)`}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Sample-collected barcode dialog */}
      <Dialog open={barcodeOrder != null} onOpenChange={(o) => !o && setBarcodeOrder(null)}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle>Sample collected</DialogTitle>
            <DialogDescription>Label the sample tube(s) with this barcode. The order now awaits result entry.</DialogDescription>
          </DialogHeader>
          <div className="py-4 text-center">
            <div className="font-mono text-2xl font-bold tracking-widest border border-dashed border-border rounded-lg py-4 px-2 select-all">
              {barcodeOrder?.barcode}
            </div>
          </div>
          <DialogFooter>
            <DialogClose asChild><Button>Done</Button></DialogClose>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {resultOrderId != null && (
        <ResultsDialog
          orderId={resultOrderId}
          onClose={() => setResultOrderId(null)}
          canEdit={has(PERMISSIONS.LabResultManage)}
          canApprove={has(PERMISSIONS.LabApprove)}
        />
      )}
    </PageContainer>
  );
}

function ResultsDialog({ orderId, onClose, canEdit, canApprove }: { orderId: number; onClose: () => void; canEdit: boolean; canApprove: boolean }) {
  const { data: tests = [], isLoading } = useLabOrderTests(orderId);
  const update = useUpdateLabResult();
  const approve = useApproveLabResult();
  const [drafts, setDrafts] = useState<Record<number, { value: string; flag: string; notes: string }>>({});
  // Which test row the critical-acknowledgment confirm is open for.
  const [criticalApproveId, setCriticalApproveId] = useState<number | null>(null);

  const getDraft = (id: number) => drafts[id] ?? { value: "", flag: "normal", notes: "" };
  const setDraft = (id: number, patch: Partial<{ value: string; flag: string; notes: string }>) =>
    setDrafts((d) => ({ ...d, [id]: { ...getDraft(id), ...patch } }));

  const save = async (testId: number) => {
    const d = getDraft(testId);
    await update.mutateAsync({ id: testId, result_value: d.value || null, result_abnormal_flag: d.flag || null, result_notes: d.notes || null });
  };

  const doApprove = async (testId: number, acknowledged: boolean) => {
    await approve.mutateAsync({ labOrderTestId: testId, criticalAcknowledged: acknowledged });
    setCriticalApproveId(null);
  };

  const criticalRows = tests.filter((t) => t.result_abnormal_flag === "critical" && t.approval_status !== "approved");
  const criticalTest = tests.find((t) => t.id === criticalApproveId);

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>Lab order #{orderId} — results & approval</DialogTitle>
          <DialogDescription>Enter results, then approve each one to release it. Critical results require acknowledgment that the doctor was contacted.</DialogDescription>
        </DialogHeader>
        {isLoading ? (
          <LoadingState rows={4} />
        ) : (
          <div className="space-y-3 max-h-[60vh] overflow-y-auto">
            {/* Critical-value alert banner */}
            {criticalRows.length > 0 && (
              <div className="flex items-start gap-3 rounded-[var(--radius-md)] border border-destructive/50 bg-destructive/10 p-3 text-sm">
                <AlertTriangle className="h-5 w-5 text-destructive shrink-0 mt-0.5" />
                <div>
                  <div className="font-semibold text-destructive">
                    {criticalRows.length} CRITICAL value{criticalRows.length > 1 ? "s" : ""} — phone the ordering doctor NOW.
                  </div>
                  <div className="text-xs text-muted-foreground mt-0.5">
                    {criticalRows.map((t) => `${t.test_name}: ${t.result_value}`).join(" · ")}
                  </div>
                </div>
              </div>
            )}
            {tests.map((t) => {
              const done = !!t.completed_at;
              const entered = t.approval_status === "entered" || t.approval_status === "amended";
              const approved = t.approval_status === "approved";
              return (
                <div key={t.id} className={`border rounded-lg p-3 space-y-2 ${t.result_abnormal_flag === "critical" && !approved ? "border-destructive/50" : "border-border"}`}>
                  <div className="flex items-center justify-between">
                    <div>
                      <div className="text-sm font-semibold">{t.test_name} <span className="text-[10px] text-muted-foreground font-normal">({t.test_code})</span></div>
                      <div className="text-[10px] text-muted-foreground">Normal range: {t.normal_range ?? "—"}</div>
                    </div>
                    <div className="flex items-center gap-2">
                      {approved && <span className="text-[10px] font-bold uppercase text-success flex items-center gap-1"><ShieldCheck className="h-3.5 w-3.5" /> Released</span>}
                      {done && !approved && <AlertTriangle className={`h-4 w-4 ${t.result_abnormal_flag === "critical" ? "text-destructive" : "text-warning"}`} />}
                      {!done && <span className="text-[10px] text-muted-foreground uppercase font-bold">Pending</span>}
                    </div>
                  </div>
                  {done ? (
                    <>
                      <div className="text-xs space-y-0.5 text-muted-foreground">
                        <div>Result: <span className="font-medium text-foreground">{t.result_value} {t.result_unit ?? ""}</span></div>
                        {t.result_notes && <div>Notes: {t.result_notes}</div>}
                      </div>
                      {canApprove && entered ? (
                        <Button
                          size="sm"
                          variant="outline"
                          disabled={approve.isPending}
                          onClick={() => {
                            if (t.result_abnormal_flag === "critical") setCriticalApproveId(t.id);
                            else void doApprove(t.id, false);
                          }}
                        >
                          {approve.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : <ShieldCheck className="h-3.5 w-3.5" />} Approve & release
                        </Button>
                      ) : canEdit && approved && (
                        <span className="text-[10px] text-muted-foreground">Released — re-entering a value creates an amendment requiring re-approval.</span>
                      )}
                    </>
                  ) : canEdit ? (
                    <div className="grid grid-cols-12 gap-2">
                      <Input className="col-span-5" placeholder="Result value" value={getDraft(t.id).value} onChange={(e) => setDraft(t.id, { value: e.target.value })} />
                      <Select value={getDraft(t.id).flag} onValueChange={(v) => setDraft(t.id, { flag: v })}>
                        <SelectTrigger className="col-span-3"><SelectValue /></SelectTrigger>
                        <SelectContent>
                          <SelectItem value="normal">Normal</SelectItem>
                          <SelectItem value="high">High</SelectItem>
                          <SelectItem value="low">Low</SelectItem>
                          <SelectItem value="critical">Critical</SelectItem>
                        </SelectContent>
                      </Select>
                      <Input className="col-span-4" placeholder="Notes" value={getDraft(t.id).notes} onChange={(e) => setDraft(t.id, { notes: e.target.value })} />
                      <Button size="sm" className="col-span-12" disabled={update.isPending} onClick={() => save(t.id)}>
                        {update.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : "Save result"}
                      </Button>
                    </div>
                  ) : (
                    <div className="text-xs text-muted-foreground">Pending — result not yet entered.</div>
                  )}
                </div>
              );
            })}
          </div>
        )}
        <DialogFooter>
          <DialogClose asChild><Button variant="outline">Close</Button></DialogClose>
        </DialogFooter>
      </DialogContent>

      {/* Critical-acknowledgment confirm (nested on purpose: a deliberate,
          blocking step — the approver confirms the doctor was phoned). */}
      <Dialog open={criticalApproveId != null} onOpenChange={(o) => !o && setCriticalApproveId(null)}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle className="text-destructive flex items-center gap-2">
              <AlertTriangle className="h-5 w-5" /> Confirm critical-value escalation
            </DialogTitle>
            <DialogDescription>
              {criticalTest?.test_name} returned <strong className="text-foreground">{criticalTest?.result_value}</strong> — flagged CRITICAL.
              Releasing this result requires confirming the ordering doctor has been contacted by phone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <DialogClose asChild><Button variant="outline">Not yet</Button></DialogClose>
            <Button
              variant="destructive"
              disabled={approve.isPending}
              onClick={() => criticalApproveId != null && doApprove(criticalApproveId, true)}
            >
              {approve.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : <CheckCircle2 className="h-4 w-4" />}
              Doctor contacted — release
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </Dialog>
  );
}
