import { describe, it, expect } from "vitest";
import { PERMISSIONS, ROLE_LABELS } from "@/lib/rbac";

describe("PERMISSIONS", () => {
  it("has DashboardView", () => {
    expect(PERMISSIONS.DashboardView).toBe("dashboard.view");
  });

  it("has PatientsView", () => {
    expect(PERMISSIONS.PatientsView).toBe("patients.view");
  });

  it("has PatientsCreate", () => {
    expect(PERMISSIONS.PatientsCreate).toBe("patients.create");
  });

  it("has AppointmentsView", () => {
    expect(PERMISSIONS.AppointmentsView).toBe("appointments.view");
  });

  it("has BillingView", () => {
    expect(PERMISSIONS.BillingView).toBe("billing.view");
  });

  it("has SettingsManage", () => {
    expect(PERMISSIONS.SettingsManage).toBe("settings.manage");
  });

  it("has IpdView", () => {
    expect(PERMISSIONS.IpdView).toBe("ipd.view");
  });

  it("has LabView", () => {
    expect(PERMISSIONS.LabView).toBe("lab.view");
  });

  it("has QueueView", () => {
    expect(PERMISSIONS.QueueView).toBe("queue.view");
  });

  it("has DoctorsView", () => {
    expect(PERMISSIONS.DoctorsView).toBe("doctors.view");
  });

  it("has UsersView", () => {
    expect(PERMISSIONS.UsersView).toBe("users.view");
  });

  it("has AuditView", () => {
    expect(PERMISSIONS.AuditView).toBe("audit.view");
  });

  // RCTF-IMPL-001 WP-1: WhatsApp permissions
  it("has WhatsAppSend", () => {
    expect(PERMISSIONS.WhatsAppSend).toBe("whatsapp.send");
  });

  it("has WhatsAppView", () => {
    expect(PERMISSIONS.WhatsAppView).toBe("whatsapp.view");
  });
});

describe("ROLE_LABELS", () => {
  it("has super_admin label", () => {
    expect(ROLE_LABELS["super_admin"]).toBeDefined();
  });

  it("has doctor label", () => {
    expect(ROLE_LABELS["doctor"]).toBeDefined();
  });

  it("has nurse label", () => {
    expect(ROLE_LABELS["nurse"]).toBeDefined();
  });

  it("has receptionist label", () => {
    expect(ROLE_LABELS["receptionist"]).toBeDefined();
  });

  it("has patient label", () => {
    expect(ROLE_LABELS["patient"]).toBeDefined();
  });
});
