/**
 * Blood Bank — donor registry, inventory, cross-matching, issue, transfusion,
 * discard, and traceability (Phase 2-E, SRS FR-0145–FR-0149).
 *
 * Page pattern matches Radiology.tsx (dashboard KPIs + tabbed list + dialogs)
 * and composes the same shared layout primitives — PageContainer, PageHeader,
 * SectionCard, PageToolbar, Table, EmptyState, LoadingState, StatusBadge,
 * StatCard, FormField, Pagination — so it is visually homogeneous.
 *
 * Five tabs:
 *   1. Inventory  — blood units list with status/group/component filters
 *   2. Donors     — donor registry + donation recording + screening
 *   3. Cross-Match — compatibility check + cross-match results
 *   4. Issues     — blood issue records + return
 *   5. Transfusions — transfusion history + reaction tracking
 *
 * Plus a Traceability dialog (opened from any unit row) that shows the full
 * chain-of-custody timeline for a unit (status history + movements +
 * crossmatches + issues + transfusions + discards).
 */
import { useState } from "react";
import {
  Droplet,
  Plus,
  Loader2,
  AlertTriangle,
  CheckCircle2,
  ShieldCheck,
  Heart,
  Activity,
  Ban,
  RotateCcw,
  History,
  TestTube2,
  ArrowRightLeft,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
  DialogClose,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  useBloodBankDashboard,
  useBloodUnits,
  useBloodDonors,
  useBloodCrossmatches,
  useBloodIssues,
  useBloodTransfusions,
  useBloodUnitTraceability,
  useCreateBloodDonor,
  useCreateBloodDonation,
  useCreateBloodUnit,
  useCreateBloodCrossmatch,
  useCheckBloodCompatibility,
  useIssueBlood,
  useReturnBloodUnit,
  useCreateBloodTransfusion,
  useDiscardBloodUnit,
  usePatientsEhr,
  useDoctors,
} from "@/lib/queries";
import { useAuth } from "@/lib/auth";
import { PERMISSIONS } from "@/lib/rbac";
import type {
  BloodUnit,
  BloodDonor,
  CreateBloodDonor,
  CreateBloodDonation,
  CreateBloodUnit,
  CreateBloodCrossmatch,
  CreateBloodIssue,
  CreateBloodTransfusion,
  CreateBloodDiscard,
} from "@/lib/models";
import {
  PageContainer,
  PageHeader,
  SectionCard,
  EmptyState,
  LoadingState,
  PageToolbar,
  StatCard,
  FormField,
  Pagination,
} from "@/components/layout/shared";

// ── Constants ──────────────────────────────────────────────────────────────

const BLOOD_GROUPS = ["A", "B", "AB", "O"] as const;
const RH_FACTORS = ["+", "-"] as const;
const COMPONENT_TYPES = [
  "whole_blood",
  "prbc",
  "ffp",
  "platelets",
  "cryoprecipitate",
  "plasma",
  "granulocytes",
] as const;
const ISSUE_TYPES = ["routine", "emergency", "uncrossmatched", "autologous"] as const;
const DISCARD_REASONS = [
  "expired",
  "contaminated",
  "hemolysed",
  "broken",
  "positive_screen",
  "insufficient_volume",
  "other",
] as const;
const CROSSMATCH_RESULTS = ["compatible", "incompatible", "weak", "indeterminate"] as const;


const COMPONENT_LABELS: Record<string, string> = {
  whole_blood: "Whole Blood",
  prbc: "PRBC",
  ffp: "FFP",
  platelets: "Platelets",
  cryoprecipitate: "Cryoprecipitate",
  plasma: "Plasma",
  granulocytes: "Granulocytes",
};

const UNIT_STATUS_STYLE: Record<string, { color: string; label: string }> = {
  available: { color: "var(--primary)", label: "Available" },
  reserved: { color: "var(--status-scheduled)", label: "Reserved" },
  issued: { color: "var(--status-confirmed)", label: "Issued" },
  transfused: { color: "var(--status-completed)", label: "Transfused" },
  discarded: { color: "var(--status-cancelled)", label: "Discarded" },
  expired: { color: "var(--destructive)", label: "Expired" },
  quarantine: { color: "var(--status-no-show)", label: "Quarantine" },
};

const DONOR_STATUS_STYLE: Record<string, { color: string; label: string }> = {
  active: { color: "var(--primary)", label: "Active" },
  deferred: { color: "var(--status-no-show)", label: "Deferred" },
  blacklisted: { color: "var(--destructive)", label: "Blacklisted" },
};

function StatusPill({ status, map }: { status: string; map: Record<string, { color: string; label: string }> }) {
  const style = map[status] ?? { color: "var(--muted-foreground)", label: status };
  return (
    <span
      className="status-badge uppercase tracking-wide"
      style={{ background: `hsl(${style.color} / 0.10)`, color: `hsl(${style.color})` }}
    >
      <span className="h-1.5 w-1.5 rounded-full" style={{ background: `hsl(${style.color})` }} />
      {style.label}
    </span>
  );
}

function BloodTypeBadge({ group, rh }: { group: string; rh: string }) {
  return (
    <span
      className="status-badge font-bold"
      style={{ background: "hsl(var(--destructive) / 0.10)", color: "hsl(var(--destructive))" }}
    >
      {group}{rh}
    </span>
  );
}

function formatDate(iso?: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  return d.toLocaleDateString("en-GB", { day: "2-digit", month: "short", year: "numeric" });
}

function formatDateTime(iso?: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  return d.toLocaleString("en-GB", {
    day: "2-digit", month: "short", year: "numeric",
    hour: "2-digit", minute: "2-digit",
  });
}

// ── Page ───────────────────────────────────────────────────────────────────

export function BloodBank() {
  const { has } = useAuth();
  const [activeTab, setActiveTab] = useState("inventory");

  const canManageDonors = has(PERMISSIONS.BloodBankDonorManage);
  const canCrossmatch = has(PERMISSIONS.BloodBankCrossmatch);
  const canIssue = has(PERMISSIONS.BloodBankIssue);
  const canTransfuse = has(PERMISSIONS.BloodBankTransfuse);
  const canDiscard = has(PERMISSIONS.BloodBankDiscard);
  const canManage = has(PERMISSIONS.BloodBankManage);

  return (
    <PageContainer>
      <PageHeader
        icon={Droplet}
        title="Blood Bank"
        description="Donor registry, inventory, cross-matching & transfusion"
      />
      <DashboardGrid />

      <Tabs value={activeTab} onValueChange={setActiveTab} className="mt-6">
        <TabsList>
          <TabsTrigger value="inventory">Inventory</TabsTrigger>
          <TabsTrigger value="donors">Donors</TabsTrigger>
          <TabsTrigger value="crossmatch">Cross-Match</TabsTrigger>
          <TabsTrigger value="issues">Issues</TabsTrigger>
          <TabsTrigger value="transfusions">Transfusions</TabsTrigger>
        </TabsList>

        <TabsContent value="inventory" className="mt-4">
          <InventoryTab
            canManage={canManage}
            canDiscard={canDiscard}
          />
        </TabsContent>

        <TabsContent value="donors" className="mt-4">
          <DonorsTab canManageDonors={canManageDonors} />
        </TabsContent>

        <TabsContent value="crossmatch" className="mt-4">
          <CrossmatchTab canCrossmatch={canCrossmatch} />
        </TabsContent>

        <TabsContent value="issues" className="mt-4">
          <IssuesTab canIssue={canIssue} />
        </TabsContent>

        <TabsContent value="transfusions" className="mt-4">
          <TransfusionsTab canTransfuse={canTransfuse} />
        </TabsContent>
      </Tabs>
    </PageContainer>
  );
}

