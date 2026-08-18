import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
 import { listen } from "@tauri-apps/api/event";
import { motion } from "motion/react";
import { Server, Laptop, ArrowRight, CheckCircle2, AlertTriangle, RefreshCw, KeyRound } from "lucide-react";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { toast } from "sonner";
import type { AppConfig } from "../App";

/**
 * First-run setup screen.
 *
 * IMPORTANT: this build's role (server vs client) is fixed at compile time
 * by which installer was run — it is NOT a choice made here.
 *
 * Server build: nothing to ask here in normal operation. PostgreSQL
 * provisioning happens once, during installation (see windows/hooks.nsh),
 * which also marks setup_complete in the machine-wide config before the
 * app is ever launched. The reception PC's admin gets a one-time pairing
 * code from Settings to hand out to whoever sets up client PCs.
 *
 * Client build: collects the reception PC's IP + a pairing code (from the
 * server's Settings screen), exchanges the code for the real DB
 * credentials over the LAN, then saves a complete config. The raw
 * PostgreSQL password is never typed by a human anywhere in this flow.
 */

type BuildMode = "server" | "client";

// Injected by Vite at build time — see vite.config.ts.
const BUILD_MODE: BuildMode = (import.meta.env.VITE_HMS_BUILD_MODE as BuildMode) || "server";

interface SetupProps {
  onSetupComplete: (config: AppConfig) => void;
}

interface PairingCreds {
  db_user: string;
  db_password: string;
  db_name: string;
  db_port: number;
  fingerprint: string;
}

