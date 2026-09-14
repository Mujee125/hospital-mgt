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
export const MOCK_DATA: Record<string, unknown> = {
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
  // UX-2026-09-13: `me` returns the same LoginResponse shape ({user,
  // roles, permissions, must_change_password}) as login — see
  // auth.rs LoginResponse — so the smoke suite can exercise
  // AUTHENTICATED screens (dashboard, patients, profile dialog);
  // previously every test stopped at the login card.
  me: {
    user: {
      id: 1,
      username: "admin",
      full_name: "Ayesha Khan",
      email: null,
      is_active: true,
      must_change_password: false,
      last_login_at: null,
    },
    roles: ["super_admin"],
    permissions: [
      "dashboard.view", "patients.view", "patients.create", "patients.update",
      "appointments.view", "appointments.create",
      "doctors.view", "billing.view", "billing.create",
      "queue.view", "queue.manage", "ipd.view", "ipd.manage",
      "lab.view", "lab.order", "lab.result.manage", "lab.approve",
      "settings.manage", "users.view", "users.manage", "audit.view",
      "reports.view", "inventory.view", "prescriptions.create",
    ],
    must_change_password: false,
  },  // A patient with an allergy — drives the safety-banner tests.
  get_patient: {
    id: 1,
    first_name: "Bilal",
    last_name: "Ahmed",
    email: null,
    phone: "03001234567",
    date_of_birth: "1985-03-12",
    gender: "Male",
    address: null,
    created_at: "2026-01-01T00:00:00Z",
    mrn: "MRN-000123",
    blood_group: "O+",
    allergies: "Penicillin",
    chronic_conditions: "Type 2 diabetes",
    emergency_contact_name: null,
    emergency_contact_phone: null,
    insurance_provider: null,
    insurance_policy_number: null,
    status: "active",
    created_by_user_id: null,
  },
  get_patients_ehr: [
    {
      id: 1,
      first_name: "Bilal",
      last_name: "Ahmed",
      email: null,
      phone: "03001234567",
      date_of_birth: "1985-03-12",
      gender: "Male",
      address: null,
      created_at: "2026-01-01T00:00:00Z",
      mrn: "MRN-000123",
      blood_group: "O+",
      allergies: "Penicillin",
      chronic_conditions: "Type 2 diabetes",
      emergency_contact_name: null,
      emergency_contact_phone: null,
      insurance_provider: null,
      insurance_policy_number: null,
      status: "active",
      created_by_user_id: null,
    },
    ...Array.from({ length: 29 }, (_, i) => ({
      id: i + 2,
      first_name: `Patient${i + 2}`,
      last_name: `Number${i + 2}`,
      email: null,
      phone: `03001234000`,
      date_of_birth: "1990-01-01",
      gender: i % 2 ? "Female" : "Male",
      address: null,
      created_at: "2026-01-01T00:00:00Z",
      mrn: `MRN-000${(i + 2).toString().padStart(3, "0")}`,
      blood_group: null,
      allergies: null,
      chronic_conditions: null,
      emergency_contact_name: null,
      emergency_contact_phone: null,
      insurance_provider: null,
      insurance_policy_number: null,
      status: "active",
      created_by_user_id: null,
    })),
  ],
  get_patients: [
    {
      id: 1,
      first_name: "Bilal",
      last_name: "Ahmed",
      email: null,
      phone: "03001234567",
      date_of_birth: "1985-03-12",
      gender: "Male",
      address: null,
      created_at: "2026-01-01T00:00:00Z",
    },
    ...Array.from({ length: 29 }, (_, i) => ({
      id: i + 2,
      first_name: `Patient${i + 2}`,
      last_name: `Number${i + 2}`,
      email: null,
      phone: "03001234000",
      date_of_birth: "1990-01-01",
      gender: i % 2 ? "Female" : "Male",
      address: null,
      created_at: "2026-01-01T00:00:00Z",
    })),
  ],
  get_prescriptions: [],
  create_encounter: 1,
};

/** The session `me` mock in MOCK_DATA — overridable per test.
 *  NOTE: must be awaited — addInitScript is async; navigating before
 *  registration completes silently skips the script (the flaky-boot
 *  root cause found via the bisect run: __TAURI_INTERNALS__ ended up
 *  undefined on a raced navigation). */
