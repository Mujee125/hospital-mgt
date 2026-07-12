// /**
//  * TitleBar — custom Windows 11-style window chrome, replacing the native
//  * OS title bar (requires `"decorations": false` in tauri.conf.json — see
//  * the setup note at the bottom of this file).
//  *
//  * Two layouts, per the RCTF spec:
//  *  - Unauthenticated (`authenticated={false}`): logo + app name + drag
//  *    region + window controls only. Used for Setup/Login/Boot/Error
//  *    screens and anywhere before a session exists.
//  *  - Authenticated (`authenticated={true}`): the same window controls,
//  *    but the space between the brand mark and the controls now hosts
//  *    the full utility cluster that used to live in the standalone
//  *    Header.tsx (page title, search, clock, notifications, theme
//  *    toggle, account menu) — Header.tsx itself is no longer mounted by
//  *    AppShell; its functionality now lives here so the whole window has
//  *    exactly one top bar instead of two stacked ones.
//  *
//  * IMPORTANT: everything except the three window-control buttons and the
//  * page title/search/clock/bell/theme/account cluster must carry
//  * `data-tauri-drag-region` so the window can still be dragged/double-
//  * click-maximized from any empty space in the bar — interactive
//  * children (buttons, inputs, the dropdown trigger) intentionally don't
//  * get that attribute so they keep receiving normal clicks.
//  */
// import { useEffect, useState } from "react";
// import { useLocation, useNavigate } from "react-router-dom";
// import { motion, AnimatePresence } from "motion/react";
// import { getCurrentWindow } from "@tauri-apps/api/window";
// import logo from "@/assets/logo_transparant.png";
// import {
//   Menu, Search, Bell, RefreshCw, LogOut, KeyRound, ChevronDown,
//   Minus, Square, Copy as RestoreIcon, X,
// } from "lucide-react";
// import { ThemeToggle } from "./ThemeToggle";
// import {
//   DropdownMenu, DropdownMenuContent, DropdownMenuItem,
//   DropdownMenuLabel, DropdownMenuSeparator, DropdownMenuTrigger,
// } from "@/components/ui/dropdown-menu";
// import { useAuth } from "@/lib/auth";
// import { ROLE_LABELS } from "@/lib/rbac";

// const TITLEBAR_HEIGHT = 40; // px — Win11-proportioned, slightly taller than
//                              // the OS default (32px) to comfortably host
//                              // the composed Header content when authenticated.

// const pageTitles: Record<string, string> = {
//   "/": "Dashboard",
//   "/appointments": "Appointments",
//   "/patients": "Patients",
//   "/doctors": "Doctors",
//   "/queue": "Patient Queue",
//   "/ipd": "In-Patient Department",
//   "/laboratory": "Laboratory",
//   "/billing": "Billing & Invoices",
//   "/messaging": "Staff Chat",
//   "/audit": "Audit Log",
//   "/users": "Users & Roles",
//   "/settings": "Settings",
// };

// interface TitleBarProps {
//   authenticated?: boolean;
//   onMenuClick?: () => void;
//   onRefresh?: () => void;
//   isRefreshing?: boolean;
// }

// export function TitleBar({ authenticated = false, onMenuClick, onRefresh, isRefreshing }: TitleBarProps) {
//   const [isMaximized, setIsMaximized] = useState(false);

//   // Track maximize/restore state so the middle window-control icon swaps
//   // correctly, including when the user maximizes via Win11 snap layouts
//   // (which bypasses our button entirely) or drags the title bar to the
//   // top of the screen.
//   useEffect(() => {
//     const win = getCurrentWindow();
//     let unlisten: (() => void) | undefined;
//     win.isMaximized().then(setIsMaximized).catch(() => {});
//     win.onResized(() => {
//       win.isMaximized().then(setIsMaximized).catch(() => {});
//     }).then((fn: () => void) => { unlisten = fn; }).catch(() => {});
//     return () => unlisten?.();
//   }, []);

//   const handleMinimize = () => getCurrentWindow().minimize().catch(() => {});
//   const handleToggleMaximize = () => getCurrentWindow().toggleMaximize().catch(() => {});
//   const handleClose = () => getCurrentWindow().close().catch(() => {});

