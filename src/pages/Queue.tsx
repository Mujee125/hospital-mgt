/**
 * Queue Management — uses shared layout components for consistency.
 */
import { useState } from "react";
import { ListOrdered, Plus, Play, Check, X, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter, DialogClose } from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useQueue, useCreateQueueToken, useCallNextToken, useSetTokenStatus, usePatientsEhr, useDoctors } from "@/lib/queries";
import { useAuth } from "@/lib/auth";
import { PERMISSIONS } from "@/lib/rbac";
import { PageContainer, PageHeader, SectionCard, EmptyState, StatusBadge, LoadingState, PageToolbar } from "@/components/layout/shared";

export function Queue() {
  const { has } = useAuth();
  const { data: tokens = [], isLoading } = useQueue();
  const { data: patients = [] } = usePatientsEhr();
  const { data: doctors = [] } = useDoctors();
  const createToken = useCreateQueueToken();
  const callNext = useCallNextToken();
  const setStatus = useSetTokenStatus();

  const [open, setOpen] = useState(false);
  const [patientId, setPatientId] = useState<number | null>(null);
  const [doctorId, setDoctorId] = useState<number | null>(null);
  const [priority, setPriority] = useState(0);

  const issue = async () => {
    if (patientId == null) return;
    await createToken.mutateAsync({ patient_id: patientId, doctor_id: doctorId, priority });
    setOpen(false);
    setPatientId(null);
    setDoctorId(null);
    setPriority(0);
  };

  return (
    <PageContainer>
      <PageHeader
        icon={ListOrdered}
        title="Patient Queue"
        description="Today's tokens · resets daily"
        actions={
          has(PERMISSIONS.QueueManage) && (
            <>
              <Button variant="outline" onClick={() => callNext.mutate({})} disabled={callNext.isPending}>
                {callNext.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : <Play className="h-4 w-4" />} Call next
              </Button>
              <Button onClick={() => setOpen(true)}>
                <Plus className="h-4 w-4" /> Issue token
              </Button>
            </>
          )
        }
      />

      <SectionCard>
        {isLoading ? (
          <LoadingState rows={6} />
        ) : tokens.length === 0 ? (
          <EmptyState icon={ListOrdered} title="Queue is empty" description="Issue a token to start managing patient flow." />
        ) : (
          <>
            <PageToolbar>
              <span className="text-sm font-medium text-muted-foreground">{tokens.length} patient{tokens.length !== 1 ? "s" : ""} in queue</span>
              <span className="text-xs text-muted-foreground ml-auto">
                {tokens.filter((t) => t.status === "waiting").length} waiting · {tokens.filter((t) => t.status === "in-progress").length} in progress
              </span>
            </PageToolbar>
            <Table>
              <TableHeader>
                <TableRow className="border-border hover:bg-transparent">
                  <TableHead scope="col" className="w-20">Token</TableHead>
                  <TableHead scope="col">Patient</TableHead>
                  <TableHead scope="col">Practitioner</TableHead>
                  <TableHead scope="col" className="w-24">Priority</TableHead>
                  <TableHead scope="col" className="text-right">Status</TableHead>
                  {has(PERMISSIONS.QueueManage) && <TableHead scope="col" className="text-right">Actions</TableHead>}
                </TableRow>
              </TableHeader>
              <TableBody>
                {tokens.map((t) => (
                  <TableRow key={t.id}>
                    <TableCell className="font-mono font-bold text-primary">#{t.token_number}</TableCell>
                    <TableCell className="font-medium">{t.patient_name ?? "—"}</TableCell>
                    <TableCell className="text-muted-foreground">{t.doctor_name ?? "Any"}</TableCell>
                    <TableCell>{t.priority > 0 ? <Badge className="bg-accent/15 text-accent">Priority</Badge> : "—"}</TableCell>
                    <TableCell className="text-right"><StatusBadge status={t.status} /></TableCell>
                    {has(PERMISSIONS.QueueManage) && (
                      <TableCell className="text-right">
                        <div className="flex justify-end gap-1">
                          {t.status === "waiting" && (
                            <Button size="icon" variant="ghost" aria-label={`Call token #${t.token_number}`} className="h-8 w-8 text-muted-foreground hover:text-foreground" onClick={() => callNext.mutate({ doctor_id: t.doctor_id })} title="Call">
                              <Play className="h-3.5 w-3.5" />
                            </Button>
                          )}
                          {t.status === "in-progress" && (
                            <Button size="icon" variant="ghost" aria-label={`Mark token #${t.token_number} completed`} className="h-8 w-8 text-muted-foreground hover:text-success" onClick={() => setStatus.mutate({ id: t.id, status: "completed" })} title="Complete">
                              <Check className="h-3.5 w-3.5" />
                            </Button>
                          )}
                          {t.status !== "completed" && (
                            <Button size="icon" variant="ghost" aria-label={`Skip token #${t.token_number}`} className="h-8 w-8 text-muted-foreground hover:text-destructive" onClick={() => setStatus.mutate({ id: t.id, status: "skipped" })} title="Skip">
                              <X className="h-3.5 w-3.5" />
                            </Button>
                          )}
                        </div>
                      </TableCell>
                    )}
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </>
        )}
      </SectionCard>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Issue queue token</DialogTitle>
            <DialogDescription>Issue a new queue token for a patient and optionally assign them to a specific practitioner.</DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-2">
            <div className="space-y-1.5">
              <Label>Patient</Label>
              <Select value={patientId?.toString() ?? ""} onValueChange={(v) => setPatientId(Number(v))}>
                <SelectTrigger><SelectValue placeholder="Select patient" /></SelectTrigger>
                <SelectContent>
                  {patients.map((p) => <SelectItem key={p.id} value={p.id.toString()}>{p.first_name} {p.last_name} · {p.phone}</SelectItem>)}
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-1.5">
              <Label>Assign practitioner (optional)</Label>
              <Select value={doctorId?.toString() ?? "any"} onValueChange={(v) => setDoctorId(v === "any" ? null : Number(v))}>
                <SelectTrigger><SelectValue /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="any">Any available</SelectItem>
                  {doctors.filter((d) => d.is_active).map((d) => <SelectItem key={d.id} value={d.id.toString()}>Dr. {d.first_name} {d.last_name} — {d.specialization}</SelectItem>)}
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-1.5">
              <Label>Priority</Label>
              <Select value={priority.toString()} onValueChange={(v) => setPriority(Number(v))}>
                <SelectTrigger><SelectValue /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="0">Standard</SelectItem>
                  <SelectItem value="1">Priority (elderly / emergency)</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
          <DialogFooter>
            <DialogClose asChild><Button variant="outline">Cancel</Button></DialogClose>
            <Button disabled={!patientId || createToken.isPending} onClick={issue}>
              {createToken.isPending ? "Issuing…" : "Issue token"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </PageContainer>
  );
}