export async function stubTauriInvoke(
  page: Page,
  overrides: Record<string, unknown> = {},
) {
  await page.addInitScript((mockData) => {
    const w = window as unknown as {
      __TAURI_INTERNALS__?: Record<string, unknown>;
      __TAURI__?: unknown;
    };

    const internals: Record<string, unknown> = {
      invoke: async (cmd: string) => {
        console.log(`[mock invoke] ${cmd}`);
        if (cmd in mockData) {
          return (mockData as Record<string, unknown>)[cmd];
        }
        // Default: return null for unmocked commands
        return null;
      },
      // Event-callback plumbing for @tauri-apps/api/event listen().
      // Returns a dummy id — unlisten just no-ops.
      transformCallback: () => 0,
      unregisterCallback: () => {},
      // Window identity for getCurrentWindow() in TitleBar.
      metadata: {
        currentWindow: { label: "main" },
        currentWebview: { label: "main" },
      },
    };

    w.__TAURI_INTERNALS__ = internals;

    // Event-plugin internals: listen() cleanup resolves
    // window.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener (a
    // separate global from __TAURI_INTERNALS__ — see @tauri-apps/api
    // event.js _unlisten). Without it, every TitleBar/AppShell unmount
    // crashed the ErrorBoundary the moment React strict remounts run
    // the effect cleanup (caught live via pageerror capture).
    const w2 = window as unknown as {
      __TAURI_EVENT_PLUGIN_INTERNALS__?: { unregisterListener: (e: string, id: number) => void };
    };
    w2.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
      unregisterListener: () => {},
    };

    // Also stub the @tauri-apps/api/core module's invoke
    // (the app imports { invoke } from "@tauri-apps/api/core" which
    // internally calls window.__TAURI_INTERNALS__.invoke)
    w.__TAURI__ = { invoke: internals.invoke };
  }, { ...MOCK_DATA, ...overrides });
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

// ── UX-2026-09-13 authenticated-surface tests ─────────────────────────────
// The `me` mock (LoginResponse shape) now lets the suite boot straight
// into the AppShell. These pin the new UX surfaces: the personalized
// dashboard greeting, the patient profile dialog with the allergy
// safety banner, and capture screenshots for the verification report.

test.describe("Authenticated surfaces (UX-2026-09-13)", () => {
  // NOTE: no beforeEach stub here — tests that need a different session
  // (e.g. the doctor-role test) must be the ONLY registration for their
  // context, so each test calls stubTauriInvoke explicitly with its own
  // overrides.

  test("dashboard greets the signed-in user with the KPI grid", async ({ page }) => {
    await stubTauriInvoke(page);
    await page.goto("/");
    await page.waitForTimeout(4500);

    // Personalized greeting (U-14) names the user from the session.
    await expect(page.getByText(/Good (morning|afternoon|evening), Ayesha/i)).toBeVisible();

    // KPI cards render and are keyboard-focusable buttons (U-12).
    const kpi = page.getByRole("button", { name: /Total patients/i });
    await expect(kpi).toBeVisible();
    await kpi.focus();
    await expect(kpi).toBeFocused();

    await page.screenshot({ path: "test-results/ux-dashboard.png", fullPage: false });
  });

  test("patient profile opens with the allergy safety banner and MRN identity", async ({ page }) => {
    await stubTauriInvoke(page);
    // HashRouter: in-app routes live after #, so navigate to /#/patients
    // (a bare /patients full reload re-boots to the dashboard route root).
    await page.goto("/#/patients");
    await page.waitForTimeout(4000);

    // The mock returns one patient (Bilal Ahmed, allergic to Penicillin).
    // Both the name link and the icon action open the profile — same
    // accessible name, so .first() resolves the strict-mode ambiguity.
    const viewProfile = page
      .getByRole("button", { name: /View medical profile for Bilal Ahmed/i })
      .first();
    await expect(viewProfile).toBeVisible();
    await viewProfile.click();

    // Identity strip: MRN two-identifier convention (F-02 carried into
    // the profile header).
    await expect(page.getByText(/MRN MRN-000123/i)).toBeVisible();

    // The ALLERGIES alert (U-01) — role=alert, prominent, text not color.
    const alert = page.getByRole("alert");
    await expect(alert).toBeVisible();
    await expect(alert.getByText("Penicillin")).toBeVisible();

    // Chronic conditions chip renders as context (not as a red alert).
    await expect(page.getByText("Type 2 diabetes")).toBeVisible();

    await page.screenshot({ path: "test-results/ux-patient-profile.png", fullPage: false });
  });

  test("vitals and ward surfaces stay reachable (no dead navigation)", async ({ page }) => {
    await stubTauriInvoke(page);
    await page.goto("/#/nursing");
    await page.waitForTimeout(4000);
    const bodyText = (await page.locator("body").innerText()).toLowerCase();
    expect(
      bodyText.includes("nursing station") || bodyText.includes("no admitted patients"),
    ).toBeTruthy();
  });

  test("doctor role sees the clinical work queue first (schedule before KPIs)", async ({ page }) => {
    // Doctor session: holds lab.approve (per the backend's doctor role
    // map in rbac.rs) so the results-review queue renders.
    await stubTauriInvoke(page, {
      me: {
        user: {
          id: 2,
          username: "drkhan",
          full_name: "Sara Khan",
          email: null,
          is_active: true,
          must_change_password: false,
          last_login_at: null,
        },
        roles: ["doctor"],
        permissions: [
          "dashboard.view", "patients.view", "patients.update",
          "appointments.view", "queue.view", "doctors.view",
          "ipd.view", "lab.view", "lab.order", "lab.approve",
          "radiology.view", "billing.view", "inventory.view",
          "reports.view", "audit.view", "prescriptions.create",
        ],
        must_change_password: false,
      },
      // One resulted order → the "awaiting your review" queue has a row.
      get_lab_orders: [
        {
          id: 101,
          patient_id: 1,
          encounter_id: null,
          ordered_by_doctor_id: null,
          ordered_by_user_id: null,
          status: "resulted",
          ordered_at: "2026-09-13T08:30:00Z",
          created_at: "2026-09-13T08:30:00Z",
          patient_name: "Bilal Ahmed",
          doctor_name: null,
          sample_barcode: "BC-101",
          sampled_at: "2026-09-13T09:00:00Z",
          sampled_by_user_id: null,
          approved_at: null,
          approved_by_user_id: null,
        },
      ],
    });

    await page.goto("/");
    await page.waitForTimeout(4500);

    // Work-queue layout: "Today's schedule" appears BEFORE the KPI grid
    // in the DOM (clinical roles, prompt §7 — a work queue, not a KPI wall).
    const schedulePos = await page
      .getByText("Today's schedule")
      .first()
      .boundingBox();
    const kpiPos = await page
      .getByRole("button", { name: /Total patients/i })
      .boundingBox();
    expect(schedulePos).not.toBeNull();
    expect(kpiPos).not.toBeNull();
    expect(schedulePos!.y).toBeLessThan(kpiPos!.y);

    // The results-review queue surfaces the resulted order.
    await expect(page.getByText(/Lab results awaiting your review/i)).toBeVisible();
    await expect(page.getByText("#101")).toBeVisible();

    await page.screenshot({ path: "test-results/ux-dashboard-doctor.png", fullPage: false });
  });
});

