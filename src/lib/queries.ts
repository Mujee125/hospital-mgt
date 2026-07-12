/**
 * React Query hooks for every backend command. Centralizing these means:
 *  - One canonical query key per resource (no more ad hoc refetchKey/
 *    remount-via-key tricks like the old `key={`pts-${refreshKey}`}`
 *    pattern in App.tsx).
 *  - Every mutation invalidates exactly the queries it actually affects,
 *    so the UI updates immediately after create/update/delete without
 *    a manual reload.
 *  - Toasts live here once, not copy-pasted into every page's try/catch.
 */
import { invoke } from "@tauri-apps/api/core";
import { useQuery, useMutation, useQueryClient, type UseQueryOptions } from "@tanstack/react-query";
import { toast } from "sonner";
import type {
  Patient,
  PatientEhr,
  CreatePatientEhr,
  UpdatePatientEhr,
  PatientConsent,
  Doctor,
  AppointmentWithDetails,
  AppointmentStats,
  ChatMessage,
  Encounter,
  QueueToken,
  Ward,
  Bed,
  IpdAdmission,
  LabTestCatalog,
  LabOrder,
  LabOrderTest,
  Bill,
  BillItem,
  Payment,
  DashboardKpis,
  AuditLog,
  LicenseInfo,
  UserProfile,
  InventoryItem,
  CreateInventoryItem,
  UpdateInventoryItem,
  InventoryMovement,
  NotificationLogEntry,
  RevenueReport,
  DailyOpdReport,
  IpdCensusReport,
  LabTurnaroundReport,
  BackupInfo,
  Medication,
  CreateMedication,
  UpdateMedication,
  Prescription,
  PrescriptionWithItems,
  CreatePrescription,
  RadiologyOrder,
  RadiologyOrdersResponse,
  CreateRadiologyOrder,
  RadiologyReport,
  CreateRadiologyReport,
  RadiologyDashboard,
  BloodDonor,
  BloodDonorsResponse,
  CreateBloodDonor,
  BloodDonationsResponse,
  CreateBloodDonation,
  BloodUnit,
  BloodUnitsResponse,
  CreateBloodUnit,
  BloodCrossmatchesResponse,
  CreateBloodCrossmatch,
  BloodIssuesResponse,
  CreateBloodIssue,
  BloodTransfusionsResponse,
  CreateBloodTransfusion,
  BloodDiscardsResponse,
  CreateBloodDiscard,
  CreateBloodReservation,
  BloodUnitHistory,
  BloodMovement,
  BloodBankDashboard,
  BloodBankStatistics,
  BloodCompatibilityResult,
  BloodUnitTraceability,
} from "./models";
import type { Session } from "./rbac";

// ── Query keys ───────────────────────────────────────────────────────────
//
// STATE-02: previously `usePatients` (basic `Patient[]`) and `usePatientsEhr`
// (EHR-expanded `PatientEhr[]`) both used `["patients", null]` as their
// query key but typed their results differently. TanStack Query cached a
// single entry under that key and returned it for whichever hook had
// mounted last — Patients.tsx and Appointments.tsx lost type access to the
// EHR-only fields (allergies, chronic_conditions, etc.) even when the cache
// actually held an EHR row, because the cache slot's type had been narrowed
// by the last writer. The keys are now distinct:
//   - basic list:   ["patients", "list", search]
//   - ehr   list:   ["patients", "ehr-list", search]
//   - basic single: ["patients", "by-id", id]
//   - ehr   single: ["patients", "ehr-by-id", id]
// Mutations still invalidate the broad `["patients"]` prefix so both caches
// refresh together (a write to one shape affects the other shape's view).
export const qk = {
  patients: (search?: string | null) => ["patients", "list", search ?? null] as const,
  patientsEhr: (search?: string | null) => ["patients", "ehr-list", search ?? null] as const,
  patient: (id: number) => ["patients", "by-id", id] as const,
  patientEhr: (id: number) => ["patients", "ehr-by-id", id] as const,
  patientConsent: (patientId: number) => ["patients", "consent", patientId] as const,
  // doctors: (search?: string | null, specialization?: string | null) =>
  //   ["doctors", search ?? null, specialization ?? null] as const,

  doctors: (activeOnly?: boolean | null) =>
    ["doctors", activeOnly ?? null] as const,

  doctor: (id: number) => ["doctors", id] as const,
  specializations: () => ["doctors", "specializations"] as const,
  appointments: (dateFilter?: string | null, statusFilter?: string | null) =>
    ["appointments", dateFilter ?? null, statusFilter ?? null] as const,
  appointment: (id: number) => ["appointments", id] as const,
  todayAppointments: () => ["appointments", "today"] as const,
  appointmentStats: () => ["appointments", "stats"] as const,
  messages: (room: string) => ["messages", room] as const,
  rooms: () => ["messages", "rooms"] as const,
  notificationLog: () => ["whatsapp", "log"] as const,
  config: () => ["config"] as const,
  inventoryItems: (categoryFilter?: string | null, lowStockOnly?: boolean | null) =>
    ["inventory", "items", categoryFilter ?? null, lowStockOnly ?? null] as const,
  inventoryItem: (id: number) => ["inventory", "items", id] as const,
  inventoryMovements: (itemId?: number | null, limit?: number | null) =>
    ["inventory", "movements", itemId ?? null, limit ?? null] as const,
};

// ── Patients ─────────────────────────────────────────────────────────────

export function usePatients(search?: string | null, options?: Partial<UseQueryOptions<Patient[]>>) {
  return useQuery({
    queryKey: qk.patients(search),
    queryFn: () => invoke<Patient[]>("get_patients", { search: search ?? null }),
    ...options,
  });
}

export function useCreatePatient() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (payload: Record<string, unknown>) => invoke<number>("create_patient", { patient: payload }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["patients"] });
      toast.success("Patient registered successfully!");
    },
    onError: (err) => toast.error(`Failed to register patient: ${err}`),
  });
}

export function useUpdatePatient() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (payload: Record<string, unknown>) => invoke("update_patient", { patient: payload }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["patients"] });
      toast.success("Patient updated successfully!");
    },
    onError: (err) => toast.error(`Failed to update patient: ${err}`),
  });
}

export function useDeletePatient() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => invoke("delete_patient", { id }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["patients"] });
      toast.success("Patient record removed.");
    },
    onError: (err) => toast.error(`Failed to delete patient: ${err}`),
  });
}

// ── Doctors ──────────────────────────────────────────────────────────────
// NOTE: get_doctors only supports an active_only filter server-side.
// Search-by-name/specialization is applied client-side over the full list
// (matches the existing pre-restyle behavior in Doctors.tsx — there's no
// search/specialization parameter on the Rust command).

export function useDoctors(activeOnly?: boolean) {
  return useQuery({
    queryKey: qk.doctors(activeOnly),
    queryFn: () => invoke<Doctor[]>("get_doctors", { activeOnly: activeOnly ?? null }),
  });
}

export function useSpecializations() {
  return useQuery({
    queryKey: qk.specializations(),
    queryFn: () => invoke<string[]>("get_specializations"),
    staleTime: 5 * 60_000,
  });
}

