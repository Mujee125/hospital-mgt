/**
 * Settings — clinic identity, WhatsApp notifications, client PC
 * pairing (server build only), and IT/advanced information.
 *
 * Presentation layer uses the shared design system (PageContainer →
 * PageHeader → SectionCard + FormField + design tokens). All hooks,
 * Tauri invoke() calls, mutations, toasts, and the `onSaved` prop
 * contract are preserved exactly from the legacy implementation.
 */
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";

import {
  Settings as SettingsIcon,
  Save,
  Bell,
  MessageCircle,
  CheckCircle2,
  XCircle,
  Loader2,
  KeyRound,
  Copy,
  ShieldCheck,
  Zap,
  ExternalLink,
  CalendarPlus,
  Clock,
  CalendarDays,
  FileBadge,
  Upload,
  DatabaseBackup,
  Plus,
  RotateCcw,
  Trash2,
  AlertTriangle,
} from "lucide-react";
import type { AppConfig } from "../App";
import {
  useNotificationLog,
  useSendWhatsAppNotification,
  useSendWhatsAppTest,
  useWhatsAppConfig,
  useSetWhatsAppConfig,
  useTestWhatsAppApi,
  useLicenseInfo,
  useHardwareFingerprint,
  useInstallLicense,
  useListBackups,
  useCreateBackup,
  useRestoreBackup,
  useDeleteBackup,
} from "@/lib/queries";
import { useAuth } from "@/lib/auth";
import { PERMISSIONS } from "@/lib/rbac";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
  DialogClose,
} from "@/components/ui/dialog";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  PageContainer,
  PageHeader,
  SectionCard,
  FormField,
  InfoCard,
  EmptyState,
  LoadingState,
} from "@/components/layout/shared";

const BUILD_MODE: "server" | "client" =
  (import.meta.env.VITE_HMS_BUILD_MODE as "server" | "client") ||
  "server";

interface SettingsProps {
  config: AppConfig | null;
  onSaved: (c: AppConfig) => void;
}