// ── Dialog responsiveness (UX-2026-09-13 follow-up) ─────────────────────────
// The shared DialogContent must center the Add-patient form on BOTH axes
// and size dynamically: full-width-minus-margins on phones, max-w capped
// on desktop, and internal scroll (never clipped) when the form is taller
// than the viewport (e.g. 150% Windows display scaling on small panels).

test.describe("Add-patient dialog is centered and responsive", () => {
  test.beforeEach(async ({ page }) => {
    await stubTauriInvoke(page);
  });

  /** Opens /#/patients and clicks "Add patient"; returns the dialog. */
  async function openAddPatient(page: import("@playwright/test").Page) {
    await page.goto("/#/patients");
    await page
      .getByRole("button", { name: /Add patient/i })
      .first()
      .click();
    const dialog = page.getByRole("dialog");
    await expect(dialog).toBeVisible();
    await expect(page.getByText("Register new patient")).toBeVisible();
    return dialog;
  }

  test("mobile (375x667): centered, margins respected, chips selectable", async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 667 });
    const dialog = await openAddPatient(page);
    const box = (await dialog.boundingBox())!;

    // Centered on both axes within 1px tolerance.
    expect(Math.abs(box.x + box.width / 2 - 375 / 2)).toBeLessThanOrEqual(1);
    expect(Math.abs(box.y + box.height / 2 - 667 / 2)).toBeLessThanOrEqual(1);
    // Dynamic width: viewport minus the 1rem side margins (16px each).
    expect(box.width).toBeCloseTo(375 - 32, 0);
    // Fully inside the viewport — nothing is clipped above the fold.
    expect(box.y).toBeGreaterThanOrEqual(0);
    expect(box.y + box.height).toBeLessThanOrEqual(667);
    // The 20-field form scrolls INTERNALLY: content overflows, the
    // centered box does not.
    const scroller = dialog.locator("div.min-h-0");
    await expect(scroller).toBeAttached();
    const scrolled = await scroller.evaluate(
      (el) => el.scrollHeight > el.clientHeight,
    );
    expect(scrolled).toBe(true);

    // Selectable allergy options: a preset chip toggles on.
    const penicillin = page.getByRole("button", { name: "Penicillin" }).first();
    await penicillin.click();
    await expect(penicillin).toHaveAttribute("aria-pressed", "true");

    await page.screenshot({ path: "test-results/ux-dialog-mobile.png" });
  });

  test("short viewport (1280x500, simulates 150% scaling): scrolls, stays centered", async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 500 });
    const dialog = await openAddPatient(page);
    const box = (await dialog.boundingBox())!;

    // Capped at the desktop width and the dynamic max-height.
    expect(box.width).toBeLessThanOrEqual(576 + 1); // sm:max-w-xl
    expect(box.height).toBeLessThanOrEqual(500 - 32 + 1); // calc(100dvh - 2rem)
    expect(Math.abs(box.y + box.height / 2 - 250)).toBeLessThanOrEqual(1);
    const scroller = dialog.locator("div.min-h-0");
    const scrolled = await scroller.evaluate(
      (el) => el.scrollHeight > el.clientHeight,
    );
    expect(scrolled).toBe(true);

    await page.screenshot({ path: "test-results/ux-dialog-short-viewport.png" });
  });

  test("desktop (1280x800): centered at the desktop width", async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    const dialog = await openAddPatient(page);
    const box = (await dialog.boundingBox())!;

    expect(Math.abs(box.x + box.width / 2 - 640)).toBeLessThanOrEqual(1);
    expect(Math.abs(box.y + box.height / 2 - 400)).toBeLessThanOrEqual(1);
    expect(box.width).toBeCloseTo(576, 0); // sm:max-w-xl

    await page.screenshot({ path: "test-results/ux-dialog-desktop.png" });
  });
});

