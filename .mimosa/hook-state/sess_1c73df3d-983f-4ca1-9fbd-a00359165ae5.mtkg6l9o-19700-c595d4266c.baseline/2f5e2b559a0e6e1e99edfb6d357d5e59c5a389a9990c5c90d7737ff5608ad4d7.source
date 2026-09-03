/**
 * Pharmacy — medication catalog, prescriptions, and dispensing
 * (Phase 2-C, SRS FR-0120–FR-0124).
 *
 * Page layout: PageContainer → PageHeader (Pill icon, "Pharmacy") →
 * two SectionCards:
 *
 *   1. Medication catalog (FR-0120) — searchable table of all
 *      medications (active + inactive). Add / edit / soft-delete buttons
 *      are gated by `InventoryManage` (FR-0124). Soft-deleted rows
 *      remain visible with an "Inactive" badge so historical
 *      prescriptions stay readable.
 *
 *   2. Prescriptions (FR-0121) — table of all prescriptions (newest
 *      first), filterable by patient + status. "New prescription"
 *      button is gated by `PatientsCreate` (doctors). Each row opens a
 *      detail dialog showing the line items + a "Dispense" action per
 *      item, gated by `InventoryManage` (pharmacists).
 *
 * Controlled-substance verification (FR-0123): when the user clicks
 * Dispense on an item with `is_controlled === true`, a confirmation
 * dialog pops up warning about the controlled-substance schedule. The
 * SRS explicitly allows the simplification (single-pharmacist confirm
 * instead of two-person witness); the backend just requires
 * `InventoryManage` — there is no second-witness table in this
 * iteration.
 *
 * Patterns reused: PageContainer / PageHeader / SectionCard / EmptyState
 * / LoadingState / PageToolbar / StatusBadge (shared.tsx), Dialog
 * (radix), Select (radix), Table (shadcn). Same motion.div-on-table-rows
 * + `initial={{ opacity: 0 }} animate={{ opacity: 1 }}` pattern as
 * Inventory.tsx for the staggered fade-in.
 */
import { useState } from "react";
import { motion } from "motion/react";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
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
import {
  Pill,
  Plus,
  Edit,
  Trash2,
  Search,
  FileText,
  AlertTriangle,
  CheckCircle2,
  Eye,
  X,
} from "lucide-react";
import {
  useMedications,
  useCreateMedication,
  useUpdateMedication,
  useDeleteMedication,
  usePrescriptions,
  usePrescription,
  useCreatePrescription,
  useDispensePrescriptionItem,
  usePatientsEhr,
  useDoctors,
} from "@/lib/queries";
import { useAuth } from "@/lib/auth";
import { PERMISSIONS } from "@/lib/rbac";
import type {
  Medication,
  CreateMedication,
  UpdateMedication,
  PrescriptionItem,
  CreatePrescription,
  CreatePrescriptionItem,
  PatientEhr,
  Doctor,
} from "@/lib/models";
import {
  PageContainer,
  PageHeader,
  SectionCard,
  EmptyState,
  LoadingState,
  PageToolbar,
  StatusBadge,
} from "@/components/layout/shared";

// ── Catalog constants ────────────────────────────────────────────────────────

const MEDICATION_FORMS = [
  "tablet",
  "capsule",
  "syrup",
  "injection",
  "inhaler",
  "cream",
  "drops",
  "other",
] as const;

const MEDICATION_SCHEDULES = [
  "non-controlled",
  "schedule-II",
  "schedule-III",
  "schedule-IV",
  "schedule-V",
] as const;

const PRESCRIPTION_STATUSES = ["active", "dispensed", "cancelled"] as const;

const ROUTES = ["oral", "IV", "IM", "topical", "inhalation", "sublingual"] as const;

const FREQUENCIES = [
  "once daily",
  "twice daily",
  "three times daily",
  "four times daily",
  "every 4 hours",
  "every 6 hours",
  "every 8 hours",
  "every 12 hours",
  "as needed",
  "at bedtime",
] as const;

/** Map a controlled-substance schedule string to a display badge color. */
function scheduleBadgeClass(schedule: string): string {
  if (schedule === "non-controlled") {
    return "bg-success/10 text-success border-success/20";
  }
  return "bg-destructive/10 text-destructive border-destructive/20";
}

function isControlledSchedule(schedule: string): boolean {
  return schedule !== "non-controlled";
}

// ── Page ─────────────────────────────────────────────────────────────────────

