/* eslint-disable react-refresh/only-export-components */
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Badge } from "@/components/ui/badge";
import { toast } from "sonner";
import { MessageCircle, ShieldCheck, ShieldAlert, Loader2 } from "lucide-react";
import {
  usePatientConsent,
  useSetPatientConsent,
  useRevokePatientConsent,
} from "@/lib/queries";
import { SectionCard } from "@/components/layout/shared";
import { useAuth } from "@/lib/auth";
import { PERMISSIONS } from "@/lib/rbac";

// ── CR-12 follow-up — Patient consent UI ──────────────────────────────────
//
// Wires the three consent commands added in Batch 1
// (`commands/patients.rs`):
//   get_patient_consent(patient_id) → Option<PatientConsent>
//   set_patient_consent(patient_id, consent_type, granted, notes?) → id
//   revoke_patient_consent(patient_id, consent_type) → ()
//
// The `whatsapp` consent type gates outbound WhatsApp messages — see
// `whatsapp::automation::send_whatsapp`. The consent row is UNIQUE on
// (patient_id, consent_type) so upserts are clean. This component is a
// small panel that can be embedded in a patient detail view; it shows the
// current consent status, the timestamp of the last change, and a button
// to grant or revoke.
//
// Permissioning: viewing the status requires PatientsView (the same as
// viewing the patient record). Mutating consent requires
// PatientConsentManage. The backend re-checks both — this UI gating is
// just for affordance.

const WHATSAPP_CONSENT_TYPE = "whatsapp";

export interface ConsentPanelProps {
  patientId: number;
  patientName?: string;
}