//   return (
//     <div
//       data-tauri-drag-region
//       onDoubleClick={handleToggleMaximize}
//       className="flex items-center shrink-0 bg-card border-b  select-none"
//       style={{ height: TITLEBAR_HEIGHT }}
//     >
//       {/* Brand mark — always present, always draggable */}

//       {/* Authenticated: composed Header content. Unauthenticated: just a
//           draggable spacer filling the rest of the bar. */}
//       {authenticated ? (
//         <AuthenticatedTitleBarContent
//           onMenuClick={onMenuClick}
//           onRefresh={onRefresh}
//           isRefreshing={isRefreshing}
//         />
//       ) : (
//         <>
//           <div
//             data-tauri-drag-region
//             className="flex items-center gap-2 pl-3 pr-4 shrink-0 bg-card border-b "
//             style={{ height: TITLEBAR_HEIGHT }}
//           >
//             <img src={logo} alt="Logo" className="w-8 h-8 object-contain " />
//             <span className="text-[13px] font-semibold  tracking-tight text-[#004291]">
//               Rasheed
//             </span>
//             <span className="text-[#027e6c]">Medical Center</span>
//           </div>
//           <div data-tauri-drag-region className="flex-1 h-full" />
//         </>
//       )}

//       {/* Window controls — Win11 standard: 46px wide, full bar height,
//           minimize/maximize hover to a neutral tint, close hovers red. */}

//       <div className="flex items-stretch h-full shrink-0">
//         <button
//           onClick={handleMinimize}
//           aria-label="Minimize"
//           title="Minimize"
//           className="w-[46px] flex items-center justify-center text-muted-foreground hover:bg-muted transition-colors"
//         >
//           <Minus className="h-4 w-4" />
//         </button>
//         <button
//           onClick={handleToggleMaximize}
//           aria-label={isMaximized ? "Restore" : "Maximize"}
//           title={isMaximized ? "Restore" : "Maximize"}
//           className="w-[46px] flex items-center justify-center text-muted-foreground hover:bg-muted transition-colors"
//         >
//           {isMaximized ? (
//             <RestoreIcon className="h-3.5 w-3.5" />
//           ) : (
//             <Square className="h-3.5 w-3.5" />
//           )}
//         </button>
//         <button
//           onClick={handleClose}
//           aria-label="Close"
//           title="Close"
//           className="w-[46px] flex items-center justify-center text-muted-foreground hover:bg-destructive hover:text-destructive-foreground transition-colors"
//         >
//           <X className="h-4 w-4" />
//         </button>
//       </div>
//     </div>
//   );
// }

// /** The former Header.tsx content, ported in as-is (same hooks, same
//  *  behavior) so nothing about search/clock/notifications/theme/account
//  *  changed — only its container (a 64px standalone bar) went away. */
// function AuthenticatedTitleBarContent({
//   onMenuClick,
//   onRefresh,
//   isRefreshing,
// }: {
//   onMenuClick?: () => void;
//   onRefresh?: () => void;
//   isRefreshing?: boolean;
// }) {
//   const location = useLocation();
//   const navigate = useNavigate();
//   const { session, logout } = useAuth();
//   const [currentTime, setCurrentTime] = useState("");
//   const [searchOpen, setSearchOpen] = useState(false);

//   useEffect(() => {
//     const update = () => {
//       setCurrentTime(new Date().toLocaleDateString("en-US", {
//         weekday: "short", month: "short", day: "numeric",
//         hour: "2-digit", minute: "2-digit",
//       }));
//     };
//     update();
//     const t = setInterval(update, 30_000);
//     return () => clearInterval(t);
//   }, []);

//   const title = pageTitles[location.pathname] ?? "Hospital Portal";
//   const primaryRole = session?.roles?.[0];

//   const handleLogout = async () => {
//     await logout();
//     navigate("/");
//     window.location.reload();
//   };

