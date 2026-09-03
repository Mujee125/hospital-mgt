/**
 * Billing & invoicing — uses shared layout components.
 */
import { useState } from "react";
import { Receipt, Plus, Loader2, DollarSign, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter, DialogClose } from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useBills, useBillItems, usePayments, useCreateBill, useRecordPayment, usePatientsEhr } from "@/lib/queries";
import { useAuth } from "@/lib/auth";
import { PERMISSIONS } from "@/lib/rbac";
import { formatMoney } from "@/lib/utils";
import { PageContainer, PageHeader, SectionCard, EmptyState, StatusBadge, LoadingState, PageToolbar } from "@/components/layout/shared";

export function Billing() {
  const { has } = useAuth();
  const { data: bills = [], isLoading } = useBills();
  const { data: patients = [] } = usePatientsEhr();
  const createBill = useCreateBill();

  const [createOpen, setCreateOpen] = useState(false);
  const [detailId, setDetailId] = useState<number | null>(null);
  const [patientId, setPatientId] = useState<number | null>(null);
  const [items, setItems] = useState([{ item_type: "consultation", description: "", quantity: "1", unit_price: "0" }]);
  const [discount, setDiscount] = useState("0");
  const [tax, setTax] = useState("0");

  const total = items.reduce((s, it) => s + (parseFloat(it.quantity) || 0) * (parseFloat(it.unit_price) || 0), 0);
  const net = Math.max(0, total - (parseFloat(discount) || 0) + (parseFloat(tax) || 0));

  const submit = async () => {
    if (!patientId || items.length === 0) return;
    await createBill.mutateAsync({
      patient_id: patientId, discount: parseFloat(discount) || 0, tax: parseFloat(tax) || 0,
      items: items.map((it) => ({ item_type: it.item_type, description: it.description, quantity: parseFloat(it.quantity) || 0, unit_price: parseFloat(it.unit_price) || 0 })),
    });
    setCreateOpen(false);
    setPatientId(null);
    setItems([{ item_type: "consultation", description: "", quantity: "1", unit_price: "0" }]);
    setDiscount("0"); setTax("0");
  };

  return (
    <PageContainer>
      <PageHeader
        icon={Receipt}
        title="Billing & Invoices"
        description="Invoices, line items & payments"
        actions={has(PERMISSIONS.BillingCreate) && (
          <Button onClick={() => setCreateOpen(true)}><Plus className="h-4 w-4" /> New invoice</Button>
        )}
      />

      <SectionCard>
        {isLoading ? (
          <LoadingState rows={5} />
        ) : bills.length === 0 ? (
          <EmptyState icon={Receipt} title="No invoices" description="Create an invoice to get started." />
        ) : (
          <>
            <PageToolbar>
              <span className="text-sm font-medium text-muted-foreground">{bills.length} total invoices</span>
              <span className="text-xs text-muted-foreground ml-auto">
                {bills.filter((b) => b.status === "paid").length} paid · {bills.filter((b) => b.status === "unpaid" || b.status === "partial").length} outstanding
              </span>
            </PageToolbar>
            <Table>
              <TableHeader>
                <TableRow className="border-border hover:bg-transparent">
                  <TableHead>Invoice #</TableHead>
                  <TableHead>Patient</TableHead>
                  <TableHead className="text-right">Net amount</TableHead>
                  <TableHead className="text-right">Paid</TableHead>
                  <TableHead className="text-right">Status</TableHead>
                  <TableHead className="text-right">Action</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {bills.map((b) => (
                  <TableRow key={b.id}>
                    <TableCell className="font-mono text-xs">{b.bill_number}</TableCell>
                    <TableCell className="font-medium">{b.patient_name ?? "—"}</TableCell>
                    <TableCell className="text-right font-medium">{formatMoney(b.net_amount)}</TableCell>
                    <TableCell className="text-right text-muted-foreground">{formatMoney(b.amount_paid ?? 0)}</TableCell>
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
      </SectionCard>

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
                <SelectContent>{patients.map((p) => <SelectItem key={p.id} value={p.id.toString()}>{p.first_name} {p.last_name} · {p.phone}</SelectItem>)}</SelectContent>
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
                <Input type="number" value={discount} onChange={(e) => setDiscount(e.target.value)} />
              </div>
              <div className="space-y-1.5">
                <Label>Tax</Label>
                <Input type="number" value={tax} onChange={(e) => setTax(e.target.value)} />
              </div>
            </div>
            <div className="flex items-center justify-between border-t border-border pt-3 text-sm">
              <span className="text-muted-foreground">Subtotal: {formatMoney(total)} · Net: </span>
              <span className="text-display-md text-foreground">{formatMoney(net)}</span>
            </div>
          </div>
          <DialogFooter>
            <DialogClose asChild><Button variant="outline">Cancel</Button></DialogClose>
            <Button disabled={!patientId || createBill.isPending} onClick={submit}>
              {createBill.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : "Create invoice"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {detailId != null && <BillDetail id={detailId} onClose={() => setDetailId(null)} canPay={has(PERMISSIONS.PaymentsManage)} />}
    </PageContainer>
  );
}

function BillDetail({ id, onClose, canPay }: { id: number; onClose: () => void; canPay: boolean }) {
  const { data: items = [] } = useBillItems(id);
  const { data: payments = [] } = usePayments(id);
  const record = useRecordPayment();
  const [amount, setAmount] = useState("");
  const [method, setMethod] = useState("cash");

  const paid = payments.reduce((s, p) => s + (p.amount as number), 0);

  const pay = async () => {
    await record.mutateAsync({ bill_id: id, amount: parseFloat(amount) || 0, payment_method: method });
    setAmount("");
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle>Invoice #{id} — details</DialogTitle>
            <DialogDescription>Line items, recorded payments, and the option to record a new payment.</DialogDescription>
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
            <h4 className="text-xs uppercase font-semibold text-muted-foreground mb-2">Payments ({formatMoney(paid)} received)</h4>
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
          {canPay && (
            <div className="border border-border rounded-lg p-3 space-y-2">
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
            </div>
          )}
        </div>
        <DialogFooter>
          <DialogClose asChild><Button variant="outline">Close</Button></DialogClose>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