export function Settings({ config, onSaved }: SettingsProps) {
  const [clinicName, setClinicName] = useState(
    config?.clinic_name ?? "VitalFlow Clinic",
  );
  const [groupName, setGroupName] = useState(
    config?.doctors_whatsapp_group ?? "",
  );
  const [saving, setSaving] = useState(false);
  const [testPhone, setTestPhone] = useState("");

  const {
    data: logs = [],
    isLoading: loadingLogs,
    refetch: refetchLogs,
  } = useNotificationLog();
  const sendNotification = useSendWhatsAppNotification();
  const sendTest = useSendWhatsAppTest();

  const [pairingCode, setPairingCode] = useState<string | null>(null);
  const [pairingSecondsLeft, setPairingSecondsLeft] = useState<number | null>(
    null,
  );
  const [generatingCode, setGeneratingCode] = useState(false);

  const [localIp, setLocalIp] = useState<string | null>(null);

  useEffect(() => {
    if (!localIp) {
      invoke<string>("get_local_ip")
        .then(setLocalIp)
        .catch(() => setLocalIp("Unavailable"));
    }
  }, [localIp]);

  useEffect(() => {
    if (BUILD_MODE !== "server") return;
    const interval = setInterval(async () => {
      try {
        const remaining = await invoke<number | null>("get_pairing_status");
        setPairingSecondsLeft(remaining);
        if (remaining === null) setPairingCode(null);
      } catch {
        /* non-fatal */
      }
    }, 1000);
    return () => clearInterval(interval);
  }, []);

  const handleGeneratePairingCode = async () => {
    setGeneratingCode(true);
    try {
      const code = await invoke<string>("generate_pairing_code");
      setPairingCode(code);
      toast.success("Pairing code generated — valid for 10 minutes.");
    } catch (err: unknown) {
      toast.error(`Could not generate code: ${String(err)}`);
    } finally {
      setGeneratingCode(false);
    }
  };

  const handleCopyCode = () => {
    if (!pairingCode) return;
    navigator.clipboard.writeText(pairingCode);
    toast.success("Copied to clipboard");
  };

  const handleSave = async () => {
    setSaving(true);
    try {
      const updated: AppConfig = {
        ...(config as AppConfig),
        clinic_name: clinicName.trim(),
        doctors_whatsapp_group: groupName.trim(),
      };
      await invoke("save_config", { config: updated });
      onSaved(updated);
      toast.success("Settings saved successfully!");
    } catch (err: unknown) {
      toast.error(`Save failed: ${String(err)}`);
    } finally {
      setSaving(false);
    }
  };

  const handleTestNotification = () => {
    if (!testPhone.trim()) {
      toast.error("Enter a phone number to test.");
      return;
    }
    sendTest.mutate({ phone: testPhone.trim(), clinicName });
  };

  const handleTestGroupDigest = () => {
    if (!groupName.trim()) {
      toast.error("Enter a group name first.");
      return;
    }
    sendNotification.mutate({
      recipient: groupName.trim(),
      message: `🏥 *${clinicName} — Group Test*\n\nThis is a test message to your doctors group.\nWhatsApp group integration is working! ✅\n\n_VitalFlow HMS_`,
      is_group: true,
      appointment_id: null,
      notification_type: "test_group",
    });
  };

  const typeLabel = (t: string) => {
    const map: Record<string, string> = {
      booked: "Booked",
      confirmed: "Confirmed",
      cancelled: "Cancelled",
      reminder: "Reminder",
      daily_digest: "Daily Digest",
      test: "Test",
      test_group: "Group Test",
    };
    return map[t] ?? t;
  };

  const typeColor = (t: string) => {
    const map: Record<string, string> = {
      booked: "bg-primary/10 text-primary",
      confirmed: "bg-success/10 text-success",
      cancelled: "bg-destructive/10 text-destructive",
      reminder: "bg-warning/10 text-warning",
      daily_digest: "bg-accent/10 text-accent",
    };
    return map[t] ?? "bg-muted text-muted-foreground";
  };

  // ── Render ─────────────────────────────────────────────────────────────
  return (
    <PageContainer>
      <PageHeader
        icon={SettingsIcon}
        title="Settings"
        description="Clinic identity, WhatsApp notifications, client PC pairing, and IT information."
      />

      {/* License panel — CR-19 (LIC-DOC-01) */}
      <div className="mb-6">
        <LicenseSection />
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* ── Clinic & notification settings ──────────────────────────── */}
        <SectionCard
          icon={Bell}
          title="Clinic & notification settings"
          description="Identity used in WhatsApp messages"
          bodyClassName="p-6 form-stack"
        >
          <FormField
            label="Clinic name"
            htmlFor="clinic-name"
            hint="Used in WhatsApp messages and the doctors group digest."
          >
            <Input
              id="clinic-name"
              value={clinicName}
              onChange={(e) => setClinicName(e.target.value)}
              placeholder="VitalFlow Clinic"
            />
          </FormField>

          <FormField
            label="Doctors WhatsApp group name"
            htmlFor="group-name"
            hint="Exact name of the WhatsApp group as it appears in WhatsApp Web. Used for the daily digest and group notifications."
          >
            <Input
              id="group-name"
              value={groupName}
              onChange={(e) => setGroupName(e.target.value)}
              placeholder="Doctors Team — VitalFlow"
            />
          </FormField>

          <div className="pt-1">
            <Button onClick={handleSave} disabled={saving}>
              {saving ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Save className="h-4 w-4" />
              )}
              {saving ? "Saving..." : "Save settings"}
            </Button>
          </div>
        </SectionCard>

        {/* ── Connect a new client PC (server build only) ────────────── */}
        {BUILD_MODE === "server" && (
          <SectionCard
            icon={KeyRound}
            title="Connect a new client PC"
            description="One-time pairing code (10-minute expiry)"
            bodyClassName="p-6 form-stack"
          >
            <p className="text-sm text-muted-foreground leading-relaxed">
              Generate a one-time code and give it — along with this PC's IP
              address — to whoever is setting up a doctor or nurse PC. The
              code expires after 10 minutes and works once per client PC.
            </p>

            {pairingCode && pairingSecondsLeft !== null && (
              <div className="flex items-center gap-3 p-4 rounded-[var(--radius-md)] bg-primary/5 border border-primary/20">
                <span className="font-mono text-2xl font-bold tracking-widest text-foreground">
                  {pairingCode}
                </span>
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={handleCopyCode}
                  title="Copy code"
                  className="h-9 w-9 rounded-full"
                >
                  <Copy className="h-4 w-4" />
                </Button>
                <span className="text-xs text-muted-foreground ml-auto tabular-nums">
                  Expires in {Math.floor(pairingSecondsLeft / 60)}:
                  {String(pairingSecondsLeft % 60).padStart(2, "0")}
                </span>
              </div>
            )}

            <div className="pt-1">
              <Button
                variant={pairingCode ? "outline" : "default"}
                onClick={handleGeneratePairingCode}
                disabled={generatingCode}
              >
                {generatingCode ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <KeyRound className="h-4 w-4" />
                )}
                {pairingCode ? "Generate new code" : "Generate pairing code"}
              </Button>
            </div>
          </SectionCard>
        )}

        {/* ── WhatsApp Business API panel (full width) ───────────────── */}
        <div className="lg:col-span-2">
          <WhatsAppBusinessApiPanel />
        </div>

        {/* ── Test WhatsApp integration ──────────────────────────────── */}
        <SectionCard
          icon={MessageCircle}
          title="Test WhatsApp integration"
          description="Send a manual test message"
          bodyClassName="p-6 form-stack"
        >
          <div className="flex items-start gap-3 p-3 rounded-[var(--radius-sm)] bg-primary/5 border border-primary/20">
            <div className="text-xs text-primary space-y-1">
              <p className="font-semibold">How it works</p>
              <p className="leading-relaxed">
                VitalFlow opens a pre-filled WhatsApp message using your
                system's default handler. Make sure{" "}
                <strong>WhatsApp Desktop</strong> is installed, or that you're
                logged into{" "}
                <span className="font-mono">web.whatsapp.com</span> in your
                default browser. The message is pre-filled — just press Send in
                WhatsApp.
              </p>
            </div>
          </div>

          <FormField
            label="Test phone number"
            htmlFor="test-phone"
            hint="e.g. 923001234567 or +92 300 1234567"
          >
            <div className="flex flex-col sm:flex-row gap-2">
              <Input
                id="test-phone"
                className="flex-1"
                placeholder="923001234567"
                value={testPhone}
                onChange={(e) => setTestPhone(e.target.value)}
              />
              <Button
                onClick={handleTestNotification}
                disabled={sendTest.isPending}
                className="shrink-0 bg-success text-success-foreground hover:bg-success/90"
              >
                {sendTest.isPending && (
                  <Loader2 className="h-4 w-4 animate-spin" />
                )}
                Send test
              </Button>
            </div>
          </FormField>

          <div className="flex flex-col sm:flex-row items-start sm:items-center gap-3 pt-1">
            <Button
              variant="outline"
              onClick={handleTestGroupDigest}
              disabled={sendNotification.isPending || !groupName.trim()}
              className="border-success/40 text-success hover:bg-success/5"
            >
              {sendNotification.isPending && (
                <Loader2 className="h-4 w-4 animate-spin" />
              )}
              Test group message
            </Button>
            <p className="text-xs text-muted-foreground">
              Sends a test to the doctors group
            </p>
          </div>
        </SectionCard>

        {/* ── Automatic notification triggers ────────────────────────── */}
        <SectionCard
          icon={Bell}
          title="Automatic notification triggers"
          description="Events that auto-send WhatsApp messages"
          bodyClassName="p-6"
        >
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
            <InfoCard
              icon={CalendarPlus}
              title="Appointment booked"
              description="→ Patient phone"
              color="primary"
            />
            <InfoCard
              icon={CheckCircle2}
              title="Appointment confirmed"
              description="→ Patient phone"
              color="success"
            />
            <InfoCard
              icon={XCircle}
              title="Appointment cancelled"
              description="→ Patient phone"
              color="destructive"
            />
            <InfoCard
              icon={Clock}
              title="1 hour before appt"
              description="→ Patient phone (reminder)"
              color="warning"
            />
            <InfoCard
              icon={CalendarDays}
              title="Daily at 07:30"
              description="→ Doctors WhatsApp group"
              color="accent"
            />
          </div>
        </SectionCard>

        {/* ── Notification log (full width) ──────────────────────────── */}
        <SectionCard
          className="lg:col-span-2"
          icon={MessageCircle}
          title="Notification log"
          description="Recent WhatsApp messages sent from this workstation"
          action={
            <Button
              variant="link"
              size="sm"
              onClick={() => refetchLogs()}
            >
              Refresh
            </Button>
          }
          bodyClassName="p-6"
        >
          {loadingLogs ? (
            <div className="space-y-2">
              {Array.from({ length: 3 }).map((_, i) => (
                <div key={i} className="skeleton h-12 w-full" />
              ))}
            </div>
          ) : logs.length === 0 ? (
            <p className="text-xs text-muted-foreground text-center py-8">
              No notifications sent yet.
            </p>
          ) : (
            <div className="space-y-2 max-h-72 overflow-y-auto pr-1">
              {logs.map((log) => (
                <div
                  key={log.id}
                  className="flex items-start gap-3 p-3 rounded-[var(--radius-sm)] bg-muted/40 border border-border"
                >
                  {log.success ? (
                    <CheckCircle2 className="h-4 w-4 text-success shrink-0 mt-0.5" />
                  ) : (
                    <XCircle className="h-4 w-4 text-destructive shrink-0 mt-0.5" />
                  )}
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 flex-wrap">
                      <span
                        className={`text-[9px] font-bold uppercase tracking-wide px-1.5 py-0.5 rounded-sm ${typeColor(log.notification_type)}`}
                      >
                        {typeLabel(log.notification_type)}
                      </span>
                      <span className="text-[10px] text-muted-foreground font-mono">
                        {log.recipient}
                      </span>
                    </div>
                    <p className="text-[10px] text-muted-foreground mt-1 truncate">
                      {log.message.split("\n")[0]}
                    </p>
                  </div>
                  <span className="text-[9px] text-muted-foreground shrink-0 tabular-nums">
                    {new Date(log.sent_at).toLocaleTimeString([], {
                      hour: "2-digit",
                      minute: "2-digit",
                    })}
                  </span>
                </div>
              ))}
            </div>
          )}
        </SectionCard>

        {/* ── Advanced / IT information (full width) ─────────────────── */}
        <SectionCard
          className="lg:col-span-2"
          icon={ShieldCheck}
          title="Advanced / IT information"
          description="For installation/IT use only — not needed day-to-day"
          bodyClassName="p-6 form-stack"
        >
          <p className="text-xs text-muted-foreground leading-relaxed">
            For installation/IT use only — not needed for day-to-day operation.
            This information identifies this PC on the hospital network.
          </p>

          {BUILD_MODE === "server" ? (
            <FormField label="Server LAN IP address">
              <p className="font-mono text-sm text-foreground px-3.5 py-2.5 rounded-[var(--radius)] bg-muted/40 border border-border">
                {localIp ?? "Loading..."}
              </p>
            </FormField>
          ) : (
            <FormField label="Connected server address">
              <p className="font-mono text-sm text-foreground px-3.5 py-2.5 rounded-[var(--radius)] bg-muted/40 border border-border">
                {config?.db_host || "Not configured"}
              </p>
            </FormField>
          )}

          {config?.pinned_server_fingerprint && (
            <FormField
              label="Pinned server certificate (SHA-256)"
              hint="Captured during pairing. If this PC ever shows a certificate-mismatch error, it means a different machine is answering at the server's address — stop and investigate before continuing, rather than re-pairing automatically."
            >
              <div className="flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wide text-success mb-1.5">
                <ShieldCheck className="h-3 w-3" />
                Verified
              </div>
              <p className="font-mono text-[10px] text-foreground break-all p-3 rounded-[var(--radius-sm)] bg-muted/40 border border-border">
                {config.pinned_server_fingerprint}
              </p>
            </FormField>
          )}

          <FormField label="Pairing port">
            <p className="font-mono text-sm text-foreground px-3.5 py-2.5 rounded-[var(--radius)] bg-muted/40 border border-border">
              42011 (TLS)
            </p>
          </FormField>
        </SectionCard>
      </div>

      {/* ── Backup & Restore (SRS §9 A-07 — Phase 2) ──────────────────── */}
      {/* Server-build only. RBAC-guarded both client-side (useAuth().has) and
          server-side (rbac::require on every command). On client/dev builds
          the four Tauri commands are not registered, so useListBackups errors
          and the section renders an inline notice instead of a table. */}
      <div className="mt-6">
        <BackupSection />
      </div>
    </PageContainer>
  );
}

