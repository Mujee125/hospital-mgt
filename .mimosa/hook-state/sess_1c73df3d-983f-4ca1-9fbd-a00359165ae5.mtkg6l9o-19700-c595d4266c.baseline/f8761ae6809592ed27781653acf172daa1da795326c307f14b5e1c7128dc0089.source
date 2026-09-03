/**
 * Consultation scheduling — modernized to the canonical VitalFlow
 * page pattern: PageContainer → PageHeader → SectionCard →
 * (LoadingState | EmptyState | PageToolbar + Table). Native
 * <select> filters are replaced with shadcn Select; the local
 * STATUS_TOKEN map / getStatusBadge are replaced with the shared
 * StatusBadge; quick-action buttons restyle to small shadcn
 * Buttons tinted by status tokens. All hooks, RBAC, deep-link
 * `?add=1` trigger, Tauri invoke() receipt flow, AppointmentForm
 * integration, and the receipt-no-print class are preserved
 * exactly.
 */
import { useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { motion } from "motion/react";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter, DialogClose } from "@/components/ui/dialog";
import { AppointmentForm } from "@/components/forms/AppointmentForm";
import { Receipt, type ReceiptData } from "@/components/Receipt";
import { Search, Calendar, Trash2, Edit, Receipt as ReceiptIcon, CalendarX, CalendarPlus } from "lucide-react";
import { toast } from "sonner";
import {
  useAppointments,
  usePatients,
  useDoctors,
  useDeleteAppointment,
  useUpdateAppointmentStatus,
} from "@/lib/queries";
import type { AppointmentWithDetails, Appointment } from "@/lib/models";
import {
  PageContainer,
  PageHeader,
  SectionCard,
  EmptyState,
  StatusBadge,
  LoadingState,
  PageToolbar,
} from "@/components/layout/shared";

// Editable subset of Appointment that the AppointmentForm expects
// (omits immutable audit fields created_at/updated_at). The form's own
// interface mirrors this shape — see AppointmentForm.tsx.
type EditableAppointment = Pick<
  Appointment,
  | "id"
  | "patient_id"
  | "doctor_id"
  | "appointment_date"
  | "appointment_time"
  | "duration_minutes"
  | "status"
  | "reason"
  | "notes"
>;

