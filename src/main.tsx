import React from "react";
import ReactDOM from "react-dom/client";
import { HashRouter } from "react-router-dom";
import { QueryClient, QueryClientProvider, QueryCache } from "@tanstack/react-query";
import { toast } from "sonner";
import App from "./App";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import "./index.css";

// QA-2026-09-08 H1: every read-path IPC failure was silently dropped —
// only Reports.tsx rendered query errors, so a database outage on a
// client PC showed cheerful "No X registered yet" empty states instead
// of an error. In a hospital that misleads staff about whether data
// exists. A single QueryCache-level onError makes every unhandled query
// failure visible. The auth probes are excluded (they fire on every
// logout / expired session by design and have their own handling in
// lib/auth.tsx).
const isAuthProbe = (queryKey: readonly unknown[]) =>
  Array.isArray(queryKey) &&
  queryKey[0] === "auth";

const queryClient = new QueryClient({
  queryCache: new QueryCache({
    onError: (error, query) => {
      if (isAuthProbe(query.queryKey)) return;
      toast.error("Couldn't load data", {
        description: String(error),
        duration: 6000,
      });
    },
  }),
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      refetchOnWindowFocus: true,
      retry: 1,
    },
  },
});

// CR-14: ErrorBoundary wraps the entire app at the root so an uncaught
// render error (malformed API response, undefined access) shows a recovery
// UI instead of a white screen — critical for a hospital reception PC.
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <QueryClientProvider client={queryClient}>
        <HashRouter>
          <App />
        </HashRouter>
      </QueryClientProvider>
    </ErrorBoundary>
  </React.StrictMode>,
);