export function useCreateDoctor() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (payload: Record<string, unknown>) => invoke<number>("create_doctor", { doctor: payload }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["doctors"] });
      toast.success("Doctor added successfully!");
    },
    onError: (err) => toast.error(`Failed to add doctor: ${err}`),
  });
}

export function useUpdateDoctor() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (payload: Record<string, unknown>) => invoke("update_doctor", { doctor: payload }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["doctors"] });
      toast.success("Doctor profile updated!");
    },
    onError: (err) => toast.error(`Failed to update doctor: ${err}`),
  });
}

export function useDeleteDoctor() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => invoke("delete_doctor", { id }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["doctors"] });
      toast.success("Doctor removed from roster.");
    },
    onError: (err) => toast.error(`Failed to remove doctor: ${err}`),
  });
}

// ── Appointments ─────────────────────────────────────────────────────────

export function useAppointments(dateFilter?: string | null, statusFilter?: string | null) {
  return useQuery({
    queryKey: qk.appointments(dateFilter, statusFilter),
    queryFn: () =>
      invoke<AppointmentWithDetails[]>("get_appointments", {
        dateFilter: dateFilter ?? null,
        statusFilter: statusFilter ?? null,
      }),
  });
}

export function useAppointment(id: number | null) {
  return useQuery({
    queryKey: qk.appointment(id ?? -1),
    queryFn: () => invoke<AppointmentWithDetails>("get_appointment", { id }),
    enabled: id != null,
  });
}

export function useTodayAppointments() {
  return useQuery({
    queryKey: qk.todayAppointments(),
    queryFn: () => invoke<AppointmentWithDetails[]>("get_today_appointments"),
  });
}

export function useAppointmentStats() {
  return useQuery({
    queryKey: qk.appointmentStats(),
    queryFn: () => invoke<AppointmentStats>("get_appointment_stats"),
  });
}

function invalidateAppointmentQueries(qc: ReturnType<typeof useQueryClient>) {
  qc.invalidateQueries({ queryKey: ["appointments"] });
}

export function useCreateAppointment() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (payload: Record<string, unknown>) =>
      invoke<number>("create_appointment", { appointment: payload }),
    onSuccess: () => {
      invalidateAppointmentQueries(qc);
      toast.success("Appointment scheduled successfully!");
    },
    onError: (err) => toast.error(`Failed to schedule appointment: ${err}`),
  });
}

export function useUpdateAppointment() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (payload: Record<string, unknown>) => invoke("update_appointment", { appointment: payload }),
    onSuccess: () => {
      invalidateAppointmentQueries(qc);
      toast.success("Appointment updated successfully!");
    },
    onError: (err) => toast.error(`Failed to update appointment: ${err}`),
  });
}

export function useUpdateAppointmentStatus() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, status }: { id: number; status: string }) =>
      invoke("update_appointment_status", { id, status }),
    onSuccess: () => {
      invalidateAppointmentQueries(qc);
      toast.success("Status updated.");
    },
    onError: (err) => toast.error(`Failed to update status: ${err}`),
  });
}

export function useDeleteAppointment() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => invoke("delete_appointment", { id }),
    onSuccess: () => {
      invalidateAppointmentQueries(qc);
      toast.success("Appointment removed.");
    },
    onError: (err) => toast.error(`Failed to delete appointment: ${err}`),
  });
}

// ── Messaging ────────────────────────────────────────────────────────────

export function useMessages(room: string, limit = 100) {
  return useQuery({
    queryKey: qk.messages(room),
    queryFn: () => invoke<ChatMessage[]>("get_messages", { room, limit }),
    refetchInterval: 4000, // staff chat — light polling until a push channel exists
  });
}

export function useRooms() {
  return useQuery({
    queryKey: qk.rooms(),
    queryFn: () => invoke<string[]>("get_rooms"),
    staleTime: 60_000,
  });
}

export function useSendMessage() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (payload: { sender: string; content: string; room: string }) =>
      invoke<ChatMessage>("send_message", { message: payload }),
    onSuccess: (_data, variables) => {
      qc.invalidateQueries({ queryKey: qk.messages(variables.room) });
    },
    onError: (err) => toast.error(`Failed to send message: ${err}`),
  });
}

export function useDeleteMessage() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => invoke("delete_message", { id }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["messages"] });
    },
    onError: (err) => toast.error(`Failed to delete message: ${err}`),
  });
}

// ── WhatsApp ─────────────────────────────────────────────────────────────

export function useNotificationLog(limit = 50) {
  return useQuery({
    queryKey: qk.notificationLog(),
    queryFn: () => invoke<NotificationLogEntry[]>("get_notification_log", { limit }),
  });
}

export function useSendWhatsAppNotification() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (message: Record<string, unknown>) => invoke("send_whatsapp_notification", { message }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: qk.notificationLog() });
      toast.success("WhatsApp opened — check your client to send the message.");
    },
    onError: (err) => toast.error(`Failed to send WhatsApp message: ${err}`),
  });
}

export function useSendWhatsAppToPatient() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ phone, message, notificationType }: { phone: string; message: string; notificationType?: string }) =>
      invoke("send_whatsapp_to_patient", { phone, message, notificationType }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: qk.notificationLog() });
      toast.success("WhatsApp opened — check your client to send the message.");
    },
    onError: (err) => toast.error(String(err)),
  });
}

export function useSendWhatsAppTest() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ phone, clinicName }: { phone: string; clinicName?: string }) =>
      invoke<string>("send_whatsapp_test", { phone, clinicName }),
    onSuccess: (msg) => {
      qc.invalidateQueries({ queryKey: qk.notificationLog() });
      toast.success(msg);
    },
    onError: (err) => toast.error(String(err)),
  });
}

// ── Authentication ───────────────────────────────────────────────────────

export function useLogin() {
  return useMutation({
    mutationFn: (creds: { username: string; password: string }) =>
      invoke<Session>("login", { request: creds }),
    onError: (err) => toast.error(String(err)),
  });
}

export function useLogout() {
  return useMutation({
    mutationFn: () => invoke("logout"),
  });
}

export function useMe() {
  return useQuery({
    queryKey: ["auth", "me"],
    queryFn: () => invoke<Session>("me"),
    retry: false,
    staleTime: 0,
  });
}

export function useChangePassword() {
  return useMutation({
    mutationFn: (req: { current_password: string; new_password: string }) =>
      invoke("change_password", { request: req }),
    onSuccess: () => toast.success("Password changed successfully."),
    onError: (err) => toast.error(String(err)),
  });
}

// ── User management ──────────────────────────────────────────────────────

export function useUsers() {
  return useQuery({
    queryKey: ["users"],
    queryFn: () => invoke<UserProfile[]>("list_users"),
  });
}

export function useRoles() {
  return useQuery({
    queryKey: ["roles"],
    queryFn: () => invoke<[number, string, string][]>("list_roles"),
    staleTime: 5 * 60_000,
  });
}

export function useUserRoles(userId: number | null) {
  return useQuery({
    queryKey: ["users", userId, "roles"],
    queryFn: () => invoke<string[]>("list_user_roles", { userId }),
    enabled: userId != null,
  });
}

export function useCreateUser() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: {
      username: string;
      full_name: string;
      email: string | null;
      password: string;
      roles: string[];
      must_change_password?: boolean;
    }) => invoke<number>("create_user", { request: req }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["users"] });
      toast.success("User created.");
    },
    onError: (err) => toast.error(String(err)),
  });
}

