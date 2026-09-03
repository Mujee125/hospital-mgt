import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "node:path";

// https://vitejs.dev/config/
//
// Tauri v2 conventions:
//   - Dev server on port 1420 (strictPort — Tauri expects exactly this).
//   - HMR on a separate ws port (1421) to avoid clashing with the Tauri IPC.
//   - Watcher ignores src-tauri/ (Rust side rebuilds separately).
//   - clearScreen: false so Tauri's own log lines stay visible.
//
// The @/* path alias mirrors tsconfig.json paths and is required by 30+ files
// across src/ (Security Matrix A.8.25 / SRS NFR-50 strict-gate dependency).
const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [react(), tailwindcss()],

  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },

  // Vite env variables prefixed with VITE_HMS_ are exposed to the client.
  // (VITE_HMS_BUILD_MODE = "server" | "client" drives conditional code paths.)
  envPrefix: ["VITE_HMS_"],

  // Tauri requires a deterministic port + webkit-friendly HMR.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? { protocol: "ws", host, port: 1421 }
      : undefined,
    watch: {
      // Ignore Rust source — Tauri rebuilds it independently.
      ignored: ["**/src-tauri/**"],
    },
  },
}));