// ── Dashboard KPI grid ─────────────────────────────────────────────────────

function DashboardGrid() {
  const { data: dashboard, isLoading } = useBloodBankDashboard();
  if (isLoading) return <LoadingState rows={6} variant="cards" />;
  if (!dashboard) return null;

  return (
    <div className="grid grid-cols-2 lg:grid-cols-3 xl:grid-cols-6 gap-4 items-stretch">
      <StatCard
        icon={Droplet}
        label="Available"
        value={dashboard.available_units}
        sub="Units in stock"
        color="primary"
      />
      <StatCard
        icon={ShieldCheck}
        label="Reserved"
        value={dashboard.reserved_units}
        sub="Held for patients"
        color="info"
      />
      <StatCard
        icon={ArrowRightLeft}
        label="Issued"
        value={dashboard.issued_units}
        sub="Currently out"
        color="warning"
      />
      <StatCard
        icon={AlertTriangle}
        label="Expiring ≤7d"
        value={dashboard.expiring_soon}
        sub="Use or discard soon"
        color="destructive"
      />
      <StatCard
        icon={Heart}
        label="Total donors"
        value={dashboard.total_donors}
        sub={`${dashboard.active_donors} active`}
        color="info"
      />
      <StatCard
        icon={Activity}
        label="Transfusions today"
        value={dashboard.transfusions_today}
        sub={`${dashboard.active_reservations} active reservations`}
        color="primary"
      />
    </div>
  );
}

// ── Inventory Tab ──────────────────────────────────────────────────────────