export function useUpdateUser() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: {
      id: number;
      full_name?: string;
      email?: string;
      is_active?: boolean;
      roles?: string[];
    }) => invoke("update_user", { request: req }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["users"] });
      toast.success("User updated.");
    },
    onError: (err) => toast.error(String(err)),
  });
}

export function useDeleteUser() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => invoke("delete_user", { id }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["users"] });
      toast.success("User removed.");
    },
    onError: (err) => toast.error(String(err)),
  });
}

export function useResetUserPassword() {
  return useMutation({
    mutationFn: ({ id, newPassword }: { id: number; newPassword: string }) =>
      invoke("reset_user_password", { id, newPassword }),
    onSuccess: () => toast.success("Password reset. User must change it at next login."),
    onError: (err) => toast.error(String(err)),
  });
}

// ── Dashboard ────────────────────────────────────────────────────────────

export function useDashboardKpis() {
  return useQuery({
    queryKey: ["dashboard", "kpis"],
    queryFn: () => invoke<DashboardKpis>("get_dashboard_kpis"),
    refetchInterval: 30_000,
  });
}

// ── Patients (EHR) ───────────────────────────────────────────────────────
// EHR-expanded variants. STATE-02: these now use DISTINCT query keys from
// the basic `usePatients` / `usePatient` hooks so the two result shapes
// don't share a cache slot. The backend `get_patients` / `get_patient`
// commands return the EHR shape (the Rust `PatientEhr` struct) for both —
// the basic `Patient` interface in models.ts simply narrows the type at
// the TS layer. The split keys keep the two callers' cache entries from
// clobbering each other on type assertions.

export function usePatientsEhr(search?: string | null) {
  return useQuery({
    queryKey: qk.patientsEhr(search),
    queryFn: () => invoke<PatientEhr[]>("get_patients", { search: search ?? null }),
  });
}

export function usePatientEhr(id: number | null) {
  return useQuery({
    queryKey: qk.patientEhr(id ?? -1),
    queryFn: () => invoke<PatientEhr>("get_patient", { id }),
    enabled: id != null,
  });
}

// ── Patient consent (CR-12, SRS FR-0035) ─────────────────────────────────
// The `whatsapp` consent type gates outbound WhatsApp messages
// (`whatsapp::automation::send_whatsapp`). The UI lets the operator grant
// or revoke that consent from the patient record.

export function usePatientConsent(patientId: number | null) {
  return useQuery({
    queryKey: qk.patientConsent(patientId ?? -1),
    queryFn: () => invoke<PatientConsent | null>("get_patient_consent", { patientId }),
    enabled: patientId != null,
  });
}

export function useSetPatientConsent() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: {
      patient_id: number;
      consent_type: string;
      granted: boolean;
      notes?: string | null;
    }) =>
      invoke<number>("set_patient_consent", {
        patientId: req.patient_id,
        consentType: req.consent_type,
        granted: req.granted,
        notes: req.notes ?? null,
      }),
    onSuccess: (_data, vars) => {
      qc.invalidateQueries({ queryKey: qk.patientConsent(vars.patient_id) });
      toast.success(
        vars.granted
          ? "Patient consent granted."
          : "Patient consent revoked.",
      );
    },
    onError: (err) => toast.error(`Failed to update consent: ${err}`),
  });
}

export function useRevokePatientConsent() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: { patient_id: number; consent_type: string }) =>
      invoke<void>("revoke_patient_consent", {
        patientId: req.patient_id,
        consentType: req.consent_type,
      }),
    onSuccess: (_data, vars) => {
      qc.invalidateQueries({ queryKey: qk.patientConsent(vars.patient_id) });
      toast.success("Patient consent revoked.");
    },
    onError: (err) => toast.error(`Failed to revoke consent: ${err}`),
  });
}

export function useCreatePatientEhr() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (payload: CreatePatientEhr) => invoke<number>("create_patient", { patient: payload }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["patients"] });
      toast.success("Patient registered successfully!");
    },
    onError: (err) => toast.error(`Failed to register patient: ${err}`),
  });
}

export function useUpdatePatientEhr() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (payload: UpdatePatientEhr) => invoke("update_patient", { patient: payload }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["patients"] });
      toast.success("Patient record updated.");
    },
    onError: (err) => toast.error(`Failed to update patient: ${err}`),
  });
}

// ── Encounters ───────────────────────────────────────────────────────────

export function useEncounters(patientId?: number | null) {
  return useQuery({
    queryKey: ["encounters", patientId ?? null],
    queryFn: () => invoke<Encounter[]>("get_encounters", { patientId: patientId ?? null }),
  });
}

export function useCreateEncounter() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: {
      patient_id: number;
      doctor_id?: number | null;
      visit_type?: string;
      chief_complaint?: string | null;
      diagnosis?: string | null;
      notes?: string | null;
    }) => invoke<number>("create_encounter", { encounter: req }),
    onSuccess: (_d, vars) => {
      qc.invalidateQueries({ queryKey: ["encounters"] });
      qc.invalidateQueries({ queryKey: ["patients"] });
      toast.success("Visit recorded.");
      void vars;
    },
    onError: (err) => toast.error(String(err)),
  });
}

// ── Queue ────────────────────────────────────────────────────────────────

export function useQueue(statusFilter?: string | null) {
  return useQuery({
    queryKey: ["queue", statusFilter ?? null],
    queryFn: () => invoke<QueueToken[]>("get_queue", { statusFilter: statusFilter ?? null }),
    refetchInterval: 10_000,
  });
}

export function useCreateQueueToken() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: {
      patient_id: number;
      department_id?: number | null;
      doctor_id?: number | null;
      priority?: number;
    }) => invoke<number>("create_queue_token", { token: req }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["queue"] });
      toast.success("Token issued.");
    },
    onError: (err) => toast.error(String(err)),
  });
}

export function useCallNextToken() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: { department_id?: number | null; doctor_id?: number | null }) =>
      invoke<QueueToken | null>("call_next_token", req),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["queue"] }),
    onError: (err) => toast.error(String(err)),
  });
}

export function useSetTokenStatus() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, status }: { id: number; status: string }) =>
      invoke("set_token_status", { id, status }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["queue"] }),
    onError: (err) => toast.error(String(err)),
  });
}

// ── IPD ──────────────────────────────────────────────────────────────────

export function useWards() {
  return useQuery({
    queryKey: ["wards"],
    queryFn: () => invoke<Ward[]>("get_wards"),
    staleTime: 60_000,
  });
}

export function useBeds(wardId?: number | null) {
  return useQuery({
    queryKey: ["beds", wardId ?? null],
    queryFn: () => invoke<Bed[]>("get_beds", { wardId: wardId ?? null }),
  });
}

export function useCreateWard() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: {
      name: string;
      code: string;
      floor?: string | null;
      genderRestriction?: string | null;
    }) =>
      invoke<number>("create_ward", {
        name: req.name,
        code: req.code,
        floor: req.floor ?? null,
        genderRestriction: req.genderRestriction ?? null,
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["wards"] });
      toast.success("Ward created.");
    },
    onError: (err) => toast.error(String(err)),
  });
}

