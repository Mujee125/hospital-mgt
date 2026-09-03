import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { ErrorBoundary } from "@/components/ErrorBoundary";

describe("ErrorBoundary", () => {
  it("renders children when no error", () => {
    render(
      <ErrorBoundary>
        <div>Test content</div>
      </ErrorBoundary>,
    );
    expect(screen.getByText("Test content")).toBeInTheDocument();
  });

  it("renders error UI when child throws", () => {
    // Suppress console.error for this test (React logs the error)
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});

    function ThrowingComponent(): never {
      throw new Error("Test error");
    }

    render(
      <ErrorBoundary>
        <ThrowingComponent />
      </ErrorBoundary>,
    );

    expect(screen.getByText("Something went wrong")).toBeInTheDocument();
    expect(screen.getByText(/Reload Application/i)).toBeInTheDocument();
    expect(screen.getByText(/Try to Continue/i)).toBeInTheDocument();
    spy.mockRestore();
  });

  it("shows an error reference ID", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});

    function ThrowingComponent(): never {
      throw new Error("Test error");
    }

    render(
      <ErrorBoundary>
        <ThrowingComponent />
      </ErrorBoundary>,
    );

    expect(screen.getAllByText(/Error Reference/i).length).toBeGreaterThan(0);
    spy.mockRestore();
  });

  it("has a reload button", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});

    function ThrowingComponent(): never {
      throw new Error("Test error");
    }

    render(
      <ErrorBoundary>
        <ThrowingComponent />
      </ErrorBoundary>,
    );

    const reloadButton = screen.getByRole("button", { name: /reload/i });
    expect(reloadButton).toBeInTheDocument();
    spy.mockRestore();
  });

  it("has a try-to-continue button", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});

    function ThrowingComponent(): never {
      throw new Error("Test error");
    }

    render(
      <ErrorBoundary>
        <ThrowingComponent />
      </ErrorBoundary>,
    );

    const continueButton = screen.getByRole("button", { name: /continue/i });
    expect(continueButton).toBeInTheDocument();
    spy.mockRestore();
  });

  it("uses role=alert for accessibility", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});

    function ThrowingComponent(): never {
      throw new Error("Test error");
    }

    const { container } = render(
      <ErrorBoundary>
        <ThrowingComponent />
      </ErrorBoundary>,
    );

    const alert = container.querySelector('[role="alert"]');
    expect(alert).toBeInTheDocument();
    spy.mockRestore();
  });

  it("recovers when try-to-continue is clicked", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});

    function ThrowingComponent(): never {
      throw new Error("Test error");
    }

    const { rerender } = render(
      <ErrorBoundary>
        <ThrowingComponent />
      </ErrorBoundary>,
    );

    // Click "Try to Continue"
    screen.getByRole("button", { name: /continue/i }).click();

    // Re-render with non-throwing children
    rerender(
      <ErrorBoundary>
        <div>Recovered</div>
      </ErrorBoundary>,
    );

    expect(screen.getByText("Recovered")).toBeInTheDocument();
    spy.mockRestore();
  });
});
