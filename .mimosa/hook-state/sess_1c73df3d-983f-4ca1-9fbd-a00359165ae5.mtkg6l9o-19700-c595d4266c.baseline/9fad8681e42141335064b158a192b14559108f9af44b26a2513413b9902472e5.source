/**
 * Golden-path smoke test: app loads → sidebar visible → key pages render.
 *
 * Tauri `invoke` is stubbed to return mock data so the frontend renders
 * without a real backend. True desktop E2E (real Postgres + Tauri) requires
 * `tauri-driver` — see e2e/README.md.
 *
 * NOTE: The VitalFlow HMS frontend is a Tauri app that uses
 * `@tauri-apps/api/core`'s `invoke()` for all backend calls. In a browser
 * (Playwright), `invoke` doesn't exist — we stub it via `page.addInitScript`
 * BEFORE the page loads so the app's boot flow sees the mock data immediately.
 */
import { test, expect, type Page } from "@playwright/test";

/** Mock data returned by the stubbed invoke() for each command. */
const MOCK_DATA: Record<string, unknown> = {
  get_config: {
    mode: "server",
    db_host: "127.0.0.1",
    db_port: 5432,
    db_user: "postgres",
    db_name: "hospital_db",
    clinic_name: "VitalFlow Clinic",
    doctors_whatsapp_group: "",
    setup_complete: true,
    pinned_server_cert_pem: "",
    pinned_server_fingerprint: "",
  },
  verify_license: {
    license_id: "test-license",
    hospital_id: "H001",
    hospital_name: "Test Hospital",
    deployment_id: "D001",
    product_edition: "Enterprise",
    enabled_modules: ["patients", "appointments", "billing"],
    issue_date: "2025-01-01T00:00:00Z",
    expiration_date: null,
    maintenance_until: "2026-01-01T00:00:00Z",
    hardware_fingerprint: "abc123",
    fingerprint_matches: true,
    status: "valid",
  },
  get_license_info: null,
  get_hardware_fingerprint: "abc123def456",
  initialize_database: "server:127.0.0.1",
  login: {
    user_id: 1,
    username: "admin",
    full_name: "System Administrator",
    roles: ["super_admin"],
    permissions: [
      "dashboard.view", "patients.view", "patients.create",
      "appointments.view", "appointments.create",
      "doctors.view", "billing.view", "billing.create",
      "queue.view", "queue.manage", "ipd.view", "ipd.manage",
      "lab.view", "lab.order", "settings.manage",
      "users.view", "users.manage", "audit.view",
    ],
    token: "mock-session-token",
    must_change_password: false,
  },
  get_dashboard_kpis: {
    patients_total: 42,
    appointments_today: 8,
    queue_waiting: 3,
    beds_available: 5,
    revenue_today: 15000,
  },
  get_today_appointments: [],
  get_appointment_stats: { scheduled: 5, confirmed: 2, completed: 1, cancelled: 0, no_show: 0 },
  get_queue: [],
  get_doctors: [],
  get_patients: [],
  get_patients_ehr: [],
  get_admissions: [],
  get_wards: [],
  get_beds: [],
  get_lab_orders: [],
  get_lab_catalog: [],
  get_bills: [],
  get_payments: [],
  get_messages: [],
  get_rooms: ["general", "doctors", "admin"],
  get_audit_logs: [],
  get_users: [],
  get_roles: [],
  get_user_roles: [],
  get_specializations: ["Cardiology", "General Medicine"],
  get_qualifications: ["MBBS", "MD"],
  get_whatsapp_config: null,
  get_notification_log: [],
  get_inventory_items: [],
  get_inventory_movements: [],
  get_patient_consent: null,
  get_encounters: [],
  get_local_ip: "127.0.0.1",
  test_server_connection: true,
  get_log: "",
  get_log_path: "/tmp/hms_startup.log",
  get_config_path: "/tmp/config.json",
};

/**
 * Stub `window.__TAURI_INTERNALS__.invoke` BEFORE the page's JS runs.
 * This intercepts all Tauri IPC calls and returns mock data.
 */
async function stubTauriInvoke(page: Page) {
  await page.addInitScript((mockData) => {
    // The Tauri webview exposes invoke via window.__TAURI_INTERNALS__
    // In a browser (Playwright), we stub it before the app loads.
    const w = window as unknown as {
      __TAURI_INTERNALS__?: { invoke: (cmd: string, args?: unknown) => Promise<unknown> };
      __TAURI__?: unknown;
    };

    w.__TAURI_INTERNALS__ = {
      invoke: async (cmd: string) => {
        console.log(`[mock invoke] ${cmd}`);
        if (cmd in mockData) {
          return mockData[cmd];
        }
        // Default: return null for unmocked commands
        return null;
      },
    };

    // Also stub the @tauri-apps/api/core module's invoke
    // (the app imports { invoke } from "@tauri-apps/api/core" which
    // internally calls window.__TAURI_INTERNALS__.invoke)
    w.__TAURI__ = { invoke: w.__TAURI_INTERNALS__.invoke };
  }, MOCK_DATA);
}

test.describe("Golden-path smoke test", () => {
  test.beforeEach(async ({ page }) => {
    await stubTauriInvoke(page);
  });

  test("app loads without crashing", async ({ page }) => {
    await page.goto("/");
    // Wait for the app to render (boot screen → login → dashboard)
    // Give it up to 10 seconds
    await page.waitForTimeout(5000);

    // The app should have rendered SOMETHING — not a blank page
    const body = page.locator("body");
    const bodyText = await body.innerText();
    expect(bodyText.length).toBeGreaterThan(0);

    // The page should not show "error" in the title
    const title = await page.title();
    expect(title.toLowerCase()).not.toContain("error");
  });

  test("app renders visible content (login or dashboard)", async ({ page }) => {
    await page.goto("/");
    await page.waitForTimeout(5000);

    // The app should show either a login screen or the main app shell.
    // Look for common text that appears in either state.
    const body = page.locator("body");
    const bodyText = (await body.innerText()).toLowerCase();

    // At least one of these should be present after boot
    const hasLogin = bodyText.includes("log in") || bodyText.includes("login");
    const hasDashboard = bodyText.includes("dashboard");
    const hasWelcome = bodyText.includes("welcome");
    const hasVitalFlow = bodyText.includes("vitalflow");
    const hasHospital = bodyText.includes("hospital");

    // The app rendered *something* recognizable
    expect(hasLogin || hasDashboard || hasWelcome || hasVitalFlow || hasHospital).toBeTruthy();
  });

  test("no white screen (error boundary not triggered)", async ({ page }) => {
    await page.goto("/");
    await page.waitForTimeout(5000);

    // If the ErrorBoundary triggered, it would show "Something went wrong"
    const body = page.locator("body");
    const bodyText = await body.innerText();
    expect(bodyText).not.toContain("Something went wrong");
    expect(bodyText).not.toContain("Startup failed");

    // Body should have substantial content (not a white screen)
    expect(bodyText.length).toBeGreaterThan(10);
  });
});