export function useCreateBed() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: {
      wardId: number;
      bedNumber: string;
      isIcu?: boolean;
      dailyRate?: number | null;
    }) =>
      invoke<number>("create_bed", {
        wardId: req.wardId,
        bedNumber: req.bedNumber,
        isIcu: req.isIcu ?? false,
        dailyRate: req.dailyRate ?? null,
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["beds"] });
      toast.success("Bed created.");
    },
    onError: (err) => toast.error(String(err)),
  });
}

export function useAdmissions(statusFilter?: string | null) {
  return useQuery({
    queryKey: ["ipd", "admissions", statusFilter ?? null],
    queryFn: () => invoke<IpdAdmission[]>("get_admissions", { statusFilter: statusFilter ?? null }),
  });
}

export function useAdmitPatient() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: {
      patient_id: number;
      doctor_id?: number | null;
      ward_id: number;
      bed_id: number;
      admission_type?: string;
      admitting_diagnosis?: string | null;
      attending_doctor_id?: number | null;
    }) => invoke<number>("admit_patient", { admission: req }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["ipd"] });
      qc.invalidateQueries({ queryKey: ["beds"] });
      qc.invalidateQueries({ queryKey: ["dashboard"] });
      toast.success("Patient admitted.");
    },
    onError: (err) => toast.error(String(err)),
  });
}

export function useDischargePatient() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: { id: number; discharge_summary?: string | null }) =>
      invoke("discharge_patient", { discharge: req }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["ipd"] });
      qc.invalidateQueries({ queryKey: ["beds"] });
      qc.invalidateQueries({ queryKey: ["dashboard"] });
      toast.success("Patient discharged.");
    },
    onError: (err) => toast.error(String(err)),
  });
}

// ── Laboratory ───────────────────────────────────────────────────────────

export function useLabCatalog() {
  return useQuery({
    queryKey: ["lab", "catalog"],
    queryFn: () => invoke<LabTestCatalog[]>("get_lab_catalog"),
    staleTime: 5 * 60_000,
  });
}

export function useLabOrders(statusFilter?: string | null) {
  return useQuery({
    queryKey: ["lab", "orders", statusFilter ?? null],
    queryFn: () => invoke<LabOrder[]>("get_lab_orders", { statusFilter: statusFilter ?? null }),
  });
}

export function useLabOrderTests(labOrderId: number | null) {
  return useQuery({
    queryKey: ["lab", "orders", labOrderId, "tests"],
    queryFn: () => invoke<LabOrderTest[]>("get_lab_order_tests", { labOrderId }),
    enabled: labOrderId != null,
  });
}

export function useCreateLabOrder() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: {
      patient_id: number;
      encounter_id?: number | null;
      ordered_by_doctor_id?: number | null;
      test_catalog_ids: number[];
    }) => invoke<number>("create_lab_order", { order: req }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["lab"] });
      qc.invalidateQueries({ queryKey: ["dashboard"] });
      toast.success("Lab order placed.");
    },
    onError: (err) => toast.error(String(err)),
  });
}

export function useUpdateLabResult() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: {
      id: number;
      result_value?: string | null;
      result_unit?: string | null;
      result_abnormal_flag?: string | null;
      result_notes?: string | null;
    }) => invoke("update_lab_result", { result: req }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["lab"] });
      toast.success("Result saved.");
    },
    onError: (err) => toast.error(String(err)),
  });
}

// ── Billing ──────────────────────────────────────────────────────────────

export function useBills(statusFilter?: string | null) {
  return useQuery({
    queryKey: ["bills", statusFilter ?? null],
    queryFn: () => invoke<Bill[]>("get_bills", { statusFilter: statusFilter ?? null }),
  });
}

export function useBill(id: number | null) {
  return useQuery({
    queryKey: ["bills", id],
    queryFn: () => invoke<Bill>("get_bill", { id }),
    enabled: id != null,
  });
}

export function useBillItems(billId: number | null) {
  return useQuery({
    queryKey: ["bills", billId, "items"],
    queryFn: () => invoke<BillItem[]>("get_bill_items", { billId }),
    enabled: billId != null,
  });
}

export function usePayments(billId: number | null) {
  return useQuery({
    queryKey: ["bills", billId, "payments"],
    queryFn: () => invoke<Payment[]>("get_payments", { billId }),
    enabled: billId != null,
  });
}

export function useCreateBill() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: {
      patient_id: number;
      encounter_id?: number | null;
      ipd_admission_id?: number | null;
      bill_type?: string;
      discount?: number;
      tax?: number;
      items: {
        item_type: string;
        description: string;
        quantity: number;
        unit_price: number;
        reference_id?: number | null;
      }[];
    }) => invoke<number>("create_bill", { bill: req }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["bills"] });
      qc.invalidateQueries({ queryKey: ["dashboard"] });
      toast.success("Invoice created.");
    },
    onError: (err) => toast.error(String(err)),
  });
}

export function useRecordPayment() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: {
      bill_id: number;
      amount: number;
      payment_method?: string;
      reference_number?: string | null;
    }) => invoke<number>("record_payment", { payment: req }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["bills"] });
      qc.invalidateQueries({ queryKey: ["dashboard"] });
      toast.success("Payment recorded.");
    },
    onError: (err) => toast.error(String(err)),
  });
}

// ── Audit ────────────────────────────────────────────────────────────────

export function useAuditLogs(limit = 200, actionFilter?: string | null, resourceFilter?: string | null) {
  return useQuery({
    queryKey: ["audit", limit, actionFilter ?? null, resourceFilter ?? null],
    queryFn: () =>
      invoke<AuditLog[]>("get_audit_logs", {
        limit,
        actionFilter: actionFilter ?? null,
        resourceFilter: resourceFilter ?? null,
      }),
  });
}

// ── Licensing ────────────────────────────────────────────────────────────

export function useVerifyLicense() {
  return useQuery({
    queryKey: ["license", "verify"],
    queryFn: () => invoke<LicenseInfo>("verify_license"),
    retry: false,
    staleTime: 0,
  });
}

export function useHardwareFingerprint() {
  return useQuery({
    queryKey: ["license", "fingerprint"],
    queryFn: () => invoke<string>("get_hardware_fingerprint"),
    staleTime: Infinity,
  });
}

export function useLicenseInfo() {
  return useQuery({
    queryKey: ["license", "info"],
    queryFn: () => invoke<LicenseInfo | null>("get_license_info"),
  });
}

export function useInstallLicense() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (licenseJson: string) => invoke<LicenseInfo>("install_license", { licenseJson }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["license"] });
      toast.success("License installed and verified.");
    },
    onError: (err) => toast.error(String(err)),
  });
}

export function useInstallFingerprint() {
  return useQuery({
    queryKey: ["license", "install-fingerprint"],
    queryFn: () => invoke<{ fingerprint: string; display: string }>("get_install_fingerprint"),
    staleTime: Infinity,
  });
}

// ── WhatsApp Business API config ──────────────────────────────────────────

export function useWhatsAppConfig() {
  return useQuery({
    queryKey: ["whatsapp", "config"],
    queryFn: () => invoke<Record<string, unknown>>("get_whatsapp_config"),
  });
}

