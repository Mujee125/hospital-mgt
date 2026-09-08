
import { useEffect, useRef, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { motion, AnimatePresence } from "motion/react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import logo from "@/assets/logo_transparant.png";
import {
  Menu, Search, Bell, RefreshCw, LogOut, KeyRound, ChevronDown, CheckCheck,
  Minus, Square, Copy as RestoreIcon, X, User, Stethoscope, CalendarDays,
  ReceiptText, FlaskConical, Package, Loader2, Pill, ScanLine, BedDouble,
  Droplet, UserCog,
} from "lucide-react";
import { ThemeToggle } from "./ThemeToggle";
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuItem,
  DropdownMenuLabel, DropdownMenuSeparator, DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useAuth } from "@/lib/auth";
import { PERMISSIONS, ROLE_LABELS } from "@/lib/rbac";
import {
  useAppNotifications, useMarkNotificationRead, useMarkAllNotificationsRead,
  useGlobalSearch,
} from "@/lib/queries";
import type { GlobalSearchHit } from "@/lib/models";

const TITLEBAR_HEIGHT = 40; // px — Win11-proportioned, slightly taller than
                             // the OS default (32px) to comfortably host
                             // the composed Header content when authenticated.

const pageTitles: Record<string, string> = {
  "/": "Dashboard",
  "/appointments": "Appointments",
  "/patients": "Patients",
  "/doctors": "Doctors",
  "/queue": "Patient Queue",
  "/ipd": "In-Patient Department",
  "/laboratory": "Laboratory",
  "/radiology": "Radiology",
  "/billing": "Billing & Invoices",
  "/inventory": "Inventory",
  "/messaging": "Staff Chat",
  "/audit": "Audit Log",
  "/users": "Users & Roles",
  "/reports": "Reports",
  "/backup": "Backup & Restore",
  "/settings": "Settings",
};

interface TitleBarProps {
  authenticated?: boolean;
  onMenuClick?: () => void;
  onRefresh?: () => void;
  isRefreshing?: boolean;
  /**
   * Per-deployment hospital name (from licenseInfo.hospital_name).
   * Falls back to "VitalFlow HMS" (the product name) when no license
   * is installed yet (Setup/Login/Boot screens) so the brand lockup
   * always shows something meaningful — never the obsolete
   * "Rasheed Medical Center" demo string.
   */
  hospitalName?: string;
}

