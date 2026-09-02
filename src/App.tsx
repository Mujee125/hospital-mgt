import { useState, useEffect, lazy, Suspense, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Routes, Route, Navigate, useNavigate } from "react-router-dom";
import { AnimatePresence, motion } from "motion/react";
import { AppShell } from "@/components/layout/AppShell";
import { TitleBar } from "@/components/layout/TitleBar";
import { Dashboard } from "@/pages/Dashboard";
import { Appointments } from "@/pages/Appointments";
import { Patients } from "@/pages/Patients";
import { Doctors } from "@/pages/Doctors";
import { Messaging } from "@/pages/Messaging";
import { Settings } from "@/pages/Settings";
import { Setup } from "@/pages/Setup";
import { Login } from "@/pages/Login";
import { Queue } from "@/pages/Queue";
import { IPD } from "@/pages/IPD";
import { Laboratory } from "@/pages/Laboratory";
import { Billing } from "@/pages/Billing";
import { Inventory } from "@/pages/Inventory";
import { Pharmacy } from "@/pages/Pharmacy";
import { AuditLog as AuditLogPage } from "@/pages/AuditLog";
import { Users as UsersPage } from "@/pages/Users";
import { Reports } from "@/pages/Reports";
import { Backup } from "@/pages/Backup";
import { Toaster } from "sonner";
import { Loader2, KeyRound, Fingerprint, Copy, Check, ArrowRight, ArrowLeft } from "lucide-react";
import logo from "@/assets/logo_transparant.png";
import { AuthProvider, useAuth } from "@/lib/auth";
import { useChangePassword, useInstallFingerprint, useInstallLicense } from "@/lib/queries";
import type { LicenseInfo } from "@/lib/models";
import { RequirePermission } from "@/components/auth/RequirePermission";
import { PERMISSIONS } from "@/lib/rbac";

// Lazy-loaded route — keeps the initial bundle smaller for users who never
// open Radiology. Mirrors the lazy-import pattern requested in RAD-3.
const Radiology = lazy(() =>
  import("@/pages/Radiology").then((m) => ({ default: m.Radiology })),
);

// Blood Bank (Phase 2-E, SRS FR-0145–FR-0149) — lazy-loaded for the same
// bundle-size reason as Radiology.
const BloodBank = lazy(() =>
  import("@/pages/BloodBank").then((m) => ({ default: m.BloodBank })),
);

/**
 * Wraps any pre-authenticated screen (boot loader, Setup, Login, license
 * wizard, error states) with the minimal (unauthenticated) TitleBar so
 * the custom window chrome — drag region + minimize/maximize/close — is
 * present from the very first frame, not just once AppShell mounts.
 * AppShell renders its own `<TitleBar authenticated>` internally, so it
 * must NOT be wrapped with this a second time.
 */
function WithTitleBar({ children }: { children: ReactNode }) {
  return (
    <div className="flex flex-col h-screen w-screen overflow-hidden">
      <TitleBar />
      <div className="flex-1 min-h-0 overflow-y-auto">{children}</div>
    </div>
  );
}

export interface AppConfig {
  mode: string;
  db_host: string;
  db_port: number;
  db_user: string;
  db_password: string;
  db_name: string;
  clinic_name: string;
  doctors_whatsapp_group: string;
  setup_complete: boolean;
  pinned_server_cert_pem: string;
  pinned_server_fingerprint: string;
}

type BootPhase =
  | "checkingSetup"
  | "needsSetup"
  | "verifyingLicense"
  | "licenseError"
  | "booting"
  | "ready"
  | "initError";

