/**
 * Nursing Station (SRS §2.7 — Phase 6.1).
 *
 * Ward workflow for nurses: pick a currently-admitted patient, then
 *   - Vitals tab: record a reading (any subset of the 7 signs) + last-7 trend
 *   - Notes tab: shift/observation/handover notes, newest first
 *   - MAR tab: medication administration record (administered/held/refused)
 *     for the patient's active prescriptions
 *
 * The backend enforces the clinical guards (active admission, ≥1 vital,
 * cross-patient MAR protection); this page is presentation + input only.
 */
import { useState } from "react";
import { HeartPulse, Loader2, Plus, StickyNote, Activity, Pill } from "lucide-react";
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
  useAdmissions, useVitalsTrend, useRecordVitals,
  useNurseNotes, useCreateNurseNote,
  useMedicationAdministrations, useRecordMedicationAdministration,
  usePrescriptions, usePrescription,
} from "@/lib/queries";
import { useAuth } from "@/lib/auth";
import { PERMISSIONS } from "@/lib/rbac";
import { PageContainer, PageHeader, SectionCard, EmptyState, ErrorState, StatusBadge, LoadingState, PageToolbar } from "@/components/layout/shared";

/**
 * Vitals entry fields: key → { label, placeholder, plausible range }.
 * RCTF F-22: min/max mirror the backend's plausibility guard exactly so
 * the form blocks missed-decimal entries (temp 370, SpO₂ 9.8) before the
 * round-trip; the backend remains the enforcement boundary.
 */
const VITAL_FIELDS = [
  { key: "temperature_c", label: "Temperature (°C)", placeholder: "37.0", min: 30, max: 43 },
  { key: "systolic_bp", label: "Systolic BP", placeholder: "120", min: 50, max: 260 },
  { key: "diastolic_bp", label: "Diastolic BP", placeholder: "80", min: 30, max: 150 },
  { key: "pulse_bpm", label: "Pulse (bpm)", placeholder: "72", min: 20, max: 250 },
  { key: "resp_rate", label: "Resp. rate", placeholder: "16", min: 4, max: 60 },
  { key: "spo2_pct", label: "SpO₂ (%)", placeholder: "98", min: 50, max: 100 },
  { key: "pain_score", label: "Pain score (0–10)", placeholder: "0", min: 0, max: 10 },
] as const;

type VitalFieldKey = (typeof VITAL_FIELDS)[number]["key"];

