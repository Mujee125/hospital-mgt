/**
 * In-Patient Department (IPD) — bed board, admissions, ward/bed management.
 *
 * v0.2.1 fix: added ward + bed creation UI (was missing — the backend commands
 * existed but there was no frontend way to call them, so the admit dialog's
 * ward/bed dropdowns were always empty).
 */
import { useState } from "react";
import { BedDouble, Plus, LogOut, Loader2, Building2, Bed as BedIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter, DialogClose } from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import {
  useWards, useBeds, useAdmissions, useAdmitPatient, useDischargePatient,
  usePatientsEhr, useDoctors, useCreateWard, useCreateBed,
} from "@/lib/queries";
import { useAuth } from "@/lib/auth";
import { PERMISSIONS } from "@/lib/rbac";
import { PageContainer, PageHeader, SectionCard, EmptyState, StatusBadge, LoadingState, PageToolbar } from "@/components/layout/shared";

export function IPD() {
  const { has } = useAuth();
  const { data: wards = [] } = useWards();
  const { data: beds = [] } = useBeds();
  const { data: admissions = [], isLoading } = useAdmissions();
  const { data: patients = [] } = usePatientsEhr();
  const { data: doctors = [] } = useDoctors();
  const admit = useAdmitPatient();
  const discharge = useDischargePatient();
  const createWard = useCreateWard();
  const createBed = useCreateBed();

  // Admit dialog state
  const [open, setOpen] = useState(false);
  const [dischargeId, setDischargeId] = useState<number | null>(null);
  const [summary, setSummary] = useState("");
  const [form, setForm] = useState({
    patientId: null as number | null,
    wardId: null as number | null,
    bedId: null as number | null,
    doctorId: null as number | null,
    diagnosis: "",
  });

  // Ward/bed management dialog state
  const [wardDialogOpen, setWardDialogOpen] = useState(false);
  const [bedDialogOpen, setBedDialogOpen] = useState(false);
  const [wardForm, setWardForm] = useState({ name: "", code: "", floor: "", genderRestriction: "" });
  const [bedForm, setBedForm] = useState({ wardId: null as number | null, bedNumber: "", isIcu: false, dailyRate: "" });

  const availableBeds = beds.filter(
    (b) => b.status === "available" && (form.wardId == null || b.ward_id === form.wardId),
  );

  const submit = async () => {
    if (!form.patientId || !form.wardId || !form.bedId) return;
    await admit.mutateAsync({
      patient_id: form.patientId,
      ward_id: form.wardId,
      bed_id: form.bedId,
      doctor_id: form.doctorId,
      admitting_diagnosis: form.diagnosis || null,
    });
    setOpen(false);
    setForm({ patientId: null, wardId: null, bedId: null, doctorId: null, diagnosis: "" });
  };

  const doDischarge = async () => {
    if (dischargeId == null) return;
    await discharge.mutateAsync({ id: dischargeId, discharge_summary: summary || null });
    setDischargeId(null);
    setSummary("");
  };

  const submitWard = async () => {
    if (!wardForm.name.trim() || !wardForm.code.trim()) return;
    await createWard.mutateAsync({
      name: wardForm.name.trim(),
      code: wardForm.code.trim(),
      floor: wardForm.floor.trim() || null,
      genderRestriction: wardForm.genderRestriction.trim() || null,
    });
    setWardDialogOpen(false);
    setWardForm({ name: "", code: "", floor: "", genderRestriction: "" });
  };

  const submitBed = async () => {
    if (!bedForm.wardId || !bedForm.bedNumber.trim()) return;
    await createBed.mutateAsync({
      wardId: bedForm.wardId,
      bedNumber: bedForm.bedNumber.trim(),
      isIcu: bedForm.isIcu,
      dailyRate: bedForm.dailyRate.trim() ? parseFloat(bedForm.dailyRate) : null,
    });
    setBedDialogOpen(false);
    setBedForm({ wardId: null, bedNumber: "", isIcu: false, dailyRate: "" });
  };

  return (
    <PageContainer>
      <PageHeader
        icon={BedDouble}
        title="In-Patient Department"
        description="Admissions, bed allocation & discharge"
        actions={has(PERMISSIONS.IpdManage) && (
          <>
            <Button variant="outline" onClick={() => setWardDialogOpen(true)}>
              <Building2 className="h-4 w-4" /> New ward
            </Button>
            <Button variant="outline" onClick={() => setBedDialogOpen(true)} disabled={wards.length === 0}>
              <BedIcon className="h-4 w-4" /> New bed
            </Button>
            <Button onClick={() => setOpen(true)} disabled={beds.length === 0}>
              <Plus className="h-4 w-4" /> Admit patient
            </Button>
          </>
        )}
      />

      {/* Bed board */}
      <SectionCard icon={BedDouble} title="Bed board">
        <div className="p-6">
          <div className="grid grid-cols-2 sm:grid-cols-4 lg:grid-cols-6 gap-3">
            {beds.map((b) => {
              const isAvail = b.status === "available";
              return (
                <div
                  key={b.id}
                  className={`border p-3 rounded-[var(--radius-md)] transition-all ${
                    isAvail ? "border-success/30 bg-success/5" : "border-border bg-card"
                  }`}
                >
                  <div className="flex items-center justify-between">
                    <span className="font-mono font-bold text-sm">{b.bed_number}</span>
                    {b.is_icu && <Badge className="bg-accent/15 text-accent text-[9px]">ICU</Badge>}
                  </div>
                  <div className="text-[10px] text-muted-foreground mt-1">
                    {wards.find((w) => w.id === b.ward_id)?.name ?? "—"}
                  </div>
                  <div className="mt-2">
                    <StatusBadge status={b.status} />
                  </div>
                </div>
              );
            })}
            {beds.length === 0 && (
              <div className="col-span-full">
                <EmptyState
                  icon={BedDouble}
                  title="No beds configured"
                  description={
                    wards.length === 0
                      ? "Create a ward first, then add beds to start managing IPD."
                      : "Add beds to your wards to start managing IPD."
                  }
                />
              </div>
            )}
          </div>
        </div>
      </SectionCard>

      {/* Ward summary (only if wards exist) */}
      {wards.length > 0 && (
        <SectionCard icon={Building2} title="Wards">
          <div className="p-6">
            <Table>
              <TableHeader>
                <TableRow className="border-border hover:bg-transparent">
                  <TableHead scope="col">Name</TableHead>
                  <TableHead scope="col">Code</TableHead>
                  <TableHead scope="col">Floor</TableHead>
                  <TableHead scope="col">Beds</TableHead>
                  <TableHead scope="col">Available</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {wards.map((w) => {
                  const wardBeds = beds.filter((b) => b.ward_id === w.id);
                  const avail = wardBeds.filter((b) => b.status === "available").length;
                  return (
                    <TableRow key={w.id}>
                      <TableCell className="font-medium">{w.name}</TableCell>
                      <TableCell className="font-mono text-xs">{w.code}</TableCell>
                      <TableCell className="text-muted-foreground">{w.floor ?? "—"}</TableCell>
                      <TableCell>{wardBeds.length}</TableCell>
                      <TableCell>
                        <span className={avail > 0 ? "text-success font-medium" : "text-muted-foreground"}>
                          {avail}
                        </span>
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          </div>
        </SectionCard>
      )}

      {/* Admissions */}
      <SectionCard icon={BedDouble} title="Current admissions">
        {isLoading ? (
          <LoadingState rows={5} />
        ) : admissions.length === 0 ? (
          <EmptyState icon={BedDouble} title="No admissions recorded" description="Admitted patients will appear here." />
        ) : (
          <>
            <PageToolbar>
              <span className="text-sm font-medium text-muted-foreground">
                {admissions.filter((a) => a.status === "admitted").length} currently admitted
              </span>
            </PageToolbar>
            <Table>
              <TableHeader>
                <TableRow className="border-border hover:bg-transparent">
                  <TableHead scope="col">Patient</TableHead>
                  <TableHead scope="col">Ward / Bed</TableHead>
                  <TableHead scope="col">Practitioner</TableHead>
                  <TableHead scope="col">Admitted</TableHead>
                  <TableHead scope="col">Diagnosis</TableHead>
                  <TableHead scope="col" className="text-right">Status</TableHead>
                  {has(PERMISSIONS.IpdManage) && <TableHead scope="col" className="text-right">Action</TableHead>}
                </TableRow>
              </TableHeader>
              <TableBody>
                {admissions.map((a) => (
                  <TableRow key={a.id}>
                    <TableCell className="font-medium">{a.patient_name ?? "—"}</TableCell>
                    <TableCell>{a.ward_name} · {a.bed_number}</TableCell>
                    <TableCell className="text-muted-foreground">{a.doctor_name ?? "—"}</TableCell>
                    <TableCell className="text-xs text-muted-foreground">
                      {new Date(a.admission_date).toLocaleDateString()}
                    </TableCell>
                    <TableCell className="text-xs text-muted-foreground max-w-[200px] truncate">
                      {a.admitting_diagnosis ?? "—"}
                    </TableCell>
                    <TableCell className="text-right"><StatusBadge status={a.status} /></TableCell>
                    {has(PERMISSIONS.IpdManage) && (
                      <TableCell className="text-right">
                        {a.status === "admitted" && (
                          <Button
                            size="sm"
                            variant="ghost"
                            className="h-8 gap-1"
                            onClick={() => setDischargeId(a.id)}
                          >
                            <LogOut className="h-3.5 w-3.5" /> Discharge
                          </Button>
                        )}
                      </TableCell>
                    )}
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </>
        )}
      </SectionCard>

      {/* Admit dialog */}
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Admit patient</DialogTitle>
            <DialogDescription>
              Select a patient, ward, bed, and optionally an attending doctor to begin a new admission.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-2">
            <div className="space-y-1.5">
              <Label htmlFor="ipd-patient">Patient</Label>
              <Select
                value={form.patientId?.toString() ?? ""}
                onValueChange={(v) => setForm({ ...form, patientId: Number(v) })}
              >
                <SelectTrigger id="ipd-patient">
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
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1.5">
                <Label htmlFor="ipd-ward">Ward</Label>
                <Select
                  value={form.wardId?.toString() ?? ""}
                  onValueChange={(v) => setForm({ ...form, wardId: Number(v), bedId: null })}
                >
                  <SelectTrigger id="ipd-ward">
                    <SelectValue placeholder="Select ward" />
                  </SelectTrigger>
                  <SelectContent>
                    {wards.map((w) => (
                      <SelectItem key={w.id} value={w.id.toString()}>
                        {w.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="ipd-bed">Bed</Label>
                <Select
                  value={form.bedId?.toString() ?? ""}
                  onValueChange={(v) => setForm({ ...form, bedId: Number(v) })}
                  disabled={!form.wardId}
                >
                  <SelectTrigger id="ipd-bed">
                    <SelectValue placeholder={form.wardId ? "Available beds" : "Select ward first"} />
                  </SelectTrigger>
                  <SelectContent>
                    {availableBeds.map((b) => (
                      <SelectItem key={b.id} value={b.id.toString()}>
                        {b.bed_number}
                        {b.is_icu ? " (ICU)" : ""}
                      </SelectItem>
                    ))}
                    {availableBeds.length === 0 && form.wardId && (
                      <div className="px-2 py-4 text-center text-sm text-muted-foreground">
                        No available beds in this ward.
                      </div>
                    )}
                  </SelectContent>
                </Select>
              </div>
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="ipd-doctor">Attending doctor (optional)</Label>
              <Select
                value={form.doctorId?.toString() ?? "none"}
                onValueChange={(v) => setForm({ ...form, doctorId: v === "none" ? null : Number(v) })}
              >
                <SelectTrigger id="ipd-doctor">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="none">—</SelectItem>
                  {doctors
                    .filter((d) => d.is_active)
                    .map((d) => (
                      <SelectItem key={d.id} value={d.id.toString()}>
                        Dr. {d.first_name} {d.last_name}
                      </SelectItem>
                    ))}
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="ipd-diagnosis">Admitting diagnosis</Label>
              <Input
                id="ipd-diagnosis"
                value={form.diagnosis}
                onChange={(e) => setForm({ ...form, diagnosis: e.target.value })}
              />
            </div>
          </div>
          <DialogFooter>
            <DialogClose asChild>
              <Button variant="outline">Cancel</Button>
            </DialogClose>
            <Button
              disabled={!form.patientId || !form.wardId || !form.bedId || admit.isPending}
              onClick={submit}
            >
              {admit.isPending ? "Admitting…" : "Admit"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Create ward dialog */}
      <Dialog open={wardDialogOpen} onOpenChange={setWardDialogOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>New ward</DialogTitle>
            <DialogDescription>
              Create a ward (e.g. General, ICU, Pediatrics). Beds are added to wards individually.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-2">
            <div className="space-y-1.5">
              <Label htmlFor="ward-name">Ward name</Label>
              <Input
                id="ward-name"
                placeholder="e.g. General Ward"
                value={wardForm.name}
                onChange={(e) => setWardForm({ ...wardForm, name: e.target.value })}
              />
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1.5">
                <Label htmlFor="ward-code">Code</Label>
                <Input
                  id="ward-code"
                  placeholder="e.g. GEN"
                  value={wardForm.code}
                  onChange={(e) => setWardForm({ ...wardForm, code: e.target.value })}
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="ward-floor">Floor (optional)</Label>
                <Input
                  id="ward-floor"
                  placeholder="e.g. 2nd"
                  value={wardForm.floor}
                  onChange={(e) => setWardForm({ ...wardForm, floor: e.target.value })}
                />
              </div>
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="ward-gender">Gender restriction (optional)</Label>
              <Select
                value={wardForm.genderRestriction || "none"}
                onValueChange={(v) =>
                  setWardForm({ ...wardForm, genderRestriction: v === "none" ? "" : v })
                }
              >
                <SelectTrigger id="ward-gender">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="none">None (all genders)</SelectItem>
                  <SelectItem value="male">Male only</SelectItem>
                  <SelectItem value="female">Female only</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
          <DialogFooter>
            <DialogClose asChild>
              <Button variant="outline">Cancel</Button>
            </DialogClose>
            <Button
              disabled={!wardForm.name.trim() || !wardForm.code.trim() || createWard.isPending}
              onClick={submitWard}
            >
              {createWard.isPending ? "Creating…" : "Create ward"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Create bed dialog */}
      <Dialog open={bedDialogOpen} onOpenChange={setBedDialogOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>New bed</DialogTitle>
            <DialogDescription>
              Add a bed to a ward. The bed starts in "available" status.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-2">
            <div className="space-y-1.5">
              <Label htmlFor="bed-ward">Ward</Label>
              <Select
                value={bedForm.wardId?.toString() ?? ""}
                onValueChange={(v) => setBedForm({ ...bedForm, wardId: Number(v) })}
              >
                <SelectTrigger id="bed-ward">
                  <SelectValue placeholder="Select ward" />
                </SelectTrigger>
                <SelectContent>
                  {wards.map((w) => (
                    <SelectItem key={w.id} value={w.id.toString()}>
                      {w.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1.5">
                <Label htmlFor="bed-number">Bed number</Label>
                <Input
                  id="bed-number"
                  placeholder="e.g. B-01"
                  value={bedForm.bedNumber}
                  onChange={(e) => setBedForm({ ...bedForm, bedNumber: e.target.value })}
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="bed-rate">Daily rate (Rs, optional)</Label>
                <Input
                  id="bed-rate"
                  type="number"
                  placeholder="e.g. 5000"
                  value={bedForm.dailyRate}
                  onChange={(e) => setBedForm({ ...bedForm, dailyRate: e.target.value })}
                />
              </div>
            </div>
            <div className="flex items-center gap-2">
              <input
                type="checkbox"
                id="bed-icu"
                checked={bedForm.isIcu}
                onChange={(e) => setBedForm({ ...bedForm, isIcu: e.target.checked })}
                className="h-4 w-4 rounded border-border"
              />
              <Label htmlFor="bed-icu" className="text-sm cursor-pointer">
                ICU bed (higher acuity)
              </Label>
            </div>
          </div>
          <DialogFooter>
            <DialogClose asChild>
              <Button variant="outline">Cancel</Button>
            </DialogClose>
            <Button
              disabled={!bedForm.wardId || !bedForm.bedNumber.trim() || createBed.isPending}
              onClick={submitBed}
            >
              {createBed.isPending ? "Creating…" : "Create bed"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Discharge dialog */}
      <Dialog open={dischargeId != null} onOpenChange={(o) => !o && setDischargeId(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Discharge patient</DialogTitle>
            <DialogDescription>
              Record a discharge summary (optional) and confirm to release the bed. This cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-2 py-2">
            <Label htmlFor="discharge-summary">Discharge summary (optional)</Label>
            <Textarea
              id="discharge-summary"
              rows={4}
              value={summary}
              onChange={(e) => setSummary(e.target.value)}
              placeholder="Outcome, medications, follow-up…"
            />
          </div>
          <DialogFooter>
            <DialogClose asChild>
              <Button variant="outline">Cancel</Button>
            </DialogClose>
            <Button disabled={discharge.isPending} onClick={doDischarge}>
              {discharge.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : "Confirm discharge"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </PageContainer>
  );
}
