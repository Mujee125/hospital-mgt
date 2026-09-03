/**
 * Users & roles — uses shared layout components.
 */
import { useState } from "react";
import { UserCog, Plus, KeyRound, Loader2, ShieldOff } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter, DialogClose } from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { useUsers, useRoles, useCreateUser, useUpdateUser, useDeleteUser, useResetUserPassword } from "@/lib/queries";
import { useAuth } from "@/lib/auth";
import { PERMISSIONS, ROLE_LABELS } from "@/lib/rbac";
import { PageContainer, PageHeader, SectionCard, EmptyState, StatusBadge, LoadingState, PageToolbar } from "@/components/layout/shared";

export function Users() {
  const { has } = useAuth();
  const { data: users = [], isLoading } = useUsers();
  const { data: roles = [] } = useRoles();
  const create = useCreateUser();
  const update = useUpdateUser();
  const remove = useDeleteUser();
  const resetPwd = useResetUserPassword();

  const [open, setOpen] = useState(false);
  const [edit, setEdit] = useState<{ id: number; full_name: string; email: string; roles: string[]; is_active: boolean } | null>(null);
  const [form, setForm] = useState({ username: "", full_name: "", email: "", password: "", roles: [] as string[] });
  const [pwdResetTarget, setPwdResetTarget] = useState<{ id: number; username: string } | null>(null);
  const [pwdDraft, setPwdDraft] = useState("");
  const [deleteTarget, setDeleteTarget] = useState<{ id: number; username: string; full_name: string } | null>(null);

  const openCreate = () => { setEdit(null); setForm({ username: "", full_name: "", email: "", password: "", roles: [] }); setOpen(true); };
  const openEdit = (u: { id: number; full_name: string; email: string | null; is_active: boolean }) => {
    setEdit({ id: u.id, full_name: u.full_name, email: u.email ?? "", roles: [], is_active: u.is_active });
    setOpen(true);
  };

  const submit = async () => {
    if (edit) {
      await update.mutateAsync({ id: edit.id, full_name: form.full_name || undefined, email: form.email || undefined, roles: form.roles.length ? form.roles : undefined });
    } else {
      await create.mutateAsync({ username: form.username, full_name: form.full_name, email: form.email || null, password: form.password, roles: form.roles });
    }
    setOpen(false);
  };

  const toggleRole = (r: string) =>
    setForm((f) => ({ ...f, roles: f.roles.includes(r) ? f.roles.filter((x) => x !== r) : [...f.roles, r] }));

  return (
    <PageContainer>
      <PageHeader
        icon={UserCog}
        title="Users & Roles"
        description="Role-based access control"
        actions={has(PERMISSIONS.UsersManage) && (
          <Button onClick={openCreate}><Plus className="h-4 w-4" /> New user</Button>
        )}
      />

      <SectionCard>
        {isLoading ? (
          <LoadingState rows={5} />
        ) : users.length === 0 ? (
          <EmptyState icon={UserCog} title="No users" description="Create a user to get started." />
        ) : (
          <>
            <PageToolbar>
              <span className="text-sm font-medium text-muted-foreground">{users.length} total users</span>
              <span className="text-xs text-muted-foreground ml-auto">{users.filter((u) => u.is_active).length} active</span>
            </PageToolbar>
            <Table>
              <TableHeader>
                <TableRow className="border-border hover:bg-transparent">
                  <TableHead scope="col">Username</TableHead>
                  <TableHead scope="col">Name</TableHead>
                  <TableHead scope="col">Email</TableHead>
                  <TableHead scope="col">Status</TableHead>
                  <TableHead scope="col">Last login</TableHead>
                  {has(PERMISSIONS.UsersManage) && <TableHead scope="col" className="text-right">Actions</TableHead>}
                </TableRow>
              </TableHeader>
              <TableBody>
                {users.map((u) => (
                  <TableRow key={u.id}>
                    <TableCell className="font-mono font-medium">@{u.username}</TableCell>
                    <TableCell className="font-medium">{u.full_name}</TableCell>
                    <TableCell className="text-muted-foreground">{u.email ?? "—"}</TableCell>
                    <TableCell>
                      <div className="flex items-center gap-1.5">
                        {u.must_change_password && <Badge variant="warning">Must change</Badge>}
                        <StatusBadge status={u.is_active ? "active" : "inactive"} />
                      </div>
                    </TableCell>
                    <TableCell className="text-xs text-muted-foreground">{u.last_login_at ? new Date(u.last_login_at).toLocaleString() : "Never"}</TableCell>
                    {has(PERMISSIONS.UsersManage) && (
                      <TableCell className="text-right">
                        <div className="flex justify-end gap-1">
                          <Button
                            size="icon"
                            variant="ghost"
                            className="h-8 w-8 text-muted-foreground hover:text-foreground"
                            title="Reset password"
                            aria-label={`Reset password for @${u.username}`}
                            onClick={() => {
                              setPwdResetTarget({ id: u.id, username: u.username });
                              setPwdDraft("");
                            }}
                          >
                            <KeyRound className="h-3.5 w-3.5" />
                          </Button>
                          <Button size="sm" variant="ghost" className="h-8" onClick={() => openEdit(u)}>Edit</Button>
                          <Button
                            size="icon"
                            variant="ghost"
                            className="h-8 w-8 text-muted-foreground hover:text-destructive"
                            title="Delete user"
                            aria-label={`Delete @${u.username}`}
                            onClick={() => setDeleteTarget({ id: u.id, username: u.username, full_name: u.full_name })}
                          >
                            <ShieldOff className="h-3.5 w-3.5" />
                          </Button>
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

      {/* Roles legend */}
      <SectionCard icon={UserCog} title="Role catalogue">
        <div className="p-6">
          <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
            {roles.map(([id, name, desc]) => (
              <div key={id} className="border border-border rounded-[var(--radius-md)] p-3">
                <div className="text-sm font-semibold">{ROLE_LABELS[name] ?? name}</div>
                <div className="text-[11px] text-muted-foreground mt-1">{desc}</div>
              </div>
            ))}
          </div>
        </div>
      </SectionCard>

      {/* Create/Edit dialog */}
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>{edit ? "Edit user" : "New user"}</DialogTitle>
            <DialogDescription>
              {edit
                ? "Update the user's display name, email, or assigned roles."
                : "Create a new user account with a temporary password they'll change on first login."}
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-2">
            {!edit && (
              <>
                <div className="space-y-1.5"><Label>Username</Label><Input value={form.username} onChange={(e) => setForm({ ...form, username: e.target.value })} /></div>
                <div className="space-y-1.5"><Label>Temporary password</Label><Input type="password" value={form.password} onChange={(e) => setForm({ ...form, password: e.target.value })} placeholder="Min 8 characters" /></div>
              </>
            )}
            <div className="space-y-1.5"><Label>Full name</Label><Input value={form.full_name} onChange={(e) => setForm({ ...form, full_name: e.target.value })} /></div>
            <div className="space-y-1.5"><Label>Email</Label><Input type="email" value={form.email} onChange={(e) => setForm({ ...form, email: e.target.value })} /></div>
            <div className="space-y-1.5">
              <Label>Roles</Label>
              <div className="grid grid-cols-2 gap-2">
                {roles.map(([_, name]) => (
                  <label key={name} className="flex items-center gap-2 p-2 border border-border rounded-[var(--radius)] cursor-pointer hover:bg-muted/50 text-sm">
                    <input type="checkbox" checked={form.roles.includes(name)} onChange={() => toggleRole(name)} className="h-4 w-4 accent-primary" />
                    {ROLE_LABELS[name] ?? name}
                  </label>
                ))}
              </div>
            </div>
          </div>
          <DialogFooter>
            <DialogClose asChild><Button variant="outline">Cancel</Button></DialogClose>
            <Button disabled={create.isPending || update.isPending} onClick={submit}>
              {(create.isPending || update.isPending) ? <Loader2 className="h-4 w-4 animate-spin" /> : edit ? "Save" : "Create user"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Reset password dialog — replaces the previous window.prompt() that
          exposed the password as plain text in a tiny webview input and was
          blocked by some webview policies. Uses a real <input type="password">
          so the value is masked and the field is announced as a password
          field by screen readers. */}
      <Dialog open={pwdResetTarget !== null} onOpenChange={(o) => !o && setPwdResetTarget(null)}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>Reset password</DialogTitle>
            <DialogDescription>Set a new temporary password for this user. They'll be asked to change it on next sign-in.</DialogDescription>
          </DialogHeader>
          <div className="space-y-1.5 py-2">
            <Label htmlFor="reset-pwd">New password for @{pwdResetTarget?.username}</Label>
            <Input
              id="reset-pwd"
              type="password"
              autoComplete="new-password"
              value={pwdDraft}
              onChange={(e) => setPwdDraft(e.target.value)}
              placeholder="Minimum 8 characters"
              minLength={8}
              autoFocus
            />
            <p className="text-xs text-muted-foreground">Minimum 8 characters. The user will be prompted to change it on next login.</p>
          </div>
          <DialogFooter>
            <DialogClose asChild><Button variant="outline">Cancel</Button></DialogClose>
            <Button
              variant="destructive"
              disabled={resetPwd.isPending || pwdDraft.length < 8}
              onClick={async () => {
                if (!pwdResetTarget || pwdDraft.length < 8) return;
                await resetPwd.mutateAsync({ id: pwdResetTarget.id, newPassword: pwdDraft });
                setPwdResetTarget(null);
                setPwdDraft("");
              }}
            >
              {resetPwd.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : "Reset password"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Delete confirmation dialog — replaces the previous window.confirm(). */}
      <Dialog open={deleteTarget !== null} onOpenChange={(o) => !o && setDeleteTarget(null)}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>Delete user?</DialogTitle>
            <DialogDescription>This action cannot be undone.</DialogDescription>
          </DialogHeader>
          <p className="text-sm text-muted-foreground leading-relaxed">
            Delete{" "}
            <span className="font-semibold text-foreground">{deleteTarget?.full_name}</span>{" "}
            (@{deleteTarget?.username})? Their account will be permanently
            removed from the hospital directory.
          </p>
          <DialogFooter>
            <DialogClose asChild><Button variant="outline">Cancel</Button></DialogClose>
            <Button
              variant="destructive"
              disabled={remove.isPending}
              onClick={async () => {
                if (!deleteTarget) return;
                await remove.mutateAsync(deleteTarget.id);
                setDeleteTarget(null);
              }}
            >
              {remove.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : "Delete user"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </PageContainer>
  );
}
