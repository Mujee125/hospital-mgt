import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

// Hoisted shared mutation mocks — tests assert the actual payload
// DoctorForm submits (stronger than UI-state assertions, and immune to
// Radix Select's jsdom-only display quirks).
const mutationMocks = vi.hoisted(() => ({
  createDoctor: vi.fn().mockResolvedValue(1),
  updateDoctor: vi.fn().mockResolvedValue(undefined),
}));

// Mock the queries module BEFORE importing DoctorForm
vi.mock("@/lib/queries", () => ({
  useCreateDoctor: () => ({
    mutateAsync: mutationMocks.createDoctor,
    isPending: false,
  }),
  useUpdateDoctor: () => ({
    mutateAsync: mutationMocks.updateDoctor,
    isPending: false,
  }),
  // UX-2026-09-13: the form now derives option lists from these hooks.
  useSpecializations: () => ({
    data: ["Cardiology", "General Medicine"],
    isLoading: false,
  }),
  useDoctors: () => ({
    data: [],
    isLoading: false,
  }),
}));

// Mock @tauri-apps/api/core invoke
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue(null),
}));

import { DoctorForm } from "@/components/forms/DoctorForm";

describe("DoctorForm", () => {
  const onSuccess = vi.fn();
  const onCancel = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders all required form fields", () => {
    render(<DoctorForm onSuccess={onSuccess} onCancel={onCancel} />);
    expect(screen.getByPlaceholderText("Sarah")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("Smith")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("+1 555-0144")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("dr.smith@vitalflow.com")).toBeInTheDocument();
  });

  it("renders the specialization select with an add-option button", () => {
    render(<DoctorForm onSuccess={onSuccess} onCancel={onCancel} />);
    // Specialization is now a dropdown (Radix Select trigger = combobox).
    expect(screen.getByRole("combobox")).toBeInTheDocument();
    // The "+ Add" button beside it lets the operator create a new option.
    expect(
      screen.getByRole("button", { name: /add new specialization option/i }),
    ).toBeInTheDocument();
  });

  it("renders selectable qualification option chips with an Add input", () => {
    render(<DoctorForm onSuccess={onSuccess} onCancel={onCancel} />);
    // Preset qualification chips render as toggle buttons.
    expect(screen.getByRole("button", { name: "MBBS" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "FCPS" })).toBeInTheDocument();
    // The add-other input is associated with the "Qualifications" label.
    expect(
      screen.getByLabelText(/Qualifications/i, { selector: "input" }),
    ).toBeInTheDocument();
    expect(screen.getByPlaceholderText("Add other qualification…")).toBeInTheDocument();
  });

  it("renders availability time inputs", () => {
    render(<DoctorForm onSuccess={onSuccess} onCancel={onCancel} />);
    const timeInputs = document.querySelectorAll('input[type="time"]');
    expect(timeInputs.length).toBeGreaterThanOrEqual(2);
  });

  it("renders Cancel and Register doctor buttons", () => {
    render(<DoctorForm onSuccess={onSuccess} onCancel={onCancel} />);
    expect(screen.getByRole("button", { name: /cancel/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /register doctor/i })).toBeInTheDocument();
  });

  it("calls onCancel when Cancel is clicked", () => {
    render(<DoctorForm onSuccess={onSuccess} onCancel={onCancel} />);
    fireEvent.click(screen.getByRole("button", { name: /cancel/i }));
    expect(onCancel).toHaveBeenCalled();
  });

  it("has required attribute on required fields", () => {
    render(<DoctorForm onSuccess={onSuccess} onCancel={onCancel} />);
    // The form uses HTML required attributes for validation, not disabled button.
    // Verify the required inputs have the required attribute.
    const firstNameInput = screen.getByPlaceholderText("Sarah");
    expect(firstNameInput).toHaveAttribute("required");
    const lastNameInput = screen.getByPlaceholderText("Smith");
    expect(lastNameInput).toHaveAttribute("required");
  });

  it("pre-fills fields when editing an existing doctor", () => {
    const doctor = {
      id: 1,
      first_name: "John",
      last_name: "Doe",
      email: "john@example.com",
      phone: "+92 300 1234567",
      specialization: "Cardiology",
      qualification: "MBBS, MD",
      available_from: "09:00",
      available_to: "17:00",
      is_active: true,
    };
    render(<DoctorForm doctor={doctor} onSuccess={onSuccess} onCancel={onCancel} />);
    expect(screen.getByDisplayValue("John")).toBeInTheDocument();
    expect(screen.getByDisplayValue("Doe")).toBeInTheDocument();
    expect(screen.getByDisplayValue("john@example.com")).toBeInTheDocument();
    // Radix Select shows the selected value as trigger text, not an input
    // value — assert the trigger's text (getByText would also match the
    // always-rendered hidden option list).
    expect(screen.getByRole("combobox")).toHaveTextContent("Cardiology");
    // The stored "MBBS, MD" parses into pressed chips.
    expect(screen.getByRole("button", { name: "MBBS" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByRole("button", { name: "MD" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });

  it("shows 'Save changes' button text when editing", () => {
    const doctor = {
      id: 1,
      first_name: "John",
      last_name: "Doe",
      email: null,
      phone: "+92 300 1234567",
      specialization: "Cardiology",
      qualification: "MBBS",
      available_from: "09:00",
      available_to: "17:00",
      is_active: true,
    };
    render(<DoctorForm doctor={doctor} onSuccess={onSuccess} onCancel={onCancel} />);
    expect(screen.getByRole("button", { name: /save changes/i })).toBeInTheDocument();
  });

  it("shows active status toggle when editing", () => {
    const doctor = {
      id: 1,
      first_name: "John",
      last_name: "Doe",
      email: null,
      phone: "+92 300 1234567",
      specialization: "Cardiology",
      qualification: "MBBS",
      available_from: "09:00",
      available_to: "17:00",
      is_active: true,
    };
    render(<DoctorForm doctor={doctor} onSuccess={onSuccess} onCancel={onCancel} />);
    const checkbox = screen.getByRole("checkbox");
    expect(checkbox).toBeInTheDocument();
    expect(checkbox).toBeChecked();
  });

  it("does not show active toggle when creating a new doctor", () => {
    render(<DoctorForm onSuccess={onSuccess} onCancel={onCancel} />);
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
  });

  it("supports adding a new specialization option and selecting qualifications", () => {
    render(<DoctorForm onSuccess={onSuccess} onCancel={onCancel} />);
    // Fill the text fields.
    fireEvent.change(screen.getByPlaceholderText("Sarah"), { target: { value: "John" } });
    fireEvent.change(screen.getByPlaceholderText("Smith"), { target: { value: "Doe" } });
    fireEvent.change(screen.getByPlaceholderText("+1 555-0144"), { target: { value: "+92 300 1234567" } });

    // Add a NEW specialization via the + button (a value not in any list).
    fireEvent.click(screen.getByRole("button", { name: /add new specialization option/i }));
    const newSpecInput = screen.getByPlaceholderText("Type the new specialization");
    fireEvent.change(newSpecInput, { target: { value: "Cosmetic Surgery" } });
    fireEvent.click(screen.getByRole("button", { name: "Add specialization option" }));

    // Select one qualification chip.
    fireEvent.click(screen.getByRole("button", { name: "MBBS" }));
    expect(screen.getByRole("button", { name: "MBBS" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );

    // Submit and verify the DATA FLOW: the session-added specialization
    // and the selected qualification land in the create payload exactly
    // as the backend contract expects. (Trigger display text is covered
    // by the Playwright e2e in a real browser — Radix Select has a
    // jsdom-only display quirk with batched item+value updates.)
    fireEvent.click(screen.getByRole("button", { name: /register doctor/i }));
    expect(mutationMocks.createDoctor).toHaveBeenCalledWith(
      expect.objectContaining({
        specialization: "Cosmetic Surgery",
        qualification: "MBBS",
      }),
    );
  });

  it("enables the toggle to be unchecked", () => {
    const doctor = {
      id: 1,
      first_name: "John",
      last_name: "Doe",
      email: null,
      phone: "+92 300 1234567",
      specialization: "Cardiology",
      qualification: "MBBS",
      available_from: "09:00",
      available_to: "17:00",
      is_active: true,
    };
    render(<DoctorForm doctor={doctor} onSuccess={onSuccess} onCancel={onCancel} />);
    const checkbox = screen.getByRole("checkbox");
    expect(checkbox).toBeChecked();
    fireEvent.click(checkbox);
    expect(checkbox).not.toBeChecked();
  });

  it("has proper labels associated with inputs", () => {
    render(<DoctorForm onSuccess={onSuccess} onCancel={onCancel} />);
    expect(screen.getByLabelText(/First name/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/Last name/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/Contact phone/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/Professional email/i)).toBeInTheDocument();
    // Specialization label → the Select trigger (role=combobox) and
    // Qualifications label → the chips Add input: assert the explicit
    // label[for] → control linkage (getByLabelText is ambiguous here
    // because the + button's aria-label also contains "specialization").
    expect(document.querySelector('label[for="specialization"]')).not.toBeNull();
    expect(document.getElementById("specialization")?.getAttribute("role")).toBe(
      "combobox",
    );
    expect(document.querySelector('label[for="qualification"]')).not.toBeNull();
    expect(document.getElementById("qualification")?.tagName).toBe("INPUT");
  });
});
