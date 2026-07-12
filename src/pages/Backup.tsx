/**
 * Backup & Restore (SRS §9 A-07 — Phase 2).
 *
 * Server-build-only page that lets an admin:
 *   - Create a full-database backup (pg_dump, custom format)
 *   - List existing backups with size + creation time
 *   - Restore from a backup (pg_restore --clean --if-exists — destructive)
 *   - Delete a backup file
 *
 * All four backend commands are RBAC-guarded by `Permission::BackupsManage`.
 * The route itself is wrapped in `<RequirePermission perm={BackupsManage}>`
 * (see App.tsx) so users without the permission never reach this page. The
 * backend re-checks on every command independently — the route guard is a UX
 * affordance, not a security control.
 *
 * On non-server builds (client / dev) the four Tauri commands are not
 * registered (the Rust source is `#[cfg(feature = "server-build")]`), so
 * `invoke` rejects. `useBackups` surfaces this as `error` state; the page
 * renders an inline notice explaining the page requires the server build,
 * instead of crashing.
 *
 * Two prominent warnings:
 *   1. A persistent banner at the top: "After restoring a backup, restart
 *      the application." — because pg_restore --clean drops+recreates every
 *      table, the app's DB pool ends up with stale prepared statements.
 *   2. A red destructive confirmation dialog before any restore, with the
 *      backup filename and a "type-to-confirm" affordance.
 */
import { useState } from "react";
import {
  DatabaseBackup,
  Plus,
  RotateCcw,
  Trash2,
  Loader2,
  AlertTriangle,
  Info,
  ShieldAlert,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
  DialogClose,
} from "@/components/ui/dialog";
import {
  PageContainer,
  PageHeader,
  SectionCard,
  EmptyState,
  LoadingState,
  PageToolbar,
} from "@/components/layout/shared";
import {
  useBackups,
  useCreateBackup,
  useRestoreBackup,
  useDeleteBackup,
} from "@/lib/queries";

// ── Helpers ────────────────────────────────────────────────────────────────

/** Formats a byte count as a human-readable string (e.g. "12.3 MB"). Uses
 *  1024-based units (binary) since backups are disk files, not SI units. */
function formatBytes(bytes: number): string {
  if (!bytes || bytes < 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let unitIndex = 0;
  let value = bytes;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex++;
  }
  // 0 decimals for B, 1 for KB+, but show 2 for small MB values so a 1.05 MB
  // backup doesn't render as "1 MB" (which would round to the same as 1.04).
  const decimals = unitIndex === 0 ? 0 : value < 10 ? 2 : 1;
  return `${value.toFixed(decimals)} ${units[unitIndex]}`;
}

// ── Page ───────────────────────────────────────────────────────────────────

