import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { toast } from "sonner";
import { useCreatePatientEhr, useUpdatePatientEhr, usePatientEhr } from "@/lib/queries";
import { ActionBar, FormField, FormSection } from "@/components/layout/shared";
import { ConsentPanel } from "@/components/forms/ConsentPanel";
import type { Patient, PatientEhr } from "@/lib/models";

// ── ARCH-03 ────────────────────────────────────────────────────────────────
//
// Previously this form declared a local `Patient` interface that omitted
// every EHR field (mrn, blood_group, allergies, chronic_conditions,
// emergency_contact_*, insurance_*). The form was clinically useless —
// even though the DB columns and the typed `CreatePatientEhr` /
// `UpdatePatientEhr` payloads existed end-to-end, the operator could never
// enter the data. This file now collects the full EHR field set, grouped:
//
//   • Demographics      — first_name, last_name, date_of_birth, gender,
//                          phone, email, address
//   • Medical record    — mrn, blood_group, allergies, chronic_conditions
//   • Emergency contact — emergency_contact_name, emergency_contact_phone
//   • Insurance         — insurance_provider, insurance_policy_number
//
// The form accepts either a basic `Patient` (from `usePatients`) or a
// `PatientEhr` (from `usePatientsEhr` / `usePatientEhr`). When editing, it
// also calls `usePatientEhr(patient.id)` to fetch the canonical EHR row —
// the basic list omits `status` and the EHR-only fields, but
// `update_patient` requires `status` in `UpdatePatientEhr`. Fetching the
// single record guarantees we always send the current status back (rather
// than silently defaulting it, which could flip a discharged patient back
// to "active").

/**
 * Initial value shape — every field is optional because the form must
 * accept either the basic `Patient` (no EHR fields) or the full
 * `PatientEhr`. Structurally compatible with both via optional fields.
 */
export interface PatientFormValue {
  id?: number;
  first_name?: string;
  last_name?: string;
  email?: string | null;
  phone?: string;
  date_of_birth?: string;
  gender?: string;
  address?: string | null;
  status?: string;
  mrn?: string | null;
  blood_group?: string | null;
  allergies?: string | null;
  chronic_conditions?: string | null;
  emergency_contact_name?: string | null;
  emergency_contact_phone?: string | null;
  insurance_provider?: string | null;
  insurance_policy_number?: string | null;
}

interface PatientFormProps {
  /** Either a basic Patient (from `usePatients`) or an EHR-expanded
   *  PatientEhr (from `usePatientsEhr`). Undefined for create. */
  patient?: Patient | PatientEhr | PatientFormValue;
  onSuccess: () => void;
  onCancel: () => void;
}

const BLOOD_GROUPS = ["A+", "A-", "B+", "B-", "AB+", "AB-", "O+", "O-"] as const;
const GENDERS = ["Male", "Female", "Other"] as const;

/** Convert a `null | ""` to `null` so the backend gets a clean Option<String>. */
const nullable = (s: string): string | null => (s.trim() === "" ? null : s);

