/**
 * PatientProfileDialog — the shared patient-context surface (RCTF
 * UX-2026-09-13 U-02/U-03, SRS §2.2 "Patient history timeline view: all
 * encounters, admissions, prescriptions, lab results in chronological
 * order").
 *
 * Previously the app had NO patient detail view at all: the Patients
 * page offered only Edit/Delete, and no other page linked a patient row
 * to their record. The backend hooks (usePatientEhr, useEncounters,
 * usePrescriptions) all existed and were exercised by tests — but no
 * page composed them.
 *
 * Progressive disclosure per the information-hierarchy levels:
 *  - Identity strip (always): name, MRN, DOB, phone — the two-identifier
 *    convention (F-02) is carried into the profile header.
 *  - Safety banner (always): allergies/chronic/blood via the shared
 *    PatientSafetyBanner — same component the prescribing screens use.
 *  - Tabs: Visits (encounters, + inline "record visit" form for holders
 *    of PatientsUpdate — the backend's exact gate for create_encounter),
 *    Prescriptions.
 *
 * The record-visit form is INLINE in the tab, not a nested modal — a
 * modal-inside-modal is an anti-pattern (RCTF §23); an expanding inline
 * form keeps the timeline visible as context while entering the visit.
 *
 * Parents must only mount this component with a non-null patientId
 * (the encounter/prescription hooks fetch ALL rows when passed null).
 */
import { useState } from "react";
import { CalendarDays, ChevronUp, FileText, Loader2, Plus, User } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  usePatientEhr,
  useEncounters,
  usePrescriptions,
  useCreateEncounter,
  useDoctors,
} from "@/lib/queries";
import { useAuth } from "@/lib/auth";
import { PERMISSIONS } from "@/lib/rbac";
import { StatusBadge, EmptyState, LoadingState } from "@/components/layout/shared";
import { PatientSafetyBanner } from "./PatientSafetyBanner";
import { formatDob } from "@/lib/utils";

const VISIT_TYPES = [
  { value: "new", label: "New visit" },
  { value: "follow-up", label: "Follow-up" },
] as const;

/** Inline "record visit" form (encounter creation). Gated by
 *  PatientsUpdate — the exact permission the backend's create_encounter
 *  requires — so the affordance never appears for users the backend
 *  would reject. */