export function useSetWhatsAppConfig() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ accessToken, phoneNumberId, enabled, preferredMethod }: {
      accessToken: string;
      phoneNumberId: string;
      enabled: boolean;
      preferredMethod: "api" | "deep_link";
    }) => invoke("set_whatsapp_config", { accessToken, phoneNumberId, enabled, preferredMethod }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["whatsapp"] });
      toast.success("WhatsApp config saved.");
    },
    onError: (err) => toast.error(String(err)),
  });
}

export function useTestWhatsAppApi() {
  return useMutation({
    mutationFn: (testPhone: string) => invoke<string>("test_whatsapp_api", { testPhone }),
    onSuccess: (msg) => toast.success(msg),
    onError: (err) => toast.error(String(err)),
  });
}

// ── Inventory (CR-21, SRS FR-0180/0181/0185) ──────────────────────────────
//
// Wires the 6 inventory commands added in Batch 1 (`commands/inventory.rs`):
//   get_inventory_items(category_filter?, low_stock_only?)
//   get_inventory_item(id)
//   create_inventory_item(item: CreateInventoryItem)
//   update_inventory_item(id, item: UpdateInventoryItem)
//   adjust_inventory(item_id, quantity_change, reason)
//   get_inventory_movements(item_id?, limit?)
// Mutations invalidate the `["inventory"]` prefix so both the items list
// and the movements list refresh together (an adjust changes both).

export function useInventoryItems(
  categoryFilter?: string | null,
  lowStockOnly?: boolean | null,
) {
  return useQuery({
    queryKey: qk.inventoryItems(categoryFilter ?? null, lowStockOnly ?? null),
    queryFn: () =>
      invoke<InventoryItem[]>("get_inventory_items", {
        categoryFilter: categoryFilter ?? null,
        lowStockOnly: lowStockOnly ?? null,
      }),
  });
}

export function useInventoryItem(id: number | null) {
  return useQuery({
    queryKey: qk.inventoryItem(id ?? -1),
    queryFn: () => invoke<InventoryItem>("get_inventory_item", { id }),
    enabled: id != null,
  });
}

export function useCreateInventoryItem() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (item: CreateInventoryItem) =>
      invoke<number>("create_inventory_item", { item }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["inventory"] });
      toast.success("Inventory item created.");
    },
    onError: (err) => toast.error(`Failed to create item: ${err}`),
  });
}

export function useUpdateInventoryItem() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, item }: { id: number; item: UpdateInventoryItem }) =>
      invoke<void>("update_inventory_item", { id, item }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["inventory"] });
      toast.success("Inventory item updated.");
    },
    onError: (err) => toast.error(`Failed to update item: ${err}`),
  });
}

export function useAdjustInventory() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: { item_id: number; quantity_change: number; reason: string }) =>
      invoke<void>("adjust_inventory", {
        itemId: req.item_id,
        quantityChange: req.quantity_change,
        reason: req.reason,
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["inventory"] });
      toast.success("Stock adjusted.");
    },
    onError: (err) => toast.error(`Failed to adjust stock: ${err}`),
  });
}

export function useInventoryMovements(itemId?: number | null, limit?: number | null) {
  return useQuery({
    queryKey: qk.inventoryMovements(itemId ?? null, limit ?? null),
    queryFn: () =>
      invoke<InventoryMovement[]>("get_inventory_movements", {
        itemId: itemId ?? null,
        limit: limit ?? null,
      }),
  });
}

// ── Reports (Phase 2-A, SRS §4.20, FR-0220–FR-0223) ──────────────────────
//
// Read-only operational reports. Each query hook wraps one Reports-module
// Tauri command. All five commands are RBAC-guarded server-side by
// `Permission::ReportsView`; the backend re-checks on every call so this
// is a UX affordance, not a security control.
//
// `useDailyOpdReport` / `useIpdCensusReport` accept an optional `date`
// (`YYYY-MM-DD`); when omitted/empty, the backend defaults to today (UTC).
// `useRevenueReport` / `useLabTurnaroundReport` require a `fromDate` +
// `toDate` range (`YYYY-MM-DD`). All four query hooks re-fetch when their
// date arguments change.
//
// `useExportReportCsv` is a mutation: it calls the generic CSV exporter
// with `report_type` + a JSON `params` object and returns the CSV string.
// The caller is responsible for wrapping it in a Blob + anchor download
// (see `src/pages/Reports.tsx::downloadCsvString`).

export function useDailyOpdReport(date?: string | null) {
  return useQuery({
    queryKey: ["reports", "daily-opd", date ?? null],
    queryFn: () =>
      invoke<DailyOpdReport>("get_daily_opd_report", {
        date: date && date.length > 0 ? date : null,
      }),
  });
}

export function useIpdCensusReport(date?: string | null) {
  return useQuery({
    queryKey: ["reports", "ipd-census", date ?? null],
    queryFn: () =>
      invoke<IpdCensusReport>("get_ipd_census_report", {
        date: date && date.length > 0 ? date : null,
      }),
  });
}

export function useRevenueReport(fromDate: string, toDate: string) {
  return useQuery({
    queryKey: ["reports", "revenue", fromDate, toDate],
    queryFn: () =>
      invoke<RevenueReport>("get_revenue_report", {
        fromDate,
        toDate,
      }),
    enabled: Boolean(fromDate) && Boolean(toDate),
  });
}

export function useLabTurnaroundReport(fromDate: string, toDate: string) {
  return useQuery({
    queryKey: ["reports", "lab-turnaround", fromDate, toDate],
    queryFn: () =>
      invoke<LabTurnaroundReport>("get_lab_turnaround_report", {
        fromDate,
        toDate,
      }),
    enabled: Boolean(fromDate) && Boolean(toDate),
  });
}

/**
 * Generic CSV export mutation. `reportType` selects the report;
 * `params` is the JSON-serialisable parameter object whose shape
 * depends on the report (see backend `export_report_csv` doc).
 *
 * On success, returns the CSV string (with a leading UTF-8 BOM so Excel
 * detects UTF-8). The caller is responsible for triggering the download.
 *
 * On error, shows a toast and re-throws so the caller can also handle it
 * (e.g. disable the button on failure).
 */
export function useExportReportCsv() {
  return useMutation({
    mutationFn: (args: { reportType: string; params: Record<string, unknown> }) =>
      invoke<string>("export_report_csv", {
        reportType: args.reportType,
        params: JSON.stringify(args.params),
      }),
    onError: (err) => toast.error(String(err)),
  });
}

// ── Backup & Restore (Phase 2, SRS §9 A-07) ──────────────────────────────
//
// All four hooks wrap a server-build-only Tauri command. On client/dev builds
// the command does not exist (it is #[cfg(feature = "server-build")] in
// backup.rs and the generate_handler! registration is gated identically), so
// `invoke` rejects. `useQuery` surfaces the rejection as `error` (graceful —
// the page shows the error state instead of crashing); `useMutation`'s
// `onError` handler surfaces it as a toast. The Backup.tsx page (and the
// Settings → Backup section) further render an inline notice if `useBackups`
// errors, so the operator understands why the page is empty on a client
// build.
//
// All four commands are RBAC-guarded server-side by `Permission::BackupsManage`;
// the route itself is wrapped in `<RequirePermission perm={BackupsManage}>`
// (see App.tsx) so users without the permission never reach this page. Every
// mutation invalidates the ["backups"] query key so the table refreshes
// immediately after create/delete/restore.
//
// `useListBackups` is the canonical hook name per the SRS §9 A-07 spec; it
// is kept as a re-export of `useBackups` for backward compatibility with the
// pre-existing Backup.tsx page (which imports `useBackups`). Both names share
// the same `["backups"]` query key, so they hit the same cache.