//   return (
//     <div className="flex-1 flex items-center justify-between gap-4 h-full px-2 min-w-0">
//       {/* Left: mobile menu + current page title */}
//       <div data-tauri-drag-region className="flex items-center gap-3 min-w-0 flex-1 h-full">
//         <button
//           onClick={onMenuClick}
//           className="p-1.5 rounded-md text-muted-foreground hover:text-foreground hover:bg-muted transition-colors lg:hidden"
//           aria-label="Open navigation menu"
//         >
//           <Menu className="h-4 w-4" />
//         </button>
//         <span className="text-[13px] font-semibold text-foreground truncate pl-4">{title}</span>
//         {onRefresh && (
//           <button
//             onClick={onRefresh}
//             disabled={isRefreshing}
//             className="p-1 rounded-md text-muted-foreground hover:text-foreground hover:bg-muted transition-colors hidden sm:inline-flex"
//             title="Refresh"
//             aria-label="Refresh data"
//           >
//             <RefreshCw className={`h-3.5 w-3.5 ${isRefreshing ? "animate-spin text-primary" : ""}`} />
//           </button>
//         )}
//       </div>

//       {/* Right: search, clock, notifications, theme, account — identical
//           behavior to the old Header.tsx, just re-homed. */}
//       <div className="flex items-center gap-1 shrink-0">
//         <div className="relative">
//           <AnimatePresence initial={false}>
//             {searchOpen ? (
//               <motion.div
//                 initial={{ width: 0, opacity: 0 }}
//                 animate={{ width: 200, opacity: 1 }}
//                 exit={{ width: 0, opacity: 0 }}
//                 transition={{ duration: 0.2, ease: [0.22, 1, 0.36, 1] }}
//                 className="overflow-hidden"
//               >
//                 <input
//                   autoFocus
//                   type="text"
//                   placeholder="Search patients, doctors…"
//                   onBlur={() => setSearchOpen(false)}
//                   className="w-full h-7 rounded-md bg-muted border border-border px-2.5 text-xs outline-none focus:ring-2 focus:ring-primary/15 focus:border-primary/40"
//                 />
//               </motion.div>
//             ) : null}
//           </AnimatePresence>
//           {!searchOpen && (
//             <button
//               onClick={() => setSearchOpen(true)}
//               className="p-1.5 rounded-md text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
//               aria-label="Search"
//             >
//               <Search className="h-[15px] w-[15px]" />
//             </button>
//           )}
//         </div>

//         <span className="text-[11px] text-muted-foreground font-medium hidden lg:block tabular-nums px-1">{currentTime}</span>

//         <button
//           className="relative p-1.5 rounded-md text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
//           aria-label="Notifications"
//         >
//           <Bell className="h-[15px] w-[15px]" />
//           <span className="absolute top-1 right-1 h-1.5 w-1.5 rounded-full bg-destructive ring-2 ring-card" />
//         </button>

//         <ThemeToggle />

//         <DropdownMenu>
//           <DropdownMenuTrigger asChild>
//             <button
//               className="flex items-center gap-1.5 pl-1 pr-1.5 py-1 rounded-md hover:bg-muted transition-colors"
//               aria-label="Account menu"
//             >
//               <span className="h-6 w-6 rounded-full bg-primary/10 flex items-center justify-center text-[11px] font-bold text-primary">
//                 {(session?.user.full_name ?? "?").slice(0, 1).toUpperCase()}
//               </span>
//               <span className="hidden md:block text-xs font-semibold text-foreground max-w-[90px] truncate">
//                 {session?.user.full_name ?? "—"}
//               </span>
//               <ChevronDown className="h-3 w-3 text-muted-foreground hidden md:block" />
//             </button>
//           </DropdownMenuTrigger>
//           <DropdownMenuContent align="end" className="w-60 rounded-lg">
//             <DropdownMenuLabel className="flex flex-col gap-0.5 py-2">
//               <span className="text-sm font-semibold">{session?.user.full_name ?? "—"}</span>
//               <span className="text-[11px] font-normal text-muted-foreground">
//                 {primaryRole ? ROLE_LABELS[primaryRole] ?? primaryRole : ""} · @{session?.user.username}
//               </span>
//             </DropdownMenuLabel>
//             <DropdownMenuSeparator />
//             <DropdownMenuItem onClick={() => navigate("/settings")} className="gap-2 rounded-md cursor-pointer">
//               <KeyRound className="h-4 w-4" /> Change password
//             </DropdownMenuItem>
//             <DropdownMenuItem onClick={handleLogout} className="gap-2 rounded-md cursor-pointer text-destructive focus:text-destructive focus:bg-destructive/8">
//               <LogOut className="h-4 w-4" /> Sign out
//             </DropdownMenuItem>
//           </DropdownMenuContent>
//         </DropdownMenu>
//       </div>
//     </div>
//   );
// }

