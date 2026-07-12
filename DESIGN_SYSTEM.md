> **⚠️ SUPERSEDED — see `/docs/09-UI-UX-Design-Specification.md` (authoritative per RCTF policy).**
>
> This document described the previous "Mayo Clinic" design language (navy + PT Serif). The application has been re-skinned to the VitalFlow brand (sky-blue #0EA5E9 + teal #14B8A6 + Inter) per the UI/UX Design Specification. This file is retained for version history and will be removed or fully reconciled in a future documentation pass.
>
> Where this document conflicts with the UI/UX Design Specification or the
> live code (in particular `src/index.css` color tokens, `src/components/layout/*`,
> and `src/components/layout/shared.tsx`), the code + UI/UX spec win. The
> colour tables, typography, and component specs below should be treated as
> historical context only.

---

# VitalFlow HMS — Design System & Architecture Reference

This document is the implementation reference for the healthcare-SaaS
redesign: color system, typography, layout architecture, component
specs, and the technical decisions behind them. Everything described
here is already implemented in the codebase — this is documentation of
what exists, not a separate proposal.

---

## 1. Technical Stack (as implemented)

| Layer | Choice | Notes |
|---|---|---|
| UI framework | React 19 + TypeScript | unchanged |
| Styling | Tailwind CSS v4 | CSS-variable-driven theme, no separate config file (v4 uses `@theme inline` in `index.css`) |
| Routing | **React Router (`HashRouter`)** | See §2 — `HashRouter`, not `BrowserRouter`, deliberately |
| Data fetching | **TanStack React Query v5** | Replaces the old `useState`/`useEffect`/manual-refetch pattern everywhere |
| Animation | **Motion** (package name `motion`, imported as `motion/react`) | API-identical successor to Framer Motion — see §2 |
| Desktop shell | Tauri v2 | unchanged |
| Icons | lucide-react | unchanged |
| Charts | Recharts | added for the Dashboard's appointment-mix chart |

---

## 2. Two Deliberate Deviations From the Literal Brief

Both are correctness fixes for running inside Tauri, not stylistic
choices — flagging them explicitly so they're never "fixed" back to the
literal wording by mistake.

### 2.1 `HashRouter`, not `BrowserRouter`

`BrowserRouter` relies on the HTML5 History API and requires the server
to redirect unknown paths back to `index.html`. In Tauri's **production**
build, the frontend is served from a custom protocol with no such
fallback — `BrowserRouter` works fine in `tauri dev` but breaks on
reload or any deep link in a built `.exe`. `HashRouter` has no such
dependency and is the correct choice for a Tauri desktop app with no
visible URL bar.

### 2.2 `motion` package, not `framer-motion`

The library was rebranded; `motion` is the actively maintained package,
`framer-motion` is the legacy name. Same API, same import shape — every
usage in this codebase is `import { motion, AnimatePresence } from
"motion/react"`.

---

## 3. Color System

All colors are defined as raw HSL components in CSS custom properties
(`--primary: 199 89% 38%`), then composed with `hsl(var(--primary))` —
this is what makes the alpha-channel syntax `hsl(var(--primary) / 0.12)`
work throughout the codebase for tinted backgrounds.

### Light theme

| Token | HSL | Used for |
|---|---|---|
| `--background` | `210 30% 98%` | Page background — soft gray, not stark white |
| `--foreground` | `215 30% 14%` | Primary text |
| `--card` | `0 0% 100%` | Card/surface background |
| `--primary` | `199 89% 38%` | Medical blue — primary actions, active nav |
| `--primary-hover` | `199 89% 32%` | Hover state for primary actions |
| `--secondary` | `187 60% 94%` | Light blue accent surface |
| `--accent` | `178 65% 42%` | Teal — secondary brand accent |
| `--muted` | `210 25% 95%` | Subdued backgrounds, skeletons |
| `--muted-foreground` | `215 16% 46%` | Secondary text |
| `--destructive` | `0 72% 51%` | Delete/danger actions |
| `--border` | `214 22% 90%` | All borders |

### Dark theme (`.dark` class on `<html>`)

| Token | HSL | Notes |
|---|---|---|
| `--background` | `222 32% 8%` | Deep navy/slate, per brief |
| `--card` | `220 28% 11%` | Slightly elevated from background |
| `--primary` | `199 85% 56%` | Brightened for dark-surface contrast |
| `--accent` | `178 70% 48%` | Cyan/teal, brief's explicit dark-mode accent |
| `--border` | `220 22% 18%` | |

### Status colors (appointment lifecycle) — same in both themes

| Token | HSL | Status |
|---|---|---|
| `--status-scheduled` | `213 75% 56%` | Scheduled |
| `--status-confirmed` | `160 60% 42%` | Confirmed |
| `--status-completed` | `215 15% 55%` | Completed |
| `--status-cancelled` | `0 65% 55%` | Cancelled |
| `--status-no-show` | `35 85% 55%` | No-show |

Usage pattern (consistent across Dashboard/Appointments/Doctors):
```tsx
style={{ background: `hsl(${token} / 0.12)`, color: `hsl(${token})` }}
```

### Sidebar-specific tokens

A separate token set (`--sidebar-bg`, `--sidebar-fg`, `--sidebar-active-bg`,
etc.) exists because the sidebar's active/hover states needed finer
control than reusing `--primary`/`--accent` directly would have allowed.

---

## 4. Typography

| Face | Use | Loaded via |
|---|---|---|
| **Lexend** | Display/heading face — geometric, high legibility, designed for reading proficiency | Google Fonts `@import` in `index.css` |
| **Inter** | Body text, table data, form inputs | Same `@import`, already in use pre-redesign |

### Scale (utility classes, defined in `index.css`)

| Class | Size | Weight | Use |
|---|---|---|---|
| `.text-display-xl` | 2rem | 700 | Page-level numbers (KPI cards) |
| `.text-display-lg` | 1.5rem | 700 | Page titles |
| `.text-display-md` | 1.125rem | 600 | Section/card titles |
| `.text-display-sm` | 0.9375rem | 600 | Sidebar brand, chat header |

Body text uses Tailwind's standard `text-sm`/`text-xs`/etc. with Inter —
no separate scale needed since these were already well-tuned for the
dense tables this app is full of.

---

## 5. Spacing, Radius, Shadow

- **Radius**: `--radius: 0.75rem` as the base; `--radius-sm/md/lg/xl`
  derive from it. Buttons and pills use `rounded-full`. Cards/sections
  use the `.surface-card`/`.surface-elevated` utility classes (below).
- **Shadow**: Three tiers (`--shadow-sm/md/lg`), all soft and low-opacity
  per the brief's explicit "avoid excessive shadows" instruction. Dark
  mode uses higher-opacity black shadows since light-mode soft shadows
  are invisible on dark backgrounds.
- **Surface utilities**:
  ```css
  .surface-card     /* border + sm shadow — most cards/sections */
  .surface-elevated /* border + md shadow — modals, the boot screen */
  ```

---

## 6. Layout Architecture

```
main.tsx
  └── QueryClientProvider (React Query)
        └── HashRouter
              └── App.tsx
                    ├── boot sequence (setup check → init → ready/error)
                    └── AppShell (once ready)
                          ├── Sidebar (desktop, persistent) / mobile drawer
                          ├── Header
                          └── <Routes> (AnimatePresence-wrapped page transitions)
```

### Sidebar behavior

- **Desktop** (`lg:` breakpoint and up): persistent, collapsible.
  Collapsed state persists across sessions via `localStorage`
  (`hms-sidebar-collapsed`). Collapsed state shows icons only, with
  `title` attributes as a tooltip fallback (no separate tooltip library
  needed — native browser tooltips via `title` are sufficient here and
  add zero bundle weight).
- **Mobile/tablet** (below `lg:`): drawer, slides from left via Motion,
  with a `bg-black/50` backdrop. **Closes automatically on every
  navigation** (a `useEffect` on `location.pathname` inside `AppShell`)
  and **closes on backdrop click**. Drag-to-dismiss is wired via Motion's
  `drag="x"` + `onDragEnd` threshold check (swipe left >80px closes it) —
  this is the "swipe-friendly" requirement from the brief.
- **Active route styling**: `NavLink`'s `isActive` render-prop drives a
  Motion `layoutId="sidebar-active-pill"` background that animates
  smoothly between nav items on click, rather than just snapping.

### Header contents (left to right)

Hamburger (mobile only) → page title → refresh button → search
(expands on click) → clock → notifications bell → theme toggle → profile
dropdown.

**The server's LAN IP is deliberately NOT in the header** — see §9.

### Responsive breakpoints

Standard Tailwind defaults are used throughout (`sm`/`md`/`lg`/`xl`), no
custom breakpoints were introduced. The one architecturally significant
breakpoint is `lg:`, which is the sidebar-to-drawer switch point.

---

## 7. Data Layer — React Query Migration

### Why

The pre-redesign pattern was: each page held its own `useState` list,
fetched in `useEffect`, and manually called the fetch function again
after every create/update/delete. A parallel mechanism in `App.tsx`
(`refreshKey` forcing a full page remount via `key={...}`) existed
specifically to work around pages not re-fetching reliably otherwise.

### What changed

`src/lib/queries.ts` centralizes every backend command behind a
`useXxx()` hook:
- **Reads** are `useQuery` with a canonical query key (`src/lib/queries.ts`
  exports a `qk` object — always use it rather than writing query-key
  arrays inline, so cache invalidation stays correct).
- **Writes** are `useMutation`, and each one's `onSuccess` invalidates
  exactly the query keys it affects — e.g. `useCreateAppointment`
  invalidates `["appointments"]`, not the whole cache.
- Toasts moved from being copy-pasted into every page's `try`/`catch`
  into the centralized `onSuccess`/`onError` handlers — one source of
  truth for "what does the user see when X succeeds/fails."

### A real bug this caught

Several Rust commands were already being called with parameters that
don't exist on their actual signatures (e.g. the frontend called
`get_doctors` with `{ search, specialization }`, but the Rust command
only accepts `active_only: Option<bool>`). These were silent no-ops —
Tauri ignores unknown keys — and search/specialization filtering was
*already* happening entirely client-side. Centralizing the commands in
`queries.ts` surfaced this immediately (TypeScript + a side-by-side read
of the Rust source caught it) and the hooks now reflect what the backend
actually supports. **No backend behavior changed; the frontend was
already not relying on the dead parameters.**

