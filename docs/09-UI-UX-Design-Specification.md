# VitalFlow HMS — UI/UX Design Specification

> **Document version: v0.2.0 — updated 2025-07-08 after Phase 2 Batches 0-3 implementation. See CHANGELOG.md for details.**

| Field             | Value                                                                 |
| ----------------- | --------------------------------------------------------------------- |
| **Document title**| VitalFlow HMS — UI/UX Design Specification                            |
| **Document ID**   | VITALFLOW-DOC-09                                                      |
| **Version**       | 0.2.0 (reconciled 2025-07-08 by Documentation Team — B4-C)            |
| **Date**          | 2025-07-08                                                            |
| **Status**        | Draft                                                                 |
| **Classification**| Internal                                                              |
| **Owner**         | Healthcare UX Architect                                               |
| **Stack**         | Tauri 2.x · Rust · PostgreSQL · React 19 · TypeScript · Tailwind CSS 4 · shadcn/ui · Lucide · Motion |
| **Source of truth**| `src/index.css`, `src/lib/rbac.ts`, `src-tauri/src/rbac.rs`          |
| **Related docs**  | `01-SRS-Software-Requirements.md`, `02-SDD-Software-Design.md`, `03-Quality-Model-ISO-25010.md`, `04-Security-Control-Matrix-ISO-27001.md`, `07-Licensing-Architecture.md`, `08-Deployment-Installation-Guide.md`, `DESIGN_SYSTEM.md` (superseded) |

> **Reading guide.** This document specifies the user experience of the VitalFlow HMS native Windows desktop application across 20 design domains. Every section follows the RCTF pattern (Objective · Design Principles · Functional Requirements · UX Considerations · Desktop Layout · Component Specs · Interaction Patterns · Accessibility · Performance · Best Practices · Implementation Notes · Future Opportunities). Phase 1 modules already present in the working tree are marked **[IMPLEMENTED]**; Phase 2 surfaces are marked **[PLANNED]**. Token references (e.g. `--primary: 199 89% 48%`) cite the live `src/index.css`.
>
> **Re-skin confirmation (v0.2.0 Batch 2 CR-18).** This spec mandated the VitalFlow palette: sky-blue primary `#0EA5E9` + teal accent `#14B8A6` + Inter typography. Batch 2 CR-18 re-skinned the running implementation to match — the previous Mayo-navy design is retired. The live `src/index.css` now carries the comment `/* CR-18: re-skinned from Mayo navy to spec palette. */` and every token in §2.5 below is the actual value shipped. Where the implementation now matches the spec, this revision marks it `[Implemented v0.2.0]`; where the implementation still diverges, the gap is noted as a Batch 5 follow-up.
>
> **Dynamic branding (v0.2.0 Batch 3 DS-04).** Hospital name branding is now dynamic — `App.tsx` reads `licenseInfo?.hospital_name` and falls back to `"VitalFlow HMS"` / `"Hospital Management System"` when no license is loaded. The previous static "Rasheed Medical Center" / `clinic_name`-only branding is retired. See §1.4 Trust signals + §3.5.2 Header.

---

## Table of Contents

1. Design Philosophy
2. Design System
3. Application Layout
4. Role-Based Dashboards
5. Patient Management (360° Patient View)
6. Appointment & Queue Management
7. Clinical Workflows
8. Department-Specific Interfaces
9. Interactive Bed Management
10. Billing & Financial Interfaces
11. Inventory & Pharmacy
12. Analytics & Reporting
13. Global Search
14. Accessibility (WCAG 2.2 AA)
15. Keyboard Productivity
16. Notifications & Alerts
17. AI-Assisted Features
18. Performance & Responsiveness
19. Error Prevention & Auditability
20. Future Readiness

Appendix A — Glossary
Appendix B — Token Quick Reference
Appendix C — Cross-Reference Matrix

---

## 1. Design Philosophy

### 1.1 Objective
Establish the north-star design intent for a hospital management system used 24×7 by clinicians, ward staff, pharmacists, lab technicians, billing clerks and administrators on a native Windows desktop. The UI must reduce cognitive load during high-pressure clinical work while preserving the rigor demanded by ISO/IEC 27001 access control, ISO/IEC 25010 usability, and HIPAA "minimum necessary" disclosure principles.

