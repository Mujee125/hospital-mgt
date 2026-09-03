/**
 * Laboratory — uses shared layout components.
 */
import { useState } from "react";
import { FlaskConical, Plus, Loader2, CheckCircle2, AlertTriangle } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter, DialogClose } from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useLabOrders, useLabCatalog, useCreateLabOrder, useLabOrderTests, useUpdateLabResult, usePatientsEhr, useDoctors } from "@/lib/queries";
import { useAuth } from "@/lib/auth";
import { PERMISSIONS } from "@/lib/rbac";
import { formatMoney } from "@/lib/utils";
import { PageContainer, PageHeader, SectionCard, EmptyState, StatusBadge, LoadingState, PageToolbar } from "@/components/layout/shared";

export function Laboratory() {
  const { has } = useAuth();
  const { data: orders = [], isLoading } = useLabOrders();
  const { data: catalog = [] } = useLabCatalog();
  const { data: patients = [] } = usePatientsEhr();
  const { data: doctors = [] } = useDoctors();
  const createOrder = useCreateLabOrder();

  const [orderOpen, setOrderOpen] = useState(false);
  const [resultOrderId, setResultOrderId] = useState<number | null>(null);
  const [form, setForm] = useState({ patientId: null as number | null, doctorId: null as number | null, testIds: [] as number[] });

  const submit = async () => {
    if (!form.patientId || form.testIds.length === 0) return;
    await createOrder.mutateAsync({ patient_id: form.patientId, ordered_by_doctor_id: form.doctorId, test_catalog_ids: form.testIds });
    setOrderOpen(false);
    setForm({ patientId: null, doctorId: null, testIds: [] });
  };

  const toggleTest = (id: number) =>
    setForm((f) => ({ ...f, testIds: f.testIds.includes(id) ? f.testIds.filter((t) => t !== id) : [...f.testIds, id] }));

  return (
    <PageContainer>
      <PageHeader
        icon={FlaskConical}
        title="Laboratory"
        description="Test orders & results"
        actions={has(PERMISSIONS.LabOrder) && (
          <Button onClick={() => setOrderOpen(true)}><Plus className="h-4 w-4" /> New lab order</Button>
        )}
      />

      <SectionCard>
        {isLoading ? (
          <LoadingState rows={5} />
        ) : orders.length === 0 ? (
          <EmptyState icon={FlaskConical} title="No lab orders" description="Create a lab order to get started." />
        ) : (
          <>
            <PageToolbar>
              <span className="text-sm font-medium text-muted-foreground">{orders.length} total orders</span>
              <span className="text-xs text-muted-foreground ml-auto">
                {orders.filter((o) => o.status === "ordered").length} pending · {orders.filter((o) => o.status === "completed").length} completed
              </span>
            </PageToolbar>
            <Table>
              <TableHeader>
                <TableRow className="border-border hover:bg-transparent">
                  <TableHead>Order #</TableHead>
                  <TableHead>Patient</TableHead>
                  <TableHead>Ordered by</TableHead>
                  <TableHead>Ordered at</TableHead>
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
                    <TableCell className="text-right"><StatusBadge status={o.status} /></TableCell>
                    <TableCell className="text-right">
                      <Button size="sm" variant="ghost" onClick={() => setResultOrderId(o.id)}>
                        {o.status === "completed" ? "View" : has(PERMISSIONS.LabResultManage) ? "Enter results" : "View"}
                      </Button>
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
                <SelectContent>{patients.map((p) => <SelectItem key={p.id} value={p.id.toString()}>{p.first_name} {p.last_name} · {p.phone}</SelectItem>)}</SelectContent>
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

      {resultOrderId != null && (
        <ResultsDialog orderId={resultOrderId} onClose={() => setResultOrderId(null)} canEdit={has(PERMISSIONS.LabResultManage)} />
      )}
    </PageContainer>
  );
}

function ResultsDialog({ orderId, onClose, canEdit }: { orderId: number; onClose: () => void; canEdit: boolean }) {
  const { data: tests = [], isLoading } = useLabOrderTests(orderId);
  const update = useUpdateLabResult();
  const [drafts, setDrafts] = useState<Record<number, { value: string; flag: string; notes: string }>>({});

  const getDraft = (id: number) => drafts[id] ?? { value: "", flag: "normal", notes: "" };
  const setDraft = (id: number, patch: Partial<{ value: string; flag: string; notes: string }>) =>
    setDrafts((d) => ({ ...d, [id]: { ...getDraft(id), ...patch } }));

  const save = async (testId: number) => {
    const d = getDraft(testId);
    await update.mutateAsync({ id: testId, result_value: d.value || null, result_abnormal_flag: d.flag || null, result_notes: d.notes || null });
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>Lab order #{orderId} — results</DialogTitle>
          <DialogDescription>Enter or review the result value, abnormal flag, and notes for each test in this order.</DialogDescription>
        </DialogHeader>
        {isLoading ? (
          <LoadingState rows={4} />
        ) : (
          <div className="space-y-3 max-h-[60vh] overflow-y-auto">
            {tests.map((t) => {
              const done = !!t.completed_at;
              return (
                <div key={t.id} className="border border-border rounded-lg p-3 space-y-2">
                  <div className="flex items-center justify-between">
                    <div>
                      <div className="text-sm font-semibold">{t.test_name} <span className="text-[10px] text-muted-foreground font-normal">({t.test_code})</span></div>
                      <div className="text-[10px] text-muted-foreground">Normal range: {t.normal_range ?? "—"}</div>
                    </div>
                    {done ? <CheckCircle2 className="h-4 w-4 text-success" /> : <AlertTriangle className="h-4 w-4 text-warning" />}
                  </div>
                  {done ? (
                    <div className="text-xs space-y-0.5 text-muted-foreground">
                      <div>Result: <span className="font-medium text-foreground">{t.result_value} {t.result_unit ?? ""}</span></div>
                      {t.result_notes && <div>Notes: {t.result_notes}</div>}
                    </div>
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
    </Dialog>
  );
}
