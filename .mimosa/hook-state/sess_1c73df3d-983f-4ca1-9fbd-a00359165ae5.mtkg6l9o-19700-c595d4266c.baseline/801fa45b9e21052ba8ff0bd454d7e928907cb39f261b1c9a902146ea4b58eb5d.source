import { Loader2 } from "lucide-react";
import RasheedMedicalLogo from "@/components/RasheedMedicalLogo";

interface LoadingScreenProps {
  /**
   * Optional label shown under the spinner. Defaults to "Loading…".
   * Use to give the user a hint about which chunk is downloading
   * (e.g. "Loading billing…") when one is known.
   */
  label?: string;
}

/**
 * Full-screen loading indicator shown inside `<Suspense fallback={…}>`
 * while a lazy-loaded route chunk is being fetched.
 *
 * Visual treatment matches the boot screen (App.tsx::BootScreen):
 * gradient-mesh background, centred brand lockup, single primary
 * spinner. Kept dependency-free (no motion / no Tauri calls) so it
 * can render synchronously on first paint of any lazy boundary.
 */
export function LoadingScreen({ label = "Loading…" }: LoadingScreenProps) {
  return (
    <div
      className="flex h-full w-full flex-col items-center justify-center bg-background gradient-mesh select-none gap-5"
      role="status"
      aria-live="polite"
    >
      <RasheedMedicalLogo className="w-14 h-14" />
      <div className="flex items-center gap-2.5">
        <Loader2 className="h-4 w-4 text-primary animate-spin shrink-0" />
        <span className="text-sm font-medium text-muted-foreground">{label}</span>
      </div>
      <span className="sr-only">{label}</span>
    </div>
  );
}

export default LoadingScreen;
