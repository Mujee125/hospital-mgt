import { defineConfig, devices } from "@playwright/test";

/**
 * Playwright E2E config for VitalFlow HMS.
 *
 * Tests run against the Vite dev server (port 1420). Tauri `invoke` calls
 * are stubbed via `page.addInitScript` in each test — true desktop E2E
 * (with a real Postgres backend) requires `tauri-driver` (planned future).
 */
export default defineConfig({
  testDir: "./e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: "html",
  use: {
    baseURL: "http://localhost:1420",
    trace: "on-first-retry",
  },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
  ],
  webServer: {
    command: "npm run dev",
    url: "http://localhost:1420",
    reuseExistingServer: !process.env.CI,
    timeout: 30_000,
  },
});