export function Pharmacy() {
  const { has } = useAuth();
  const canManageInventory = has(PERMISSIONS.InventoryManage);
  const canPrescribe = has(PERMISSIONS.PatientsCreate);

  return (
    <PageContainer>
      <PageHeader
        icon={Pill}
        title="Pharmacy"
        description="Medication catalog, prescriptions, and dispensing. Stock decrements are audited against inventory movements."
      />

      <MedicationCatalogSection canManage={canManageInventory} />
      <PrescriptionsSection canPrescribe={canPrescribe} canDispense={canManageInventory} />
    </PageContainer>
  );
}

// ── Medication catalog section ───────────────────────────────────────────────

function MedicationCatalogSection({ canManage }: { canManage: boolean }) {
  const [search, setSearch] = useState("");
  const [createOpen, setCreateOpen] = useState(false);
  const [editTarget, setEditTarget] = useState<Medication | null>(null);

  // Pass null (not undefined) when search is empty so the query key is
  // stable across re-renders.
  const { data: medications = [], isLoading } = useMedications(search.trim() || null);

  return (
    <SectionCard
      icon={Pill}
      title="Medication catalog"
      description="Brand / generic medications with form, strength, and controlled-substance schedule."
      action={
        canManage && (
          <Button onClick={() => setCreateOpen(true)} size="sm" className="gap-2">
            <Plus className="h-4 w-4" /> Add medication
          </Button>
        )
      }
    >
      {isLoading ? (
        <LoadingState rows={6} />
      ) : medications.length === 0 ? (
        <EmptyState
          icon={Pill}
          title="No medications"
          description="Add your first medication to the catalog, or adjust your search."
          action={
            canManage && (
              <Button onClick={() => setCreateOpen(true)} size="sm" className="gap-2">
                <Plus className="h-3.5 w-3.5" /> Add medication
              </Button>
            )
          }
        />
      ) : (
        <>
          <PageToolbar>
            <div className="relative w-full max-w-md">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
              <Input
                placeholder="Search brand or generic name…"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                className="h-10 pl-9"
              />
            </div>
            <span className="text-xs text-muted-foreground ml-auto tabular-nums">
              {medications.length} medication{medications.length === 1 ? "" : "s"}
              {medications.some((m) => !m.is_active) && (
                <>
                  {" · "}
                  <span className="text-muted-foreground">
                    {medications.filter((m) => !m.is_active).length} inactive
                  </span>
                </>
              )}
            </span>
          </PageToolbar>

          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead>Brand</TableHead>
                <TableHead>Generic</TableHead>
                <TableHead>Form</TableHead>
                <TableHead>Strength</TableHead>
                <TableHead>Schedule</TableHead>
                <TableHead>Category</TableHead>
                <TableHead>Status</TableHead>
                {canManage && <TableHead className="text-right">Actions</TableHead>}
              </TableRow>
            </TableHeader>
            <TableBody>
              {medications.map((m, i) => (
                <motion.tr
                  key={m.id}
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  transition={{ duration: 0.15, delay: Math.min(i * 0.02, 0.3) }}
                  className="border-b border-border/70 transition-colors hover:bg-muted/40"
                >
                  <TableCell className="font-semibold text-foreground">{m.brand_name}</TableCell>
                  <TableCell className="text-muted-foreground">{m.generic_name}</TableCell>
                  <TableCell className="capitalize">{m.form}</TableCell>
                  <TableCell className="font-mono text-xs">{m.strength}</TableCell>
                  <TableCell>
                    <span
                      className={`inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[11px] font-semibold ${scheduleBadgeClass(m.schedule)}`}
                    >
                      {isControlledSchedule(m.schedule) && (
                        <AlertTriangle className="h-3 w-3" />
                      )}
                      {m.schedule.replace("-", " ")}
                    </span>
                  </TableCell>
                  <TableCell className="text-muted-foreground">
                    {m.category ?? "—"}
                  </TableCell>
                  <TableCell>
                    {m.is_active ? (
                      <StatusBadge status="active" />
                    ) : (
                      <StatusBadge status="inactive" />
                    )}
                  </TableCell>
                  {canManage && (
                    <TableCell className="text-right">
                      <div className="flex justify-end gap-1">
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => setEditTarget(m)}
                          className="h-8 w-8 text-muted-foreground hover:text-foreground"
                          title="Edit medication"
                        >
                          <Edit className="h-4 w-4" />
                        </Button>
                      </div>
                    </TableCell>
                  )}
                </motion.tr>
              ))}
            </TableBody>
          </Table>
        </>
      )}

      {createOpen && (
        <MedicationFormDialog mode="create" onClose={() => setCreateOpen(false)} />
      )}
      {editTarget && (
        <MedicationFormDialog
          mode="edit"
          medication={editTarget}
          onClose={() => setEditTarget(null)}
        />
      )}
    </SectionCard>
  );
}