// ── WhatsApp sending method panel ────────────────────────────────────────
// Lets the user choose between two methods:
//   1. Deep Link (manual) — opens WhatsApp, user clicks Send. Free, no setup.
//   2. Business API (automatic) — sends directly via Meta Cloud API. No UI.
// Both methods are always available; the user picks which one to use.
function WhatsAppBusinessApiPanel() {
  const { data: config } = useWhatsAppConfig();
  const saveConfig = useSetWhatsAppConfig();
  const testApi = useTestWhatsAppApi();

  const [accessToken, setAccessToken] = useState("");
  const [phoneNumberId, setPhoneNumberId] = useState("");
  const [preferredMethod, setPreferredMethod] = useState<"api" | "deep_link">("deep_link");
  const [testPhone, setTestPhone] = useState("");
  const [showToken, setShowToken] = useState(false);

  useEffect(() => {
    if (config) {
      setPhoneNumberId((config.phone_number_id as string) ?? "");
      setPreferredMethod((config.preferred_method as "api" | "deep_link") ?? "deep_link");
    }
  }, [config]);

  const isConfigured = (config?.configured as boolean) ?? false;

  const saveMethod = (method: "api" | "deep_link") => {
    setPreferredMethod(method);
    // Save immediately so the preference takes effect
    saveConfig.mutate({
      accessToken: "",  // empty = preserve existing token
      phoneNumberId,
      enabled: method === "api",
      preferredMethod: method,
    });
  };

  const saveCredentials = () => {
    if (!accessToken.trim() && !isConfigured) {
      toast.error("Enter your access token to configure the Business API.");
      return;
    }
    if (!phoneNumberId.trim()) {
      toast.error("Enter your Phone Number ID.");
      return;
    }
    saveConfig.mutate({
      accessToken,
      phoneNumberId,
      enabled: preferredMethod === "api",
      preferredMethod,
    });
    setAccessToken("");
  };

  const test = () => {
    if (!testPhone.trim()) {
      toast.error("Enter a test phone number.");
      return;
    }
    testApi.mutate(testPhone.trim());
  };

  return (
    <SectionCard
      icon={MessageCircle}
      title="WhatsApp sending method"
      description="Choose between a free manual deep link or the automatic Business API"
      bodyClassName="p-6 form-stack"
    >
      {/* Method selector — two interactive tiles */}
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
        {/* Method 1: Deep Link (manual) */}
        <button
          type="button"
          onClick={() => saveMethod("deep_link")}
          className={`text-left p-4 rounded-[var(--radius-md)] border-2 transition-all duration-200 cursor-pointer ${
            preferredMethod === "deep_link"
              ? "border-primary bg-primary/5 shadow-sm"
              : "border-border bg-card hover:border-primary/30"
          }`}
        >
          <div className="flex items-center justify-between mb-2">
            <div className="flex items-center gap-2">
              <div
                className={`h-8 w-8 rounded-[var(--radius-sm)] flex items-center justify-center ${preferredMethod === "deep_link" ? "bg-primary/10" : "bg-muted"}`}
              >
                <ExternalLink
                  className={`h-4 w-4 ${preferredMethod === "deep_link" ? "text-primary" : "text-muted-foreground"}`}
                />
              </div>
              <span className="text-sm font-semibold text-foreground">
                Manual (free)
              </span>
            </div>
            {preferredMethod === "deep_link" && (
              <CheckCircle2 className="h-5 w-5 text-primary" />
            )}
          </div>
          <p className="text-xs text-muted-foreground leading-relaxed">
            Opens WhatsApp with the message pre-filled. You click Send. No
            setup, no cost. Works with WhatsApp Desktop or Web.
          </p>
        </button>

        {/* Method 2: Business API (automatic) */}
        <button
          type="button"
          onClick={() => saveMethod("api")}
          className={`text-left p-4 rounded-[var(--radius-md)] border-2 transition-all duration-200 cursor-pointer ${
            preferredMethod === "api"
              ? "border-warning bg-warning/5 shadow-sm"
              : "border-border bg-card hover:border-warning/30"
          }`}
        >
          <div className="flex items-center justify-between mb-2">
            <div className="flex items-center gap-2">
              <div
                className={`h-8 w-8 rounded-[var(--radius-sm)] flex items-center justify-center ${preferredMethod === "api" ? "bg-warning/10" : "bg-muted"}`}
              >
                <Zap
                  className={`h-4 w-4 ${preferredMethod === "api" ? "text-warning" : "text-muted-foreground"}`}
                />
              </div>
              <span className="text-sm font-semibold text-foreground">
                Automatic (API)
              </span>
            </div>
            {preferredMethod === "api" && (
              <CheckCircle2 className="h-5 w-5 text-warning" />
            )}
          </div>
          <p className="text-xs text-muted-foreground leading-relaxed">
            Sends directly via WhatsApp Business Cloud API — no WhatsApp opens.
            Requires Meta Business account. ~$0.004/msg.
          </p>
        </button>
      </div>

      {/* Business API credentials — shown when API method is selected or configured */}
      {(preferredMethod === "api" || isConfigured) && (
        <div className="form-stack pt-5 mt-2 border-t border-border">
          <div className="flex items-center gap-2">
            <Zap className="h-3.5 w-3.5 text-warning" />
            <h4 className="text-xs font-bold uppercase tracking-wider text-foreground">
              Business API credentials
            </h4>
            {isConfigured && (
              <span className="ml-auto flex items-center gap-1 text-[10px] font-bold uppercase text-success">
                <CheckCircle2 className="h-3 w-3" /> Configured
              </span>
            )}
          </div>

          <div className="flex items-start gap-3 p-3 rounded-[var(--radius-sm)] bg-warning/5 border border-warning/25 text-xs text-warning">
            <p>
              Get credentials at{" "}
              <a
                href="https://business.whatsapp.com/"
                target="_blank"
                rel="noopener noreferrer"
                className="font-semibold underline inline-flex items-center gap-0.5"
              >
                business.whatsapp.com <ExternalLink className="h-3 w-3" />
              </a>
            </p>
          </div>

          {isConfigured && (
            <div className="flex items-center gap-2 p-2.5 rounded-[var(--radius-sm)] bg-success/10 border border-success/30 text-xs text-success">
              <CheckCircle2 className="h-4 w-4 shrink-0" />
              <span>
                Token:{" "}
                <code className="font-mono">
                  {config?.access_token_masked as string}
                </code>
                {" · "}Phone ID:{" "}
                <code className="font-mono">
                  {config?.phone_number_id as string}
                </code>
              </span>
            </div>
          )}

          <FormField
            label={`Access token${isConfigured ? " (enter new to replace)" : ""}`}
            htmlFor="access-token"
          >
            <div className="relative">
              <Input
                id="access-token"
                type={showToken ? "text" : "password"}
                className="pr-16 font-mono"
                placeholder="EAAG..."
                value={accessToken}
                onChange={(e) => setAccessToken(e.target.value)}
              />
              <button
                type="button"
                onClick={() => setShowToken(!showToken)}
                className="absolute right-3 top-1/2 -translate-y-1/2 text-[10px] font-bold uppercase text-muted-foreground hover:text-foreground transition-colors"
              >
                {showToken ? "Hide" : "Show"}
              </button>
            </div>
          </FormField>

          <FormField label="Phone Number ID" htmlFor="phone-number-id">
            <Input
              id="phone-number-id"
              type="text"
              className="font-mono"
              placeholder="123456789012345"
              value={phoneNumberId}
              onChange={(e) => setPhoneNumberId(e.target.value)}
            />
          </FormField>

          <div className="pt-1">
            <Button
              onClick={saveCredentials}
              disabled={saveConfig.isPending}
              className="bg-warning text-warning-foreground hover:bg-warning/90"
            >
              {saveConfig.isPending ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Save className="h-4 w-4" />
              )}
              Save credentials
            </Button>
          </div>

          {isConfigured && (
            <FormField
              label="Test API — send a test message to this number"
              htmlFor="api-test-phone"
            >
              <div className="flex gap-2">
                <Input
                  id="api-test-phone"
                  type="text"
                  className="flex-1"
                  placeholder="923001234567"
                  value={testPhone}
                  onChange={(e) => setTestPhone(e.target.value)}
                />
                <Button
                  variant="outline"
                  onClick={test}
                  disabled={testApi.isPending}
                  className="border-warning/40 text-warning hover:bg-warning/5 shrink-0"
                >
                  {testApi.isPending && (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  )}
                  Send test
                </Button>
              </div>
            </FormField>
          )}
        </div>
      )}
    </SectionCard>
  );
}

