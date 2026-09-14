/**
 * PatientSafetyBanner — the shared patient-context safety strip (RCTF
 * UX-2026-09-13 U-01/U-02, SRS §2.2 business rule "Allergy flags are
 * shown on every prescription screen and every medication dispensing
 * screen as a prominent warning").
 *
 * The patient's allergies / chronic conditions / blood group are
 * collected on registration (PatientForm collects the full EHR field
 * set) but were never surfaced in ANY clinical workflow — prescribing,
 * dispensing, or MAR — although the backend returns them on every
 * get_patient / get_patients round-trip. This component closes that
 * gap with a consistent, impossible-to-miss banner reused at each
 * medication-decision point.
 *
 * Design (per SRS: "prominent warning"):
 *  - ALLERGIES (when present): destructive-tinted strip with the
 *    AlertTriangle icon + "ALLERGIES" in bold caps + the allergen list.
 *    Color + icon + text — never color alone (WCAG 1.4.1).
 *  - Chronic conditions + blood group: neutral info chips beneath, so
 *    the allergy strip stays the single red signal (alert-fatigue
 *    discipline — chronic conditions are context, not an emergency).
 *  - "No known allergies" is NOT rendered — a green "no allergy" chip
 *    next to every form adds noise; absence of the strip means exactly
 *    what the EHR says.
 *
 * Data source: `usePatientEhr(patientId)` — the same canonical row the
 * PatientForm edits, so a banner shown during prescribing reflects the
 * registration data, not a stale copy.
 */
import { AlertTriangle, Droplet, HeartPulse } from "lucide-react";
import { usePatientEhr } from "@/lib/queries";

export function PatientSafetyBanner({ patientId }: { patientId: number | null }) {
  const { data: ehr, isLoading } = usePatientEhr(patientId);

  if (isLoading || patientId == null) return null;

  const allergies = (ehr?.allergies ?? "").trim();
  const chronic = (ehr?.chronic_conditions ?? "").trim();
  const blood = (ehr?.blood_group ?? "").trim();
  if (!allergies && !chronic && !blood) return null;

  return (
    <div className="space-y-2" data-testid="patient-safety-banner">
      {allergies && (
        <div
          role="alert"
          className="flex items-start gap-3 rounded-[var(--radius-md)] border border-destructive/50 bg-destructive/10 px-4 py-3"
        >
          <AlertTriangle className="h-5 w-5 text-destructive shrink-0 mt-0.5" />
          <div className="min-w-0">
            <div className="text-xs font-bold uppercase tracking-wide text-destructive">
              Allergies
            </div>
            <div className="text-sm font-semibold text-foreground leading-snug break-words">
              {allergies}
            </div>
          </div>
        </div>
      )}
      {(chronic || blood) && (
        <div className="flex flex-wrap items-center gap-2">
          {blood && (
            <span className="inline-flex items-center gap-1.5 rounded-full border border-border bg-muted/50 px-2.5 py-1 text-xs font-semibold">
              <Droplet className="h-3.5 w-3.5 text-destructive" />
              Blood group <span className="font-bold text-foreground">{blood}</span>
            </span>
          )}
          {chronic && (
            <span className="inline-flex items-center gap-1.5 rounded-full border border-border bg-muted/50 px-2.5 py-1 text-xs font-medium text-muted-foreground">
              <HeartPulse className="h-3.5 w-3.5 text-warning" />
              <span className="text-foreground font-semibold">{chronic}</span>
            </span>
          )}
        </div>
      )}
    </div>
  );
}
