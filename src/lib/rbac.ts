/**
 * Client-side RBAC primitives — mirrors the Rust `Permission` enum in
 * `src-tauri/src/rbac.rs` exactly. The server is the source of truth and
 * re-checks every command; these constants drive UI affordances (show/hide
 * nav items, disable buttons) so users never see actions they can't perform.
 *
 * If you add a permission to the Rust enum, add it here too and to the
 * role→permission map in `rbac.rs::permissions_for_role`.
 */

export const PERMISSIONS = {
  DashboardView: "dashboard.view",
  PatientsView: "patients.view",
  PatientsCreate: "patients.create",
  PatientsUpdate: "patients.update",
  PatientsDelete: "patients.delete",
  PatientConsentManage: "patients.consent.manage",
  AppointmentsView: "appointments.view",
  AppointmentsCreate: "appointments.create",
  AppointmentsUpdate: "appointments.update",
  AppointmentsDelete: "appointments.delete",
  QueueView: "queue.view",
  QueueManage: "queue.manage",
  DoctorsView: "doctors.view",
  DoctorsManage: "doctors.manage",
  IpdView: "ipd.view",
  IpdManage: "ipd.manage",
  BedsManage: "beds.manage",
  LabView: "lab.view",
  LabOrder: "lab.order",
  LabResultManage: "lab.result.manage",
  LabCatalogManage: "lab.catalog.manage",
  RadiologyView: "radiology.view",
  RadiologyCreate: "radiology.create",
  RadiologyUpdate: "radiology.update",
  RadiologyDelete: "radiology.delete",
  RadiologyReport: "radiology.report",
  RadiologyVerify: "radiology.verify",
  RadiologyManage: "radiology.manage",
  BloodBankView: "bloodbank.view",
  BloodBankManage: "bloodbank.manage",
  BloodBankDonorManage: "bloodbank.donor.manage",
  BloodBankCrossmatch: "bloodbank.crossmatch",
  BloodBankIssue: "bloodbank.issue",
  BloodBankTransfuse: "bloodbank.transfuse",
  BloodBankDiscard: "bloodbank.discard",
  BloodBankVerify: "bloodbank.verify",
  BillingView: "billing.view",
  BillingCreate: "billing.create",
  BillingManage: "billing.manage",
  PaymentsManage: "payments.manage",
  InventoryView: "inventory.view",
  InventoryManage: "inventory.manage",
  UsersView: "users.view",
  UsersManage: "users.manage",
  RolesManage: "roles.manage",
  AuditView: "audit.view",
  ReportsView: "reports.view",
  // RCTF-IMPL-001 WP-1: WhatsApp permissions for patient-facing comms.
  WhatsAppSend: "whatsapp.send",
  WhatsAppView: "whatsapp.view",
  SettingsManage: "settings.manage",
  LicenseManage: "license.manage",
  BackupsManage: "backups.manage",
} as const;

export type Permission = (typeof PERMISSIONS)[keyof typeof PERMISSIONS];

export const ROLE_LABELS: Record<string, string> = {
  super_admin: "Super Administrator",
  doctor: "Doctor",
  nurse: "Nurse",
  receptionist: "Receptionist",
  lab_technician: "Lab Technician",
  pharmacist: "Pharmacist",
  billing_clerk: "Billing Clerk",
  patient: "Patient",
};

export interface AuthUser {
  id: number;
  username: string;
  full_name: string;
  email: string | null;
  is_active: boolean;
  must_change_password: boolean;
  last_login_at: string | null;
}

export interface Session {
  user: AuthUser;
  roles: string[];
  permissions: string[];
  must_change_password: boolean;
}

export function can(permissions: string[] | undefined, perm: Permission): boolean {
  return !!permissions?.includes(perm);
}

/** Sidebar navigation item with the permission required to see it. */
export interface NavItem {
  to: string;
  label: string;
  icon: unknown; // Lucide icon component
  end?: boolean;
  badge?: string;
  requiredPermission?: Permission;
  disabled?: boolean;
  note?: string;
}