// ── LicenseSection ───────────────────────────────────────────────────────────
// CR-19 (LIC-DOC-01): Settings → License panel. Referenced by 3 docs (Licensing
// Architecture, Licensing Workflow, SDD §7) but missing from the UI. The
// backend commands + frontend hooks already existed — this wires them together.
// Lets an admin view the current license, install a new one, and see the
// hardware fingerprint (needed when requesting a license from the issuer).
function LicenseSection() {
  const licenseInfo = useLicenseInfo();
  const fingerprint = useHardwareFingerprint();
  const installLicense = useInstallLicense();
  const [licenseJson, setLicenseJson] = useState("");

  const handleInstall = () => {
    const trimmed = licenseJson.trim();
    if (!trimmed) {
      toast.error("Paste a license JSON file first.");
      return;
    }
    installLicense.mutate(trimmed, {
      onSuccess: () => {
        setLicenseJson("");
      },
    });
  };

  const handleCopyFingerprint = () => {
    if (fingerprint.data) {
      navigator.clipboard.writeText(fingerprint.data);
      toast.success("Hardware fingerprint copied to clipboard.");
    }
  };

  const statusColor = (status: string) => {
    if (status === "active" || status === "valid") return "hsl(var(--success))";
    if (status === "expired" || status === "invalid") return "hsl(var(--destructive))";
    if (status === "grace" || status === "warning") return "hsl(var(--warning))";
    return "hsl(var(--muted-foreground))";
  };

  return (
    <SectionCard
      icon={FileBadge}
      title="License"
      description="View, install, or update the VitalFlow HMS license"
      bodyClassName="p-6 form-stack"
    >
      {/* Current license info */}
      {licenseInfo.isLoading ? (
        <div className="space-y-2">
          <div className="skeleton h-5 w-3/4" />
          <div className="skeleton h-5 w-1/2" />
        </div>
      ) : licenseInfo.data ? (
        <div className="rounded-[var(--radius-md)] border border-border bg-muted/30 p-4 space-y-3">
          <div className="flex items-center justify-between">
            <span className="text-xs font-semibold text-muted-foreground uppercase tracking-wide">
              Current License
            </span>
            <span
              className="status-badge"
              style={{
                background: `hsl(${statusColor(licenseInfo.data.status).replace("hsl(", "").replace(")", "")} / 0.12)`,
                color: statusColor(licenseInfo.data.status),
              }}
            >
              {licenseInfo.data.status}
            </span>
          </div>
          <dl className="grid grid-cols-1 sm:grid-cols-2 gap-x-4 gap-y-2 text-sm">
            <div>
              <dt className="text-xs text-muted-foreground">Hospital</dt>
              <dd className="font-medium text-foreground">
                {licenseInfo.data.hospital_name}
              </dd>
            </div>
            <div>
              <dt className="text-xs text-muted-foreground">Edition</dt>
              <dd className="font-medium text-foreground">
                {licenseInfo.data.product_edition}
              </dd>
            </div>
            <div>
              <dt className="text-xs text-muted-foreground">License ID</dt>
              <dd className="font-mono text-xs text-foreground break-all">
                {licenseInfo.data.license_id}
              </dd>
            </div>
            <div>
              <dt className="text-xs text-muted-foreground">Issued</dt>
              <dd className="font-medium text-foreground">
                {licenseInfo.data.issue_date
                  ? new Date(licenseInfo.data.issue_date).toLocaleDateString()
                  : "—"}
              </dd>
            </div>
            <div>
              <dt className="text-xs text-muted-foreground">Expires</dt>
              <dd className="font-medium text-foreground">
                {licenseInfo.data.expiration_date
                  ? new Date(licenseInfo.data.expiration_date).toLocaleDateString()
                  : "Perpetual"}
              </dd>
            </div>
            <div>
              <dt className="text-xs text-muted-foreground">Maintenance until</dt>
              <dd className="font-medium text-foreground">
                {licenseInfo.data.maintenance_until
                  ? new Date(licenseInfo.data.maintenance_until).toLocaleDateString()
                  : "—"}
              </dd>
            </div>
          </dl>
          <div className="flex items-center gap-2 pt-2 border-t border-border">
            {licenseInfo.data.fingerprint_matches ? (
              <CheckCircle2
                className="h-4 w-4 shrink-0"
                style={{ color: "hsl(var(--success))" }}
              />
            ) : (
              <XCircle
                className="h-4 w-4 shrink-0"
                style={{ color: "hsl(var(--destructive))" }}
              />
            )}
            <span className="text-xs text-muted-foreground">
              {licenseInfo.data.fingerprint_matches
                ? "Hardware fingerprint matches this machine."
                : "Hardware fingerprint MISMATCH — license is bound to a different machine."}
            </span>
          </div>
          {licenseInfo.data.enabled_modules.length > 0 && (
            <div className="pt-2 border-t border-border">
              <dt className="text-xs text-muted-foreground mb-1.5">
                Enabled modules
              </dt>
              <div className="flex flex-wrap gap-1.5">
                {licenseInfo.data.enabled_modules.map((m) => (
                  <span
                    key={m}
                    className="status-badge"
                    style={{
                      background: "hsl(var(--primary) / 0.12)",
                      color: "hsl(var(--primary))",
                    }}
                  >
                    {m}
                  </span>
                ))}
              </div>
            </div>
          )}
        </div>
      ) : (
        <div className="rounded-[var(--radius-md)] border border-warning/30 bg-warning/5 p-4">
          <p className="text-sm text-foreground">
            No license is currently installed. The application is running in
            development/trial mode. Install a license to enable full
            functionality.
          </p>
        </div>
      )}

      {/* Hardware fingerprint */}
      <FormField
        label="Hardware fingerprint"
        htmlFor="hw-fingerprint"
        hint="Provide this to your license issuer when requesting a license. It uniquely identifies this machine."
      >
        <div className="flex gap-2">
          <Input
            id="hw-fingerprint"
            readOnly
            value={fingerprint.data ?? "—"}
            className="font-mono text-xs"
          />
          <Button
            variant="outline"
            onClick={handleCopyFingerprint}
            disabled={!fingerprint.data}
            className="shrink-0"
          >
            <Copy className="h-4 w-4" />
            Copy
          </Button>
        </div>
      </FormField>

      {/* Install license */}
      <FormField
        label="Install license"
        htmlFor="license-json"
        hint="Paste the contents of the .license JSON file issued to this machine."
      >
        <Textarea
          id="license-json"
          className="font-mono text-xs min-h-[120px]"
          placeholder='{"license_id":"...","payload":{...},"signature":"..."}'
          value={licenseJson}
          onChange={(e) => setLicenseJson(e.target.value)}
        />
      </FormField>
      <div className="flex gap-2">
        <Button
          onClick={handleInstall}
          disabled={installLicense.isPending || !licenseJson.trim()}
        >
          {installLicense.isPending ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <Upload className="h-4 w-4" />
          )}
          Install & Verify License
        </Button>
      </div>
    </SectionCard>
  );
}

