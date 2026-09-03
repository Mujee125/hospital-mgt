/**
 * Dashboard — role-aware KPI grid using shared layout components.
 */
import {
  Calendar, UserPlus, Users, PlusCircle, CheckCircle,
  BedDouble, FlaskConical, ListOrdered, DollarSign,
  ArrowRight, TrendingUp, Clock, Activity,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { PieChart, Pie, Cell, ResponsiveContainer, Tooltip } from "recharts";
import { useAppointmentStats, useTodayAppointments, useDashboardKpis, useQueue, usePatients } from "@/lib/queries";
import { useAuth } from "@/lib/auth";
import { PERMISSIONS } from "@/lib/rbac";
import { formatMoney } from "@/lib/utils";
import { PageContainer, PageHeader, StatCard, SectionCard, EmptyState, StatusBadge } from "@/components/layout/shared";

interface DashboardProps {
  onNavigate: (tab: string) => void;
  triggerAddPatient: () => void;
  triggerAddAppointment: () => void;
}

export function Dashboard({ onNavigate, triggerAddPatient, triggerAddAppointment }: DashboardProps) {
  const { has } = useAuth();
  const { data: kpis } = useDashboardKpis();
  const { data: stats } = useAppointmentStats();
  const { data: todaySchedule = [] } = useTodayAppointments();
  const { data: queue = [] } = useQueue();
  const { data: patients = [] } = usePatients();

  // Guard: if no patients exist, "New appointment" should redirect to patient
  // registration instead of showing an error toast on the Appointments page.
  const handleNewAppointment = () => {
    if (patients.length === 0) {
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
        { name: "Completed", value: stats.completed, color: "hsl(var(--status-completed))" },
        { name: "Cancelled", value: cancelledTotal, color: "hsl(var(--status-cancelled))" },
      ].filter((d) => d.value > 0)
    : [];

  const fmtTime = (t: string) => t.slice(0, 5);

  return (
    <PageContainer>
      <PageHeader
        title="Welcome back"
        description="Here's what's happening at your hospital today."
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

      {/* KPI grid — items-stretch + StatCard's internal h-full guarantees
          every card renders at identical height regardless of whether it
          has a `sub` line, per the equal-height KPI card standard. */}
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

      {/* Main grid */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-7 mt-7 mb-6">
        {/* Today's schedule */}
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
                    <TableRow key={a.id} className="cursor-pointer border-border hover:bg-muted/50 transition-colors" onClick={() => onNavigate("appointments")}>
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

        {/* Right column */}
        <div className="space-y-7">
          {chartData.length > 0 && (
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
          )}

          {has(PERMISSIONS.QueueView) && (
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
                    <div key={t.id} className="flex items-center gap-3 px-4 py-2.5 rounded-[var(--radius)] hover:bg-muted/50 transition-colors">
                      <span className="font-mono text-xs font-bold text-primary w-8">#{t.token_number}</span>
                      <span className="truncate text-sm flex-1">{t.patient_name}</span>
                      <StatusBadge status={t.status} />
                    </div>
                  ))}
                </div>
              )}
            </SectionCard>
          )}
        </div>
      </div>

      {/* Quick access */}
      {/* <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-4">
        <QuickAccess icon={Users} label="Patients" perm={PERMISSIONS.PatientsView} onClick={() => onNavigate("patients")} />
        <QuickAccess icon={Stethoscope} label="Doctors" perm={PERMISSIONS.DoctorsView} onClick={() => onNavigate("doctors")} />
        <QuickAccess icon={BedDouble} label="IPD" perm={PERMISSIONS.IpdView} onClick={() => onNavigate("ipd")} />
        <QuickAccess icon={FlaskConical} label="Lab" perm={PERMISSIONS.LabView} onClick={() => onNavigate("laboratory")} />
        <QuickAccess icon={DollarSign} label="Billing" perm={PERMISSIONS.BillingView} onClick={() => onNavigate("billing")} />
        <QuickAccess icon={ListOrdered} label="Queue" perm={PERMISSIONS.QueueView} onClick={() => onNavigate("queue")} />
      </div> */}
    </PageContainer>
  );
}

// function QuickAccess({ icon: Icon, label, perm, onClick }: { icon: React.ComponentType<{ className?: string }>; label: string; perm: import("@/lib/rbac").Permission; onClick: () => void }) {
//   const { has } = useAuth();
//   if (!has(perm)) return null;
//   return (
//     <button onClick={onClick} className="flex flex-col items-center gap-3 py-6 px-4 rounded-[var(--radius-md)] border border-border bg-card hover:border-primary/30 hover:shadow-md hover:-translate-y-0.5 transition-all duration-200 group">
//       <div className="h-11 w-11 rounded-[var(--radius-sm)] bg-primary/10 flex items-center justify-center group-hover:bg-primary/15 transition-colors">
//         <Icon className="h-5 w-5 text-primary" />
//       </div>
//       <span className="text-xs font-semibold text-foreground">{label}</span>
//     </button>
//   );
// }