### Centralized models

`src/lib/models.ts` is the single source of truth for every TypeScript
shape that mirrors a Rust struct. Previously, `Patient`/`Doctor`/
`AppointmentWithDetails` were redefined per-page and had already drifted
— one copy of `AppointmentWithDetails` was missing `created_at`/
`updated_at`, which caused a real runtime bug (the receipt feature
crashed reading a field that didn't exist on that page's local type).
**Always import models from `lib/models.ts`; never redefine them
per-page.**

---

## 8. Component Specs (per brief's requested modules)

### Dashboard
- KPI cards: scheduled / confirmed / completed (from `useAppointmentStats`)
  + a cancellations/no-shows card with its own status-tinted background.
- Today's schedule table, clickable rows → navigates to Appointments.
- Recharts donut chart of the appointment-status mix (only rendered once
  there's at least one appointment — no empty chart frames).
- Active clinic roster (patient/doctor counts) + a LAN-status card whose
  copy points to **Settings → Advanced**, not the header (§9).

### Patients / Doctors
- Search (client-side, matching pre-existing behavior — see §7's note on
  the backend not supporting server-side search for these two resources).
- Loading state: skeleton rows (`.skeleton` shimmer class), not a spinner
  — per the brief's explicit "loading skeletons" requirement.
- Empty state: icon + contextual message (different copy for "no results
  for this search" vs. "nothing registered yet") + a CTA button in the
  latter case only.
- Doctors adds a literal **availability indicator** beyond the existing
  active/inactive flag: a client-side check of whether the current time
  falls within the doctor's `available_from`/`available_to` window,
  shown as "Available now" / "Off duty" / "Inactive."

### Appointments
- Filters: search, doctor, status, date — all client-side (matches the
  real backend capability, see §7).
- Quick status-change buttons (Confirm/Complete/Cancel) call
  `useUpdateAppointmentStatus`, which invalidates all appointment queries.
- Receipt flow (built in an earlier session) is preserved exactly: booking
  a new appointment opens a print-ready receipt dialog sized for 80mm
  thermal paper via `@media print` CSS; a reprint button exists on every
  row.

### Messaging
- Sidebar conversations list (3 fixed channels: general/doctors/admin —
  matches the existing Postgres schema, which doesn't support
  user-created rooms).
- Message bubbles: own messages right-aligned with `--primary`
  background; others left-aligned with a card background + border.
  Consecutive messages from the same sender collapse the
  avatar/name/timestamp header (standard chat-app pattern).
- **Live updates**: a Tauri event listener (`new_message`) writes
  directly into the React Query cache via `queryClient.setQueryData`,
  giving instant updates without waiting for the next poll —
  `useMessages`' own `refetchInterval: 4000` exists only as a
  reliability fallback in case an event is ever missed.

### WhatsApp (inside Settings, not a separate route)
- Notification history: `useNotificationLog`, success/failure icon,
  type badge, truncated message preview, relative-ish timestamp.
- Send test modal equivalent: inline test-phone input + "Send test" /
  "Test group message" buttons, both via `useSendWhatsAppNotification`.
- The brief's "Delivery status indicators" maps to the existing
  success/failure boolean on each log row — there's no intermediate
  "delivered"/"read" state available, since this integration drives
  WhatsApp Web directly rather than the Business API, which doesn't
  expose delivery receipts to this app.

---

## 9. Why the Server IP Is Not in the Header

This was a deliberate decision from an earlier session, preserved and
reinforced in this redesign: day-to-day staff (reception, doctors,
nurses) never need to know or see the server's IP address. It's
relevant only to whoever installs/maintains the LAN setup. It lives in
a **collapsed-by-default "Advanced / Developer Info" section** at the
bottom of Settings — discoverable by someone who knows to look, invisible
otherwise. The old `Header.tsx` (pre-redesign) showed the IP directly in
the top bar; this redesign removes that and fixes a stale Dashboard
string that referenced "the value shown in the top header."

---

## 10. Accessibility

- Focus rings: a global `:focus-visible` rule (2px solid ring, using the
  same `--ring` token as everything else) rather than relying on
  browser/Tailwind defaults, which are inconsistent across components.
- `prefers-reduced-motion: reduce` is respected globally — both via a
  CSS media query (collapses all CSS transitions/animations to ~0) and
  Motion's own animations, which read the same media query natively.
- Color contrast: status colors and the primary/accent palette were
  chosen at saturation/lightness values that hold WCAG AA contrast
  against both the light (`98%` lightness) and dark (`8%` lightness)
  backgrounds — verify with a contrast checker if you adjust any token,
  since shifting lightness even 5-10% can drop below AA at small text
  sizes.
- All icon-only buttons (sidebar collapse toggle, header icons, table row
  actions) have `title` attributes; several also have `aria-label` where
  the visual icon alone wouldn't convey intent to a screen reader (e.g.
  "Open navigation menu" on the hamburger).

---

## 11. What Was NOT Built (explicitly out of scope this round)

- **Reports and Billing pages** — reserved as disabled nav items with
  "Coming soon" labels, per explicit instruction to restyle existing
  modules only, not build new ones.
- **Real-time push for anything other than chat** — appointments/
  patients/doctors rely on React Query's `staleTime`/window-focus
  refetch + explicit invalidation after mutations, not a live socket.
  This matches the actual backend (no push mechanism exists for those
  resources) rather than a redesign gap.
- **A dedicated "Profile"/"Preferences" page** — the header's profile
  dropdown has placeholder disabled items for these; no backend concept
  of user accounts exists yet to build a real one against.

---

## 12. File Map (what changed, where)

```
src/
  main.tsx                  — QueryClientProvider + HashRouter wrapping
  App.tsx                   — boot sequence + route table (was: tab-state switch)
  index.css                 — full token system (was: smaller teal/blue set)
  lib/
    models.ts               — NEW: centralized TS shapes matching Rust structs
    queries.ts              — NEW: every React Query hook
    utils.ts                — unchanged
  components/
    layout/
      AppShell.tsx          — NEW: sidebar + header + mobile drawer composition
      Sidebar.tsx            — rewritten: collapsible, NavLink-based, persisted state
      Header.tsx             — rewritten: IP removed, search/notifications/profile added
      ThemeToggle.tsx        — migrated to Motion
    ui/
      dropdown-menu.tsx      — NEW: shadcn-style wrapper (profile menu needed it)
      card.tsx, tabs.tsx     — legacy animation classes removed
    forms/
      *.tsx (all three)      — migrated to React Query mutations
    Receipt.tsx               — unchanged (built in an earlier session)
  pages/
    Dashboard.tsx             — React Query + Recharts + fixed stale IP copy
    Patients.tsx              — React Query + route-param add-trigger + skeleton/empty states
    Doctors.tsx                — React Query + availability indicator + skeleton/empty states
    Appointments.tsx           — React Query + route-param add-trigger + status tokens
    Messaging.tsx               — React Query + live cache updates via Tauri events
    Settings.tsx                — React Query for log/test-send; pairing/TLS logic untouched
    Setup.tsx                   — styling migrated only; pairing/cert-pinning logic untouched
```
