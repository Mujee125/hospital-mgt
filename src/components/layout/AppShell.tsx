import { useState, useEffect, type ReactNode } from "react";
import { useLocation } from "react-router-dom";
import { motion, AnimatePresence } from "motion/react";
import { Sidebar } from "./Sidebar";
import { TitleBar } from "./TitleBar";
import type { AppConfig } from "@/App";

const COLLAPSE_STORAGE_KEY = "hms-sidebar-collapsed";

interface AppShellProps {
  children: ReactNode;
  config: AppConfig | null;
  serverMode: boolean;
  serverIp: string;
  /**
   * Per-deployment hospital name (from licenseInfo.hospital_name).
   * Forwarded to TitleBar + Sidebar so the brand lockup reflects
   * the licensed hospital rather than a hard-coded demo name.
   */
  hospitalName?: string;
}

export function AppShell({ children, serverMode, hospitalName }: AppShellProps) {
  const [collapsed, setCollapsed] = useState<boolean>(() => {
    try {
      return localStorage.getItem(COLLAPSE_STORAGE_KEY) === "1";
    } catch {
      return false;
    }
  });
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const location = useLocation();

  // Close the mobile drawer automatically on every navigation.
  useEffect(() => {
    setDrawerOpen(false);
  }, [location.pathname]);

  const toggleCollapsed = () => {
    setCollapsed((prev) => {
      const next = !prev;
      try {
        localStorage.setItem(COLLAPSE_STORAGE_KEY, next ? "1" : "0");
      } catch {
        /* non-fatal — collapse state just won't persist this session */
      }
      return next;
    });
  };

  const handleRefresh = () => {
    setIsRefreshing(true);
    // Each page subscribes to its own React Query cache; a global
    // refresh just nudges the most recently focused query set to
    // revalidate. Concretely, this dispatches a custom event pages can
    // listen for, OR — more simply — pages already revalidate on window
    // focus per the shared QueryClient default, so this button mainly
    // exists for "I want it to happen right now" reassurance.
    window.dispatchEvent(new CustomEvent("hms:refresh"));
    setTimeout(() => setIsRefreshing(false), 600);
  };

  return (
    <div className="flex h-full w-full bg-background text-foreground overflow-hidden">
      {/* Desktop sidebar */}
      <div className="hidden lg:block h-full shrink-0">
        <Sidebar
          serverMode={serverMode}
          collapsed={collapsed}
          onToggleCollapsed={toggleCollapsed}
          hospitalName={hospitalName}
        />
      </div>

      {/* Mobile drawer */}
      <AnimatePresence>
        {drawerOpen && (
          <>
            <motion.div
              className="fixed inset-0 z-40 bg-black/50 lg:hidden"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              onClick={() => setDrawerOpen(false)}
            />
            <motion.div
              className="fixed inset-y-0 left-0 z-50 w-[280px] lg:hidden"
              initial={{ x: "-100%" }}
              animate={{ x: 0 }}
              exit={{ x: "-100%" }}
              transition={{ type: "spring", stiffness: 380, damping: 35 }}
              drag="x"
              dragConstraints={{ left: 0, right: 0 }}
              dragElastic={{ left: 0.15, right: 0 }}
              onDragEnd={(_, info) => {
                if (info.offset.x < -80) setDrawerOpen(false);
              }}
            >
              <Sidebar
                serverMode={serverMode}
                collapsed={false}
                onToggleCollapsed={() => {}}
                isMobileDrawer
                onNavigate={() => setDrawerOpen(false)}
                hospitalName={hospitalName}
              />
            </motion.div>
          </>
        )}
      </AnimatePresence>

      <div className="flex-1 flex flex-col h-full overflow-hidden">
        <TitleBar
          authenticated
          onMenuClick={() => setDrawerOpen(true)}
          onRefresh={handleRefresh}
          isRefreshing={isRefreshing}
          hospitalName={hospitalName}
        />
        <main className="flex-1 overflow-y-auto bg-background">
          <div className="w-full page-shell">
            {children}
          </div>
        </main>
      </div>
    </div>
  );
}