// ── Medication form dialog (create + edit) ───────────────────────────────────

interface MedicationFormState {
  brand_name: string;
  generic_name: string;
  form: string;
  strength: string;
  schedule: string;
  category: string;
  manufacturer: string;
  reorder_level: string;
  is_active: boolean;
}

const EMPTY_MED_FORM: MedicationFormState = {
  brand_name: "",
  generic_name: "",
  form: "tablet",
  strength: "",
  schedule: "non-controlled",
  category: "",
  manufacturer: "",
  reorder_level: "10",
  is_active: true,
};

function MedicationFormDialog({
  mode,
  medication,
  onClose,
}: {
  mode: "create" | "edit";
  medication?: Medication;
  onClose: () => void;
}) {
  const createMed = useCreateMedication();
  const updateMed = useUpdateMedication();
  const deleteMed = useDeleteMedication();
  const loading = createMed.isPending || updateMed.isPending;

  const [form, setForm] = useState<MedicationFormState>(
    medication
      ? {
          brand_name: medication.brand_name,
          generic_name: medication.generic_name,
          form: medication.form,
          strength: medication.strength,
          schedule: medication.schedule,
          category: medication.category ?? "",
          manufacturer: medication.manufacturer ?? "",
          reorder_level: String(medication.reorder_level),
          is_active: medication.is_active,
        }
      : EMPTY_MED_FORM,
  );
  const [confirmDelete, setConfirmDelete] = useState(false);

  const set = <K extends keyof MedicationFormState>(k: K, v: MedicationFormState[K]) =>
    setForm((f) => ({ ...f, [k]: v }));

  const handleSubmit = async () => {
    if (!form.brand_name.trim() || !form.generic_name.trim() || !form.strength.trim()) {
      return;
    }
    const reorder = Number(form.reorder_level) || 0;

    if (mode === "create") {
      const payload: CreateMedication = {
        brand_name: form.brand_name.trim(),
        generic_name: form.generic_name.trim(),
        form: form.form,
        strength: form.strength.trim(),
        schedule: form.schedule,
        category: form.category.trim() === "" ? null : form.category.trim(),
        manufacturer: form.manufacturer.trim() === "" ? null : form.manufacturer.trim(),
        reorder_level: reorder,
        is_active: form.is_active,
      };
      try {
        await createMed.mutateAsync(payload);
        onClose();
      } catch {
        /* toast already shown */
      }
    } else if (medication) {
      const payload: UpdateMedication = {
        id: medication.id,
        brand_name: form.brand_name.trim(),
        generic_name: form.generic_name.trim(),
        form: form.form,
        strength: form.strength.trim(),
        schedule: form.schedule,
        category: form.category.trim() === "" ? null : form.category.trim(),
        manufacturer: form.manufacturer.trim() === "" ? null : form.manufacturer.trim(),
        reorder_level: reorder,
        is_active: form.is_active,
      };
      try {
        await updateMed.mutateAsync({ id: medication.id, medication: payload });
        onClose();
      } catch {
        /* toast already shown */
      }
    }
  };

  const handleDelete = async () => {
    if (!medication) return;
    try {
      await deleteMed.mutateAsync(medication.id);
      onClose();
    } catch {
      /* toast already shown */
    }
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>
            {mode === "create" ? "Add medication" : "Edit medication"}
          </DialogTitle>
          <DialogDescription>
            {mode === "create"
              ? "Add a new medication to the catalog. Brand / generic / strength / schedule are required."
              : "Update medication details. Deactivating hides it from new-prescription dropdowns but preserves historical prescriptions."}
          </DialogDescription>
        </DialogHeader>

        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 py-2">
          <div className="space-y-1.5">
            <Label htmlFor="med-brand">Brand name *</Label>
            <Input
              id="med-brand"
              placeholder="Panadol"
              value={form.brand_name}
              onChange={(e) => set("brand_name", e.target.value)}
              disabled={loading}
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="med-generic">Generic name *</Label>
            <Input
              id="med-generic"
              placeholder="Paracetamol"
              value={form.generic_name}
              onChange={(e) => set("generic_name", e.target.value)}
              disabled={loading}
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="med-form">Form</Label>
            <Select
              value={form.form}
              onValueChange={(v) => set("form", v)}
              disabled={loading}
            >
              <SelectTrigger id="med-form">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {MEDICATION_FORMS.map((f) => (
                  <SelectItem key={f} value={f} className="capitalize">
                    {f}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="med-strength">Strength *</Label>
            <Input
              id="med-strength"
              placeholder="500mg"
              value={form.strength}
              onChange={(e) => set("strength", e.target.value)}
              disabled={loading}
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="med-schedule">Controlled-substance schedule</Label>
            <Select
              value={form.schedule}
              onValueChange={(v) => set("schedule", v)}
              disabled={loading}
            >
              <SelectTrigger id="med-schedule">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {MEDICATION_SCHEDULES.map((s) => (
                  <SelectItem key={s} value={s} className="capitalize">
                    {s.replace("-", " ")}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="med-reorder">Reorder level</Label>
            <Input
              id="med-reorder"
              type="number"
              inputMode="numeric"
              value={form.reorder_level}
              onChange={(e) => set("reorder_level", e.target.value)}
              disabled={loading}
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="med-category">Category</Label>
            <Input
              id="med-category"
              placeholder="Analgesic"
              value={form.category}
              onChange={(e) => set("category", e.target.value)}
              disabled={loading}
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="med-manufacturer">Manufacturer</Label>
            <Input
              id="med-manufacturer"
              placeholder="GSK"
              value={form.manufacturer}
              onChange={(e) => set("manufacturer", e.target.value)}
              disabled={loading}
            />
          </div>
          {mode === "edit" && (
            <div className="sm:col-span-2 flex items-center gap-3 p-3 rounded-[var(--radius-md)] bg-muted/40 border border-border select-none">
              <input
                id="med-active"
                type="checkbox"
                checked={form.is_active}
                onChange={(e) => set("is_active", e.target.checked)}
                disabled={loading}
                className="h-4 w-4 rounded-[var(--radius-sm)] border border-border accent-primary cursor-pointer focus:ring-2 focus:ring-primary/30"
              />
              <Label htmlFor="med-active" className="text-sm font-medium cursor-pointer">
                Active (medication appears in new-prescription dropdowns)
              </Label>
            </div>
          )}
        </div>

        <DialogFooter className="gap-2">
          {mode === "edit" && (
            <Button
              variant="destructive"
              onClick={() => setConfirmDelete(true)}
              disabled={loading}
              className="mr-auto gap-2"
            >
              <Trash2 className="h-4 w-4" /> Deactivate
            </Button>
          )}
          <DialogClose asChild>
            <Button variant="outline" disabled={loading}>
              Cancel
            </Button>
          </DialogClose>
          <Button
            onClick={handleSubmit}
            disabled={
              loading ||
              !form.brand_name.trim() ||
              !form.generic_name.trim() ||
              !form.strength.trim()
            }
          >
            {loading ? "Saving…" : mode === "create" ? "Add medication" : "Save changes"}
          </Button>
        </DialogFooter>
      </DialogContent>

      {confirmDelete && medication && (
        <Dialog open onOpenChange={(o) => !o && setConfirmDelete(false)}>
          <DialogContent className="max-w-md">
            <DialogHeader>
              <DialogTitle className="flex items-center gap-2 text-destructive">
                <AlertTriangle className="h-5 w-5" /> Deactivate medication?
              </DialogTitle>
              <DialogDescription>
                <strong>{medication.brand_name}</strong> ({medication.generic_name}){" "}
                will be marked inactive. Historical prescriptions remain readable; the
                medication will no longer appear in new-prescription dropdowns.
              </DialogDescription>
            </DialogHeader>
            <DialogFooter>
              <DialogClose asChild>
                <Button variant="outline">Cancel</Button>
              </DialogClose>
              <Button
                variant="destructive"
                onClick={handleDelete}
                disabled={deleteMed.isPending}
              >
                {deleteMed.isPending ? "Deactivating…" : "Deactivate"}
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      )}
    </Dialog>
  );
}

// ── Prescriptions section ────────────────────────────────────────────────────

function PrescriptionsSection({
  canPrescribe,
  canDispense,
}: {
  canPrescribe: boolean;
  canDispense: boolean;
}) {
  const [statusFilter, setStatusFilter] = useState<string>("");
  const [createOpen, setCreateOpen] = useState(false);
  const [detailId, setDetailId] = useState<number | null>(null);

  // Pass null when filter is empty so the query key stays stable.
  const { data: prescriptions = [], isLoading } = usePrescriptions(
    null,
    statusFilter || null,
  );

  return (
    <SectionCard
      icon={FileText}
      title="Prescriptions"
      description="Patient prescriptions with line items. Click a row to view items and dispense."
      action={
        canPrescribe && (
          <Button onClick={() => setCreateOpen(true)} size="sm" className="gap-2">
            <Plus className="h-4 w-4" /> New prescription
          </Button>
        )
      }
    >
      {isLoading ? (
        <LoadingState rows={5} />
      ) : prescriptions.length === 0 ? (
        <EmptyState
          icon={FileText}
          title="No prescriptions"
          description={
            canPrescribe
              ? "Create the first prescription to get started."
              : "Prescriptions will appear here once a doctor creates one."
          }
          action={
            canPrescribe && (
              <Button onClick={() => setCreateOpen(true)} size="sm" className="gap-2">
                <Plus className="h-3.5 w-3.5" /> New prescription
              </Button>
            )
          }
        />
      ) : (
        <>
          <PageToolbar>
            <Select
              value={statusFilter || "all"}
              onValueChange={(v) => setStatusFilter(v === "all" ? "" : v)}
            >
              <SelectTrigger className="w-[180px] h-10">
                <SelectValue placeholder="All statuses" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All statuses</SelectItem>
                {PRESCRIPTION_STATUSES.map((s) => (
                  <SelectItem key={s} value={s} className="capitalize">
                    {s}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <span className="text-xs text-muted-foreground ml-auto tabular-nums">
              {prescriptions.length} prescription{prescriptions.length === 1 ? "" : "s"}
            </span>
          </PageToolbar>

          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead>#</TableHead>
                <TableHead>Patient</TableHead>
                <TableHead>Doctor</TableHead>
                <TableHead>Created</TableHead>
                <TableHead>Status</TableHead>
                <TableHead className="text-right">Action</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {prescriptions.map((p, i) => (
                <motion.tr
                  key={p.id}
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  transition={{ duration: 0.15, delay: Math.min(i * 0.02, 0.3) }}
                  className="border-b border-border/70 transition-colors hover:bg-muted/40"
                >
                  <TableCell className="font-mono text-xs">#{p.id}</TableCell>
                  <TableCell className="font-medium">
                    {p.patient_name ?? `Patient #${p.patient_id}`}
                  </TableCell>
                  <TableCell className="text-muted-foreground">
                    {p.doctor_name ?? "—"}
                  </TableCell>
                  <TableCell className="text-xs text-muted-foreground">
                    {new Date(p.created_at).toLocaleString()}
                  </TableCell>
                  <TableCell>
                    <StatusBadge status={p.status} />
                  </TableCell>
                  <TableCell className="text-right">
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => setDetailId(p.id)}
                      className="gap-2"
                    >
                      <Eye className="h-4 w-4" /> View
                    </Button>
                  </TableCell>
                </motion.tr>
              ))}
            </TableBody>
          </Table>
        </>
      )}

      {createOpen && <CreatePrescriptionDialog onClose={() => setCreateOpen(false)} />}
      {detailId != null && (
        <PrescriptionDetailDialog
          id={detailId}
          canDispense={canDispense}
          onClose={() => setDetailId(null)}
        />
      )}
    </SectionCard>
  );
}

// ── Create prescription dialog ───────────────────────────────────────────────

interface PrescriptionItemDraft {
  medication_id: number | null;
  medication_name: string;
  dose: string;
  route: string;
  frequency: string;
  duration: string;
  quantity: string;
}

function emptyItemDraft(): PrescriptionItemDraft {
  return {
    medication_id: null,
    medication_name: "",
    dose: "",
    route: "oral",
    frequency: "twice daily",
    duration: "7 days",
    quantity: "1",
  };
}

function CreatePrescriptionDialog({ onClose }: { onClose: () => void }) {
  const { data: patients = [] } = usePatientsEhr(null);
  const { data: doctors = [] } = useDoctors(true);
  const { data: medications = [] } = useMedications(null);
  const createRx = useCreatePrescription();

  const [patientId, setPatientId] = useState<number | null>(null);
  const [doctorId, setDoctorId] = useState<number | null>(null);
  const [notes, setNotes] = useState("");
  const [items, setItems] = useState<PrescriptionItemDraft[]>([emptyItemDraft()]);

  const setItem = (idx: number, patch: Partial<PrescriptionItemDraft>) =>
    setItems((arr) => arr.map((it, i) => (i === idx ? { ...it, ...patch } : it)));

  const removeItem = (idx: number) =>
    setItems((arr) => arr.filter((_, i) => i !== idx));

  const addItem = () => setItems((arr) => [...arr, emptyItemDraft()]);

  // When a medication is selected from the dropdown, snapshot its name
  // into medication_name (the backend also reads medication_name to know
  // what to display, and uses medication_id to look up the schedule for
  // the controlled-substance flag).
  const selectMedication = (idx: number, medId: number) => {
    const med = medications.find((m) => m.id === medId);
    if (med) {
      setItem(idx, {
        medication_id: med.id,
        medication_name: `${med.brand_name} ${med.strength} (${med.generic_name})`,
      });
    }
  };

  const canSubmit =
    patientId != null &&
    items.length > 0 &&
    items.every(
      (it) =>
        it.medication_name.trim() !== "" &&
        it.dose.trim() !== "" &&
        it.frequency.trim() !== "",
    );

  const handleSubmit = async () => {
    if (!canSubmit || patientId == null) return;
    const payload: CreatePrescription = {
      patient_id: patientId,
      doctor_id: doctorId,
      encounter_id: null,
      notes: notes.trim() === "" ? null : notes.trim(),
      items: items.map<CreatePrescriptionItem>((it) => ({
        medication_id: it.medication_id,
        medication_name: it.medication_name.trim(),
        dose: it.dose.trim(),
        route: it.route,
        frequency: it.frequency.trim(),
        duration: it.duration.trim() === "" ? null : it.duration.trim(),
        quantity: Number(it.quantity) || 1,
      })),
    };
    try {
      await createRx.mutateAsync(payload);
      onClose();
    } catch {
      /* toast already shown */
    }
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-3xl">
        <DialogHeader>
          <DialogTitle>New prescription</DialogTitle>
          <DialogDescription>
            Select a patient and one or more medications with dose, route, frequency, and
            duration. Controlled-substance items will be flagged automatically based on
            the medication's catalog schedule.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-2 max-h-[70vh] overflow-y-auto pr-1">
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <div className="space-y-1.5">
              <Label>Patient *</Label>
              <Select
                value={patientId?.toString() ?? ""}
                onValueChange={(v) => setPatientId(Number(v))}
              >
                <SelectTrigger>
                  <SelectValue placeholder="Select patient" />
                </SelectTrigger>
                <SelectContent>
                  {patients.map((p: PatientEhr) => (
                    <SelectItem key={p.id} value={p.id.toString()}>
                      {p.first_name} {p.last_name} · {p.phone}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-1.5">
              <Label>Prescribing doctor (optional)</Label>
              <Select
                value={doctorId?.toString() ?? "none"}
                onValueChange={(v) => setDoctorId(v === "none" ? null : Number(v))}
              >
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="none">—</SelectItem>
                  {doctors
                    .filter((d: Doctor) => d.is_active)
                    .map((d: Doctor) => (
                      <SelectItem key={d.id} value={d.id.toString()}>
                        Dr. {d.first_name} {d.last_name} · {d.specialization}
                      </SelectItem>
                    ))}
                </SelectContent>
              </Select>
            </div>
          </div>

          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <Label>Medication items</Label>
              <Button type="button" variant="outline" size="sm" onClick={addItem} className="gap-2">
                <Plus className="h-3.5 w-3.5" /> Add item
              </Button>
            </div>

            {items.map((it, idx) => (
              <div
                key={idx}
                className="rounded-[var(--radius-md)] border border-border bg-card p-4 space-y-3"
              >
                <div className="flex items-start justify-between gap-2">
                  <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Item #{idx + 1}
                  </span>
                  {items.length > 1 && (
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon"
                      className="h-7 w-7 text-muted-foreground hover:text-destructive"
                      onClick={() => removeItem(idx)}
                      title="Remove item"
                    >
                      <X className="h-3.5 w-3.5" />
                    </Button>
                  )}
                </div>
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                  <div className="space-y-1.5 sm:col-span-2">
                    <Label>Medication *</Label>
                    <div className="flex gap-2">
                      <Select
                        value={it.medication_id?.toString() ?? ""}
                        onValueChange={(v) => selectMedication(idx, Number(v))}
                      >
                        <SelectTrigger className="flex-1">
                          <SelectValue placeholder="Pick from catalog (or type below)" />
                        </SelectTrigger>
                        <SelectContent>
                          {medications
                            .filter((m: Medication) => m.is_active)
                            .map((m: Medication) => (
                              <SelectItem key={m.id} value={m.id.toString()}>
                                {m.brand_name} {m.strength} · {m.generic_name}
                                {isControlledSchedule(m.schedule) ? " ⚠" : ""}
                              </SelectItem>
                            ))}
                        </SelectContent>
                      </Select>
                    </div>
                    <Input
                      placeholder="…or type medication name manually"
                      value={it.medication_name}
                      onChange={(e) =>
                        setItem(idx, {
                          medication_name: e.target.value,
                          medication_id: null,
                        })
                      }
                    />
                  </div>
                  <div className="space-y-1.5">
                    <Label>Dose *</Label>
                    <Input
                      placeholder="1 tablet"
                      value={it.dose}
                      onChange={(e) => setItem(idx, { dose: e.target.value })}
                    />
                  </div>
                  <div className="space-y-1.5">
                    <Label>Route</Label>
                    <Select
                      value={it.route}
                      onValueChange={(v) => setItem(idx, { route: v })}
                    >
                      <SelectTrigger>
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        {ROUTES.map((r) => (
                          <SelectItem key={r} value={r}>
                            {r}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                  <div className="space-y-1.5">
                    <Label>Frequency *</Label>
                    <Select
                      value={it.frequency}
                      onValueChange={(v) => setItem(idx, { frequency: v })}
                    >
                      <SelectTrigger>
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        {FREQUENCIES.map((f) => (
                          <SelectItem key={f} value={f}>
                            {f}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                  <div className="space-y-1.5">
                    <Label>Duration</Label>
                    <Input
                      placeholder="7 days"
                      value={it.duration}
                      onChange={(e) => setItem(idx, { duration: e.target.value })}
                    />
                  </div>
                  <div className="space-y-1.5">
                    <Label>Quantity to dispense</Label>
                    <Input
                      type="number"
                      inputMode="numeric"
                      min={1}
                      value={it.quantity}
                      onChange={(e) => setItem(idx, { quantity: e.target.value })}
                    />
                  </div>
                </div>
              </div>
            ))}
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="rx-notes">Notes (optional)</Label>
            <Textarea
              id="rx-notes"
              placeholder="Free-text notes — e.g. take with food, avoid alcohol"
              rows={2}
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              className="resize-none"
            />
          </div>
        </div>

        <DialogFooter>
          <DialogClose asChild>
            <Button variant="outline" disabled={createRx.isPending}>
              Cancel
            </Button>
          </DialogClose>
          <Button onClick={handleSubmit} disabled={!canSubmit || createRx.isPending}>
            {createRx.isPending ? "Creating…" : `Create prescription (${items.length} item${items.length === 1 ? "" : "s"})`}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// ── Prescription detail dialog ───────────────────────────────────────────────

function PrescriptionDetailDialog({
  id,
  canDispense,
  onClose,
}: {
  id: number;
  canDispense: boolean;
  onClose: () => void;
}) {
  const { data, isLoading } = usePrescription(id);
  const dispense = useDispensePrescriptionItem();
  const [pendingItemId, setPendingItemId] = useState<number | null>(null);

  // The dialog opens with `id` set; while loading we show the skeleton.
  // Once loaded, `data` is `PrescriptionWithItems` (header fields + items[]).
  const rx = data;
  const items: PrescriptionItem[] = rx?.items ?? [];

  const handleDispense = async (itemId: number) => {
    try {
      await dispense.mutateAsync(itemId);
      setPendingItemId(null);
    } catch {
      /* toast already shown */
    }
  };

  // For a controlled item, this opens the confirm dialog (FR-0123). For
  // a non-controlled item, dispense immediately. The backend re-checks
  // permission; the UI gate is an affordance, not a security control.
  const startDispense = (item: PrescriptionItem) => {
    if (item.is_controlled) {
      setPendingItemId(item.id);
    } else {
      void handleDispense(item.id);
    }
  };

  const pendingItem = items.find((it) => it.id === pendingItemId) ?? null;

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-3xl">
        <DialogHeader>
          <DialogTitle>Prescription #{id}</DialogTitle>
          <DialogDescription>
            {rx ? (
              <>
                <strong>{rx.patient_name ?? `Patient #${rx.patient_id}`}</strong>
                {rx.doctor_name ? ` · prescribed by Dr. ${rx.doctor_name}` : ""}
                {` · ${new Date(rx.created_at).toLocaleString()}`}
              </>
            ) : (
              "Loading prescription…"
            )}
          </DialogDescription>
        </DialogHeader>

        {isLoading || !rx ? (
          <LoadingState rows={4} />
        ) : (
          <>
            {rx.notes && (
              <div className="rounded-[var(--radius-md)] border border-border bg-muted/40 px-4 py-3 text-sm">
                <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Notes
                </span>
                <p className="mt-1 text-foreground">{rx.notes}</p>
              </div>
            )}

            <div className="rounded-[var(--radius-md)] border border-border overflow-hidden">
              <Table>
                <TableHeader>
                  <TableRow className="hover:bg-transparent">
                    <TableHead>Medication</TableHead>
                    <TableHead>Dose</TableHead>
                    <TableHead>Route</TableHead>
                    <TableHead>Frequency</TableHead>
                    <TableHead>Duration</TableHead>
                    <TableHead className="text-right">Qty</TableHead>
                    <TableHead className="text-right">Status</TableHead>
                    {canDispense && <TableHead className="text-right">Action</TableHead>}
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {items.map((it) => (
                    <TableRow key={it.id}>
                      <TableCell className="font-medium">
                        <div className="flex flex-col">
                          <span>{it.medication_name}</span>
                          {it.is_controlled && (
                            <Badge
                              variant="outline"
                              className="mt-1 w-fit bg-destructive/10 text-destructive border-destructive/20 text-[10px] gap-1 px-1.5 py-0"
                            >
                              <AlertTriangle className="h-2.5 w-2.5" />
                              Controlled
                            </Badge>
                          )}
                        </div>
                      </TableCell>
                      <TableCell className="text-xs">{it.dose}</TableCell>
                      <TableCell className="text-xs">{it.route}</TableCell>
                      <TableCell className="text-xs">{it.frequency}</TableCell>
                      <TableCell className="text-xs text-muted-foreground">
                        {it.duration ?? "—"}
                      </TableCell>
                      <TableCell className="text-right tabular-nums">{it.quantity}</TableCell>
                      <TableCell className="text-right">
                        {it.dispensed ? (
                          <span className="inline-flex items-center gap-1 text-xs text-success font-medium">
                            <CheckCircle2 className="h-3.5 w-3.5" />
                            {it.dispensed_at
                              ? new Date(it.dispensed_at).toLocaleDateString()
                              : "Dispensed"}
                          </span>
                        ) : (
                          <span className="text-xs text-warning font-medium">Pending</span>
                        )}
                      </TableCell>
                      {canDispense && (
                        <TableCell className="text-right">
                          <Button
                            size="sm"
                            variant={it.dispensed ? "outline" : "default"}
                            disabled={it.dispensed || dispense.isPending}
                            onClick={() => startDispense(it)}
                            className="gap-1.5"
                          >
                            {it.dispensed ? (
                              <>
                                <CheckCircle2 className="h-3.5 w-3.5" /> Done
                              </>
                            ) : (
                              <>
                                <Pill className="h-3.5 w-3.5" /> Dispense
                              </>
                            )}
                          </Button>
                        </TableCell>
                      )}
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>

            <div className="flex items-center justify-between text-xs text-muted-foreground">
              <span>
                {items.filter((i) => i.dispensed).length} of {items.length} item
                {items.length === 1 ? "" : "s"} dispensed
              </span>
              <span>Status: <StatusBadge status={rx.status} /></span>
            </div>
          </>
        )}

        <DialogFooter>
          <DialogClose asChild>
            <Button variant="outline">Close</Button>
          </DialogClose>
        </DialogFooter>
      </DialogContent>

      {pendingItem && (
        <Dialog open onOpenChange={(o) => !o && setPendingItemId(null)}>
          <DialogContent className="max-w-md">
            <DialogHeader>
              <DialogTitle className="flex items-center gap-2 text-destructive">
                <AlertTriangle className="h-5 w-5" /> Controlled substance
              </DialogTitle>
              <DialogDescription>
                <strong>{pendingItem.medication_name}</strong> is a controlled substance.
                Confirm you are a licensed pharmacist authorising this dispense. This
                action is audit-logged.
              </DialogDescription>
            </DialogHeader>
            <div className="rounded-[var(--radius-md)] border border-border bg-muted/40 px-4 py-3 text-xs space-y-1">
              <div>
                <span className="text-muted-foreground">Dose:</span>{" "}
                <span className="font-medium">{pendingItem.dose}</span>
              </div>
              <div>
                <span className="text-muted-foreground">Frequency:</span>{" "}
                <span className="font-medium">{pendingItem.frequency}</span>
              </div>
              <div>
                <span className="text-muted-foreground">Quantity:</span>{" "}
                <span className="font-medium">{pendingItem.quantity}</span>
              </div>
            </div>
            <DialogFooter>
              <DialogClose asChild>
                <Button variant="outline">Cancel</Button>
              </DialogClose>
              <Button
                variant="default"
                onClick={() => void handleDispense(pendingItem.id)}
                disabled={dispense.isPending}
                className="gap-2"
              >
                {dispense.isPending ? "Dispensing…" : "Confirm dispense"}
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      )}
    </Dialog>
  );
}
