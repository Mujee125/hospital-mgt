
import { NavLink } from "react-router-dom";
import logo from "@/assets/logo_transparant.png";
import {
  LayoutDashboard,
  Calendar,
  Users,
  Settings,
  MessageSquare,
  BarChart3,
  Receipt,
  ChevronsLeft,
  ChevronsRight,
  ChevronRight,
  ListOrdered,
  BedDouble,
  FlaskConical,
  ScrollText,
  UserCog,
  Stethoscope,
  Package,
  DatabaseBackup,
  Pill,
  ScanLine,
  Droplet,
} from "lucide-react";
import { useAuth } from "@/lib/auth";
import { PERMISSIONS, ROLE_LABELS, type NavItem } from "@/lib/rbac";

const TITLEBAR_HEIGHT = 40;
interface SidebarProps {
  serverMode: boolean;
  collapsed: boolean;
  onToggleCollapsed: () => void;
  isMobileDrawer?: boolean;
  onNavigate?: () => void;
  /**
   * Per-deployment hospital name (from licenseInfo.hospital_name).
   * Falls back to "VitalFlow HMS" when not set. Replaces the
   * hard-coded "Rasheed Medical Center" demo string so the sidebar
   * brand lockup is correct for every deployment.
   */
  hospitalName?: string;
}

const menuItems: NavItem[] = [
  {
    to: "/",
    label: "Dashboard",
    icon: LayoutDashboard,
    end: true,
    requiredPermission: PERMISSIONS.DashboardView,
  },
  {
    to: "/appointments",
    label: "Appointments",
    icon: Calendar,
    requiredPermission: PERMISSIONS.AppointmentsView,
  },
  {
    to: "/patients",
    label: "Patients",
    icon: Users,
    requiredPermission: PERMISSIONS.PatientsView,
  },
  {
    to: "/queue",
    label: "Queue",
    icon: ListOrdered,
    requiredPermission: PERMISSIONS.QueueView,
  },
  {
    to: "/doctors",
    label: "Doctors",
    icon: Stethoscope,
    requiredPermission: PERMISSIONS.DoctorsView,
  },
  {
    to: "/ipd",
    label: "In-Patient",
    icon: BedDouble,
    requiredPermission: PERMISSIONS.IpdView,
  },
  {
    to: "/laboratory",
    label: "Laboratory",
    icon: FlaskConical,
    requiredPermission: PERMISSIONS.LabView,
  },
  {
    to: "/radiology",
    label: "Radiology",
    icon: ScanLine,
    requiredPermission: PERMISSIONS.RadiologyView,
  },
  {
    to: "/blood-bank",
    label: "Blood Bank",
    icon: Droplet,
    requiredPermission: PERMISSIONS.BloodBankView,
  },
  {
    to: "/billing",
    label: "Billing",
    icon: Receipt,
    requiredPermission: PERMISSIONS.BillingView,
  },
  {
    to: "/inventory",
    label: "Inventory",
    icon: Package,
    requiredPermission: PERMISSIONS.InventoryView,
  },
  {
    to: "/pharmacy",
    label: "Pharmacy",
    icon: Pill,
    requiredPermission: PERMISSIONS.InventoryView,
  },
  { to: "/messaging", label: "Staff chat", icon: MessageSquare, badge: "Live" },
  {
    to: "/audit",
    label: "Audit log",
    icon: ScrollText,
    requiredPermission: PERMISSIONS.AuditView,
  },
  {
    to: "/users",
    label: "Users & roles",
    icon: UserCog,
    requiredPermission: PERMISSIONS.UsersView,
  },
  {
    to: "/reports",
    label: "Reports",
    icon: BarChart3,
    requiredPermission: PERMISSIONS.ReportsView,
  },
  {
    to: "/backup",
    label: "Backup",
    icon: DatabaseBackup,
    requiredPermission: PERMISSIONS.BackupsManage,
  },
  {
    to: "/settings",
    label: "Settings",
    icon: Settings,
    requiredPermission: PERMISSIONS.SettingsManage,
  },
];

// Nav item base — plain Tailwind so layout renders even before CSS helpers
// propagate. The active/inactive states are applied per-link below.
const NAV_ITEM_BASE =
  "group relative flex items-center gap-3 w-full px-3.5 py-2.5 rounded-[var(--radius-md)] border-transparent transition-all duration-150";

