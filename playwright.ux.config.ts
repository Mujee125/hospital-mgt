import { defineConfig, devices } from "@playwright/test";

/**
 * UX-validation config (RCTF UX-2026-09-13).
 *
 * The default playwright.config.ts targets port 1420 with
 * `reuseExistingServer: true` — on this machine another application's
 * dev server was already squatting on 1420, so the smoke suite silently
 * tested THE WRONG APP (verified live: the page served on 1420 is
 * "KarachiPOS"). This config boots its OWN Vite instance on 1430 with
 * reuse disabled, guaranteeing the tests exercise this repository.
 */
export default defineConfig({
  testDir: "./e2e",
  // Serial: the boot flow (mock → license → DB → session) is timing-heavy;
  // parallel chromium instances starve each other and caused flaky boots.
  fullyParallel: false,
  workers: 1,
  timeout: 60_000,
  forbidOnly: !!process.env.CI,
  retries: 0,
  reporter: "line",
  use: {
    baseURL: "http://localhost:1430",
    trace: "off",
    screenshot: "only-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: "npx vite --port 1430 --strictPort",
    url: "http://localhost:1430",
    reuseExistingServer: false,
    timeout: 60_000,
  },
});