export function Setup({ onSetupComplete }: SetupProps) {
  const [dbHost, setDbHost] = useState("");
  const [pairingCode, setPairingCode] = useState("");
  const [pairing, setPairing] = useState(false);
  const [creds, setCreds] = useState<PairingCreds | null>(null);
  const [pairError, setPairError] = useState<string | null>(null);
  const [initializing, setInitializing] = useState(false);

  const handleRedeemCode = async () => {
    if (!dbHost.trim() || !pairingCode.trim()) {
      toast.error("Enter both the server IP and the pairing code.");
      return;
    }
    setPairing(true);
    setPairError(null);
    try {
      // redeem_pairing_code does the TLS pairing handshake AND persists
      // the pinned certificate to this PC's config as part of the same
      // operation — see pairing.rs. We must not overwrite that pin with a
      // fresh config object below; instead we load what was just saved
      // and layer the remaining fields (host, mode, setup flag) on top.
      const result = await invoke<PairingCreds>("redeem_pairing_code", {
        serverIp: dbHost.trim(),
        code: pairingCode.trim(),
      });
      setCreds(result);
      toast.success("Connected! Credentials received.");
    } catch (err: unknown) {
      setCreds(null);
      const msg = String(err);
      setPairError(msg);
      toast.error(msg);
    } finally {
      setPairing(false);
    }
  };

  // const handleSaveAndInitialize = async () => {
  //   if (!creds) return;
  //   setInitializing(true);
  //   try {
  //     // Load the config redeem_pairing_code already wrote (it includes
  //     // pinned_server_cert_pem / pinned_server_fingerprint) and extend it,
  //     // rather than constructing a fresh object that would wipe the pin.
  //     const existing = await invoke<AppConfig | null>("get_config");
  //     const config: AppConfig = {
  //       ...(existing as AppConfig),
  //       mode: "client",
  //       db_host: dbHost.trim(),
  //       db_port: creds.db_port,
  //       db_user: creds.db_user,
  //       db_password: creds.db_password,
  //       db_name: creds.db_name,
  //       clinic_name: existing?.clinic_name ?? "",
  //       doctors_whatsapp_group: existing?.doctors_whatsapp_group ?? "",
  //       setup_complete: true,
  //     };

  //     await invoke("save_config", { config });
  //     toast.info("Connecting to the hospital server...");
  //     await invoke("initialize_database");

  //     toast.success("Setup complete!");
  //     onSetupComplete(config);
  //   } catch (err: any) {
  //     toast.error(`Setup failed: ${err}`);
  //     console.error(err);
  //   } finally {
  //     setInitializing(false);
  //   }
  // };

  // const handleSaveAndInitialize = async () => {
  //   if (!creds) return;
  //   setInitializing(true);
  //   try {
  //     // complete_pairing_and_connect does three things in one safe sequence:
  //     //   1. Reads the config that redeem_pairing_code already wrote
  //     //      (which contains the pinned cert PEM + fingerprint)
  //     //   2. Opens a real PostgreSQL connection to verify everything works,
  //     //      then sets setup_complete = true
  //     //   3. Initialises the full DB pool, runs migrations, starts scheduler
  //     //
  //     // We do NOT re-save the config here — doing so risks overwriting the
  //     // pinned_server_cert_pem that redeem_pairing_code stored, which would
  //     // break the SSL connection.
  //     await invoke("complete_pairing_and_connect");

  //     toast.success("Setup complete!");

  //     // Re-read the config so onSetupComplete gets the fully-populated
  //     // object (with server_ip, db_port, pinned cert, etc.)
  //     const finalConfig = await invoke<AppConfig>("get_config");
  //     onSetupComplete(finalConfig!);
  //   } catch (err: any) {
  //     toast.error(`Setup failed: ${err}`);
  //     console.error("complete_pairing_and_connect error:", err);
  //   } finally {
  //     setInitializing(false);
  //   }
  // };

 

  const handleSaveAndInitialize = async () => {
    if (!creds) return;
    setInitializing(true);

    // Show live progress from the backend
    const unlisten = await listen<string>("init_status", (event) => {
      toast.info(event.payload, { id: "init-progress" });
    });

    try {
      await invoke("complete_pairing_and_connect");
      toast.success("Setup complete!", { id: "init-progress" });
      const finalConfig = await invoke<AppConfig>("get_config");
      onSetupComplete(finalConfig!);
    } catch (err: unknown) {
      toast.error(`Setup failed: ${String(err)}`);
      console.error("complete_pairing_and_connect error:", err);
    } finally {
      unlisten();
      setInitializing(false);
    }
  };

  // ── Server build: this screen should not normally appear. PostgreSQL
  //    provisioning happens once, during installation (see windows/hooks.nsh),
  //    and the installer marks setup_complete before the app is ever
  //    launched. Seeing this usually means installation didn't finish
  //    correctly (installer not run as Administrator) — but it can also mean
  //    PostgreSQL is fine and only config.json itself went missing/corrupt
  //    (deleted, disk issue, etc). For that second case, repair_server_config
  //    lets an admin re-enter the DB password and recover without a full
  //    reinstall. ──
  if (BUILD_MODE === "server") {
    return <ServerRepairScreen />;
  }

  // ── Client build: pair with the reception PC ───────────────────────────
  return (
    <div className="flex-1 flex items-center justify-center h-full bg-background p-6">
      <motion.div initial={{ opacity: 0, scale: 0.96 }} animate={{ opacity: 1, scale: 1 }} transition={{ duration: 0.25 }}>
        <Card className="w-full max-w-lg surface-elevated border-0">
          <CardHeader className="space-y-1.5">
            <div className="h-10 w-10 bg-primary/10 rounded-xl flex items-center justify-center mb-1">
              <Laptop className="h-5 w-5 text-primary" />
            </div>
            <CardTitle className="text-display-lg">Connect to hospital server</CardTitle>
            <CardDescription>Ask reception for the server's IP address and a pairing code (Settings → Connect a new client PC).</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-1.5">
              <Label htmlFor="host" className="text-xs font-bold uppercase tracking-wider text-muted-foreground">
                Server IP address
              </Label>
              <Input
                id="host"
                placeholder="e.g. 192.168.1.10"
                value={dbHost}
                onChange={(e) => {
                  setDbHost(e.target.value);
                  setCreds(null);
                }}
                className="font-mono text-sm"
                autoFocus
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="code" className="text-xs font-bold uppercase tracking-wider text-muted-foreground">
                Pairing code
              </Label>
              <Input
                id="code"
                placeholder="e.g. XK4P9R"
                value={pairingCode}
                onChange={(e) => {
                  setPairingCode(e.target.value.toUpperCase());
                  setCreds(null);
                }}
                className="font-mono text-sm uppercase tracking-widest"
                maxLength={6}
              />
              <p className="text-[10px] text-muted-foreground">Codes expire after 10 minutes. Ask reception to generate a new one if yours doesn't work.</p>
            </div>

            {creds && (
              <div className="flex items-center gap-2 text-emerald-600 dark:text-emerald-500 text-sm font-semibold">
                <CheckCircle2 className="h-4 w-4" /> Paired successfully
              </div>
            )}
            {pairError && (
              <div className="flex items-center gap-2 text-rose-600 dark:text-rose-500 text-sm font-semibold">
                <AlertTriangle className="h-4 w-4" /> {pairError}
              </div>
            )}
          </CardContent>
          <CardFooter className="flex items-center justify-between border-t border-border pt-5">
            <Button variant="outline" onClick={handleRedeemCode} disabled={pairing || !dbHost.trim() || pairingCode.trim().length !== 6} className="rounded-full">
              {pairing ? <RefreshCw className="h-4 w-4 mr-2 animate-spin" /> : <KeyRound className="h-4 w-4 mr-2" />}
              Pair
            </Button>
            <Button onClick={handleSaveAndInitialize} disabled={initializing || !creds} className="rounded-full">
              {initializing ? <RefreshCw className="h-4 w-4 mr-2 animate-spin" /> : null}
              Save & continue <ArrowRight className="h-4 w-4 ml-2" />
            </Button>
          </CardFooter>
        </Card>
      </motion.div>
    </div>
  );
}

