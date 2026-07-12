/**
 * Inventory — wires the 6 inventory commands added in Batch 1
 * (`src-tauri/src/commands/inventory.rs`). Pattern matches Doctors.tsx /
 * Laboratory.tsx: PageContainer → PageHeader → SectionCard →
 * (LoadingState | EmptyState | PageToolbar + Table). Four dialogs:
 *
 *   1. CreateItemDialog  — create_inventory_item
 *   2. EditItemDialog    — update_inventory_item
 *   3. AdjustStockDialog — adjust_inventory (quantity_change + reason)
 *   4. MovementsDialog   — get_inventory_movements(item_id)
 *
 * Stock changes NEVER go through the edit dialog's stock_quantity field
 * directly — the backend's `update_inventory_item` does accept a new
 * stock_quantity (it's a privileged "fix wrong count" override), but the
 * canonical path for day-to-day dispense/replenish is `adjust_inventory`,
 * which atomically updates `stock_quantity` AND writes a movement row with
 * the resulting balance snapshot. The AdjustStockDialog enforces a reason
 * (the backend rejects empty reasons).
 */
import { useState } from "react";
import { motion } from "motion/react";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Label } from "@/components/ui/label";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
  DialogClose,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Package, Plus, Edit, ArrowUpDown, History, AlertTriangle } from "lucide-react";
import {
  useInventoryItems,
  useCreateInventoryItem,
  useUpdateInventoryItem,
  useAdjustInventory,
  useInventoryMovements,
} from "@/lib/queries";
import { useAuth } from "@/lib/auth";
import { PERMISSIONS } from "@/lib/rbac";
import { formatMoney } from "@/lib/utils";
import type { InventoryItem, InventoryMovement, CreateInventoryItem, UpdateInventoryItem } from "@/lib/models";
import {
  PageContainer,
  PageHeader,
  SectionCard,
  EmptyState,
  LoadingState,
  PageToolbar,
  StatusBadge,
} from "@/components/layout/shared";

const INVENTORY_CATEGORIES = [
  "medication",
  "consumable",
  "equipment",
  "lab_supply",
  "other",
] as const;

/** Stock status for a single item, derived from quantity vs reorder level. */
function deriveStockStatus(item: InventoryItem): {
  label: string;
  variant: "in-stock" | "low-stock" | "out-of-stock" | "inactive";
} {
  if (!item.is_active) return { label: "Inactive", variant: "inactive" };
  if (item.stock_quantity <= 0) return { label: "Out of stock", variant: "out-of-stock" };
  if (item.stock_quantity <= item.reorder_level)
    return { label: "Low stock", variant: "low-stock" };
  return { label: "In stock", variant: "in-stock" };
}

const STOCK_BADGE_CLASS: Record<string, string> = {
  "in-stock": "bg-success/10 text-success border-success/20",
  "low-stock": "bg-warning/10 text-warning border-warning/20",
  "out-of-stock": "bg-destructive/10 text-destructive border-destructive/20",
  inactive: "bg-muted text-muted-foreground border-border",
};