// /**
//  * SETUP REQUIRED (not a file this component can change itself):
//  * Custom decorations only take effect if the Tauri window is configured
//  * without native chrome. In src-tauri/tauri.conf.json, under
//  * `app.windows[0]`, set:
//  *   "decorations": false
//  * Without this, the OS will still draw its own title bar above this one.
//  */
/**
 * TitleBar — custom Windows 11-style window chrome, replacing the native
 * OS title bar (requires `"decorations": false` in tauri.conf.json — see
 * the setup note at the bottom of this file).
 *
 * Two layouts, per the RCTF spec:
 *  - Unauthenticated (`authenticated={false}`): logo + app name + drag
 *    region + window controls only. Used for Setup/Login/Boot/Error
 *    screens and anywhere before a session exists.
 *  - Authenticated (`authenticated={true}`): the same window controls,
 *    but the space between the brand mark and the controls now hosts
 *    the full utility cluster that used to live in the standalone
 *    Header.tsx (page title, search, clock, notifications, theme
 *    toggle, account menu) — Header.tsx itself is no longer mounted by
 *    AppShell; its functionality now lives here so the whole window has
 *    exactly one top bar instead of two stacked ones.
 *
 * IMPORTANT: everything except the three window-control buttons and the
 * page title/search/clock/bell/theme/account cluster must carry
 * `data-tauri-drag-region` so the window can still be dragged/double-
 * click-maximized from any empty space in the bar — interactive
 * children (buttons, inputs, the dropdown trigger) intentionally don't
 * get that attribute so they keep receiving normal clicks.
 */
import { useEffect, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { motion, AnimatePresence } from "motion/react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import logo from "@/assets/logo.png";
import {
  Menu, Search, Bell, RefreshCw, LogOut, KeyRound, ChevronDown,
  Minus, Square, Copy as RestoreIcon, X,
} from "lucide-react";
import { ThemeToggle } from "./ThemeToggle";
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuItem,
  DropdownMenuLabel, DropdownMenuSeparator, DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useAuth } from "@/lib/auth";
import { ROLE_LABELS } from "@/lib/rbac";

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
            <img src={logo} alt="Logo" className="w-8 h-8 object-contain" />
            <span className="text-[13px] font-semibold tracking-tight text-primary">
              {hospitalName ?? "VitalFlow HMS"}
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
  const [searchOpen, setSearchOpen] = useState(false);

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
        <div className="relative">
          <AnimatePresence initial={false}>
            {searchOpen ? (
              <motion.div
                initial={{ width: 0, opacity: 0 }}
                animate={{ width: 200, opacity: 1 }}
                exit={{ width: 0, opacity: 0 }}
                transition={{ duration: 0.2, ease: [0.22, 1, 0.36, 1] }}
                className="overflow-hidden"
              >
                <input
                  autoFocus
                  type="text"
                  placeholder="Search patients, doctors…"
                  onBlur={() => setSearchOpen(false)}
                  className="w-full h-7 rounded-md bg-muted border border-border px-2.5 text-xs outline-none focus:ring-2 focus:ring-primary/15 focus:border-primary/40"
                />
              </motion.div>
            ) : null}
          </AnimatePresence>
          {!searchOpen && (
            <button
              onClick={() => setSearchOpen(true)}
              className="p-1.5 rounded-md text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
              aria-label="Search"
            >
              <Search className="h-[15px] w-[15px]" />
            </button>
          )}
        </div>

        <span className="text-[11px] text-muted-foreground font-medium hidden lg:block tabular-nums px-1">{currentTime}</span>

        <button
          className="relative p-1.5 rounded-md text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
          aria-label="Notifications"
        >
          <Bell className="h-[15px] w-[15px]" />
          <span className="absolute top-1 right-1 h-1.5 w-1.5 rounded-full bg-destructive ring-2 ring-card" />
        </button>

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
