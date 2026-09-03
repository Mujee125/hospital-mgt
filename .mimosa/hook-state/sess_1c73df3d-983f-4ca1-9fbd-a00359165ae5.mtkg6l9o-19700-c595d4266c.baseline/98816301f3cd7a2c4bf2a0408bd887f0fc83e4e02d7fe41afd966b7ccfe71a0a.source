/**
 * @deprecated As of the Windows 11 title-bar redesign, this component is
 * no longer mounted by AppShell. Its search/clock/notifications/theme/
 * account-menu content was ported into TitleBar.tsx's
 * `AuthenticatedTitleBarContent`, so the whole window has a single top
 * bar instead of a native-OS bar + this 64px standalone one stacked
 * beneath it. This file is kept only for reference — it is safe to
 * delete once you've confirmed TitleBar.tsx covers everything you need.
 */
import { useEffect, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { motion, AnimatePresence } from "motion/react";
import { ThemeToggle } from "./ThemeToggle";
import { Menu, Search, Bell, RefreshCw, LogOut, KeyRound, ChevronDown } from "lucide-react";
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuItem,
  DropdownMenuLabel, DropdownMenuSeparator, DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useAuth } from "@/lib/auth";
import { ROLE_LABELS } from "@/lib/rbac";

interface HeaderProps {
  onMenuClick: () => void;
  onRefresh?: () => void;
  isRefreshing?: boolean;
}

const titles: Record<string, string> = {
  "/": "Dashboard",
  "/appointments": "Appointments",
  "/patients": "Patients",
  "/doctors": "Doctors",
  "/queue": "Patient Queue",
  "/ipd": "In-Patient Department",
  "/laboratory": "Laboratory",
  "/billing": "Billing & Invoices",
  "/messaging": "Staff Chat",
  "/audit": "Audit Log",
  "/users": "Users & Roles",
  "/settings": "Settings",
};

export function Header({ onMenuClick, onRefresh, isRefreshing }: HeaderProps) {
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

  const title = titles[location.pathname] ?? "Hospital Portal";
  const primaryRole = session?.roles?.[0];

  const handleLogout = async () => {
    await logout();
    navigate("/");
    window.location.reload();
  };

  return (
    <header className="h-[var(--header-height)] shrink-0 border-b border-border bg-card/80 backdrop-blur-xl px-4 md:px-6 flex items-center justify-between gap-4 select-none">
      {/* Left: title */}
      <div className="flex items-center gap-3 min-w-0">
        <button
          onClick={onMenuClick}
          className="p-2 -ml-2 rounded-lg text-muted-foreground hover:text-foreground hover:bg-muted transition-colors lg:hidden"
          aria-label="Open navigation menu"
        >
          <Menu className="h-5 w-5" />
        </button>
        <h2 className="text-display-md text-foreground truncate">{title}</h2>
        {onRefresh && (
          <button
            onClick={onRefresh}
            disabled={isRefreshing}
            className="p-1.5 -ml-1 rounded-lg text-muted-foreground hover:text-foreground hover:bg-muted transition-colors hidden sm:inline-flex"
            title="Refresh"
            aria-label="Refresh data"
          >
            <RefreshCw className={`h-4 w-4 ${isRefreshing ? "animate-spin text-primary" : ""}`} />
          </button>
        )}
      </div>

      {/* Right: actions */}
      <div className="flex items-center gap-1.5 md:gap-2">
        {/* Search */}
        <div className="relative">
          <AnimatePresence initial={false}>
            {searchOpen ? (
              <motion.div
                initial={{ width: 0, opacity: 0 }}
                animate={{ width: 240, opacity: 1 }}
                exit={{ width: 0, opacity: 0 }}
                transition={{ duration: 0.2, ease: [0.22, 1, 0.36, 1] }}
                className="overflow-hidden"
              >
                <input
                  autoFocus
                  type="text"
                  placeholder="Search patients, doctors…"
                  onBlur={() => setSearchOpen(false)}
                  className="w-full h-9 rounded-lg bg-muted border border-border px-3.5 text-sm outline-none focus:ring-2 focus:ring-primary/15 focus:border-primary/40"
                />
              </motion.div>
            ) : null}
          </AnimatePresence>
          {!searchOpen && (
            <button
              onClick={() => setSearchOpen(true)}
              className="p-2 rounded-lg text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
              aria-label="Search"
            >
              <Search className="h-[18px] w-[18px]" />
            </button>
          )}
        </div>

        <span className="text-xs text-muted-foreground font-medium hidden lg:block tabular-nums">{currentTime}</span>

        <div className="h-5 w-px bg-border hidden lg:block" />

        <button
          className="relative p-2 rounded-lg text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
          aria-label="Notifications"
        >
          <Bell className="h-[18px] w-[18px]" />
          <span className="absolute top-1.5 right-1.5 h-2 w-2 rounded-full bg-destructive ring-2 ring-card" />
        </button>

        <ThemeToggle />

        {/* User menu */}
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <button
              className="flex items-center gap-2 pl-1.5 pr-2 py-1 rounded-lg hover:bg-muted transition-colors"
              aria-label="Account menu"
            >
              <span className="h-7 w-7 rounded-full bg-primary/10 flex items-center justify-center text-xs font-bold text-primary">
                {(session?.user.full_name ?? "?").slice(0, 1).toUpperCase()}
              </span>
              <span className="hidden md:block text-xs font-semibold text-foreground max-w-[100px] truncate">
                {session?.user.full_name ?? "—"}
              </span>
              <ChevronDown className="h-3.5 w-3.5 text-muted-foreground hidden md:block" />
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
    </header>
  );
}