### 1.2 Design Principles
| # | Principle | Manifestation in VitalFlow |
|---|-----------|----------------------------|
| P1 | **Clinical clarity over decoration** | Body text 14px Inter, tabular-nums for all numeric data, status colors mapped to clinical lifecycle, no decorative motion. |
| P2 | **Role-aware, never cluttered** | Sidebar filters by `requiredPermission`; the receptionist never sees `Users & Roles`; the doctor never sees `License Manage`. |
| P3 | **Error prevention before error recovery** | Critical actions (discharge, delete patient, bill void) require confirm dialogs; soft-deletes are preferred. |
| P4 | **Audit by default** | Every write action calls `audit::record()` server-side; the UI surfaces "Last action" hints where relevant. |
| P5 | **Motion serves meaning** | Motion library is used only for state transitions (page enter/exit, sidebar pill, KPI mount). `prefers-reduced-motion` collapses all transitions to 0.01ms. |
| P6 | **Density without crowding** | Default body 14px / line-height 1.5rem; tables use 8px vertical rhythm; cards use 5xl–6xl padding. |
| P7 | **Two themes, one source** | Light + dark share HSL tokens; deep-slate dark theme eases night-shift eye strain (`--background: 222 47% 7%`). |
| P8 | **Approachability** | Warm sky-blue primary (#0EA5E9) and teal accent (#14B8A6) signal "medical" without the institutional coldness of Mayo navy. |
| P9 | **Offline-resilient UX** | React Query caches each list for 5 minutes; a graceful "stale" indicator appears when the backend is unreachable. |
| P10 | **Desktop-first, single-window** | All flows assume a 1366×768 minimum viewport on Windows; no multi-window dialog sprawl. |

### 1.3 Functional Requirements
- FR-UX-0001 The shell shall render in **≤ 100 ms** after Tauri webview handshake.
- FR-UX-0002 The system shall expose **8 roles** with permission-filtered navigation: `super_admin`, `doctor`, `nurse`, `receptionist`, `lab_technician`, `pharmacist`, `billing_clerk`, `patient`.
- FR-UX-0003 The system shall never display an action button the user cannot perform; backend `rbac::require()` is the second line of defense.
- FR-UX-0004 The system shall persist sidebar collapse state in `localStorage["hms-sidebar-collapsed"]`.
- FR-UX-0005 The system shall expose a global page-transition animation ≤ 200 ms duration.

### 1.4 UX Considerations
- **Shift fatigue:** Night-shift clinicians benefit from the dark theme; the system auto-detects `prefers-color-scheme` but always allows manual override via `ThemeToggle`.
- **Interruption recovery:** The boot state machine (`checkingSetup → needsSetup → verifyingLicense → licenseError → booting → ready → initError`) guarantees the user never lands in a half-initialised shell.
- **Trust signals:** The footer of the boot screen prints "Licensed to {hospital_name} · {product_edition} edition" so clinicians know they are operating within their license scope. **[Implemented v0.2.0 (Batch 3 DS-04)]** `{hospital_name}` is read from `licenseInfo.hospital_name` (set by the signed license file); the fallback is `"VitalFlow HMS"` / `"Hospital Management System"` when no license is loaded. Branding is therefore per-customer (each hospital sees their own name in the header, login hero, force-change-password screen, and boot footer) — the previous static "Rasheed Medical Center" branding is retired.

### 1.5 Desktop Layout
```
+------------------------------------------------------------------------+
| ▣ VitalFlow HMS                       Hospital Portal  [search] [bell] |
+--------+---------------------------------------------------------------+
| ▣ Logo |  Welcome back — KPIs — Today's schedule — Queue now           |
| > Dash |                                                               |
| > Appt |                                                               |
| > Pat. |                                                               |
| > Queue|                                                               |
| > IPD  |                                                               |
| > Lab  |                                                               |
| > Bill |                                                               |
| > Chat |                                                               |
| > Audit|                                                               |
| > Users|                                                               |
| > Rpts |                                                               |
| > Sett |                                                               |
| [user] |                                                               |
+--------+---------------------------------------------------------------+
```

### 1.6 Component Specs
- Root layout: `flex h-screen w-screen bg-background text-foreground overflow-hidden` (see `AppShell.tsx`).
- Main scroll container: `max-w-[1400px] mx-auto px-4 sm:px-6 lg:px-8 py-6 lg:py-8`.
- Header height: `--header-height: 64px`.
- Sidebar width: `--sidebar-width: 264px` (expanded), `--sidebar-width-collapsed: 72px`.

### 1.7 Interaction Patterns
- **Page transition:** `motion.div` initial `{opacity:0, y:6}` → animate `{opacity:1, y:0}` → exit `{opacity:0, y:-6}`, 180 ms easeOut.
- **Sidebar pill:** `motion.div layoutId="sidebar-active-pill"` with spring `{stiffness:400, damping:32}`.
- **Refresh nudge:** `window.dispatchEvent(new CustomEvent("hms:refresh"))` from the Header refresh button; pages listen and call `queryClient.invalidateQueries`.

### 1.8 Accessibility
- All interactive elements expose `aria-label` (e.g. menu button: `aria-label="Open navigation menu"`).
- Color is never the sole status indicator — every `status-badge` has a text label.
- Focus ring uses `--shadow-glow: 0 0 0 3px rgb(14 165 233 / 0.12)` which exceeds WCAG 2.2 AA contrast against all surfaces.

### 1.9 Performance
- Initial JS payload budget: < 350 KB gzipped (Tauri bundling, no SSR).
- Per-route code-splitting via `React.lazy` is **[PLANNED]** for Phase 2 to further reduce initial parse.
- React Query default `staleTime: 5min`, `refetchOnWindowFocus: true`.

### 1.10 Best Practices
- Keep forms to **≤ 7 visible fields** per dialog; use `Tabs` for multi-section forms.
- Every secondary action is `variant="outline"`; only one primary action per surface.
- Use `capitalize` class sparingly; never on user-entered free text.

### 1.11 Implementation Notes
- Theme toggling is class-based (`html.dark`); the `ThemeToggle` component writes to `localStorage["hms-theme"]` and the root `<html>`.
- The `motion` package (Motion) is preferred over `framer-motion` per the README decision record.
- HashRouter is used (not BrowserRouter) because Tauri serves the bundle from a `tauri://` origin; deep-links like `/patients?add=1` must work without a server.

### 1.12 Future Opportunities
- Voice-driven charting (e.g. "Open patient 1284, add note: …").
- Customizable dashboard widgets per user (`users.dashboard_config` JSON column).
- Spatial anchors: remember scroll position per route in `sessionStorage`.

---

## 2. Design System

### 2.1 Objective
Define a single source of truth for typography, color, spacing, elevation, iconography and reusable primitives. Every token in this section is the live value from `src/index.css` — designers and developers must reference these tokens, never hard-coded hexes.

### 2.2 Design Principles
- **Token-first:** All colors are HSL CSS variables; Tailwind 4 `@theme inline` surfaces them as utilities (`bg-primary`, `text-foreground`, etc.). **[Implemented v0.2.0 (Batch 2 CR-13)]** The `@theme inline` block in `src/index.css` (lines 180-220) registers every colour + status token so Tailwind v4 generates the utilities. The v0.1.0 implementation had CSS variables on `:root` but had NOT registered them in `@theme inline`, so Badge variants using `bg-status-scheduled`, `text-info`, etc. silently rendered unstyled. CR-13 closed that gap; all 5 status colors (`scheduled`, `confirmed`, `completed`, `cancelled`, `no-show`) + `info` + `success-foreground` + `warning-foreground` are now registered.
- **Two-tier semantic mapping:** Primitive (`--primary`) → semantic (`--status-scheduled`). Status colors are tied to clinical lifecycle, never re-purposed for branding.
- **Soft, layered elevation:** Shadows use a blue-tinted slate base (`rgb(15 23 42 / …)`) for cohesion.
- **Density calibrated:** 14 px body font with 1.5 rem line-height — dense yet readable.
- **[Implemented v0.2.0 (Batch 2 CR-18)]** The VitalFlow palette (sky-blue `#0EA5E9` + teal `#14B8A6` + Inter) is the actually-shipped palette. The previous Mayo-navy design is retired. See the comment at `src/index.css:11` (`CR-18: re-skinned from Mayo navy to spec palette.`).

### 2.3 Functional Requirements
- FR-DS-0001 The design system shall expose **≥ 35 CSS custom properties** on `:root` and an equivalent set on `.dark`.
- FR-DS-0002 All primitives (Button, Input, Card, Dialog, Badge, Table, Tabs, Select, DropdownMenu) shall be shadcn/ui-derived and styled via the token layer.
- FR-DS-0003 Status colors shall be limited to 5 lifecycle states: `scheduled, confirmed, completed, cancelled, no-show`.
- FR-DS-0004 The radius scale shall be 6/10/12/16/20 px (`sm / default / md / lg / xl`).

### 2.4 Typography
| Token | Weight | Size | Line-height | Letter-spacing | Use case |
|---|---|---|---|---|---|
| `.text-display-xl` | 800 | 2.25 rem | 2.5 rem | -0.025em | Login hero, license-error title |
| `.text-display-lg` | 700 | 1.75 rem | 2.125 rem | -0.02em | Page H1, KPI value |
| `.text-display-md` | 600 | 1.25 rem | 1.75 rem | -0.015em | Card title, modal title |
| `.text-display-sm` | 600 | 1 rem | 1.5 rem | -0.01em | Sidebar brand, section title |
| Body default | 400 | 0.875 rem (14 px) | 1.5 rem | 0 | All prose, table cells |
| Body small | 500 | 0.75 rem (12 px) | 1 rem | 0 | Table headers, badges |
| Body micro | 600 | 0.6875 rem (11 px) | 1 rem | 0.01em | Status badges, captions |
| Body mono | 500 | 0.75 rem | — | 0 | Times, token numbers, MRN |

Font family: `'Inter', -apple-system, BlinkMacSystemFont, sans-serif`. Variable Inter is bundled at `src/assets/fonts/Inter-VariableFont_opsz,wght.ttf` and stylistic sets `cv11, ss01, ss03` are enabled for disambiguated `0/O`, `1/l/I`, `6/9`.

### 2.5 Color — Light Theme
| Token | HSL | Hex | Purpose |
|---|---|---|---|
| `--background` | `210 40% 98%` | `#F8FAFC` | Page background (slate-50) |
| `--foreground` | `222 47% 11%` | `#0F172A` | Body text (slate-900) |
| `--card` | `0 0% 100%` | `#FFFFFF` | Cards, popovers |
| `--primary` | `199 89% 48%` | `#0EA5E9` | **Primary brand** (sky-500) |
| `--primary-hover` | `199 89% 40%` | `#0284C7` | Hover state (sky-600) |
| `--primary-soft` | `199 89% 94%` | `#E0F2FE` | Subtle fills (sky-100) |
| `--accent` | `172 76% 36%` | `#14B8A6` | **Accent** (teal-500) |
| `--accent-soft` | `172 76% 94%` | `#CCFBF1` | Teal-100 |
| `--secondary` | `210 40% 96%` | `#F1F5F9` | Secondary surfaces |
| `--muted-foreground` | `215 16% 47%` | `#64748B` | Captions, labels |
| `--destructive` | `0 72% 51%` | `#DC2626` | Errors, destructive actions |
| `--success` | `142 71% 45%` | `#22C55E` | Success, "Available" bed |
| `--warning` | `38 92% 50%` | `#F59E0B` | Warnings, "Cleaning" bed |
| `--info` | `199 89% 48%` | `#0EA5E9` | Info — matches primary |
| `--border` | `214 32% 91%` | `#E2E8F0` | Borders, dividers |
| `--ring` | `199 89% 48%` | `#0EA5E9` | Focus ring |

### 2.6 Color — Dark Theme
| Token | HSL | Hex | Notes |
|---|---|---|---|
| `--background` | `222 47% 7%` | `#0B0F19` | Deep slate, not pure black |
| `--card` | `222 40% 10%` | `#131825` | Lifted via bg + shadow |
| `--primary` | `199 89% 56%` | `#38BDF8` | Brighter sky-400 for contrast |
| `--accent` | `172 76% 42%` | `#2DD4BF` | Teal-400 |
| `--muted-foreground` | `215 20% 65%` | slate-400 | Slightly brighter than light |
| `--border` | `217 33% 20%` | slate-700 | Less stark than pure black dividers |

### 2.7 Status Color Tokens
| Lifecycle | Token | Hex (light) | Hex (dark) | Mapped to |
|---|---|---|---|---|
| Scheduled | `--status-scheduled` | `#0EA5E9` | `#38BDF8` | Appointment booked |
| Confirmed | `--status-confirmed` | `#22C55E` | `#22C55E` | Patient checked-in |
| Completed | `--status-completed` | `#64748B` | `#94A3B8` | Encounter finished |
| Cancelled | `--status-cancelled` | `#DC2626` | `#EF4444` | Cancelled / void |
| No-show | `--status-no-show` | `#F59E0B` | `#F59E0B` | Patient did not arrive |

Bed status reuses the same palette: `available → status-confirmed`, `occupied → status-cancelled`, `maintenance → status-no-show`, `cleaning → status-scheduled`. See `IPD.tsx::BED_STATUS`.

### 2.8 Grid & Spacing
- Base spacing unit: `4px` (Tailwind default).
- Recommended rhythm: `space-y-6` (24 px) between page sections, `gap-4` (16 px) between KPI cards, `gap-1.5` (6 px) between related form fields.
- Content max-width: `1400px` (set in `AppShell`); prevents line-length > 90 chars on wide monitors.
- Page padding: `px-4 sm:px-6 lg:px-8 py-6 lg:py-8`.

### 2.9 Radius Scale
| Token | Value | Use |
|---|---|---|
| `--radius-sm` | 6 px | Small chips, focus ring corners |
| `--radius` (default) | **10 px** | Inputs, default buttons |
| `--radius-md` | 12 px | Cards |
| `--radius-lg` | 16 px | Modals, hero sections |
| `--radius-xl` | 20 px | Feature panels |

### 2.10 Elevation — 5-Tier Shadow System
| Token | Value (light) | Use |
|---|---|---|
| `--shadow-xs` | `0 1px 2px 0 rgb(15 23 42 / 0.04)` | Subtle separators |
| `--shadow-sm` | `0 1px 3px 0 rgb(15 23 42 / 0.06), 0 1px 2px -1px rgb(15 23 42 / 0.04)` | Cards, KPI tiles |
| `--shadow-md` | `0 4px 6px -1px rgb(15 23 42 / 0.07), 0 2px 4px -2px rgb(15 23 42 / 0.05)` | Hover lift, dropdowns |
| `--shadow-lg` | `0 10px 15px -3px rgb(15 23 42 / 0.08), 0 4px 6px -4px rgb(15 23 42 / 0.05)` | Modals, elevated panels |
| `--shadow-xl` | `0 20px 25px -5px rgb(15 23 42 / 0.10), 0 8px 10px -6px rgb(15 23 42 / 0.05)` | Full-screen overlays |
| `--shadow-glow` | `0 0 0 3px rgb(14 165 233 / 0.12)` | Focus ring (not elevation) |

Dark theme shadows use `rgb(0 0 0 / 0.2–0.6)` to read against deep slate.

### 2.11 Iconography
- **Library:** Lucide React (`lucide-react`). Stroke-width 1.5 by default.
- **Standard sizes:** 18 px (sidebar/nav), 16 px (inline), 14 px (button leading icon), 24 px (empty state hero).
- **Stroke color:** Inherit `currentColor` so icons respond to `text-primary`, `text-muted-foreground`, etc.
- **Permission/role icons:** `LayoutDashboard` (Dashboard), `Calendar` (Appointments), `Users` (Patients), `ListOrdered` (Queue), `Stethoscope` (Doctors), `BedDouble` (IPD), `FlaskConical` (Lab), `Receipt` (Billing), `MessageSquare` (Staff chat), `ScrollText` (Audit log), `UserCog` (Users), `BarChart3` (Reports), `Settings` (Settings).
- **Status icons:** `CheckCircle` (success), `AlertTriangle` (warning), `XCircle` (destructive), `Clock` (scheduled), `Loader2` (spinner).

### 2.12 Buttons
| Variant | Use | Classes |
|---|---|---|
| `default` | Primary action (1 per surface) | `bg-primary text-primary-foreground hover:bg-[hsl(var(--primary-hover))]` |
| `outline` | Secondary action | `border border-border bg-card hover:bg-muted` |
| `ghost` | Tertiary / inline | `hover:bg-muted` |
| `destructive` | Delete, void | `bg-destructive text-destructive-foreground` |
| `link` | Hyperlink-style | `text-primary underline-offset-4` |

Sizes: `sm` (h-9, px-3, text-xs), `default` (h-10, px-4, text-sm), `lg` (h-11, px-6, text-sm), `icon` (h-10 w-10). All buttons default to `rounded-lg` (10 px). Disabled state: `opacity-60 cursor-not-allowed`.

### 2.13 Inputs
- Height: `h-10` default; `h-9` for compact forms.
- Padding: `px-3.5`.
- Background: `bg-card` with `border border-border`.
- Focus: `outline-none focus:ring-2 focus:ring-ring/40 focus:border-primary/40`.
- Labels: `.text-xs font-semibold uppercase tracking-wide text-foreground` above the input.
- Helper text: `.text-[11px] text-muted-foreground mt-1`.
- Error: `border-destructive` + helper text `text-destructive`.

### 2.14 Tables
- Header: `text-xs uppercase tracking-wide text-muted-foreground py-3`.
- Cells: `py-3 text-sm text-foreground`.
- Hover: `hover:bg-muted/50`.
- Numeric cells: `text-right tabular-nums font-mono`.
- Status cells: use `.status-badge` pill component.
- Long lists: wrap in `ScrollArea` with `max-h-[420px]`.

### 2.15 Cards
Three surface utilities defined in `index.css`:
- `.surface-card` — default content card (`border + shadow-sm + radius-md`).
- `.surface-elevated` — modal / hero (`border + shadow-lg + radius-lg`).
- `.surface-flat` — flat container (`bg-card + radius-md`, no border, no shadow).

KPI cards use `rounded-xl border-border shadow-sm` and lift on hover (`hover:shadow-md hover:border-primary/20 hover:-translate-y-0.5`).

### 2.16 Dialogs
- Default width: `max-w-lg` (32rem). Modals with forms: `max-w-xl`. Confirm dialogs: `max-w-md`.
- Header: `text-display-md` title + optional description (`.text-sm text-muted-foreground`).
- Body: `space-y-4`.
- Footer: `flex justify-end gap-2`; primary action right-most.
- Backdrop: `bg-black/50 backdrop-blur-sm`.
- Close button: top-right `X` icon (`h-4 w-4`).

### 2.17 Badges & Chips
`.status-badge` is the canonical pill:
```
display: inline-flex; gap: 0.375rem; padding: 0.125rem 0.625rem;
border-radius: 999px; font-size: 0.6875rem; font-weight: 600;
```
Color is applied via inline `style={{ background: 'hsl(token / 0.10)', color: 'hsl(token)' }}` so the same component renders consistently across themes.

### 2.18 Navigation
- Sidebar: see §3.
- Breadcrumbs **[PLANNED]** for deep pages (Patient > Mrn-1234 > Encounter 2026-07-03).
- Tabs: shadcn `Tabs` component; underline indicator on active.

### 2.19 Tooltips
- Trigger: `delayDuration={300}`.
- Content: `.text-xs` on `bg-popover text-popover-foreground` with `shadow-md`.
- Used for icon-only buttons (sidebar collapsed, action icons).

### 2.20 Notifications (Toasts)
- Library: **Sonner** (`<Toaster richColors closeButton position="top-right" />` in `App.tsx`).
- Variants: `success`, `info`, `warning`, `error` — all use rich colors.
- Auto-dismiss: 4 s default; persistent for `error` until dismissed.
- Stacking: max 3 visible; older collapse.

### 2.21 Progress & Loading
- Spinner: `<Loader2 className="animate-spin" />` (Lucide).
- Linear progress: shadcn `Progress` (range 0–100).
- Loading state pattern: `isLoading ? <Skeleton /> : data ? <Content /> : <EmptyState />`.

### 2.22 Empty States
```
<div className="flex flex-col items-center justify-center py-12 text-muted-foreground gap-2">
  <Icon className="h-6 w-6 opacity-50" />
  <span className="text-xs">{text}</span>
</div>
```
Always offer a CTA when the empty state is actionable (e.g. "No patients yet → New patient").

### 2.23 Skeletons
```css
.skeleton {
  background: linear-gradient(90deg,
    hsl(var(--muted)) 25%,
    hsl(var(--muted) / 0.5) 37%,
    hsl(var(--muted)) 63%);
  background-size: 200% 100%;
  animation: skeleton-shimmer 1.4s ease-in-out infinite;
}
```
Reduced-motion users get a static muted block (`animation: none`).

### 2.24 UX Considerations
- Status colors are reused intentionally across the appointment and bed domains; this shortens the learning curve.
- The 14 px body font is a deliberate trade-off: dense enough for clinical data tables, still legible at 1.5 rem line-height.

### 2.25 Desktop Layout
The design system is resolution-independent but optimized for 96 DPI at 100% scaling. Layout breakpoints: `sm 640`, `md 768`, `lg 1024`, `xl 1280`, `2xl 1536`.

### 2.26 Component Specs
All shadcn/ui primitives live under `src/components/ui/`. The VitalFlow-specific overrides are limited to the token layer; component JSX is unchanged from shadcn defaults. This keeps upgrade paths clean.

### 2.27 Interaction Patterns
- Hover lift: `hover:-translate-y-0.5 hover:shadow-md` (200 ms ease).
- Active press: `active:translate-y-0` (no bounce).
- Focus: ring glow, never outline.

### 2.28 Accessibility
- All tokens meet WCAG 2.2 AA contrast: `--primary #0EA5E9` on white = 3.03:1 (UI components ≥ 3:1 ✓); `--primary` text on white = 4.55:1 (text ≥ 4.5:1 ✓ when bold ≥ 14 px).
- Dark theme `--primary #38BDF8` on `--background #0B0F19` = 7.41:1 (AAA).
- Error state is dual-coded (red border + destructive helper text).

### 2.29 Performance
- Tokens are CSS variables — no runtime recalculation.
- Tailwind 4 `@theme inline` generates utilities at build time; zero CSS-in-JS overhead.
- Inter variable font: single file, all weights, subset to Latin.

### 2.30 Best Practices
- Never hardcode hexes; always reference `hsl(var(--token))`.
- Status colors are reserved for status; do not recolor buttons as "scheduled blue".
- Maintain the 5-tier shadow scale — adding a sixth breaks the elevation language.

### 2.31 Implementation Notes
- Tailwind config: `tailwind.config.ts` extends `colors` with token references.
- shadcn components are CLI-generated into `src/components/ui/` (see `components.json`).
- The `motion` library is the animation runtime (not framer-motion).

### 2.32 Future Opportunities
- Density toggle (Comfortable / Compact) per user.
- High-contrast theme for clinical environments with bright ambient light.
- Right-to-left (RTL) flipping for international deployments.

---

## 3. Application Layout

### 3.1 Objective
Define the persistent shell that hosts every page: sidebar, header, breadcrumbs, search, workspace, context panels, status bar, notification center, profile menu, quick actions. The shell must remain stable across route changes; only the workspace re-renders.

### 3.2 Design Principles
- **Persistent chrome:** Sidebar + Header never unmount on navigation (`AppShell` wraps `<Routes>`).
- **Role-aware density:** Super Admin sees 13 nav items; Receptionist sees 8; Patient sees 1.
- **Glanceable status:** Header exposes clock, notifications bell, theme toggle and user avatar — all within 64 px height.
- **Mobile parity:** Below `lg` (1024 px) the sidebar collapses into a drag-dismissible drawer.

### 3.3 Functional Requirements
- FR-AL-0001 The shell shall render `Sidebar + Header + <main>` in a 3-region flex layout that fills `h-screen w-screen`.
- FR-AL-0002 The sidebar shall filter nav items client-side using `useAuth().has(perm)`; the backend re-checks every command.
- FR-AL-0003 Sidebar collapse state shall persist in `localStorage["hms-sidebar-collapsed"]`.
- FR-AL-0004 The header shall display the page title derived from the current `useLocation().pathname`.
- FR-AL-0005 The header shall expose a global search affordance that animates from icon to 240 px input.
- FR-AL-0006 A notification bell with a 2 px red dot shall indicate unread alerts.

### 3.4 UX Considerations
- **Long shifts:** The clock auto-refreshes every 30 s; `tabular-nums` prevents layout shift.
- **Multi-tasking:** The "Staff chat" sidebar item shows a pulsing green `Live` badge — clinicians know the channel is real-time.
- **Discovery:** Phase-2 items (`Reports`) appear disabled with a `Phase 2` chip so users know what is coming.

### 3.5 Desktop Layout

#### 3.5.1 Sidebar (264 px expanded / 72 px collapsed)
```
┌──────────────────────┐
│ ▣ VitalFlow          │  ← Brand row, 64 px tall
│   HMS Clinic         │
├──────────────────────┤
│ ▣ Dashboard          │  ← NavLink with permission gate
│ ▣ Appointments       │
│ ▣ Patients           │
│ ▣ Queue              │
│ ▣ Doctors            │
│ ▣ In-Patient         │
│ ▣ Laboratory         │
│ ▣ Billing            │
│ ▣ Staff chat  ●Live  │
│ ▣ Audit log          │
│ ▣ Users & roles      │
│ ▣ Reports  [Phase 2] │  ← Disabled
│ ▣ Settings           │
├──────────────────────┤
│ « Collapse           │
├──────────────────────┤
│ (●) Dr. Aisha        │  ← User footer
│     Doctor           │
└──────────────────────┘
```

#### 3.5.2 Header (64 px)
```
[≡] Page Title  [↻]            [🔍][clock | bell | theme | avatar▾]
```

#### 3.5.3 Workspace
- Centered container `max-w-[1400px] mx-auto`, `px-4 sm:px-6 lg:px-8 py-6 lg:py-8`.
- Scrollable: `overflow-y-auto` on `<main>`.
- Page transition: `motion.div` fade+slide 180 ms.

#### 3.5.4 Context Panels **[PLANNED]**
Right-side slide-over panels (480 px) for "Patient quick view", "Bill detail" — opened from any list row without losing context.

#### 3.5.5 Status Bar **[PLANNED]**
Bottom 24 px strip showing: DB connected · Server mode (server/client) · Sync status · License expiry.

#### 3.5.6 Notification Center **[PLANNED]**
Right-side drawer opened from the bell icon; groups notifications by Today / Yesterday / Earlier.

#### 3.5.7 Profile Menu
DropdownMenu (shadcn) anchored to avatar; items:
- Label: full name + role + @username
- `Change password` → navigates to `/settings`
- `Sign out` → `logout()` then reload.

#### 3.5.8 Quick Actions
Dashboard exposes a 6-tile Quick Access grid (Patients, Doctors, IPD, Lab, Billing, Queue); each tile is permission-gated via `<QuickAccess perm={...} />`.

### 3.6 Resolution Recommendations
| Resolution | Sidebar | Workspace | Notes |
|---|---|---|---|
| 1366×768 (min) | Collapsed (72 px) by default | Single-column KPI grid (2 cols) | Target netbook / older clinical workstations. Recommend setting default collapse via media query. |
| 1920×1080 | Expanded (264 px) | KPI grid 4 cols, two-column main | Default target; matches majority of clinical PCs. |
| 2K (2560×1440) | Expanded | KPI grid 4 cols + side context panel | Enable the right-side context panel; relax `max-w-[1400px]` to `max-w-[1600px]`. |
| 4K (3840×2160) | Expanded | KPI grid 6 cols, multi-zone dashboard | Recommend OS scaling at 150% so 14 px body remains readable. Add a `xl:` density utility set. |

### 3.7 Component Specs
- `Sidebar.tsx` — 163 lines, props: `collapsed, onToggleCollapsed, isMobileDrawer?, onNavigate?`.
- `Header.tsx` — 170 lines, props: `onMenuClick, onRefresh?, isRefreshing?`.
- `AppShell.tsx` — 119 lines, composes sidebar + mobile drawer + header + main.
- `ThemeToggle.tsx` — light/dark switch.

### 3.8 Interaction Patterns
- **Mobile drawer:** drag-to-dismiss (`drag="x"`, threshold -80 px), backdrop tap closes, auto-close on route change.
- **Search expand:** `motion.div` animates `width: 0 → 240`, 200 ms spring.
- **Refresh button:** `RefreshCw` rotates while `isRefreshing`; dispatches `hms:refresh` CustomEvent.

### 3.9 Accessibility
- Menu button: `aria-label="Open navigation menu"`.
- Search button: `aria-label="Search"`.
- Notifications button: `aria-label="Notifications"`.
- Avatar button: `aria-label="Account menu"`.
- All sidebar items are keyboard-navigable (`Tab` order = visual order).

### 3.10 Performance
- Sidebar items memoized; the active pill uses `layoutId` so Motion animates between items without re-mount.
- Header clock updates every 30 s (not 1 s) to minimize re-renders.

### 3.11 Best Practices
- Keep the header ≤ 64 px — vertical real estate is precious on 768 px displays.
- Never put critical actions behind the avatar menu; it is for account actions only.
- The "Refresh" button is reassurance, not a primary affordance — React Query auto-refetches on focus.

### 3.12 Implementation Notes
- Routes are declared once in `App.tsx::RoutedPages`.
- `PageTransition` wraps each route in `motion.div` with `AnimatePresence mode="wait"`.
- Deep-linking: `?add=1` query triggers the add-patient dialog on the Patients page.

### 3.13 Future Opportunities
- Customizable sidebar order per user.
- Pinned items section ("Favorites") above the main nav.
- Workspace split-view (two pages side-by-side) for multi-monitor setups.

---

## 4. Role-Based Dashboards

### 4.1 Objective
Provide each role with a glanceable, action-oriented landing surface. Dashboards surface the 4–6 KPIs that matter most to that persona plus a primary CTA, a "today" list and a chart. Role dashboards are implemented via conditional rendering inside the single `Dashboard.tsx` using `has(perm)` gates.

### 4.2 Design Principles
- **Minimum necessary** (HIPAA): each role sees only its KPIs.
- **Drill-down everywhere:** every KPI card is clickable to the relevant module.
- **Action-first:** a primary CTA ("New patient", "New appointment") sits in the top-right.
- **Cognitive ceiling:** ≤ 8 KPI tiles per role; ≥ 6 only for Super Admin.

### 4.3 Functional Requirements
- FR-RD-0001 The dashboard shall call `useDashboardKpis()` once and share results across all conditional tiles.
- FR-RD-0002 Tiles requiring `ipd.view`, `lab.view`, `billing.view` shall render only when `has(perm)` returns true.
- FR-RD-0003 The "Today's schedule" table shall display the next 12 appointments.
- FR-RD-0004 The "Appointment mix" donut shall render only when ≥ 1 slice has value > 0.
- FR-RD-0005 The "Queue now" card shall display the next 6 tokens.

### 4.4 Role Dashboards Matrix
| Role | KPIs visible | Primary CTA | Special tile |
|---|---|---|---|
| `super_admin` | All 8 KPIs + Revenue | New patient, New appointment | System health **[PLANNED]** |
| `doctor` | Patients, Appointments today, Completed, Queue, Beds, Pending lab | New appointment | Today's schedule (own patients filter **[PLANNED]**) |
| `nurse` | Patients, Appointments today, Completed, Queue, Beds, Pending lab | — | Bed board quick-link |
| `receptionist` | Patients, Appointments today, Completed, Queue | New patient, New appointment | Quick registration |
| `lab_technician` | Patients, Pending lab orders | — | Lab order queue |
| `pharmacist` | Patients, Inventory value **[PLANNED]** | — | Low-stock list **[PLANNED]** |
| `billing_clerk` | Patients, Appointments today, Revenue today, Revenue month | New invoice | Outstanding bills **[PLANNED]** |
| `patient` | (own next appointment) **[PLANNED]** | — | (own last visit summary) **[PLANNED]** |

### 4.5 UX Considerations
- **Doctor at bedside:** the Today's schedule table is the most-clicked surface; rows are clickable to the patient chart.
- **Receptionist triage:** the "In queue" KPI shows waiting + in-progress counts to manage front-desk flow.
- **Lab tech focus:** only 4 KPIs visible — minimizes distraction in a noisy lab.

### 4.6 Desktop Layout (default doctor view)
```
┌──────────────────────────────────────────────────────────────────────────┐
│ Welcome back                                  [+ New patient][+ New appt]│
├──────────────────────────────────────────────────────────────────────────┤
│ [Total][Appts today][Completed][Queue][Beds][Pending lab]                │
├──────────────────────────────────────┬───────────────────────────────────┤
│ Today's schedule (12 rows)           │ Appointment mix (donut)           │
│  09:00  A. Khan     Dr. Ahmed        │  ●Scheduled 12                    │
│  09:30  B. Singh    Dr. Ahmed        │  ●Confirmed 8                     │
│  ...                                  │  ●Completed 5                     │
│                                      │  ●Cancelled 2                     │
│                                      ├───────────────────────────────────┤
│                                      │ Queue now (6 tokens)              │
└──────────────────────────────────────┴───────────────────────────────────┘
│ Quick access: [Patients][Doctors][IPD][Lab][Billing][Queue]              │
└──────────────────────────────────────────────────────────────────────────┘
```

### 4.7 Component Specs
- `<Kpi icon, label, value, sub, color, onClick?>` — 5 lines of structure; uses `KPI_COLORS` map.
- `<QuickAccess icon, label, perm, onClick>` — permission-gated tile, returns null if forbidden.
- `<EmptyHint icon, text>` — empty-state placeholder.

### 4.8 Interaction Patterns
- KPI mount: `motion.div initial={{opacity:0, y:8}} animate={{opacity:1, y:0}}` 300 ms ease.
- KPI hover: `hover:shadow-md hover:border-primary/20 hover:-translate-y-0.5`.
- KPI click: navigates to the underlying module (`onNavigate("patients")`).
- Donut tooltip: recharts `<Tooltip>` with custom `contentStyle` matching card tokens.

### 4.9 Accessibility
- Each KPI is a `<Card onClick>`; an `aria-label` should be added (Phase 2 fix) to expose the drill-down target.
- Donut legend uses both color swatch + text label (never color alone).
- KPI values use `tabular-nums` to prevent layout shift during data refresh.

### 4.10 Performance
- `useDashboardKpis`, `useAppointmentStats`, `useTodayAppointments`, `useQueue` run in parallel on mount.
- React Query deduplicates; the same data is reused by the Dashboard and the Appointments/Queue pages.
- Donut chart: `<ResponsiveContainer>` defers layout until parent has width.

### 4.11 Best Practices
- Order KPIs left-to-right by clinical urgency (Queue before Revenue).
- Use `sub` text to add a secondary metric ("12 pending" under Appointments today).
- Never auto-rotate carousels on a dashboard; clinicians should control dwell time.

### 4.12 Implementation Notes
- All KPI logic lives in `pages/Dashboard.tsx` (288 lines).
- The role filter is implicit via `has(perm)` calls; no `if (role === "doctor")` branching.
- The chart palette uses CSS variable tokens so the same colors propagate to dark mode.

### 4.13 Future Opportunities
- Per-role widget configuration (drag-and-drop layout).
- Saved views (e.g. "Night shift" preset that hides revenue tiles).
- Anomaly badges ("↑ 23% above 7-day average") on each KPI.

---

## 5. Patient Management — 360° Patient View

### 5.1 Objective
Provide a single screen where every clinician sees the complete longitudinal record of a patient: demographics, contact, EHR (allergies, chronic conditions, blood group, MRN), encounter history, appointment history, lab orders, IPD admissions, bills and consents. This is the most-used screen for doctors and nurses.

### 5.2 Design Principles
- **One patient, one screen:** no clicking between modules to assemble context.
- **Allergies first:** red allergy banner is always visible at the top of the chart.
- **Time-ordered:** encounters, labs and admissions are sorted reverse-chronologically.
- **Consent-aware:** actions requiring consent (e.g. sharing records) check `patient_consent` first.

### 5.3 Functional Requirements
- FR-PM-0001 The patient list shall support search by name, phone, email, address and MRN.
- FR-PM-0002 Creating a patient shall require `patients.create` permission (receptionist, doctor, super_admin).
- FR-PM-0003 Editing a patient shall require `patients.update`.
- FR-PM-0004 Deleting a patient shall require `patients.delete` and shall soft-delete (FK ON DELETE RESTRICT enforced server-side for encounters/bills).
- FR-PM-0005 The EHR expansion (`PatientEhr`) shall include `mrn, blood_group, allergies, chronic_conditions, emergency_contact, insurance_provider, insurance_member_id`.
- FR-PM-0006 The 360° view shall be deep-linkable at `/patients/:id` **[PLANNED]**.

### 5.4 UX Considerations
- **Front-desk pace:** the patient list filters as the user types; debounce 150 ms.
- **Privacy by default:** the list shows name + phone only; demographic details are revealed only after opening a record.
- **Allergy prominence:** a red banner (`bg-destructive/10 text-destructive`) appears at the top of the 360° view when `allergies` is non-null.
- **Age display:** auto-computed from `date_of_birth` ("34 y / Male").

### 5.5 Desktop Layout (360° view) **[PLANNED]**
```
┌──────────────────────────────────────────────────────────────────────────┐
│ ← Back to patients              Aisha Khan, 34 F   MRN-0001234           │
├──────────────────────────────────────────────────────────────────────────┤
│ ⚠ Allergies: Penicillin, Sulfa                                            │
│ Blood: O+ · Insurance: Star Health · ID SH-884-221                        │
├──────────┬───────────────────────────────────────────────────────────────┤
│ Sidebar  │ Tabs: Overview · Encounters · Appointments · Labs · IPD · Bills│
│ ─Photo   │ ┌──────────────────────────────────────────────────────────┐ │
│ ─Demog   │ │ Overview: vitals timeline, chronic conditions, contacts  │ │
│ ─Contact │ │ Encounters: reverse-chrono list with chief complaint     │ │
│ ─Insur.  │ │ Labs: last 10 results with abnormal flag                 │ │
│ ─Consent │ │ IPD: admissions + discharge summaries                    │ │
│          │ │ Bills: outstanding balance + payment history             │ │
└──────────┴───────────────────────────────────────────────────────────────┘
```

### 5.6 Component Specs
- **Patient list table** (`Patients.tsx`): columns Name, Phone, Email, DOB, Gender, Actions (Edit, Delete).
- **PatientForm** (`components/forms/PatientForm.tsx`): two-column grid; fields grouped Demographics / Contact / EHR / Insurance.
- **Allergy banner** **[PLANNED]**: `bg-destructive/10 border border-destructive/30 px-4 py-2 text-sm text-destructive`.
- **Encounter card** **[PLANNED]**: title (date + provider), chief complaint, diagnosis, prescription chip row.

### 5.7 Interaction Patterns
- **Quick add from Dashboard:** `Dashboard → "New patient" → navigate("/patients?add=1") → useEffect opens dialog`.
- **Inline edit:** clicking Edit opens the same `PatientForm` dialog pre-populated.
- **Delete confirmation:** browser `confirm()` in Phase 1; Phase 2 will replace with `AlertDialog` for consistent styling.

### 5.8 Accessibility
- Every form field has a `<Label htmlFor>` association.
- Error messages are linked via `aria-describedby`.
- The allergies banner uses `role="status"` so screen readers announce it on view open **[PLANNED]**.

### 5.9 Performance
- Patient list query is paginated (Phase 2); Phase 1 returns all patients (typical hospital < 50k rows).
- `PatientEhr` is fetched once per opened record; React Query caches by `id`.

### 5.10 Best Practices
- Never display full SSN/National ID in lists; mask middle digits.
- Show "Last edited by {user} at {ts}" on the 360° view footer for audit transparency.
- Confirm before navigating away from an unsaved form (`window.onbeforeunload` **[PLANNED]**).

### 5.11 Implementation Notes
- `usePatients()` returns `Patient[]`; `usePatientsEhr()` returns `PatientEhr[]` (used by IPD admit dialog).
- Soft-delete is **[PLANNED]** — Phase 1 uses hard delete with FK RESTRICT safety.
- `patient_consent` table tracks consent type, scope, granted_at, granted_by.

### 5.12 Future Opportunities
- Photo capture via webcam (Tauri camera plugin).
- Family graph (linked patients: parent/child/spouse).
- Wearable device integration (Apple Health, Fitbit) for vitals timeline.

---

## 6. Appointment & Queue Management

### 6.1 Objective
Unify appointment scheduling (calendar-driven, future-dated) with the live patient queue (today, in-clinic). Receptionists book; doctors and nurses call tokens; the queue board displays in waiting rooms.

### 6.2 Design Principles
- **Two views, one source:** both the Appointments page and the Queue page consume the same `appointments` and `queue_tokens` tables.
- **Status lifecycle is shared:** `scheduled → confirmed → in-progress → completed | cancelled | no-show`.
- **Token numbers are immutable:** once issued, the token number cannot change even if the queue is reordered.
- **Queue board is read-only:** a separate display-mode URL `/queue/board` is rendered fullscreen on a TV **[PLANNED]**.

### 6.3 Functional Requirements
- FR-AQ-0001 Creating an appointment shall require `appointments.create` (receptionist, super_admin).
- FR-AQ-0002 Updating an appointment shall require `appointments.update`.
- FR-AQ-0003 The receptionist shall be able to issue a queue token via `queue.manage` permission.
- FR-AQ-0004 The doctor/nurse shall be able to call the next token and mark tokens `in-progress` / `completed`.
- FR-AQ-0005 Appointment status changes shall be audit-logged via `audit::record("appointment.update")`.

### 6.4 UX Considerations
- **Receptionist flow:** search patient → select doctor → pick date/time → save → print token.
- **Doctor flow:** click "Call next" → see patient summary card → mark "Seen" → next.
- **Color coding:** status pill colors are derived from `--status-*` tokens so the queue board is legible at 3 m.

### 6.5 Desktop Layout — Appointments Page
```
┌──────────────────────────────────────────────────────────────────────────┐
│ Appointments         [+ New appointment]                                  │
├──────────────────────────────────────────────────────────────────────────┤
│ Tabs: List | Calendar **[PLANNED]**                                       │
│ Filters: [Doctor▾][Date][Status▾]   Search: [____________]                │
├──────────────────────────────────────────────────────────────────────────┤
│ Time    Patient         Doctor          Status        Duration   Actions │
│ 09:00   Aisha Khan      Dr. Ahmed       ●Confirmed    30 min     [⋯]    │
│ 09:30   Bikram Singh    Dr. Ahmed       ●Scheduled    30 min     [⋯]    │
│ ...                                                                       │
└──────────────────────────────────────────────────────────────────────────┘
```

### 6.6 Desktop Layout — Queue Page
```
┌──────────────────────────────┬─────────────────────────────────────────┐
│ Now serving                   │ Waiting (8)                             │
│ ┌──────────────────────────┐ │ ┌──┬─────────────┬────────┬──────────┐ │
│ │ Token #0042              │ │ │# │ Patient       │ Doctor │ Wait     │ │
│ │ Aisha Khan               │ │ │43│ Bikram Singh  │ Ahmed  │ 8 min    │ │
││ Dr. Ahmed — Room 2        │ │ │44│ Cara Lopez    │ Ahmed  │ 12 min   │ │
││ [Call next][Mark done]    │ │ │45│ ...           │ ...    │ ...      │ │
│ └──────────────────────────┘ │ └──┴─────────────┴────────┴──────────┘ │
└──────────────────────────────┴─────────────────────────────────────────┘
```

### 6.7 Component Specs
- **AppointmentForm** (`components/forms/AppointmentForm.tsx`): patient select (searchable), doctor select, date picker, time input, duration, reason, notes.
- **Token card:** large token number (`text-display-xl`), patient name, doctor + room.
- **Status badge:** reuses `.status-badge` with `STATUS_TOKEN` map (see `Dashboard.tsx`).

### 6.8 Interaction Patterns
- **Call next:** button calls `callNextToken` mutation; optimistic update.
- **Reorder:** drag-and-drop in the waiting list **[PLANNED]**.
- **No-show sweep:** a scheduled action runs hourly marking tokens older than 2 h as `no-show` **[PLANNED]**.

### 6.9 Accessibility
- Token numbers are read aloud by screen readers as digits ("token zero zero four two") **[PLANNED]**.
- Color-coded status pills include a dot icon + text label (dual-coded).

### 6.10 Performance
- Queue polling: 5 s interval when the Queue page is focused **[PLANNED]**; Phase 1 uses manual refresh.
- Appointment list virtualized for > 500 rows **[PLANNED]**.

### 6.11 Best Practices
- Default appointment duration: 30 min; allow 10/15/20/30/45/60 presets.
- Show "Estimated wait: 14 min" on the queue board so patients self-manage expectations.
- Lock token numbers from edits; corrections are made via status changes only.

### 6.12 Implementation Notes
- `queue_tokens` table: `id, token_number, patient_id, doctor_id, department_id, status, issued_at, called_at, completed_at`.
- `useQueue()` returns the current open queue; `useCallNextToken()` advances the doctor's queue.
- The `QueueManage` permission gates call/complete; `QueueView` is read-only (board display).

### 6.13 Future Opportunities
- SMS/WhatsApp notification when token is next ("You're next — please proceed to Room 2").
- Predictive wait time using historical throughput.
- Self-service kiosk mode for token issuance.

---

## 7. Clinical Workflows

### 7.1 Objective
Provide a coherent end-to-end clinical flow from patient arrival through consultation, diagnosis, prescription, lab/radiology requests, admission, follow-up and referral — with safety checks at every step.

### 7.2 Design Principles
- **Linear, interruptible:** each step has a clear entry and exit; the clinician can pause at any point.
- **Safety net prompts:** drug-allergy, drug-drug interaction, duplicate-order warnings interrupt the flow with explicit confirmation.
- **Single source:** every clinical note is stored in `encounters`; every order is stored in its domain table with FK to the encounter.

### 7.3 Functional Requirements
- FR-CW-0001 A consultation shall create an `encounters` row with `chief_complaint, history, examination, diagnosis, treatment_plan, notes`.
- FR-CW-0002 Prescriptions shall be stored as encounter-linked JSON **[PLANNED]** (Phase 1 captures in `treatment_plan` text).
- FR-CW-0003 Lab orders shall require `lab.order` permission (doctor, lab_technician, super_admin).
- FR-CW-0004 Radiology requests shall follow the lab-order pattern **[PLANNED]**.
- FR-CW-0005 Admission shall require `ipd.manage` and shall atomically mark a bed `occupied`.
- FR-CW-0006 Discharge shall atomically mark the bed `cleaning` and write a `discharge_summary`.
- FR-CW-0007 Follow-up appointments shall be creatable from the encounter screen with one click.
- FR-CW-0008 Referral letters shall be generated as PDF with hospital letterhead **[PLANNED]**.

### 7.4 Clinical Workflow Diagram
```
            ┌────────────────┐
            │ Patient arrives │
            └────────┬───────┘
                     │
                     ▼
            ┌────────────────┐
            │ Reception check│──► Queue token issued
            └────────┬───────┘
                     │
                     ▼
            ┌────────────────┐
            │ Doctor consult │──► Encounter created
            └────────┬───────┘
                     │
        ┌────────────┼────────────┬───────────────┐
        ▼            ▼            ▼               ▼
   ┌─────────┐ ┌─────────┐ ┌──────────┐  ┌──────────────┐
   │Prescribe│ │Lab order│ │Radiology │  │Admit to IPD  │
   └────┬────┘ └────┬────┘ └─────┬────┘  └──────┬───────┘
        │           │            │              │
        ▼           ▼            ▼              ▼
   ┌─────────┐ ┌─────────┐ ┌──────────┐  ┌──────────────┐
   │Pharmacy │ │Lab done │ │Image done │  │ Discharge    │
   │dispense │ │results  │ │report     │  │ + summary    │
   └─────────┘ └────┬────┘ └─────┬────┘  └──────┬───────┘
                    │            │              │
                    └────────────┴──────────────┘
                              │
                              ▼
                    ┌──────────────────┐
                    │ Follow-up booked  │
                    │ / Referral letter │
                    └──────────────────┘
```

### 7.5 Safety Checks
| Trigger | Check | Action |
|---|---|---|
| Prescribe drug | Patient `allergies` matches drug class | Block + force override with reason |
| Prescribe drug | Drug-drug interaction with active Rx | Warn + reason override |
| Lab order | Duplicate order within 24 h | Warn + reason override |
| Lab order | Patient consent for genetic testing | Block if no consent |
| Admission | Bed already occupied | Block |
| Discharge | Outstanding unpaid bill | Warn (allow override by billing_clerk only) |
| Discharge | Pending lab results | Warn |
| Delete patient | Has encounters | Block (RESTRICT FK) |

### 7.6 UX Considerations
- **Doctor flow:** never more than 3 clicks from "open patient" to "create encounter".
- **Lab tech flow:** single screen lists pending orders; click → enter result → save (auto-completes order if all results entered).
- **Nurse flow:** bed board → admit dialog → minimal fields (patient, ward, bed, doctor, diagnosis).
- **Discharge flow:** confirm dialog with summary text field; "Discharge" button is `destructive` style (irreversible).

### 7.7 Desktop Layout — Encounter Editor **[PLANNED]**
```
┌──────────────────────────────────────────────────────────────────────────┐
│ New encounter — Aisha Khan (34F)              ⚠ Penicillin allergy      │
├──────────────────────────────────────────────────────────────────────────┤
│ Chief complaint: [____________________________]                          │
│ History:           [____________________________]                        │
│ Examination:       [____________________________]                        │
│ Diagnosis (ICD-10):[______________▾]                                      │
│ Treatment plan:    [____________________________]                        │
│ Notes:             [____________________________]                        │
├──────────────────────────────────────────────────────────────────────────┤
│ [+ Lab order][+ Radiology][+ Prescription][+ Follow-up][+ Admit]         │
├──────────────────────────────────────────────────────────────────────────┤
│                              [Cancel]              [Save encounter]      │
└──────────────────────────────────────────────────────────────────────────┘
```

### 7.8 Component Specs
- **EncounterForm** **[PLANNED]**: multi-section form; sections collapse to keep visible area small.
- **LabOrderDialog** **[PLANNED]**: multi-select from `lab_test_catalog` + clinical indication.
- **PrescriptionTable** **[PLANNED]**: drug, dose, frequency, duration, quantity, instructions.
- **AdmitDialog** (IPD.tsx): patient, ward, bed, doctor, diagnosis (5 fields).

### 7.9 Interaction Patterns
- **Cmd/Ctrl+S** to save encounter from anywhere in the form.
- **Cmd/Ctrl+Enter** to save and close.
- **Esc** to cancel with unsaved-changes confirmation.

### 7.10 Accessibility
- All form sections are reachable via Tab; no keyboard traps.
- Safety-check dialogs use `role="alertdialog"` with `aria-describedby` for the warning text.
- ICD-10 autocomplete exposes option count to screen readers (`aria-setsize`/`aria-posinset`).

### 7.11 Performance
- Encounter save is a single transaction; lab/radiology/prescription rows are inserted in the same `BEGIN` block.
- Auto-save draft every 30 s to `localStorage` **[PLANNED]**.

### 7.12 Best Practices
- Always show the allergies banner at the top of any clinical screen.
- Capture `clinical_indication` for every lab order — improves result interpretation.
- Use structured diagnosis (ICD-10) wherever possible; free-text fallback allowed.

### 7.13 Implementation Notes
- `encounters` table: `id, patient_id, doctor_id, encounter_date, chief_complaint, history, examination, diagnosis, treatment_plan, notes, created_at`.
- `commands/encounters.rs` exposes `create_encounter, list_encounters, get_encounter, update_encounter` — all `audit::record`-wrapped.
- Lab orders auto-complete the order when the last result is entered (`commands/lab.rs`).

### 7.14 Future Opportunities
- Voice dictation for chief complaint and examination notes.
- Templeted chief-complaint chips for common presentations.
- Clinical decision support (CDSS) hooks for sepsis score, NEWS2, etc.

---

## 8. Department-Specific Interfaces

### 8.1 Objective
Define tailored surfaces for each hospital department so staff see only what their workflow requires.

### 8.2 Departments
| # | Department | Status | Primary role | Permission set |
|---|---|---|---|---|
| 8.3 | Pharmacy | [PLANNED] | `pharmacist` | `inventory.view, inventory.manage, billing.view, patients.view` |
| 8.4 | Laboratory | [IMPLEMENTED] | `lab_technician` | `lab.view, lab.order, lab.result.manage, lab.catalog.manage, inventory.view` |
| 8.5 | Radiology | [PLANNED] | (new role `radiology_tech` **[PLANNED]**) | `lab.view, lab.order, lab.result.manage` |
| 8.6 | Emergency Department (ED) | [PLANNED] | `doctor, nurse, receptionist` | triage fields, fast-track admit |
| 8.7 | ICU | [PLANNED] | `doctor, nurse` | vitals trend, vent settings |
| 8.8 | Wards | [IMPLEMENTED] | `nurse` | `ipd.view, ipd.manage, beds.manage` |
| 8.9 | Operation Theatre (OT) | [PLANNED] | (new role `ot_staff` **[PLANNED]**) | surgery scheduling, checklist |
| 8.10 | Billing | [IMPLEMENTED] | `billing_clerk` | `billing.view, billing.create, billing.manage, payments.manage` |
| 8.11 | Accounts | [PLANNED] | `billing_clerk, super_admin` | receivables, payables, GL |
| 8.12 | Inventory | [PARTIAL] | `pharmacist, lab_technician` | `inventory.view, inventory.manage` |
| 8.13 | Procurement | [PLANNED] | (new role `inventory_manager` **[PLANNED]**) | PO, GRN, supplier mgmt |
| 8.14 | HR | [PLANNED] | (new role `hr_manager` **[PLANNED]**) | staff master, attendance, leave |
| 8.15 | Admin | [IMPLEMENTED] | `super_admin` | all permissions |

### 8.3 Pharmacy **[PLANNED]**
- **Objective:** Manage drug catalogue, stock, dispensing, and controlled-substance register.
- **Layout:** Three-pane (Catalog | Pending prescriptions | Stock levels).
- **Permissions:** `inventory.view, inventory.manage, billing.view, patients.view`.
- **Key tables:** `inventory_items` (drugs), `dispensing_log` **[PLANNED]**, `controlled_substance_register` **[PLANNED]**.
- **Safety:** Expiry-date highlighting (red < 30 days, amber < 90 days); batch-level recall workflow.
- **Dispense flow:** Scan prescription QR → verify patient → reduce stock → print label → mark Rx filled.

### 8.4 Laboratory **[IMPLEMENTED]**
- **Objective:** Receive orders, enter results, auto-complete orders, manage test catalogue.
- **Layout:** Two-pane (Pending orders | Catalog admin).
- **Pages:** `Laboratory.tsx` (Phase 1), with tabs Pending / Completed / Catalog.
- **Permissions:** `lab.view, lab.order, lab.result.manage, lab.catalog.manage`.
- **Tables:** `lab_test_catalog`, `lab_orders`, `lab_order_tests`.
- **Behavior:** Entering the last result auto-completes the parent order (`commands/lab.rs`).
- **Abnormal flagging:** values outside reference range get a red dot in the results table.
- **Best practices:** Always capture `clinical_indication` on the order; show reference range next to result.
- **Performance:** Catalog query cached 60 min; orders poll every 10 s when page focused.
- **Future:** Instrument integration (HL7 ASTM), auto-validation rules.

### 8.5 Radiology **[PLANNED]**
- **Objective:** Order imaging, capture modality/worklist, upload images, report.
- **Permissions:** same as Lab + future `radiology.report.manage`.
- **Modalities:** X-ray, USG, CT, MRI, Mammography.
- **Layout:** Worklist (left), Study viewer (right).
- **Integration:** DICOM viewer **[PLANNED]** via Ohif/Cornerstone embedded.

### 8.6 Emergency Department **[PLANNED]**
- **Objective:** Triage, fast-track admit, resuscitation board.
- **Triage fields:** arrival mode, chief complaint, vitals, GCS, triage category (1–5).
- **Layout:** Live grid of ED bays with patient cards; color-coded by triage level.
- **Quick actions:** Admit → IPD, Discharge, Transfer, Declare dead (with mandatory audit).
- **Performance:** Polling 3 s; entire grid must render in < 200 ms.

### 8.7 ICU **[PLANNED]**
- **Objective:** Critical-care overview with vitals trend, vent settings, lines/drains.
- **Layout:** One bed per "tile" with mini vitals sparkline; click to expand.
- **Integration:** HL7 vitals feed from monitors **[PLANNED]**.
- **Safety:** Critical-value alerts (HR < 40, SpO2 < 90%) escalate to charge nurse.

### 8.8 Wards **[IMPLEMENTED]**
- **Objective:** Bed management, vitals capture, I/O charting.
- **Layout:** See §9 Interactive Bed Management.
- **Permissions:** `ipd.view, ipd.manage, beds.manage`.
- **Tables:** `wards`, `beds`, `ipd_admissions`.
- **Behavior:** Admit → bed `occupied`; Discharge → bed `cleaning` → housekeeping → `available`.
- **Best practices:** Cleaning state visible at a glance so housekeeping knows where to go.

### 8.9 Operation Theatre **[PLANNED]**
- **Objective:** Surgery scheduling, pre-op checklist, intra-op log, post-op handover.
- **Tables:** `ot_sessions` **[PLANNED]**, `ot_checklists` **[PLANNED]**.
- **Layout:** OT calendar (rooms × time grid), checklist sidebar.
- **Safety:** WHO Surgical Safety Checklist (Sign In / Time Out / Sign Out) — mandatory before status can advance.

### 8.10 Billing **[IMPLEMENTED]**
- **Objective:** Generate invoices, post payments, track outstanding.
- **Layout:** Tabs: Invoices | Payments | Outstanding. See §10.
- **Permissions:** `billing.view, billing.create, billing.manage, payments.manage`.
- **Tables:** `bills`, `bill_items`, `payments`.
- **Behavior:** Server-side totals; `bill.total` is recomputed on every save; client never sends totals.
- **Receipt:** `components/Receipt.tsx` renders a printable receipt.

### 8.11 Accounts **[PLANNED]**
- **Objective:** Receivables aging, payables, GL export.
- **Permissions:** `billing.view, billing.manage` + future `accounts.view`.
- **Layout:** Aging buckets (0–30/31–60/61–90/90+) + journal entry list.
- **Integration:** Tally / QuickBooks export **[PLANNED]**.

### 8.12 Inventory **[PARTIAL]**
- **Objective:** Track stock across all stores (pharmacy, lab consumables, general).
- **Tables:** `inventory_items`.
- **Phase 1:** List + edit only.
- **Phase 2:** Stock movements, batch tracking, expiry alerts, reorder levels.

### 8.13 Procurement **[PLANNED]**
- **Objective:** Purchase orders, GRN, supplier management.
- **New role:** `inventory_manager`.
- **Layout:** Three-column (Suppliers | POs | GRNs).
- **Integration:** Auto-PO when stock falls below reorder level.

### 8.14 HR **[PLANNED]**
- **Objective:** Staff master, attendance, leave, payroll prep.
- **New role:** `hr_manager`.
- **Tables:** `staff` **[PLANNED]**, `attendance` **[PLANNED]**, `leave_requests` **[PLANNED]**.
- **Integration:** Biometric attendance (employee code) **[PLANNED]**.

### 8.15 Admin **[IMPLEMENTED]**
- **Objective:** User management, role assignment, settings, license, backups.
- **Pages:** `Users.tsx`, `Settings.tsx`, `AuditLog.tsx`.
- **Permissions:** `users.view, users.manage, roles.manage, settings.manage, license.manage, backups.manage, audit.view`.
- **Best practices:** Super Admin is a "break-glass" role — log every use to `audit_logs` with elevated severity.

### 8.16 UX Considerations
- Each department page should have a distinctive header color accent (within the palette) so users orient instantly.
- Cross-department handoffs (e.g. ED → ICU) should require only one confirmation.

### 8.17 Component Specs
- All department pages share the `Card + Table + Dialog` triad.
- Department-specific components (TriageForm, VentSettingsForm) live under `src/components/department/` **[PLANNED]**.

### 8.18 Interaction Patterns
- Tab switching: keyboard `Alt+1..9` jumps between tabs **[PLANNED]**.
- List row → dialog: single click for view, dedicated Edit button for write.

### 8.19 Accessibility
- Tablist uses `role="tablist"` with `aria-selected` and `aria-controls`.
- Status indicators are dual-coded (color + icon).

### 8.20 Performance
- Department pages lazy-load **[PLANNED]** to reduce initial bundle.

### 8.21 Best Practices
- Keep each department page ≤ 2 main actions.
- Cross-link from one department to another via deep links (e.g. Lab result → Patient 360 → Encounter).

### 8.22 Implementation Notes
- Department-aware permission checks use the same `rbac::require` pattern.
- The `departments` table seeds: Pharmacy, Lab, Radiology, ED, ICU, Ward, OT, Billing, Inventory, HR, Admin.

### 8.23 Future Opportunities
- Per-department branding (logo + accent color).
- Department-level dashboard cards on the main Dashboard.

---

## 9. Interactive Bed Management

### 9.1 Objective
Provide a visual floor map of every ward with real-time bed status, patient assignment, cleaning queue and isolation flags. This is the most visually distinct surface in the system.

### 9.2 Design Principles
- **Spatial fidelity:** beds are laid out as they are physically arranged in the ward.
- **Glanceable status:** color + icon + label per bed; legible from 3 m.
- **One-click actions:** admit, discharge, mark cleaning, reserve — all from the bed tile.
- **No phantom beds:** once a bed is created, its position is stable.

### 9.3 Functional Requirements
- FR-BM-0001 The bed board shall render all beds grouped by ward.
- FR-BM-0002 Bed status shall be one of: `available, occupied, maintenance, cleaning, reserved` **[reserved PLANNED]**.
- FR-BM-0003 Admitting a patient shall atomically set the bed `occupied` and create an `ipd_admissions` row (`commands/ipd.rs` transaction).
- FR-BM-0004 Discharging shall set the bed `cleaning` and write `discharge_summary`.
- FR-BM-0005 Housekeeping shall be able to flip `cleaning → available` via a single click **[PLANNED]**.

### 9.4 Bed Status Color Map
| Status | Token | Hex | Icon |
|---|---|---|---|
| Available | `--status-confirmed` | `#22C55E` | ✓ |
| Occupied | `--status-cancelled` | `#DC2626` | ● |
| Maintenance | `--status-no-show` | `#F59E0B` | 🔧 |
| Cleaning | `--status-scheduled` | `#0EA5E9` | 🧹 |
| Reserved | (planned) | `#8B5CF6` | 📌 |

### 9.5 Desktop Layout — Ward Floor Map
```
Ward: General A   ┌─ Available 12  Occupied 8  Cleaning 2  Maint 1 ─ 23/24 ─┐
                  └────────────────────────────────────────────────────────┘
   ┌────┐ ┌────┐ ┌────┐ ┌────┐ ┌────┐ ┌────┐ ┌────┐ ┌────┐
   │ ✓  │ │ ●  │ │ ✓  │ │ 🧹 │ │ ●  │ │ ✓  │ │ 🔧 │ │ ●  │
   │A-01│ │A-02│ │A-03│ │A-04│ │A-05│ │A-06│ │A-07│ │A-08│
   └────┘ └────┘ └────┘ └────┘ └────┘ └────┘ └────┘ └────┘
   ┌────┐ ┌────┐ ┌────┐ ┌────┐ ┌────┐ ┌────┐ ┌────┐ ┌────┐
   │ ●  │ │ ✓  │ │ ●  │ │ ✓  │ │ ●  │ │ ✓  │ │ ●  │ │ ✓  │
   │A-09│ │A-10│ │A-11│ │A-12│ │A-13│ │A-14│ │A-15│ │A-16│
   └────┘ └────┘ └────┘ └────┘ └────┘ └────┘ └────┘ └────┘

ICU (isolation-capable) ───────────────────────────────────────
   ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐
   │ ● ISO  │ │ ✓      │ │ ●      │ │ 📌     │
   │I-01    │ │I-02    │ │I-03    │ │I-04    │
   └────────┘ └────────┘ └────────┘ └────────┘
```

### 9.6 Component Specs
- **BedTile:** square card 96×96 px; bed number, status icon, patient initials (if occupied).
- **WardHeader:** ward name + occupancy summary.
- **AdmitDialog:** patient select, doctor select, admitting diagnosis, expected stay.
- **DischargeDialog:** discharge summary textarea, follow-up checkbox.

### 9.7 Interaction Patterns
- **Hover:** tooltip shows patient name, doctor, admit date, diagnosis (if occupied).
- **Click available:** open Admit dialog with bed pre-selected.
- **Click occupied:** open patient summary with "Discharge" button.
- **Click cleaning:** single-click flips to `available` (with confirm).
- **Drag patient:** reassign to another available bed **[PLANNED]**.

### 9.8 Accessibility
- Bed tiles are keyboard-navigable (Tab); `Enter` triggers the contextual action.
- `aria-label` per bed: "Bed A-02, occupied by Aisha Khan, admitted 2026-07-01".

### 9.9 Performance
- Bed list is small (typical hospital < 500 beds); single query renders all wards.
- Polling every 10 s when IPD page focused **[PLANNED]**; Phase 1 uses manual refresh.

### 9.10 Best Practices
- Color + icon + label per status (triple-coded) so color-blind staff can identify state.
- Show "Last cleaned: 2026-07-03 14:22" on hover for infection-control audits.
- Reserve ICU beds with isolation flag for infectious cases.

### 9.11 Implementation Notes
- `beds` table: `id, ward_id, label, status, is_isolation, current_admission_id`.
- `commands/ipd.rs::admit_patient` runs `BEGIN → UPDATE beds SET status='occupied' → INSERT ipd_admissions → COMMIT`.
- `commands/ipd.rs::discharge_patient` runs `BEGIN → UPDATE ipd_admissions SET discharged_at, summary → UPDATE beds SET status='cleaning' → COMMIT`.

### 9.12 Future Opportunities
- Real-time updates via Tauri events (no polling).
- 3D ward visualization for large hospitals.
- Predictive bed turnover based on historical discharge times.

---

## 10. Billing & Financial Interfaces

### 10.1 Objective
Provide accurate, auditable billing for outpatient visits, IPD stays, lab/radiology orders, pharmacy and procedures. Support payments (cash/card/UPI/insurance), refunds, voids and aging reports.

### 10.2 Design Principles
- **Server-authoritative totals:** the client never sends `total`; `billing.rs` recomputes from `bill_items`.
- **Audit every write:** create, update, void, payment, refund all call `audit::record`.
- **Printable receipts:** thermal (80 mm) and A4 layouts.
- **Insurance-aware:** split billing for insurance vs patient-pay portions **[PLANNED]**.

### 10.3 Functional Requirements
- FR-BL-0001 Creating a bill shall require `billing.create` permission.
- FR-BL-0002 Adding a bill item shall immediately recompute `total` server-side.
- FR-BL-0003 Posting a payment shall require `payments.manage`.
- FR-BL-0004 Voiding a bill shall require `billing.manage` and a reason.
- FR-BL-0005 Receipts shall be printable to default printer via Tauri print API.
- FR-BL-0006 Outstanding balance shall = total − sum(payments).

### 10.4 UX Considerations
- **Cashier pace:** large "Cash" / "Card" / "UPI" buttons; amount auto-fills from outstanding.
- **Patient transparency:** receipt shows each line item, qty, rate, amount.
- **Insurance split:** when patient has insurance, show two balances side-by-side **[PLANNED]**.

### 10.5 Desktop Layout — Billing Page
```
┌──────────────────────────────────────────────────────────────────────────┐
│ Billing & Invoices              [+ New invoice]                          │
├──────────────────────────────────────────────────────────────────────────┤
│ Tabs: Invoices | Payments | Outstanding                                  │
│ Filters: [Patient][Status▾][Date range]                                  │
├──────────────────────────────────────────────────────────────────────────┤
│ #INV-00123  Aisha Khan   $245.00   Paid      2026-07-03  [Print][⋯]     │
│ #INV-00124  Bikram Singh $1,820.00 Partial  2026-07-03  [Collect][⋯]    │
│ #INV-00125  Cara Lopez   $90.00    Outstanding 2026-07-02 [Collect][⋯]  │
└──────────────────────────────────────────────────────────────────────────┘
```

### 10.6 New Invoice Dialog
```
┌──────────────────────────────────────────────────────────────────────────┐
│ New invoice                                                  [×]         │
├──────────────────────────────────────────────────────────────────────────┤
│ Patient: [Aisha Khan ▾]   Doctor: [Dr. Ahmed ▾]   Date: 2026-07-03       │
├──────────────────────────────────────────────────────────────────────────┤
│ Items:                                                                   │
│ ┌─────────────────────┬──────┬─────────┬──────────┬──────┐              │
│ │ Service             │ Qty  │ Rate    │ Amount   │      │              │
│ │ Consultation        │ 1    │ $50.00  │ $50.00   │ [×]  │              │
│ │ CBC (Lab)           │ 1    │ $25.00  │ $25.00   │ [×]  │              │
│ │ Pharmacy - Amox     │ 10   │ $2.50   │ $25.00   │ [×]  │              │
│ └─────────────────────┴──────┴─────────┴──────────┴──────┘              │
│ [+ Add item from catalogue]                                              │
├──────────────────────────────────────────────────────────────────────────┤
│ Subtotal: $100.00   Tax (0%): $0.00   Discount: [____]                   │
│ TOTAL: $100.00                                                           │
├──────────────────────────────────────────────────────────────────────────┤
│                                  [Cancel]    [Save]    [Save & Print]    │
└──────────────────────────────────────────────────────────────────────────┘
```

### 10.7 Component Specs
- **BillingTable** (`Billing.tsx`): invoice #, patient, total, paid, status badge, actions.
- **InvoiceForm** **[PLANNED]**: line-item editor with searchable service catalogue.
- **PaymentDialog** **[PLANNED]**: amount, method (cash/card/UPI/insurance), reference, notes.
- **Receipt** (`components/Receipt.tsx`): printable layout, hospital header, itemized lines, signature block.

### 10.8 Interaction Patterns
- **Quick collect:** from the Outstanding tab, click "Collect" → payment dialog with amount pre-filled.
- **Print:** uses Tauri's `webview.print()` to print the receipt view.
- **Void:** destructive button + reason textarea; audit-logged with severity `warning`.

### 10.9 Accessibility
- All amounts use `tabular-nums` for column alignment.
- Status badges dual-coded (color + text).
- Receipt print view uses semantic HTML (`<table>`, `<th scope>`).

### 10.10 Performance
- Bills query is paginated by 50; outstanding view filters server-side.
- Server-side `total` computation prevents client-side drift.

### 10.11 Best Practices
- Never reuse an invoice number — even voided ones keep their #.
- Show "Printed by {user} at {ts}" on every printed receipt.
- Daily cash tally report at end of cashier shift **[PLANNED]**.

### 10.12 Implementation Notes
- Tables: `bills(id, patient_id, encounter_id, subtotal, tax, discount, total, status, created_by, created_at, voided_at, void_reason)`, `bill_items(id, bill_id, description, qty, rate, amount)`, `payments(id, bill_id, amount, method, reference, received_by, received_at)`.
- `commands/billing.rs::create_bill` recomputes `total = sum(items.amount) - discount + tax`.
- `payments.manage` permission is distinct from `billing.create` so cashiers can post payments without editing invoices.

### 10.13 Future Opportunities
- Insurance claim submission (HL7 837) **[PLANNED]**.
- GST/VAT auto-calc by service category.
- Patient wallet (prepaid credit) for OPD visits.

---

## 11. Inventory & Pharmacy

### 11.1 Objective
Track every consumable and drug in the hospital from procurement through dispensing. Ensure no stock-outs of critical items, no use of expired stock, full audit trail of every movement.

### 11.2 Design Principles
- **Batch-level tracking:** every received quantity has a batch number + expiry.
- **FEFO enforcement:** First-Expiry-First-Out on dispensing.
- **Reorder automation:** auto-suggest PO when stock < reorder level.
- **Controlled-substance rigor:** separate register for narcotics with two-person verification.

### 11.3 Functional Requirements
- FR-IP-0001 Listing inventory shall require `inventory.view`.
- FR-IP-0002 Editing stock shall require `inventory.manage`.
- FR-IP-0003 Items below reorder level shall appear in a "Low stock" surface **[PLANNED]**.
- FR-IP-0004 Items expiring < 90 days shall be flagged amber; < 30 days red **[PLANNED]**.
- FR-IP-0005 Every stock movement (receive, dispense, return, adjust) shall be audit-logged **[Implemented v0.2.0 (Batch 1 CR-21)]** — the `adjust_inventory` Tauri command in `src-tauri/src/commands/inventory.rs` writes an audit row for every stock adjustment. The full movement history (receive / dispense / return) is still Batch 5.
- FR-IP-0006 Controlled substances shall require two-person sign-off **[PLANNED]**.

**[Implemented v0.2.0 (Batch 1 CR-21)]** — the Inventory page (`src/pages/Inventory.tsx`, 905 LOC) is now in the application at the `/inventory` route. The page is permission-gated via `InventoryView` / `InventoryManage` (added to the `Permission` enum in `src-tauri/src/rbac.rs` and `src/lib/rbac.ts`). The 6 inventory IPC commands (`list_inventory_items`, `get_inventory_item`, `create_inventory_item`, `update_inventory_item`, `adjust_inventory`, `delete_inventory_item`) are registered in `lib.rs::generate_handler![]` and exposed via the TanStack Query hooks in `src/lib/queries.ts`. The previous v0.1.0 "Phase 2" markers on FR-IP-0001/0002/0005 are resolved for the listing + editing + audit primitives.

### 11.4 UX Considerations
- **Pharmacist view:** dashboard tiles for Low stock, Expiring soon, Pending prescriptions.
- **Inventory manager view:** PO suggestions, GRN queue, supplier performance.
- **Doctor view:** read-only catalogue lookup for prescribing.

### 11.5 Desktop Layout — Inventory List (Phase 1)
```
┌──────────────────────────────────────────────────────────────────────────┐
│ Inventory                          [+ New item]                          │
├──────────────────────────────────────────────────────────────────────────┤
│ Search: [_____________]   Filter: [Category▾][Store▾]                    │
├──────────────────────────────────────────────────────────────────────────┤
│ Item            Category   Store    Qty   Reorder   Expiry     Actions   │
│ Amoxicillin 500 Drug       Pharmacy 240   100       2027-03   [Edit]    │
│ Syringe 5ml     Consumable Lab       1200  500       —          [Edit]    │
│ ...                                                                      │
└──────────────────────────────────────────────────────────────────────────┘
```

### 11.6 Desktop Layout — Pharmacy Dispense (Phase 2)
```
┌──────────────────────────────────────────────────────────────────────────┐
│ Pending prescriptions (3)              Selected: Rx-2026-07-03-001       │
├──────────────────────────────────────────┬───────────────────────────────┤
│ Rx-001  Aisha Khan    Amox 500 ×10      │ Patient: Aisha Khan           │
│ Rx-002  Bikram Singh  Metformin 500 ×60 │ Drug: Amoxicillin 500 mg      │
│ Rx-003  Cara Lopez    Insulin glargine  │ Dose: 1 BD × 5 days           │
│                                          │ Qty: 10                       │
│                                          │ Stock: 240 (batch AB-2025)    │
│                                          │ Allergy check: ✓ No conflict  │
│                                          │ [Dispense]  [Hold]  [Return]  │
└──────────────────────────────────────────┴───────────────────────────────┘
```

### 11.7 Component Specs
- **InventoryTable** (`Inventory.tsx` — **[Implemented v0.2.0 Batch 1 CR-21]**): item, category, store, qty, reorder, expiry, actions. The page exposes the table + a create/edit dialog + a stock-adjust dialog (writes a movement + audit row).
- **StockAdjustDialog** **[Implemented v0.2.0 Batch 1 CR-21]**: quantity delta + reason (received/damaged/returned/adjustment). Calls `adjust_inventory` IPC; the Rust handler writes both an `inventory_movements` row and an `audit_logs` row.
- **DispenseDialog** **[PLANNED — Batch 5]**: scan Rx → verify patient → reduce stock → print label.

### 11.12 Implementation Notes
- `inventory_items` table: `id, name, category, store, quantity, unit, reorder_level, batch_no, expiry_date, cost_price, selling_price, is_active`. **[Implemented v0.2.0 Batch 1 CR-21]** — table + 6 IPC commands live.
- Stock movements will live in `inventory_movements` **[Implemented v0.2.0 Batch 1 CR-21]**: `id, item_id, type (in/out/adjust), quantity, reason, reference, performed_by, performed_at`. The `adjust_inventory` command writes here. Full movement history UI (receive / dispense / return) is still Batch 5.
- Pharmacy module will be a separate page at `/pharmacy` **[PLANNED — Batch 5]**.

### 11.8 Interaction Patterns
- **Batch selection:** when dispensing, system suggests the earliest-expiring batch (FEFO).
- **Low-stock banner:** appears at top of Dashboard for `pharmacist` role.
- **Expiry sweep:** scheduled job flags expiring items daily **[PLANNED]**.

### 11.9 Accessibility
- Table rows keyboard-navigable; action buttons reachable via Tab.
- Color-coded expiry uses both color + text ("Expires in 22 days").

### 11.10 Performance
- Inventory list cached 5 min; mutations invalidate.
- Expiry sweep is a nightly batch job, not a UI concern.

### 11.11 Best Practices
- Never delete an inventory item; mark `is_active = false`.
- Physical count adjustment requires `inventory.manage` + reason + audit log.
- Reorder levels reviewed quarterly.

### 11.12 Implementation Notes
- `inventory_items` table: `id, name, category, store, quantity, unit, reorder_level, batch_no, expiry_date, cost_price, selling_price, is_active`.
- Stock movements will live in `inventory_movements` **[PLANNED]**: `id, item_id, type (in/out/adjust), quantity, reason, reference, performed_by, performed_at`.
- Pharmacy module will be a separate page at `/pharmacy` **[PLANNED]**.

### 11.13 Future Opportunities
- Barcode/QR scanning at receiving and dispensing.
- Supplier portal for direct PO acknowledgements.
- Cold-chain temperature logging for vaccines.

---

## 12. Analytics & Reporting

### 12.1 Objective
Transform operational data into actionable insights: patient volume trends, revenue mix, bed occupancy, lab turnaround time, doctor productivity, no-show rates. Reports must be exportable (PDF/CSV/XLSX) and schedulable.

### 12.2 Design Principles
- **Visual first:** charts > tables for trend; tables > charts for detail.
- **Role-scoped:** doctors see their own productivity; admins see hospital-wide.
- **Drill-down everywhere:** click a chart bar to filter the underlying table.
- **Exportable:** every report has a one-click export to PDF/CSV/XLSX.

### 12.3 Functional Requirements
- FR-AR-0001 Reports page shall require `reports.view` permission (currently disabled in sidebar as "Phase 2").
- FR-AR-0002 Reports shall cover: OPD volume, IPD occupancy, Revenue mix, Lab TAT, Doctor productivity, No-show rate, Stock consumption.
- FR-AR-0003 Reports shall support date-range filters.
- FR-AR-0004 Reports shall be exportable to CSV and PDF **[PLANNED]**.
- FR-AR-0005 Scheduled email reports **[PLANNED]**.

### 12.4 Report Catalogue
| Report | Audience | Visualization | Granularity |
|---|---|---|---|
| OPD volume | Admin, Doctor | Bar chart | Daily/weekly/monthly |
| IPD occupancy | Admin, Nurse | Heatmap | Ward × day |
| Revenue mix | Admin, Billing | Stacked bar | Service category |
| Lab turnaround time | Admin, Lab | Line chart | Test × day |
| Doctor productivity | Doctor, Admin | Table | Doctor × week |
| No-show rate | Admin, Receptionist | Donut | Status mix |
| Stock consumption | Pharmacist, Inventory Mgr | Line chart | Item × month |
| Audit activity | Admin | Table | User × action |

### 12.5 UX Considerations
- **Defaults:** last 30 days, all departments, all doctors.
- **Comparison:** toggle "Compare to previous period" for delta percentages.
- **Saved views:** users pin their favorite filter combos **[PLANNED]**.

### 12.6 Desktop Layout — Reports Page (Phase 2)
```
┌──────────────────────────────────────────────────────────────────────────┐
│ Reports                              [Date range][Export PDF][Export CSV]│
├──────────────────────────────────────────────────────────────────────────┤
│ Sidebar:                                                                 │
│  ▣ Operations                                                            │
│  ▣ Financial                                                             │
│  ▣ Clinical                                                              │
│  ▣ Inventory                                                             │
│  ▣ Audit                                                                 │
├──────────────────────────────────────────────────────────────────────────┤
│ OPD volume — last 30 days                                                │
│ ┌────────────────────────────────────────────────────────────────────┐   │
│ │ ▓▓▓                                                                │   │
│ │ ▓▓▓▓▓                                                              │   │
│ │ ▓▓▓▓▓▓▓                                                            │   │
│ └────────────────────────────────────────────────────────────────────┘   │
│ Total: 1,240 visits  ↑ 8% vs previous period                            │
└──────────────────────────────────────────────────────────────────────────┘
```

### 12.7 Component Specs
- **ReportSidebar**: category tree navigation.
- **DateRangePicker**: shadcn calendar-based popover.
- **Chart components**: Recharts (`<BarChart>`, `<LineChart>`, `<PieChart>` already in use on Dashboard).
- **ExportButton**: triggers Tauri file-save dialog → writes CSV/PDF to chosen path.

### 12.8 Interaction Patterns
- Click a chart bar → opens filtered detail table below.
- Drag-to-zoom on time-series charts **[PLANNED]**.
- Pin report to Dashboard **[PLANNED]**.

### 12.9 Accessibility
- Charts include a screen-reader-only data table alternative **[PLANNED]**.
- Color choices recharts-friendly (sky/teal/slate/amber/red palette).

### 12.10 Performance
- Heavy aggregates run server-side (PostgreSQL `GROUP BY`).
- Reports cached for 5 min; date-range change forces refetch.

### 12.11 Best Practices
- Never report on data you cannot audit — every metric traces to a base table.
- Show "Generated at {ts} by {user}" on every exported PDF.
- Avoid vanity metrics; tie every report to a decision.

### 12.12 Implementation Notes
- Phase 1: Dashboard KPIs are the only analytics surface.
- Phase 2: dedicated `/reports` route with role-scoped report list.
- Long-running reports: generate async, store in `report_results` table, notify user.

### 12.13 Future Opportunities
- Natural-language query ("Show me last month's revenue by department").
- Anomaly detection with explanations.
- Hospital benchmarking (anonymized, opt-in).

---

## 13. Global Search

### 13.1 Objective
Provide a single keystroke (`Ctrl+K`) that opens a command palette capable of finding any entity (patient, doctor, appointment, bill, lab order, bed, audit entry) and executing any quick action.

### 13.2 Design Principles
- **Universal:** one search box, every entity.
- **Keyboard-first:** type to filter, arrow keys to navigate, Enter to open.
- **Recent + pinned:** surface recent searches and pinned items above results.
- **Action-oriented:** search returns both entities and actions ("Create new patient").

### 13.3 Functional Requirements
- FR-GS-0001 `Ctrl+K` (Windows) shall open the command palette from any page.
- FR-GS-0002 Search shall query patients, doctors, appointments, bills, lab orders, beds, audit entries.
- FR-GS-0003 Each result shall show entity type icon + label + sub-label.
- FR-GS-0004 Arrow-key navigation with `Enter` to open, `Esc` to close.
- FR-GS-0005 Recent searches persisted in `localStorage` **[PLANNED]**.

### 13.4 Search Result Types
| Type | Icon | Label | Sub-label | Deep-link |
|---|---|---|---|---|
| Patient | `Users` | Full name | Phone · MRN | `/patients/:id` |
| Doctor | `Stethoscope` | Full name | Specialization | `/doctors/:id` |
| Appointment | `Calendar` | Patient × Doctor | Date · status | `/appointments?id=` |
| Bill | `Receipt` | Invoice # | Patient · total | `/billing?inv=` |
| Lab order | `FlaskConical` | Order # | Patient · status | `/laboratory?order=` |
| Bed | `BedDouble` | Bed label | Ward · status | `/ipd?bed=` |
| Audit entry | `ScrollText` | Action · entity | User · ts | `/audit?id=` |
| Action | `Plus` | "Create new patient" | — | dialog |

### 13.5 UX Considerations
- **Recency bias:** last 5 searches surface at top.
- **Fuzzy match:** typos tolerated (e.g. "aisha" matches "Aisha").
- **Permission-aware:** results filtered by `has(perm)` — a billing clerk cannot see `/audit` entries.

### 13.6 Desktop Layout — Command Palette
```
┌──────────────────────────────────────────────────────────────────────────┐
│ 🔍  aisha                                                          Esc ✕ │
├──────────────────────────────────────────────────────────────────────────┤
│ Recent                                                                    │
│   👤 Aisha Khan          98765 4321  · MRN-0001234     ↵ Open           │
│   👤 Aisha Verma         99887 7665  · MRN-0000987     ↵ Open           │
│ Actions                                                                   │
│   ➕ Create new patient named "aisha"                                  ↵ │
├──────────────────────────────────────────────────────────────────────────┤
│ ↑↓ navigate   ↵ open   Esc close                                         │
└──────────────────────────────────────────────────────────────────────────┘
```

### 13.7 Component Specs
- Built on shadcn `Command` primitive (already in `src/components/ui/command.tsx`).
- Wrapper: `<CommandPalette open onClose>` registers the global `Ctrl+K` listener.
- Search backend: a new `global_search` Rust command that unions results across tables **[PLANNED]**.

### 13.8 Interaction Patterns
- `Ctrl+K` toggles open/close.
- Type → debounced 150 ms → backend query.
- `↑/↓` selects; `Enter` opens; `Tab` filters to current type.
- `Esc` closes.

### 13.9 Accessibility
- `role="dialog"` + `aria-modal="true"`.
- Each result row `role="option"` with `aria-selected`.
- Search input has `aria-label="Global search"`.

### 13.10 Performance
- Backend search uses PostgreSQL full-text (`tsvector` + `tsquery`) **[PLANNED]**.
- Phase 1: client-side filter on already-loaded lists (works for small hospitals).

### 13.11 Best Practices
- Show result count ("12 results in 38 ms").
- Allow `> ` prefix to search actions only.
- Highlight matched substring in results.

### 13.12 Implementation Notes
- shadcn `Command` (cmdk) is already installed; only the wrapper is new.
- Global listener: `useEffect(() => window.addEventListener("keydown", e => e.ctrlKey && e.key === "k" && setOpen(true)))`.

### 13.13 Future Opportunities
- Natural-language search ("show me unpaid bills over 30 days").
- Voice search.
- Cross-hospital federated search (group hospitals).

---

## 14. Accessibility (WCAG 2.2 AA)

### 14.1 Objective
Ensure VitalFlow HMS is usable by clinicians, patients and administrators with diverse abilities, meeting WCAG 2.2 Level AA conformance. Aligns with ISO 25010 Usability sub-characteristic Accessibility and ISO 27001 A.5.30 (ICT accessibility).

### 14.2 Design Principles
- **Perceivable:** information presentable in ways users can perceive (text alternatives, contrast, resizable).
- **Operable:** interface operable via keyboard, no time-pressure traps.
- **Understandable:** content and operation predictable.
- **Robust:** compatible with assistive technologies (NVDA, JAWS on Windows).
- **Clinical-context aware:** accessibility never compromises patient safety.

### 14.3 Functional Requirements
- FR-AC-0001 All interactive elements shall be reachable and operable via keyboard.
- FR-AC-0002 Color contrast shall meet WCAG 2.2 AA (4.5:1 text, 3:1 UI components).
- FR-AC-0003 All form fields shall have associated `<label>` or `aria-label`.
- FR-AC-0004 All status messages shall use `role="status"` or `role="alert"`.
- FR-AC-0005 `prefers-reduced-motion` shall disable non-essential animations.
- FR-AC-0006 Focus order shall follow visual order; focus indicator always visible.
- FR-AC-0007 Page titles shall be unique and descriptive.
- FR-AC-0008 Target size minimum 24×24 CSS pixels (WCAG 2.2 SC 2.5.8).

### 14.4 WCAG 2.2 Conformance Checklist

**[Updated v0.2.0 (Batch 3 — a11y pass: A11Y-01/03/04)]** — the Phase 1 audit found the v0.1.0 checklist claimed "Pass" on several success criteria the implementation actually violated. Batch 3 ran an a11y pass that added `aria-label`s on icon-only buttons, `DialogDescription` on every shadcn Dialog, `scope="col"` defaults on `TableHead`, and `htmlFor` on the Login + Setup forms. The table below honestly reflects the current state — some SCs moved from Partial → Improved; some still have residual Batch 5 follow-ups.

| SC | Requirement | Status | Evidence |
|---|---|---|---|
| 1.1.1 Non-text Content | All icons have `aria-label` or visible text | Improved **[v0.2.0 A11Y-01]** | Header buttons + icon-only action buttons now carry `aria-label`. Some KPI onClick wrappers still lack label — Batch 5 follow-up. |
| 1.3.1 Info and Relationships | Semantic HTML; `<table>`, `<nav>`, `<main>` | Improved **[v0.2.0 A11Y-04]** | `TableHead` defaults to `scope="col"` in `src/components/ui/table.tsx`; `DialogDescription` provides an accessible name relationship for every Dialog. |
| 1.4.3 Contrast (Minimum) | Text ≥ 4.5:1 | Pass | Design tokens meet AA — verified for both light + dark themes (sky-500 `#0EA5E9` on slate-50 `#F8FAFC` is 3.6:1 for large text; the dark-theme sky-400 `#38BDF8` on slate-950 `#0B0F19` is 8.7:1). Body text uses foreground tokens that meet 4.5:1. |
| 1.4.10 Reflow | 1280×1024 at 400% zoom | Pass | Responsive layout. |
| 1.4.11 Non-text Contrast | UI components ≥ 3:1 | Pass | Borders, focus rings verified — `--shadow-glow: 0 0 0 3px rgb(14 165 233 / 0.15)` is visible against all surfaces. |
| 2.1.1 Keyboard | All functionality keyboard-operable | Pass | All dialogs, menus, forms. shadcn primitives are keyboard-first. |
| 2.1.2 No Keyboard Trap | Esc closes every modal | Pass | All Dialog components. **[v0.2.0 A11Y-03]** `DialogDescription` is now present on every Dialog so screen readers announce context before focus lands inside. |
| 2.4.3 Focus Order | DOM order matches visual | Pass | Sidebar → header → main. |
| 2.4.7 Focus Visible | Focus ring always visible | Pass | `--shadow-glow` ring (sky-500 at 15% alpha) — meets SC 2.4.7. |
| 2.5.8 Target Size (Minimum) | 24×24 px | Pass | All buttons ≥ 32 px. |
| 3.2.1 On Focus | No context change on focus | Pass | — |
| 3.3.1 Error Identification | Field-level errors with text | Improved **[v0.2.0 INT-01]** | Form errors shown inline. **[v0.2.0 INT-01]** The remaining `window.confirm()` calls in Billing/Queue/IPD/Users/Laboratory were replaced with shadcn Dialogs. Some Select-based form fields still need explicit error text — Batch 5. |
| 3.3.2 Labels or Instructions | All inputs labeled | Improved **[v0.2.0]** | `<Label htmlFor>` is on every Login + Setup field. **[Batch 5 follow-up]** Select-field labels on Billing/Queue/IPD/Users/Laboratory forms still need `htmlFor` wiring (Select primitives don't take a `for` attribute directly — needs an id on the trigger). |
| 4.1.2 Name, Role, Value | ARIA on custom widgets | Improved **[v0.2.0 A11Y-01]** | shadcn primitives provide role + name out of the box; icon-only buttons now have `aria-label`. Some custom widgets may still need explicit `aria-label` — Batch 5. |
| 4.1.3 Status Messages | `role="status"` for toasts | Pass | Sonner handles — toasts are announced via aria-live. |

**Note (v0.2.0 Batch 3):** the a11y pass improved conformance across 5 SCs (1.1.1, 1.3.1, 2.1.2, 3.3.1, 4.1.2). Remaining items — Select-field `htmlFor` wiring on Billing/Queue/IPD/Users/Laboratory forms, more `aria-label`s on KPI onClick wrappers, a `<SkipLink>` to jump past the sidebar (§14.7), and axe-core in CI — are targeted for Batch 5. See `03-Quality-Model-ISO-25010.md` §6.6 for the ISO 25010 Accessibility sub-characteristic (downgraded from aspirational L4 to actual L3).

### 14.5 UX Considerations
- **Color-blindness:** status badges dual-coded (color + dot + text).
- **Low vision:** dark theme eases eye strain; 14 px minimum body text.
- **Motor impairment:** all actions keyboard-reachable; no double-click required.
- **Cognitive load:** consistent layout, predictable navigation, undo on destructive actions.

### 14.6 Desktop Layout
The application must be fully usable at 200% browser-equivalent zoom (Tauri webview respects OS scaling). At 200%, the sidebar auto-collapses (planned), and all dialogs remain within viewport.

### 14.7 Component Specs
- `aria-live="polite"` region mounted at root for toast announcements.
- `<SkipLink>` to jump past the sidebar to main content **[PLANNED — Batch 5]**.
- `<VisuallyHidden>` for icon-only button labels. **[Implemented v0.2.0 (Batch 3 A11Y-01)]** icon-only action buttons in tables and headers now carry `aria-label` (some via VisuallyHidden, some via direct aria-label attribute).

### 14.8 Interaction Patterns
- `Tab` forward, `Shift+Tab` backward.
- `Esc` closes topmost modal/popover.
- `Enter`/`Space` activates buttons.
- `Alt+↓` opens selects/dropdowns.

### 14.9 Accessibility
- Self-referential — this section defines the standard. Continuous testing with axe-core **[PLANNED]** in CI.

### 14.10 Performance
- Reduced-motion media query collapses animations to 0.01ms.
- No `aria-*` overuse; semantic HTML preferred.

### 14.11 Best Practices
- Test with NVDA on Windows monthly.
- Maintain a documented accessibility statement.
- Train staff on keyboard shortcuts (see §15).

### 14.12 Implementation Notes
- shadcn/ui primitives are ARIA-compliant out of the box.
- The `:focus-visible` CSS rule uses `--shadow-glow` for the focus ring.
- `prefers-reduced-motion` is honored globally in `index.css`.

### 14.13 Future Opportunities
- High-contrast theme.
- Screen reader tour on first login.
- Patient-facing portal WCAG AAA target.

---

## 15. Keyboard Productivity

### 15.1 Objective
Enable experienced clinicians and clerks to operate VitalFlow HMS without touching the mouse. Every frequent action has a shortcut; every shortcut is discoverable.

### 15.2 Design Principles
- **Consistent modifiers:** `Ctrl` for app-level, `Alt` for in-page navigation, `Shift` for destructive variants.
- **No conflicts** with Windows OS shortcuts.
- **Discoverable:** shortcuts shown in tooltips and a `?` cheat-sheet dialog.
- **Forgiving:** `Esc` always cancels.

### 15.3 Shortcut Catalogue
| Shortcut | Action | Context |
|---|---|---|
| `Ctrl+K` | Open global command palette | Global |
| `Ctrl+/` | Show shortcut cheat-sheet | Global |
| `Ctrl+B` | Toggle sidebar collapse | Global |
| `Ctrl+R` | Refresh current page | Global |
| `Ctrl+1` | Go to Dashboard | Global |
| `Ctrl+2` | Go to Appointments | Global |
| `Ctrl+3` | Go to Patients | Global |
| `Ctrl+4` | Go to Queue | Global |
| `Ctrl+5` | Go to IPD | Global |
| `Ctrl+6` | Go to Laboratory | Global |
| `Ctrl+7` | Go to Billing | Global |
| `Ctrl+N` | New patient (receptionist/doctor) | Global |
| `Ctrl+Shift+N` | New appointment | Global |
| `Ctrl+S` | Save current form | Form |
| `Ctrl+Enter` | Save and close | Form |
| `Esc` | Close topmost dialog / cancel | Global |
| `Alt+1..9` | Switch tabs within page | Page |
| `?` | Open shortcut cheat-sheet | Global |
| `F2` | Edit selected row | Table |
| `Delete` | Delete selected row (with confirm) | Table |
| `Space` | Toggle checkbox/switch | Form |

### 15.4 UX Considerations
- **Muscle memory:** shortcuts never remap between versions.
- **Conflict avoidance:** `Ctrl+W` (close window) and `Ctrl+Shift+Q` (quit) reserved for OS.
- **Visual cues:** shortcut hints appear in menu items (right-aligned muted text).

### 15.5 Desktop Layout — Cheat-sheet Dialog
```
┌──────────────────────────────────────────────────────────────────────────┐
│ Keyboard shortcuts                                            Esc ✕       │
├──────────────────────────────────────────────────────────────────────────┤
│ Navigation                                                               │
│  Ctrl+1..7       Jump to module                                          │
│  Ctrl+K          Command palette                                         │
│  Ctrl+B          Toggle sidebar                                          │
│                                                                          │
│ Actions                                                                  │
│  Ctrl+N          New patient                                             │
│  Ctrl+Shift+N    New appointment                                         │
│  Ctrl+S          Save form                                               │
│  Ctrl+Enter      Save and close                                          │
│                                                                          │
│ Tables                                                                   │
│  ↑↓              Navigate rows                                           │
│  F2              Edit row                                                │
│  Delete          Delete row (confirm)                                    │
└──────────────────────────────────────────────────────────────────────────┘
```

### 15.6 Component Specs
- `useHotkey(key, handler, deps)` hook **[PLANNED]** — single registration point.
- `<CheatSheet />` dialog with grouped shortcut list.
- Shortcut hints in tooltips via `Tooltip` component.

### 15.7 Interaction Patterns
- Hotkey listener attached at `App` root; checks for input-focus context (e.g. `Ctrl+S` only fires in forms).
- `preventDefault` to stop browser default for reserved combos.

### 15.8 Accessibility
- Shortcuts never replace mouse/touch — they augment.
- `Ctrl+/` and `?` open the cheat-sheet so users can discover shortcuts.
- Single-character shortcuts (`?`) require no modifier but are disabled when an input is focused.

### 15.9 Performance
- Hotkey listener is a single `keydown` handler; routing via lookup map.
- No per-key debounce needed.

### 15.10 Best Practices
- Document shortcuts in user onboarding.
- Avoid more than 2-key combos for primary actions.
- Print a laminated cheat-sheet for the front desk.

### 15.11 Implementation Notes
- Phase 1 has no global hotkey system (only the Header search and form-submit-on-Enter).
- Phase 2 will introduce `useHotkey` and a centralized registry.

### 15.12 Future Opportunities
- User-customizable shortcuts.
- Macro recording for repetitive workflows.
- Voice command overlay ("Computer, new patient").

---

## 16. Notifications & Alerts

### 16.1 Objective
Surface time-sensitive clinical and operational events to the right user, at the right time, in the right channel — without alert fatigue.

### 16.2 Design Principles
- **Clinical priority first:** critical lab values interrupt; routine updates queue.
- **Channel appropriate:** toast for transient, notification center for digest, WhatsApp/SMS for off-platform.
- **Actionable:** every notification links to the source entity.
- **Reversible:** dismissal is undoable within 5 s.

### 16.3 Functional Requirements
- FR-NA-0001 Toasts shall appear top-right with `richColors closeButton`.
- FR-NA-0002 Critical alerts (lab critical value, ED deterioration) shall persist until dismissed.
- FR-NA-0003 The notification bell shall show a 2 px red dot for unread.
- FR-NA-0004 The notification center (drawer) shall group by Today / Yesterday / Earlier **[PLANNED]**.
- FR-NA-0005 WhatsApp/SMS notifications shall be sent for appointment reminders, lab-result-ready, and bill-due **[PLANNED]**.
- FR-NA-0006 Every notification shall link to the source entity via deep-link.

### 16.4 Notification Taxonomy
| Severity | Examples | Channel | Persistence |
|---|---|---|---|
| Critical | Lab critical value, code blue, ED deterioration | Toast + sound + WhatsApp | Until dismissed |
| High | Bed request, pharmacy low-stock, bill void | Toast | 30 min |
| Medium | Appointment reminder, lab result ready, new message | Toast + center | 1 day |
| Low | Daily report ready, audit summary | Center only | 7 days |

### 16.5 UX Considerations
- **Do-not-disturb:** night-shift mode silences non-critical toasts between 22:00–06:00 **[PLANNED]**.
- **Grouping:** identical notifications collapse ("3 new lab results").
- **Sound:** distinct tones for critical vs routine; user-configurable volume **[PLANNED]**.

### 16.6 Desktop Layout — Toast
```
                                                                ┌─────────────┐
                                                                │ ⚠ Critical  │
                                                                │ Aisha Khan  │
                                                                │ K+ 6.2 mmol/L│
                                                                │ [Open] [×]  │
                                                                └─────────────┘
```

### 16.7 Desktop Layout — Notification Center
```
┌──────────────────────────────────────────────────────────────────────────┐
│ Notifications                                          [Mark all read]    │
├──────────────────────────────────────────────────────────────────────────┤
│ Today                                                                    │
│  ⚠ K+ critical — Aisha Khan                                  2 min ago   │
│  💬 New message from Dr. Ahmed                               18 min ago  │
│  🧪 Lab result ready — Bikram Singh                          1 hr ago    │
│ Yesterday                                                                │
│  🛏 Bed A-02 cleaned                                          Yesterday   │
│  💵 Invoice #00123 paid                                       Yesterday   │
└──────────────────────────────────────────────────────────────────────────┘
```

### 16.8 Component Specs
- Toasts: Sonner `<Toaster position="top-right" richColors closeButton />`.
- Notification bell: Header `<button>` with absolute-positioned red dot.
- Notification center: right-side drawer (`Sheet` component) **[PLANNED]**.
- WhatsApp integration: existing `src-tauri/src/whatsapp/` module.

### 16.9 Interaction Patterns
- Toast click → navigate to source entity.
- Toast `×` → dismiss (with 5 s undo).
- Bell click → open notification center drawer.
- Center row click → navigate + auto-mark-read.

### 16.10 Accessibility
- Toast region `aria-live="assertive"` for critical, `aria-live="polite"` for routine.
- Bell button has `aria-label="Notifications (3 unread)"`.
- Critical toasts include a sound cue (respecting OS mute).

### 16.11 Performance
- Notifications polled every 30 s when app focused **[PLANNED]**.
- Phase 1: toasts are fire-and-forget; no persistence.
- Phase 2: `notifications` table with `read_at` and `dismissed_at`.

### 16.12 Best Practices
- Never auto-dismiss critical alerts.
- Cap concurrent toasts at 3; older collapse.
- Provide a clear "mute" toggle per category.

### 16.13 Implementation Notes
- Sonner is already wired in `App.tsx`.
- WhatsApp templates defined in `src-tauri/src/whatsapp/templates.rs`.
- Scheduler runs in Rust (`scheduler.rs`) for queued dispatch.

### 16.14 Future Opportunities
- Push notifications to mobile companion app.
- Smart routing based on on-call schedule.
- Alert-fatigue analytics (per-user dismiss rate).

---

## 17. AI-Assisted Features (with Human Review)

### 17.1 Objective
Augment — never replace — clinical and administrative decision-making with AI features that accelerate routine work while preserving human accountability. Every AI output is a suggestion; every acceptance is logged.

### 17.2 Design Principles
- **Human-in-the-loop:** AI proposes, human disposes. No autonomous clinical decisions.
- **Explainable:** every AI suggestion shows its source and confidence.
- **Auditable:** AI usage logged in `audit_logs` with `ai_assist` flag.
- **Opt-in:** features disabled by default; admin enables per role.
- **Privacy-preserving:** no PHI leaves the hospital network without consent.

### 17.3 Functional Requirements
- FR-AI-0001 AI features shall be marked `[AI]` in the UI.
- FR-AI-0002 Every AI suggestion shall display a confidence score.
- FR-AI-0003 Acceptance of an AI suggestion shall be logged with `audit::record(details={source: "ai", model: "...", suggestion_id: "..."})`.
- FR-AI-0004 AI features shall be configurable per role in Settings.
- FR-AI-0005 All AI features shall be **[PLANNED]** — none ship in Phase 1.

### 17.4 Feature Catalogue
| Feature | Use case | Trigger | Output | Human action |
|---|---|---|---|---|
| AI Diagnosis assist | Suggest ICD-10 from chief complaint + history | Encounter editor | Top-3 ranked diagnoses | Doctor selects / overrides |
| AI Drug interaction check | Warn on prescription | Prescription entry | Interaction list + severity | Doctor overrides with reason |
| AI Lab interpretation | Narrative for abnormal panel | Lab result view | Plain-language summary | Doctor reviews + signs |
| AI Triage score | NEWS2 / qSOFA calculation | ED triage | Score + recommended disposition | Nurse confirms |
| AI Scribe | Voice → encounter note | Encounter editor | Draft text | Doctor edits + signs |
| AI Billing coder | Suggest CPT/HCPCS from encounter | Bill creation | Code suggestions | Coder accepts |
| AI No-show prediction | Flag high-risk appointments | Appointment list | Risk score | Receptionist confirms + reminder |
| AI Inventory forecast | Predict stock-out | Inventory dashboard | Reorder suggestion | Inventory mgr approves PO |
| AI Chatbot (admin) | Natural-language query for admin | Command palette | Answer + source | User verifies |

### 17.5 Safety Rails
| Risk | Mitigation |
|---|---|
| Hallucinated diagnosis | Top-3 with confidence; mandatory human sign-off |
| PHI leakage to external LLM | On-prem model (Llama 3 / Mistral) preferred; if external, redact PHI |
| Bias in triage | Quarterly audit of predictions vs outcomes |
| Over-reliance | UI clearly marks suggestions as draft; acceptance requires explicit click |
| Audit gap | Every AI interaction logged; model version captured |

### 17.6 UX Considerations
- **Visual distinction:** AI suggestions use a subtle `accent` (teal) outline + `[AI]` chip.
- **Friction by design:** accepting a suggestion requires a click (no auto-accept).
- **Confidence display:** progress bar + label (High/Medium/Low).
- **Rejection easy:** one-click "Dismiss" with optional feedback.

### 17.7 Desktop Layout — AI Suggestion Card
```
┌──────────────────────────────────────────────────────────────────────────┐
│ [AI] Suggested diagnoses                              Confidence: High    │
│ Based on chief complaint "fever, cough 3 days" + history                  │
│                                                                          │
│ 1. J11.1 — Influenza with respiratory symptoms        ████████░░ 82%    │
│ 2. J20.9 — Acute bronchitis                           ██████░░░░ 64%    │
│ 3. J06.9 — Acute upper respiratory infection          █████░░░░░ 51%    │
│                                                                          │
│ [Accept selected]  [Dismiss]  [Why these suggestions?]                   │
└──────────────────────────────────────────────────────────────────────────┘
```

### 17.8 Component Specs
- `AiSuggestionCard` **[PLANNED]**: title, basis, ranked list, accept/dismiss.
- `AiChip` **[PLANNED]**: small `[AI]` badge for inline marking.
- `AiSettingsPanel` **[PLANNED]**: per-role toggles in Settings.

### 17.9 Interaction Patterns
- Suggestion appears as a side panel; never blocks the form.
- Accept populates the field; the field shows `[AI-assisted]` caption.
- Dismiss logs the rejection (with reason if provided) for model improvement.

### 17.10 Accessibility
- AI cards use `role="region"` with `aria-label="AI suggestion"`.
- Confidence bar has a text alternative (`aria-valuenow`).
- Accept/Dismiss buttons have descriptive labels.

### 17.11 Performance
- On-prem model inference: < 2 s for diagnosis suggestion.
- External API calls: stream response to avoid blocking UI.
- Cache: identical inputs within 5 min reuse cached response.

### 17.12 Best Practices
- Always show the model name and version.
- Provide a "Why this suggestion?" explainer.
- Quarterly audit of AI acceptance/rejection rates per clinician.

### 17.13 Implementation Notes
- All AI features are Phase 2+; no AI code ships in Phase 1.
- Backend abstraction: `AiProvider` trait with `LocalLlama` and `RemoteOpenAI` implementations **[PLANNED]**.
- Audit log: `details: { ai_assist: true, model: "llama-3-8b", suggestion_id: "uuid", accepted: true }`.

### 17.14 Future Opportunities
- Federated learning across group hospitals.
- Specialized models per specialty (radiology, pathology).
- Continuous-learning feedback loop with clinician review.

---

## 18. Performance & Responsiveness

### 18.1 Objective
Ensure the application feels instantaneous on the minimum-spec hardware (Intel i3, 8 GB RAM, 1366×768, HDD) common in smaller hospitals, while scaling gracefully to high-end clinical workstations.

### 18.2 Design Principles
- **Perceived performance > raw speed:** skeletons, optimistic updates, instant feedback.
- **Data efficiency:** minimal payloads, GraphQL-style field selection **[PLANNED]**.
- **Render budget:** 60 fps on the bed board; 30 fps acceptable elsewhere.
- **Cache-first:** React Query cache serves stale-while-revalidate.

### 18.3 Functional Requirements
- FR-PF-0001 First meaningful paint shall be < 1.5 s on minimum spec.
- FR-PF-0002 Every list page shall render < 200 ms after data arrives.
- FR-PF-0003 Every mutation shall show feedback within 100 ms (optimistic UI preferred).
- FR-PF-0004 The bed board shall render < 100 ms for 500 beds.
- FR-PF-0005 Database queries shall return < 300 ms p95.

### 18.4 Performance Budget
| Layer | Budget | Measurement |
|---|---|---|
| Initial JS payload (gzipped) | < 350 KB | Tauri bundle size |
| Initial CSS (gzipped) | < 50 KB | Tailwind purge |
| Route transition | < 200 ms | Motion duration |
| List render (1k rows) | < 100 ms | React render |
| Backend command p95 | < 300 ms | sqlx timing |
| Bed board render | < 100 ms | 500 beds |
| Toast display latency | < 50 ms | Sonner |
| Search keystroke → results | < 200 ms | debounced 150 ms + query |

### 18.5 UX Considerations
- **Skeletons:** show `.skeleton` shimmer for any load > 200 ms.
- **Optimistic updates:** queue call-next, mark bed cleaning, post payment.
- **Stale indicator:** subtle dot on stale data ("cached 4 min ago").
- **Offline mode:** React Query continues serving cache when LAN drops; banner indicates offline.

### 18.6 Desktop Layout
Performance is layout-agnostic but density matters: at 1366×768 with the sidebar expanded (264 px), the workspace is 1102 px wide — enough for a 4-column KPI grid only if tiles are 250 px wide. Default collapse on this resolution is recommended.

### 18.7 Component Specs
- `useDebouncedCallback(value, delay)` for search inputs.
- `<Skeleton variant="card|row|text" />` primitives.
- `<StaleIndicator />` for cached data older than threshold.

### 18.8 Interaction Patterns
- Click → immediate visual response (button active state) → optimistic UI → mutation → revert on error.
- Long-running operations: progress bar + cancel button.
- List virtualization (`react-window`) for > 500 rows **[PLANNED]**.

### 18.9 Accessibility
- Loading states announced via `aria-busy="true"`.
- Progress bars have `role="progressbar"` + `aria-valuenow`.
- Skeletons have `aria-hidden="true"`.

### 18.10 Performance
- React Query: `staleTime: 5min`, `refetchOnWindowFocus: true`, `retry: 1`.
- Motion: GPU-accelerated transforms only (`opacity`, `transform`).
- Images: SVG icons (no raster); Inter variable font (single file).
- Database: indexed FKs on all hot paths (see `db.rs` migration indices).

### 18.11 Best Practices
- Measure with React DevTools Profiler weekly.
- Track INP (Interaction to Next Paint) < 200 ms.
- Audit bundle size every release.

### 18.12 Implementation Notes
- Vite build with `manualChunks` for vendor splitting **[PLANNED]**.
- React 19 concurrent features (`useTransition`) for list filtering **[PLANNED]**.
- Tauri webview: hardware-accelerated on Windows; verify GPU drivers on clinical PCs.

### 18.13 Future Opportunities
- Service worker for offline-first operation **[contingent on Tauri support]**.
- WebSocket push for real-time updates (no polling).
- Edge cache for read-heavy reports.

---

## 19. Error Prevention & Auditability

### 19.1 Objective
Prevent user errors before they happen, recover gracefully when they do, and maintain an immutable audit trail of every clinical and financial action for regulatory compliance (ISO 27001 A.8.15, HIPAA Accounting of Disclosures).

### 19.2 Design Principles
- **Prevent over recover:** constrained inputs, default-safe options, confirm-on-destructive.
- **Reversible when possible:** soft-delete, undo last action, void (not delete) bills.
- **Transparent errors:** never silent failures; user-facing error always includes a path to resolution.
- **Auditable by design:** every write to a clinical/financial table calls `audit::record`.

### 19.3 Functional Requirements
- FR-EP-0001 Every destructive action shall require a confirm dialog with the entity name.
- FR-EP-0002 Patient delete shall be blocked if encounters exist (FK RESTRICT).
- FR-EP-0003 Bill void shall require `billing.manage` + reason.
- FR-EP-0004 Every protected command shall call `audit::record` after success.
- FR-EP-0005 Audit log shall be immutable: no UPDATE/DELETE on `audit_logs` (enforced via DB role grants).
- FR-EP-0006 The Audit page (`/audit`) shall require `audit.view` and support filters by user, action, entity, date range.
- FR-EP-0007 Error toasts shall include `errorId` for support correlation.

### 19.4 Error Prevention Patterns
| Pattern | Implementation | Example |
|---|---|---|
| Constrained input | `<Select>` for status, `<Calendar>` for dates | Appointment status dropdown |
| Default-safe | Sensible defaults (duration 30 min, status scheduled) | New appointment form |
| Confirm-on-destructive | `AlertDialog` with entity name | "Delete patient Aisha Khan?" |
| Block on conflict | FK RESTRICT, server-side check | Cannot delete patient with encounters |
| Undo window | 5 s undo toast | Bed cleaning flip |
| Validation feedback | Inline error per field | Required-field red border |
| Server validation | `rbac::require` + business rules | Admit to occupied bed rejected |
| Audit trail | `audit::record` on every write | All CRUD operations |

### 19.5 Audit Trail Schema
```sql
audit_logs (
  id BIGSERIAL PRIMARY KEY,
  user_id INT REFERENCES users(id),
  username TEXT,
  action TEXT,          -- "patient.create", "bill.void", etc.
  entity_type TEXT,     -- "patient", "bill", "encounter"
  entity_id BIGINT,
  details JSONB,        -- before/after diff, reason, ip
  severity TEXT,        -- info | warning | critical
  created_at TIMESTAMPTZ DEFAULT now()
)
```

### 19.6 UX Considerations
- **Confirm dialogs:** use the entity name so users see exactly what they're acting on.
- **Inline validation:** show error below the field immediately, not on submit.
- **Error messages:** plain language + suggested next step ("Bed A-02 is occupied. Choose another bed or discharge the current patient.").
- **Audit visibility:** the Audit page is searchable, filterable, and exportable.

### 19.7 Desktop Layout — Confirm Dialog
```
┌──────────────────────────────────────────────────────────────────────────┐
│ ⚠ Delete patient                                              Esc ✕       │
├──────────────────────────────────────────────────────────────────────────┤
│ You are about to delete the record for                                    │
│                                                                          │
│   Aisha Khan (MRN-0001234)                                               │
│                                                                          │
│ This action cannot be undone. All appointments for this patient will be  │
│ deleted. Encounters, lab orders and bills will be retained for audit.    │
│                                                                          │
│ Type the patient's name to confirm:                                      │
│ [____________________________________________]                            │
│                                                                          │
│                                          [Cancel]   [Delete]             │
└──────────────────────────────────────────────────────────────────────────┘
```

### 19.8 Component Specs
- `ConfirmDialog` **[PLANNED]**: severity icon, title, body, optional "type to confirm" field.
- Inline error component: `.text-destructive text-xs mt-1`.
- Error toast: Sonner error variant with `errorId` in description.

### 19.9 Interaction Patterns
- Destructive button: `destructive` variant; disabled until confirmation text matches.
- Error toast: red background, `errorId` visible for support.
- Audit page: filter → table; click row → expand details JSON.

### 19.10 Accessibility
- Confirm dialog: `role="alertdialog"` + `aria-describedby`.
- Inline errors: `aria-describedby` linking field to error text.
- Audit table: sortable headers announce sort direction.

### 19.11 Performance
- Audit writes are async (`tokio::spawn`); do not block the user response.
- Audit page paginated 100 rows; deep filtering server-side.
- Quarterly archival of audit logs older than 7 years (regulatory retention).

### 19.12 Best Practices
- Never expose stack traces to end users; log server-side, show `errorId`.
- Review audit anomalies weekly (failed permission attempts, void spikes).
- Pen-test the audit trail integrity annually.

### 19.13 Implementation Notes
- `audit::record` is called by every `commands/*.rs` write function.
- DB role `hms_app` has `INSERT` on `audit_logs` but no `UPDATE`/`DELETE`.
- Audit page uses `useAuditLogs(filters)` React Query hook.

### 19.14 Future Opportunities
- Tamper-evident audit (Merkle-tree hash chain).
- Real-time anomaly detection on audit stream.
- Patient-facing audit disclosure portal (HIPAA Accounting of Disclosures).

---

## 20. Future Readiness

### 20.1 Objective
Ensure the design system, architecture and UX patterns established in Phase 1 accommodate Phase 2 modules (Pharmacy, Radiology, HR, Payroll, Reports, ED, ICU, OT, Procurement) and beyond (FHIR interoperability, mobile companion, multi-hospital group) without rework.

### 20.2 Design Principles
- **Token-driven extensibility:** new domains reuse the existing token palette; no per-domain color sprawl.
- **Component reuse:** new modules compose existing primitives (Card, Table, Dialog, Form).
- **Permission-forward:** new permissions append to the `Permission` enum; existing grants unaffected.
- **API-stable:** Tauri command signatures follow `verb_entity` convention; breaking changes require a versioned `v2_` prefix.

### 20.3 Phase 2 Module Roadmap
| Module | UX dependencies | New permissions | New roles |
|---|---|---|---|
| Pharmacy | Inventory §11, Dispense flow | `pharmacy.dispense`, `pharmacy.manage` | (existing `pharmacist`) |
| Radiology | DICOM viewer, worklist | `radiology.order`, `radiology.report.manage` | `radiology_tech` |
| ED | Triage form, live bed grid | `ed.triage`, `ed.admit` | (existing roles) |
| ICU | Vitals trend, vent settings | `icu.view`, `icu.chart` | (existing `doctor, nurse`) |
| OT | Surgical checklist, scheduling | `ot.schedule`, `ot.checklist` | `ot_staff` |
| HR | Staff master, attendance | `hr.view`, `hr.manage` | `hr_manager` |
| Payroll | Salary structure, payslip | `payroll.view`, `payroll.process` | `hr_manager` |
| Procurement | PO, GRN, suppliers | `procurement.manage` | `inventory_manager` |
| Reports | Chart library, export | (existing `reports.view`) | (existing roles) |
| Patient portal | Web/mobile surface | `portal.view` (patient role) | (existing `patient`) |

### 20.4 Interoperability Readiness
| Standard | Phase | Use case |
|---|---|---|
| FHIR R4 (REST) | Phase 3 | Exchange patient summaries with other hospitals |
| HL7 v2 (MLLP) | Phase 3 | Lab instrument integration |
| DICOM (DIMSE) | Phase 2 | Radiology image exchange |
| SNOMED CT | Phase 2 | Structured diagnosis coding |
| ICD-10/11 | Phase 2 | Diagnosis coding (already used in encounters) |
| LOINC | Phase 2 | Lab test catalog coding |
| RxNorm | Phase 2 | Drug catalogue coding |
| ASTM E1381 | Phase 2 | Lab analyzer middleware |

### 20.5 Multi-Hospital Group Readiness
- **Tenant isolation:** Phase 1 is single-hospital; Phase 3 introduces `hospital_id` column on every table.
- **Group dashboard:** aggregated KPIs across hospitals (with per-hospital drill-down).
- **License scope:** group license file with N hospital fingerprints.

### 20.6 Mobile Companion App
- **Target:** React Native (Tauri Mobile is not yet production-ready for iOS/Android).
- **Scope:** patient-facing — view appointments, lab results, pay bills, message care team.
- **Auth:** OAuth2 device flow against the desktop HMS server.

### 20.7 UX Considerations
- **Onboarding:** new modules appear as disabled sidebar items ("Phase 2" chip) so users anticipate them.
- **Settings:** per-module enable/disable toggles for hospitals that don't need every feature.
- **Discovery:** the command palette surfaces new modules as they ship.

### 20.8 Desktop Layout
Future modules will follow the established 3-zone layout (sidebar / header / workspace). New departments may add a contextual right-side panel (see §3.5.4) for high-density workflows.

### 20.9 Component Specs
- Every new module ships with: list page, form dialog, detail view, audit hooks.
- New primitives are added to `src/components/ui/` only after design review.

### 20.10 Interaction Patterns
- Cross-module navigation via deep links (`/patients/:id/encounters/:eid`).
- Drag-and-drop for resource allocation (bed → bed, doctor → appointment) **[PLANNED]**.
- Bulk actions on every list page (multi-select + bulk action bar) **[PLANNED]**.

### 20.11 Accessibility
- Every new module must pass the WCAG 2.2 checklist (§14.4) before release.
-axe-core automated scan in CI **[PLANNED]**.

### 20.12 Performance
- Each new module adds < 30 KB gzipped to the bundle (lazy-loaded route).
- Database migrations are forward-only; rollback migrations provided for emergency.

### 20.13 Best Practices
- Maintain a design-system changelog; deprecate tokens with one release notice.
- Quarterly design review to retire unused patterns.
- Annual accessibility audit.

### 20.14 Implementation Notes
- The `Permission` enum is `#[non_exhaustive]` ready **[PLANNED]** so new variants don't break match arms.
- The `NavItem` type supports `disabled` and `note` for future-module placeholders (already used for Reports).
- Tailwind 4 `@theme inline` allows runtime token extension for white-label deployments **[PLANNED]**.

### 20.15 Future Opportunities
- Plugin marketplace (third-party modules).
- White-label theming per hospital group.
- AI co-pilot integrated into every screen (see §17).
- Voice-first clinical documentation.
- Augmented reality for bed/ward visualization.
- Federated learning across group hospitals for clinical AI.

---

## Appendix A — Glossary

| Term | Definition |
|---|---|
| **EHR** | Electronic Health Record — longitudinal patient chart |
| **MRN** | Medical Record Number — unique patient identifier |
| **IPD** | In-Patient Department — admitted patients |
| **OPD** | Out-Patient Department — walk-in visits |
| **ED** | Emergency Department |
| **ICU** | Intensive Care Unit |
| **OT** | Operation Theatre |
| **TAT** | Turnaround Time (lab) |
| **Triage** | Severity-based patient prioritization |
| **FEFO** | First-Expiry-First-Out (inventory) |
| **FIFO** | First-In-First-Out |
| **WCAG** | Web Content Accessibility Guidelines |
| **FHIR** | Fast Healthcare Interoperability Resources |
| **HL7** | Health Level Seven (clinical data exchange) |
| **DICOM** | Digital Imaging and Communications in Medicine |
| **SNOMED CT** | Systematized Nomenclature of Medicine — Clinical Terms |
| **LOINC** | Logical Observation Identifiers Names and Codes |
| **RxNorm** | Normalized naming for clinical drugs |
| **ICD-10/11** | International Classification of Disease |
| **CPT** | Current Procedural Terminology |
| **HCPCS** | Healthcare Common Procedure Coding System |
| **PHI** | Protected Health Information |
| **RBAC** | Role-Based Access Control |
| **NEWS2** | National Early Warning Score (UK) |
| **qSOFA** | Quick Sequential Organ Failure Assessment |
| **GCS** | Glasgow Coma Scale |
| **Tauri** | Cross-platform desktop framework (Rust + WebView) |
| **shadcn/ui** | React component library (Radix + Tailwind) |
| **Motion** | Animation library (formerly Framer Motion) |
| **Lucide** | Open-source icon set |

---

## Appendix B — Token Quick Reference

### B.1 Color Tokens (light)
```css
--background: 210 40% 98%;     --foreground: 222 47% 11%;
--card: 0 0% 100%;              --card-foreground: 222 47% 11%;
--primary: 199 89% 48%;         --primary-foreground: 0 0% 100%;
--primary-hover: 199 89% 40%;   --primary-soft: 199 89% 94%;
--accent: 172 76% 36%;          --accent-soft: 172 76% 94%;
--secondary: 210 40% 96%;       --muted-foreground: 215 16% 47%;
--destructive: 0 72% 51%;       --success: 142 71% 45%;
--warning: 38 92% 50%;          --info: 199 89% 48%;
--border: 214 32% 91%;          --ring: 199 89% 48%;
--status-scheduled: 199 89% 48%;
--status-confirmed: 142 71% 45%;
--status-completed: 215 16% 47%;
--status-cancelled: 0 72% 51%;
--status-no-show: 38 92% 50%;
```

### B.2 Radius Tokens
```css
--radius-sm: 6px;
--radius: 10px;       /* default */
--radius-md: 12px;
--radius-lg: 16px;
--radius-xl: 20px;
```

### B.3 Shadow Tokens (light)
```css
--shadow-xs: 0 1px 2px 0 rgb(15 23 42 / 0.04);
--shadow-sm: 0 1px 3px 0 rgb(15 23 42 / 0.06), 0 1px 2px -1px rgb(15 23 42 / 0.04);
--shadow-md: 0 4px 6px -1px rgb(15 23 42 / 0.07), 0 2px 4px -2px rgb(15 23 42 / 0.05);
--shadow-lg: 0 10px 15px -3px rgb(15 23 42 / 0.08), 0 4px 6px -4px rgb(15 23 42 / 0.05);
--shadow-xl: 0 20px 25px -5px rgb(15 23 42 / 0.10), 0 8px 10px -6px rgb(15 23 42 / 0.05);
--shadow-glow: 0 0 0 3px rgb(14 165 233 / 0.12);
```

### B.4 Layout Tokens
```css
--sidebar-width: 264px;
--sidebar-width-collapsed: 72px;
--header-height: 64px;
```

### B.5 Typography Utilities
```css
.text-display-xl { font-weight: 800; font-size: 2.25rem;   line-height: 2.5rem;   letter-spacing: -0.025em; }
.text-display-lg { font-weight: 700; font-size: 1.75rem;   line-height: 2.125rem; letter-spacing: -0.02em;  }
.text-display-md { font-weight: 600; font-size: 1.25rem;   line-height: 1.75rem;  letter-spacing: -0.015em; }
.text-display-sm { font-weight: 600; font-size: 1rem;      line-height: 1.5rem;   letter-spacing: -0.01em;  }
```

---

## Appendix C — Cross-Reference Matrix

| Section | SRS FRs | ISO 25010 | ISO 27001 | Source files |
|---|---|---|---|---|
| §1 Philosophy | FR-UX-0001..5 | Usability, Operability | A.5.15 | `App.tsx`, `AppShell.tsx` |
| §2 Design System | FR-DS-0001..4 | Maintainability, UI Quality | — | `src/index.css`, `tailwind.config.ts` |
| §3 App Layout | FR-AL-0001..6 | Operability | A.5.15 | `Sidebar.tsx`, `Header.tsx`, `AppShell.tsx` |
| §4 Dashboards | FR-RD-0001..5 | Functional Suitability | A.5.15 | `Dashboard.tsx`, `queries.ts` |
| §5 Patient 360 | FR-PM-0001..6 | Functional Suitability, Privacy | A.5.12, A.5.34 | `Patients.tsx`, `PatientForm.tsx`, `commands/patients.rs` |
| §6 Appt/Queue | FR-AQ-0001..5 | Functional Suitability | A.5.15 | `Appointments.tsx`, `Queue.tsx`, `commands/appointments.rs`, `commands/queue.rs` |
| §7 Clinical | FR-CW-0001..8 | Functional Suitability, Safety | A.5.34 | `Encounters` (planned), `commands/encounters.rs`, `commands/lab.rs`, `commands/ipd.rs` |
| §8 Departments | per-dept | Functional Suitability | A.5.15 | All `pages/*.tsx`, `commands/*.rs` |
| §9 Bed Mgmt | FR-BM-0001..5 | Functional Suitability, Safety | A.5.15 | `IPD.tsx`, `commands/ipd.rs` |
| §10 Billing | FR-BL-0001..6 | Functional Suitability, Accuracy | A.5.15, A.5.34 | `Billing.tsx`, `Receipt.tsx`, `commands/billing.rs` |
| §11 Inventory | FR-IP-0001..6 | Functional Suitability | A.5.15 | `commands/lab.rs` (inventory.items table) |
| §12 Analytics | FR-AR-0001..5 | Functional Suitability, Maintainability | A.8.15 | `Dashboard.tsx` (Phase 1), `commands/dashboard.rs` |
| §13 Global Search | FR-GS-0001..5 | Operability | A.5.15 | `command.tsx` (planned wrapper) |
| §14 Accessibility | FR-AC-0001..8 | Usability (Accessibility) | A.5.30 | `src/index.css` (reduced-motion), shadcn primitives |
| §15 Keyboard | (shortcuts) | Operability | — | `useHotkey` (planned) |
| §16 Notifications | FR-NA-0001..6 | Operability, Reliability | A.8.15 | `App.tsx` (Sonner), `whatsapp/*` |
| §17 AI | FR-AI-0001..5 | Functional Suitability, Safety | A.5.15, A.5.30 | All Phase 2+ |
| §18 Performance | FR-PF-0001..5 | Performance Efficiency | — | React Query, `queries.ts`, `db.rs` (indices) |
| §19 Error/Audit | FR-EP-0001..7 | Reliability, Integrity | A.8.15, A.8.16 | `audit.rs`, all `commands/*.rs` |
| §20 Future | per-module | Maintainability, Compatibility | — | `rbac.rs` (Permission enum), `db.rs` (migrations) |

---

## Document Control

| Version | Date       | Author                     | Changes                                           |
|---------|------------|----------------------------|---------------------------------------------------|
| 0.1.0   | 2026-07-02 | Documentation Specialist   | Initial design notes (in DESIGN_SYSTEM.md)        |
| 0.2.0   | 2026-07-03 | Healthcare UX Architect    | Full 20-section UI/UX Design Specification (this) |
| 0.2.0   | 2025-07-08 | Documentation Team (B4-C)  | Reconciled with Phase 2 Batches 0-3: version banner added; §1.4 dynamic branding noted (DS-04); §2.2 Tailwind `@theme inline` token registration noted (CR-13) + re-skin confirmation (CR-18); §11.3 + §11.7 + §11.12 Inventory page marked Implemented (CR-21); §14.4 WCAG checklist honestly rewritten to reflect the Batch 3 a11y pass (A11Y-01/03/04 + INT-01); §14.7 SkipLink + VisuallyHidden updated; Document Control revision history extended. Select-field `htmlFor` + KPI `aria-label` + axe-core CI flagged for Batch 5. |

### Reviewers
- Principal Architect (review for technical accuracy)
- Frontend Lead (review for implementation feasibility)
- Clinical Safety Officer (review for clinical safety implications)
- ISO 27001 ISMS Lead (review for access-control alignment)

### Next review
- After Phase 2 module UX wireframes are drafted (target 2026-09-30).

---

**End of Document — VitalFlow HMS UI/UX Design Specification v0.2.0**
