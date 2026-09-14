import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { PatientSafetyBanner } from "./PatientSafetyBanner";
import { usePatientEhr } from "@/lib/queries";
import type { PatientEhr } from "@/lib/models";

/**
 * U-01/U-02/U-03 tests — the allergy banner is the SRS §2.2 "prominent
 * warning" on every prescribing / dispensing / MAR screen. These tests
 * pin its contract: shows allergies when present, stays silent when the
 * record is empty (alert-fatigue discipline), and renders nothing while
 * loading (never a false "no allergies" flash while the record streams
 * in — that would be actively dangerous during prescribing).
 */

vi.mock("@/lib/queries", async (importOriginal) => {
  const mod = await importOriginal<typeof import("@/lib/queries")>();
  return {
    ...mod,
    usePatientEhr: vi.fn(),
  };
});

const ehrRow = (over: Partial<PatientEhr>): PatientEhr => ({
  id: 1,
  first_name: "Test",
  last_name: "Patient",
  email: null,
  phone: "03001234567",
  date_of_birth: "1990-05-12",
  gender: "Male",
  address: null,
  created_at: "2026-01-01T00:00:00Z",
  mrn: "MRN-000123",
  blood_group: "O+",
  allergies: null,
  chronic_conditions: null,
  emergency_contact_name: null,
  emergency_contact_phone: null,
  insurance_provider: null,
  insurance_policy_number: null,
  status: "active",
  created_by_user_id: null,
  ...over,
});

function renderBanner(patientId: number | null) {
  const qc = new QueryClient();
  qc.setQueryData(["patients", "ehr-by-id", 1], ehrRow({}));
  return render(
    <QueryClientProvider client={qc}>
      <PatientSafetyBanner patientId={patientId} />
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  vi.mocked(usePatientEhr).mockImplementation((_id: number | null) => {
    const qc = new QueryClient();
    const data = qc.getQueryData<unknown>(["patients", "ehr-by-id", 1]);
    return { data, isLoading: false } as ReturnType<typeof usePatientEhr>;
  });
});

describe("PatientSafetyBanner", () => {
  it("renders nothing while the EHR row is still loading", async () => {
    vi.mocked(usePatientEhr).mockReturnValue({
      data: undefined,
      isLoading: true,
    } as unknown as ReturnType<typeof usePatientEhr>);
    const { container } = renderBanner(1);
    expect(container.firstChild).toBeNull();
  });

  it("renders nothing for a null patient id (dialog not opened yet)", async () => {
    vi.mocked(usePatientEhr).mockReturnValue({
      data: undefined,
      isLoading: false,
    } as unknown as ReturnType<typeof usePatientEhr>);
    const { container } = renderBanner(null);
    expect(container.firstChild).toBeNull();
  });

  it("shows the ALLERGIES alert with the allergen text when present", async () => {
    vi.mocked(usePatientEhr).mockImplementation((id: number | null) => {
      const data = id === 1 ? ehrRow({ allergies: "Penicillin, latex" }) : undefined;
      return { data, isLoading: false } as unknown as ReturnType<typeof usePatientEhr>;
    });
    renderBanner(1);
    expect(screen.getByRole("alert")).toBeInTheDocument();
    expect(screen.getByText("Allergies")).toBeInTheDocument();
    expect(screen.getByText("Penicillin, latex")).toBeInTheDocument();
  });

  it("stays silent for a patient with no allergies, conditions, or blood group", async () => {
    vi.mocked(usePatientEhr).mockImplementation((id: number | null) => {
      const data =
        id === 1
          ? ehrRow({ allergies: null, chronic_conditions: null, blood_group: null })
          : undefined;
      return { data, isLoading: false } as unknown as ReturnType<typeof usePatientEhr>;
    });
    const { container } = renderBanner(1);
    await waitFor(() => expect(container.firstChild).toBeNull());
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("shows blood group + chronic conditions without the red allergy alert", async () => {
    vi.mocked(usePatientEhr).mockImplementation((id: number | null) => {
      const data =
        id === 1 ? ehrRow({ chronic_conditions: "Type 2 diabetes", blood_group: "B+" }) : undefined;
      return { data, isLoading: false } as unknown as ReturnType<typeof usePatientEhr>;
    });
    renderBanner(1);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByText("Type 2 diabetes")).toBeInTheDocument();
    expect(screen.getByText("B+")).toBeInTheDocument();
  });
});
