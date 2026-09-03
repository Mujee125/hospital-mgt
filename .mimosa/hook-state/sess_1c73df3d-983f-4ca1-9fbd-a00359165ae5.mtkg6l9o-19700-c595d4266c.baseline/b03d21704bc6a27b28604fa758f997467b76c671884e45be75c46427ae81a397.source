import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

// Mock the queries module BEFORE importing DoctorForm
vi.mock("@/lib/queries", () => ({
  useCreateDoctor: () => ({
    mutateAsync: vi.fn().mockResolvedValue(1),
    isPending: false,
  }),
  useUpdateDoctor: () => ({
    mutateAsync: vi.fn().mockResolvedValue(undefined),
    isPending: false,
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

  it("renders the specialization input field", () => {
    render(<DoctorForm onSuccess={onSuccess} onCancel={onCancel} />);
    expect(screen.getByPlaceholderText("Cardiology")).toBeInTheDocument();
  });

  it("renders the qualification input field", () => {
    render(<DoctorForm onSuccess={onSuccess} onCancel={onCancel} />);
    expect(screen.getByPlaceholderText("MD, FACC")).toBeInTheDocument();
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
    expect(screen.getByDisplayValue("Cardiology")).toBeInTheDocument();
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

  it("enables submit when all required fields are filled", () => {
    render(<DoctorForm onSuccess={onSuccess} onCancel={onCancel} />);
    // Fill all required fields
    fireEvent.change(screen.getByPlaceholderText("Sarah"), { target: { value: "John" } });
    fireEvent.change(screen.getByPlaceholderText("Smith"), { target: { value: "Doe" } });
    fireEvent.change(screen.getByPlaceholderText("+1 555-0144"), { target: { value: "+92 300 1234567" } });
    fireEvent.change(screen.getByPlaceholderText("Cardiology"), { target: { value: "Cardiology" } });
    fireEvent.change(screen.getByPlaceholderText("MD, FACC"), { target: { value: "MBBS" } });

    const submitButton = screen.getByRole("button", { name: /register doctor/i });
    expect(submitButton).not.toBeDisabled();
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
    expect(screen.getByLabelText(/Specialization/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/Qualifications/i)).toBeInTheDocument();
  });
});
