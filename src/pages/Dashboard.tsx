/**
 * Dashboard — role-prioritized work surface (RCTF UX-2026-09-13 U-14).
 *
 * Previously every role (doctor, nurse, receptionist, admin) saw the
 * same KPI-first grid. Per the role analysis:
 *  - Doctor  → work queue first: today's schedule + lab results awaiting
 *    their review (LabApprove), then KPIs. The schedule is hospital-wide
 *    because users are not linked to doctors rows in this system's data
 *    model (no user_id on doctors) — "my patients only" would require
 *    inventing a mapping the schema does not have.
 *  - Nurse   → ward first: admitted patients (ward/bed/diagnosis), then
 *    queue, then KPIs.
 *  - Receptionist / admin / others → KPI-first (operational overview),
 *    the previous layout, unchanged.
 *
 * Schedule and queue rows are patient-openable (shared profile dialog)
 * for holders of PatientsView — the doctor reads the chart without a
 * detour through the Patients module.
 *
 * The `usePatients()` full-directory fetch was replaced by the KPI
 * total that was ALREADY being fetched (`kpis.patients_total`) — the
 * guard "redirect to registration when empty" previously downloaded
 * every patient row just to call .length on it (U-10 perf).
 */
import {
  Calendar, UserPlus, Users, PlusCircle, CheckCircle,
  BedDouble, FlaskConical, ListOrdered, DollarSign,
  ArrowRight, TrendingUp, Clock, Activity, ShieldCheck, HeartPulse,
  AlertTriangle,
} from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { PieChart, Pie, Cell, ResponsiveContainer, Tooltip } from "recharts";
import {
  useAppointmentStats, useTodayAppointments, useDashboardKpis,
  useQueue, useLabOrders, useAdmissions, useFailedNotifications,
} from "@/lib/queries";
import { useAuth } from "@/lib/auth";
import { PERMISSIONS } from "@/lib/rbac";
import { formatMoney } from "@/lib/utils";
import { PageContainer, PageHeader, StatCard, SectionCard, EmptyState, StatusBadge, LoadingState } from "@/components/layout/shared";
import { PatientProfileDialog } from "@/components/clinical/PatientProfileDialog";

interface DashboardProps {
  onNavigate: (tab: string) => void;
  triggerAddPatient: () => void;
  triggerAddAppointment: () => void;
}

/** Time-of-day greeting — "Good morning" beats "Welcome back" for a
 *  12-hour clinical shift context. */
function greetingFor(now: Date): string {
  const h = now.getHours();
  if (h < 12) return "Good morning";
  if (h < 17) return "Good afternoon";
  return "Good evening";
}