export function Nursing() {
  const { has } = useAuth();
  const { data: admissions = [], isLoading, isError, refetch, isFetching } = useAdmissions("admitted");
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const selected = admissions.find((a) => a.id === selectedId) ?? null;

  const canManage = has(PERMISSIONS.IpdManage);

  return (
    <PageContainer>
      <PageHeader
        icon={HeartPulse}
        title="Nursing Station"
        description="Vitals, shift notes & medication administration for admitted patients"
      />

      {!selected ? (
        <SectionCard icon={HeartPulse} title="Admitted patients">
          {isLoading ? (
            <LoadingState rows={5} />
          ) : isError ? (
            <ErrorState onRetry={() => void refetch()} retrying={isFetching} />
          ) : admissions.length === 0 ? (
            <EmptyState
              icon={HeartPulse}
              title="No admitted patients"
              description="Patients admitted via the In-Patient page will appear here for ward care."
            />
          ) : (
            <>
              <PageToolbar>
                <span className="text-sm font-medium text-muted-foreground">
                  {admissions.length} currently admitted
                </span>
              </PageToolbar>
              <Table>
                <TableHeader>
                  <TableRow className="border-border hover:bg-transparent">
                    <TableHead scope="col">Patient</TableHead>
                    <TableHead scope="col">Ward / Bed</TableHead>
                    <TableHead scope="col">Admitted</TableHead>
                    <TableHead scope="col">Diagnosis</TableHead>
                    <TableHead scope="col" className="text-right">Action</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {admissions.map((a) => (
                    <TableRow key={a.id}>
                      <TableCell className="font-medium">{a.patient_name ?? "—"}</TableCell>
                      <TableCell>{a.ward_name} · {a.bed_number}</TableCell>
                      <TableCell className="text-xs text-muted-foreground">
                        {new Date(a.admission_date).toLocaleDateString()}
                      </TableCell>
                      <TableCell className="text-xs text-muted-foreground max-w-[220px] truncate">
                        {a.admitting_diagnosis ?? "—"}
                      </TableCell>
                      <TableCell className="text-right">
                        <Button size="sm" variant="outline" onClick={() => setSelectedId(a.id)}>
                          Open chart
                        </Button>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </>
          )}
        </SectionCard>
      ) : (
        <SectionCard
          icon={HeartPulse}
          title={`${selected.patient_name ?? "Patient"} — ${selected.ward_name} · ${selected.bed_number}`}
          description={`Admitted ${new Date(selected.admission_date).toLocaleDateString()}${selected.admitting_diagnosis ? ` · ${selected.admitting_diagnosis}` : ""}`}
          action={
            <Button variant="outline" onClick={() => setSelectedId(null)}>
              Back to ward list
            </Button>
          }
        >
          <div className="p-6">
            <Tabs defaultValue="vitals">
              <TabsList>
                <TabsTrigger value="vitals"><Activity className="h-4 w-4 mr-1.5" /> Vitals & trend</TabsTrigger>
                <TabsTrigger value="notes"><StickyNote className="h-4 w-4 mr-1.5" /> Nurse notes</TabsTrigger>
                <TabsTrigger value="mar"><Pill className="h-4 w-4 mr-1.5" /> Medication (MAR)</TabsTrigger>
              </TabsList>
              <TabsContent value="vitals" className="pt-4">
                <VitalsPanel admissionId={selected.id} canManage={canManage} />
              </TabsContent>
              <TabsContent value="notes" className="pt-4">
                <NotesPanel admissionId={selected.id} canManage={canManage} />
              </TabsContent>
              <TabsContent value="mar" className="pt-4">
                <MarPanel admission={selected} canManage={canManage} />
              </TabsContent>
            </Tabs>
          </div>
        </SectionCard>
      )}
    </PageContainer>
  );
}

// ── Vitals panel ─────────────────────────────────────────────────────────────

function VitalsPanel({ admissionId, canManage }: { admissionId: number; canManage: boolean }) {
  const { data: trend = [], isLoading } = useVitalsTrend(admissionId);
  const record = useRecordVitals();
  const [open, setOpen] = useState(false);
  // Raw string state per field; converted to number|null on submit so empty
  // inputs stay "not recorded" rather than 0.
  const [form, setForm] = useState<Record<VitalFieldKey, string>>({
    temperature_c: "", systolic_bp: "", diastolic_bp: "", pulse_bpm: "",
    resp_rate: "", spo2_pct: "", pain_score: "",
  });
  const [notes, setNotes] = useState("");

  const hasAnyValue = Object.values(form).some((v) => v.trim() !== "");

  const submit = async () => {
    const num = (v: string) => (v.trim() === "" ? null : Number(v));
    await record.mutateAsync({
      admission_id: admissionId,
      temperature_c: num(form.temperature_c),
      systolic_bp: num(form.systolic_bp),
      diastolic_bp: num(form.diastolic_bp),
      pulse_bpm: num(form.pulse_bpm),
      resp_rate: num(form.resp_rate),
      spo2_pct: num(form.spo2_pct),
      pain_score: num(form.pain_score),
      notes: notes || null,
    });
    setOpen(false);
    setForm({
      temperature_c: "", systolic_bp: "", diastolic_bp: "", pulse_bpm: "",
      resp_rate: "", spo2_pct: "", pain_score: "",
    });
    setNotes("");
  };

  // Trend is newest-first from the backend; display oldest → newest so the
  // reading order reads left-to-right / top-to-bottom like a paper chart.
  const chronological = [...trend].reverse();

  return (
    <div className="space-y-4">
      {canManage && (
        <div className="flex justify-end">
          <Button onClick={() => setOpen(true)}>
            <Plus className="h-4 w-4" /> Record vitals
          </Button>
        </div>
      )}
      {isLoading ? (
        <LoadingState rows={3} />
      ) : chronological.length === 0 ? (
        <EmptyState
          icon={Activity}
          title="No vitals recorded yet"
          description="Recorded readings build the 7-reading trend for this admission."
        />
      ) : (
        <Table>
          <TableHeader>
            <TableRow className="border-border hover:bg-transparent">
              <TableHead scope="col">Time</TableHead>
              <TableHead scope="col">Temp (°C)</TableHead>
              <TableHead scope="col">BP</TableHead>
              <TableHead scope="col">Pulse</TableHead>
              <TableHead scope="col">Resp.</TableHead>
              <TableHead scope="col">SpO₂</TableHead>
              <TableHead scope="col">Pain</TableHead>
              <TableHead scope="col">Notes</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {chronological.map((r) => (
              <TableRow key={r.id}>
                <TableCell className="text-xs text-muted-foreground whitespace-nowrap">
                  {new Date(r.recorded_at).toLocaleString()}
                </TableCell>
                <TableCell>{r.temperature_c ?? "—"}</TableCell>
                <TableCell>
                  {r.systolic_bp != null || r.diastolic_bp != null
                    ? `${r.systolic_bp ?? "—"}/${r.diastolic_bp ?? "—"}`
                    : "—"}
                </TableCell>
                <TableCell>{r.pulse_bpm ?? "—"}</TableCell>
                <TableCell>{r.resp_rate ?? "—"}</TableCell>
                <TableCell>{r.spo2_pct ?? "—"}</TableCell>
                <TableCell>{r.pain_score ?? "—"}</TableCell>
                <TableCell className="text-xs text-muted-foreground max-w-[220px] truncate">
                  {r.notes ?? "—"}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}

      {/* Vitals entry dialog */}
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Record vitals</DialogTitle>
            <DialogDescription>
              Fill in the readings taken; leave blank anything not measured. At least one value is required.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-2">
            <div className="grid grid-cols-2 gap-3">
              {VITAL_FIELDS.map((f) => (
                <div key={f.key} className="space-y-1.5">
                  <Label htmlFor={`vital-${f.key}`}>{f.label}</Label>
                  <Input
                    id={`vital-${f.key}`}
                    type="number"
                    step="any"
                    placeholder={f.placeholder}
                    min={f.min}
                    max={f.max}
                    value={form[f.key]}
                    onChange={(e) => setForm({ ...form, [f.key]: e.target.value })}
                  />
                </div>
              ))}
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="vital-notes">Notes (optional)</Label>
              <Textarea
                id="vital-notes"
                rows={2}
                value={notes}
                onChange={(e) => setNotes(e.target.value)}
                placeholder="e.g. patient alert and oriented"
              />
            </div>
          </div>
          <DialogFooter>
            <DialogClose asChild>
              <Button variant="outline">Cancel</Button>
            </DialogClose>
            <Button disabled={!hasAnyValue || record.isPending} onClick={submit}>
              {record.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : "Save reading"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

// ── Notes panel ───────────────────────────────────────────────────────────────

const NOTE_TYPES = [
  { value: "shift", label: "Shift note" },
  { value: "observation", label: "Observation" },
  { value: "handover", label: "Handover" },
] as const;

function NotesPanel({ admissionId, canManage }: { admissionId: number; canManage: boolean }) {
  const { data: notes = [], isLoading } = useNurseNotes(admissionId);
  const create = useCreateNurseNote();
  const [content, setContent] = useState("");
  const [noteType, setNoteType] = useState<string>("shift");

  const submit = async () => {
    if (!content.trim()) return;
    await create.mutateAsync({
      admission_id: admissionId,
      note_type: noteType,
      content: content.trim(),
    });
    setContent("");
  };

  return (
    <div className="space-y-4">
      {canManage && (
        <div className="space-y-2 rounded-[var(--radius-md)] border border-border p-4">
          <div className="flex items-center gap-2">
            <Select value={noteType} onValueChange={setNoteType}>
              <SelectTrigger className="w-[180px]">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {NOTE_TYPES.map((t) => (
                  <SelectItem key={t.value} value={t.value}>{t.label}</SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Button
              className="ml-auto"
              disabled={!content.trim() || create.isPending}
              onClick={submit}
            >
              {create.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : <Plus className="h-4 w-4" />}
              Add note
            </Button>
          </div>
          <Textarea
            rows={3}
            value={content}
            onChange={(e) => setContent(e.target.value)}
            placeholder="Observation, intervention, handover context…"
          />
        </div>
      )}
      {isLoading ? (
        <LoadingState rows={3} />
      ) : notes.length === 0 ? (
        <EmptyState
          icon={StickyNote}
          title="No notes yet"
          description="Shift notes, observations, and handovers appear here, newest first."
        />
      ) : (
        <div className="space-y-2">
          {notes.map((n) => (
            <div key={n.id} className="rounded-[var(--radius-md)] border border-border p-3">
              <div className="flex items-center gap-2">
                <Badge variant="outline" className="text-[9px] uppercase tracking-wide">
                  {n.note_type}
                </Badge>
                <span className="text-[11px] text-muted-foreground">
                  {new Date(n.created_at).toLocaleString()}
                </span>
              </div>
              <p className="text-sm mt-1.5 whitespace-pre-wrap">{n.content}</p>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

// ── MAR panel ────────────────────────────────────────────────────────────────

function MarPanel({ admission, canManage }: { admission: { id: number; patient_id: number; patient_name: string | null }; canManage: boolean }) {
  const { data: mar = [], isLoading: marLoading } = useMedicationAdministrations(admission.id);
  // Active prescriptions for THIS patient — the nurse marks administrations
  // against them. (Backend re-validates item↔patient↔admission on write.)
  const { data: prescriptions = [] } = usePrescriptions(admission.patient_id, "active");
  const [rxId, setRxId] = useState<number | null>(null);
  const { data: rxDetail } = usePrescription(rxId);
  const record = useRecordMedicationAdministration();
  const [status, setStatus] = useState("administered");
  const [notes, setNotes] = useState("");

  const markAdministered = async (itemId: number) => {
    await record.mutateAsync({
      admission_id: admission.id,
      prescription_item_id: itemId,
      status,
      notes: notes || null,
    });
    setNotes("");
  };

  return (
    <div className="space-y-4">
      {canManage && (
        <div className="space-y-2 rounded-[var(--radius-md)] border border-border p-4">
          <div className="flex items-center gap-2">
            <Select
              value={rxId?.toString() ?? ""}
              onValueChange={(v) => setRxId(Number(v))}
            >
              <SelectTrigger className="w-[260px]">
                <SelectValue placeholder={prescriptions.length ? "Select prescription" : "No active prescriptions"} />
              </SelectTrigger>
              <SelectContent>
                {prescriptions.map((p) => (
                  <SelectItem key={p.id} value={p.id.toString()}>
                    #{p.id} · {new Date(p.created_at).toLocaleDateString()} · {p.doctor_name ?? "—"}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Select value={status} onValueChange={setStatus}>
              <SelectTrigger className="w-[170px]">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="administered">Administered</SelectItem>
                <SelectItem value="held">Held</SelectItem>
                <SelectItem value="refused">Refused (by patient)</SelectItem>
              </SelectContent>
            </Select>
          </div>
          {rxDetail && (
            <Table>
              <TableHeader>
                <TableRow className="border-border hover:bg-transparent">
                  <TableHead scope="col">Medication</TableHead>
                  <TableHead scope="col">Dose / route</TableHead>
                  <TableHead scope="col">Frequency</TableHead>
                  <TableHead scope="col" className="text-right">Action</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {rxDetail.items.map((it) => (
                  <TableRow key={it.id}>
                    <TableCell className="font-medium">{it.medication_name}</TableCell>
                    <TableCell>{it.dose} · {it.route}</TableCell>
                    <TableCell>{it.frequency}</TableCell>
                    <TableCell className="text-right">
                      <Button
                        size="sm"
                        variant="outline"
                        disabled={record.isPending}
                        onClick={() => markAdministered(it.id)}
                      >
                        Mark {status}
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
          <Input
            placeholder="Optional note for the next MAR entry"
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
          />
        </div>
      )}
      {marLoading ? (
        <LoadingState rows={3} />
      ) : mar.length === 0 ? (
        <EmptyState
          icon={Pill}
          title="No administrations recorded"
          description="Each administered / held / refused medication event is logged here for the medication chart."
        />
      ) : (
        <Table>
          <TableHeader>
            <TableRow className="border-border hover:bg-transparent">
              <TableHead scope="col">Time</TableHead>
              <TableHead scope="col">Medication</TableHead>
              <TableHead scope="col">Status</TableHead>
              <TableHead scope="col">Notes</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {mar.map((m) => (
              <TableRow key={m.id}>
                <TableCell className="text-xs text-muted-foreground whitespace-nowrap">
                  {new Date(m.administered_at).toLocaleString()}
                </TableCell>
                <TableCell className="font-medium">
                  {m.medication_name ?? <span className="font-mono text-xs">#{m.prescription_item_id}</span>}
                </TableCell>
                <TableCell><StatusBadge status={m.status} /></TableCell>
                <TableCell className="text-xs text-muted-foreground max-w-[220px] truncate">
                  {m.notes ?? "—"}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
    </div>
  );
}