function App() {
  const [phase, setPhase] = useState<BootPhase>("checkingSetup");
  const [initError, setInitError] = useState<string | null>(null);
  const [licenseError, setLicenseError] = useState<string | null>(null);
  const [licenseInfo, setLicenseInfo] = useState<LicenseInfo | null>(null);
  const [initStatus, setInitStatus] = useState("Verifying license...");
  const [serverMode, setServerMode] = useState(false);
  const [serverIp, setServerIp] = useState("");
  const [config, setConfig] = useState<AppConfig | null>(null);

  useEffect(() => {
    let unlistenFn: (() => void) | null = null;
    listen<string>("init_status", (e) => setInitStatus(e.payload)).then((f) => {
      unlistenFn = f;
    });
    checkSetupThenBoot();
    return () => {
      unlistenFn?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const checkSetupThenBoot = async () => {
    setPhase("checkingSetup");
    try {
      const cfg = await invoke<AppConfig | null>("get_config");
      const buildMode = (import.meta as { env?: { VITE_HMS_BUILD_MODE?: string } }).env?.VITE_HMS_BUILD_MODE || "server";
      if (buildMode === "client" && (!cfg || !cfg.setup_complete)) {
        setPhase("needsSetup");
        return;
      }
      await verifyLicenseAndBoot(cfg);
    } catch {
      const buildMode = (import.meta as { env?: { VITE_HMS_BUILD_MODE?: string } }).env?.VITE_HMS_BUILD_MODE || "server";
      if (buildMode === "client") {
        setPhase("needsSetup");
      } else {
        await verifyLicenseAndBoot(null);
      }
    }
  };

  const verifyLicenseAndBoot = async (cfg: AppConfig | null) => {
    setPhase("verifyingLicense");
    setInitStatus("Verifying license...");
    try {
      const info = await invoke<LicenseInfo>("verify_license");
      setLicenseInfo(info);
    } catch (err) {
      // License missing/invalid — block boot. The app must not touch the DB.
      setLicenseError(String(err));
      setPhase("licenseError");
      return;
    }
    await bootApp(cfg);
  };

  const handleReconfigureClient = async () => {
    const buildMode = (import.meta as { env?: { VITE_HMS_BUILD_MODE?: string } }).env?.VITE_HMS_BUILD_MODE || "server";
    if (buildMode !== "client") return;
    try {
      await invoke("clear_config");
      setPhase("needsSetup");
      setInitError(null);
    } catch (err) {
      console.error("Failed to clear config:", err);
    }
  };

  const bootApp = async (cfgArg: AppConfig | null) => {
    setPhase("booting");
    setInitError(null);
    try {
      const result = await invoke<string>("initialize_database");
      const [role, ip] = result.split(":");
      setServerMode(role === "server");
      setServerIp(ip || "127.0.0.1");

      const cfg = cfgArg ?? (await invoke<AppConfig | null>("get_config"));
      setConfig(cfg);
      setPhase("ready");
    } catch (err) {
      setInitError(String(err));
      setPhase("initError");
    }
  };

  const handleSetupComplete = () => {
    verifyLicenseAndBoot(null);
  };

  // ── Setup ──
  if (phase === "checkingSetup") {
    return (
      <WithTitleBar>
        <div className="flex h-full w-full items-center justify-center bg-background gradient-mesh">
          <Loader2 className="h-6 w-6 text-primary animate-spin" />
        </div>
      </WithTitleBar>
    );
  }
  if (phase === "needsSetup") {
    return (
      <WithTitleBar>
        <Setup onSetupComplete={handleSetupComplete} />
      </WithTitleBar>
    );
  }

  // ── License error → show the setup wizard ──
  if (phase === "licenseError") {
    return (
      <WithTitleBar>
        <LicenseSetupScreen
          error={licenseError}
          onInstalled={() => verifyLicenseAndBoot(config)}
        />
      </WithTitleBar>
    );
  }

  // ── Boot screen ──
  if (phase === "booting") {
    return (
      <WithTitleBar>
        <BootScreen status={initStatus} licenseInfo={licenseInfo} />
      </WithTitleBar>
    );
  }

  // ── Init error ──
  if (phase === "initError") {
    const buildMode = (import.meta as { env?: { VITE_HMS_BUILD_MODE?: string } }).env?.VITE_HMS_BUILD_MODE || "server";
    const isClient = buildMode === "client";
    return (
      <WithTitleBar>
        <div className="flex h-full w-full items-center justify-center bg-background gradient-mesh p-4 select-none">
          <motion.div
            className="w-full max-w-[440px] surface-elevated p-8 text-center"
            initial={{ opacity: 0, y: 12, scale: 0.97 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            transition={{ duration: 0.3, ease: [0.22, 1, 0.36, 1] }}
          >
            <div className="h-14 w-14 bg-destructive/10 flex items-center justify-center mx-auto mb-5">
              <span className="text-2xl font-bold text-destructive">!</span>
            </div>
            <h3 className="text-display-lg text-foreground mb-2">Startup failed</h3>
            <p className="text-sm text-muted-foreground leading-relaxed mb-6">{initError}</p>
            <div className="flex gap-2 justify-center">
              <button onClick={() => bootApp(config)} className="h-11 px-5 bg-primary text-primary-foreground rounded-lg text-sm font-semibold hover:bg-[hsl(var(--primary-hover))] transition-all shadow-sm shadow-primary/20 active:scale-[0.98]">
                Try again
              </button>
              {isClient && (
                <button onClick={handleReconfigureClient} className="h-11 px-5 border border-border bg-card text-foreground rounded-lg text-sm font-semibold hover:bg-muted transition-all active:scale-[0.98]">
                  Reconfigure
                </button>
              )}
            </div>
          </motion.div>
        </div>
      </WithTitleBar>
    );
  }

  // ── Ready: auth gate ──
  return (
    <AuthProvider>
      <AuthGate config={config} setConfig={setConfig} serverMode={serverMode} serverIp={serverIp} licenseInfo={licenseInfo} />
      <Toaster richColors closeButton position="top-right" />
    </AuthProvider>
  );
}

function BootScreen({ status, licenseInfo }: { status: string; licenseInfo: LicenseInfo | null }) {
  return (
    <div className="flex h-full w-full flex-col items-center justify-center bg-background gradient-mesh select-none gap-6">
      <motion.div
        className="flex flex-col items-center gap-4"
        initial={{ opacity: 0, scale: 0.96 }}
        animate={{ opacity: 1, scale: 1 }}
        transition={{ duration: 0.3 }}
      >
        <div className="h-16 w-16 bg-primary/10 border border-primary/30 flex items-center justify-center">
          <img src={logo} alt="VitalFlow HMS Logo" className="w-36 h-36 object-contain" />
        </div>
        <div className="text-center">
          <h2 className="text-display-lg text-foreground">RASHEED MEDICAL CENTER HMS</h2>
          <p className="text-xs text-muted-foreground mt-1 uppercase tracking-widest">
            {licenseInfo?.hospital_name ?? "Hospital Management System"}
          </p>
        </div>
      </motion.div>

      <motion.div
        className="surface-card px-8 py-5 max-w-sm w-full mx-4"
        initial={{ opacity: 0, y: 8 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.3, delay: 0.1 }}
      >
        <div className="flex items-center gap-3 mb-3">
          <Loader2 className="h-4 w-4 text-primary animate-spin shrink-0" />
          <p className="text-sm font-medium text-foreground">{status}</p>
        </div>
        <div className="space-y-1.5">
          {[
            "Connecting to the hospital database",
            "Verifying tables are up to date",
            "Starting notification scheduler",
          ].map((step, i) => (
            <div key={i} className="flex items-center gap-2">
              <div className="h-1.5 w-1.5 rounded-full bg-primary/30" />
              <p className="text-xs text-muted-foreground">{step}</p>
            </div>
          ))}
        </div>
      </motion.div>

      <p className="text-[10px] text-muted-foreground max-w-xs text-center px-4">
        Licensed to {licenseInfo?.hospital_name ?? "this hospital"} ·{" "}
        {licenseInfo?.product_edition ?? "Enterprise"} edition
      </p>
    </div>
  );
}

/** Decides between Login, forced password change, and the main shell. */
function AuthGate({
  config,
  setConfig,
  serverMode,
  serverIp,
  licenseInfo,
}: {
  config: AppConfig | null;
  setConfig: (c: AppConfig) => void;
  serverMode: boolean;
  serverIp: string;
  licenseInfo: LicenseInfo | null;
}) {
  const { session, isLoading } = useAuth();

  if (isLoading) {
    return (
      <WithTitleBar>
        <div className="flex items-center justify-center h-full bg-background">
          <Loader2 className="h-6 w-6 text-primary animate-spin" />
        </div>
      </WithTitleBar>
    );
  }

  if (!session) {
    return (
      <WithTitleBar>
        <Login hospitalName={licenseInfo?.hospital_name ?? config?.clinic_name} />
      </WithTitleBar>
    );
  }

  if (session.must_change_password) {
    return (
      <WithTitleBar>
        <ForceChangePassword hospitalName={licenseInfo?.hospital_name} />
      </WithTitleBar>
    );
  }

  return (
    <AppShell
      config={config}
      serverMode={serverMode}
      serverIp={serverIp}
      hospitalName={licenseInfo?.hospital_name}
    >
      <RoutedPages config={config} setConfig={setConfig} />
    </AppShell>
  );
}

function ForceChangePassword({ hospitalName }: { hospitalName?: string }) {
  const changePwd = useChangePassword();
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [confirm, setConfirm] = useState("");
  const [err, setErr] = useState<string | null>(null);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setErr(null);
    if (next.length < 8) { setErr("New password must be at least 8 characters."); return; }
    if (next !== confirm) { setErr("Passwords do not match."); return; }
    try {
      await changePwd.mutateAsync({ current_password: current, new_password: next });
      window.location.reload();
    } catch (e2) {
      setErr(String(e2));
    }
  };

  return (
    <div className="flex min-h-vh w-full items-center justify-center bg-background gradient-mesh p-6 select-none">
      <motion.div initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} className="w-full max-w-md">
        <div className="flex flex-col items-center text-center gap-3 mb-6">
          <div className="h-14 w-14 rounded-[var(--radius-md)] bg-accent/10 border border-accent/20 flex items-center justify-center">
            <KeyRound className="h-7 w-7 text-accent" />
          </div>
          <div>
            <h1 className="text-display-lg text-foreground">Change your password</h1>
            <p className="text-sm text-muted-foreground mt-1.5">{hospitalName ?? "RASHEED MEDICAL CENTER HMS"} requires a new password before you continue.</p>
          </div>
        </div>
        <div className="auth-card">
          <form onSubmit={submit} className="form-stack">
            <Field label="Current password"><input type="password" autoComplete="current-password" value={current} onChange={(e)=>setCurrent(e.target.value)} required className="h-12 px-4 bg-card border border-border rounded-[var(--radius)] text-sm outline-none focus:border-primary/50 focus:ring-2 focus:ring-primary/15" /></Field>
            <Field label="New password"><input type="password" autoComplete="new-password" value={next} onChange={(e)=>setNext(e.target.value)} required className="h-12 px-4 bg-card border border-border rounded-[var(--radius)] text-sm outline-none focus:border-primary/50 focus:ring-2 focus:ring-primary/15" /></Field>
            <Field label="Confirm new password"><input type="password" autoComplete="new-password" value={confirm} onChange={(e)=>setConfirm(e.target.value)} required className="h-12 px-4 bg-card border border-border rounded-[var(--radius)] text-sm outline-none focus:border-primary/50 focus:ring-2 focus:ring-primary/15" /></Field>
            {err && <div className="rounded-[var(--radius)] border border-destructive/30 bg-destructive/8 px-4 py-3 text-xs text-destructive">{err}</div>}
            <button type="submit" disabled={changePwd.isPending} className="h-12 mt-1 rounded-full bg-primary text-primary-foreground text-[15px] font-semibold hover:bg-[hsl(var(--primary-hover))] hover:shadow-md transition-all shadow-sm disabled:opacity-60">
              {changePwd.isPending ? "Saving…" : "Update password"}
            </button>
          </form>
        </div>
      </motion.div>
    </div>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-1.5">
      <label className="text-xs font-semibold text-foreground uppercase tracking-wide">{label}</label>
      {children}
    </div>
  );
}

/** Route table + page transition. AppShell never remounts on navigation. */
function RoutedPages({ config, setConfig }: { config: AppConfig | null; setConfig: (c: AppConfig) => void }) {
  const navigate = useNavigate();
  return (
    <AnimatePresence mode="wait">
      <Routes>
        <Route path="/" element={<PageTransition><Dashboard onNavigate={(tab) => navigate(`/${tab}`)} triggerAddPatient={() => navigate("/patients?add=1")} triggerAddAppointment={() => navigate("/appointments?add=1")} /></PageTransition>} />
        <Route path="/appointments" element={<PageTransition><Appointments /></PageTransition>} />
        <Route path="/patients" element={<PageTransition><Patients /></PageTransition>} />
        <Route path="/doctors" element={<PageTransition><Doctors /></PageTransition>} />
        <Route path="/queue" element={<PageTransition><Queue /></PageTransition>} />
        <Route path="/ipd" element={<PageTransition><IPD /></PageTransition>} />
        <Route path="/laboratory" element={<PageTransition><Laboratory /></PageTransition>} />
        <Route
          path="/radiology"
          element={
            <PageTransition>
              <RequirePermission perm={PERMISSIONS.RadiologyView}>
                <Suspense
                  fallback={
                    <div className="flex items-center justify-center h-full">
                      <Loader2 className="h-6 w-6 text-primary animate-spin" />
                    </div>
                  }
                >
                  <Radiology />
                </Suspense>
              </RequirePermission>
            </PageTransition>
          }
        />
        <Route
          path="/blood-bank"
          element={
            <PageTransition>
              <RequirePermission perm={PERMISSIONS.BloodBankView}>
                <Suspense
                  fallback={
                    <div className="flex items-center justify-center h-full">
                      <Loader2 className="h-6 w-6 text-primary animate-spin" />
                    </div>
                  }
                >
                  <BloodBank />
                </Suspense>
              </RequirePermission>
            </PageTransition>
          }
        />
        <Route path="/billing" element={<PageTransition><Billing /></PageTransition>} />
        <Route path="/inventory" element={<PageTransition><Inventory /></PageTransition>} />
        <Route
          path="/pharmacy"
          element={
            <PageTransition>
              <RequirePermission perm={PERMISSIONS.InventoryView}>
                <Pharmacy />
              </RequirePermission>
            </PageTransition>
          }
        />
        <Route path="/messaging" element={<PageTransition><Messaging /></PageTransition>} />
        <Route path="/audit" element={<PageTransition><AuditLogPage /></PageTransition>} />
        <Route path="/users" element={<PageTransition><UsersPage /></PageTransition>} />
        <Route
          path="/reports"
          element={
            <PageTransition>
              <RequirePermission perm={PERMISSIONS.ReportsView}>
                <Reports />
              </RequirePermission>
            </PageTransition>
          }
        />
        <Route
          path="/backup"
          element={
            <PageTransition>
              <RequirePermission perm={PERMISSIONS.BackupsManage}>
                <Backup />
              </RequirePermission>
            </PageTransition>
          }
        />
        <Route path="/settings" element={<PageTransition><Settings config={config} onSaved={setConfig} /></PageTransition>} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </AnimatePresence>
  );
}

function PageTransition({ children }: { children: ReactNode }) {
  return (
    <motion.div initial={{ opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -6 }} transition={{ duration: 0.18, ease: "easeOut" }} className="flex-1 flex flex-col h-full overflow-hidden">
      {children}
    </motion.div>
  );
}

// ── First-run license setup wizard ────────────────────────────────────────────
// Shown when no license is installed or the installed license is invalid.
// Customer flow:
//   Step 1: view + copy this machine's fingerprint → email to software company
//   Step 2: paste the signed license.json received back → install + verify
function LicenseSetupScreen({ onInstalled, error }: { onInstalled: () => void; error?: string | null }) {
  const { data: fpData, isLoading: fpLoading } = useInstallFingerprint();
  const installLic = useInstallLicense();
  const [step, setStep] = useState<"fingerprint" | "install">("fingerprint");
  const [licenseText, setLicenseText] = useState("");
  const [copied, setCopied] = useState(false);
  const [installError, setInstallError] = useState<string | null>(error ?? null);

  const copyFingerprint = async () => {
    if (!fpData) return;
    try {
      await navigator.clipboard.writeText(fpData.fingerprint);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // fallback for environments without clipboard API
    }
  };

  const install = async () => {
    setInstallError(null);
    try {
      await installLic.mutateAsync(licenseText);
      onInstalled();
    } catch (e) {
      setInstallError(String(e));
    }
  };

  return (
    <div className="flex h-full w-full items-center justify-center bg-background gradient-mesh p-4 select-none">
      <motion.div
        className="w-full max-w-[520px] surface-elevated p-8"
        initial={{ opacity: 0, y: 12, scale: 0.97 }}
        animate={{ opacity: 1, y: 0, scale: 1 }}
        transition={{ duration: 0.3, ease: [0.22, 1, 0.36, 1] }}
      >
        {/* Header */}
        <div className="flex items-center gap-3 mb-6">
          <div className="h-12 w-12 bg-primary/10 flex items-center justify-center shrink-0">
            <Fingerprint className="h-6 w-6 text-primary" />
          </div>
          <div>
            <h2 className="text-display-md text-foreground">License setup</h2>
            <p className="text-xs text-muted-foreground mt-0.5">Step {step === "fingerprint" ? "1" : "2"} of 2</p>
          </div>
        </div>

        {installError && (
          <div className="mb-5 px-4 py-3 rounded-lg bg-destructive/8 border border-destructive/20 text-xs text-destructive">
            {installError}
          </div>
        )}

        {step === "fingerprint" && (
          <div className="space-y-5">
            <div>
              <h3 className="text-display-sm text-foreground mb-1.5">Get your machine fingerprint</h3>
              <p className="text-sm text-muted-foreground leading-relaxed">
                This 64-character string uniquely identifies this computer. Send it to your software vendor to receive a signed license file.
              </p>
            </div>
            <div className="space-y-2">
              <label className="text-xs font-semibold text-foreground uppercase tracking-wide">Hardware fingerprint</label>
              {fpLoading ? (
                <div className="skeleton h-24 w-full rounded-lg" />
              ) : (
                <div className="relative">
                  <pre className="text-xs bg-muted p-4 rounded-lg border border-border font-mono break-all whitespace-pre-wrap max-h-32 overflow-y-auto">
                    {fpData?.fingerprint ?? "Computing…"}
                  </pre>
                </div>
              )}
              <button
                onClick={copyFingerprint}
                disabled={!fpData}
                className="h-10 px-4 bg-primary text-primary-foreground rounded-lg text-sm font-semibold hover:bg-[hsl(var(--primary-hover))] transition-all shadow-sm shadow-primary/20 active:scale-[0.98] flex items-center gap-2 disabled:opacity-50"
              >
                {copied ? <><Check className="h-4 w-4" /> Copied!</> : <><Copy className="h-4 w-4" /> Copy fingerprint</>}
              </button>
            </div>
            <div className="pt-3 border-t border-border">
              <button
                onClick={() => setStep("install")}
                className="h-11 w-full bg-primary text-primary-foreground rounded-lg text-sm font-semibold hover:bg-[hsl(var(--primary-hover))] transition-all shadow-sm shadow-primary/20 active:scale-[0.98] flex items-center justify-center gap-2"
              >
                I have my license, continue <ArrowRight className="h-4 w-4" />
              </button>
            </div>
          </div>
        )}

        {step === "install" && (
          <div className="space-y-5">
            <div>
              <h3 className="text-display-sm text-foreground mb-1.5">Paste your license</h3>
              <p className="text-sm text-muted-foreground leading-relaxed">
                Paste the entire license.json content your vendor sent you. It will be verified against this machine&apos;s fingerprint before installation.
              </p>
            </div>
            <div className="space-y-2">
              <label className="text-xs font-semibold text-foreground uppercase tracking-wide">License JSON</label>
              <textarea
                value={licenseText}
                onChange={(e) => setLicenseText(e.target.value)}
                rows={10}
                placeholder='{"deployment_id":"DEP-...","signature":"..."}'
                className="w-full text-xs font-mono p-3 bg-muted/50 border border-border rounded-lg outline-none focus:bg-card focus:border-primary/40 focus:ring-2 focus:ring-primary/15 transition-all resize-y"
              />
            </div>
            <div className="flex gap-2 pt-3 border-t border-border">
              <button
                onClick={() => setStep("fingerprint")}
                className="h-11 px-5 border border-border bg-card text-foreground rounded-lg text-sm font-semibold hover:bg-muted transition-all active:scale-[0.98] flex items-center gap-2"
              >
                <ArrowLeft className="h-4 w-4" /> Back
              </button>
              <button
                onClick={install}
                disabled={!licenseText.trim() || installLic.isPending}
                className="h-11 flex-1 bg-primary text-primary-foreground rounded-lg text-sm font-semibold hover:bg-[hsl(var(--primary-hover))] transition-all shadow-sm shadow-primary/20 active:scale-[0.98] disabled:opacity-50 flex items-center justify-center gap-2"
              >
                {installLic.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : "Install & verify license"}
              </button>
            </div>
          </div>
        )}
      </motion.div>
    </div>
  );
}

export default App;
