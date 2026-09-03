/**
 * Patients directory — modernized to the canonical VitalFlow page
 * pattern: PageContainer → PageHeader → SectionCard →
 * (LoadingState | EmptyState | PageToolbar + Table). All hooks,
 * RBAC, deep-link `?add=1` logic, and PatientForm integration are
 * preserved exactly.
 */
import { useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { motion } from "motion/react";
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
  LoadingState,
  PageToolbar,
} from "@/components/layout/shared";

export function Patients() {
  const [searchParams, setSearchParams] = useSearchParams();
  const [searchQuery, setSearchQuery] = useState("");

  const { data: patients = [], isLoading } = usePatients();
  const deletePatient = useDeletePatient();

  const [dialogOpen, setDialogOpen] = useState(false);
  const [selectedPatient, setSelectedPatient] = useState<Patient | undefined>(undefined);
  const [deleteTarget, setDeleteTarget] = useState<Patient | null>(null);

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
                  onChange={(e) => setSearchQuery(e.target.value)}
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
                {filteredPatients.map((patient, i) => (
                  <motion.tr
                    key={patient.id}
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    transition={{ duration: 0.15, delay: Math.min(i * 0.02, 0.3) }}
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
            <DialogTitle>{selectedPatient ? "Edit patient details" : "Register new patient"}</DialogTitle>
            <DialogDescription>
              {selectedPatient
                ? "Modify details below. Click save to store modifications."
                : "Create a new medical file for a patient by filling in details."}
            </DialogDescription>
          </DialogHeader>
          <div className="pt-2">
            <PatientForm patient={selectedPatient} onSuccess={handleFormSuccess} onCancel={() => setDialogOpen(false)} />
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