export function Dashboard({ onNavigate, triggerAddPatient, triggerAddAppointment }: DashboardProps) {
  const { session, has } = useAuth();
  const canAppointments = has(PERMISSIONS.AppointmentsView);
  const { data: kpis } = useDashboardKpis();
  const { data: stats } = useAppointmentStats(canAppointments);

  const primaryRole = session?.roles?.[0];
  const firstName = (session?.user.full_name ?? "").split(" ")[0] || "there";
  const isDoctor = primaryRole === "doctor";
  const isNurse = primaryRole === "nurse";

  // RBAC fetch gates. The cards below are already hidden by these same
  // permissions, but hiding a card does not stop its query: an ungated
  // invoke hits the backend `require` guard and returns "Access denied…",
  // which the global QueryCache onError in main.tsx toasts to the user as
  // "Couldn't load data" — reading to staff like a system fault rather than
  // correct least-privilege behaviour. Each read is enabled only when the
  // caller holds the exact permission that read's command enforces, so a
  // forbidden module never errors in the first place.
  const canQueue = has(PERMISSIONS.QueueView);
  const canIpd = has(PERMISSIONS.IpdView);
  // The approvals card renders on LabApprove, but `get_lab_orders` requires
  // LabView — gate on both so the fetch can never be rejected. Every seeded
  // LabApprove holder also has LabView; the AND keeps that invariant true if
  // an admin edits role grants in the UI.
  const canLab = has(PERMISSIONS.LabApprove) && has(PERMISSIONS.LabView);

  // Null-safe: a `= []` destructure default only covers `undefined`; a
  // successful fetch that returns JSON null leaves data === null and
  // crashed the page on .length/.filter (caught live by the e2e debug
  // run — a backend null on any of these commands white-screened the
  // dashboard behind the ErrorBoundary). A disabled (permission-gated)
  // query also yields data === undefined — `?? []` guards both cases.
  const todaySchedule = useTodayAppointments(canAppointments).data ?? [];
  const queue = useQueue(null, canQueue).data ?? [];
  const { data: labOrdersData, isLoading: labOrdersLoading } = useLabOrders(null, canLab);
  const labOrders = labOrdersData ?? [];
  const { data: admittedData, isLoading: admittedLoading } = useAdmissions(
    "admitted",
    canIpd && isNurse,
  );
  const admitted = admittedData ?? [];
  // PK-2026-09-14 gap-6: recent failed WhatsApp sends, for front-desk
  // follow-up (whatsapp_notifications.success was already persisted,
  // just never read back anywhere before this).
  const failedNotifications = useFailedNotifications(canAppointments).data ?? [];

  const canOpenPatient = has(PERMISSIONS.PatientsView);
  const [profileId, setProfileId] = useState<number | null>(null);
  const openPatient = (id: number) => {
    if (canOpenPatient) setProfileId(id);
  };

  // Doctor's lab-approval work queue (LabApprove holders only — matches
  // the backend's approve gate; lab_technician deliberately lacks it).
  // Tied to `canLab` so the card renders exactly when its data can load.
  const showLabApprovals = canLab;
  const awaitingApproval = showLabApprovals
    ? labOrders.filter((o) => o.status === "resulted").slice(0, 6)
    : [];

  // Nurse's ward census.
  const showWard = canIpd;

  // Guard: if no patients exist, "New appointment" should redirect to patient
  // registration instead of showing an error toast on the Appointments page.
  // U-10: uses the KPI total already on this page — no full-directory fetch.
  const handleNewAppointment = () => {
    if ((kpis?.patients_total ?? 0) === 0) {
      triggerAddPatient();
    } else {
      triggerAddAppointment();
    }
  };

  const cancelledTotal = (stats?.cancelled ?? 0) + (stats?.no_show ?? 0);
  const chartData = stats
    ? [
        { name: "Scheduled", value: stats.scheduled, color: "hsl(var(--status-scheduled))" },
        { name: "Confirmed", value: stats.confirmed, color: "hsl(var(--status-confirmed))" },
        // PK-2026-09-14 gap-1: distinct "arrived" (physically checked in)
        // segment, separate from "confirmed" (phone/WhatsApp confirmed).
        { name: "Arrived", value: stats.arrived, color: "hsl(var(--status-arrived))" },
        { name: "Completed", value: stats.completed, color: "hsl(var(--status-completed))" },
        { name: "Cancelled", value: cancelledTotal, color: "hsl(var(--status-cancelled))" },
      ].filter((d) => d.value > 0)
    : [];

  const fmtTime = (t: string) => t.slice(0, 5);

  const description = isDoctor
    ? "Your clinical worklist for today — schedule, queue, and results awaiting your review."
    : isNurse
      ? "Your ward and the queue — the patients who need you next."
      : "Here's what's happening at your hospital today.";

  const kpiGrid = (
    <div className="grid grid-cols-2 lg:grid-cols-4 gap-5 items-stretch">
      <StatCard icon={Users} label="Total patients" value={kpis?.patients_total ?? "—"} color="primary" onClick={() => onNavigate("patients")} />
      <StatCard icon={Calendar} label="Appointments today" value={kpis?.appointments_today ?? "—"} sub={`${kpis?.appointments_scheduled ?? 0} pending`} color="info" onClick={() => onNavigate("appointments")} />
      <StatCard icon={CheckCircle} label="Completed today" value={kpis?.appointments_completed ?? "—"} color="success" />
      <StatCard icon={ListOrdered} label="In queue" value={kpis?.queue_waiting ?? "—"} sub={`${kpis?.queue_in_progress ?? 0} in progress`} color="warning" onClick={() => onNavigate("queue")} />
      {has(PERMISSIONS.IpdView) && (
        <StatCard icon={BedDouble} label="Beds available" value={kpis ? `${kpis.beds_available} / ${kpis.beds_total}` : "—"} sub={`${kpis?.ipd_admitted ?? 0} admitted`} color="accent" onClick={() => onNavigate("ipd")} />
      )}
      {has(PERMISSIONS.LabView) && (
        <StatCard icon={FlaskConical} label="Pending lab orders" value={kpis?.pending_lab_orders ?? "—"} color="destructive" onClick={() => onNavigate("laboratory")} />
      )}
      {has(PERMISSIONS.BillingView) && (
        <>
          <StatCard icon={DollarSign} label="Revenue today" value={kpis ? formatMoney(kpis.revenue_today) : "—"} color="success" onClick={() => onNavigate("billing")} />
          <StatCard icon={TrendingUp} label="Revenue this month" value={kpis ? formatMoney(kpis.revenue_month) : "—"} color="primary" />
        </>
      )}
    </div>
  );

  const scheduleCard = (
    <SectionCard
      className="lg:col-span-2"
      icon={Clock}
      title="Today's schedule"
      action={<Button variant="ghost" size="sm" className="text-xs gap-1" onClick={() => onNavigate("appointments")}>View all <ArrowRight className="h-3 w-3" /></Button>}
    >
      {todaySchedule.length === 0 ? (
        <EmptyState icon={Calendar} title="No appointments today" description="Schedule appointments to see them here." />
      ) : (
        <div className="max-h-[420px] overflow-y-auto">
          <Table>
            <TableHeader>
              <TableRow className="border-border hover:bg-transparent">
                <TableHead className="w-20">Time</TableHead>
                <TableHead>Patient</TableHead>
                <TableHead>Practitioner</TableHead>
                <TableHead className="text-right">Status</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {todaySchedule.slice(0, 12).map((a) => (
                <TableRow
                  key={a.id}
                  className={`border-border transition-colors ${canOpenPatient ? "cursor-pointer hover:bg-muted/50" : ""}`}
                  onClick={() => openPatient(a.patient_id)}
                  title={canOpenPatient ? `Open ${a.patient_first_name} ${a.patient_last_name}'s profile` : undefined}
                >
                  <TableCell className="font-mono text-xs font-semibold tabular-nums">{fmtTime(a.appointment_time)}</TableCell>
                  <TableCell className="font-medium">{a.patient_first_name} {a.patient_last_name}</TableCell>
                  <TableCell className="text-muted-foreground">{a.doctor_first_name} {a.doctor_last_name}</TableCell>
                  <TableCell className="text-right"><StatusBadge status={a.status} /></TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      )}
    </SectionCard>
  );

  const queueCard = has(PERMISSIONS.QueueView) && (
    <SectionCard
      icon={ListOrdered}
      title="Queue now"
      action={<Button variant="ghost" size="sm" className="text-xs" onClick={() => onNavigate("queue")}>Open</Button>}
    >
      {queue.length === 0 ? (
        <EmptyState icon={ListOrdered} title="Queue is empty" />
      ) : (
        <div className="p-4 space-y-2 max-h-[200px] overflow-y-auto">
          {queue.slice(0, 6).map((t) => (
            <div
              key={t.id}
              className={`flex items-center gap-3 px-4 py-2.5 rounded-[var(--radius)] transition-colors ${canOpenPatient ? "hover:bg-muted/50 cursor-pointer" : ""}`}
              onClick={() => openPatient(t.patient_id)}
            >
              <span className="font-mono text-xs font-bold text-primary w-8">#{t.token_number}</span>
              <span className="truncate text-sm flex-1">{t.patient_name}</span>
              <StatusBadge status={t.status} />
            </div>
          ))}
        </div>
      )}
    </SectionCard>
  );

  const appointmentMixCard = chartData.length > 0 && (
    <SectionCard icon={Activity} title="Appointment mix">
      <div className="p-6">
        <div className="h-40">
          <ResponsiveContainer width="100%" height="100%">
            <PieChart>
              <Pie data={chartData} dataKey="value" nameKey="name" innerRadius={45} outerRadius={70} paddingAngle={3} stroke="none">
                {chartData.map((d, i) => <Cell key={i} fill={d.color} />)}
              </Pie>
              <Tooltip contentStyle={{ borderRadius: "10px", border: "1px solid hsl(var(--border))", background: "hsl(var(--card))", fontSize: "12px", boxShadow: "var(--shadow-md)" }} />
            </PieChart>
          </ResponsiveContainer>
        </div>
        <div className="flex flex-wrap gap-x-5 gap-y-2.5 justify-center mt-4">
          {chartData.map((d) => (
            <div key={d.name} className="flex items-center gap-1.5 text-[11px] text-muted-foreground">
              <span className="h-2 w-2 rounded-full" style={{ background: d.color }} />
              <span className="font-medium">{d.name}</span>
              <span className="text-foreground font-semibold">{d.value}</span>
            </div>
          ))}
        </div>
      </div>
    </SectionCard>
  );

  // Doctor's results-awaiting-review queue. The row deep-links to the
  // Laboratory page where the approve workflow + critical-value protocol
  // live (approval itself is never one click from the dashboard — the
  // release flow requires the lab's acknowledgment steps).
  const labApprovalsCard = showLabApprovals && (
    <SectionCard
      icon={ShieldCheck}
      title="Lab results awaiting your review"
      description="Resulted orders need approval to release"
      action={<Button variant="ghost" size="sm" className="text-xs" onClick={() => onNavigate("laboratory")}>Open lab <ArrowRight className="h-3 w-3" /></Button>}
    >
      {labOrdersLoading ? (
        <LoadingState rows={2} />
      ) : awaitingApproval.length === 0 ? (
        <EmptyState icon={ShieldCheck} title="Nothing to review" description="Resulted lab orders appear here for approval." />
      ) : (
        <div className="p-4 space-y-2">
          {awaitingApproval.map((o) => (
            <div
              key={o.id}
              className="flex items-center gap-3 px-3 py-2 rounded-[var(--radius)] hover:bg-muted/50 transition-colors cursor-pointer"
              onClick={() => onNavigate("laboratory")}
            >
              <span className="font-mono text-xs font-bold text-primary">#{o.id}</span>
              <span className="truncate text-sm flex-1">{o.patient_name ?? "—"}</span>
              <span className="text-xs text-muted-foreground">
                {new Date(o.ordered_at).toLocaleDateString()}
              </span>
            </div>
          ))}
        </div>
      )}
    </SectionCard>
  );

  // Nurse's ward census — the admitted patients the nurse is responsible
  // for, with bed locations (the Nursing Station page keeps the chart).
  const wardCard = showWard && isNurse && (
    <SectionCard
      icon={HeartPulse}
      title="Your ward"
      description={`${admitted.length} currently admitted`}
      action={<Button variant="ghost" size="sm" className="text-xs" onClick={() => onNavigate("nursing")}>Nursing station <ArrowRight className="h-3 w-3" /></Button>}
    >
      {admittedLoading ? (
        <LoadingState rows={3} />
      ) : admitted.length === 0 ? (
        <EmptyState icon={HeartPulse} title="No admitted patients" description="Admissions from the In-Patient page will appear here." />
      ) : (
        <div className="p-4 space-y-2 max-h-[240px] overflow-y-auto">
          {admitted.slice(0, 8).map((a) => (
            <div
              key={a.id}
              className={`flex items-center gap-3 px-3 py-2 rounded-[var(--radius)] transition-colors ${canOpenPatient ? "hover:bg-muted/50 cursor-pointer" : ""}`}
              onClick={() => openPatient(a.patient_id)}
              title={canOpenPatient ? `Open ${a.patient_name ?? "patient"}'s profile` : undefined}
            >
              <BedDouble className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
              <span className="truncate text-sm font-medium flex-1">{a.patient_name ?? "—"}</span>
              <span className="text-xs text-muted-foreground shrink-0">{a.ward_name} · {a.bed_number}</span>
            </div>
          ))}
        </div>
      )}
    </SectionCard>
  );

  // PK-2026-09-14 gap-6: surfaces whatsapp_notifications.success = FALSE
  // rows, which were previously only visible as a server stderr log line.
  // Only rendered when there's something to show, same as appointmentMixCard.
  const failedRemindersCard = has(PERMISSIONS.AppointmentsView) &&
    failedNotifications.length > 0 && (
      <SectionCard icon={AlertTriangle} title="Failed reminders">
        <div className="p-4 space-y-2 max-h-[200px] overflow-y-auto">
          {failedNotifications.slice(0, 6).map((n) => (
            <div key={n.id} className="flex items-center gap-3 px-4 py-2.5 rounded-[var(--radius)] hover:bg-muted/50 transition-colors">
              <span className="truncate text-sm flex-1">
                {n.patient_name || n.recipient}
                <span className="block text-[10px] text-muted-foreground">
                  {n.notification_type} · {new Date(n.sent_at).toLocaleString(undefined, { dateStyle: "short", timeStyle: "short" })}
                </span>
              </span>
              <span className="text-[10px] font-semibold uppercase tracking-wide text-status-cancelled shrink-0">
                Failed
              </span>
            </div>
          ))}
        </div>
      </SectionCard>
    );

  return (
    <PageContainer>
      <PageHeader
        title={`${greetingFor(new Date())}, ${firstName}`}
        description={description}
        actions={
          <>
            {has(PERMISSIONS.PatientsCreate) && (
              <Button onClick={triggerAddPatient}>
                <UserPlus className="h-4 w-4" /> New patient
              </Button>
            )}
            {has(PERMISSIONS.AppointmentsCreate) && (
              <Button onClick={handleNewAppointment} variant="outline">
                <PlusCircle className="h-4 w-4" /> New appointment
              </Button>
            )}
          </>
        }
      />

      {isDoctor || isNurse ? (
        // Clinical roles: work queue first, KPIs after (prompt §7-8 — the
        // dashboard behaves as a work queue, not a decorative KPI wall).
        <>
          <div className="grid grid-cols-1 lg:grid-cols-3 gap-7">
            {scheduleCard}
            <div className="space-y-7">
              {isDoctor && labApprovalsCard}
              {isNurse && wardCard}
              {queueCard}
              {failedRemindersCard}
            </div>
          </div>
          {kpiGrid}
        </>
      ) : (
        // Operational roles: KPIs first, then schedule + mix/queue.
        <>
          {kpiGrid}
          <div className="grid grid-cols-1 lg:grid-cols-3 gap-7 mt-7">
            {scheduleCard}
            <div className="space-y-7">
              {appointmentMixCard}
              {queueCard}
              {failedRemindersCard}
            </div>
          </div>
        </>
      )}

      {profileId !== null && (
        <PatientProfileDialog
          patientId={profileId}
          open={profileId !== null}
          onOpenChange={(o) => !o && setProfileId(null)}
        />
      )}
    </PageContainer>
  );
}