/**
 * Server build recovery screen.
 *
 * Reached when config.json is missing/incomplete on a machine that should
 * already be a provisioned HMS Server (see App.tsx `initError` handling).
 *
 * IMPORTANT: this build never asks a human for the PostgreSQL password —
 * it's auto-generated during install and written straight into
 * config.json, never shown on screen or logged anywhere. So if config.json
 * is gone, nobody — not the receptionist, not you — actually knows it.
 * There's nothing to type in here.
 *
 * The real fix already exists in windows/hooks.nsh: the installer checks
 * for an existing PostgreSQL data directory on every run. If it finds one
 * but no config.json (exactly this situation), it takes the
 * `run_setup_repair` path — resets the DB password to a fresh
 * auto-generated one via a temporary trust-auth window, and rewrites
 * config.json. Fully automatic, no prompts, and it never touches or drops
 * existing patient data (only credentials are reset). This screen's job is
 * just to point there clearly, since the running app itself deliberately
 * never requests elevation or touches the PostgreSQL service directly
 * (see pg_provision.rs) — only the elevated installer can do this repair.
 */
function ServerRepairScreen() {
  return (
    <div className="flex-1 flex items-center justify-center h-full bg-background p-6">
      <motion.div initial={{ opacity: 0, scale: 0.96 }} animate={{ opacity: 1, scale: 1 }} transition={{ duration: 0.25 }}>
        <Card className="w-full max-w-md surface-elevated border-0 text-center">
          <CardHeader>
            <div className="h-12 w-12 bg-amber-100 dark:bg-amber-950/30 rounded-2xl flex items-center justify-center mx-auto mb-2">
              <Server className="h-6 w-6 text-amber-600" />
            </div>
            <CardTitle className="text-display-md">Configuration is missing</CardTitle>
            <CardDescription className="text-left space-y-3">
              <span className="block">
                This app's configuration file wasn't found. Nothing here needs a password — PostgreSQL is
                fully managed by the installer.
              </span>
              <span className="block font-semibold text-foreground">To fix it:</span>
              <span className="block">
                1. Close this app.<br />
                2. Re-run the HMS Server installer — <span className="font-semibold">right-click → Run as
                administrator</span>.<br />
                3. The installer detects your existing database automatically and repairs the
                configuration on its own.
              </span>
              <span className="block text-xs text-muted-foreground pt-1">
                Your patient data is not affected — this only resets the app's connection credentials.
              </span>
            </CardDescription>
          </CardHeader>
        </Card>
      </motion.div>
    </div>
  );
}