export function useBackups() {
  return useQuery({
    queryKey: ["backups"],
    queryFn: () => invoke<BackupInfo[]>("list_backups"),
  });
}

/** Canonical list-backups hook per the SRS §9 A-07 spec. Alias of
 *  `useBackups` (kept for backward compat with the existing Backup.tsx page
 *  that imports `useBackups` directly). */
export function useListBackups() {
  return useBackups();
}

export function useCreateBackup() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => invoke<BackupInfo>("create_backup"),
    onSuccess: (info) => {
      qc.invalidateQueries({ queryKey: ["backups"] });
      // Per spec: "toast on success with file path". `info.path` is the
      // absolute filesystem path to the .sql backup file (e.g.
      // `C:\ProgramData\HMS\backups\hospital_db_20250115_120000.sql`).
      toast.success(`Backup created: ${info.path}`);
    },
    onError: (err) => toast.error(String(err)),
  });
}

export function useRestoreBackup() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (backupFilename: string) =>
      invoke<void>("restore_backup", { backupFilename }),
    onSuccess: (_data, backupFilename) => {
      qc.invalidateQueries({ queryKey: ["backups"] });
      toast.success(
        `Restored from ${backupFilename}. Restart the application now.`,
      );
    },
    onError: (err) => toast.error(String(err)),
  });
}

export function useDeleteBackup() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (backupFilename: string) =>
      invoke<void>("delete_backup", { backupFilename }),
    onSuccess: (_data, backupFilename) => {
      qc.invalidateQueries({ queryKey: ["backups"] });
      toast.success(`Deleted ${backupFilename}.`);
    },
    onError: (err) => toast.error(String(err)),
  });
}

// ── Pharmacy (Phase 2-C, SRS FR-0120–FR-0124) ──────────────────────────────
//
// Wires the 8 pharmacy commands added in `src-tauri/src/commands/pharmacy.rs`:
//   get_medications(search?)
//   create_medication(medication: CreateMedication)
//   update_medication(id, medication: UpdateMedication)
//   delete_medication(id)
//   get_prescriptions(patient_id?, status?)
//   get_prescription(id)                     → PrescriptionWithItems
//   create_prescription(prescription: CreatePrescription)
//   dispense_prescription_item(prescription_item_id)
//
// Catalog mutations invalidate `["pharmacy", "medications"]` so the table
// refreshes after add/edit/delete. Prescription mutations invalidate
// `["pharmacy", "prescriptions"]` (list) and any open detail query
// (`["pharmacy", "prescription", id]`). Dispensing ALSO invalidates
// `["inventory"]` because a successful dispense decrements the matched
// inventory_items row + writes an inventory_movements row — see the
// backend's `dispense_prescription_item` doc.
//
// All read hooks pass `null` (not undefined) for absent filters so the
// query key is stable across renders.

export function useMedications(search?: string | null) {
  return useQuery({
    queryKey: ["pharmacy", "medications", search ?? null],
    queryFn: () =>
      invoke<Medication[]>("get_medications", {
        search: search && search.length > 0 ? search : null,
      }),
  });
}

export function useCreateMedication() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (medication: CreateMedication) =>
      invoke<number>("create_medication", { medication }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["pharmacy", "medications"] });
      toast.success("Medication added to catalog.");
    },
    onError: (err) => toast.error(`Failed to add medication: ${err}`),
  });
}

export function useUpdateMedication() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, medication }: { id: number; medication: UpdateMedication }) =>
      invoke<void>("update_medication", { id, medication }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["pharmacy", "medications"] });
      toast.success("Medication updated.");
    },
    onError: (err) => toast.error(`Failed to update medication: ${err}`),
  });
}

export function useDeleteMedication() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => invoke<void>("delete_medication", { id }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["pharmacy", "medications"] });
      toast.success("Medication deactivated.");
    },
    onError: (err) => toast.error(`Failed to deactivate medication: ${err}`),
  });
}

export function usePrescriptions(patientId?: number | null, status?: string | null) {
  return useQuery({
    queryKey: ["pharmacy", "prescriptions", patientId ?? null, status ?? null],
    queryFn: () =>
      invoke<Prescription[]>("get_prescriptions", {
        patientId: patientId ?? null,
        status: status && status.length > 0 ? status : null,
      }),
  });
}

export function usePrescription(id: number | null) {
  return useQuery({
    queryKey: ["pharmacy", "prescription", id ?? -1],
    queryFn: () =>
      invoke<PrescriptionWithItems>("get_prescription", { id }),
    enabled: id != null,
  });
}

export function useCreatePrescription() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (prescription: CreatePrescription) =>
      invoke<number>("create_prescription", { prescription }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["pharmacy", "prescriptions"] });
      toast.success("Prescription created.");
    },
    onError: (err) => toast.error(`Failed to create prescription: ${err}`),
  });
}

/**
 * Dispense a single prescription item. On success, invalidates BOTH the
 * `["pharmacy"]` queries (prescription list + open detail) AND the
 * `["inventory"]` queries — because the backend decrements the matched
 * `inventory_items` row and writes an `inventory_movements` row inside
 * the same transaction. Failing to invalidate inventory would leave the
 * Inventory page showing a stale stock count until manual refetch.
 */
export function useDispensePrescriptionItem() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (prescriptionItemId: number) =>
      invoke<void>("dispense_prescription_item", {
        prescriptionItemId,
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["pharmacy"] });
      qc.invalidateQueries({ queryKey: ["inventory"] });
      toast.success("Item dispensed. Inventory updated.");
    },
    onError: (err) => toast.error(`Failed to dispense: ${err}`),
  });
}

// ── Radiology (Phase 2-D, SRS FR-0140–FR-0142) ────────────────────────────
//
// One hook per backend command in `src-tauri/src/commands/radiology.rs`:
//   get_radiology_orders, get_radiology_order, create_radiology_order,
//   update_radiology_order_status, delete_radiology_order,
//   get_radiology_report, create_radiology_report,
//   verify_radiology_report, get_radiology_dashboard.
//
// All mutations invalidate the broad `["radiology"]` prefix so the worklist,
// the open report dialog, and the dashboard KPIs refresh together — every
// write (create order / flip status / delete / file report / verify) changes
// at least one of those three views.
//
// Read hooks pass `null` (not undefined) for absent filters so the query
// key is stable across renders (same convention as appointments / lab /
// pharmacy).

export function useRadiologyOrders(
  statusFilter?: string | null,
  priorityFilter?: string | null,
  page?: number,
  pageSize?: number,
) {
  return useQuery({
    queryKey: ["radiology", "orders", statusFilter ?? null, priorityFilter ?? null, page ?? 1, pageSize ?? 10],
    queryFn: () =>
      invoke<RadiologyOrdersResponse>("get_radiology_orders", {
        statusFilter: statusFilter && statusFilter.length > 0 ? statusFilter : null,
        priorityFilter: priorityFilter && priorityFilter.length > 0 ? priorityFilter : null,
        page: page ?? 1,
        pageSize: pageSize ?? 10,
      }),
  });
}

