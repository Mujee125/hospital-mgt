/**
 * Practitioners directory — modernized to the canonical VitalFlow
 * page pattern: PageContainer → PageHeader → SectionCard →
 * (LoadingState | EmptyState | PageToolbar + Table). Availability
 * indicators now use the shared StatusBadge. All hooks, the
 * isCurrentlyAvailable() helper, and DoctorForm integration are
 * preserved exactly.
 */
import { useState } from "react";
import { motion } from "motion/react";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter, DialogClose } from "@/components/ui/dialog";
import { DoctorForm } from "@/components/forms/DoctorForm";
import { Search, UserPlus, Edit, Trash2, Stethoscope, KeyRound, Copy, Check, Loader2 } from "lucide-react";
import { useDoctors, useDeleteDoctor, useCreateLoginForDoctor } from "@/lib/queries";
import type { Doctor } from "@/lib/models";
import { useAuth } from "@/lib/auth";
import { PERMISSIONS } from "@/lib/rbac";
import { Label } from "@/components/ui/label";
import { toast } from "sonner";
import {
  PageContainer,
  PageHeader,
  SectionCard,
  EmptyState,
  LoadingState,
  PageToolbar,
  StatusBadge,
} from "@/components/layout/shared";

export function Doctors() {
  const [searchQuery, setSearchQuery] = useState("");
  const { data: doctors = [], isLoading } = useDoctors();
  const deleteDoctor = useDeleteDoctor();
  const createLogin = useCreateLoginForDoctor();
  // RBAC: the directory is readable with doctors.view, but creating,
  // editing and deleting practitioner profiles require doctors.manage
  // (super_admin only in the seed). Without these gates a receptionist
  // sees the buttons and is refused only at submit.
  const { has } = useAuth();
  const canManage = has(PERMISSIONS.DoctorsManage);
  // RCTF Step 11: "Create login" is a user-management action (distinct from
  // DoctorsManage), so it's gated on UsersManage — matching the backend's
  // authorization boundary on create_login_for_doctor. UX only; the backend
  // enforces UsersManage regardless of what this button shows.
  const canManageUsers = has(PERMISSIONS.UsersManage);

  const [dialogOpen, setDialogOpen] = useState(false);
  const [selectedDoctor, setSelectedDoctor] = useState<Doctor | undefined>(undefined);
  const [deleteTarget, setDeleteTarget] = useState<Doctor | null>(null);

  // ── RCTF Step 11: create-login flow ───────────────────────────────────
  const [loginTarget, setLoginTarget] = useState<Doctor | null>(null);
  const [loginUsername, setLoginUsername] = useState("");
  const [loginResult, setLoginResult] = useState<{ username: string; password: string } | null>(null);
  const [copied, setCopied] = useState(false);

  /** Suggest `firstname.lastname`, normalized to the backend's allowed
   *  charset (lowercase letters, digits, '.', '_', '-'). The backend
   *  remains the authoritative uniqueness/format check — this is a
   *  starting point the admin can edit, not a guarantee. */
  const suggestUsername = (doctor: Doctor) => {
    const normalize = (s: string) =>
      s
        .toLowerCase()
        .normalize("NFD")
        .replace(/[\u0300-\u036f]/g, "") // strip accents
        .replace(/[^a-z0-9]/g, "");
    const first = normalize(doctor.first_name);
    const last = normalize(doctor.last_name);
    return [first, last].filter(Boolean).join(".");
  };

  const openCreateLogin = (doctor: Doctor) => {
    setLoginTarget(doctor);
    setLoginUsername(suggestUsername(doctor));
    setLoginResult(null);
    setCopied(false);
  };

  const closeLoginDialog = () => {
    // Do not provide a later "show password again" operation — clearing
    // component state here is the only place the plaintext password lives.
    setLoginTarget(null);
    setLoginUsername("");
    setLoginResult(null);
    setCopied(false);
  };

  const submitCreateLogin = async () => {
    if (!loginTarget) return;
    try {
      const [username, password] = await createLogin.mutateAsync({
        doctorId: loginTarget.id,
        username: loginUsername.trim(),
      });
      setLoginResult({ username, password });
    } catch {
      // useCreateLoginForDoctor's onError already toasts the message.
    }
  };

  const copyPassword = async () => {
    if (!loginResult) return;
    try {
      await navigator.clipboard.writeText(loginResult.password);
      setCopied(true);
      toast.success("Temporary password copied.");
      setTimeout(() => setCopied(false), 2000);
    } catch {
      toast.error("Couldn't copy to clipboard — select and copy manually.");
    }
  };

  const handleAddDoctor = () => {
    setSelectedDoctor(undefined);
    setDialogOpen(true);
  };

  const handleEditDoctor = (doctor: Doctor) => {
    setSelectedDoctor(doctor);
    setDialogOpen(true);
  };

  const handleDeleteDoctor = (doctor: Doctor) => {
    setDeleteTarget(doctor);
  };

  const confirmDeleteDoctor = () => {
    if (!deleteTarget) return;
    deleteDoctor.mutate(deleteTarget.id, {
      onSettled: () => setDeleteTarget(null),
    });
  };

  const handleFormSuccess = () => setDialogOpen(false);

  const filteredDoctors = doctors.filter((d) => {
    const fullName = `Dr. ${d.first_name} ${d.last_name}`.toLowerCase();
    const query = searchQuery.toLowerCase();
    return (
      fullName.includes(query) ||
      d.specialization.toLowerCase().includes(query) ||
      d.qualification.toLowerCase().includes(query) ||
      d.phone.includes(query)
    );
  });

  const formatTime = (timeStr: string) => timeStr.slice(0, 5);

  /** Availability indicator: is `now` inside this doctor's consultation
   *  hours, given they're also marked active in the system. */
  const isCurrentlyAvailable = (doc: Doctor) => {
    if (!doc.is_active) return false;
    const now = new Date();
    const nowMinutes = now.getHours() * 60 + now.getMinutes();
    // Parse "HH:MM" safely — split/map may yield undefined under strict mode.
    const parseMinutes = (t: string): number => {
      const [h, m] = t.split(":").map(Number);
      return (h ?? 0) * 60 + (m ?? 0);
    };
    const fromMin = parseMinutes(doc.available_from);
    const toMin = parseMinutes(doc.available_to);
    return nowMinutes >= fromMin && nowMinutes <= toMin;
  };

  const isSearchActive = !!searchQuery.trim();

  return (
    <PageContainer>
      <PageHeader
        icon={Stethoscope}
        title="Practitioners directory"
        description="Medical staff profiles, specialties, and duty timings."
        actions={
          canManage && (
            <Button onClick={handleAddDoctor} className="gap-2">
              <UserPlus className="h-4 w-4" /> Add doctor
            </Button>
          )
        }
      />

      <SectionCard>
        {isLoading ? (
          <LoadingState rows={6} />
        ) : filteredDoctors.length === 0 ? (
          <EmptyState
            icon={Stethoscope}
            title={isSearchActive ? "No practitioners match your search" : "No practitioners registered yet"}
            description={
              isSearchActive
                ? "Try a different name, specialization, or qualification."
                : "Add your first doctor to begin scheduling appointments."
            }
            action={
              !isSearchActive && canManage && (
                <Button onClick={handleAddDoctor} size="sm" className="gap-2">
                  <UserPlus className="h-3.5 w-3.5" /> Add doctor
                </Button>
              )
            }
          />
        ) : (
          <>
            <PageToolbar>
              <div className="relative w-full max-w-md">
                <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground pointer-events-none" />
                <Input
                  placeholder="Search by name, specialization, or qualification…"
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  className="pl-9 h-10"
                />
              </div>
              <span className="text-xs text-muted-foreground ml-auto tabular-nums">
                {filteredDoctors.length} of {doctors.length} practitioners
              </span>
            </PageToolbar>

            <Table>
              <TableHeader>
                <TableRow className="hover:bg-transparent">
                  <TableHead scope="col">Practitioner</TableHead>
                  <TableHead scope="col">Specialization</TableHead>
                  <TableHead scope="col">Qualification</TableHead>
                  <TableHead scope="col">Phone</TableHead>
                  <TableHead scope="col">Consultation hours</TableHead>
                  <TableHead scope="col">Availability</TableHead>
                  <TableHead scope="col" className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {filteredDoctors.map((doc, i) => {
                  const available = isCurrentlyAvailable(doc);
                  return (
                    <motion.tr
                      key={doc.id}
                      initial={{ opacity: 0 }}
                      animate={{ opacity: 1 }}
                      transition={{ duration: 0.15, delay: Math.min(i * 0.02, 0.3) }}
                      className="border-b border-border/70 transition-colors hover:bg-muted/40"
                    >
                      <TableCell className="font-semibold text-foreground">
                        Dr. {doc.first_name} {doc.last_name}
                      </TableCell>
                      <TableCell>
                        <Badge
                          variant="outline"
                          className="bg-primary/5 text-primary border-primary/20 font-semibold text-[11px] capitalize rounded-full"
                        >
                          {doc.specialization}
                        </Badge>
                      </TableCell>
                      <TableCell className="text-xs text-muted-foreground font-medium">{doc.qualification}</TableCell>
                      <TableCell className="font-mono text-xs font-semibold">{doc.phone}</TableCell>
                      <TableCell className="font-mono text-xs text-muted-foreground">
                        {formatTime(doc.available_from)} – {formatTime(doc.available_to)}
                      </TableCell>
                      <TableCell>
                        {!doc.is_active ? (
                          <StatusBadge status="inactive" />
                        ) : available ? (
                          <StatusBadge status="available" />
                        ) : (
                          <StatusBadge status="off duty" />
                        )}
                      </TableCell>
                      <TableCell className="text-right">
                        <div className="flex justify-end gap-1">
                          {canManageUsers && doc.user_id == null && (
                            <Button
                              variant="ghost"
                              size="icon"
                              onClick={() => openCreateLogin(doc)}
                              className="h-8 w-8 text-muted-foreground hover:text-foreground"
                              title="Create login"
                              aria-label={`Create login for Dr. ${doc.first_name} ${doc.last_name}`}
                            >
                              <KeyRound className="h-4 w-4" />
                            </Button>
                          )}
                          {canManage && (
                            <Button
                              variant="ghost"
                              size="icon"
                              onClick={() => handleEditDoctor(doc)}
                              className="h-8 w-8 text-muted-foreground hover:text-foreground"
                              title="Edit doctor details"
                              aria-label={`Edit Dr. ${doc.first_name} ${doc.last_name}`}
                            >
                              <Edit className="h-4 w-4" />
                            </Button>
                          )}
                          {canManage && (
                            <Button
                              variant="ghost"
                              size="icon"
                              onClick={() => handleDeleteDoctor(doc)}
                              disabled={deleteDoctor.isPending}
                              className="h-8 w-8 text-destructive hover:text-destructive hover:bg-destructive/10"
                              title="Delete doctor profile"
                              aria-label={`Delete Dr. ${doc.first_name} ${doc.last_name}`}
                            >
                              <Trash2 className="h-4 w-4" />
                            </Button>
                          )}
                        </div>
                      </TableCell>
                    </motion.tr>
                  );
                })}
              </TableBody>
            </Table>
          </>
        )}
      </SectionCard>

      <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle>{selectedDoctor ? "Edit doctor profile" : "Register practitioner"}</DialogTitle>
            <DialogDescription>
              {selectedDoctor
                ? "Update doctor contact details or availability schedule."
                : "Add a new physician profile to the hospital roster."}
            </DialogDescription>
          </DialogHeader>
          <div className="pt-2">
            <DoctorForm doctor={selectedDoctor} onSuccess={handleFormSuccess} onCancel={() => setDialogOpen(false)} />
          </div>
        </DialogContent>
      </Dialog>

      {/* Delete confirmation dialog — replaces the previous window.confirm(). */}
      <Dialog open={deleteTarget !== null} onOpenChange={(o) => !o && setDeleteTarget(null)}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>Delete practitioner profile?</DialogTitle>
            <DialogDescription>This action cannot be undone.</DialogDescription>
          </DialogHeader>
          <p className="text-sm text-muted-foreground leading-relaxed">
            Are you sure you want to delete{" "}
            <span className="font-semibold text-foreground">
              Dr. {deleteTarget?.first_name} {deleteTarget?.last_name}
            </span>{" "}
            — including all of their scheduled appointments? This will remove
            them from the practitioners directory.
          </p>
          <DialogFooter>
            <DialogClose asChild>
              <Button variant="outline">Cancel</Button>
            </DialogClose>
            <Button
              variant="destructive"
              onClick={confirmDeleteDoctor}
              disabled={deleteDoctor.isPending}
            >
              {deleteDoctor.isPending ? "Deleting…" : "Delete doctor"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      {/* RCTF Step 11: create-login dialog. Two phases in one Dialog:
          (1) review/edit the suggested username and confirm, (2) on
          success, a one-time credential panel — replaces the confirm
          form rather than stacking a second dialog. */}
      <Dialog open={loginTarget !== null} onOpenChange={(o) => !o && closeLoginDialog()}>
        <DialogContent className="max-w-md">
          {!loginResult ? (
            <>
              <DialogHeader>
                <DialogTitle>Create login</DialogTitle>
                <DialogDescription>
                  Create a system login for Dr. {loginTarget?.first_name} {loginTarget?.last_name}.
                  A temporary password will be generated and shown once — they'll be required to
                  change it on first sign-in.
                </DialogDescription>
              </DialogHeader>
              <div className="space-y-1.5 py-2">
                <Label htmlFor="login-username">Username</Label>
                <Input
                  id="login-username"
                  value={loginUsername}
                  onChange={(e) => setLoginUsername(e.target.value)}
                  placeholder="firstname.lastname"
                  autoFocus
                />
                <p className="text-xs text-muted-foreground">
                  Lowercase letters, digits, '.', '_', and '-' only. You can edit the suggested
                  username before creating the account.
                </p>
              </div>
              <DialogFooter>
                <DialogClose asChild><Button variant="outline">Cancel</Button></DialogClose>
                <Button
                  onClick={submitCreateLogin}
                  disabled={createLogin.isPending || loginUsername.trim().length < 3}
                >
                  {createLogin.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : "Create login"}
                </Button>
              </DialogFooter>
            </>
          ) : (
            <>
              <DialogHeader>
                <DialogTitle>Login created</DialogTitle>
                <DialogDescription>
                  This is the only time the temporary password will be shown. Deliver it to
                  the practitioner securely — they must change it on first login.
                </DialogDescription>
              </DialogHeader>
              <div className="space-y-3 py-2">
                <div className="space-y-1.5">
                  <Label>Username</Label>
                  <p className="font-mono text-sm px-3 py-2 rounded-[var(--radius)] bg-muted/50 border border-border">
                    {loginResult.username}
                  </p>
                </div>
                <div className="space-y-1.5">
                  <Label>Temporary password</Label>
                  <div className="flex items-center gap-2">
                    <p className="flex-1 font-mono text-sm px-3 py-2 rounded-[var(--radius)] bg-muted/50 border border-border select-all">
                      {loginResult.password}
                    </p>
                    <Button variant="outline" size="icon" onClick={copyPassword} title="Copy password">
                      {copied ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
                    </Button>
                  </div>
                </div>
                <p className="text-xs text-muted-foreground">
                  It will not be shown again after you close this dialog.
                </p>
              </div>
              <DialogFooter>
                <Button onClick={closeLoginDialog}>Done</Button>
              </DialogFooter>
            </>
          )}
        </DialogContent>
      </Dialog>
    </PageContainer>
  );
}