export function Appointments() {
  const [searchParams, setSearchParams] = useSearchParams();

  const { data: appointments = [], isLoading } = useAppointments();
  const { data: patients = [] } = usePatients();
  const { data: doctors = [] } = useDoctors();
  const deleteAppointment = useDeleteAppointment();
  const updateStatus = useUpdateAppointmentStatus();

  const [searchQuery, setSearchQuery] = useState("");
  const [filterDoctor, setFilterDoctor] = useState<number | "all">("all");
  const [filterStatus, setFilterStatus] = useState<string>("all");
  const [filterDate, setFilterDate] = useState<string>("");

  const [dialogOpen, setDialogOpen] = useState(false);
  const [selectedAppointment, setSelectedAppointment] = useState<EditableAppointment | undefined>(undefined);
  const [receiptOpen, setReceiptOpen] = useState(false);
  const [receiptData, setReceiptData] = useState<ReceiptData | null>(null);
  const [loadingReceipt, setLoadingReceipt] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<AppointmentWithDetails | null>(null);

  useEffect(() => {
    if (searchParams.get("add") === "1") {
      handleBookAppointment();
      searchParams.delete("add");
      setSearchParams(searchParams, { replace: true });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [searchParams, patients.length, doctors.length]);

  const handleBookAppointment = () => {
    if (patients.length === 0) {
      toast.error("Please register at least one patient first before scheduling appointments!");
      return;
    }
    if (doctors.filter((d) => d.is_active).length === 0) {
      if (doctors.length > 0) {
        toast.error("No active doctors found. Please mark a doctor as active in the Doctors page, then try again.");
      } else {
        toast.error("Please register at least one doctor first!");
      }
      return;
    }
    setSelectedAppointment(undefined);
    setDialogOpen(true);
  };

  const handleEditAppointment = (appt: AppointmentWithDetails) => {
    setSelectedAppointment({
      id: appt.id,
      patient_id: appt.patient_id,
      doctor_id: appt.doctor_id,
      appointment_date: appt.appointment_date,
      appointment_time: appt.appointment_time,
      duration_minutes: appt.duration_minutes,
      status: appt.status,
      reason: appt.reason,
      notes: appt.notes,
    });
    setDialogOpen(true);
  };

  const handleQuickStatusChange = (id: number, nextStatus: string) => {
    updateStatus.mutate({ id, status: nextStatus });
  };

  const handleDeleteAppointment = (appt: AppointmentWithDetails) => {
    setDeleteTarget(appt);
  };

  const confirmDeleteAppointment = () => {
    if (!deleteTarget) return;
    deleteAppointment.mutate(deleteTarget.id, {
      onSettled: () => setDeleteTarget(null),
    });
  };

  const handleFormSuccess = (newAppointmentId?: number) => {
    setDialogOpen(false);
    if (newAppointmentId) {
      showReceiptFor(newAppointmentId);
    }
  };

  const showReceiptFor = async (appointmentId: number) => {
    setLoadingReceipt(true);
    try {
      const [details, config] = await Promise.all([
        invoke<AppointmentWithDetails>("get_appointment", { id: appointmentId }),
        invoke<{ clinic_name: string } | null>("get_config"),
      ]);
      const patient = patients.find((p) => p.id === details.patient_id);

      setReceiptData({
        clinicName: config?.clinic_name || "VitalFlow Clinic",
        appointmentId: details.id,
        patientName: `${details.patient_first_name} ${details.patient_last_name}`,
        patientPhone: patient?.phone || "—",
        doctorName: `${details.doctor_first_name} ${details.doctor_last_name}`,
        doctorSpecialization: details.doctor_specialization,
        date: new Date(details.appointment_date).toLocaleDateString(undefined, {
          day: "2-digit",
          month: "short",
          year: "numeric",
        }),
        time: details.appointment_time.slice(0, 5),
        durationMinutes: details.duration_minutes,
        reason: details.reason,
        status: details.status,
        bookedAt: new Date(details.created_at).toLocaleString(undefined, {
          dateStyle: "short",
          timeStyle: "short",
        }),
      });
      setReceiptOpen(true);
    } catch (err: unknown) {
      toast.error(`Could not load receipt: ${String(err)}`);
      console.error(err);
    } finally {
      setLoadingReceipt(false);
    }
  };

  const filteredAppointments = appointments.filter((appt) => {
    const ptFullName = `${appt.patient_first_name} ${appt.patient_last_name}`.toLowerCase();
    const docFullName = `dr. ${appt.doctor_first_name} ${appt.doctor_last_name}`.toLowerCase();
    const query = searchQuery.toLowerCase();

    const matchesQuery =
      ptFullName.includes(query) || docFullName.includes(query) || (appt.reason && appt.reason.toLowerCase().includes(query));
    const matchesDoctor = filterDoctor === "all" || appt.doctor_id === Number(filterDoctor);
    const matchesStatus = filterStatus === "all" || appt.status.toLowerCase() === filterStatus.toLowerCase();
    const matchesDate = !filterDate || appt.appointment_date === filterDate;

    return matchesQuery && matchesDoctor && matchesStatus && matchesDate;
  });

  const formatTime = (timeStr: string) => timeStr.slice(0, 5);

  return (
    <PageContainer>
      <PageHeader
        icon={CalendarPlus}
        title="Consultation scheduling"
        description="Book consults, track queues, and update appointment status."
        actions={
          <Button onClick={handleBookAppointment} className="w-full sm:w-auto gap-2">
            <Calendar className="h-4 w-4" /> Book appointment
          </Button>
        }
      />

      <SectionCard>
        {isLoading ? (
          <LoadingState rows={6} />
        ) : filteredAppointments.length === 0 ? (
          <EmptyState
            icon={CalendarX}
            title={appointments.length === 0 ? "No appointments scheduled yet" : "No appointments match these filters"}
            description={
              appointments.length === 0
                ? "Book the first consultation to start managing today's clinic flow."
                : "Try adjusting the search, doctor, status, or date filters above."
            }
            action={
              appointments.length === 0 && (
                <Button onClick={handleBookAppointment} size="sm" className="gap-2">
                  <Calendar className="h-3.5 w-3.5" /> Book appointment
                </Button>
              )
            }
          />
        ) : (
          <>
            <PageToolbar>
              <div className="relative w-full max-w-xs">
                <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground pointer-events-none" />
                <Input
                  placeholder="Search patient or doctor…"
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  className="pl-9"
                />
              </div>

              <Select
                value={filterDoctor.toString()}
                onValueChange={(v) => setFilterDoctor(v === "all" ? "all" : Number(v))}
              >
                <SelectTrigger className="w-[180px]">
                  <SelectValue placeholder="All doctors" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">All doctors</SelectItem>
                  {doctors.map((d) => (
                    <SelectItem key={d.id} value={d.id.toString()}>
                      Dr. {d.first_name} {d.last_name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>

              <Select value={filterStatus} onValueChange={setFilterStatus}>
                <SelectTrigger className="w-[160px]">
                  <SelectValue placeholder="All statuses" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">All statuses</SelectItem>
                  <SelectItem value="scheduled">Scheduled</SelectItem>
                  <SelectItem value="confirmed">Confirmed</SelectItem>
                  <SelectItem value="completed">Completed</SelectItem>
                  <SelectItem value="cancelled">Cancelled</SelectItem>
                  <SelectItem value="no-show">No-show</SelectItem>
                </SelectContent>
              </Select>

              <Input
                type="date"
                value={filterDate}
                onChange={(e) => setFilterDate(e.target.value)}
                className="w-[170px]"
              />

              <span className="text-xs text-muted-foreground ml-auto tabular-nums">
                {filteredAppointments.length} of {appointments.length} appointments
              </span>
            </PageToolbar>

            <Table>
              <TableHeader>
                <TableRow className="hover:bg-transparent">
                  <TableHead scope="col">Date &amp; time</TableHead>
                  <TableHead scope="col">Patient</TableHead>
                  <TableHead scope="col">Doctor</TableHead>
                  <TableHead scope="col">Status</TableHead>
                  <TableHead scope="col">Reason</TableHead>
                  <TableHead scope="col">Quick actions</TableHead>
                  <TableHead scope="col" className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {filteredAppointments.map((appt, i) => (
                  <motion.tr
                    key={appt.id}
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    transition={{ duration: 0.15, delay: Math.min(i * 0.02, 0.3) }}
                    className="border-b border-border/70 transition-colors hover:bg-muted/40"
                  >
                    <TableCell>
                      <div className="font-semibold text-xs text-foreground font-mono">
                        {appt.appointment_date}
                      </div>
                      <div className="text-[10px] text-muted-foreground font-mono mt-0.5">
                        {formatTime(appt.appointment_time)} ({appt.duration_minutes}m)
                      </div>
                    </TableCell>
                    <TableCell className="font-semibold text-foreground">
                      {appt.patient_first_name} {appt.patient_last_name}
                    </TableCell>
                    <TableCell className="text-muted-foreground">
                      Dr. {appt.doctor_first_name} {appt.doctor_last_name}
                      <span className="block text-[10px] text-muted-foreground">
                        {appt.doctor_specialization}
                      </span>
                    </TableCell>
                    <TableCell>
                      <StatusBadge status={appt.status} />
                    </TableCell>
                    <TableCell
                      className="max-w-[150px] truncate text-xs text-muted-foreground"
                      title={appt.reason || ""}
                    >
                      {appt.reason || "—"}
                    </TableCell>
                    <TableCell>
                      <div className="flex flex-wrap gap-1">
                        {appt.status !== "confirmed" &&
                          appt.status !== "completed" &&
                          appt.status !== "cancelled" && (
                            <Button
                              variant="outline"
                              size="sm"
                              className="h-7 px-2.5 text-[11px] font-semibold text-status-confirmed border-status-confirmed/30 hover:bg-status-confirmed/10"
                              onClick={() => handleQuickStatusChange(appt.id, "confirmed")}
                            >
                              Confirm
                            </Button>
                          )}
                        {appt.status !== "completed" && appt.status !== "cancelled" && (
                          <Button
                            variant="outline"
                            size="sm"
                            className="h-7 px-2.5 text-[11px] font-semibold"
                            onClick={() => handleQuickStatusChange(appt.id, "completed")}
                          >
                            Complete
                          </Button>
                        )}
                        {appt.status !== "cancelled" && appt.status !== "completed" && (
                          <Button
                            variant="outline"
                            size="sm"
                            className="h-7 px-2.5 text-[11px] font-semibold text-status-cancelled border-status-cancelled/30 hover:bg-status-cancelled/10"
                            onClick={() => handleQuickStatusChange(appt.id, "cancelled")}
                          >
                            Cancel
                          </Button>
                        )}
                      </div>
                    </TableCell>
                    <TableCell className="text-right">
                      <div className="flex justify-end gap-1">
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => showReceiptFor(appt.id)}
                          disabled={loadingReceipt}
                          className="h-8 w-8 text-muted-foreground hover:text-foreground"
                          title="Print receipt"
                          aria-label={`Print receipt for ${appt.patient_first_name} ${appt.patient_last_name}`}
                        >
                          <ReceiptIcon className="h-4 w-4" />
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => handleEditAppointment(appt)}
                          className="h-8 w-8 text-muted-foreground hover:text-foreground"
                          title="Edit appointment"
                          aria-label={`Edit appointment for ${appt.patient_first_name} ${appt.patient_last_name}`}
                        >
                          <Edit className="h-4 w-4" />
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => handleDeleteAppointment(appt)}
                          disabled={deleteAppointment.isPending}
                          className="h-8 w-8 text-destructive hover:text-destructive hover:bg-destructive/10"
                          title="Delete appointment"
                          aria-label={`Delete appointment for ${appt.patient_first_name} ${appt.patient_last_name}`}
                        >
                          <Trash2 className="h-4 w-4" />
                        </Button>
                      </div>
                    </TableCell>
                  </motion.tr>
                ))}
              </TableBody>
            </Table>
          </>
        )}
      </SectionCard>

      <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle>
              {selectedAppointment ? "Edit appointment schedule" : "Book new appointment"}
            </DialogTitle>
            <DialogDescription>
              Select patient, doctor, date, time, and notes to schedule the consult.
            </DialogDescription>
          </DialogHeader>
          <div className="pt-2">
            <AppointmentForm
              appointment={selectedAppointment}
              patients={patients}
              doctors={doctors}
              onSuccess={handleFormSuccess}
              onCancel={() => setDialogOpen(false)}
            />
          </div>
        </DialogContent>
      </Dialog>

      <Dialog open={receiptOpen} onOpenChange={setReceiptOpen}>
        <DialogContent className="max-w-md">
          <DialogHeader className="receipt-no-print">
            <DialogTitle>Appointment receipt</DialogTitle>
            <DialogDescription>
              Print this for the patient, or close this window to skip printing.
            </DialogDescription>
          </DialogHeader>
          {receiptData ? <Receipt data={receiptData} /> : null}
        </DialogContent>
      </Dialog>

      {/* Delete confirmation dialog — replaces the previous window.confirm(). */}
      <Dialog open={deleteTarget !== null} onOpenChange={(o) => !o && setDeleteTarget(null)}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>Delete appointment?</DialogTitle>
            <DialogDescription>This action cannot be undone.</DialogDescription>
          </DialogHeader>
          {deleteTarget && (
            <p className="text-sm text-muted-foreground leading-relaxed">
              Cancel and permanently delete the{" "}
              <span className="font-semibold text-foreground">
                {deleteTarget.appointment_date} {formatTime(deleteTarget.appointment_time)}
              </span>{" "}
              appointment for{" "}
              <span className="font-semibold text-foreground">
                {deleteTarget.patient_first_name} {deleteTarget.patient_last_name}
              </span>{" "}
              with Dr. {deleteTarget.doctor_first_name} {deleteTarget.doctor_last_name}?
            </p>
          )}
          <DialogFooter>
            <DialogClose asChild>
              <Button variant="outline">Cancel</Button>
            </DialogClose>
            <Button
              variant="destructive"
              onClick={confirmDeleteAppointment}
              disabled={deleteAppointment.isPending}
            >
              {deleteAppointment.isPending ? "Deleting…" : "Delete appointment"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </PageContainer>
  );
}
