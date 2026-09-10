/**
 * Billing & invoicing (SRS §2.10 — Phase 6.3).
 *
 * Three tabs:
 *   Invoices — list + create (with >5% discount manager-approval flow) +
 *              detail (line items, payments, refunds, advance apply,
 *              credit-note cancellation, claim creation)
 *   Advances  — patient deposits: receive + track remaining balances
 *   Claims    — insurance/TPA pipeline: draft → submitted → approved /
 *              partially approved / rejected → settled
 *
 * Financial guards (refund over-payment, cancelled-bill payments, discount
 * threshold) are enforced server-side; the UI only surfaces them.
 */
import { useState } from "react";
import { Receipt, Plus, Loader2, DollarSign, Trash2, Undo2, Ban, ShieldCheck, Wallet, FileText, Landmark, TrendingDown } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter, DialogClose } from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  useBills, useBillItems, usePayments, useCreateBill, useRecordPayment,
  useRefunds, useRecordRefund, useCancelBill,
  usePatientAdvances, useRecordAdvance, useApplyAdvance,
  useInsuranceClaims, useCreateInsuranceClaim, useUpdateInsuranceClaimStatus,
  usePatientsEhr, useExpenses, useCreateExpense, useVoidExpense, useAccountsSummary,
} from "@/lib/queries";
import type { Expense } from "@/lib/models";
import { useAuth } from "@/lib/auth";
import { PERMISSIONS } from "@/lib/rbac";
import { formatMoney, patientDescriptor } from "@/lib/utils";
import { PageContainer, PageHeader, SectionCard, EmptyState, ErrorState, StatusBadge, LoadingState, PageToolbar } from "@/components/layout/shared";

export function Billing() {
  const { has } = useAuth();
  return (
    <PageContainer>
      <PageHeader
        icon={Receipt}
        title="Billing & Invoices"
        description="Invoices, payments, refunds, advances & insurance claims"
      />
      <div className="space-y-4">
        <Tabs defaultValue="invoices">
          <TabsList>
            <TabsTrigger value="invoices"><Receipt className="h-4 w-4 mr-1.5" /> Invoices</TabsTrigger>
            <TabsTrigger value="advances"><Wallet className="h-4 w-4 mr-1.5" /> Advances</TabsTrigger>
            <TabsTrigger value="claims"><Landmark className="h-4 w-4 mr-1.5" /> Insurance claims</TabsTrigger>
            <TabsTrigger value="expenses"><TrendingDown className="h-4 w-4 mr-1.5" /> Expenses</TabsTrigger>
          </TabsList>
          <TabsContent value="invoices" className="pt-4">
            <InvoicesTab canApprove={has(PERMISSIONS.BillingApprove)} />
          </TabsContent>
          <TabsContent value="advances" className="pt-4">
            <AdvancesTab />
          </TabsContent>
          <TabsContent value="claims" className="pt-4">
            <ClaimsTab />
          </TabsContent>
          <TabsContent value="expenses" className="pt-4">
            <ExpensesTab canManage={has(PERMISSIONS.BillingManage)} canApprove={has(PERMISSIONS.BillingApprove)} />
          </TabsContent>
        </Tabs>
      </div>
    </PageContainer>
  );
}

// ── Invoices tab ──────────────────────────────────────────────────────────────