export function PatientForm({ patient, onSuccess, onCancel }: PatientFormProps) {
  const isEdit = !!patient?.id;
  const createPatient = useCreatePatientEhr();
  const updatePatient = useUpdatePatientEhr();
  const loading = createPatient.isPending || updatePatient.isPending;

  // Fetch the full EHR record when editing so we have the current
  // `status` (the basic `Patient` from `usePatients` doesn't include it,
  // but `update_patient` requires it in `UpdatePatientEhr`). The query is
  // enabled only in edit mode.
  const ehrId = isEdit && patient?.id ? patient.id : null;
  const { data: ehrRecord, isLoading: ehrLoading } = usePatientEhr(ehrId);

  // Seed the form from either the basic patient prop OR the fetched EHR
  // row (which has the EHR-only fields). When the EHR row arrives, prefer
  // it — it's authoritative.
  const seed: PatientFormValue = ehrRecord ?? (patient as PatientFormValue | undefined) ?? {};

  const [firstName, setFirstName] = useState(seed.first_name ?? "");
  const [lastName, setLastName] = useState(seed.last_name ?? "");
  const [email, setEmail] = useState(seed.email ?? "");
  const [phone, setPhone] = useState(seed.phone ?? "");
  const [dob, setDob] = useState(seed.date_of_birth ?? "");
  const [gender, setGender] = useState<string>(seed.gender ?? "Male");
  const [address, setAddress] = useState(seed.address ?? "");

  const [mrn, setMrn] = useState(seed.mrn ?? "");
  const [bloodGroup, setBloodGroup] = useState<string>(seed.blood_group ?? "unknown");
  const [allergies, setAllergies] = useState(seed.allergies ?? "");
  const [chronicConditions, setChronicConditions] = useState(seed.chronic_conditions ?? "");

  const [emergencyName, setEmergencyName] = useState(seed.emergency_contact_name ?? "");
  const [emergencyPhone, setEmergencyPhone] = useState(seed.emergency_contact_phone ?? "");

  const [insuranceProvider, setInsuranceProvider] = useState(seed.insurance_provider ?? "");
  const [insurancePolicyNumber, setInsurancePolicyNumber] = useState(
    seed.insurance_policy_number ?? "",
  );

  // If the EHR record arrives AFTER first paint (it's a separate query),
  // re-seed the EHR-only fields. We avoid clobbering fields the operator
  // may have started typing by only running this once per (ehrId, ehrRecord
  // reference) pair — the keys in the deps array.
  useEffect(() => {
    if (!ehrRecord) return;
    setMrn(ehrRecord.mrn ?? "");
    setBloodGroup(ehrRecord.blood_group ?? "unknown");
    setAllergies(ehrRecord.allergies ?? "");
    setChronicConditions(ehrRecord.chronic_conditions ?? "");
    setEmergencyName(ehrRecord.emergency_contact_name ?? "");
    setEmergencyPhone(ehrRecord.emergency_contact_phone ?? "");
    setInsuranceProvider(ehrRecord.insurance_provider ?? "");
    setInsurancePolicyNumber(ehrRecord.insurance_policy_number ?? "");
  }, [ehrRecord]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();

    // Basic client-side validation. The backend re-checks everything, but
    // catching the obvious cases here saves a round-trip and a toast.
    if (!firstName.trim() || !lastName.trim() || !phone.trim() || !dob) {
      toast.error("Please fill in all required fields (name, phone, date of birth).");
      return;
    }
    // Phone format hint: accept digits, spaces, +, -, (). Just warn — the
    // backend doesn't enforce a strict pattern, but a 7+ digit number is
    // the minimum for a useful Pakistani phone number.
    const phoneDigits = phone.replace(/\D/g, "");
    if (phoneDigits.length < 7) {
      toast.error("Phone number looks too short. Include area/mobile prefix.");
      return;
    }

    // Shared payload — the backend `create_patient` accepts `CreatePatientEhr`
    // and `update_patient` accepts `UpdatePatientEhr`. Both share the same
    // EHR field set; the update variant adds `id` + `status`.
    const ehrPayload = {
      first_name: firstName.trim(),
      last_name: lastName.trim(),
      email: nullable(email),
      phone: phone.trim(),
      date_of_birth: dob,
      gender,
      address: nullable(address),
      mrn: nullable(mrn),
      blood_group: bloodGroup === "unknown" ? null : bloodGroup,
      allergies: nullable(allergies),
      chronic_conditions: nullable(chronicConditions),
      emergency_contact_name: nullable(emergencyName),
      emergency_contact_phone: nullable(emergencyPhone),
      insurance_provider: nullable(insuranceProvider),
      insurance_policy_number: nullable(insurancePolicyNumber),
    };

    try {
      if (isEdit && patient?.id) {
        // `status` is required by UpdatePatientEhr. Prefer the fetched EHR
        // record's status; fall back to "active" only if the EHR fetch
        // failed (which is rare — the record exists if we're editing it).
        const status = ehrRecord?.status ?? seed.status ?? "active";
        await updatePatient.mutateAsync({
          id: patient.id,
          ...ehrPayload,
          status,
        });
      } else {
        await createPatient.mutateAsync(ehrPayload);
      }
      onSuccess();
    } catch {
      /* toast already shown by the mutation's onError */
    }
  };

  return (
    <form onSubmit={handleSubmit} className="form-stack">
      <FormSection title="Demographics" description="Identity and contact details.">
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <FormField label="First name" htmlFor="first_name" required>
            <Input
              id="first_name"
              placeholder="John"
              value={firstName}
              onChange={(e) => setFirstName(e.target.value)}
              disabled={loading}
              required
            />
          </FormField>
          <FormField label="Last name" htmlFor="last_name" required>
            <Input
              id="last_name"
              placeholder="Doe"
              value={lastName}
              onChange={(e) => setLastName(e.target.value)}
              disabled={loading}
              required
            />
          </FormField>
        </div>

        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <FormField
            label="Phone number"
            htmlFor="phone"
            required
            hint="Include country/area code, e.g. +92 300 1234567."
          >
            <Input
              id="phone"
              placeholder="+92 300 1234567"
              value={phone}
              onChange={(e) => setPhone(e.target.value)}
              disabled={loading}
              required
              inputMode="tel"
            />
          </FormField>
          <FormField label="Email address" htmlFor="email">
            <Input
              id="email"
              type="email"
              placeholder="john.doe@example.com"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              disabled={loading}
            />
          </FormField>
        </div>

        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <FormField label="Date of birth" htmlFor="dob" required>
            <Input
              id="dob"
              type="date"
              value={dob}
              onChange={(e) => setDob(e.target.value)}
              disabled={loading}
              required
            />
          </FormField>
          <FormField label="Gender" htmlFor="gender" required>
            <Select
              value={gender}
              onValueChange={(val) => setGender(val)}
              disabled={loading}
            >
              <SelectTrigger id="gender">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {GENDERS.map((g) => (
                  <SelectItem key={g} value={g}>
                    {g}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </FormField>
        </div>

        <FormField label="Residential address" htmlFor="address">
          <Textarea
            id="address"
            placeholder="123 Health Ave, Clinic District"
            rows={3}
            value={address}
            onChange={(e) => setAddress(e.target.value)}
            disabled={loading}
            className="resize-none"
          />
        </FormField>
      </FormSection>

      <FormSection
        title="Medical record"
        description="Clinically-critical fields used at the point of care."
      >
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <FormField
            label="Medical record number (MRN)"
            htmlFor="mrn"
            hint="Unique patient identifier within the hospital. Auto-assigned if left blank."
          >
            <Input
              id="mrn"
              placeholder="MRN-00001"
              value={mrn}
              onChange={(e) => setMrn(e.target.value)}
              disabled={loading}
            />
          </FormField>
          <FormField label="Blood group" htmlFor="blood_group">
            <Select
              value={bloodGroup}
              onValueChange={(val) => setBloodGroup(val)}
              disabled={loading}
            >
              <SelectTrigger id="blood_group">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="unknown">Unknown</SelectItem>
                {BLOOD_GROUPS.map((bg) => (
                  <SelectItem key={bg} value={bg}>
                    {bg}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </FormField>
        </div>

        <FormField
          label="Allergies"
          htmlFor="allergies"
          hint="Comma-separated or free text. Enter 'None known' if applicable."
        >
          <Textarea
            id="allergies"
            placeholder="Penicillin, Peanuts, Latex"
            rows={2}
            value={allergies}
            onChange={(e) => setAllergies(e.target.value)}
            disabled={loading}
            className="resize-none"
          />
        </FormField>

        <FormField
          label="Chronic conditions"
          htmlFor="chronic_conditions"
          hint="Long-term diagnoses that affect care decisions."
        >
          <Textarea
            id="chronic_conditions"
            placeholder="Type 2 diabetes, Hypertension, Asthma"
            rows={2}
            value={chronicConditions}
            onChange={(e) => setChronicConditions(e.target.value)}
            disabled={loading}
            className="resize-none"
          />
        </FormField>
      </FormSection>

      <FormSection
        title="Emergency contact"
        description="Person to reach if the patient is unreachable or incapacitated."
      >
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <FormField label="Contact name" htmlFor="emergency_contact_name">
            <Input
              id="emergency_contact_name"
              placeholder="Jane Doe"
              value={emergencyName}
              onChange={(e) => setEmergencyName(e.target.value)}
              disabled={loading}
            />
          </FormField>
          <FormField label="Contact phone" htmlFor="emergency_contact_phone">
            <Input
              id="emergency_contact_phone"
              placeholder="+92 300 7654321"
              value={emergencyPhone}
              onChange={(e) => setEmergencyPhone(e.target.value)}
              disabled={loading}
              inputMode="tel"
            />
          </FormField>
        </div>
      </FormSection>

      <FormSection
        title="Insurance"
        description="Provider and policy for billing claims."
      >
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <FormField label="Insurance provider" htmlFor="insurance_provider">
            <Input
              id="insurance_provider"
              placeholder="Jubilee Health Insurance"
              value={insuranceProvider}
              onChange={(e) => setInsuranceProvider(e.target.value)}
              disabled={loading}
            />
          </FormField>
          <FormField label="Policy number" htmlFor="insurance_policy_number">
            <Input
              id="insurance_policy_number"
              placeholder="JHI-2024-001234"
              value={insurancePolicyNumber}
              onChange={(e) => setInsurancePolicyNumber(e.target.value)}
              disabled={loading}
            />
          </FormField>
        </div>
      </FormSection>

      {ehrLoading && isEdit && (
        <p className="text-xs text-muted-foreground">Loading medical record…</p>
      )}

      {isEdit && patient?.id ? (
        <ConsentPanel
          patientId={patient.id}
          patientName={`${firstName} ${lastName}`.trim()}
        />
      ) : null}

      <ActionBar>
        <Button
          type="button"
          variant="outline"
          onClick={onCancel}
          disabled={loading}
        >
          Cancel
        </Button>
        <Button type="submit" disabled={loading}>
          {loading
            ? "Saving..."
            : isEdit
              ? "Save changes"
              : "Register patient"}
        </Button>
      </ActionBar>
    </form>
  );
}