export function Inventory() {
  const { has } = useAuth();
  const canManage = has(PERMISSIONS.InventoryManage);

  const [searchQuery, setSearchQuery] = useState("");
  const [categoryFilter, setCategoryFilter] = useState<string>("");
  const [lowStockOnly, setLowStockOnly] = useState(false);

  // The `useInventoryItems` hook accepts categoryFilter and lowStockOnly
  // as separate params; the backend composes them into a single WHERE
  // clause. Pass `null` (not undefined) when the filter is empty so the
  // query key is stable.
  const { data: items = [], isLoading } = useInventoryItems(
    categoryFilter || null,
    lowStockOnly || null,
  );

  const [createOpen, setCreateOpen] = useState(false);
  const [editTarget, setEditTarget] = useState<InventoryItem | null>(null);
  const [adjustTarget, setAdjustTarget] = useState<InventoryItem | null>(null);
  const [movementsTarget, setMovementsTarget] = useState<InventoryItem | null>(null);

  // Client-side search on top of the server-side category/low-stock filters
  // (the backend doesn't expose a name-search param, so we filter the
  // returned list client-side — same pattern as Doctors.tsx).
  const filteredItems = items.filter((it) => {
    const q = searchQuery.toLowerCase();
    if (!q) return true;
    return (
      it.name.toLowerCase().includes(q) ||
      (it.sku?.toLowerCase().includes(q) ?? false) ||
      it.category.toLowerCase().includes(q) ||
      (it.batch_number?.toLowerCase().includes(q) ?? false)
    );
  });

  const lowStockCount = items.filter(
    (it) => it.is_active && it.stock_quantity <= it.reorder_level,
  ).length;
  const outOfStockCount = items.filter(
    (it) => it.is_active && it.stock_quantity <= 0,
  ).length;

  return (
    <PageContainer>
      <PageHeader
        icon={Package}
        title="Inventory"
        description="Track stock levels, record dispenses, and audit every movement."
        actions={
          canManage && (
            <Button onClick={() => setCreateOpen(true)} className="gap-2">
              <Plus className="h-4 w-4" /> Add item
            </Button>
          )
        }
      />

      <SectionCard>
        {isLoading ? (
          <LoadingState rows={6} />
        ) : items.length === 0 ? (
          <EmptyState
            icon={Package}
            title="No inventory items"
            description="Add your first item to start tracking stock levels."
            action={
              canManage && (
                <Button onClick={() => setCreateOpen(true)} size="sm" className="gap-2">
                  <Plus className="h-3.5 w-3.5" /> Add item
                </Button>
              )
            }
          />
        ) : (
          <>
            <PageToolbar>
              <div className="relative w-full max-w-md">
                <Input
                  placeholder="Search by name, SKU, category, or batch…"
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  className="h-10"
                />
              </div>
              <Select
                value={categoryFilter || "all"}
                onValueChange={(v) => setCategoryFilter(v === "all" ? "" : v)}
              >
                <SelectTrigger className="w-[180px] h-10">
                  <SelectValue placeholder="All categories" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">All categories</SelectItem>
                  {INVENTORY_CATEGORIES.map((c) => (
                    <SelectItem key={c} value={c} className="capitalize">
                      {c.replace("_", " ")}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <Button
                type="button"
                variant={lowStockOnly ? "default" : "outline"}
                size="sm"
                onClick={() => setLowStockOnly((v) => !v)}
                className="gap-2"
                title="Show only items at or below their reorder level"
              >
                <AlertTriangle className="h-4 w-4" />
                Low stock only
              </Button>
              <span className="text-xs text-muted-foreground ml-auto tabular-nums">
                {filteredItems.length} of {items.length} items
                {lowStockCount > 0 && (
                  <>
                    {" · "}
                    <span className="text-warning font-medium">{lowStockCount} low</span>
                  </>
                )}
                {outOfStockCount > 0 && (
                  <>
                    {" · "}
                    <span className="text-destructive font-medium">{outOfStockCount} out</span>
                  </>
                )}
              </span>
            </PageToolbar>

            <Table>
              <TableHeader>
                <TableRow className="hover:bg-transparent">
                  <TableHead>Item</TableHead>
                  <TableHead>Category</TableHead>
                  <TableHead className="text-right">In stock</TableHead>
                  <TableHead className="text-right">Reorder at</TableHead>
                  <TableHead className="text-right">Unit cost</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {filteredItems.map((item, i) => {
                  const status = deriveStockStatus(item);
                  return (
                    <motion.tr
                      key={item.id}
                      initial={{ opacity: 0 }}
                      animate={{ opacity: 1 }}
                      transition={{ duration: 0.15, delay: Math.min(i * 0.02, 0.3) }}
                      className="border-b border-border/70 transition-colors hover:bg-muted/40"
                    >
                      <TableCell className="font-semibold text-foreground">
                        <div className="flex flex-col">
                          <span>{item.name}</span>
                          {(item.sku || item.batch_number) && (
                            <span className="text-[11px] text-muted-foreground font-mono">
                              {item.sku ? `SKU: ${item.sku}` : ""}
                              {item.sku && item.batch_number ? " · " : ""}
                              {item.batch_number ? `Batch: ${item.batch_number}` : ""}
                            </span>
                          )}
                        </div>
                      </TableCell>
                      <TableCell>
                        <Badge
                          variant="outline"
                          className="bg-primary/5 text-primary border-primary/20 font-semibold text-[11px] capitalize rounded-full"
                        >
                          {item.category.replace("_", " ")}
                        </Badge>
                      </TableCell>
                      <TableCell className="text-right tabular-nums font-semibold">
                        {item.stock_quantity.toLocaleString("en-PK")}
                        {item.unit && (
                          <span className="text-[11px] text-muted-foreground ml-1 font-normal">
                            {item.unit}
                          </span>
                        )}
                      </TableCell>
                      <TableCell className="text-right tabular-nums text-muted-foreground text-xs">
                        {item.reorder_level.toLocaleString("en-PK")}
                      </TableCell>
                      <TableCell className="text-right tabular-nums text-muted-foreground text-xs">
                        {formatMoney(item.unit_cost)}
                      </TableCell>
                      <TableCell>
                        {status.variant === "inactive" ? (
                          <StatusBadge status="inactive" />
                        ) : (
                          <span
                            className={`status-badge ${STOCK_BADGE_CLASS[status.variant] ?? ""}`}
                          >
                            <span
                              className="h-1.5 w-1.5 rounded-full"
                              style={{ background: "currentColor" }}
                            />
                            {status.label}
                          </span>
                        )}
                      </TableCell>
                      <TableCell className="text-right">
                        <div className="flex justify-end gap-1">
                          {canManage && (
                            <>
                              <Button
                                variant="ghost"
                                size="icon"
                                onClick={() => setAdjustTarget(item)}
                                className="h-8 w-8 text-primary hover:text-primary hover:bg-primary/10"
                                title="Adjust stock (dispense / replenish)"
                              >
                                <ArrowUpDown className="h-4 w-4" />
                              </Button>
                              <Button
                                variant="ghost"
                                size="icon"
                                onClick={() => setEditTarget(item)}
                                className="h-8 w-8 text-muted-foreground hover:text-foreground"
                                title="Edit item details"
                              >
                                <Edit className="h-4 w-4" />
                              </Button>
                            </>
                          )}
                          <Button
                            variant="ghost"
                            size="icon"
                            onClick={() => setMovementsTarget(item)}
                            className="h-8 w-8 text-muted-foreground hover:text-foreground"
                            title="View movement history"
                          >
                            <History className="h-4 w-4" />
                          </Button>
                        </div>
                      </TableCell>
                    </motion.tr>
                  );
                })}
              </TableBody>
            </Table>
          </>
        )}
      </SectionCard>

      {createOpen && (
        <ItemFormDialog
          mode="create"
          onClose={() => setCreateOpen(false)}
        />
      )}

      {editTarget && (
        <ItemFormDialog
          mode="edit"
          item={editTarget}
          onClose={() => setEditTarget(null)}
        />
      )}

      {adjustTarget && (
        <AdjustStockDialog item={adjustTarget} onClose={() => setAdjustTarget(null)} />
      )}

      {movementsTarget && (
        <MovementsDialog item={movementsTarget} onClose={() => setMovementsTarget(null)} />
      )}
    </PageContainer>
  );
}

// ── Create / Edit dialog ────────────────────────────────────────────────────

const EMPTY_FORM: ItemFormState = {
  name: "",
  sku: "",
  category: "medication",
  unit: "units",
  stock_quantity: "0",
  reorder_level: "0",
  expiry_date: "",
  batch_number: "",
  unit_cost: "0",
  is_active: true,
};

interface ItemFormState {
  name: string;
  sku: string;
  category: string;
  unit: string;
  stock_quantity: string;
  reorder_level: string;
  expiry_date: string;
  batch_number: string;
  unit_cost: string;
  is_active: boolean;
}

function ItemFormDialog({
  mode,
  item,
  onClose,
}: {
  mode: "create" | "edit";
  item?: InventoryItem;
  onClose: () => void;
}) {
  const createItem = useCreateInventoryItem();
  const updateItem = useUpdateInventoryItem();
  const loading = createItem.isPending || updateItem.isPending;

  const [form, setForm] = useState<ItemFormState>(
    item
      ? {
          name: item.name,
          sku: item.sku ?? "",
          category: item.category,
          unit: item.unit ?? "units",
          stock_quantity: String(item.stock_quantity),
          reorder_level: String(item.reorder_level),
          expiry_date: item.expiry_date ?? "",
          batch_number: item.batch_number ?? "",
          unit_cost: String(item.unit_cost),
          is_active: item.is_active,
        }
      : EMPTY_FORM,
  );

  const set = <K extends keyof ItemFormState>(k: K, v: ItemFormState[K]) =>
    setForm((f) => ({ ...f, [k]: v }));

  const handleSubmit = async () => {
    if (!form.name.trim()) {
      return;
    }

    if (mode === "create") {
      const payload: CreateInventoryItem = {
        name: form.name.trim(),
        sku: form.sku.trim() === "" ? null : form.sku.trim(),
        category: form.category,
        unit: form.unit.trim() === "" ? null : form.unit.trim(),
        stock_quantity: Number(form.stock_quantity) || 0,
        reorder_level: Number(form.reorder_level) || 0,
        expiry_date: form.expiry_date.trim() === "" ? null : form.expiry_date.trim(),
        batch_number: form.batch_number.trim() === "" ? null : form.batch_number.trim(),
        unit_cost: Number(form.unit_cost) || 0,
        is_active: form.is_active,
      };
      try {
        await createItem.mutateAsync(payload);
        onClose();
      } catch {
        /* toast already shown by the mutation's onError */
      }
    } else if (item) {
      const payload: UpdateInventoryItem = {
        id: item.id,
        name: form.name.trim(),
        sku: form.sku.trim() === "" ? null : form.sku.trim(),
        category: form.category,
        unit: form.unit.trim() === "" ? null : form.unit.trim(),
        stock_quantity: Number(form.stock_quantity) || 0,
        reorder_level: Number(form.reorder_level) || 0,
        expiry_date: form.expiry_date.trim() === "" ? null : form.expiry_date.trim(),
        batch_number: form.batch_number.trim() === "" ? null : form.batch_number.trim(),
        unit_cost: Number(form.unit_cost) || 0,
        is_active: form.is_active,
      };
      try {
        await updateItem.mutateAsync({ id: item.id, item: payload });
        onClose();
      } catch {
        /* toast already shown by the mutation's onError */
      }
    }
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>
            {mode === "create" ? "Add inventory item" : "Edit inventory item"}
          </DialogTitle>
          <DialogDescription>
            {mode === "create"
              ? "Record a new medication, consumable, or equipment item."
              : "Update item details. To change stock quantity, use the Adjust Stock action — that writes a movement row with a reason."}
          </DialogDescription>
        </DialogHeader>

        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 py-2">
          <div className="sm:col-span-2 space-y-1.5">
            <Label htmlFor="item-name">Item name *</Label>
            <Input
              id="item-name"
              placeholder="Paracetamol 500mg"
              value={form.name}
              onChange={(e) => set("name", e.target.value)}
              disabled={loading}
              required
            />
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="item-sku">SKU / code</Label>
            <Input
              id="item-sku"
              placeholder="MED-PARA-500"
              value={form.sku}
              onChange={(e) => set("sku", e.target.value)}
              disabled={loading}
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="item-category">Category</Label>
            <Select
              value={form.category}
              onValueChange={(v) => set("category", v)}
              disabled={loading}
            >
              <SelectTrigger id="item-category">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {INVENTORY_CATEGORIES.map((c) => (
                  <SelectItem key={c} value={c} className="capitalize">
                    {c.replace("_", " ")}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="item-unit">Unit</Label>
            <Input
              id="item-unit"
              placeholder="tablets, vials, boxes"
              value={form.unit}
              onChange={(e) => set("unit", e.target.value)}
              disabled={loading}
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="item-batch">Batch number</Label>
            <Input
              id="item-batch"
              placeholder="B-2024-001"
              value={form.batch_number}
              onChange={(e) => set("batch_number", e.target.value)}
              disabled={loading}
            />
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="item-stock">
              {mode === "create" ? "Opening stock" : "Stock quantity"}
            </Label>
            <Input
              id="item-stock"
              type="number"
              inputMode="decimal"
              step="0.01"
              value={form.stock_quantity}
              onChange={(e) => set("stock_quantity", e.target.value)}
              disabled={loading}
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="item-reorder">Reorder level</Label>
            <Input
              id="item-reorder"
              type="number"
              inputMode="decimal"
              step="0.01"
              value={form.reorder_level}
              onChange={(e) => set("reorder_level", e.target.value)}
              disabled={loading}
            />
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="item-cost">Unit cost (PKR)</Label>
            <Input
              id="item-cost"
              type="number"
              inputMode="decimal"
              step="0.01"
              value={form.unit_cost}
              onChange={(e) => set("unit_cost", e.target.value)}
              disabled={loading}
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="item-expiry">Expiry date</Label>
            <Input
              id="item-expiry"
              type="date"
              value={form.expiry_date}
              onChange={(e) => set("expiry_date", e.target.value)}
              disabled={loading}
            />
          </div>

          {mode === "edit" && (
            <div className="sm:col-span-2 flex items-center gap-3 p-3 rounded-[var(--radius-md)] bg-muted/40 border border-border select-none">
              <input
                id="item-active"
                type="checkbox"
                checked={form.is_active}
                onChange={(e) => set("is_active", e.target.checked)}
                disabled={loading}
                className="h-4 w-4 rounded-[var(--radius-sm)] border border-border accent-primary cursor-pointer focus:ring-2 focus:ring-primary/30 focus:ring-offset-2 focus:ring-offset-background"
              />
              <Label
                htmlFor="item-active"
                className="text-sm font-medium cursor-pointer"
              >
                Active (item is in use and appears in dispense / reorder lists)
              </Label>
            </div>
          )}
        </div>

        <DialogFooter>
          <DialogClose asChild>
            <Button variant="outline" disabled={loading}>
              Cancel
            </Button>
          </DialogClose>
          <Button
            onClick={handleSubmit}
            disabled={loading || !form.name.trim()}
          >
            {loading
              ? "Saving…"
              : mode === "create"
                ? "Create item"
                : "Save changes"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// ── Adjust stock dialog ─────────────────────────────────────────────────────

const ADJUST_REASONS = [
  "dispense",
  "replenish",
  "return",
  "damaged",
  "expired",
  "stocktake_adjustment",
  "initial_stock",
] as const;

function AdjustStockDialog({
  item,
  onClose,
}: {
  item: InventoryItem;
  onClose: () => void;
}) {
  const adjust = useAdjustInventory();
  const [quantityChange, setQuantityChange] = useState("");
  const [reason, setReason] = useState<string>("");
  const [notes, setNotes] = useState("");

  // Parse the quantity change. Allow negative (dispense) and positive
  // (replenish). The backend rejects 0 and empty reason.
  const parsedChange = parseInt(quantityChange, 10);
  const isValidChange = !isNaN(parsedChange) && parsedChange !== 0;
  const projectedBalance = item.stock_quantity + (isValidChange ? parsedChange : 0);
  const wouldGoNegative = isValidChange && projectedBalance < 0;

  const handleSubmit = async () => {
    if (!isValidChange) return;
    if (!reason) return;
    if (wouldGoNegative) return;

    try {
      await adjust.mutateAsync({
        item_id: item.id,
        quantity_change: parsedChange,
        reason: notes.trim() === "" ? reason : `${reason}: ${notes.trim()}`,
      });
      onClose();
    } catch {
      /* toast already shown by the mutation's onError */
    }
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>Adjust stock — {item.name}</DialogTitle>
          <DialogDescription>
            Record a dispense, replenish, return, or write-off. Every
            adjustment writes a movement row with the resulting balance
            snapshot, so the audit trail stays in sync with the stock count.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-2">
          <div className="grid grid-cols-2 gap-3 text-sm">
            <div className="p-3 rounded-[var(--radius-md)] bg-muted/40 border border-border">
              <p className="text-[11px] uppercase tracking-wide text-muted-foreground">
                Current balance
              </p>
              <p className="text-display-md text-foreground tabular-nums mt-1">
                {item.stock_quantity.toLocaleString("en-PK")}
                {item.unit && (
                  <span className="text-xs text-muted-foreground ml-1 font-normal">
                    {item.unit}
                  </span>
                )}
              </p>
            </div>
            <div className="p-3 rounded-[var(--radius-md)] bg-muted/40 border border-border">
              <p className="text-[11px] uppercase tracking-wide text-muted-foreground">
                Projected balance
              </p>
              <p
                className={`text-display-md tabular-nums mt-1 ${
                  wouldGoNegative
                    ? "text-destructive"
                    : isValidChange && parsedChange < 0
                      ? "text-warning"
                      : "text-foreground"
                }`}
              >
                {projectedBalance.toLocaleString("en-PK")}
                {item.unit && (
                  <span className="text-xs text-muted-foreground ml-1 font-normal">
                    {item.unit}
                  </span>
                )}
              </p>
            </div>
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="qty-change">Quantity change *</Label>
            <Input
              id="qty-change"
              type="number"
              inputMode="numeric"
              placeholder="e.g. -10 for dispense, +50 for replenish"
              value={quantityChange}
              onChange={(e) => setQuantityChange(e.target.value)}
              disabled={adjust.isPending}
            />
            <p className="text-[11px] text-muted-foreground">
              Negative for dispense / write-off, positive for replenish /
              return. Cannot be zero.
            </p>
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="reason">Reason *</Label>
            <Select
              value={reason}
              onValueChange={(v) => setReason(v)}
              disabled={adjust.isPending}
            >
              <SelectTrigger id="reason">
                <SelectValue placeholder="Select a reason" />
              </SelectTrigger>
              <SelectContent>
                {ADJUST_REASONS.map((r) => (
                  <SelectItem key={r} value={r} className="capitalize">
                    {r.replace(/_/g, " ")}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="notes">Notes (optional)</Label>
            <Textarea
              id="notes"
              placeholder="e.g. Patient John Doe prescription #1234"
              rows={2}
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              disabled={adjust.isPending}
              className="resize-none"
            />
          </div>

          {wouldGoNegative && (
            <div className="rounded-[var(--radius-md)] border border-destructive/30 bg-destructive/8 px-4 py-3 text-xs text-destructive">
              This adjustment would drive stock negative. Dispense cannot
              exceed the current balance.
            </div>
          )}
        </div>

        <DialogFooter>
          <DialogClose asChild>
            <Button variant="outline" disabled={adjust.isPending}>
              Cancel
            </Button>
          </DialogClose>
          <Button
            onClick={handleSubmit}
            disabled={adjust.isPending || !isValidChange || !reason || wouldGoNegative}
          >
            {adjust.isPending ? "Adjusting…" : "Apply adjustment"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// ── Movements dialog ────────────────────────────────────────────────────────

function MovementsDialog({
  item,
  onClose,
}: {
  item: InventoryItem;
  onClose: () => void;
}) {
  // Fetch the last 100 movements for this item. The backend orders by
  // created_at DESC and limits to the requested count.
  const { data: movements = [], isLoading } = useInventoryMovements(item.id, 100);

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-3xl">
        <DialogHeader>
          <DialogTitle>Movement history — {item.name}</DialogTitle>
          <DialogDescription>
            Last 100 stock movements for this item. Each row records the
            quantity change, the resulting balance, and the reason. Newest
            first.
          </DialogDescription>
        </DialogHeader>

        {isLoading ? (
          <LoadingState rows={5} />
        ) : movements.length === 0 ? (
          <div className="py-12 text-center text-sm text-muted-foreground">
            No movements recorded yet. Adjustments made via the Adjust Stock
            action will appear here.
          </div>
        ) : (
          <div className="max-h-[60vh] overflow-y-auto -mx-2">
            <Table>
              <TableHeader className="sticky top-0 bg-background">
                <TableRow className="hover:bg-transparent">
                  <TableHead>When</TableHead>
                  <TableHead className="text-right">Change</TableHead>
                  <TableHead className="text-right">Balance after</TableHead>
                  <TableHead>Reason</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {movements.map((m: InventoryMovement) => (
                  <TableRow key={m.id}>
                    <TableCell className="text-xs text-muted-foreground font-mono">
                      {new Date(m.created_at).toLocaleString()}
                    </TableCell>
                    <TableCell
                      className={`text-right tabular-nums font-semibold ${
                        m.quantity_change > 0
                          ? "text-success"
                          : "text-destructive"
                      }`}
                    >
                      {m.quantity_change > 0 ? "+" : ""}
                      {m.quantity_change.toLocaleString("en-PK")}
                    </TableCell>
                    <TableCell className="text-right tabular-nums text-muted-foreground">
                      {m.balance_after.toLocaleString("en-PK")}
                    </TableCell>
                    <TableCell>
                      <div className="flex flex-col">
                        <span className="text-xs font-medium capitalize">
                          {m.reason.replace(/_/g, " ")}
                        </span>
                        {m.notes && (
                          <span className="text-[11px] text-muted-foreground">
                            {m.notes}
                          </span>
                        )}
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        )}

        <DialogFooter>
          <DialogClose asChild>
            <Button variant="outline">Close</Button>
          </DialogClose>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