// ── App frame: sidebar + titlebar stay fixed while content scrolls ──────────

test.describe("App frame: chrome stays fixed during content scroll", () => {
  test("scrolling the patients list moves ONLY <main> — sidebar and titlebar never move", async ({ page }) => {
    await stubTauriInvoke(page);
    // 30 mock patients → a 25-row page forces internal scrolling.
    await page.setViewportSize({ width: 1280, height: 800 });
    await page.goto("/#/patients");

    const sidebar = page.getByRole("complementary", { name: "Primary navigation" });
    const titlebar = page.getByRole("button", { name: "Minimize" });
    const main = page.locator("main");
    await expect(sidebar).toBeVisible();

    const sidebarBefore = (await sidebar.boundingBox())!;
    const titlebarBefore = (await titlebar.boundingBox())!;

    // Wheel over the middle of the content area.
    await page.mouse.move(640, 400);
    await page.mouse.wheel(0, 900);

    // The scroll happened INSIDE main…
    const mainScrolled = await main.evaluate((el) => el.scrollTop > 0);
    expect(mainScrolled).toBe(true);
    // …the document itself never scrolled (the h-dvh app-frame fix)…
    const docScrollY = await page.evaluate(() => window.scrollY);
    expect(docScrollY).toBe(0);
    // …and the sidebar + titlebar stayed exactly where they were.
    const sidebarAfter = (await sidebar.boundingBox())!;
    const titlebarAfter = (await titlebar.boundingBox())!;
    expect(Math.abs(sidebarAfter.y - sidebarBefore.y)).toBeLessThanOrEqual(1);
    expect(Math.abs(titlebarAfter.y - titlebarBefore.y)).toBeLessThanOrEqual(1);

    await page.screenshot({ path: "test-results/ux-app-frame.png" });
  });
});

// ── Doctor form: specialization options + Add + qualification chips ─────────
// Real-browser verdict on the + Add flow (the jsdom suite covers the data
// flow via the create payload; this verifies the visible trigger display).

test.describe("Doctor form: selectable specialization and qualifications", () => {
  test("add-new specialization selects it; qualification chips toggle", async ({ page }) => {
    await stubTauriInvoke(page);
    await page.goto("/#/doctors");
    await page
      .getByRole("button", { name: /Add doctor/i })
      .first()
      .click();
    const dialog = page.getByRole("dialog");
    await expect(dialog).toBeVisible();

    // The specialization dropdown renders (curated + DB options).
    const combobox = page.getByRole("combobox");
    await expect(combobox).toBeVisible();

    // + Add a NEW specialization inline.
    await page.getByRole("button", { name: /add new specialization option/i }).click();
    await page
      .getByPlaceholder("Type the new specialization")
      .fill("Cosmetic Surgery");
    await page.getByRole("button", { name: "Add specialization option" }).click();

    // The trigger now displays the session-added option (real-browser
    // check of the flushSync fix against Radix's bubble-select echo).
    await expect(combobox).toHaveText(/Cosmetic Surgery/);

    // Qualifications: preset chips toggle on.
    const mbbs = page.getByRole("button", { name: "MBBS" }).first();
    await mbbs.click();
    await expect(mbbs).toHaveAttribute("aria-pressed", "true");

    await page.screenshot({ path: "test-results/ux-doctor-form.png" });
  });
});
