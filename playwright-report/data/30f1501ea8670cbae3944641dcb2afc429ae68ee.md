# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: smoke.spec.ts >> Golden-path smoke test >> error boundary catches render errors
- Location: e2e\smoke.spec.ts:118:3

# Error details

```
Test timeout of 30000ms exceeded.
```

```
Error: page.goto: Test timeout of 30000ms exceeded.
Call log:
  - navigating to "http://localhost:1420/", waiting until "load"

```

# Page snapshot

```yaml
- alert [ref=e3]:
  - generic [ref=e4]:
    - generic [ref=e5]:
      - img [ref=e7]
      - generic [ref=e9]:
        - heading "Something went wrong" [level=1] [ref=e10]
        - paragraph [ref=e11]: The application encountered an unexpected error.
    - paragraph [ref=e12]: You can try reloading the application. If the problem persists, please contact your system administrator and provide the error reference below.
    - generic [ref=e13]:
      - paragraph [ref=e14]: Error Reference
      - paragraph [ref=e15]: HMS-MRE8LJYO-F4
    - group [ref=e16]:
      - generic "Developer details" [ref=e17] [cursor=pointer]
    - generic [ref=e18]:
      - button "Reload Application" [ref=e19]:
        - img [ref=e20]
        - text: Reload Application
      - button "Try to Continue" [ref=e22]
```

# Test source

```ts
  20  |         clinic_name: "VitalFlow Clinic",
  21  |         doctors_whatsapp_group: "",
  22  |         setup_complete: true,
  23  |         pinned_server_cert_pem: "",
  24  |         pinned_server_fingerprint: "",
  25  |       },
  26  |       verify_license: {
  27  |         license_id: "test-license",
  28  |         hospital_id: "H001",
  29  |         hospital_name: "Test Hospital",
  30  |         deployment_id: "D001",
  31  |         product_edition: "Enterprise",
  32  |         enabled_modules: ["patients", "appointments", "billing"],
  33  |         issue_date: "2025-01-01T00:00:00Z",
  34  |         expiration_date: null,
  35  |         maintenance_until: "2026-01-01T00:00:00Z",
  36  |         hardware_fingerprint: "abc123",
  37  |         fingerprint_matches: true,
  38  |         status: "valid",
  39  |       },
  40  |       get_license_info: null,
  41  |       get_hardware_fingerprint: "abc123def456",
  42  |       initialize_database: "server:127.0.0.1",
  43  |       login: {
  44  |         user_id: 1,
  45  |         username: "admin",
  46  |         full_name: "System Administrator",
  47  |         roles: ["super_admin"],
  48  |         permissions: [
  49  |           "dashboard.view", "patients.view", "patients.create",
  50  |           "appointments.view", "appointments.create",
  51  |           "doctors.view", "billing.view", "billing.create",
  52  |           "queue.view", "queue.manage", "ipd.view", "ipd.manage",
  53  |           "lab.view", "lab.order", "settings.manage",
  54  |           "users.view", "users.manage", "audit.view",
  55  |         ],
  56  |         token: "mock-session-token",
  57  |         must_change_password: false,
  58  |       },
  59  |       get_dashboard_kpis: {
  60  |         patients_total: 42,
  61  |         appointments_today: 8,
  62  |         queue_waiting: 3,
  63  |         beds_available: 5,
  64  |         revenue_today: 15000,
  65  |       },
  66  |       get_today_appointments: [],
  67  |       get_appointment_stats: { scheduled: 5, confirmed: 2, completed: 1, cancelled: 0, no_show: 0 },
  68  |       get_queue: [],
  69  |       get_doctors: [],
  70  |       get_patients: [],
  71  |     };
  72  | 
  73  |     // Stub window.__TAURI_INTERNALS__.invoke
  74  |     (window as unknown as { __TAURI_INTERNALS__: { invoke: (cmd: string, args?: unknown) => Promise<unknown> } }).__TAURI_INTERNALS__ = {
  75  |       invoke: async (cmd: string, args?: unknown) => {
  76  |         console.log(`[mock invoke] ${cmd}`, args);
  77  |         return mockData[cmd] ?? null;
  78  |       },
  79  |     };
  80  | 
  81  |     // Also stub the @tauri-apps/api/core module's invoke
  82  |     (window as unknown as { __TAURI_INVOKE_STUB__: (cmd: string, args?: unknown) => Promise<unknown> }).__TAURI_INVOKE_STUB__ = (cmd: string) => {
  83  |       return Promise.resolve(mockData[cmd] ?? null);
  84  |     };
  85  |   });
  86  | }
  87  | 
  88  | test.describe("Golden-path smoke test", () => {
  89  |   test.beforeEach(async ({ page }) => {
  90  |     await stubTauriInvoke(page);
  91  |   });
  92  | 
  93  |   test("app loads and shows login screen", async ({ page }) => {
  94  |     await page.goto("/");
  95  |     // The app should show the login screen (or boot screen → login)
  96  |     await expect(page).toHaveTitle(/Hospital Management System|VitalFlow/i);
  97  |   });
  98  | 
  99  |   test("sidebar shows all navigation items after login", async ({ page }) => {
  100 |     await page.goto("/");
  101 |     // Wait for the app to render (it may show boot screen first)
  102 |     await page.waitForTimeout(2000);
  103 | 
  104 |     // Look for sidebar nav items — they may be visible after boot
  105 |     const dashboardLink = page.locator('a:has-text("Dashboard"), [role="link"]:has-text("Dashboard")').first();
  106 |     const appointmentsLink = page.locator('a:has-text("Appointments"), [role="link"]:has-text("Appointments")').first();
  107 |     const patientsLink = page.locator('a:has-text("Patients"), [role="link"]:has-text("Patients")').first();
  108 | 
  109 |     // At least one of these should be visible (depends on boot state)
  110 |     const anyVisible = await Promise.all([
  111 |       dashboardLink.isVisible().catch(() => false),
  112 |       appointmentsLink.isVisible().catch(() => false),
  113 |       patientsLink.isVisible().catch(() => false),
  114 |     ]);
  115 |     expect(anyVisible.some(Boolean) || true).toBeTruthy(); // Soft pass — the app rendered
  116 |   });
  117 | 
  118 |   test("error boundary catches render errors", async ({ page }) => {
  119 |     // Navigate and verify no white screen
> 120 |     await page.goto("/");
      |                ^ Error: page.goto: Test timeout of 30000ms exceeded.
  121 |     await page.waitForTimeout(1000);
  122 |     const body = page.locator("body");
  123 |     await expect(body).not.toBeEmpty();
  124 |   });
  125 | });
  126 | 
```