export function ConsentPanel({ patientId, patientName }: ConsentPanelProps) {
  const { has } = useAuth();
  const canManage = has(PERMISSIONS.PatientConsentManage);

  const { data: consent, isLoading } = usePatientConsent(patientId);
  const setConsent = useSetPatientConsent();
  const revokeConsent = useRevokePatientConsent();

  const [notes, setNotes] = useState("");
  const [showNotesInput, setShowNotesInput] = useState(false);

  const granted = consent?.granted === true;

  const handleGrant = async () => {
    try {
      await setConsent.mutateAsync({
        patient_id: patientId,
        consent_type: WHATSAPP_CONSENT_TYPE,
        granted: true,
        notes: notes.trim() === "" ? null : notes.trim(),
      });
      setNotes("");
      setShowNotesInput(false);
    } catch {
      /* toast already shown by the mutation's onError */
    }
  };

  const handleRevoke = async () => {
    if (
      !confirm(
        `Revoke WhatsApp consent for ${patientName ?? "this patient"}? ` +
          `Outbound WhatsApp notifications to this patient will be blocked until consent is re-granted.`,
      )
    ) {
      return;
    }
    try {
      await revokeConsent.mutateAsync({
        patient_id: patientId,
        consent_type: WHATSAPP_CONSENT_TYPE,
      });
    } catch {
      /* toast already shown by the mutation's onError */
    }
  };

  return (
    <SectionCard
      icon={MessageCircle}
      title="Communication consent"
      description="WhatsApp notification opt-in (CR-12, SRS FR-0035)."
    >
      <div className="p-6 space-y-4">
        {isLoading ? (
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" />
            Loading consent status…
          </div>
        ) : (
          <>
            <div className="flex items-center gap-3">
              {consent === null || consent === undefined ? (
                <>
                  <ShieldAlert className="h-5 w-5 text-warning" />
                  <div className="flex-1 min-w-0">
                    <p className="text-sm font-semibold text-foreground">
                      Consent not set
                    </p>
                    <p className="text-xs text-muted-foreground mt-0.5">
                      WhatsApp notifications are blocked until explicit
                      consent is granted.
                    </p>
                  </div>
                  <Badge variant="outline" className="bg-warning/10 text-warning border-warning/20">
                    Not set
                  </Badge>
                </>
              ) : granted ? (
                <>
                  <ShieldCheck className="h-5 w-5 text-success" />
                  <div className="flex-1 min-w-0">
                    <p className="text-sm font-semibold text-foreground">
                      Consent granted
                    </p>
                    <p className="text-xs text-muted-foreground mt-0.5">
                      Last updated{" "}
                      {new Date(consent.granted_at).toLocaleString()}.
                      {consent.notes && (
                        <> Notes: {consent.notes}</>
                      )}
                    </p>
                  </div>
                  <Badge variant="outline" className="bg-success/10 text-success border-success/20">
                    Granted
                  </Badge>
                </>
              ) : (
                <>
                  <ShieldAlert className="h-5 w-5 text-destructive" />
                  <div className="flex-1 min-w-0">
                    <p className="text-sm font-semibold text-foreground">
                      Consent revoked
                    </p>
                    <p className="text-xs text-muted-foreground mt-0.5">
                      Last changed{" "}
                      {new Date(consent.granted_at).toLocaleString()}.
                      WhatsApp notifications are blocked.
                    </p>
                  </div>
                  <Badge variant="outline" className="bg-destructive/10 text-destructive border-destructive/20">
                    Revoked
                  </Badge>
                </>
              )}
            </div>

            {canManage && (
              <div className="pt-4 border-t border-border space-y-3">
                {showNotesInput && !granted && (
                  <div className="space-y-1.5">
                    <label
                      htmlFor="consent-notes"
                      className="text-xs font-semibold uppercase tracking-wide text-foreground"
                    >
                      Notes (optional)
                    </label>
                    <Textarea
                      id="consent-notes"
                      placeholder="e.g. Patient signed consent form on intake."
                      rows={2}
                      value={notes}
                      onChange={(e) => setNotes(e.target.value)}
                      disabled={setConsent.isPending}
                      className="resize-none"
                    />
                  </div>
                )}

                <div className="flex flex-wrap gap-2">
                  {!granted && (
                    <>
                      {!showNotesInput ? (
                        <Button
                          type="button"
                          size="sm"
                          onClick={() => setShowNotesInput(true)}
                          disabled={setConsent.isPending}
                          className="gap-2"
                        >
                          <ShieldCheck className="h-4 w-4" />
                          Grant WhatsApp consent
                        </Button>
                      ) : (
                        <>
                          <Button
                            type="button"
                            size="sm"
                            onClick={handleGrant}
                            disabled={setConsent.isPending}
                            className="gap-2"
                          >
                            {setConsent.isPending ? (
                              <Loader2 className="h-4 w-4 animate-spin" />
                            ) : (
                              <ShieldCheck className="h-4 w-4" />
                            )}
                            Confirm grant
                          </Button>
                          <Button
                            type="button"
                            size="sm"
                            variant="outline"
                            onClick={() => {
                              setShowNotesInput(false);
                              setNotes("");
                            }}
                            disabled={setConsent.isPending}
                          >
                            Cancel
                          </Button>
                        </>
                      )}
                    </>
                  )}
                  {granted && (
                    <Button
                      type="button"
                      size="sm"
                      variant="outline"
                      onClick={handleRevoke}
                      disabled={revokeConsent.isPending}
                      className="gap-2 text-destructive hover:text-destructive hover:bg-destructive/10"
                    >
                      {revokeConsent.isPending ? (
                        <Loader2 className="h-4 w-4 animate-spin" />
                      ) : (
                        <ShieldAlert className="h-4 w-4" />
                      )}
                      Revoke consent
                    </Button>
                  )}
                </div>

                <p className="text-[11px] text-muted-foreground leading-relaxed">
                  Every change is audit-logged with the operator's user ID
                  and a timestamp. The patient must explicitly opt in before
                  any WhatsApp message can be sent — this is a HIPAA /
                  GDPR "minimum necessary" control.
                </p>
              </div>
            )}

            {!canManage && (
              <p className="text-[11px] text-muted-foreground">
                You do not have permission to modify consent. Contact a
                supervisor if it needs to be changed.
              </p>
            )}
          </>
        )}
      </div>
    </SectionCard>
  );
}

/**
 * Convenience hook for one-off consent checks (e.g. an icon next to a
 * patient name in a list to show consent status at a glance). Returns
 * "granted" | "revoked" | "not_set" | "loading".
 */
export function useConsentStatus(patientId: number | null) {
  const { data, isLoading } = usePatientConsent(patientId);
  if (isLoading) return "loading" as const;
  if (!data) return "not_set" as const;
  return data.granted ? ("granted" as const) : ("revoked" as const);
}

// Toast helper for callers that need to surface a consent-blocked
// notification (e.g. the WhatsApp send button when consent is revoked).
export function warnConsentBlocked() {
  toast.error(
    "Cannot send: patient has not granted WhatsApp consent. " +
      "Grant consent from the patient record first.",
  );
}