function InventoryTab({ canManage, canDiscard }: { canManage: boolean; canDiscard: boolean }) {
  const [statusFilter, setStatusFilter] = useState("available");
  const [groupFilter, setGroupFilter] = useState("");
  const [componentFilter, setComponentFilter] = useState("");
  const [page, setPage] = useState(1);
  const [rowsPerPage, setRowsPerPage] = useState(10);

  const [unitDialogOpen, setUnitDialogOpen] = useState(false);
  const [discardTarget, setDiscardTarget] = useState<BloodUnit | null>(null);
  const [traceabilityUnitId, setTraceabilityUnitId] = useState<number | null>(null);

  const { data: unitsResp, isLoading } = useBloodUnits(
    statusFilter || undefined,
    groupFilter || undefined,
    undefined,
    componentFilter || undefined,
    undefined,
    page,
    rowsPerPage,
  );

  const units = unitsResp?.units ?? [];
  const total = unitsResp?.total ?? 0;

  return (
    <SectionCard>
      <PageToolbar>
        <Select value={statusFilter} onValueChange={(v) => { setStatusFilter(v === "all" ? "" : v); setPage(1); }}>
          <SelectTrigger className="w-[160px]"><SelectValue placeholder="Status" /></SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All statuses</SelectItem>
            <SelectItem value="available">Available</SelectItem>
            <SelectItem value="reserved">Reserved</SelectItem>
            <SelectItem value="issued">Issued</SelectItem>
            <SelectItem value="quarantine">Quarantine</SelectItem>
            <SelectItem value="transfused">Transfused</SelectItem>
            <SelectItem value="discarded">Discarded</SelectItem>
            <SelectItem value="expired">Expired</SelectItem>
          </SelectContent>
        </Select>
        <Select value={groupFilter || "all"} onValueChange={(v) => { setGroupFilter(v === "all" ? "" : v); setPage(1); }}>
          <SelectTrigger className="w-[120px]"><SelectValue placeholder="Blood group" /></SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All groups</SelectItem>
            {BLOOD_GROUPS.map((g) => RH_FACTORS.map((r) => (
              <SelectItem key={`${g}${r}`} value={`${g}${r}`}>{g}{r}</SelectItem>
            )))}
          </SelectContent>
        </Select>
        <Select value={componentFilter || "all"} onValueChange={(v) => { setComponentFilter(v === "all" ? "" : v); setPage(1); }}>
          <SelectTrigger className="w-[160px]"><SelectValue placeholder="Component" /></SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All components</SelectItem>
            {COMPONENT_TYPES.map((c) => (
              <SelectItem key={c} value={c}>{COMPONENT_LABELS[c]}</SelectItem>
            ))}
          </SelectContent>
        </Select>
        {canManage && (
          <Button onClick={() => setUnitDialogOpen(true)} className="ml-auto">
            <Plus className="h-4 w-4" /> Add unit
          </Button>
        )}
      </PageToolbar>

      {isLoading ? (
        <LoadingState rows={6} />
      ) : units.length === 0 ? (
        <EmptyState
          icon={Droplet}
          title="No blood units found"
          description="Adjust the filters or add a new unit to inventory."
        />
      ) : (
        <>
          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead>Unit #</TableHead>
                <TableHead>Type</TableHead>
                <TableHead>Component</TableHead>
                <TableHead>Volume</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Expiry</TableHead>
                <TableHead>Donor</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {units.map((u) => {
                const days = u.days_to_expiry;
                const expiring = days !== null && days !== undefined && days <= 7 && u.status === "available";
                return (
                  <TableRow key={u.id}>
                    <TableCell className="font-mono text-xs">{u.unit_number}</TableCell>
                    <TableCell><BloodTypeBadge group={u.blood_group} rh={u.rh_factor} /></TableCell>
                    <TableCell>{COMPONENT_LABELS[u.component_type] ?? u.component_type}</TableCell>
                    <TableCell>{u.volume_ml} ml</TableCell>
                    <TableCell><StatusPill status={u.status} map={UNIT_STATUS_STYLE} /></TableCell>
                    <TableCell className={expiring ? "text-destructive font-medium" : "text-xs text-muted-foreground"}>
                      {formatDate(u.expiry_date)}
                      {expiring && ` (${days}d)`}
                    </TableCell>
                    <TableCell className="text-xs">{u.donor_name ?? "—"}</TableCell>
                    <TableCell className="text-right">
                      <div className="flex justify-end gap-1">
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => setTraceabilityUnitId(u.id)}
                          aria-label="View traceability"
                        >
                          <History className="h-4 w-4" />
                        </Button>
                        {canDiscard && u.status !== "transfused" && u.status !== "discarded" && u.status !== "expired" && (
                          <Button
                            variant="ghost"
                            size="sm"
                            className="text-destructive"
                            onClick={() => setDiscardTarget(u)}
                            aria-label="Discard unit"
                          >
                            <Ban className="h-4 w-4" />
                          </Button>
                        )}
                      </div>
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
          <Pagination
            totalItems={total}
            page={page}
            rowsPerPage={rowsPerPage}
            onPageChange={setPage}
            onRowsPerPageChange={(r) => { setRowsPerPage(r); setPage(1); }}
          />
        </>
      )}

      {unitDialogOpen && (
        <CreateUnitDialog open={unitDialogOpen} onOpenChange={setUnitDialogOpen} />
      )}
      {discardTarget && (
        <DiscardDialog unit={discardTarget} onClose={() => setDiscardTarget(null)} />
      )}
      {traceabilityUnitId !== null && (
        <TraceabilityDialog unitId={traceabilityUnitId} onClose={() => setTraceabilityUnitId(null)} />
      )}
    </SectionCard>
  );
}

// ── Donors Tab ─────────────────────────────────────────────────────────────

function DonorsTab({ canManageDonors }: { canManageDonors: boolean }) {
  const [search, setSearch] = useState("");
  const [page, setPage] = useState(1);
  const [rowsPerPage, setRowsPerPage] = useState(10);
  const [donorDialogOpen, setDonorDialogOpen] = useState(false);
  const [donationDialogDonor, setDonationDialogDonor] = useState<BloodDonor | null>(null);

  const { data: donorsResp, isLoading } = useBloodDonors(search || undefined, undefined, undefined, page, rowsPerPage);
  const donors = donorsResp?.donors ?? [];
  const total = donorsResp?.total ?? 0;

  return (
    <SectionCard>
      <PageToolbar>
        <Input
          placeholder="Search by name, donor #, or phone…"
          value={search}
          onChange={(e) => { setSearch(e.target.value); setPage(1); }}
          className="w-[280px]"
        />
        {canManageDonors && (
          <Button onClick={() => setDonorDialogOpen(true)} className="ml-auto">
            <Plus className="h-4 w-4" /> Register donor
          </Button>
        )}
      </PageToolbar>

      {isLoading ? (
        <LoadingState rows={6} />
      ) : donors.length === 0 ? (
        <EmptyState
          icon={Heart}
          title="No donors registered"
          description="Register a new blood donor to begin collecting donations."
        />
      ) : (
        <>
          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead>Donor #</TableHead>
                <TableHead>Name</TableHead>
                <TableHead>Blood type</TableHead>
                <TableHead>Phone</TableHead>
                <TableHead>Donations</TableHead>
                <TableHead>Last donation</TableHead>
                <TableHead>Status</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {donors.map((d) => (
                <TableRow key={d.id}>
                  <TableCell className="font-mono text-xs">{d.donor_number}</TableCell>
                  <TableCell className="font-medium">
                    {d.first_name} {d.last_name}
                  </TableCell>
                  <TableCell><BloodTypeBadge group={d.blood_group} rh={d.rh_factor} /></TableCell>
                  <TableCell className="text-xs">{d.phone ?? "—"}</TableCell>
                  <TableCell>{d.total_donations}</TableCell>
                  <TableCell className="text-xs text-muted-foreground">{formatDate(d.last_donation_date)}</TableCell>
                  <TableCell><StatusPill status={d.status} map={DONOR_STATUS_STYLE} /></TableCell>
                  <TableCell className="text-right">
                    {canManageDonors && d.status === "active" && (
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => setDonationDialogDonor(d)}
                      >
                        <Droplet className="h-4 w-4" /> Record donation
                      </Button>
                    )}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
          <Pagination
            totalItems={total}
            page={page}
            rowsPerPage={rowsPerPage}
            onPageChange={setPage}
            onRowsPerPageChange={(r) => { setRowsPerPage(r); setPage(1); }}
          />
        </>
      )}

      {donorDialogOpen && (
        <CreateDonorDialog open={donorDialogOpen} onOpenChange={setDonorDialogOpen} />
      )}
      {donationDialogDonor && (
        <CreateDonationDialog
          donor={donationDialogDonor}
          onClose={() => setDonationDialogDonor(null)}
        />
      )}
    </SectionCard>
  );
}

// ── Cross-Match Tab ────────────────────────────────────────────────────────

function CrossmatchTab({ canCrossmatch }: { canCrossmatch: boolean }) {
  const [resultFilter, setResultFilter] = useState("");
  const [page, setPage] = useState(1);
  const [rowsPerPage, setRowsPerPage] = useState(10);
  const [crossmatchDialogOpen, setCrossmatchDialogOpen] = useState(false);

  const { data: resp, isLoading } = useBloodCrossmatches(
    undefined, undefined, resultFilter || undefined, page, rowsPerPage,
  );
  const crossmatches = resp?.crossmatches ?? [];
  const total = resp?.total ?? 0;

  const RESULT_STYLE: Record<string, { color: string; label: string }> = {
    compatible: { color: "var(--status-completed)", label: "Compatible" },
    incompatible: { color: "var(--destructive)", label: "Incompatible" },
    weak: { color: "var(--status-no-show)", label: "Weak" },
    indeterminate: { color: "var(--status-scheduled)", label: "Indeterminate" },
    pending: { color: "var(--muted-foreground)", label: "Pending" },
  };

  return (
    <SectionCard>
      <PageToolbar>
        <Select value={resultFilter || "all"} onValueChange={(v) => { setResultFilter(v === "all" ? "" : v); setPage(1); }}>
          <SelectTrigger className="w-[160px]"><SelectValue placeholder="Result" /></SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All results</SelectItem>
            {CROSSMATCH_RESULTS.map((r) => (
              <SelectItem key={r} value={r}>{RESULT_STYLE[r]?.label ?? r}</SelectItem>
            ))}
          </SelectContent>
        </Select>
        {canCrossmatch && (
          <Button onClick={() => setCrossmatchDialogOpen(true)} className="ml-auto">
            <TestTube2 className="h-4 w-4" /> New cross-match
          </Button>
        )}
      </PageToolbar>

      {isLoading ? (
        <LoadingState rows={6} />
      ) : crossmatches.length === 0 ? (
        <EmptyState
          icon={TestTube2}
          title="No cross-match records"
          description="Perform a cross-match test before issuing blood to a patient."
        />
      ) : (
        <>
          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead>Unit #</TableHead>
                <TableHead>Patient</TableHead>
                <TableHead>Date</TableHead>
                <TableHead>Method</TableHead>
                <TableHead>Result</TableHead>
                <TableHead>Verified</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {crossmatches.map((c) => (
                <TableRow key={c.id}>
                  <TableCell className="font-mono text-xs">{c.unit_number ?? "—"}</TableCell>
                  <TableCell>{c.patient_name ?? "—"}</TableCell>
                  <TableCell className="text-xs">{formatDateTime(c.crossmatch_date)}</TableCell>
                  <TableCell className="text-xs uppercase">{c.method ?? "—"}</TableCell>
                  <TableCell><StatusPill status={c.result} map={RESULT_STYLE} /></TableCell>
                  <TableCell>
                    {c.verified_at ? (
                      <CheckCircle2 className="h-4 w-4 text-primary" />
                    ) : (
                      <span className="text-xs text-muted-foreground">Pending</span>
                    )}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
          <Pagination
            totalItems={total}
            page={page}
            rowsPerPage={rowsPerPage}
            onPageChange={setPage}
            onRowsPerPageChange={(r) => { setRowsPerPage(r); setPage(1); }}
          />
        </>
      )}

      {crossmatchDialogOpen && (
        <CreateCrossmatchDialog open={crossmatchDialogOpen} onOpenChange={setCrossmatchDialogOpen} />
      )}
    </SectionCard>
  );
}

// ── Issues Tab ─────────────────────────────────────────────────────────────

function IssuesTab({ canIssue }: { canIssue: boolean }) {
  const [page, setPage] = useState(1);
  const [rowsPerPage, setRowsPerPage] = useState(10);
  const [issueDialogOpen, setIssueDialogOpen] = useState(false);

  const { data: resp, isLoading } = useBloodIssues(undefined, undefined, page, rowsPerPage);
  const issues = resp?.issues ?? [];
  const total = resp?.total ?? 0;

  const ISSUE_TYPE_STYLE: Record<string, { color: string; label: string }> = {
    routine: { color: "var(--primary)", label: "Routine" },
    emergency: { color: "var(--destructive)", label: "Emergency" },
    uncrossmatched: { color: "var(--status-no-show)", label: "Uncrossmatched" },
    autologous: { color: "var(--status-completed)", label: "Autologous" },
  };

  return (
    <SectionCard>
      <PageToolbar>
        {canIssue && (
          <Button onClick={() => setIssueDialogOpen(true)} className="ml-auto">
            <ArrowRightLeft className="h-4 w-4" /> Issue blood
          </Button>
        )}
      </PageToolbar>

      {isLoading ? (
        <LoadingState rows={6} />
      ) : issues.length === 0 ? (
        <EmptyState
          icon={ArrowRightLeft}
          title="No blood issues recorded"
          description="Issue blood from the bank to a patient or ward."
        />
      ) : (
        <>
          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead>Issue #</TableHead>
                <TableHead>Unit #</TableHead>
                <TableHead>Patient</TableHead>
                <TableHead>Type</TableHead>
                <TableHead>Issued at</TableHead>
                <TableHead>Issued by</TableHead>
                <TableHead>Returned</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {issues.map((i) => (
                <TableRow key={i.id}>
                  <TableCell className="font-mono text-xs">{i.issue_number}</TableCell>
                  <TableCell className="font-mono text-xs">{i.unit_number ?? "—"}</TableCell>
                  <TableCell>{i.patient_name ?? "—"}</TableCell>
                  <TableCell><StatusPill status={i.issue_type} map={ISSUE_TYPE_STYLE} /></TableCell>
                  <TableCell className="text-xs">{formatDateTime(i.issued_at)}</TableCell>
                  <TableCell className="text-xs">{i.issued_by_name ?? "—"}</TableCell>
                  <TableCell>
                    {i.returned_at ? (
                      <span className="text-xs">{formatDate(i.returned_at)}</span>
                    ) : (
                      <span className="text-xs text-muted-foreground">—</span>
                    )}
                  </TableCell>
                  <TableCell className="text-right">
                    {canIssue && !i.returned_at && (
                      <ReturnButton issueId={i.id} />
                    )}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
          <Pagination
            totalItems={total}
            page={page}
            rowsPerPage={rowsPerPage}
            onPageChange={setPage}
            onRowsPerPageChange={(r) => { setRowsPerPage(r); setPage(1); }}
          />
        </>
      )}

      {issueDialogOpen && (
        <IssueBloodDialog open={issueDialogOpen} onOpenChange={setIssueDialogOpen} />
      )}
    </SectionCard>
  );
}

function ReturnButton({ issueId }: { issueId: number }) {
  const returnMutation = useReturnBloodUnit();
  return (
    <Button
      variant="ghost"
      size="sm"
      disabled={returnMutation.isPending}
      onClick={() => returnMutation.mutate({ issueId, reason: "Returned unused" })}
    >
      <RotateCcw className="h-4 w-4" /> Return
    </Button>
  );
}

// ── Transfusions Tab ───────────────────────────────────────────────────────

function TransfusionsTab({ canTransfuse }: { canTransfuse: boolean }) {
  const [page, setPage] = useState(1);
  const [rowsPerPage, setRowsPerPage] = useState(10);
  const [transfusionDialogOpen, setTransfusionDialogOpen] = useState(false);

  const { data: resp, isLoading } = useBloodTransfusions(undefined, page, rowsPerPage);
  const transfusions = resp?.transfusions ?? [];
  const total = resp?.total ?? 0;

  const OUTCOME_STYLE: Record<string, { color: string; label: string }> = {
    completed: { color: "var(--status-completed)", label: "Completed" },
    reaction: { color: "var(--destructive)", label: "Reaction" },
    incomplete: { color: "var(--status-no-show)", label: "Incomplete" },
    cancelled: { color: "var(--muted-foreground)", label: "Cancelled" },
  };

  return (
    <SectionCard>
      <PageToolbar>
        {canTransfuse && (
          <Button onClick={() => setTransfusionDialogOpen(true)} className="ml-auto">
            <Activity className="h-4 w-4" /> Record transfusion
          </Button>
        )}
      </PageToolbar>

      {isLoading ? (
        <LoadingState rows={6} />
      ) : transfusions.length === 0 ? (
        <EmptyState
          icon={Activity}
          title="No transfusions recorded"
          description="Record a transfusion event after blood is administered to a patient."
        />
      ) : (
        <>
          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead>Transfusion #</TableHead>
                <TableHead>Unit #</TableHead>
                <TableHead>Patient</TableHead>
                <TableHead>Started</TableHead>
                <TableHead>Volume</TableHead>
                <TableHead>Reaction</TableHead>
                <TableHead>Outcome</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {transfusions.map((t) => (
                <TableRow key={t.id}>
                  <TableCell className="font-mono text-xs">{t.transfusion_number}</TableCell>
                  <TableCell className="font-mono text-xs">{t.unit_number ?? "—"}</TableCell>
                  <TableCell>{t.patient_name ?? "—"}</TableCell>
                  <TableCell className="text-xs">{formatDateTime(t.started_at)}</TableCell>
                  <TableCell>{t.volume_transfused_ml ? `${t.volume_transfused_ml} ml` : "—"}</TableCell>
                  <TableCell>
                    {t.reaction_observed ? (
                      <span className="status-badge" style={{ background: "hsl(var(--destructive) / 0.10)", color: "hsl(var(--destructive))" }}>
                        <AlertTriangle className="h-3 w-3" /> {t.reaction_severity ?? "observed"}
                      </span>
                    ) : (
                      <CheckCircle2 className="h-4 w-4 text-primary" />
                    )}
                  </TableCell>
                  <TableCell>
                    {t.outcome && <StatusPill status={t.outcome} map={OUTCOME_STYLE} />}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
          <Pagination
            totalItems={total}
            page={page}
            rowsPerPage={rowsPerPage}
            onPageChange={setPage}
            onRowsPerPageChange={(r) => { setRowsPerPage(r); setPage(1); }}
          />
        </>
      )}

      {transfusionDialogOpen && (
        <CreateTransfusionDialog open={transfusionDialogOpen} onOpenChange={setTransfusionDialogOpen} />
      )}
    </SectionCard>
  );
}

// ── Dialogs ────────────────────────────────────────────────────────────────

function CreateDonorDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (o: boolean) => void }) {
  const createDonor = useCreateBloodDonor();
  const [form, setForm] = useState({
    first_name: "",
    last_name: "",
    blood_group: "O",
    rh_factor: "+",
    phone: "",
    email: "",
    gender: "",
    date_of_birth: "",
    notes: "",
  });

  const set = <K extends keyof typeof form>(key: K, value: (typeof form)[K]) =>
    setForm((f) => ({ ...f, [key]: value }));

  const submit = async () => {
    if (!form.first_name.trim() || !form.last_name.trim()) return;
    const payload: CreateBloodDonor = {
      first_name: form.first_name.trim(),
      last_name: form.last_name.trim(),
      blood_group: form.blood_group,
      rh_factor: form.rh_factor,
      phone: form.phone.trim() || undefined,
      email: form.email.trim() || undefined,
      gender: form.gender || undefined,
      date_of_birth: form.date_of_birth || undefined,
      notes: form.notes.trim() || undefined,
    };
    await createDonor.mutateAsync(payload);
    setForm({ first_name: "", last_name: "", blood_group: "O", rh_factor: "+", phone: "", email: "", gender: "", date_of_birth: "", notes: "" });
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>Register blood donor</DialogTitle>
          <DialogDescription>
            Enter the donor's details. Blood group and Rh factor are required.
          </DialogDescription>
        </DialogHeader>
        <div className="grid grid-cols-2 gap-4 py-2">
          <FormField label="First name" required>
            <Input value={form.first_name} onChange={(e) => set("first_name", e.target.value)} />
          </FormField>
          <FormField label="Last name" required>
            <Input value={form.last_name} onChange={(e) => set("last_name", e.target.value)} />
          </FormField>
          <FormField label="Blood group" required>
            <Select value={form.blood_group} onValueChange={(v) => set("blood_group", v)}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                {BLOOD_GROUPS.map((g) => <SelectItem key={g} value={g}>{g}</SelectItem>)}
              </SelectContent>
            </Select>
          </FormField>
          <FormField label="Rh factor" required>
            <Select value={form.rh_factor} onValueChange={(v) => set("rh_factor", v)}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                {RH_FACTORS.map((r) => <SelectItem key={r} value={r}>{r}</SelectItem>)}
              </SelectContent>
            </Select>
          </FormField>
          <FormField label="Phone">
            <Input value={form.phone} onChange={(e) => set("phone", e.target.value)} placeholder="+92…" />
          </FormField>
          <FormField label="Email">
            <Input value={form.email} onChange={(e) => set("email", e.target.value)} type="email" />
          </FormField>
          <FormField label="Gender">
            <Select value={form.gender || "none"} onValueChange={(v) => set("gender", v === "none" ? "" : v)}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="none">—</SelectItem>
                <SelectItem value="male">Male</SelectItem>
                <SelectItem value="female">Female</SelectItem>
                <SelectItem value="other">Other</SelectItem>
              </SelectContent>
            </Select>
          </FormField>
          <FormField label="Date of birth">
            <Input type="date" value={form.date_of_birth} onChange={(e) => set("date_of_birth", e.target.value)} />
          </FormField>
          <FormField label="Notes" className="col-span-2">
            <Textarea value={form.notes} onChange={(e) => set("notes", e.target.value)} rows={2} />
          </FormField>
        </div>
        <DialogFooter>
          <DialogClose asChild><Button variant="outline">Cancel</Button></DialogClose>
          <Button onClick={submit} disabled={createDonor.isPending}>
            {createDonor.isPending && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
            Register donor
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function CreateDonationDialog({ donor, onClose }: { donor: BloodDonor; onClose: () => void }) {
  const createDonation = useCreateBloodDonation();
  const [form, setForm] = useState({
    volume_ml: 450,
    collection_site: "",
    bag_type: "single",
    hemoglobin_level: "",
    blood_pressure: "",
    pulse: "",
    temperature_c: "",
    notes: "",
  });

  const set = <K extends keyof typeof form>(key: K, value: (typeof form)[K]) =>
    setForm((f) => ({ ...f, [key]: value }));

  const submit = async () => {
    if (form.volume_ml <= 0 || form.volume_ml > 600) return;
    const payload: CreateBloodDonation = {
      donor_id: donor.id,
      collection_site: form.collection_site.trim() || undefined,
      volume_ml: form.volume_ml,
      blood_group: donor.blood_group,
      rh_factor: donor.rh_factor,
      bag_type: form.bag_type || undefined,
      hemoglobin_level: form.hemoglobin_level ? Number(form.hemoglobin_level) : undefined,
      blood_pressure: form.blood_pressure.trim() || undefined,
      pulse: form.pulse ? Number(form.pulse) : undefined,
      temperature_c: form.temperature_c ? Number(form.temperature_c) : undefined,
      notes: form.notes.trim() || undefined,
    };
    await createDonation.mutateAsync(payload);
    onClose();
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>Record donation — {donor.first_name} {donor.last_name} ({donor.blood_group}{donor.rh_factor})</DialogTitle>
          <DialogDescription>
            Collection creates a whole-blood unit with 35-day expiry. Screening status defaults to 'pending'.
          </DialogDescription>
        </DialogHeader>
        <div className="grid grid-cols-2 gap-4 py-2">
          <FormField label="Volume (ml)" required>
            <Input type="number" value={form.volume_ml} onChange={(e) => set("volume_ml", Number(e.target.value))} min={1} max={600} />
          </FormField>
          <FormField label="Collection site">
            <Input value={form.collection_site} onChange={(e) => set("collection_site", e.target.value)} placeholder="e.g. Blood Bank, Mobile Camp" />
          </FormField>
          <FormField label="Bag type">
            <Select value={form.bag_type} onValueChange={(v) => set("bag_type", v)}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="single">Single</SelectItem>
                <SelectItem value="double">Double</SelectItem>
                <SelectItem value="triple">Triple</SelectItem>
              </SelectContent>
            </Select>
          </FormField>
          <FormField label="Hemoglobin (g/dL)">
            <Input type="number" step="0.1" value={form.hemoglobin_level} onChange={(e) => set("hemoglobin_level", e.target.value)} />
          </FormField>
          <FormField label="Blood pressure">
            <Input value={form.blood_pressure} onChange={(e) => set("blood_pressure", e.target.value)} placeholder="120/80" />
          </FormField>
          <FormField label="Pulse (bpm)">
            <Input type="number" value={form.pulse} onChange={(e) => set("pulse", e.target.value)} />
          </FormField>
          <FormField label="Temperature (°C)">
            <Input type="number" step="0.1" value={form.temperature_c} onChange={(e) => set("temperature_c", e.target.value)} />
          </FormField>
          <FormField label="Notes" className="col-span-2">
            <Textarea value={form.notes} onChange={(e) => set("notes", e.target.value)} rows={2} />
          </FormField>
        </div>
        <DialogFooter>
          <DialogClose asChild><Button variant="outline">Cancel</Button></DialogClose>
          <Button onClick={submit} disabled={createDonation.isPending}>
            {createDonation.isPending && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
            Record donation
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function CreateUnitDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (o: boolean) => void }) {
  const createUnit = useCreateBloodUnit();
  const { data: donorsResp } = useBloodDonors(undefined, undefined, "active", 1, 100);
  const availableDonors = donorsResp?.donors ?? [];
  const [form, setForm] = useState({
    donor_id: "" as string,
    component_type: "whole_blood",
    blood_group: "O",
    rh_factor: "+",
    volume_ml: 450,
    storage_temperature: "2-6°C",
    expiry_date: "",
  });

  const set = <K extends keyof typeof form>(key: K, value: (typeof form)[K]) =>
    setForm((f) => ({ ...f, [key]: value }));

  const submit = async () => {
    if (!form.donor_id) return;
    // Default expiry: 35 days from now for whole blood, 5 days for platelets, 1 year for FFP.
    const expiry = form.expiry_date || (() => {
      const d = new Date();
      if (form.component_type === "platelets") d.setDate(d.getDate() + 5);
      else if (form.component_type === "ffp" || form.component_type === "cryoprecipitate") d.setFullYear(d.getFullYear() + 1);
      else d.setDate(d.getDate() + 35);
      return d.toISOString();
    })();

    const payload: CreateBloodUnit = {
      donor_id: Number(form.donor_id),
      component_type: form.component_type,
      blood_group: form.blood_group,
      rh_factor: form.rh_factor,
      volume_ml: form.volume_ml,
      storage_temperature: form.storage_temperature || undefined,
      expiry_date: expiry,
    };
    await createUnit.mutateAsync(payload);
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>Add blood unit to inventory</DialogTitle>
          <DialogDescription>
            Manually add a unit (for component separation or external receipt).
          </DialogDescription>
        </DialogHeader>
        <div className="grid grid-cols-2 gap-4 py-2">
          <FormField label="Donor" required className="col-span-2">
            <Select value={form.donor_id} onValueChange={(v) => set("donor_id", v)}>
              <SelectTrigger><SelectValue placeholder="Select donor" /></SelectTrigger>
              <SelectContent>
                {availableDonors.map((d) => (
                  <SelectItem key={d.id} value={d.id.toString()}>
                    {d.first_name} {d.last_name} · {d.blood_group}{d.rh_factor} · {d.donor_number}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </FormField>
          <FormField label="Component type" required>
            <Select value={form.component_type} onValueChange={(v) => set("component_type", v)}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                {COMPONENT_TYPES.map((c) => <SelectItem key={c} value={c}>{COMPONENT_LABELS[c]}</SelectItem>)}
              </SelectContent>
            </Select>
          </FormField>
          <FormField label="Volume (ml)" required>
            <Input type="number" value={form.volume_ml} onChange={(e) => set("volume_ml", Number(e.target.value))} min={1} />
          </FormField>
          <FormField label="Blood group" required>
            <Select value={form.blood_group} onValueChange={(v) => set("blood_group", v)}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                {BLOOD_GROUPS.map((g) => <SelectItem key={g} value={g}>{g}</SelectItem>)}
              </SelectContent>
            </Select>
          </FormField>
          <FormField label="Rh factor" required>
            <Select value={form.rh_factor} onValueChange={(v) => set("rh_factor", v)}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                {RH_FACTORS.map((r) => <SelectItem key={r} value={r}>{r}</SelectItem>)}
              </SelectContent>
            </Select>
          </FormField>
          <FormField label="Storage temperature">
            <Input value={form.storage_temperature} onChange={(e) => set("storage_temperature", e.target.value)} />
          </FormField>
          <FormField label="Expiry date (optional — auto-calculated if blank)">
            <Input type="datetime-local" value={form.expiry_date} onChange={(e) => set("expiry_date", e.target.value)} />
          </FormField>
        </div>
        <DialogFooter>
          <DialogClose asChild><Button variant="outline">Cancel</Button></DialogClose>
          <Button onClick={submit} disabled={createUnit.isPending}>
            {createUnit.isPending && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
            Add unit
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function DiscardDialog({ unit, onClose }: { unit: BloodUnit; onClose: () => void }) {
  const discardMutation = useDiscardBloodUnit();
  const [reason, setReason] = useState<string>("expired");
  const [notes, setNotes] = useState("");

  const submit = async () => {
    const payload: CreateBloodDiscard = {
      unit_id: unit.id,
      discard_reason: reason,
      discard_notes: notes.trim() || undefined,
    };
    await discardMutation.mutateAsync(payload);
    onClose();
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Discard blood unit {unit.unit_number}</DialogTitle>
          <DialogDescription>
            This action is irreversible. The unit will be moved to 'discarded' (terminal) status
            and recorded in the discard log for traceability.
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-4 py-2">
          <FormField label="Discard reason" required>
            <Select value={reason} onValueChange={setReason}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                {DISCARD_REASONS.map((r) => <SelectItem key={r} value={r} className="capitalize">{r.replace(/_/g, " ")}</SelectItem>)}
              </SelectContent>
            </Select>
          </FormField>
          <FormField label="Notes">
            <Textarea value={notes} onChange={(e) => setNotes(e.target.value)} rows={3} placeholder="Additional context…" />
          </FormField>
        </div>
        <DialogFooter>
          <DialogClose asChild><Button variant="outline">Cancel</Button></DialogClose>
          <Button variant="destructive" onClick={submit} disabled={discardMutation.isPending}>
            {discardMutation.isPending && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
            Discard unit
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function CreateCrossmatchDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (o: boolean) => void }) {
  const createCrossmatch = useCreateBloodCrossmatch();
  const checkCompat = useCheckBloodCompatibility();
  const { data: unitsResp } = useBloodUnits("available", undefined, undefined, undefined, undefined, 1, 100);
  const { data: patients = [] } = usePatientsEhr();
  const { data: doctors = [] } = useDoctors();
  const availableUnits = unitsResp?.units ?? [];

  const [form, setForm] = useState({
    unit_id: "",
    patient_id: "",
    doctor_id: "",
    method: "saline_37c",
    result: "compatible",
    notes: "",
  });

  const set = <K extends keyof typeof form>(key: K, value: (typeof form)[K]) =>
    setForm((f) => ({ ...f, [key]: value }));

  const runCompatCheck = async () => {
    if (!form.unit_id || !form.patient_id) return;
    await checkCompat.mutateAsync({ unitId: Number(form.unit_id), patientId: Number(form.patient_id) });
  };

  const submit = async () => {
    if (!form.unit_id || !form.patient_id) return;
    const payload: CreateBloodCrossmatch = {
      unit_id: Number(form.unit_id),
      patient_id: Number(form.patient_id),
      doctor_id: form.doctor_id ? Number(form.doctor_id) : null,
      method: form.method,
      result: form.result,
      notes: form.notes.trim() || undefined,
    };
    await createCrossmatch.mutateAsync(payload);
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>New cross-match test</DialogTitle>
          <DialogDescription>
            Record the result of a compatibility test between a donor unit and a recipient patient.
          </DialogDescription>
        </DialogHeader>
        <div className="grid grid-cols-2 gap-4 py-2">
          <FormField label="Blood unit" required className="col-span-2">
            <Select value={form.unit_id} onValueChange={(v) => set("unit_id", v)}>
              <SelectTrigger><SelectValue placeholder="Select available unit" /></SelectTrigger>
              <SelectContent>
                {availableUnits.map((u) => (
                  <SelectItem key={u.id} value={u.id.toString()}>
                    {u.unit_number} · {u.blood_group}{u.rh_factor} · {COMPONENT_LABELS[u.component_type]} · exp {formatDate(u.expiry_date)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </FormField>
          <FormField label="Patient" required className="col-span-2">
            <Select value={form.patient_id} onValueChange={(v) => set("patient_id", v)}>
              <SelectTrigger><SelectValue placeholder="Select patient" /></SelectTrigger>
              <SelectContent>
                {patients.map((p) => (
                  <SelectItem key={p.id} value={p.id.toString()}>
                    {p.first_name} {p.last_name} · {p.phone}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </FormField>
          <FormField label="Doctor">
            <Select value={form.doctor_id || "none"} onValueChange={(v) => set("doctor_id", v === "none" ? "" : v)}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="none">—</SelectItem>
                {doctors.filter((d) => d.is_active).map((d) => (
                  <SelectItem key={d.id} value={d.id.toString()}>
                    Dr. {d.first_name} {d.last_name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </FormField>
          <FormField label="Method">
            <Select value={form.method} onValueChange={(v) => set("method", v)}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="saline_37c">Saline 37°C</SelectItem>
                <SelectItem value="ahg">AHG</SelectItem>
                <SelectItem value="gel_card">Gel Card</SelectItem>
                <SelectItem value="tube_ahg">Tube AHG</SelectItem>
                <SelectItem value="electronic">Electronic</SelectItem>
              </SelectContent>
            </Select>
          </FormField>
          <FormField label="Result" required>
            <Select value={form.result} onValueChange={(v) => set("result", v)}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                {CROSSMATCH_RESULTS.map((r) => <SelectItem key={r} value={r} className="capitalize">{r}</SelectItem>)}
              </SelectContent>
            </Select>
          </FormField>
          <FormField label="Notes" className="col-span-2">
            <Textarea value={form.notes} onChange={(e) => set("notes", e.target.value)} rows={2} />
          </FormField>

          {checkCompat.data && (
            <div className={`col-span-2 rounded-md p-3 text-sm ${checkCompat.data.compatible ? "bg-primary/10 text-primary" : "bg-destructive/10 text-destructive"}`}>
              <strong>ABO/Rh check:</strong> {checkCompat.data.reason}
              {" — donor "}{checkCompat.data.donor_group}{checkCompat.data.donor_rh}
              {" → patient "}{checkCompat.data.patient_group}{checkCompat.data.patient_rh}
            </div>
          )}
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={runCompatCheck} disabled={checkCompat.isPending || !form.unit_id || !form.patient_id}>
            {checkCompat.isPending && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
            Check ABO/Rh compatibility
          </Button>
          <DialogClose asChild><Button variant="outline">Cancel</Button></DialogClose>
          <Button onClick={submit} disabled={createCrossmatch.isPending}>
            {createCrossmatch.isPending && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
            Record cross-match
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function IssueBloodDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (o: boolean) => void }) {
  const issueBlood = useIssueBlood();
  const { data: unitsResp } = useBloodUnits(undefined, undefined, undefined, undefined, undefined, 1, 100);
  const { data: patients = [] } = usePatientsEhr();
  const { data: doctors = [] } = useDoctors();
  const issuableUnits = (unitsResp?.units ?? []).filter((u) => u.status === "available" || u.status === "reserved");

  const [form, setForm] = useState({
    unit_id: "",
    patient_id: "",
    doctor_id: "",
    issue_type: "routine",
    issued_to_location: "",
    clinical_indication: "",
    special_instructions: "",
  });

  const set = <K extends keyof typeof form>(key: K, value: (typeof form)[K]) =>
    setForm((f) => ({ ...f, [key]: value }));

  const submit = async () => {
    if (!form.unit_id || !form.patient_id) return;
    const payload: CreateBloodIssue = {
      unit_id: Number(form.unit_id),
      patient_id: Number(form.patient_id),
      doctor_id: form.doctor_id ? Number(form.doctor_id) : null,
      issue_type: form.issue_type,
      issued_to_location: form.issued_to_location.trim() || undefined,
      clinical_indication: form.clinical_indication.trim() || undefined,
      special_instructions: form.special_instructions.trim() || undefined,
    };
    await issueBlood.mutateAsync(payload);
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>Issue blood</DialogTitle>
          <DialogDescription>
            Issue a blood unit from the bank to a patient. The unit must be available or reserved for this patient.
          </DialogDescription>
        </DialogHeader>
        <div className="grid grid-cols-2 gap-4 py-2">
          <FormField label="Blood unit" required className="col-span-2">
            <Select value={form.unit_id} onValueChange={(v) => set("unit_id", v)}>
              <SelectTrigger><SelectValue placeholder="Select unit" /></SelectTrigger>
              <SelectContent>
                {issuableUnits.map((u) => (
                  <SelectItem key={u.id} value={u.id.toString()}>
                    {u.unit_number} · {u.blood_group}{u.rh_factor} · {COMPONENT_LABELS[u.component_type]} · {u.status}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </FormField>
          <FormField label="Patient" required className="col-span-2">
            <Select value={form.patient_id} onValueChange={(v) => set("patient_id", v)}>
              <SelectTrigger><SelectValue placeholder="Select patient" /></SelectTrigger>
              <SelectContent>
                {patients.map((p) => (
                  <SelectItem key={p.id} value={p.id.toString()}>
                    {p.first_name} {p.last_name} · {p.phone}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </FormField>
          <FormField label="Doctor">
            <Select value={form.doctor_id || "none"} onValueChange={(v) => set("doctor_id", v === "none" ? "" : v)}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="none">—</SelectItem>
                {doctors.filter((d) => d.is_active).map((d) => (
                  <SelectItem key={d.id} value={d.id.toString()}>
                    Dr. {d.first_name} {d.last_name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </FormField>
          <FormField label="Issue type" required>
            <Select value={form.issue_type} onValueChange={(v) => set("issue_type", v)}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                {ISSUE_TYPES.map((t) => <SelectItem key={t} value={t} className="capitalize">{t}</SelectItem>)}
              </SelectContent>
            </Select>
          </FormField>
          <FormField label="Issued to location">
            <Input value={form.issued_to_location} onChange={(e) => set("issued_to_location", e.target.value)} placeholder="e.g. OT, Ward 3, ICU" />
          </FormField>
          <FormField label="Clinical indication">
            <Input value={form.clinical_indication} onChange={(e) => set("clinical_indication", e.target.value)} placeholder="e.g. Acute blood loss" />
          </FormField>
          <FormField label="Special instructions" className="col-span-2">
            <Textarea value={form.special_instructions} onChange={(e) => set("special_instructions", e.target.value)} rows={2} />
          </FormField>
        </div>
        <DialogFooter>
          <DialogClose asChild><Button variant="outline">Cancel</Button></DialogClose>
          <Button onClick={submit} disabled={issueBlood.isPending}>
            {issueBlood.isPending && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
            Issue blood
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function CreateTransfusionDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (o: boolean) => void }) {
  const createTransfusion = useCreateBloodTransfusion();
  const { data: issuesResp } = useBloodIssues(undefined, undefined, 1, 100);
  const { data: doctors = [] } = useDoctors();
  const openIssues = (issuesResp?.issues ?? []).filter((i) => !i.returned_at);

  const [form, setForm] = useState({
    issue_id: "",
    unit_id: "",
    patient_id: "",
    doctor_id: "",
    volume_transfused_ml: "",
    pre_bp: "",
    post_bp: "",
    pre_pulse: "",
    post_pulse: "",
    reaction_observed: false,
    reaction_type: "",
    reaction_severity: "",
    outcome: "completed",
    notes: "",
  });

  const set = <K extends keyof typeof form>(key: K, value: (typeof form)[K]) =>
    setForm((f) => ({ ...f, [key]: value }));

  const submit = async () => {
    if (!form.issue_id) return;
    const issue = openIssues.find((i) => i.id === Number(form.issue_id));
    if (!issue) return;
    const payload: CreateBloodTransfusion = {
      issue_id: issue.id,
      unit_id: issue.unit_id,
      patient_id: issue.patient_id,
      doctor_id: form.doctor_id ? Number(form.doctor_id) : null,
      volume_transfused_ml: form.volume_transfused_ml ? Number(form.volume_transfused_ml) : null,
      pre_transfusion_bp: form.pre_bp || undefined,
      post_transfusion_bp: form.post_bp || undefined,
      pre_transfusion_pulse: form.pre_pulse ? Number(form.pre_pulse) : undefined,
      post_transfusion_pulse: form.post_pulse ? Number(form.post_pulse) : undefined,
      reaction_observed: form.reaction_observed,
      reaction_type: form.reaction_type || undefined,
      reaction_severity: form.reaction_severity || undefined,
      outcome: form.outcome,
      notes: form.notes.trim() || undefined,
    };
    await createTransfusion.mutateAsync(payload);
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>Record transfusion</DialogTitle>
          <DialogDescription>
            Record the administration of blood to a patient. Select an open (non-returned) issue.
          </DialogDescription>
        </DialogHeader>
        <div className="grid grid-cols-2 gap-4 py-2">
          <FormField label="Blood issue" required className="col-span-2">
            <Select value={form.issue_id} onValueChange={(v) => set("issue_id", v)}>
              <SelectTrigger><SelectValue placeholder="Select open issue" /></SelectTrigger>
              <SelectContent>
                {openIssues.map((i) => (
                  <SelectItem key={i.id} value={i.id.toString()}>
                    {i.issue_number} · {i.unit_number} · {i.patient_name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </FormField>
          <FormField label="Doctor">
            <Select value={form.doctor_id || "none"} onValueChange={(v) => set("doctor_id", v === "none" ? "" : v)}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="none">—</SelectItem>
                {doctors.filter((d) => d.is_active).map((d) => (
                  <SelectItem key={d.id} value={d.id.toString()}>
                    Dr. {d.first_name} {d.last_name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </FormField>
          <FormField label="Volume transfused (ml)">
            <Input type="number" value={form.volume_transfused_ml} onChange={(e) => set("volume_transfused_ml", e.target.value)} />
          </FormField>
          <FormField label="Pre-transfusion BP">
            <Input value={form.pre_bp} onChange={(e) => set("pre_bp", e.target.value)} placeholder="120/80" />
          </FormField>
          <FormField label="Post-transfusion BP">
            <Input value={form.post_bp} onChange={(e) => set("post_bp", e.target.value)} placeholder="120/80" />
          </FormField>
          <FormField label="Pre-transfusion pulse">
            <Input type="number" value={form.pre_pulse} onChange={(e) => set("pre_pulse", e.target.value)} />
          </FormField>
          <FormField label="Post-transfusion pulse">
            <Input type="number" value={form.post_pulse} onChange={(e) => set("post_pulse", e.target.value)} />
          </FormField>
          <FormField label="Outcome">
            <Select value={form.outcome} onValueChange={(v) => set("outcome", v)}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="completed">Completed</SelectItem>
                <SelectItem value="reaction">Reaction</SelectItem>
                <SelectItem value="incomplete">Incomplete</SelectItem>
                <SelectItem value="cancelled">Cancelled</SelectItem>
              </SelectContent>
            </Select>
          </FormField>
          <FormField label="Reaction observed" className="col-span-2">
            <Select value={form.reaction_observed ? "yes" : "no"} onValueChange={(v) => set("reaction_observed", v === "yes")}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="no">No reaction</SelectItem>
                <SelectItem value="yes">Reaction observed</SelectItem>
              </SelectContent>
            </Select>
          </FormField>
          {form.reaction_observed && (
            <>
              <FormField label="Reaction type">
                <Input value={form.reaction_type} onChange={(e) => set("reaction_type", e.target.value)} placeholder="e.g. Febrile, Allergic" />
              </FormField>
              <FormField label="Reaction severity">
                <Select value={form.reaction_severity || "none"} onValueChange={(v) => set("reaction_severity", v === "none" ? "" : v)}>
                  <SelectTrigger><SelectValue /></SelectTrigger>
                  <SelectContent>
                    <SelectItem value="none">—</SelectItem>
                    <SelectItem value="mild">Mild</SelectItem>
                    <SelectItem value="moderate">Moderate</SelectItem>
                    <SelectItem value="severe">Severe</SelectItem>
                    <SelectItem value="fatal">Fatal</SelectItem>
                  </SelectContent>
                </Select>
              </FormField>
            </>
          )}
          <FormField label="Notes" className="col-span-2">
            <Textarea value={form.notes} onChange={(e) => set("notes", e.target.value)} rows={2} />
          </FormField>
        </div>
        <DialogFooter>
          <DialogClose asChild><Button variant="outline">Cancel</Button></DialogClose>
          <Button onClick={submit} disabled={createTransfusion.isPending}>
            {createTransfusion.isPending && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
            Record transfusion
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function TraceabilityDialog({ unitId, onClose }: { unitId: number; onClose: () => void }) {
  const { data: trace, isLoading } = useBloodUnitTraceability(unitId);

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-3xl max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>Unit traceability — #{unitId}</DialogTitle>
          <DialogDescription>
            Full chain-of-custody timeline (FR-0149): status history, movements, cross-matches, issues, transfusions, and discards.
          </DialogDescription>
        </DialogHeader>
        {isLoading ? (
          <LoadingState rows={6} />
        ) : !trace ? (
          <EmptyState icon={History} title="No data" description="No traceability records found for this unit." />
        ) : (
          <div className="space-y-4 py-2">
            <TraceSection title="Status history" items={trace.status_history.map((h) => ({
              date: formatDateTime(h.changed_at),
              label: h.status,
              detail: h.notes ?? undefined,
              actor: h.changed_by_name ?? undefined,
            }))} />
            <TraceSection title="Inventory movements" items={trace.movements.map((m) => ({
              date: formatDateTime(m.moved_at),
              label: m.movement_type,
              detail: [m.from_location, m.to_location].filter(Boolean).join(" → ") || undefined,
              actor: m.moved_by_name ?? undefined,
            }))} />
            <TraceSection title="Cross-matches" items={trace.crossmatches.map((c) => ({
              date: formatDateTime(c.crossmatch_date),
              label: c.result,
              detail: c.patient_name ?? undefined,
            }))} />
            <TraceSection title="Issues" items={trace.issues.map((i) => ({
              date: formatDateTime(i.issued_at),
              label: i.issue_type,
              detail: i.patient_name ?? undefined,
              actor: i.issued_by_name ?? undefined,
            }))} />
            <TraceSection title="Transfusions" items={trace.transfusions.map((t) => ({
              date: formatDateTime(t.started_at),
              label: t.outcome ?? "in-progress",
              detail: t.patient_name ?? undefined,
            }))} />
            <TraceSection title="Discards" items={trace.discards.map((d) => ({
              date: formatDateTime(d.discarded_at),
              label: d.discard_reason,
              detail: d.discard_notes ?? undefined,
              actor: d.discarded_by_name ?? undefined,
            }))} />
          </div>
        )}
        <DialogFooter>
          <DialogClose asChild><Button variant="outline">Close</Button></DialogClose>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function TraceSection({ title, items }: { title: string; items: Array<{ date: string; label: string; detail?: string; actor?: string }> }) {
  if (items.length === 0) return null;
  return (
    <div>
      <h4 className="text-sm font-semibold text-muted-foreground mb-2 uppercase tracking-wide">{title}</h4>
      <div className="space-y-1 max-h-48 overflow-y-auto">
        {items.map((item, idx) => (
          <div key={idx} className="flex items-start gap-3 text-sm border-l-2 border-primary/30 pl-3 py-1">
            <div className="flex-1">
              <span className="font-medium capitalize">{item.label}</span>
              {item.detail && <span className="text-muted-foreground"> — {item.detail}</span>}
              {item.actor && <span className="text-xs text-muted-foreground"> · by {item.actor}</span>}
            </div>
            <span className="text-xs text-muted-foreground whitespace-nowrap">{item.date}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