// ── BackupSection (SRS §9 A-07 — Phase 2) ─────────────────────────────────
//
// Lets an admin create a full-database backup (pg_dump -Fc), list existing
// backups, restore from a backup (pg_restore --clean --if-exists — destructive),
// and delete a backup file. All four backend commands are RBAC-guarded by
// `Permission::BackupsManage`; the section itself is hidden client-side for
// users without that permission (the backend re-checks on every command, so
// the client-side hide is a UX affordance, not a security control).
//
// On non-server builds (client / dev) the four Tauri commands are not
// registered (the Rust source is `#[cfg(feature = "server-build")]`), so
// `useListBackups` rejects and the section renders an inline notice instead
// of the table.
//
// Two prominent warnings about restore:
//   1. A persistent warning callout inside the section.
//   2. A red destructive confirmation dialog before any restore, with the
//      backup filename shown for the operator to verify.
//
// NOTE: this section intentionally reuses the same hooks as the dedicated
// `/backup` page (Backup.tsx) — they share the `["backups"]` query key, so
// creating/deleting/restoring in either location refreshes the other. The
// Settings section is provided as a convenience (one-stop admin panel); the
// dedicated page is the primary entry point.
function BackupSection() {
  const { has } = useAuth();
  const canManage = has(PERMISSIONS.BackupsManage);

  const { data: backups = [], isLoading, error } = useListBackups();
  const create = useCreateBackup();
  const restore = useRestoreBackup();
  const remove = useDeleteBackup();

  // Restore confirmation target — set when the user clicks "Restore" on a row.
  const [restoreTarget, setRestoreTarget] = useState<string | null>(null);
  // Delete confirmation target.
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);

  if (!canManage) {
    // The user's role does not grant BackupsManage. The backend would reject
    // every command anyway; hide the section rather than showing buttons that
    // would only error out when clicked.
    return null;
  }

  const handleCreate = () => {
    create.mutate();
  };

  const handleRestoreConfirm = async () => {
    if (!restoreTarget) return;
    await restore.mutateAsync(restoreTarget);
    setRestoreTarget(null);
  };

  const handleDeleteConfirm = async () => {
    if (!deleteTarget) return;
    await remove.mutateAsync(deleteTarget);
    setDeleteTarget(null);
  };

  return (
    <SectionCard
      icon={DatabaseBackup}
      title="Backup & Restore"
      description="Create and restore full-database backups (server build only)"
      bodyClassName="p-6 form-stack"
      action={
        <Button
          onClick={handleCreate}
          disabled={create.isPending}
          size="sm"
        >
          {create.isPending ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <Plus className="h-4 w-4" />
          )}
          {create.isPending ? "Creating…" : "Create backup"}
        </Button>
      }
    >
      {/* Persistent restart-after-restore warning. Always visible because the
          requirement is non-obvious and a missed restart would cause
          cascading "prepared statement does not exist" errors elsewhere in
          the app (pg_restore --clean drops+recreates every table → DB pool
          holds stale prepared statements against the old table OIDs). */}
      <div className="flex items-start gap-3 rounded-[var(--radius-md)] border border-warning/30 bg-warning/10 px-4 py-3 text-sm">
        <AlertTriangle className="h-4 w-4 shrink-0 mt-0.5 text-warning" />
        <div className="space-y-1">
          <p className="font-semibold text-warning-foreground">
            Restoring will overwrite the current database.
          </p>
          <p className="text-xs text-muted-foreground leading-relaxed">
            The app must be restarted after restore — the database connection
            pool will hold stale prepared statements against the old tables.
            Any data created after the backup was taken will be permanently lost.
          </p>
        </div>
      </div>

      {/* Error state — typically reached on client/dev builds where the
          server-build-only commands are not registered. */}
      {error ? (
        <div className="flex items-start gap-3 p-3 rounded-[var(--radius-sm)] bg-destructive/5 border border-destructive/20">
          <AlertTriangle className="h-4 w-4 shrink-0 mt-0.5 text-destructive" />
          <div className="space-y-1">
            <p className="text-sm font-semibold text-foreground">
              Backups are only available on the server build.
            </p>
            <p className="text-xs text-muted-foreground leading-relaxed">
              The backup commands shell out to the bundled PostgreSQL
              (<code className="font-mono">pg_dump</code> /{" "}
              <code className="font-mono">pg_restore</code>), which only ships
              with the server build. On a client build, connect to the server
              machine and run backups from there.
            </p>
            <p className="text-[11px] text-muted-foreground mt-2 font-mono break-all">
              {String(error)}
            </p>
          </div>
        </div>
      ) : isLoading ? (
        <LoadingState rows={3} />
      ) : backups.length === 0 ? (
        <EmptyState
          icon={DatabaseBackup}
          title="No backups yet"
          description="Create your first full-database backup to get started. Backups are saved as PostgreSQL custom-format archives under %ProgramData%\\HMS\\backups\\."
          action={
            <Button onClick={handleCreate} disabled={create.isPending}>
              {create.isPending ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin" />
                  Creating…
                </>
              ) : (
                <>
                  <Plus className="h-4 w-4" />
                  Create backup
                </>
              )}
            </Button>
          }
        />
      ) : (
        <>
          <div className="flex items-center justify-between">
            <span className="text-sm font-medium text-muted-foreground">
              {backups.length} backup{backups.length === 1 ? "" : "s"} on disk
            </span>
            <span className="text-xs text-muted-foreground font-mono">
              %ProgramData%\HMS\backups\
            </span>
          </div>
          <Table>
            <TableHeader>
              <TableRow className="border-border hover:bg-transparent">
                <TableHead scope="col">Filename</TableHead>
                <TableHead scope="col">Size</TableHead>
                <TableHead scope="col">Created (UTC)</TableHead>
                <TableHead scope="col" className="text-right">
                  Actions
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {backups.map((b) => (
                <TableRow key={b.filename}>
                  <TableCell className="font-mono text-xs text-foreground break-all">
                    {b.filename}
                  </TableCell>
                  <TableCell className="tabular-nums text-muted-foreground">
                    {formatBytes(b.size_bytes)}
                  </TableCell>
                  <TableCell className="text-muted-foreground">
                    {b.created_at}
                  </TableCell>
                  <TableCell className="text-right">
                    <div className="flex justify-end gap-1">
                      <Button
                        size="sm"
                        variant="ghost"
                        className="h-8"
                        disabled={restore.isPending}
                        onClick={() => setRestoreTarget(b.filename)}
                      >
                        <RotateCcw className="h-3.5 w-3.5" />
                        Restore
                      </Button>
                      <Button
                        size="icon"
                        variant="ghost"
                        className="h-8 w-8 text-muted-foreground hover:text-destructive"
                        title="Delete backup"
                        aria-label={`Delete ${b.filename}`}
                        disabled={remove.isPending}
                        onClick={() => setDeleteTarget(b.filename)}
                      >
                        <Trash2 className="h-3.5 w-3.5" />
                      </Button>
                    </div>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </>
      )}

      {/* ── Restore confirmation dialog ─────────────────────────────────── */}
      {/* Destructive — red variant. The operator must click through a clear
          confirmation; the backup filename is shown explicitly so they can
          verify they picked the right row. */}
      <Dialog
        open={restoreTarget !== null}
        onOpenChange={(o) => {
          if (!o) setRestoreTarget(null);
        }}
      >
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle className="text-destructive">
              Restore from backup?
            </DialogTitle>
            <DialogDescription>
              This will replace ALL current data with the contents of the
              backup. Any data created after the backup was taken will be
              permanently lost.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-3 py-2">
            <div className="rounded-[var(--radius)] border border-destructive/30 bg-destructive/8 px-3 py-2.5 text-xs text-destructive flex items-start gap-2">
              <AlertTriangle className="h-3.5 w-3.5 shrink-0 mt-0.5" />
              <div>
                <p className="font-semibold">This action cannot be undone.</p>
                <p className="mt-0.5 text-destructive/80">
                  After the restore completes you must restart the application —
                  the database connection pool will hold stale prepared
                  statements against the old tables.
                </p>
              </div>
            </div>
            <p className="text-sm text-muted-foreground leading-relaxed">
              You are about to restore from:
            </p>
            <p className="font-mono text-xs bg-muted px-3 py-2 rounded-[var(--radius)] border border-border break-all">
              {restoreTarget}
            </p>
          </div>
          <DialogFooter>
            <DialogClose asChild>
              <Button variant="outline">Cancel</Button>
            </DialogClose>
            <Button
              variant="destructive"
              disabled={restore.isPending}
              onClick={handleRestoreConfirm}
            >
              {restore.isPending ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin" />
                  Restoring…
                </>
              ) : (
                <>
                  <RotateCcw className="h-4 w-4" />
                  Restore &amp; replace all data
                </>
              )}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* ── Delete confirmation dialog ──────────────────────────────────── */}
      <Dialog
        open={deleteTarget !== null}
        onOpenChange={(o) => !o && setDeleteTarget(null)}
      >
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>Delete backup file?</DialogTitle>
            <DialogDescription>
              The backup file will be permanently removed from disk. The
              database is not affected.
            </DialogDescription>
          </DialogHeader>
          <p className="text-sm text-muted-foreground leading-relaxed py-2">
            Delete{" "}
            <span className="font-mono text-xs font-semibold text-foreground break-all">
              {deleteTarget}
            </span>{" "}
            ? This cannot be undone.
          </p>
          <DialogFooter>
            <DialogClose asChild>
              <Button variant="outline">Cancel</Button>
            </DialogClose>
            <Button
              variant="destructive"
              disabled={remove.isPending}
              onClick={handleDeleteConfirm}
            >
              {remove.isPending ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Trash2 className="h-4 w-4" />
              )}
              Delete file
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </SectionCard>
  );
}

/** Formats a byte count as a human-readable string (e.g. "12.3 MB"). Uses
 *  1024-based units (binary) since backups are disk files, not SI units.
 *  Inlined here (rather than imported from utils.ts) because this is the
 *  only consumer in Settings.tsx; Backup.tsx has its own copy for the same
 *  reason. If a third consumer appears, promote to utils.ts. */
function formatBytes(bytes: number): string {
  if (!bytes || bytes < 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let unitIndex = 0;
  let value = bytes;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex++;
  }
  // 0 decimals for B, 1 for KB+, but show 2 for small MB values so a 1.05 MB
  // backup doesn't render as "1 MB" (which would round to the same as 1.04).
  const decimals = unitIndex === 0 ? 0 : value < 10 ? 2 : 1;
  return `${value.toFixed(decimals)} ${units[unitIndex]}`;
}
