/**
 * Patients directory — modernized to the canonical VitalFlow page
 * pattern: PageContainer → PageHeader → SectionCard →
 * (LoadingState | EmptyState | PageToolbar + Table). All hooks,
 * RBAC, deep-link `?add=1` logic, and PatientForm integration are
 * preserved exactly.
 */
import { useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter, DialogClose } from "@/components/ui/dialog";
import { PatientForm } from "@/components/forms/PatientForm";
import { Search, UserPlus, Edit, Trash2, Users } from "lucide-react";
import { usePatients, useDeletePatient } from "@/lib/queries";
import type { Patient } from "@/lib/models";
import {
  PageContainer,
  PageHeader,
  SectionCard,
  EmptyState,
  ErrorState,
  LoadingState,
  PageToolbar,
  Pagination,
  useDiscardGuard,
} from "@/components/layout/shared";

export function Patients() {
  const [searchParams, setSearchParams] = useSearchParams();
  const [searchQuery, setSearchQuery] = useState("");

  // F-09: a failed load must show the ERROR state, never "No patients
  // registered yet" (staff would re-register patients mid-outage).
  const { data: patients = [], isLoading, isError, refetch, isFetching } = usePatients();
  const deletePatient = useDeletePatient();

  // F-13: pagination. A 10k-patient DB previously rendered every row (with
  // per-row animations — the motion import is gone too); now 25 rows are
  // mounted at a time. Clamping page when the filter shrinks the result
  // (e.g. searching) avoids an out-of-range page state.
  const [page, setPage] = useState(1);
  const [rowsPerPage, setRowsPerPage] = useState(25);

  const [dialogOpen, setDialogOpen] = useState(false);
  const [selectedPatient, setSelectedPatient] = useState<Patient | undefined>(undefined);
  const [deleteTarget, setDeleteTarget] = useState<Patient | null>(null);
  // F-21: tracks unsaved edits in the open patient dialog so Esc/overlay
  // clicks confirm instead of silently discarding a 20-field edit.
  const [formDirty, setFormDirty] = useState(false);
  const guardClose = useDiscardGuard();

  // Deep-linkable "add" trigger — Dashboard's quick-action button
  // navigates to /patients?add=1 instead of the old prop-drilled
  // shouldTriggerAdd/onResetTrigger pattern.
  useEffect(() => {
    if (searchParams.get("add") === "1") {
      handleAddPatient();
      searchParams.delete("add");
      setSearchParams(searchParams, { replace: true });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [searchParams]);

  const handleAddPatient = () => {
    setSelectedPatient(undefined);
    setDialogOpen(true);
  };

  const handleEditPatient = (patient: Patient) => {
    setSelectedPatient(patient);
    setDialogOpen(true);
  };

  const handleDeletePatient = (patient: Patient) => {
    setDeleteTarget(patient);
  };

  const confirmDeletePatient = () => {
    if (!deleteTarget) return;
    deletePatient.mutate(deleteTarget.id, {
      onSettled: () => setDeleteTarget(null),
    });
  };

  const handleFormSuccess = () => {
    setDialogOpen(false);
  };

  const filteredPatients = patients.filter((p) => {
    const fullName = `${p.first_name} ${p.last_name}`.toLowerCase();
    const query = searchQuery.toLowerCase();
    return (
      fullName.includes(query) ||
      p.phone.includes(query) ||
      (p.email && p.email.toLowerCase().includes(query)) ||
      (p.address && p.address.toLowerCase().includes(query))
    );
  });

  // F-13: clamp the page into range (filter shrink / data refresh) and
  // slice the visible window.
  const pageCount = Math.max(1, Math.ceil(filteredPatients.length / rowsPerPage));
  const safePage = Math.min(page, pageCount);
  const pagePatients = filteredPatients.slice(
    (safePage - 1) * rowsPerPage,
    safePage * rowsPerPage,
  );

  const isSearchActive = !!searchQuery.trim();

  return (
    <PageContainer>
      <PageHeader
        icon={Users}
        title="Patient directory"
        description="Manage patient records, demographics, and contact history."
        actions={
          <Button onClick={handleAddPatient} className="gap-2">
            <UserPlus className="h-4 w-4" /> Add patient
          </Button>
        }
      />

      <SectionCard>
        {isLoading ? (
          <LoadingState rows={6} />
        ) : isError ? (
          <ErrorState onRetry={() => void refetch()} retrying={isFetching} />
        ) : filteredPatients.length === 0 ? (
          <EmptyState
            icon={Users}
            title={isSearchActive ? "No patients match your search" : "No patients registered yet"}
            description={
              isSearchActive
                ? "Try a different name, phone number, email, or address."
                : "Register your first patient to start building the clinic's records."
            }
            action={
              !isSearchActive && (
                <Button onClick={handleAddPatient} size="sm" className="gap-2">
                  <UserPlus className="h-3.5 w-3.5" /> Add patient
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
                  placeholder="Search by name, phone, email, or address…"
                  value={searchQuery}
                  onChange={(e) => {
                    setSearchQuery(e.target.value);
                    setPage(1); // F-13: a new search starts on page 1
                  }}
                  className="pl-9 h-10"
                />
              </div>
              <span className="text-xs text-muted-foreground ml-auto tabular-nums">
                {filteredPatients.length} of {patients.length} patients
              </span>
            </PageToolbar>

            <Table>
              <TableHeader>
                <TableRow className="hover:bg-transparent">
                  <TableHead scope="col">Name</TableHead>
                  <TableHead scope="col">Gender</TableHead>
                  <TableHead scope="col">Date of birth</TableHead>
                  <TableHead scope="col">Phone</TableHead>
                  <TableHead scope="col">Email</TableHead>
                  <TableHead scope="col">Address</TableHead>
                  <TableHead scope="col" className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {/* F-13: only the visible page's rows are mounted; the
                    per-row motion animation is gone (10k staggered
                    animations froze the reception workstation). */}
                {pagePatients.map((patient) => (
                  <TableRow
                    key={patient.id}
                    className="border-b border-border/70 transition-colors hover:bg-muted/40"
                  >
                    <TableCell className="font-semibold text-foreground">
                      {patient.first_name} {patient.last_name}
                    </TableCell>
                    <TableCell className="capitalize text-xs font-medium text-muted-foreground">{patient.gender}</TableCell>
                    <TableCell className="font-mono text-xs text-muted-foreground">{patient.date_of_birth}</TableCell>
                    <TableCell className="font-mono text-xs font-semibold">{patient.phone}</TableCell>
                    <TableCell className="text-xs text-muted-foreground">{patient.email || "—"}</TableCell>
                    <TableCell className="max-w-[200px] truncate text-xs text-muted-foreground" title={patient.address || ""}>
                      {patient.address || "—"}
                    </TableCell>
                    <TableCell className="text-right">
                      <div className="flex justify-end gap-1">
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => handleEditPatient(patient)}
                          className="h-8 w-8 text-muted-foreground hover:text-foreground"
                          title="Edit patient details"
                          aria-label={`Edit ${patient.first_name} ${patient.last_name}`}
                        >
                          <Edit className="h-4 w-4" />
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => handleDeletePatient(patient)}
                          disabled={deletePatient.isPending}
                          className="h-8 w-8 text-destructive hover:text-destructive hover:bg-destructive/10"
                          title="Delete patient record"
                          aria-label={`Delete ${patient.first_name} ${patient.last_name}`}
                        >
                          <Trash2 className="h-4 w-4" />
                        </Button>
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
            <Pagination
              totalItems={filteredPatients.length}
              page={safePage}
              rowsPerPage={rowsPerPage}
              onPageChange={setPage}
              onRowsPerPageChange={(rows) => {
                setRowsPerPage(rows);
                setPage(1);
              }}
            />
          </>
        )}
      </SectionCard>

      {/* F-21: closing (Esc/overlay) with unsaved edits confirms first. */}
      <Dialog
        open={dialogOpen}
        onOpenChange={(o) => {
          guardClose(o, formDirty, () => {
            setDialogOpen(false);
            setFormDirty(false);
          });
        }}
      >
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle>{selectedPatient ? "Edit patient details" : "Register new patient"}</DialogTitle>
            <DialogDescription>
              {selectedPatient
                ? "Modify details below. Click save to store modifications."
                : "Create a new medical file for a patient by filling in details."}
            </DialogDescription>
          </DialogHeader>
          <div className="pt-2">
            <PatientForm
              patient={selectedPatient}
              onSuccess={handleFormSuccess}
              onCancel={() => setDialogOpen(false)}
              onDirtyChange={setFormDirty}
            />
          </div>
        </DialogContent>
      </Dialog>

      {/* Delete confirmation dialog — replaces the previous window.confirm()
          that didn't match the app's design language and is blocked by some
          webview policies. State-driven so the focus order is predictable
          for screen readers. */}
      <Dialog open={deleteTarget !== null} onOpenChange={(o) => !o && setDeleteTarget(null)}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>Delete patient record?</DialogTitle>
            <DialogDescription>This action cannot be undone.</DialogDescription>
          </DialogHeader>
          <p className="text-sm text-muted-foreground leading-relaxed">
            Are you sure you want to delete the record for{" "}
            <span className="font-semibold text-foreground">
              {deleteTarget?.first_name} {deleteTarget?.last_name}
            </span>{" "}
            — including all of their appointments? This will also remove the
            record from the patient directory.
          </p>
          <DialogFooter>
            <DialogClose asChild>
              <Button variant="outline">Cancel</Button>
            </DialogClose>
            <Button
              variant="destructive"
              onClick={confirmDeletePatient}
              disabled={deletePatient.isPending}
            >
              {deletePatient.isPending ? "Deleting…" : "Delete patient"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </PageContainer>
  );
}
