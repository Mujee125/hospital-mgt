/// <reference types="vite/client" />

// Per SRS NFR-50 / Security Matrix A.8.25 — typed Vite env.
//
// The HMS frontend is built in three modes driven by VITE_HMS_BUILD_MODE:
//   - undefined  → dev/fallback (single-machine, local Postgres)
//   - "server"   → server build (bundles + provisions PostgreSQL)
//   - "client"   → client build (pairs with a server over LAN)
interface ImportMetaEnv {
  readonly VITE_HMS_BUILD_MODE?: "server" | "client";
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
