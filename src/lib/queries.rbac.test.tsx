/**
 * RBAC query-gate regression tests.
 *
 * Regression guard for the "Couldn't load data / Access denied: this action
 * requires the 'queue.view' permission." toast wall. Root cause: the
 * Dashboard rendered its Queue / IPD / Lab cards conditionally on permission
 * but called the underlying hooks unconditionally, so a role lacking those
 * permissions (billing_clerk, pharmacist, patient) still fired the invoke,
 * the backend `rbac::require` rejected it, and the global QueryCache onError
 * toasted the denial as a data fault.
 *
 * The fix gave the Dashboard's read hooks an `enabled` flag — `useQueue` /
 * `useAdmissions` / `useLabOrders` plus the three AppointmentsView-gated
 * reads (`useTodayAppointments`, `useAppointmentStats`,
 * `useFailedNotifications`). These tests pin the contract: disabled ⇒ no
 * invoke at all, so a forbidden module can never reach the backend and can
 * never toast.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import {
  useQueue,
  useAdmissions,
  useLabOrders,
  useTodayAppointments,
  useAppointmentStats,
  useFailedNotifications,
} from "@/lib/queries";

function wrapper(qc: QueryClient) {
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={qc}>{children}</QueryClientProvider>
  );
}

describe("RBAC query gates", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue([]);
  });

  it("does not invoke get_queue when disabled (no queue.view)", async () => {
    const qc = new QueryClient();
    const { result } = renderHook(() => useQueue(null, false), { wrapper: wrapper(qc) });
    // Give any would-be fetch a window in which it could have fired.
    await new Promise((r) => setTimeout(r, 50));
    expect(invoke).not.toHaveBeenCalled();
    // The observer may exist, but a disabled query never leaves idle.
    expect(result.current.fetchStatus).toBe("idle");
  });

  it("invokes get_queue when enabled", async () => {
    const qc = new QueryClient();
    renderHook(() => useQueue(null, true), { wrapper: wrapper(qc) });
    await waitFor(() => expect(invoke).toHaveBeenCalled());
    expect(invoke).toHaveBeenCalledWith("get_queue", { statusFilter: null });
  });

  it("does not invoke get_admissions when disabled (no ipd.view)", async () => {
    const qc = new QueryClient();
    const { result } = renderHook(() => useAdmissions("admitted", false), {
      wrapper: wrapper(qc),
    });
    await new Promise((r) => setTimeout(r, 50));
    expect(invoke).not.toHaveBeenCalled();
    expect(result.current.fetchStatus).toBe("idle");
  });

  it("invokes get_admissions when enabled", async () => {
    const qc = new QueryClient();
    renderHook(() => useAdmissions("admitted", true), { wrapper: wrapper(qc) });
    await waitFor(() => expect(invoke).toHaveBeenCalled());
    expect(invoke).toHaveBeenCalledWith("get_admissions", { statusFilter: "admitted" });
  });

  it("does not invoke get_lab_orders when disabled (no lab.view)", async () => {
    const qc = new QueryClient();
    const { result } = renderHook(() => useLabOrders(null, false), { wrapper: wrapper(qc) });
    await new Promise((r) => setTimeout(r, 50));
    expect(invoke).not.toHaveBeenCalled();
    expect(result.current.fetchStatus).toBe("idle");
  });

  it("invokes get_lab_orders when enabled", async () => {
    const qc = new QueryClient();
    renderHook(() => useLabOrders(null, true), { wrapper: wrapper(qc) });
    await waitFor(() => expect(invoke).toHaveBeenCalled());
    expect(invoke).toHaveBeenCalledWith("get_lab_orders", { statusFilter: null });
  });

  // The three appointment reads all require AppointmentsView; roles without
  // it (lab_technician, pharmacist, patient) hit the Dashboard too.
  it.each([
    ["get_today_appointments", useTodayAppointments],
    ["get_appointment_stats", useAppointmentStats],
    ["get_failed_notifications", useFailedNotifications],
  ])("does not invoke %s when disabled (no appointments.view)", async (_cmd, hook) => {
    const qc = new QueryClient();
    const { result } = renderHook(() => (hook as (e?: boolean) => unknown)(false), {
      wrapper: wrapper(qc),
    });
    await new Promise((r) => setTimeout(r, 50));
    expect(invoke).not.toHaveBeenCalled();
    expect(result.current).toBeDefined();
  });
});