export function useRadiologyOrder(id: number | null) {
  return useQuery({
    queryKey: ["radiology", "order", id ?? -1],
    queryFn: () => invoke<RadiologyOrder>("get_radiology_order", { id }),
    enabled: id != null,
  });
}

export function useCreateRadiologyOrder() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (order: CreateRadiologyOrder) =>
      invoke<number>("create_radiology_order", { order }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["radiology"] });
      toast.success("Radiology order created.");
    },
    onError: (err) => toast.error(`Failed to create radiology order: ${err}`),
  });
}

export function useUpdateRadiologyOrderStatus() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, status, notes }: { id: number; status: string; notes?: string | null }) =>
      invoke<void>("update_radiology_order_status", { id, status, notes: notes ?? null }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["radiology"] });
      toast.success("Order status updated.");
    },
    onError: (err) => toast.error(`Failed to update status: ${err}`),
  });
}

export function useDeleteRadiologyOrder() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (args: { id: number; reason?: string | null }) =>
      invoke<void>("delete_radiology_order", { id: args.id, reason: args.reason ?? null }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["radiology"] });
      toast.success("Radiology order deleted.");
    },
    onError: (err) => toast.error(`Failed to delete order: ${err}`),
  });
}

export function useRadiologyReport(orderId: number | null) {
  return useQuery({
    queryKey: ["radiology", "report", orderId ?? -1],
    queryFn: () => invoke<RadiologyReport | null>("get_radiology_report", { orderId }),
    enabled: orderId != null,
  });
}

export function useCreateRadiologyReport() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (report: CreateRadiologyReport) =>
      invoke<number>("create_radiology_report", { report }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["radiology"] });
      toast.success("Report filed.");
    },
    onError: (err) => toast.error(`Failed to file report: ${err}`),
  });
}

export function useVerifyRadiologyReport() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (reportId: number) =>
      invoke<void>("verify_radiology_report", { reportId }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["radiology"] });
      toast.success("Report verified.");
    },
    onError: (err) => toast.error(`Failed to verify report: ${err}`),
  });
}

export function useRadiologyDashboard() {
  return useQuery({
    queryKey: ["radiology", "dashboard"],
    queryFn: () => invoke<RadiologyDashboard>("get_radiology_dashboard"),
  });
}

// ── Blood Bank (Phase 2-E, SRS FR-0145–FR-0149) ────────────────────────────
//
// One hook per backend command in `src-tauri/src/commands/blood_bank.rs`:
//   get_blood_donors, get_blood_donor, create_blood_donor, delete_blood_donor,
//   get_blood_donations, create_blood_donation, update_blood_donation_screening,
//   get_blood_units, get_blood_unit, create_blood_unit, update_blood_unit_status,
//   delete_blood_unit, search_blood_inventory,
//   get_blood_crossmatches, check_blood_compatibility, create_blood_crossmatch,
//   verify_blood_crossmatch, create_blood_reservation, cancel_blood_reservation,
//   get_blood_issues, issue_blood, return_blood_unit,
//   get_blood_transfusions, create_blood_transfusion,
//   discard_blood_unit, get_blood_discards,
//   get_blood_unit_history, get_blood_unit_movements, get_blood_unit_traceability,
//   get_blood_bank_dashboard, get_blood_bank_statistics.
//
// All mutations invalidate the broad `["bloodbank"]` prefix so the inventory
// list, dashboard, and any open dialogs all refresh together.

export function useBloodDonors(
  search?: string,
  bloodGroup?: string,
  status?: string,
  page = 1,
  pageSize = 10,
) {
  return useQuery({
    queryKey: ["bloodbank", "donors", search ?? null, bloodGroup ?? null, status ?? null, page, pageSize],
    queryFn: () =>
      invoke<BloodDonorsResponse>("get_blood_donors", {
        search: search && search.trim() ? search.trim() : null,
        bloodGroupFilter: bloodGroup || null,
        statusFilter: status || null,
        page,
        pageSize,
      }),
  });
}

export function useBloodDonor(id: number | null) {
  return useQuery({
    queryKey: ["bloodbank", "donor", id ?? -1],
    queryFn: () => invoke<BloodDonor>("get_blood_donor", { id }),
    enabled: id !== null,
  });
}

export function useCreateBloodDonor() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (donor: CreateBloodDonor) =>
      invoke<number>("create_blood_donor", { donor }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["bloodbank"] });
      toast.success("Blood donor registered.");
    },
    onError: (err) => toast.error(`Failed to register donor: ${err}`),
  });
}

export function useDeleteBloodDonor() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (args: { id: number; reason?: string }) =>
      invoke<void>("delete_blood_donor", { id: args.id, reason: args.reason ?? null }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["bloodbank"] });
      toast.success("Donor deleted.");
    },
    onError: (err) => toast.error(`Failed to delete donor: ${err}`),
  });
}

export function useBloodDonations(
  donorId?: number,
  screeningStatus?: string,
  page = 1,
  pageSize = 10,
) {
  return useQuery({
    queryKey: ["bloodbank", "donations", donorId ?? null, screeningStatus ?? null, page, pageSize],
    queryFn: () =>
      invoke<BloodDonationsResponse>("get_blood_donations", {
        donorId: donorId ?? null,
        screeningStatusFilter: screeningStatus || null,
        page,
        pageSize,
      }),
  });
}

export function useCreateBloodDonation() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (donation: CreateBloodDonation) =>
      invoke<number>("create_blood_donation", { donation }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["bloodbank"] });
      toast.success("Donation recorded. Blood unit created in inventory.");
    },
    onError: (err) => toast.error(`Failed to record donation: ${err}`),
  });
}

export function useUpdateDonationScreening() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (args: { donationId: number; screeningStatus: string; notes?: string }) =>
      invoke<void>("update_blood_donation_screening", {
        donationId: args.donationId,
        screeningStatus: args.screeningStatus,
        notes: args.notes ?? null,
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["bloodbank"] });
      toast.success("Screening status updated.");
    },
    onError: (err) => toast.error(`Failed to update screening: ${err}`),
  });
}

export function useBloodUnits(
  status?: string,
  bloodGroup?: string,
  rhFactor?: string,
  componentType?: string,
  expiringDays?: number,
  page = 1,
  pageSize = 10,
) {
  return useQuery({
    queryKey: ["bloodbank", "units", status ?? null, bloodGroup ?? null, rhFactor ?? null, componentType ?? null, expiringDays ?? null, page, pageSize],
    queryFn: () =>
      invoke<BloodUnitsResponse>("get_blood_units", {
        statusFilter: status || null,
        bloodGroupFilter: bloodGroup || null,
        rhFilter: rhFactor || null,
        componentFilter: componentType || null,
        expiringDays: expiringDays ?? null,
        page,
        pageSize,
      }),
  });
}

export function useBloodUnit(id: number | null) {
  return useQuery({
    queryKey: ["bloodbank", "unit", id ?? -1],
    queryFn: () => invoke<BloodUnit>("get_blood_unit", { id }),
    enabled: id !== null,
  });
}

export function useCreateBloodUnit() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (unit: CreateBloodUnit) =>
      invoke<number>("create_blood_unit", { unit }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["bloodbank"] });
      toast.success("Blood unit added to inventory.");
    },
    onError: (err) => toast.error(`Failed to create unit: ${err}`),
  });
}