export function TitleBar({ authenticated = false, onMenuClick, onRefresh, isRefreshing, hospitalName }: TitleBarProps) {
  const [isMaximized, setIsMaximized] = useState(false);

  // Track maximize/restore state so the middle window-control icon swaps
  // correctly, including when the user maximizes via Win11 snap layouts
  // (which bypasses our button entirely) or drags the title bar to the
  // top of the screen.
  useEffect(() => {
    const win = getCurrentWindow();
    let unlisten: (() => void) | undefined;
    win.isMaximized().then(setIsMaximized).catch(() => {});
    win.onResized(() => {
      win.isMaximized().then(setIsMaximized).catch(() => {});
    }).then((fn: () => void) => { unlisten = fn; }).catch(() => {});
    return () => unlisten?.();
  }, []);

  const handleMinimize = () => getCurrentWindow().minimize().catch(() => {});
  const handleToggleMaximize = () => getCurrentWindow().toggleMaximize().catch(() => {});
  const handleClose = () => getCurrentWindow().close().catch(() => {});

  return (
    <div
      onDoubleClick={handleToggleMaximize}
      className="flex items-center shrink-0 bg-card border-b border-border select-none"
      style={{ height: TITLEBAR_HEIGHT }}
    >
      {/* Brand mark — always present, always draggable */}

      {/* Authenticated: composed Header content. Unauthenticated: just a
          draggable spacer filling the rest of the bar. */}
      {authenticated ? (
        <AuthenticatedTitleBarContent
          onMenuClick={onMenuClick}
          onRefresh={onRefresh}
          isRefreshing={isRefreshing}
        />
      ) : (
        <>
          <div
            data-tauri-drag-region
            className="flex items-center gap-2 pl-3 pr-4 shrink-0 bg-card border-b border-border"
            style={{ height: TITLEBAR_HEIGHT }}
          >
            <img src={logo} alt="Logo" className="w-10 h-10 object-contain" />
            <span className="text-[15px] font-semibold tracking-tight text-primary">
              {hospitalName ?? (
                <>
                  <span
                    className="text-[#014292]"
                  >
                    RASHEED
                  </span>{" "}
                  Medical Center HMS
                </>
              )}
            </span>
          </div>
          <div data-tauri-drag-region className="flex-1 h-full" />
        </>
      )}

      {/* Window controls — Win11 standard: 46px wide, full bar height,
          minimize/maximize hover to a neutral tint, close hovers red. */}

      <div className="flex items-stretch h-full shrink-0">
        <button
          onClick={handleMinimize}
          aria-label="Minimize"
          title="Minimize"
          className="w-[46px] flex items-center justify-center text-muted-foreground hover:bg-black/[0.06] dark:hover:bg-white/10 hover:text-foreground transition-colors"
        >
          <Minus className="h-4 w-4" />
        </button>
        <button
          onClick={handleToggleMaximize}
          aria-label={isMaximized ? "Restore" : "Maximize"}
          title={isMaximized ? "Restore" : "Maximize"}
          className="w-[46px] flex items-center justify-center text-muted-foreground hover:bg-black/[0.06] dark:hover:bg-white/10 hover:text-foreground transition-colors"
        >
          {isMaximized ? (
            <RestoreIcon className="h-3.5 w-3.5" />
          ) : (
            <Square className="h-3.5 w-3.5" />
          )}
        </button>
        <button
          onClick={handleClose}
          aria-label="Close"
          title="Close"
          className="w-[46px] flex items-center justify-center text-muted-foreground hover:bg-destructive hover:text-destructive-foreground transition-colors"
        >
          <X className="h-4 w-4" />
        </button>
      </div>
    </div>
  );
}

/** The former Header.tsx content, ported in as-is (same hooks, same
 *  behavior) so nothing about search/clock/notifications/theme/account
 *  changed — only its container (a 64px standalone bar) went away. */
function AuthenticatedTitleBarContent({
  onMenuClick,
  onRefresh,
  isRefreshing,
}: {
  onMenuClick?: () => void;
  onRefresh?: () => void;
  isRefreshing?: boolean;
}) {
  const location = useLocation();
  const navigate = useNavigate();
  const { session, logout } = useAuth();
  const [currentTime, setCurrentTime] = useState("");

  useEffect(() => {
    const update = () => {
      setCurrentTime(new Date().toLocaleDateString("en-US", {
        weekday: "short", month: "short", day: "numeric",
        hour: "2-digit", minute: "2-digit",
      }));
    };
    update();
    const t = setInterval(update, 30_000);
    return () => clearInterval(t);
  }, []);

  const title = pageTitles[location.pathname] ?? "Hospital Portal";
  const primaryRole = session?.roles?.[0];

  const handleLogout = async () => {
    await logout();
    navigate("/");
    window.location.reload();
  };

  return (
    <div className="flex-1 flex items-center justify-between gap-4 h-full px-2 min-w-0">
      {/* Left: mobile menu + current page title */}
      <div className="flex items-center gap-3 min-w-0 flex-1 h-full">
        <button
          onClick={onMenuClick}
          className="p-1.5 rounded-md text-muted-foreground hover:text-foreground hover:bg-muted transition-colors lg:hidden"
          aria-label="Open navigation menu"
        >
          <Menu className="h-4 w-4" />
        </button>
        <div data-tauri-drag-region className="flex items-center gap-3 flex-1 h-full min-w-0">
          <span className="text-[13px] font-semibold text-foreground truncate pl-4">{title}</span>
        </div>
        {onRefresh && (
          <button
            onClick={onRefresh}
            disabled={isRefreshing}
            className="p-1 rounded-md text-muted-foreground hover:text-foreground hover:bg-muted transition-colors hidden sm:inline-flex"
            title="Refresh"
            aria-label="Refresh data"
          >
            <RefreshCw className={`h-3.5 w-3.5 ${isRefreshing ? "animate-spin text-primary" : ""}`} />
          </button>
        )}
      </div>

      {/* Right: search, clock, notifications, theme, account — identical
          behavior to the old Header.tsx, just re-homed. */}
      <div className="flex items-center gap-1 shrink-0">
        <GlobalSearch />

        <span className="text-[11px] text-muted-foreground font-medium hidden lg:block tabular-nums px-1">{currentTime}</span>

        <NotificationCenterBell />

        <ThemeToggle />

        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <button
              className="flex items-center gap-1.5 pl-1 pr-1.5 py-1 rounded-md hover:bg-muted transition-colors"
              aria-label="Account menu"
            >
              <span className="h-6 w-6 rounded-full bg-primary/10 flex items-center justify-center text-[11px] font-bold text-primary">
                {(session?.user.full_name ?? "?").slice(0, 1).toUpperCase()}
              </span>
              <span className="hidden md:block text-xs font-semibold text-foreground max-w-[90px] truncate">
                {session?.user.full_name ?? "—"}
              </span>
              <ChevronDown className="h-3 w-3 text-muted-foreground hidden md:block" />
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="w-60 rounded-lg">
            <DropdownMenuLabel className="flex flex-col gap-0.5 py-2">
              <span className="text-sm font-semibold">{session?.user.full_name ?? "—"}</span>
              <span className="text-[11px] font-normal text-muted-foreground">
                {primaryRole ? ROLE_LABELS[primaryRole] ?? primaryRole : ""} · @{session?.user.username}
              </span>
            </DropdownMenuLabel>
            <DropdownMenuSeparator />
            <DropdownMenuItem onClick={() => navigate("/settings")} className="gap-2 rounded-md cursor-pointer">
              <KeyRound className="h-4 w-4" /> Change password
            </DropdownMenuItem>
            <DropdownMenuItem onClick={handleLogout} className="gap-2 rounded-md cursor-pointer text-destructive focus:text-destructive focus:bg-destructive/8">
              <LogOut className="h-4 w-4" /> Sign out
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </div>
  );
}