export function Sidebar({
  collapsed,
  onToggleCollapsed,
  isMobileDrawer = false,
  onNavigate,
  hospitalName,
}: SidebarProps) {
  const { session, has } = useAuth();
  const showExpanded = isMobileDrawer || !collapsed;
  const visibleItems = menuItems.filter(
    (item) => !item.requiredPermission || has(item.requiredPermission),
  );
  const primaryRole = session?.roles?.[0];

  return (
    <aside
      className="flex flex-col h-full border-white/10 select-none transition-[width] duration-200 ease-[cubic-bezier(0.22,1,0.36,1)]"
      style={{
        width: isMobileDrawer ? "100%" : showExpanded ? "264px" : "72px",
        background:
          "linear-gradient(to bottom, hsl(168 89% 30%) 0%, hsl(159 91% 18%) 50%, hsl(230 94% 18%) 100%)",
      }}
    >
      {/* Brand */}
      <div
        data-tauri-drag-region
        className="flex items-center gap-2 pl-3 pr-4 shrink-0 border-b border-white/10"
        style={{ height: TITLEBAR_HEIGHT }}
      >
        <img src={logo} alt="Logo" className="w-10 h-10 ml-3 object-contain brightness-0 invert" />
        <span className="text-[13px] font-semibold tracking-tight text-white">
          {hospitalName ?? "Rasheed Medical Center"}
        </span>
      </div>

      {/* Nav */}
      <nav className="flex-1 px-3 py-4 space-y-1 overflow-y-auto overflow-x-hidden no-scrollbar">
        {visibleItems.map((item) => {
          const Icon = item.icon as React.ComponentType<{ className?: string }>;

          if (item.disabled) {
            return (
              <div
                key={item.label}
                title={
                  showExpanded ? undefined : `${item.label} — ${item.note}`
                }
                className={`${NAV_ITEM_BASE} border-white/5 text-white/30 cursor-not-allowed ${!showExpanded ? "justify-center" : ""}`}
              >
                <Icon className="h-[18px] w-[18px] shrink-0" />
                {showExpanded && (
                  <>
                    <span className="truncate flex-1 text-sm font-medium">
                      {item.label}
                    </span>
                    <span className="shrink-0 text-[9px] uppercase tracking-wide font-bold px-1.5 py-0.5 rounded-full bg-white/10 text-white/50">
                      {item.note}
                    </span>
                  </>
                )}
              </div>
            );
          }

          return (
            <NavLink
              key={item.to}
              to={item.to}
              end={(item as NavItem & { end?: boolean }).end}
              onClick={onNavigate}
              title={showExpanded ? undefined : item.label}
              className={({ isActive }) =>
                `${NAV_ITEM_BASE} ${!showExpanded ? "justify-center" : ""} ${
                  isActive
                    ? "bg-white/[0.10] text-white border-white/20 shadow-sm"
                    : "text-white/60 hover:bg-white/[0.06] hover:text-white border-transparent"
                }`
              }
            >
              {({ isActive }) => (
                <>
                  {/* Active accent bar */}
                  {isActive && (
                    <span className="absolute left-0 top-1/2 -translate-y-1/2 h-5 w-1 rounded-full bg-primary" />
                  )}
                  <Icon
                    className={`h-[18px] w-[18px] shrink-0 transition-colors ${isActive ? "text-white" : ""}`}
                  />
                  {showExpanded && (
                    <>
                      <span
                        className={`truncate flex-1 text-sm ${isActive ? "font-semibold" : "font-medium"}`}
                      >
                        {item.label}
                      </span>
                      {item.badge && (
                        <span className="flex items-center gap-1 shrink-0">
                          <span className="h-1.5 w-1.5 rounded-full bg-success animate-pulse" />
                          <span className="text-[9px] font-bold uppercase tracking-wide text-success">
                            {item.badge}
                          </span>
                        </span>
                      )}
                      <ChevronRight
                        className={`h-3.5 w-3.5 shrink-0 transition-opacity ${isActive ? "opacity-70" : "opacity-0 group-hover:opacity-40"}`}
                      />
                    </>
                  )}
                </>
              )}
            </NavLink>
          );
        })}
      </nav>

      {/* Collapse toggle (desktop) */}
      {!isMobileDrawer && (
        <button
          onClick={onToggleCollapsed}
          className="flex items-center gap-2 px-3.5 py-2.5 mx-3 mb-2 rounded-[var(--radius-md)] text-xs font-semibold text-white/55 hover:bg-white/[0.06] hover:text-white transition-colors shrink-0"
          title={collapsed ? "Expand sidebar" : "Collapse sidebar"}
        >
          {collapsed ? (
            <ChevronsRight className="h-4 w-4 mx-auto" />
          ) : (
            <>
              <ChevronsLeft className="h-4 w-4" />
              <span>Collapse</span>
            </>
          )}
        </button>
      )}

      {/* User footer */}
      <div
        className={`p-3 border-t border-white/10 shrink-0 ${!showExpanded ? "flex justify-center" : ""}`}
      >
        <div className="flex items-center gap-3">
          <div className="h-9 w-9 shrink-0 rounded-full bg-white/10 flex items-center justify-center text-sm font-bold text-white ring-2 ring-white/10">
            {(session?.user.full_name ?? "?").slice(0, 1).toUpperCase()}
          </div>
          {showExpanded && (
            <div className="min-w-0 flex-1">
              <div className="text-sm font-semibold text-white truncate">
                {session?.user.full_name ?? "—"}
              </div>
              <div className="text-[11px] text-white/55 truncate flex items-center gap-1.5">
                <span className="h-1 w-1 rounded-full bg-success shrink-0" />
                {primaryRole ? (ROLE_LABELS[primaryRole] ?? primaryRole) : ""}
              </div>
            </div>
          )}
        </div>
      </div>
    </aside>
  );
}
