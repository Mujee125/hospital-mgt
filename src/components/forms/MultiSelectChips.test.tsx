import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { MultiSelectChips } from "./MultiSelectChips";

/**
 * MultiSelectChips contract tests (UX-2026-09-13): the control stores the
 * SAME comma-separated string the old free-text Textareas produced, so
 * existing records and downstream consumers (PatientSafetyBanner,
 * pickers, get_specializations DISTINCT) are untouched.
 */
const OPTIONS = ["Penicillin", "Latex", "Soy"];

function setup(value = "", onChange = vi.fn()) {
  render(
    <MultiSelectChips
      id="allergies"
      label="Allergies"
      options={OPTIONS}
      value={value}
      onChange={onChange}
      placeholder="Add other allergy…"
    />,
  );
  return { onChange };
}

describe("MultiSelectChips — preset chips", () => {
  it("renders every option as a toggle chip, unselected by default", () => {
    setup();
    for (const opt of OPTIONS) {
      const chip = screen.getByRole("button", { name: opt });
      expect(chip).toHaveAttribute("aria-pressed", "false");
    }
  });

  it("toggling a chip calls onChange with the joined string", () => {
    const { onChange } = setup();
    fireEvent.click(screen.getByRole("button", { name: "Penicillin" }));
    expect(onChange).toHaveBeenCalledWith("Penicillin");
  });

  it("toggling a second chip appends in click order", () => {
    const { onChange } = setup("Penicillin");
    fireEvent.click(screen.getByRole("button", { name: "Latex" }));
    expect(onChange).toHaveBeenCalledWith("Penicillin, Latex");
  });

  it("toggling off removes only that item", () => {
    const { onChange } = setup("Penicillin, Latex");
    fireEvent.click(screen.getByRole("button", { name: "Penicillin" }));
    expect(onChange).toHaveBeenCalledWith("Latex");
  });

  it("marks stored values as pressed (case-insensitive match)", () => {
    setup("penicillin");
    expect(
      screen.getByRole("button", { name: "Penicillin" }),
    ).toHaveAttribute("aria-pressed", "true");
  });
});

describe("MultiSelectChips — custom items", () => {
  it("renders unknown stored values as removable chips", () => {
    setup("Bee stings"); // not in OPTIONS
    expect(
      screen.getByRole("button", { name: "Remove Bee stings" }),
    ).toBeInTheDocument();
  });

  it("clicking a custom chip's remove calls onChange without it", () => {
    const { onChange } = setup("Penicillin, Bee stings");
    fireEvent.click(screen.getByRole("button", { name: "Remove Bee stings" }));
    expect(onChange).toHaveBeenCalledWith("Penicillin");
  });
});

describe("MultiSelectChips — the Add input", () => {
  it("the Add button adds the typed value", () => {
    const { onChange } = setup("Penicillin");
    const input = screen.getByLabelText("Add new allergies option");
    fireEvent.change(input, { target: { value: "Kiwi" } });
    fireEvent.click(screen.getByRole("button", { name: /^Add$/ }));
    expect(onChange).toHaveBeenCalledWith("Penicillin, Kiwi");
    expect(input).toHaveValue("");
  });

  it("Enter adds the typed value and never submits the surrounding form", () => {
    const onSubmit = vi.fn();
    const onChange = vi.fn();
    render(
      <form onSubmit={onSubmit}>
        <MultiSelectChips
          id="allergies"
          label="Allergies"
          options={OPTIONS}
          value=""
          onChange={onChange}
        />
      </form>,
    );
    const input = screen.getByLabelText("Add new allergies option");
    fireEvent.change(input, { target: { value: "Kiwi" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onChange).toHaveBeenCalledWith("Kiwi");
    expect(onSubmit).not.toHaveBeenCalled(); // preventDefault guard
  });

  it("comma key adds too (common typing habit)", () => {
    const { onChange } = setup();
    const input = screen.getByLabelText("Add new allergies option");
    fireEvent.change(input, { target: { value: "Kiwi," } });
    fireEvent.keyDown(input, { key: "," });
    expect(onChange).toHaveBeenCalledWith("Kiwi");
  });

  it("adding a case-insensitive duplicate is a no-op (no double entry)", () => {
    const { onChange } = setup("Penicillin");
    const input = screen.getByLabelText("Add new allergies option");
    fireEvent.change(input, { target: { value: "penicillin" } });
    fireEvent.click(screen.getByRole("button", { name: /^Add$/ }));
    expect(onChange).not.toHaveBeenCalled();
    expect(input).toHaveValue("");
  });

  it("the Add button is disabled while the draft is empty", () => {
    setup();
    expect(screen.getByRole("button", { name: /^Add$/ })).toBeDisabled();
  });
});

describe("MultiSelectChips — accessibility", () => {
  it("exposes the group label and disables cleanly", () => {
    render(
      <MultiSelectChips
        id="chronic_conditions"
        label="Chronic conditions"
        options={["Asthma"]}
        value=""
        onChange={vi.fn()}
        disabled
      />,
    );
    expect(screen.getByRole("group", { name: "Chronic conditions" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Asthma" })).toBeDisabled();
  });
});