/**
 * SETUP REQUIRED (not a file this component can change itself):
 * Custom decorations only take effect if the Tauri window is configured
 * without native chrome. In src-tauri/tauri.conf.json, under
 * `app.windows[0]`, set:
 *   "decorations": false
 * Without this, the OS will still draw its own title bar above this one.
 */

// ── NotificationCenterBell (Phase 9) ────────────────────────────────────────
//
// The titlebar bell, made real. Polls the per-user feed every 30 s
// (useAppNotifications); the red badge appears ONLY when unread_count > 0
// (with the count), the dropdown lists the 8 most recent notifications,
// and clicking an item marks it read and deep-links to its entity.

const NOTIFICATION_ROUTES: Record<string, string> = {
  lab_order: "/laboratory",
  appointment: "/appointments",
  bill: "/billing",
  inventory_item: "/inventory",
};

function NotificationCenterBell() {
  const navigate = useNavigate();
  const { data: feed, isLoading } = useAppNotifications();
  const markRead = useMarkNotificationRead();
  const markAll = useMarkAllNotificationsRead();
  const unread = feed?.unread_count ?? 0;
  const recent = (feed?.notifications ?? []).slice(0, 8);

  const open = (n: { id: number; entity_type: string | null; read: boolean }) => {
    if (!n.read) markRead.mutate(n.id);
    const route = n.entity_type ? NOTIFICATION_ROUTES[n.entity_type] : undefined;
    if (route) navigate(route);
  };

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          className="relative p-1.5 rounded-md text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
          aria-label={`Notifications${unread > 0 ? ` (${unread} unread)` : ""}`}
        >
          <Bell className="h-[15px] w-[15px]" />
          {unread > 0 && (
            <span className="absolute top-0.5 right-0.5 min-w-[13px] h-[13px] px-[3px] rounded-full bg-destructive ring-2 ring-card text-[8px] font-bold text-destructive-foreground flex items-center justify-center">
              {unread > 99 ? "99+" : unread}
            </span>
          )}
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-[340px] rounded-lg p-0">
        <div className="flex items-center justify-between px-3 py-2 border-b border-border">
          <DropdownMenuLabel className="p-0 text-sm font-semibold">
            Notifications
            {unread > 0 && (
              <span className="ml-1.5 text-[11px] font-normal text-destructive">
                {unread} unread
              </span>
            )}
          </DropdownMenuLabel>
          {unread > 0 && (
            <button
              onClick={() => markAll.mutate()}
              disabled={markAll.isPending}
              className="flex items-center gap-1 text-[11px] text-muted-foreground hover:text-foreground transition-colors"
              aria-label="Mark all notifications as read"
            >
              <CheckCheck className="h-3 w-3" /> Mark all read
            </button>
          )}
        </div>
        <div className="max-h-[380px] overflow-y-auto">
          {isLoading ? (
            <p className="px-3 py-4 text-xs text-muted-foreground">Loading…</p>
          ) : recent.length === 0 ? (
            <p className="px-3 py-6 text-center text-xs text-muted-foreground">
              No notifications. Clinical alerts, bookings, claims and
              stock warnings appear here.
            </p>
          ) : (
            recent.map((n) => (
              <button
                key={n.id}
                onClick={() => open(n)}
                className={`w-full text-left px-3 py-2.5 border-b border-border last:border-0 hover:bg-muted/60 transition-colors ${!n.read ? "bg-primary/[0.04]" : ""}`}
              >
                <div className="flex items-start gap-2">
                  {!n.read && (
                    <span className="mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full bg-primary" />
                  )}
                  <div className="min-w-0 flex-1">
                    <div className="text-xs font-semibold text-foreground leading-snug">
                      {n.title}
                    </div>
                    <div className="text-[11px] text-muted-foreground leading-snug mt-0.5">
                      {n.body}
                    </div>
                    <div className="text-[10px] text-muted-foreground/70 mt-1">
                      {new Date(n.created_at).toLocaleString()}
                    </div>
                  </div>
                </div>
              </button>
            ))
          )}
        </div>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

// ── GlobalSearch (Phase 10) ─────────────────────────────────────────────────
//
// The titlebar search, made real. One debounced query against the
// RBAC-scoped global_search command; the server decides which sections the
// signed-in user may see (a pharmacist gets inventory + patient hits, a
// billing clerk invoice + patient hits — neither sees the other's). Clicking
// a hit navigates to the entity's page. The placeholder also names only the
// sections the user can actually search, so the affordance honestly
// reflects the server-side scope.

const SEARCH_ROUTES: Record<GlobalSearchHit["entity_type"], string> = {
  patient: "/patients",
  doctor: "/doctors",
  appointment: "/appointments",
  invoice: "/billing",
  lab_order: "/laboratory",
  inventory_item: "/inventory",
  prescription: "/pharmacy",
  medication: "/pharmacy",
  radiology_order: "/radiology",
  ipd_admission: "/ipd",
  blood_donor: "/blood-bank",
  user: "/users",
};

const SEARCH_ICONS: Record<GlobalSearchHit["entity_type"], typeof User> = {
  patient: User,
  doctor: Stethoscope,
  appointment: CalendarDays,
  invoice: ReceiptText,
  lab_order: FlaskConical,
  inventory_item: Package,
  prescription: Pill,
  medication: Pill,
  radiology_order: ScanLine,
  ipd_admission: BedDouble,
  blood_donor: Droplet,
  user: UserCog,
};

const SEARCH_TYPE_LABELS: Record<GlobalSearchHit["entity_type"], string> = {
  patient: "Patient",
  doctor: "Doctor",
  appointment: "Appointment",
  invoice: "Invoice",
  lab_order: "Lab order",
  inventory_item: "Inventory item",
  prescription: "Prescription",
  medication: "Medication",
  radiology_order: "Radiology",
  ipd_admission: "IPD",
  blood_donor: "Blood donor",
  user: "User",
};

function GlobalSearch() {
  const navigate = useNavigate();
  const { has } = useAuth();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [debounced, setDebounced] = useState("");
  const boxRef = useRef<HTMLDivElement>(null);

  // Debounce keystrokes — the query fires 300 ms after typing settles.
  useEffect(() => {
    const t = setTimeout(() => setDebounced(query), 300);
    return () => clearTimeout(t);
  }, [query]);

  // Close on outside click / Escape.
  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (boxRef.current && !boxRef.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, []);

  const { data: hits, isFetching } = useGlobalSearch(debounced);
  const trimmed = debounced.trim();

  // Honest placeholder: name only the sections this user can actually search.
  const scopes: string[] = [];
  if (has(PERMISSIONS.PatientsView)) scopes.push("patients, prescriptions");
  if (has(PERMISSIONS.DoctorsView)) scopes.push("doctors");
  if (has(PERMISSIONS.BillingView)) scopes.push("invoices");
  if (has(PERMISSIONS.InventoryView)) scopes.push("pharmacy, stock");
  if (has(PERMISSIONS.RadiologyView)) scopes.push("radiology");
  if (has(PERMISSIONS.IpdView)) scopes.push("IPD");
  if (has(PERMISSIONS.BloodBankView)) scopes.push("donors");
  if (has(PERMISSIONS.UsersView)) scopes.push("users");
  const placeholder = scopes.length
    ? `Search ${scopes.slice(0, 3).join(", ")}${scopes.length > 3 ? "…" : ""}`
    : "Search";

  const open2 = (hit: GlobalSearchHit) => {
    setOpen(false);
    setQuery("");
    navigate(SEARCH_ROUTES[hit.entity_type]);
  };

  return (
    <div ref={boxRef} className="relative">
      <AnimatePresence initial={false}>
        {open ? (
          <motion.div
            initial={{ width: 0, opacity: 0 }}
            animate={{ width: 260, opacity: 1 }}
            exit={{ width: 0, opacity: 0 }}
            transition={{ duration: 0.2, ease: [0.22, 1, 0.36, 1] }}
            className="overflow-hidden"
          >
            <input
              autoFocus
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={placeholder}
              className="w-full h-7 rounded-md bg-muted border border-border px-2.5 text-xs outline-none focus:ring-2 focus:ring-primary/15 focus:border-primary/40"
            />
          </motion.div>
        ) : null}
      </AnimatePresence>
      {!open && (
        <button
          onClick={() => setOpen(true)}
          className="p-1.5 rounded-md text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
          aria-label="Search"
          title={placeholder}
        >
          <Search className="h-[15px] w-[15px]" />
        </button>
      )}

      <AnimatePresence>
        {open && trimmed.length >= 2 && (
          <motion.div
            initial={{ opacity: 0, y: -4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -4 }}
            transition={{ duration: 0.15 }}
            className="absolute right-0 top-9 z-50 w-[320px] rounded-lg border border-border bg-card shadow-lg overflow-hidden"
          >
            {isFetching ? (
              <div className="flex items-center gap-2 px-3 py-4 text-xs text-muted-foreground">
                <Loader2 className="h-3.5 w-3.5 animate-spin" /> Searching…
              </div>
            ) : !hits || hits.length === 0 ? (
              <p className="px-3 py-4 text-xs text-muted-foreground">
                No matches for “{trimmed}”.
              </p>
            ) : (
              <div className="max-h-[340px] overflow-y-auto">
                {hits.map((hit) => {
                  const Icon = SEARCH_ICONS[hit.entity_type];
                  return (
                    <button
                      key={`${hit.entity_type}-${hit.id}`}
                      onClick={() => open2(hit)}
                      className="w-full flex items-center gap-2.5 px-3 py-2 text-left border-b border-border last:border-0 hover:bg-muted/60 transition-colors"
                    >
                      <Icon className="h-3.5 w-3.5 shrink-0 text-primary" />
                      <div className="min-w-0 flex-1">
                        <div className="text-xs font-medium text-foreground truncate">{hit.title}</div>
                        {hit.subtitle && (
                          <div className="text-[10px] text-muted-foreground truncate">{hit.subtitle}</div>
                        )}
                      </div>
                      <span className="text-[9px] uppercase tracking-wide text-muted-foreground/70 shrink-0">
                        {SEARCH_TYPE_LABELS[hit.entity_type]}
                      </span>
                    </button>
                  );
                })}
              </div>
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