function RecordVisitForm({ patientId, onRecorded }: { patientId: number; onRecorded: () => void }) {
  const create = useCreateEncounter();
  const { data: doctors = [] } = useDoctors(true);
  const [visitType, setVisitType] = useState<string>("new");
  const [doctorId, setDoctorId] = useState<number | null>(null);
  const [complaint, setComplaint] = useState("");
  const [diagnosis, setDiagnosis] = useState("");
  const [notes, setNotes] = useState("");

  const canSubmit = complaint.trim() !== "" || diagnosis.trim() !== "" || notes.trim() !== "";

  const submit = async () => {
    if (!canSubmit) return;
    await create.mutateAsync({
      patient_id: patientId,
      doctor_id: doctorId,
      visit_type: visitType,
      chief_complaint: complaint.trim() === "" ? null : complaint.trim(),
      diagnosis: diagnosis.trim() === "" ? null : diagnosis.trim(),
      notes: notes.trim() === "" ? null : notes.trim(),
    });
    setComplaint("");
    setDiagnosis("");
    setNotes("");
    setDoctorId(null);
    setVisitType("new");
    onRecorded();
  };

  return (
    <div className="space-y-3 rounded-[var(--radius-md)] border border-border bg-muted/30 p-4">
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
        <div className="space-y-1.5">
          <Label htmlFor="visit-type">Visit type</Label>
          <Select value={visitType} onValueChange={setVisitType}>
            <SelectTrigger id="visit-type">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {VISIT_TYPES.map((t) => (
                <SelectItem key={t.value} value={t.value}>
                  {t.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="space-y-1.5">
          <Label htmlFor="visit-doctor">Doctor (optional)</Label>
          <Select
            value={doctorId?.toString() ?? "none"}
            onValueChange={(v) => setDoctorId(v === "none" ? null : Number(v))}
          >
            <SelectTrigger id="visit-doctor">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="none">—</SelectItem>
              {doctors.map((d) => (
                <SelectItem key={d.id} value={d.id.toString()}>
                  Dr. {d.first_name} {d.last_name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>
      <div className="space-y-1.5">
        <Label htmlFor="visit-complaint">Chief complaint</Label>
        <Input
          id="visit-complaint"
          value={complaint}
          onChange={(e) => setComplaint(e.target.value)}
          placeholder="e.g. fever for 2 days"
        />
      </div>
      <div className="space-y-1.5">
        <Label htmlFor="visit-diagnosis">Diagnosis</Label>
        <Input
          id="visit-diagnosis"
          value={diagnosis}
          onChange={(e) => setDiagnosis(e.target.value)}
          placeholder="Working diagnosis"
        />
      </div>
      <div className="space-y-1.5">
        <Label htmlFor="visit-notes">Clinical notes</Label>
        <Textarea
          id="visit-notes"
          rows={2}
          value={notes}
          onChange={(e) => setNotes(e.target.value)}
          placeholder="Examination findings, plan…"
        />
      </div>
      <div className="flex justify-end">
        <Button size="sm" onClick={submit} disabled={!canSubmit || create.isPending}>
          {create.isPending ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <Plus className="h-4 w-4" />
          )}
          Record visit
        </Button>
      </div>
    </div>
  );
}

export function PatientProfileDialog({
  patientId,
  open,
  onOpenChange,
}: {
  patientId: number | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { has } = useAuth();
  const canRecordVisits = has(PERMISSIONS.PatientsUpdate);
  const { data: ehr, isLoading } = usePatientEhr(patientId);
  const { data: encounters = [], isLoading: encountersLoading } =
    useEncounters(patientId);
  const { data: prescriptions = [], isLoading: rxLoading } = usePrescriptions(
    patientId,
    null,
  );
  const [showRecordForm, setShowRecordForm] = useState(false);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <User className="h-5 w-5 text-primary" />
            {ehr
              ? `${ehr.first_name} ${ehr.last_name}`
              : patientId != null
                ? `Patient #${patientId}`
                : "Patient"}
          </DialogTitle>
          <DialogDescription>
            {ehr
              ? `MRN ${ehr.mrn ?? "—"} · DOB ${formatDob(ehr.date_of_birth)} · ${ehr.phone}`
              : "Loading patient record…"}
          </DialogDescription>
        </DialogHeader>

        {isLoading ? (
          <LoadingState rows={4} />
        ) : ehr ? (
          <div className="space-y-4">
            <PatientSafetyBanner patientId={patientId} />
            <Tabs defaultValue="timeline">
              <TabsList>
                <TabsTrigger value="timeline">
                  <CalendarDays className="h-4 w-4 mr-1.5" /> Visits ({encounters.length})
                </TabsTrigger>
                <TabsTrigger value="prescriptions">
                  <FileText className="h-4 w-4 mr-1.5" /> Prescriptions ({prescriptions.length})
                </TabsTrigger>
              </TabsList>

              <TabsContent value="timeline" className="pt-4 space-y-3">
                {canRecordVisits && (
                  <div className="flex justify-end">
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => setShowRecordForm((s) => !s)}
                      aria-expanded={showRecordForm}
                    >
                      {showRecordForm ? (
                        <>
                          <ChevronUp className="h-3.5 w-3.5" /> Close visit form
                        </>
                      ) : (
                        <>
                          <Plus className="h-3.5 w-3.5" /> Record visit
                        </>
                      )}
                    </Button>
                  </div>
                )}
                {showRecordForm && canRecordVisits && (
                  <RecordVisitForm
                    patientId={patientId!}
                    onRecorded={() => setShowRecordForm(false)}
                  />
                )}
                {encountersLoading ? (
                  <LoadingState rows={3} />
                ) : encounters.length === 0 && !showRecordForm ? (
                  <EmptyState
                    icon={CalendarDays}
                    title="No visits recorded"
                    description={
                      canRecordVisits
                        ? "Record this patient's consultation details with the button above."
                        : "Outpatient encounters will appear here, newest first."
                    }
                  />
                ) : (
                  <ol className="space-y-3">
                    {encounters.map((enc) => (
                      <li
                        key={enc.id}
                        className="rounded-[var(--radius-md)] border border-border p-3"
                      >
                        <div className="flex items-center justify-between gap-3 flex-wrap">
                          <div className="min-w-0">
                            <div className="text-sm font-semibold">
                              {enc.chief_complaint ?? "General consultation"}
                            </div>
                            <div className="text-xs text-muted-foreground mt-0.5">
                              {new Date(enc.visit_date).toLocaleDateString()} ·{" "}
                              {enc.visit_type === "ipd" ? "In-patient" : "Out-patient"}
                            </div>
                          </div>
                        </div>
                        {enc.diagnosis && (
                          <div className="text-xs mt-1.5">
                            <span className="text-muted-foreground">Diagnosis:</span>{" "}
                            <span className="font-medium text-foreground">
                              {enc.diagnosis}
                            </span>
                          </div>
                        )}
                        {enc.notes && (
                          <p className="text-xs text-muted-foreground mt-1 leading-relaxed whitespace-pre-wrap">
                            {enc.notes}
                          </p>
                        )}
                      </li>
                    ))}
                  </ol>
                )}
              </TabsContent>

              <TabsContent value="prescriptions" className="pt-4">
                {rxLoading ? (
                  <LoadingState rows={3} />
                ) : prescriptions.length === 0 ? (
                  <EmptyState
                    icon={FileText}
                    title="No prescriptions"
                    description="Prescriptions issued to this patient appear here."
                  />
                ) : (
                  <ol className="space-y-2">
                    {prescriptions.map((rx) => (
                      <li
                        key={rx.id}
                        className="flex items-center justify-between gap-3 rounded-[var(--radius-md)] border border-border px-3 py-2.5"
                      >
                        <div className="min-w-0">
                          <div className="text-sm font-medium">
                            Prescription #{rx.id}
                          </div>
                          <div className="text-xs text-muted-foreground">
                            {new Date(rx.created_at).toLocaleString()}
                            {rx.doctor_name ? ` · Dr. ${rx.doctor_name}` : ""}
                          </div>
                        </div>
                        <StatusBadge status={rx.status} />
                      </li>
                    ))}
                  </ol>
                )}
              </TabsContent>
            </Tabs>
          </div>
        ) : null}
      </DialogContent>
    </Dialog>
  );
}
