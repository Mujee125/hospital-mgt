import { useState } from "react";
import { flushSync } from "react-dom";
import { Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { toast } from "sonner";
import {
  useCreateDoctor,
  useUpdateDoctor,
  useSpecializations,
  useDoctors,
} from "@/lib/queries";
import { ActionBar, FormField } from "@/components/layout/shared";
import { MultiSelectChips } from "@/components/forms/MultiSelectChips";
import {
  SPECIALIZATION_PRESETS,
  QUALIFICATION_PRESETS,
  mergeOptions,
  parseSelection,
} from "@/lib/clinicalPresets";

interface Doctor {
  id?: number;
  first_name: string;
  last_name: string;
  email: string | null;
  phone: string;
  specialization: string;
  qualification: string;
  available_from: string; // "HH:MM:SS" or "HH:MM"
  available_to: string;
  is_active: boolean;
}

interface DoctorFormProps {
  doctor?: Doctor;
  onSuccess: () => void;
  onCancel: () => void;
}

export function DoctorForm({ doctor, onSuccess, onCancel }: DoctorFormProps) {
  const isEdit = !!doctor?.id;
  const createDoctor = useCreateDoctor();
  const updateDoctor = useUpdateDoctor();
  const loading = createDoctor.isPending || updateDoctor.isPending;

  const [firstName, setFirstName] = useState(doctor?.first_name || "");
  const [lastName, setLastName] = useState(doctor?.last_name || "");
  const [email, setEmail] = useState(doctor?.email || "");
  const [phone, setPhone] = useState(doctor?.phone || "");
  const [specialization, setSpecialization] = useState(doctor?.specialization || "");
  const [qualification, setQualification] = useState(doctor?.qualification || "");

  // Selectable-option state (UX-2026-09-13): the specialization dropdown
  // is curated presets ∪ the DB-distinct list (get_specializations is
  // SELECT DISTINCT over doctors) ∪ options added inline this session —
  // a newly typed option persists automatically once a doctor is saved
  // with it. Qualification options are derived client-side from the
  // registered roster (the backend exposes no get_qualifications command;
  // each doctor's stored value is comma-separated) ∪ curated presets.
  const { data: dbSpecializations = [] } = useSpecializations();
  const { data: roster = [] } = useDoctors();
  const [extraSpecOptions, setExtraSpecOptions] = useState<string[]>([]);
  const [addingSpec, setAddingSpec] = useState(false);
  const [specDraft, setSpecDraft] = useState("");

  const specializationOptions = mergeOptions(
    SPECIALIZATION_PRESETS,
    dbSpecializations,
    extraSpecOptions,
  );
  const qualificationOptions = mergeOptions(
    QUALIFICATION_PRESETS,
    roster.flatMap((d) => parseSelection(d.qualification)),
  );


  /** + Add (specialization): register the typed option and select it.
   *  The option registration is flushed synchronously BEFORE the value
   *  flips: Radix's hidden form-integration <select> (SelectBubbleInput)
   *  reacts to a controlled value CHANGE by assigning the value and
   *  dispatching a native change event — against whatever <option>s are
   *  mounted at that moment. If the new option hasn't registered/remounted
   *  yet, the browser clamps the value to "" and the change ECHOES
   *  onValueChange(""), silently discarding the selection (reproduced
   *  and caught by the unit test; same path in real browsers). */
  const commitSpecDraft = () => {
    const trimmed = specDraft.trim();
    if (trimmed === "") return;
    flushSync(() => setExtraSpecOptions((prev) => [...prev, trimmed]));
    setSpecialization(trimmed);
    setSpecDraft("");
    setAddingSpec(false);
  };

  const formatTimeForInput = (timeStr?: string) => {
    if (!timeStr) return "";
    return timeStr.slice(0, 5);
  };

  const [availableFrom, setAvailableFrom] = useState(
    formatTimeForInput(doctor?.available_from) || "09:00",
  );
  const [availableTo, setAvailableTo] = useState(
    formatTimeForInput(doctor?.available_to) || "17:00",
  );
  const [isActive, setIsActive] = useState(
    doctor !== undefined ? doctor.is_active : true,
  );

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (
      !firstName ||
      !lastName ||
      !phone ||
      !specialization ||
      !qualification ||
      !availableFrom ||
      !availableTo
    ) {
      toast.error("Please fill in all required fields.");
      return;
    }

    const payload = {
      first_name: firstName,
      last_name: lastName,
      email: email.trim() === "" ? null : email,
      phone,
      specialization,
      qualification,
      available_from: availableFrom,
      available_to: availableTo,
    };

    try {
      if (isEdit && doctor?.id) {
        await updateDoctor.mutateAsync({
          id: doctor.id,
          is_active: isActive,
          ...payload,
        });
      } else {
        await createDoctor.mutateAsync(payload);
      }
      onSuccess();
    } catch {
      /* toast already shown by the mutation's onError */
    }
  };

  return (
    <form onSubmit={handleSubmit} className="form-stack">
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
        <FormField label="First name" htmlFor="first_name" required>
          <Input
            id="first_name"
            placeholder="Sarah"
            value={firstName}
            onChange={(e) => setFirstName(e.target.value)}
            disabled={loading}
            required
          />
        </FormField>
        <FormField label="Last name" htmlFor="last_name" required>
          <Input
            id="last_name"
            placeholder="Smith"
            value={lastName}
            onChange={(e) => setLastName(e.target.value)}
            disabled={loading}
            required
          />
        </FormField>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
        <FormField label="Contact phone" htmlFor="phone" required>
          <Input
            id="phone"
            placeholder="+1 555-0144"
            value={phone}
            onChange={(e) => setPhone(e.target.value)}
            disabled={loading}
            required
          />
        </FormField>
        <FormField label="Professional email" htmlFor="email">
          <Input
            id="email"
            type="email"
            placeholder="dr.smith@vitalflow.com"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            disabled={loading}
          />
        </FormField>
      </div>

      {/* Specialization + Qualifications: selectable options with an
          inline "+ Add" path for anything the lists don't cover. */}
      <div className="grid grid-cols-1 gap-4">
        <FormField
          label="Specialization"
          htmlFor="specialization"
          required
          hint="Pick from the list — or use the + button to add a new option."
        >
          <div className="flex gap-2">
            <Select
              value={specialization}
              onValueChange={(val) => setSpecialization(val)}
              disabled={loading}
            >
              <SelectTrigger id="specialization" className="flex-1">
                <SelectValue placeholder="Select specialization" />
              </SelectTrigger>
              <SelectContent>
                {specializationOptions.map((s) => (
                  <SelectItem key={s} value={s}>
                    {s}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Button
              type="button"
              variant="outline"
              size="icon"
              aria-label="Add new specialization option"
              title="Add a new specialization option"
              onClick={() => {
                setAddingSpec((v) => !v);
                setSpecDraft("");
              }}
              disabled={loading}
              className="h-10 w-10 shrink-0"
            >
              <Plus className="h-4 w-4" />
            </Button>
          </div>
          {addingSpec && (
            <div className="flex gap-2 mt-2">
              <Input
                id="specialization_new"
                value={specDraft}
                onChange={(e) => setSpecDraft(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    // Never submit the parent form from this input.
                    e.preventDefault();
                    commitSpecDraft();
                  }
                }}
                placeholder="Type the new specialization"
                autoFocus
              />
              <Button
                type="button"
                size="sm"
                variant="outline"
                onClick={commitSpecDraft}
                disabled={specDraft.trim() === ""}
                className="shrink-0 gap-1"
                aria-label="Add specialization option"
              >
                <Plus className="h-3.5 w-3.5" /> Add
              </Button>
            </div>
          )}
        </FormField>

        <FormField
          label="Qualifications"
          htmlFor="qualification"
          required
          hint="Select one or more — or type others and click Add."
        >
          <MultiSelectChips
            id="qualification"
            label="Qualifications"
            options={qualificationOptions}
            value={qualification}
            onChange={setQualification}
            disabled={loading}
            placeholder="Add other qualification…"
          />
        </FormField>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
        <FormField label="Availability starts" htmlFor="avail_from" required>
          <Input
            id="avail_from"
            type="time"
            value={availableFrom}
            onChange={(e) => setAvailableFrom(e.target.value)}
            disabled={loading}
            required
          />
        </FormField>
        <FormField label="Availability ends" htmlFor="avail_to" required>
          <Input
            id="avail_to"
            type="time"
            value={availableTo}
            onChange={(e) => setAvailableTo(e.target.value)}
            disabled={loading}
            required
          />
        </FormField>
      </div>

      {isEdit && (
        <div className="flex items-center gap-3 p-3 rounded-[var(--radius-md)] bg-muted/40 border border-border select-none">
          <input
            id="active-toggle"
            type="checkbox"
            checked={isActive}
            onChange={(e) => setIsActive(e.target.checked)}
            disabled={loading}
            className="h-4 w-4 rounded-[var(--radius-sm)] border border-border accent-primary cursor-pointer focus:ring-2 focus:ring-primary/30 focus:ring-offset-2 focus:ring-offset-background"
          />
          <Label
            htmlFor="active-toggle"
            className="text-sm font-medium normal-case tracking-normal cursor-pointer"
          >
            Active status (doctor is currently accepting appointments)
          </Label>
        </div>
      )}

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
          {loading ? "Saving..." : isEdit ? "Save changes" : "Register doctor"}
        </Button>
      </ActionBar>
    </form>
  );
}
