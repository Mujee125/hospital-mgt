import { Component, type ErrorInfo, type ReactNode } from "react";

/**
 * ErrorBoundary — top-level React error boundary (CR-14).
 *
 * Per QUAL-03: the previous app had NO error boundary anywhere, so a single
 * uncaught render error (malformed API response, undefined access, etc.)
 * crashed the entire app to a white screen — operationally critical for a
 * hospital reception PC.
 *
 * This boundary wraps the entire app at the root. On an uncaught error it
 * shows a recovery UI with:
 *   - A clear message (no stack trace leaked to the user)
 *   - A "Reload" button (calls location.reload())
 *   - An error ID (generated from the error hash) for support reporting
 *   - The error + component stack logged to the console for developers
 *
 * The boundary does NOT catch errors in:
 *   - Event handlers (use try/catch)
 *   - Async code (use try/catch in the async function)
 *   - Server-side rendering (N/A — this is a Tauri desktop app)
 */
interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
  errorId: string;
}

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false, error: null, errorId: "" };
  }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    // Generate a short error ID for support reporting (not a true hash, just
    // a timestamp + truncated message so the user can read it over the phone).
    const errorId = `HMS-${Date.now().toString(36).toUpperCase()}-${(
      error.message.length * 7 +
      error.name.length * 13
    )
      .toString(36)
      .toUpperCase()
      .slice(0, 4)}`;
    return { hasError: true, error, errorId };
  }

  override componentDidCatch(error: Error, errorInfo: ErrorInfo): void {
    // Log to console for developers (the Tauri webview devtools capture this).
    console.error("[HMS ErrorBoundary] Uncaught error:", error);
    console.error("[HMS ErrorBoundary] Component stack:", errorInfo.componentStack);
  }

  handleReload = (): void => {
    this.setState({ hasError: false, error: null, errorId: "" });
    window.location.reload();
  };

  handleDismiss = (): void => {
    // Allow the user to try to continue without a full reload — the boundary
    // resets and the next render may succeed (e.g. if the error was transient).
    this.setState({ hasError: false, error: null, errorId: "" });
  };

  override render(): ReactNode {
    if (!this.state.hasError) {
      return this.props.children;
    }

    const isProduction = !(import.meta as { env?: { DEV?: boolean } }).env?.DEV;

    return (
      <div
        className="flex h-screen w-screen items-center justify-center bg-background p-6"
        role="alert"
        aria-live="assertive"
      >
        <div className="w-full max-w-md rounded-[var(--radius-lg)] border border-border bg-card p-8 shadow-lg">
          <div className="mb-5 flex items-center gap-3">
            <div
              className="flex h-11 w-11 shrink-0 items-center justify-center rounded-full"
              style={{ background: "hsl(var(--destructive) / 0.12)" }}
            >
              <svg
                className="h-6 w-6"
                style={{ color: "hsl(var(--destructive))" }}
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                strokeWidth={2}
                aria-hidden="true"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="M12 9v3.75m0 3.75h.007M5.84 19.5h12.32a2.25 2.25 0 001.94-3.39l-6.16-10.66a2.25 2.25 0 00-3.88 0L3.9 16.11a2.25 2.25 0 001.94 3.39z"
                />
              </svg>
            </div>
            <div>
              <h1 className="text-display-md text-foreground">
                Something went wrong
              </h1>
              <p className="text-sm text-muted-foreground mt-0.5">
                The application encountered an unexpected error.
              </p>
            </div>
          </div>

          <p className="text-sm text-muted-foreground leading-relaxed mb-5">
            You can try reloading the application. If the problem persists,
            please contact your system administrator and provide the error
            reference below.
          </p>

          <div className="mb-6 rounded-[var(--radius-sm)] border border-border bg-muted/40 px-4 py-3">
            <p className="text-[11px] font-semibold text-muted-foreground uppercase tracking-wide mb-1">
              Error Reference
            </p>
            <p className="font-mono text-sm text-foreground select-all">
              {this.state.errorId}
            </p>
          </div>

          {!isProduction && this.state.error && (
            <details className="mb-5 rounded-[var(--radius-sm)] border border-border bg-muted/30 px-4 py-3">
              <summary className="cursor-pointer text-xs font-semibold text-muted-foreground uppercase tracking-wide">
                Developer details
              </summary>
              <p className="mt-2 text-xs font-mono text-foreground whitespace-pre-wrap break-all">
                {this.state.error.name}: {this.state.error.message}
              </p>
            </details>
          )}

          <div className="flex gap-3">
            <button
              type="button"
              onClick={this.handleReload}
              className="flex-1 inline-flex items-center justify-center gap-2 rounded-[var(--radius)] bg-primary px-4 py-2.5 text-sm font-semibold text-primary-foreground transition-colors hover:bg-primary-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              <svg
                className="h-4 w-4"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                strokeWidth={2}
                aria-hidden="true"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="M16.023 9.348h4.992V4.356M19.65 9.405a8.25 8.25 0 10-9.99 9.99M3 14.652v4.992h4.992"
                />
              </svg>
              Reload Application
            </button>
            <button
              type="button"
              onClick={this.handleDismiss}
              className="inline-flex items-center justify-center rounded-[var(--radius)] border border-border bg-card px-4 py-2.5 text-sm font-semibold text-foreground transition-colors hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              Try to Continue
            </button>
          </div>
        </div>
      </div>
    );
  }
}