export function Backup() {
  const { data: backups = [], isLoading, error } = useBackups();
  const create = useCreateBackup();
  const restore = useRestoreBackup();
  const remove = useDeleteBackup();

  // Restore confirmation target — set when the user clicks "Restore" on a row.
  const [restoreTarget, setRestoreTarget] = useState<string | null>(null);
  // Typed confirmation phrase. Must match the filename exactly before the
  // destructive Restore button enables — same UX pattern as GitHub's
  // "type the repo name to delete" flow.
  const [restoreConfirmText, setRestoreConfirmText] = useState("");
  // Delete confirmation target.
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);

  const handleCreate = () => {
    // The mutation's onSuccess toast surfaces the resulting filename.
    create.mutate();
  };

  const handleRestoreConfirm = async () => {
    if (!restoreTarget || restoreConfirmText !== restoreTarget) return;
    await restore.mutateAsync(restoreTarget);
    setRestoreTarget(null);
    setRestoreConfirmText("");
  };

  const handleDeleteConfirm = async () => {
    if (!deleteTarget) return;
    await remove.mutateAsync(deleteTarget);
    setDeleteTarget(null);
  };

  // ── Render ──────────────────────────────────────────────────────────────

  return (
    <PageContainer>
      <PageHeader
        icon={DatabaseBackup}
        title="Backup & Restore"
        description="Create and restore full-database backups (server build only)"
        actions={
          <Button onClick={handleCreate} disabled={create.isPending}>
            {create.isPending ? (
              <>
                <Loader2 className="h-4 w-4 animate-spin" />
                Creating backup…
              </>
            ) : (
              <>
                <Plus className="h-4 w-4" />
                Create backup
              </>
            )}
          </Button>
        }
      />

      {/* Persistent warning banner — always visible because the restore
          flow's "restart the app" requirement applies even after a
          successful restore (the DB pool's prepared statements are stale
          after pg_restore --clean). */}
      <div className="flex items-start gap-3 rounded-[var(--radius-md)] border border-warning/30 bg-warning/10 px-4 py-3 text-sm text-warning-foreground">
        <AlertTriangle className="h-4 w-4 shrink-0 mt-0.5 text-warning" />
        <div className="space-y-1">
          <p className="font-semibold text-warning">After restoring a backup, restart the application.</p>
          <p className="text-xs text-muted-foreground leading-relaxed">
            Restoring replaces every table in the database. The app&apos;s in-memory connection pool
            will hold stale prepared statements against the old table OIDs, so a restart is required
            before any other command will work reliably.
          </p>
        </div>
      </div>

      {/* Error state — typically reached on client/dev builds where the
          server-build-only commands are not registered. */}
      {error && (
        <SectionCard>
          <div className="flex items-start gap-3 p-6">
            <ShieldAlert className="h-5 w-5 shrink-0 mt-0.5 text-destructive" />
            <div className="space-y-1">
              <p className="text-sm font-semibold text-foreground">
                Backups are only available on the server build.
              </p>
              <p className="text-xs text-muted-foreground leading-relaxed">
                The <code className="px-1 py-0.5 rounded bg-muted text-foreground">create_backup</code>,
                <code className="px-1 py-0.5 rounded bg-muted text-foreground ml-1">list_backups</code>,
                <code className="px-1 py-0.5 rounded bg-muted text-foreground ml-1">restore_backup</code>, and
                <code className="px-1 py-0.5 rounded bg-muted text-foreground ml-1">delete_backup</code>
                commands are gated to the server build (they shell out to <code>pg_dump</code> /
                <code className="ml-1">pg_restore</code>, which only ship with the bundled PostgreSQL).
                On a client build, connect to the server machine and run backups from there.
              </p>
              <p className="text-[11px] text-muted-foreground mt-2 font-mono">
                {String(error)}
              </p>
            </div>
          </div>
        </SectionCard>
      )}

      <SectionCard>
        {isLoading ? (
          <LoadingState rows={4} />
        ) : backups.length === 0 && !error ? (
          <EmptyState
            icon={DatabaseBackup}
            title="No backups yet"
            description="Create your first full-database backup to get started. Backups are saved as PostgreSQL custom-format .backup files."
            action={
              <Button onClick={handleCreate} disabled={create.isPending}>
                {create.isPending ? (
                  <>
                    <Loader2 className="h-4 w-4 animate-spin" />
                    Creating…
                  </>
                ) : (
                  <>
                    <Plus className="h-4 w-4" />
                    Create backup
                  </>
                )}
              </Button>
            }
          />
        ) : (
          <>
            <PageToolbar>
              <span className="text-sm font-medium text-muted-foreground">
                {backups.length} backup{backups.length === 1 ? "" : "s"} on disk
              </span>
              <span className="text-xs text-muted-foreground ml-auto">
                Stored in <code className="font-mono">%ProgramData%\HMS\backups\</code>
              </span>
            </PageToolbar>
            <Table>
              <TableHeader>
                <TableRow className="border-border hover:bg-transparent">
                  <TableHead scope="col">Filename</TableHead>
                  <TableHead scope="col">Size</TableHead>
                  <TableHead scope="col">Created (UTC)</TableHead>
                  <TableHead scope="col" className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {backups.map((b) => (
                  <TableRow key={b.filename}>
                    <TableCell className="font-mono text-xs text-foreground">
                      {b.filename}
                    </TableCell>
                    <TableCell className="tabular-nums text-muted-foreground">
                      {formatBytes(b.size_bytes)}
                    </TableCell>
                    <TableCell className="text-muted-foreground">
                      {b.created_at}
                    </TableCell>
                    <TableCell className="text-right">
                      <div className="flex justify-end gap-1">
                        <Button
                          size="sm"
                          variant="ghost"
                          className="h-8"
                          disabled={restore.isPending}
                          onClick={() => {
                            setRestoreTarget(b.filename);
                            setRestoreConfirmText("");
                          }}
                        >
                          <RotateCcw className="h-3.5 w-3.5" />
                          Restore
                        </Button>
                        <Button
                          size="icon"
                          variant="ghost"
                          className="h-8 w-8 text-muted-foreground hover:text-destructive"
                          title="Delete backup"
                          aria-label={`Delete ${b.filename}`}
                          disabled={remove.isPending}
                          onClick={() => setDeleteTarget(b.filename)}
                        >
                          <Trash2 className="h-3.5 w-3.5" />
                        </Button>
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </>
        )}
      </SectionCard>

      {/* Info card: what each backup contains + operational notes. */}
      <SectionCard icon={Info} title="What gets backed up">
        <div className="p-6 space-y-3 text-sm text-muted-foreground leading-relaxed">
          <p>
            Each backup is a full snapshot of the hospital database at the moment of creation —
            patients, appointments, encounters, IPD admissions, lab orders, bills, payments,
            inventory, users, roles, audit logs, and system settings. Backups use PostgreSQL&apos;s
            custom format (<code className="font-mono">pg_dump --format=custom</code>), which is
            compressed and supports parallel restore.
          </p>
          <p>
            <span className="font-semibold text-foreground">Restore</span> drops every existing
            table and recreates it from the backup (<code className="font-mono">pg_restore --clean --if-exists</code>).
            Any data created after the backup was taken is lost. The action is recorded in the audit
            log under action <code className="font-mono">backup_restore</code>.
          </p>
          <p>
            <span className="font-semibold text-foreground">Delete</span> only removes the
            <code className="font-mono mx-1">.backup</code>
            file from disk — the database is not touched. Use this to clean up old backups you no
            longer need.
          </p>
        </div>
      </SectionCard>

      {/* ── Restore confirmation dialog ─────────────────────────────────── */}
      {/* Destructive — requires the operator to type the exact filename to
          enable the Restore button. This mirrors the GitHub "type the repo
          name to delete" pattern; it is the strongest non-password
          confirmation affordance available in a desktop UI. */}
      <Dialog
        open={restoreTarget !== null}
        onOpenChange={(o) => {
          if (!o) {
            setRestoreTarget(null);
            setRestoreConfirmText("");
          }
        }}
      >
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle className="text-destructive">Restore from backup?</DialogTitle>
            <DialogDescription>
              WARNING: This will replace ALL current data with the contents of the backup.
              Any data created after the backup was taken will be permanently lost.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-3 py-2">
            <div className="rounded-[var(--radius)] border border-destructive/30 bg-destructive/8 px-3 py-2.5 text-xs text-destructive flex items-start gap-2">
              <AlertTriangle className="h-3.5 w-3.5 shrink-0 mt-0.5" />
              <div>
                <p className="font-semibold">This action cannot be undone.</p>
                <p className="mt-0.5 text-destructive/80">
                  After the restore completes you must restart the application — the database
                  connection pool will hold stale prepared statements against the old tables.
                </p>
              </div>
            </div>
            <p className="text-sm text-muted-foreground leading-relaxed">
              You are about to restore from:
            </p>
            <p className="font-mono text-xs bg-muted px-3 py-2 rounded-[var(--radius)] border border-border break-all">
              {restoreTarget}
            </p>
            <div className="space-y-1.5">
              <label
                htmlFor="restore-confirm"
                className="text-xs font-semibold text-foreground uppercase tracking-wide"
              >
                Type the backup filename to confirm
              </label>
              <input
                id="restore-confirm"
                type="text"
                autoComplete="off"
                spellCheck={false}
                value={restoreConfirmText}
                onChange={(e) => setRestoreConfirmText(e.target.value)}
                placeholder={restoreTarget ?? ""}
                className="h-10 w-full px-3 bg-card border border-border rounded-[var(--radius)] text-sm font-mono outline-none focus:border-primary/50 focus:ring-2 focus:ring-primary/15"
              />
            </div>
          </div>
          <DialogFooter>
            <DialogClose asChild>
              <Button variant="outline">Cancel</Button>
            </DialogClose>
            <Button
              variant="destructive"
              disabled={restore.isPending || restoreConfirmText !== restoreTarget}
              onClick={handleRestoreConfirm}
            >
              {restore.isPending ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin" />
                  Restoring…
                </>
              ) : (
                <>
                  <RotateCcw className="h-4 w-4" />
                  Restore & replace all data
                </>
              )}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* ── Delete confirmation dialog ──────────────────────────────────── */}
      <Dialog
        open={deleteTarget !== null}
        onOpenChange={(o) => !o && setDeleteTarget(null)}
      >
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>Delete backup file?</DialogTitle>
            <DialogDescription>
              The .backup file will be permanently removed from disk. The database is not affected.
            </DialogDescription>
          </DialogHeader>
          <p className="text-sm text-muted-foreground leading-relaxed py-2">
            Delete{" "}
            <span className="font-mono text-xs font-semibold text-foreground">
              {deleteTarget}
            </span>{" "}
            ? This cannot be undone.
          </p>
          <DialogFooter>
            <DialogClose asChild>
              <Button variant="outline">Cancel</Button>
            </DialogClose>
            <Button
              variant="destructive"
              disabled={remove.isPending}
              onClick={handleDeleteConfirm}
            >
              {remove.isPending ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Trash2 className="h-4 w-4" />
              )}
              Delete file
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </PageContainer>
  );
}
