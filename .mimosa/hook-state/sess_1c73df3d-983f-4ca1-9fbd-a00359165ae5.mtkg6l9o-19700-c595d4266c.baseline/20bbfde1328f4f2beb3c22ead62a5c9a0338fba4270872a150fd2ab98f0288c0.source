import { useState } from "react";
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
import { useCreateAppointment, useUpdateAppointment } from "@/lib/queries";
import type { Patient, Doctor } from "@/lib/models";
import { ActionBar, FormField } from "@/components/layout/shared";

interface Appointment {
  id?: number;
  patient_id: number;
  doctor_id: number;
  appointment_date: string; // "YYYY-MM-DD"
  appointment_time: string; // "HH:MM:SS" or "HH:MM"
  duration_minutes: number;
  status: string;
  reason: string | null;
  notes: string | null;
}

interface AppointmentFormProps {
  appointment?: Appointment;
  patients: Patient[];
  doctors: Doctor[];
  onSuccess: (newAppointmentId?: number) => void;
  onCancel: () => void;
}

export function AppointmentForm({
  appointment,
  patients,
  doctors,
  onSuccess,
  onCancel,
}: AppointmentFormProps) {
  const isEdit = !!appointment?.id;
  const createAppointment = useCreateAppointment();
  const updateAppointment = useUpdateAppointment();
  const loading = createAppointment.isPending || updateAppointment.isPending;

  const [patientId, setPatientId] = useState<number>(
    appointment?.patient_id || patients[0]?.id || 0,
  );
  const [doctorId, setDoctorId] = useState<number>(
    appointment?.doctor_id ||
      doctors.filter((d) => d.is_active)[0]?.id ||
      0,
  );
  const [date, setDate] = useState(
    appointment?.appointment_date ||
      new Date().toISOString().split("T")[0],
  );

  const formatTime = (timeStr?: string) => {
    if (!timeStr) return "09:00";
    return timeStr.slice(0, 5);
  };
  const [time, setTime] = useState(formatTime(appointment?.appointment_time));
  const [duration, setDuration] = useState<number>(
    appointment?.duration_minutes || 30,
  );
  const [status, setStatus] = useState(appointment?.status || "scheduled");
  const [reason, setReason] = useState(appointment?.reason || "");
  const [notes, setNotes] = useState(appointment?.notes || "");

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!patientId || !doctorId || !date || !time) {
      toast.error("Please ensure patient, doctor, date, and time are selected.");
      return;
    }

    const payload = {
      patient_id: Number(patientId),
      doctor_id: Number(doctorId),
      appointment_date: date,
      appointment_time: time,
      duration_minutes: Number(duration),
      reason: reason.trim() === "" ? null : reason,
      notes: notes.trim() === "" ? null : notes,
    };

    try {
      if (isEdit && appointment?.id) {
        await updateAppointment.mutateAsync({
          id: appointment.id,
          status,
          ...payload,
        });
        onSuccess();
      } else {
        const newId = await createAppointment.mutateAsync(payload);
        onSuccess(newId);
      }
    } catch {
      /* toast already shown by the mutation's onError */
    }
  };

  const activeDoctors = doctors.filter(
    (d) => d.is_active || d.id === appointment?.doctor_id,
  );

  return (
    <form onSubmit={handleSubmit} className="form-stack">
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
        <FormField label="Select patient" htmlFor="patient_select" required>
          <Select
            value={patientId ? String(patientId) : undefined}
            onValueChange={(val) => setPatientId(Number(val))}
            disabled={loading || isEdit}
          >
            <SelectTrigger id="patient_select">
              <SelectValue
                placeholder={
                  patients.length === 0
                    ? "No patients registered"
                    : "Select patient"
                }
              />
            </SelectTrigger>
            <SelectContent>
              {patients.map((p) => (
                <SelectItem key={p.id} value={String(p.id)}>
                  {p.first_name} {p.last_name} ({p.phone})
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </FormField>

        <FormField label="Select practitioner" htmlFor="doctor_select" required>
          <Select
            value={doctorId ? String(doctorId) : undefined}
            onValueChange={(val) => setDoctorId(Number(val))}
            disabled={loading}
          >
            <SelectTrigger id="doctor_select">
              <SelectValue
                placeholder={
                  activeDoctors.length === 0
                    ? "No active doctors registered"
                    : "Select practitioner"
                }
              />
            </SelectTrigger>
            <SelectContent>
              {activeDoctors.map((d) => (
                <SelectItem key={d.id} value={String(d.id)}>
                  Dr. {d.first_name} {d.last_name} ({d.specialization})
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </FormField>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
        <FormField label="Date" htmlFor="app_date" required>
          <Input
            id="app_date"
            type="date"
            value={date}
            onChange={(e) => setDate(e.target.value)}
            disabled={loading}
            required
          />
        </FormField>
        <FormField label="Time" htmlFor="app_time" required>
          <Input
            id="app_time"
            type="time"
            value={time}
            onChange={(e) => setTime(e.target.value)}
            disabled={loading}
            required
          />
        </FormField>
        <FormField label="Duration (mins)" htmlFor="app_duration" required>
          <Input
            id="app_duration"
            type="number"
            value={duration}
            onChange={(e) => setDuration(Number(e.target.value))}
            disabled={loading}
            required
            min={5}
            max={180}
          />
        </FormField>
      </div>

      {isEdit && (
        <FormField label="Appointment status" htmlFor="status_select" required>
          <Select
            value={status}
            onValueChange={(val) => setStatus(val)}
            disabled={loading}
          >
            <SelectTrigger id="status_select">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="scheduled">Scheduled</SelectItem>
              <SelectItem value="confirmed">Confirmed</SelectItem>
              <SelectItem value="completed">Completed</SelectItem>
              <SelectItem value="cancelled">Cancelled</SelectItem>
              <SelectItem value="no-show">No-show</SelectItem>
            </SelectContent>
          </Select>
        </FormField>
      )}

      <FormField label="Reason for appointment" htmlFor="reason">
        <Input
          id="reason"
          placeholder="Routine checkup, symptoms check, follow-up..."
          value={reason}
          onChange={(e) => setReason(e.target.value)}
          disabled={loading}
        />
      </FormField>

      <FormField label="Practitioner's notes (internal)" htmlFor="notes">
        <Textarea
          id="notes"
          placeholder="Symptoms, medication logs, future recommendations..."
          rows={3}
          value={notes}
          onChange={(e) => setNotes(e.target.value)}
          disabled={loading}
          className="resize-none"
        />
      </FormField>

      <ActionBar>
        <Button
          type="button"
          variant="outline"
          onClick={onCancel}
          disabled={loading}
        >
          Cancel
        </Button>
        <Button
          type="submit"
          disabled={
            loading || patients.length === 0 || activeDoctors.length === 0
          }
        >
          {loading
            ? "Saving..."
            : isEdit
              ? "Save changes"
              : "Book appointment"}
        </Button>
      </ActionBar>
    </form>
  );
}
