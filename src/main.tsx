import React from "react";
import ReactDOM from "react-dom/client";
import { HashRouter } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import App from "./App";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import "./index.css";

// One shared query client for the whole app. Tuned conservatively for a
// LAN-local desktop app talking to its own Postgres: data changes when
// staff act on it, not from external sources, so we don't need aggressive
// background refetching — but we do want fast invalidation right after a
// mutation (create/update/delete), which each page's mutations trigger
// explicitly via queryClient.invalidateQueries.
const queryClient = new QueryClient({
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