export function useUpdateBloodUnitStatus() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (args: { id: number; status: string; notes?: string }) =>
      invoke<void>("update_blood_unit_status", {
        id: args.id,
        status: args.status,
        notes: args.notes ?? null,
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["bloodbank"] });
      toast.success("Unit status updated.");
    },
    onError: (err) => toast.error(`Failed to update status: ${err}`),
  });
}

export function useDeleteBloodUnit() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (args: { id: number; reason?: string }) =>
      invoke<void>("delete_blood_unit", { id: args.id, reason: args.reason ?? null }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["bloodbank"] });
      toast.success("Unit deleted.");
    },
    onError: (err) => toast.error(`Failed to delete unit: ${err}`),
  });
}

export function useSearchBloodInventory(
  bloodGroup?: string,
  rhFactor?: string,
  componentType?: string,
) {
  return useQuery({
    queryKey: ["bloodbank", "search", bloodGroup ?? null, rhFactor ?? null, componentType ?? null],
    queryFn: () =>
      invoke<BloodUnit[]>("search_blood_inventory", {
        bloodGroup: bloodGroup || null,
        rhFactor: rhFactor || null,
        componentType: componentType || null,
      }),
  });
}

export function useBloodCrossmatches(
  patientId?: number,
  unitId?: number,
  resultFilter?: string,
  page = 1,
  pageSize = 10,
) {
  return useQuery({
    queryKey: ["bloodbank", "crossmatches", patientId ?? null, unitId ?? null, resultFilter ?? null, page, pageSize],
    queryFn: () =>
      invoke<BloodCrossmatchesResponse>("get_blood_crossmatches", {
        patientId: patientId ?? null,
        unitId: unitId ?? null,
        resultFilter: resultFilter || null,
        page,
        pageSize,
      }),
  });
}

export function useCheckBloodCompatibility() {
  return useMutation({
    mutationFn: (args: { unitId: number; patientId: number }) =>
      invoke<BloodCompatibilityResult>("check_blood_compatibility", {
        unitId: args.unitId,
        patientId: args.patientId,
      }),
    onError: (err) => toast.error(`Compatibility check failed: ${err}`),
  });
}

export function useCreateBloodCrossmatch() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (crossmatch: CreateBloodCrossmatch) =>
      invoke<number>("create_blood_crossmatch", { crossmatch }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["bloodbank"] });
      toast.success("Cross-match recorded.");
    },
    onError: (err) => toast.error(`Failed to record cross-match: ${err}`),
  });
}

export function useVerifyBloodCrossmatch() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (crossmatchId: number) =>
      invoke<void>("verify_blood_crossmatch", { crossmatchId }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["bloodbank"] });
      toast.success("Cross-match verified.");
    },
    onError: (err) => toast.error(`Failed to verify cross-match: ${err}`),
  });
}

export function useCreateBloodReservation() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (reservation: CreateBloodReservation) =>
      invoke<number>("create_blood_reservation", { reservation }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["bloodbank"] });
      toast.success("Unit reserved for patient.");
    },
    onError: (err) => toast.error(`Failed to create reservation: ${err}`),
  });
}

export function useCancelBloodReservation() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (args: { reservationId: number; reason?: string }) =>
      invoke<void>("cancel_blood_reservation", {
        reservationId: args.reservationId,
        reason: args.reason ?? null,
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["bloodbank"] });
      toast.success("Reservation cancelled. Unit released.");
    },
    onError: (err) => toast.error(`Failed to cancel reservation: ${err}`),
  });
}

export function useBloodIssues(patientId?: number, issueType?: string, page = 1, pageSize = 10) {
  return useQuery({
    queryKey: ["bloodbank", "issues", patientId ?? null, issueType ?? null, page, pageSize],
    queryFn: () =>
      invoke<BloodIssuesResponse>("get_blood_issues", {
        patientId: patientId ?? null,
        issueTypeFilter: issueType || null,
        page,
        pageSize,
      }),
  });
}

export function useIssueBlood() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (issue: CreateBloodIssue) =>
      invoke<number>("issue_blood", { issue }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["bloodbank"] });
      toast.success("Blood issued successfully.");
    },
    onError: (err) => toast.error(`Failed to issue blood: ${err}`),
  });
}

export function useReturnBloodUnit() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (args: { issueId: number; reason?: string }) =>
      invoke<void>("return_blood_unit", {
        issueId: args.issueId,
        reason: args.reason ?? null,
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["bloodbank"] });
      toast.success("Unit returned to inventory.");
    },
    onError: (err) => toast.error(`Failed to return unit: ${err}`),
  });
}

export function useBloodTransfusions(patientId?: number, page = 1, pageSize = 10) {
  return useQuery({
    queryKey: ["bloodbank", "transfusions", patientId ?? null, page, pageSize],
    queryFn: () =>
      invoke<BloodTransfusionsResponse>("get_blood_transfusions", {
        patientId: patientId ?? null,
        page,
        pageSize,
      }),
  });
}

export function useCreateBloodTransfusion() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (transfusion: CreateBloodTransfusion) =>
      invoke<number>("create_blood_transfusion", { transfusion }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["bloodbank"] });
      toast.success("Transfusion recorded.");
    },
    onError: (err) => toast.error(`Failed to record transfusion: ${err}`),
  });
}

export function useBloodDiscards(page = 1, pageSize = 10) {
  return useQuery({
    queryKey: ["bloodbank", "discards", page, pageSize],
    queryFn: () =>
      invoke<BloodDiscardsResponse>("get_blood_discards", { page, pageSize }),
  });
}

export function useDiscardBloodUnit() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (discard: CreateBloodDiscard) =>
      invoke<number>("discard_blood_unit", { discard }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["bloodbank"] });
      toast.success("Unit discarded.");
    },
    onError: (err) => toast.error(`Failed to discard unit: ${err}`),
  });
}

export function useBloodUnitHistory(unitId: number | null) {
  return useQuery({
    queryKey: ["bloodbank", "history", unitId ?? -1],
    queryFn: () => invoke<BloodUnitHistory[]>("get_blood_unit_history", { unitId }),
    enabled: unitId !== null,
  });
}

export function useBloodUnitMovements(unitId: number | null) {
  return useQuery({
    queryKey: ["bloodbank", "movements", unitId ?? -1],
    queryFn: () => invoke<BloodMovement[]>("get_blood_unit_movements", { unitId }),
    enabled: unitId !== null,
  });
}

export function useBloodUnitTraceability(unitId: number | null) {
  return useQuery({
    queryKey: ["bloodbank", "traceability", unitId ?? -1],
    queryFn: () =>
      invoke<BloodUnitTraceability>("get_blood_unit_traceability", { unitId }),
    enabled: unitId !== null,
  });
}

export function useBloodBankDashboard() {
  return useQuery({
    queryKey: ["bloodbank", "dashboard"],
    queryFn: () => invoke<BloodBankDashboard>("get_blood_bank_dashboard"),
  });
}

export function useBloodBankStatistics(months = 12) {
  return useQuery({
    queryKey: ["bloodbank", "statistics", months],
    queryFn: () => invoke<BloodBankStatistics>("get_blood_bank_statistics", { months }),
  });
}
