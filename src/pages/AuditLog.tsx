/**
 * Audit log viewer — uses shared layout components.
 */
import { useState } from "react";
import { ScrollText, Filter } from "lucide-react";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { useAuditLogs } from "@/lib/queries";
import { PageContainer, PageHeader, SectionCard, EmptyState, LoadingState, PageToolbar } from "@/components/layout/shared";

export function AuditLog() {
  const [action, setAction] = useState("");
  const [resource, setResource] = useState("");
  const { data: logs = [], isLoading } = useAuditLogs(300, action || null, resource || null);

  return (
    <PageContainer>
      <PageHeader
        icon={ScrollText}
        title="Audit Log"
        description="Immutable record of security-relevant actions"
      />

      <SectionCard>
        <PageToolbar>
          <Filter className="h-4 w-4 text-muted-foreground" />
          <Input className="max-w-[200px]" aria-label="Filter by action" placeholder="Action (e.g. login_success)" value={action} onChange={(e) => setAction(e.target.value)} />
          <Input className="max-w-[200px]" aria-label="Filter by resource" placeholder="Resource (e.g. patients)" value={resource} onChange={(e) => setResource(e.target.value)} />
          <span className="text-xs text-muted-foreground ml-auto">{logs.length} entries</span>
        </PageToolbar>

        {isLoading ? (
          <LoadingState rows={8} />
        ) : logs.length === 0 ? (
          <EmptyState icon={ScrollText} title="No audit entries" description="Try adjusting your filters." />
        ) : (
          <div className="max-h-[70vh] overflow-y-auto">
            <Table>
              <TableHeader>
                <TableRow className="border-border hover:bg-transparent">
                  <TableHead scope="col" className="w-44">When</TableHead>
                  <TableHead scope="col">User</TableHead>
                  <TableHead scope="col">Action</TableHead>
                  <TableHead scope="col">Resource</TableHead>
                  <TableHead scope="col">Details</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {logs.map((l) => (
                  <TableRow key={l.id}>
                    <TableCell className="text-xs text-muted-foreground whitespace-nowrap">{new Date(l.created_at).toLocaleString()}</TableCell>
                    <TableCell className="text-sm font-medium">{l.username ?? "—"}</TableCell>
                    <TableCell><Badge variant="outline" className="font-mono text-[10px]">{l.action}</Badge></TableCell>
                    <TableCell className="text-xs text-muted-foreground">{l.resource}{l.resource_id ? ` #${l.resource_id}` : ""}</TableCell>
                    <TableCell className="text-[11px] text-muted-foreground max-w-[320px] truncate font-mono">
                      {l.details ? JSON.stringify(l.details) : "—"}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        )}
      </SectionCard>
    </PageContainer>
  );
}
