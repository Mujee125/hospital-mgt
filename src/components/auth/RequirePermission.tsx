/**
 * `<RequirePermission>` — declarative RBAC guard for page sections. Renders
 * children only if the current session holds the permission; otherwise
 * renders an inline "access denied" notice (or nothing, with `hide`).
 *
 * The backend re-checks every command independently, so this is a UX guard,
 * not a security control.
 */
import type { ReactNode } from "react";
import { ShieldAlert } from "lucide-react";
import { useAuth } from "@/lib/auth";
import type { Permission } from "@/lib/rbac";

interface Props {
  perm: Permission;
  children: ReactNode;
  hide?: boolean;
}

export function RequirePermission({ perm, children, hide = false }: Props) {
  const { has } = useAuth();
  if (has(perm)) return <>{children}</>;
  if (hide) return null;
  return (
    <div className="flex items-center gap-3 p-4 border border-border bg-muted/40 text-sm text-muted-foreground">
      <ShieldAlert className="h-4 w-4 shrink-0" />
      <span>You don&apos;t have permission to view this section.</span>
    </div>
  );
}
