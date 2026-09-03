/**
 * Login screen — restyled after mayoclinic.org: a thin navy top rule,
 * a flat (non-gradient) navy shield mark, serif brand lockup, and a
 * crisp white card with sharp corners instead of the soft rounded/
 * gradient look. Security copy stays generic to avoid leaking account
 * existence. Behavior is unchanged from the previous version.
 */
import { useState } from "react";
import { motion } from "motion/react";
import { Loader2, Lock, User, ArrowRight, ShieldCheck } from "lucide-react";
import { useAuth } from "@/lib/auth";
import logo from "@/assets/logo_transparant.png";
export function Login({ hospitalName }: { hospitalName?: string }) {
  const { login } = useAuth();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await login(username.trim(), password);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex h-full w-full flex-col bg-background">
      <div className="flex flex-1 items-center justify-center p-6 gradient-mesh">
        <motion.div
          initial={{ opacity: 0, y: 12 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.3, ease: [0.22, 1, 0.36, 1] }}
          className="w-full max-w-[440px]"
        >
          {/* Brand */}
          <div className="flex flex-col items-center text-center pb-2">
            <div
              data-tauri-drag-region
              className="flex items-center gap-2 pl-3 pr-4 shrink-0 "
              
            >
              <img src={logo} alt="VitalFlow HMS Logo" className="w-36 h-36 object-contain" />
           
            </div>
            <div>
              <h1 className="text-display-xl text-foreground">Welcome</h1>
              <p className="text-sm text-muted-foreground mt-2 font-medium">
                Log in to your {hospitalName ?? "VitalFlow HMS"} account
              </p>
            </div>
          </div>

          {/* Card — bordered, minimal shadow, matching the real Mayo login card */}
          <div className="auth-card">
            <form onSubmit={submit} className="form-stack">
              <div className="flex flex-col gap-2">
                <label
                  htmlFor="username"
                  className="text-sm font-semibold text-foreground"
                >
                  Username
                </label>
                <div className="relative group">
                  <User className="absolute left-4 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground group-focus-within:text-primary transition-colors" />
                  <input
                    id="username"
                    type="text"
                    autoComplete="username"
                    value={username}
                    onChange={(e) => setUsername(e.target.value)}
                    required
                    autoFocus
                    className="w-full h-12 pl-11 pr-4 bg-card border border-border rounded-[var(--radius)] text-sm outline-none transition-all focus:border-primary/50 focus:ring-2 focus:ring-primary/15 placeholder:text-muted-foreground/60"
                    placeholder="e.g. admin"
                  />
                </div>
              </div>

              <div className="flex flex-col gap-2">
                <label
                  htmlFor="password"
                  className="text-sm font-semibold text-foreground"
                >
                  Password
                </label>
                <div className="relative group">
                  <Lock className="absolute left-4 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground group-focus-within:text-primary transition-colors" />
                  <input
                    id="password"
                    type="password"
                    autoComplete="current-password"
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    required
                    className="w-full h-12 pl-11 pr-4 bg-card border border-border rounded-[var(--radius)] text-sm outline-none transition-all focus:border-primary/50 focus:ring-2 focus:ring-primary/15 placeholder:text-muted-foreground/60"
                    placeholder="••••••••"
                  />
                </div>
              </div>

              {error && (
                <motion.div
                  initial={{ opacity: 0, height: 0 }}
                  animate={{ opacity: 1, height: "auto" }}
                  className="flex items-start gap-2.5 px-4 py-3 rounded-[var(--radius)] bg-destructive/8 border border-destructive/20 text-xs text-destructive"
                >
                  <span className="font-medium">{error}</span>
                </motion.div>
              )}

              <button
                type="submit"
                disabled={busy}
                className="h-12 mt-1 rounded-full bg-primary text-primary-foreground text-[15px] font-semibold hover:bg-[hsl(var(--primary-hover))] hover:shadow-md transition-all shadow-sm disabled:opacity-50 disabled:shadow-none flex items-center justify-center gap-2 group w-full"
              >
                {busy ? (
                  <>
                    <Loader2 className="h-4 w-4 animate-spin" /> Signing in…
                  </>
                ) : (
                  <>
                    Log in{" "}
                    <ArrowRight className="h-4 w-4 transition-transform group-hover:translate-x-0.5" />
                  </>
                )}
              </button>
            </form>
          </div>

          {/* First-run hint */}
          <div className="mt-8 px-5 py-4 rounded-[var(--radius-md)] bg-card border border-border">
            <div className="flex items-center gap-2 mb-1.5">
              <ShieldCheck className="h-3.5 w-3.5 text-primary" />
              <span className="text-xs font-semibold text-foreground">
                First-time setup
              </span>
            </div>
            <p className="text-[11px] text-muted-foreground leading-relaxed">
              The bootstrap administrator password is randomly generated at
              install time and written to{" "}
              <code className="px-1.5 py-0.5 rounded bg-muted font-mono text-[10px]">
                C:\ProgramData\HMS\bootstrap-credentials.txt
              </code>
              . Read it with an Administrator account, log in, and you will be
              required to change it immediately.
            </p>
          </div>

          <p className="text-[11px] text-muted-foreground/70 text-center mt-8">
            Authorized personnel only · All access is audited · HIPAA-aligned
          </p>
        </motion.div>
      </div>
    </div>
  );
}