function InvoicesTab({ canApprove }: { canApprove: boolean }) {
  const { has } = useAuth();
  const { data: bills = [], isLoading, isError, refetch, isFetching } = useBills();
  const { data: patients = [] } = usePatientsEhr();
  const createBill = useCreateBill();

  const [createOpen, setCreateOpen] = useState(false);
  const [detailId, setDetailId] = useState<number | null>(null);
  const [patientId, setPatientId] = useState<number | null>(null);
  const [items, setItems] = useState([{ item_type: "consultation", description: "", quantity: "1", unit_price: "0" }]);
  const [discount, setDiscount] = useState("0");
  const [tax, setTax] = useState("0");
  const [discountApproved, setDiscountApproved] = useState(false);

  const total = items.reduce((s, it) => s + (parseFloat(it.quantity) || 0) * (parseFloat(it.unit_price) || 0), 0);
  const net = Math.max(0, total - (parseFloat(discount) || 0) + (parseFloat(tax) || 0));
  const needsDiscountApproval = (parseFloat(discount) || 0) > total * 0.05 && total > 0;

  const submit = async () => {
    if (!patientId || items.length === 0) return;
    await createBill.mutateAsync({
      patient_id: patientId, discount: parseFloat(discount) || 0, tax: parseFloat(tax) || 0,
      discountApproved,
      items: items.map((it) => ({ item_type: it.item_type, description: it.description, quantity: parseFloat(it.quantity) || 0, unit_price: parseFloat(it.unit_price) || 0 })),
    });
    setCreateOpen(false);
    setPatientId(null);
    setItems([{ item_type: "consultation", description: "", quantity: "1", unit_price: "0" }]);
    setDiscount("0"); setTax("0"); setDiscountApproved(false);
  };

  return (
    <SectionCard>
      {isLoading ? (
        <LoadingState rows={5} />
      ) : isError ? (
        <ErrorState onRetry={() => void refetch()} retrying={isFetching} />
      ) : bills.length === 0 ? (
        <EmptyState icon={Receipt} title="No invoices" description="Create an invoice to get started." />
      ) : (
        <>
          <PageToolbar>
            <span className="text-sm font-medium text-muted-foreground">{bills.length} total invoices</span>
            {has(PERMISSIONS.BillingCreate) && (
              <Button className="ml-auto" onClick={() => setCreateOpen(true)}><Plus className="h-4 w-4" /> New invoice</Button>
            )}
          </PageToolbar>
          <Table>
            <TableHeader>
              <TableRow className="border-border hover:bg-transparent">
                <TableHead>Invoice #</TableHead>
                <TableHead>Patient</TableHead>
                <TableHead className="text-right">Net amount</TableHead>
                <TableHead className="text-right">Paid</TableHead>
                <TableHead className="text-right">Refunded</TableHead>
                <TableHead className="text-right">Status</TableHead>
                <TableHead className="text-right">Action</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {bills.map((b) => (
                <TableRow key={b.id}>
                  <TableCell className="font-mono text-xs">
                    {b.bill_number}
                    {b.credit_note_number && (
                      <div className="font-mono text-[10px] text-destructive">CN: {b.credit_note_number}</div>
                    )}
                  </TableCell>
                  <TableCell className="font-medium">{b.patient_name ?? "—"}</TableCell>
                  <TableCell className="text-right font-medium">{formatMoney(b.net_amount)}</TableCell>
                  <TableCell className="text-right text-muted-foreground">{formatMoney(b.amount_paid ?? 0)}</TableCell>
                  <TableCell className="text-right text-muted-foreground">{formatMoney(b.refund_total ?? 0)}</TableCell>
                  <TableCell className="text-right"><StatusBadge status={b.status} /></TableCell>
                  <TableCell className="text-right">
                    <Button size="sm" variant="ghost" onClick={() => setDetailId(b.id)}>View</Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </>
      )}

      {/* Create invoice dialog */}
      <Dialog open={createOpen} onOpenChange={setCreateOpen}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle>New invoice</DialogTitle>
            <DialogDescription>Create a new invoice for a patient. Add line items, discount, and tax, then submit to record the bill.</DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-2">
            <div className="space-y-1.5">
              <Label>Patient</Label>
              <Select value={patientId?.toString() ?? ""} onValueChange={(v) => setPatientId(Number(v))}>
                <SelectTrigger><SelectValue placeholder="Select patient" /></SelectTrigger>
                <SelectContent>{patients.map((p) => <SelectItem key={p.id} value={p.id.toString()}>{patientDescriptor(p)}</SelectItem>)}</SelectContent>
              </Select>
            </div>
            <div className="space-y-2">
              <Label>Line items</Label>
              <div className="space-y-2">
                {items.map((it, i) => (
                  <div key={i} className="grid grid-cols-12 gap-2 items-center">
                    <Select value={it.item_type} onValueChange={(v) => setItems((arr) => arr.map((x, j) => j === i ? { ...x, item_type: v } : x))}>
                      <SelectTrigger className="col-span-3"><SelectValue /></SelectTrigger>
                      <SelectContent>
                        {["consultation", "lab", "pharmacy", "procedure", "room", "other"].map((t) => <SelectItem key={t} value={t} className="capitalize">{t}</SelectItem>)}
                      </SelectContent>
                    </Select>
                    <Input className="col-span-4" placeholder="Description" value={it.description} onChange={(e) => setItems((arr) => arr.map((x, j) => j === i ? { ...x, description: e.target.value } : x))} />
                    <Input className="col-span-2" type="number" placeholder="Qty" value={it.quantity} onChange={(e) => setItems((arr) => arr.map((x, j) => j === i ? { ...x, quantity: e.target.value } : x))} />
                    <Input className="col-span-2" type="number" placeholder="Unit price" value={it.unit_price} onChange={(e) => setItems((arr) => arr.map((x, j) => j === i ? { ...x, unit_price: e.target.value } : x))} />
                    <Button variant="ghost" size="icon" aria-label="Remove line item" className="col-span-1 h-9 w-9 text-muted-foreground hover:text-destructive" disabled={items.length === 1} onClick={() => setItems((arr) => arr.filter((_, j) => j !== i))}>
                      <Trash2 className="h-4 w-4" />
                    </Button>
                  </div>
                ))}
              </div>
              <Button variant="outline" size="sm" className="gap-2" onClick={() => setItems((arr) => [...arr, { item_type: "other", description: "", quantity: "1", unit_price: "0" }])}>
                <Plus className="h-3.5 w-3.5" /> Add line
              </Button>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1.5">
                <Label>Discount</Label>
                <Input type="number" value={discount} onChange={(e) => { setDiscount(e.target.value); setDiscountApproved(false); }} />
              </div>
              <div className="space-y-1.5">
                <Label>Tax</Label>
                <Input type="number" value={tax} onChange={(e) => setTax(e.target.value)} />
              </div>
            </div>
            {needsDiscountApproval && (
              <div className="flex items-start gap-2 rounded-[var(--radius-md)] border border-warning/40 bg-warning/10 p-3 text-xs">
                <ShieldCheck className="h-4 w-4 text-warning shrink-0 mt-0.5" />
                <div>
                  <div className="font-semibold">This discount exceeds 5% and needs manager approval.</div>
                  {canApprove ? (
                    <label className="flex items-center gap-2 mt-1.5 cursor-pointer">
                      <input type="checkbox" checked={discountApproved} onChange={(e) => setDiscountApproved(e.target.checked)} className="h-4 w-4 accent-primary" />
                      I approve this discount (manager).
                    </label>
                  ) : (
                    <div className="text-muted-foreground mt-1">A manager (billing.approve) must approve this — ask your administrator.</div>
                  )}
                </div>
              </div>
            )}
            <div className="flex items-center justify-between border-t border-border pt-3 text-sm">
              <span className="text-muted-foreground">Subtotal: {formatMoney(total)} · Net: </span>
              <span className="text-display-md text-foreground">{formatMoney(net)}</span>
            </div>
          </div>
          <DialogFooter>
            <DialogClose asChild><Button variant="outline">Cancel</Button></DialogClose>
            <Button
              disabled={!patientId || createBill.isPending || (needsDiscountApproval && (!canApprove || !discountApproved))}
              onClick={submit}
            >
              {createBill.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : "Create invoice"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {detailId != null && (
        <BillDetail
          id={detailId}
          onClose={() => setDetailId(null)}
          canPay={has(PERMISSIONS.PaymentsManage)}
          canApprove={canApprove}
        />
      )}
    </SectionCard>
  );
}

function BillDetail({ id, onClose, canPay, canApprove }: { id: number; onClose: () => void; canPay: boolean; canApprove: boolean }) {
  const { data: items = [] } = useBillItems(id);
  const { data: payments = [] } = usePayments(id);
  const { data: refunds = [] } = useRefunds(id);
  const { data: bills = [] } = useBills();
  const bill = bills.find((b) => b.id === id) ?? null;
  const { data: advances = [] } = usePatientAdvances(bill?.patient_id ?? null);
  const record = useRecordPayment();
  const refund = useRecordRefund();
  const apply = useApplyAdvance();
  const cancel = useCancelBill();
  const createClaim = useCreateInsuranceClaim();
  const [amount, setAmount] = useState("");
  const [method, setMethod] = useState("cash");
  const [refundAmount, setRefundAmount] = useState("");
  const [refundReason, setRefundReason] = useState("");
  const [cancelOpen, setCancelOpen] = useState(false);
  const [cancelReason, setCancelReason] = useState("");
  const [applyAdvanceId, setApplyAdvanceId] = useState<string>("");
  const [applyAmount, setApplyAmount] = useState("");
  const [claimOpen, setClaimOpen] = useState(false);
  const [claimInsurer, setClaimInsurer] = useState("");
  const [claimPolicy, setClaimPolicy] = useState("");
  const [claimAmount, setClaimAmount] = useState("");

  const paid = payments.reduce((s, p) => s + (p.amount as number), 0) - refunds.reduce((s, r) => s + (r.amount as number), 0);
  const cancelled = bill?.status === "cancelled";
  const activeAdvances = advances.filter((a) => a.status === "active" && a.remaining > 0);

  const pay = async () => {
    await record.mutateAsync({ bill_id: id, amount: parseFloat(amount) || 0, payment_method: method });
    setAmount("");
  };
  const doRefund = async () => {
    await refund.mutateAsync({ bill_id: id, amount: parseFloat(refundAmount) || 0, reason: refundReason });
    setRefundAmount(""); setRefundReason("");
  };
  const doApply = async () => {
    if (!applyAdvanceId) return;
    await apply.mutateAsync({
      advanceId: Number(applyAdvanceId),
      billId: id,
      amount: parseFloat(applyAmount) || 0,
    });
    setApplyAdvanceId(""); setApplyAmount("");
  };
  const doCancel = async () => {
    await cancel.mutateAsync({ billId: id, reason: cancelReason });
    setCancelOpen(false); setCancelReason("");
  };
  const doClaim = async () => {
    await createClaim.mutateAsync({
      bill_id: id,
      insurer: claimInsurer,
      policy_number: claimPolicy || null,
      claim_amount: parseFloat(claimAmount) || 0,
    });
    setClaimOpen(false); setClaimInsurer(""); setClaimPolicy(""); setClaimAmount("");
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>{bill ? bill.bill_number : `Invoice #${id}`} — details</DialogTitle>
          <DialogDescription>
            Line items, payments, refunds and financial actions.
            {cancelled && " This invoice is CANCELLED — payments are closed."}
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-4 py-2 max-h-[60vh] overflow-y-auto">
          <div>
            <h4 className="text-xs uppercase font-semibold text-muted-foreground mb-2">Line items</h4>
            <Table>
              <TableHeader><TableRow className="border-border hover:bg-transparent"><TableHead>Description</TableHead><TableHead className="text-right">Qty</TableHead><TableHead className="text-right">Unit</TableHead><TableHead className="text-right">Total</TableHead></TableRow></TableHeader>
              <TableBody>
                {items.map((it) => (
                  <TableRow key={it.id}>
                    <TableCell className="capitalize">{it.item_type} — {it.description}</TableCell>
                    <TableCell className="text-right">{it.quantity}</TableCell>
                    <TableCell className="text-right">{formatMoney(it.unit_price)}</TableCell>
                    <TableCell className="text-right font-medium">{formatMoney(it.total)}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
          <div>
            <h4 className="text-xs uppercase font-semibold text-muted-foreground mb-2">
              Payments ({formatMoney(paid)} effective received)
            </h4>
            {payments.length === 0 ? <p className="text-xs text-muted-foreground">No payments recorded.</p> : (
              <Table>
                <TableHeader><TableRow className="border-border hover:bg-transparent"><TableHead>Method</TableHead><TableHead>Reference</TableHead><TableHead className="text-right">Amount</TableHead><TableHead>Date</TableHead></TableRow></TableHeader>
                <TableBody>
                  {payments.map((p) => (
                    <TableRow key={p.id}>
                      <TableCell className="capitalize">{p.payment_method}</TableCell>
                      <TableCell className="text-xs text-muted-foreground">{p.reference_number ?? "—"}</TableCell>
                      <TableCell className="text-right font-medium">{formatMoney(p.amount)}</TableCell>
                      <TableCell className="text-xs text-muted-foreground">{new Date(p.paid_at).toLocaleString()}</TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            )}
          </div>
          {refunds.length > 0 && (
            <div>
              <h4 className="text-xs uppercase font-semibold text-muted-foreground mb-2">Refunds</h4>
              <Table>
                <TableHeader><TableRow className="border-border hover:bg-transparent"><TableHead>Reason</TableHead><TableHead className="text-right">Amount</TableHead><TableHead>Date</TableHead></TableRow></TableHeader>
                <TableBody>
                  {refunds.map((r) => (
                    <TableRow key={r.id}>
                      <TableCell>{r.reason}</TableCell>
                      <TableCell className="text-right font-medium text-destructive">−{formatMoney(r.amount)}</TableCell>
                      <TableCell className="text-xs text-muted-foreground">{new Date(r.refunded_at).toLocaleString()}</TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>
          )}

          {/* Payment / refund / advance / cancel actions */}
          {!cancelled && canPay && (
            <div className="border border-border rounded-lg p-3 space-y-3">
              <h4 className="text-xs uppercase font-semibold text-muted-foreground">Record payment</h4>
              <div className="grid grid-cols-12 gap-2">
                <Input className="col-span-4" type="number" placeholder="Amount" value={amount} onChange={(e) => setAmount(e.target.value)} />
                <Select value={method} onValueChange={setMethod}>
                  <SelectTrigger className="col-span-4"><SelectValue /></SelectTrigger>
                  <SelectContent>{["cash", "card", "insurance", "online"].map((m) => <SelectItem key={m} value={m} className="capitalize">{m}</SelectItem>)}</SelectContent>
                </Select>
                <Button className="col-span-4" disabled={record.isPending || !amount} onClick={pay}>
                  {record.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : <DollarSign className="h-4 w-4" />} Record
                </Button>
              </div>
              {activeAdvances.length > 0 && (
                <div className="grid grid-cols-12 gap-2 items-end">
                  <div className="col-span-5 space-y-1">
                    <Label className="text-xs">Apply advance</Label>
                    <Select value={applyAdvanceId} onValueChange={setApplyAdvanceId}>
                      <SelectTrigger><SelectValue placeholder="Advance" /></SelectTrigger>
                      <SelectContent>
                        {activeAdvances.map((a) => (
                          <SelectItem key={a.id} value={a.id.toString()}>
                            #{a.id} · balance {formatMoney(a.remaining)}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                  <Input className="col-span-3" type="number" placeholder="Amount" value={applyAmount} onChange={(e) => setApplyAmount(e.target.value)} />
                  <Button className="col-span-4" variant="outline" disabled={!applyAdvanceId || apply.isPending} onClick={doApply}>
                    <Wallet className="h-4 w-4" /> Apply
                  </Button>
                </div>
              )}
            </div>
          )}
          {!cancelled && canApprove && (
            <div className="border border-border rounded-lg p-3 space-y-3">
              <h4 className="text-xs uppercase font-semibold text-muted-foreground">Refund (manager)</h4>
              <div className="grid grid-cols-12 gap-2">
                <Input className="col-span-3" type="number" placeholder="Amount" value={refundAmount} onChange={(e) => setRefundAmount(e.target.value)} />
                <Input className="col-span-6" placeholder="Reason (required)" value={refundReason} onChange={(e) => setRefundReason(e.target.value)} />
                <Button className="col-span-3" variant="outline" disabled={refund.isPending || !refundAmount || !refundReason} onClick={doRefund}>
                  {refund.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : <Undo2 className="h-4 w-4" />} Refund
                </Button>
              </div>
              <Button variant="ghost" size="sm" className="text-destructive hover:text-destructive" onClick={() => setCancelOpen(true)}>
                <Ban className="h-4 w-4" /> Cancel invoice (credit note)…
              </Button>
            </div>
          )}
          {!cancelled && (
            <Button variant="outline" size="sm" onClick={() => setClaimOpen(true)}>
              <Landmark className="h-4 w-4" /> Create insurance claim
            </Button>
          )}
        </div>
        <DialogFooter>
          <DialogClose asChild><Button variant="outline">Close</Button></DialogClose>
        </DialogFooter>
      </DialogContent>

      {/* Cancel confirm */}
      <Dialog open={cancelOpen} onOpenChange={setCancelOpen}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle className="text-destructive">Cancel invoice — issue credit note?</DialogTitle>
            <DialogDescription>
              The invoice rows are preserved for audit, but it is marked cancelled and closed to payments. A sequential credit note number is issued.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-2 py-2">
            <Label htmlFor="cancel-reason">Reason (required)</Label>
            <Textarea id="cancel-reason" rows={3} value={cancelReason} onChange={(e) => setCancelReason(e.target.value)} placeholder="e.g. billed in error, duplicate invoice…" />
          </div>
          <DialogFooter>
            <DialogClose asChild><Button variant="outline">Keep invoice</Button></DialogClose>
            <Button variant="destructive" disabled={!cancelReason.trim() || cancel.isPending} onClick={doCancel}>
              {cancel.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : "Cancel invoice"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Claim creation */}
      <Dialog open={claimOpen} onOpenChange={setClaimOpen}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>Insurance / TPA claim</DialogTitle>
            <DialogDescription>Track a claim against this invoice through submission, approval and settlement.</DialogDescription>
          </DialogHeader>
          <div className="space-y-3 py-2">
            <div className="space-y-1.5">
              <Label htmlFor="claim-insurer">Insurer / TPA</Label>
              <Input id="claim-insurer" value={claimInsurer} onChange={(e) => setClaimInsurer(e.target.value)} placeholder="e.g. State Life, Jubilee TPA" />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="claim-policy">Policy number (optional)</Label>
              <Input id="claim-policy" value={claimPolicy} onChange={(e) => setClaimPolicy(e.target.value)} />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="claim-amount">Claim amount</Label>
              <Input id="claim-amount" type="number" value={claimAmount} onChange={(e) => setClaimAmount(e.target.value)} />
            </div>
          </div>
          <DialogFooter>
            <DialogClose asChild><Button variant="outline">Cancel</Button></DialogClose>
            <Button disabled={!claimInsurer.trim() || !claimAmount || createClaim.isPending} onClick={doClaim}>
              {createClaim.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : <FileText className="h-4 w-4" />} Create claim
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </Dialog>
  );
}

// ── Advances tab ──────────────────────────────────────────────────────────────

function AdvancesTab() {
  const { has } = useAuth();
  const { data: advances = [], isLoading } = usePatientAdvances(null);
  const { data: patients = [] } = usePatientsEhr();
  const record = useRecordAdvance();
  const [open, setOpen] = useState(false);
  const [patientId, setPatientId] = useState<number | null>(null);
  const [amount, setAmount] = useState("");
  const [method, setMethod] = useState("cash");
  const [notes, setNotes] = useState("");

  const submit = async () => {
    if (!patientId) return;
    await record.mutateAsync({
      patient_id: patientId,
      amount: parseFloat(amount) || 0,
      method,
      notes: notes || null,
    });
    setOpen(false);
    setPatientId(null); setAmount(""); setNotes("");
  };

  return (
    <SectionCard icon={Wallet} title="Patient advances & deposits">
      {isLoading ? (
        <LoadingState rows={4} />
      ) : (
        <div className="p-6 space-y-4">
          {has(PERMISSIONS.PaymentsManage) && (
            <div className="flex justify-end">
              <Button onClick={() => setOpen(true)}><Plus className="h-4 w-4" /> Receive advance</Button>
            </div>
          )}
          {advances.length === 0 ? (
            <EmptyState icon={Wallet} title="No active advances" description="Deposits received before invoicing appear here." />
          ) : (
            <Table>
              <TableHeader>
                <TableRow className="border-border hover:bg-transparent">
                  <TableHead>#</TableHead>
                  <TableHead>Patient</TableHead>
                  <TableHead className="text-right">Received</TableHead>
                  <TableHead className="text-right">Remaining</TableHead>
                  <TableHead>Method</TableHead>
                  <TableHead className="text-right">Status</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {advances.map((a) => {
                  const p = patients.find((x) => x.id === a.patient_id);
                  return (
                    <TableRow key={a.id}>
                      <TableCell className="font-mono text-xs">#{a.id}</TableCell>
                      <TableCell className="font-medium">{p ? `${p.first_name} ${p.last_name}` : `Patient #${a.patient_id}`}</TableCell>
                      <TableCell className="text-right">{formatMoney(a.amount)}</TableCell>
                      <TableCell className="text-right font-medium">{formatMoney(a.remaining)}</TableCell>
                      <TableCell className="capitalize text-muted-foreground">{a.method ?? "cash"}</TableCell>
                      <TableCell className="text-right"><StatusBadge status={a.status} /></TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          )}
        </div>
      )}

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>Receive advance</DialogTitle>
            <DialogDescription>A deposit held against the patient's account, applied to an invoice at settlement.</DialogDescription>
          </DialogHeader>
          <div className="space-y-3 py-2">
            <div className="space-y-1.5">
              <Label>Patient</Label>
              <Select value={patientId?.toString() ?? ""} onValueChange={(v) => setPatientId(Number(v))}>
                <SelectTrigger><SelectValue placeholder="Select patient" /></SelectTrigger>
                <SelectContent>{patients.map((p) => <SelectItem key={p.id} value={p.id.toString()}>{patientDescriptor(p)}</SelectItem>)}</SelectContent>
              </Select>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1.5">
                <Label htmlFor="adv-amount">Amount</Label>
                <Input id="adv-amount" type="number" value={amount} onChange={(e) => setAmount(e.target.value)} />
              </div>
              <div className="space-y-1.5">
                <Label>Method</Label>
                <Select value={method} onValueChange={setMethod}>
                  <SelectTrigger><SelectValue /></SelectTrigger>
                  <SelectContent>{["cash", "card", "online"].map((m) => <SelectItem key={m} value={m} className="capitalize">{m}</SelectItem>)}</SelectContent>
                </Select>
              </div>
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="adv-notes">Notes (optional)</Label>
              <Input id="adv-notes" value={notes} onChange={(e) => setNotes(e.target.value)} placeholder="e.g. IPD admission deposit" />
            </div>
          </div>
          <DialogFooter>
            <DialogClose asChild><Button variant="outline">Cancel</Button></DialogClose>
            <Button disabled={!patientId || !amount || record.isPending} onClick={submit}>
              {record.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : "Record advance"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </SectionCard>
  );
}

// ── Claims tab ───────────────────────────────────────────────────────────────

const CLAIM_TRANSITIONS: Record<string, string[]> = {
  draft: ["submitted"],
  submitted: ["approved", "partially_approved", "rejected"],
  approved: ["settled"],
  partially_approved: ["settled"],
  rejected: [],
  settled: [],
};

function ClaimsTab() {
  const { has } = useAuth();
  const { data: claims = [], isLoading } = useInsuranceClaims();
  const update = useUpdateInsuranceClaimStatus();
  const [approveId, setApproveId] = useState<number | null>(null);
  const [approveAmount, setApproveAmount] = useState("");
  const canManage = has(PERMISSIONS.BillingManage);

  const doTransition = async (id: number, status: string, approvedAmount?: number) => {
    await update.mutateAsync({
      id,
      status,
      approved_amount: approvedAmount ?? null,
    });
    setApproveId(null); setApproveAmount("");
  };

  return (
    <SectionCard icon={Landmark} title="Insurance / TPA claims">
      {isLoading ? (
        <LoadingState rows={4} />
      ) : claims.length === 0 ? (
        <EmptyState icon={Landmark} title="No claims" description="Create a claim from an invoice's detail panel." />
      ) : (
        <div className="p-6">
          <Table>
            <TableHeader>
              <TableRow className="border-border hover:bg-transparent">
                <TableHead>Invoice</TableHead>
                <TableHead>Patient</TableHead>
                <TableHead>Insurer</TableHead>
                <TableHead className="text-right">Claimed</TableHead>
                <TableHead className="text-right">Approved</TableHead>
                <TableHead className="text-right">Status</TableHead>
                {canManage && <TableHead className="text-right">Next step</TableHead>}
              </TableRow>
            </TableHeader>
            <TableBody>
              {claims.map((c) => {
                const nexts = CLAIM_TRANSITIONS[c.status] ?? [];
                return (
                  <TableRow key={c.id}>
                    <TableCell className="font-mono text-xs">{c.bill_number ?? `#${c.bill_id}`}</TableCell>
                    <TableCell className="font-medium">{c.patient_name ?? "—"}</TableCell>
                    <TableCell>{c.insurer}</TableCell>
                    <TableCell className="text-right">{formatMoney(c.claim_amount)}</TableCell>
                    <TableCell className="text-right text-muted-foreground">{c.approved_amount != null ? formatMoney(c.approved_amount) : "—"}</TableCell>
                    <TableCell className="text-right"><StatusBadge status={c.status} /></TableCell>
                    {canManage && (
                      <TableCell className="text-right space-x-1">
                        {nexts.includes("partially_approved") && (
                          <Button size="sm" variant="outline" disabled={update.isPending} onClick={() => setApproveId(c.id)}>
                            Partial…
                          </Button>
                        )}
                        {nexts.filter((n) => n !== "partially_approved").map((n) => (
                          <Button
                            key={n}
                            size="sm"
                            variant="outline"
                            disabled={update.isPending}
                            onClick={() => doTransition(c.id, n)}
                          >
                            {n.replace("_", " ")}
                          </Button>
                        ))}
                        {nexts.length === 0 && <span className="text-xs text-muted-foreground">—</span>}
                      </TableCell>
                    )}
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </div>
      )}

      {/* Partial-approval amount dialog */}
      <Dialog open={approveId != null} onOpenChange={(o) => !o && setApproveId(null)}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle>Partially approve claim</DialogTitle>
            <DialogDescription>Enter the amount the insurer actually approved; the claim then proceeds to settlement.</DialogDescription>
          </DialogHeader>
          <div className="space-y-2 py-2">
            <Label htmlFor="approved-amount">Approved amount</Label>
            <Input id="approved-amount" type="number" value={approveAmount} onChange={(e) => setApproveAmount(e.target.value)} />
          </div>
          <DialogFooter>
            <DialogClose asChild><Button variant="outline">Cancel</Button></DialogClose>
            <Button disabled={!approveAmount || update.isPending} onClick={() => approveId != null && doTransition(approveId, "partially_approved", parseFloat(approveAmount) || 0)}>
              {update.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : "Approve partially"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </SectionCard>
  );
}

// ── Expenses tab (SRS §2.15 — Phase 8 accounts module) ──────────────────────
//
// Income-vs-expense summary strip + expense ledger with record/void.
// Recording needs BillingManage; voiding needs BillingApprove (both
// enforced server-side; the UI only gates the affordances).

const EXPENSE_CATEGORIES = [
  "salary", "utilities", "rent", "supplies", "maintenance",
  "equipment", "transport", "marketing", "other",
] as const;

function ExpensesTab({ canManage, canApprove }: { canManage: boolean; canApprove: boolean }) {
  const today = new Date();
  const monthStart = new Date(today.getFullYear(), today.getMonth(), 1);
  const fmt = (d: Date) => d.toISOString().slice(0, 10);
  const [fromDate, setFromDate] = useState(fmt(monthStart));
  const [toDate, setToDate] = useState(fmt(today));

  const { data: summary } = useAccountsSummary(fromDate, toDate);
  const { data: expenses = [], isLoading } = useExpenses(fromDate, toDate);
  const create = useCreateExpense();
  const voidExp = useVoidExpense();

  const [addOpen, setAddOpen] = useState(false);
  const [form, setForm] = useState({
    category: "supplies",
    description: "",
    amount: "",
    expense_date: fmt(today),
    paid_to: "",
    method: "cash",
    reference_number: "",
  });
  const [voidTarget, setVoidTarget] = useState<Expense | null>(null);
  const [voidReason, setVoidReason] = useState("");

  const submit = async () => {
    await create.mutateAsync({
      category: form.category,
      description: form.description,
      amount: parseFloat(form.amount) || 0,
      expense_date: form.expense_date || null,
      paid_to: form.paid_to || null,
      payment_method: form.method,
      reference_number: form.reference_number || null,
    });
    setAddOpen(false);
    setForm({ category: "supplies", description: "", amount: "", expense_date: fmt(today), paid_to: "", method: "cash", reference_number: "" });
  };

  const doVoid = async () => {
    if (!voidTarget) return;
    await voidExp.mutateAsync({ id: voidTarget.id, reason: voidReason });
    setVoidTarget(null);
    setVoidReason("");
  };

  return (
    <SectionCard icon={TrendingDown} title="Expenses & financial summary">
      {isLoading ? (
        <LoadingState rows={5} />
      ) : (
        <div className="p-6 space-y-4">
          {/* Income-vs-expense summary strip */}
          {summary && (
            <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
              <div className="border border-border rounded-[var(--radius-md)] p-3">
                <div className="text-[10px] uppercase font-semibold tracking-wide text-muted-foreground">Net revenue</div>
                <div className="text-display-sm font-bold text-success">{formatMoney(summary.total_revenue)}</div>
                <div className="text-[10px] text-muted-foreground">{fromDate} → {toDate}</div>
              </div>
              <div className="border border-border rounded-[var(--radius-md)] p-3">
                <div className="text-[10px] uppercase font-semibold tracking-wide text-muted-foreground">Total expenses</div>
                <div className="text-display-sm font-bold text-destructive">{formatMoney(summary.total_expenses)}</div>
                <div className="text-[10px] text-muted-foreground">{summary.expense_count} entry(ies)</div>
              </div>
              <div className="border border-border rounded-[var(--radius-md)] p-3">
                <div className="text-[10px] uppercase font-semibold tracking-wide text-muted-foreground">Net position</div>
                <div className={`text-display-sm font-bold ${summary.net_position >= 0 ? "text-success" : "text-destructive"}`}>
                  {formatMoney(summary.net_position)}
                </div>
                <div className="text-[10px] text-muted-foreground">Revenue − expenses</div>
              </div>
              <div className="border border-border rounded-[var(--radius-md)] p-3">
                <div className="text-[10px] uppercase font-semibold tracking-wide text-muted-foreground">Top category</div>
                <div className="text-sm font-bold text-foreground mt-1.5 truncate">
                  {summary.by_category[0]?.category ?? "—"}
                </div>
                <div className="text-[10px] text-muted-foreground">
                  {summary.by_category[0] ? formatMoney(summary.by_category[0].total) : "No expenses"}
                </div>
              </div>
            </div>
          )}

          <PageToolbar>
            <div className="flex items-center gap-2">
              <Input type="date" className="w-[150px]" value={fromDate} onChange={(e) => setFromDate(e.target.value)} aria-label="From date" />
              <span className="text-xs text-muted-foreground">→</span>
              <Input type="date" className="w-[150px]" value={toDate} onChange={(e) => setToDate(e.target.value)} aria-label="To date" />
            </div>
            {canManage && (
              <Button className="ml-auto" onClick={() => setAddOpen(true)}>
                <Plus className="h-4 w-4" /> Record expense
              </Button>
            )}
          </PageToolbar>

          {expenses.length === 0 ? (
            <EmptyState
              icon={TrendingDown}
              title="No expenses in this range"
              description="Salaries, utilities, rent and other operating expenses appear here with the income-vs-expense position."
            />
          ) : (
            <Table>
              <TableHeader>
                <TableRow className="border-border hover:bg-transparent">
                  <TableHead>Date</TableHead>
                  <TableHead>Category</TableHead>
                  <TableHead>Description</TableHead>
                  <TableHead>Paid to</TableHead>
                  <TableHead className="text-right">Amount</TableHead>
                  <TableHead className="text-right">Status</TableHead>
                  {canApprove && <TableHead className="text-right">Action</TableHead>}
                </TableRow>
              </TableHeader>
              <TableBody>
                {expenses.map((e) => (
                  <TableRow key={e.id} className={e.voided_at ? "opacity-60" : ""}>
                    <TableCell className="font-mono text-xs">{e.expense_date}</TableCell>
                    <TableCell className="capitalize font-medium">{e.category}</TableCell>
                    <TableCell className="max-w-[240px] truncate" title={e.description}>{e.description}</TableCell>
                    <TableCell className="text-muted-foreground">{e.paid_to ?? "—"}</TableCell>
                    <TableCell className="text-right font-medium">{formatMoney(e.amount)}</TableCell>
                    <TableCell className="text-right">
                      {e.voided_at ? (
                        <Badge variant="outline" className="text-destructive border-destructive/40" title={e.void_reason ?? ""}>
                          voided
                        </Badge>
                      ) : (
                        <span className="text-[10px] uppercase font-bold text-success">active</span>
                      )}
                    </TableCell>
                    {canApprove && (
                      <TableCell className="text-right">
                        {!e.voided_at && (
                          <Button size="sm" variant="ghost" className="text-destructive hover:text-destructive" onClick={() => setVoidTarget(e)}>
                            <Ban className="h-3.5 w-3.5" /> Void
                          </Button>
                        )}
                      </TableCell>
                    )}
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </div>
      )}

      {/* Record expense dialog */}
      <Dialog open={addOpen} onOpenChange={setAddOpen}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>Record expense</DialogTitle>
            <DialogDescription>An operating-expense entry for the accounts ledger.</DialogDescription>
          </DialogHeader>
          <div className="space-y-3 py-2">
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1.5">
                <Label>Category</Label>
                <Select value={form.category} onValueChange={(v) => setForm({ ...form, category: v })}>
                  <SelectTrigger><SelectValue /></SelectTrigger>
                  <SelectContent>
                    {EXPENSE_CATEGORIES.map((c) => (
                      <SelectItem key={c} value={c} className="capitalize">{c}</SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="exp-amount">Amount</Label>
                <Input id="exp-amount" type="number" min="0" value={form.amount} onChange={(e) => setForm({ ...form, amount: e.target.value })} />
              </div>
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="exp-desc">Description</Label>
              <Input id="exp-desc" value={form.description} onChange={(e) => setForm({ ...form, description: e.target.value })} placeholder="e.g. August electricity bill" />
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1.5">
                <Label htmlFor="exp-date">Date</Label>
                <Input id="exp-date" type="date" value={form.expense_date} onChange={(e) => setForm({ ...form, expense_date: e.target.value })} />
              </div>
              <div className="space-y-1.5">
                <Label>Method</Label>
                <Select value={form.method} onValueChange={(v) => setForm({ ...form, method: v })}>
                  <SelectTrigger><SelectValue /></SelectTrigger>
                  <SelectContent>{["cash", "card", "bank", "cheque"].map((m) => <SelectItem key={m} value={m} className="capitalize">{m}</SelectItem>)}</SelectContent>
                </Select>
              </div>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1.5">
                <Label htmlFor="exp-paidto">Paid to (optional)</Label>
                <Input id="exp-paidto" value={form.paid_to} onChange={(e) => setForm({ ...form, paid_to: e.target.value })} placeholder="e. g. LESCO" />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="exp-ref">Reference (optional)</Label>
                <Input id="exp-ref" value={form.reference_number} onChange={(e) => setForm({ ...form, reference_number: e.target.value })} />
              </div>
            </div>
          </div>
          <DialogFooter>
            <DialogClose asChild><Button variant="outline">Cancel</Button></DialogClose>
            <Button disabled={!form.description.trim() || !form.amount || create.isPending} onClick={submit}>
              {create.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : "Record expense"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Void confirm */}
      <Dialog open={voidTarget != null} onOpenChange={(o) => !o && setVoidTarget(null)}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle className="text-destructive">Void expense?</DialogTitle>
            <DialogDescription>
              The entry stays in the ledger for audit but is excluded from all summaries. This requires the manager permission.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-2 py-2">
            <Label htmlFor="void-reason">Reason (required)</Label>
            <Input id="void-reason" value={voidReason} onChange={(e) => setVoidReason(e.target.value)} placeholder="e.g. duplicate entry" />
          </div>
          <DialogFooter>
            <DialogClose asChild><Button variant="outline">Keep entry</Button></DialogClose>
            <Button variant="destructive" disabled={!voidReason.trim() || voidExp.isPending} onClick={doVoid}>
              {voidExp.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : "Void expense"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </SectionCard>
  );
}
