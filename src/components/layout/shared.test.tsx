import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { Pagination } from "@/components/layout/shared";

describe("Pagination", () => {
  const defaultProps = {
    totalItems: 25,
    page: 1,
    rowsPerPage: 10,
    onPageChange: vi.fn(),
    onRowsPerPageChange: vi.fn(),
  };

  describe("display math", () => {
    it("renders 'Showing 1-10 of 25' on page 1 with 10 rows per page", () => {
      render(<Pagination {...defaultProps} />);
      expect(screen.getByText(/Showing 1.*10 of 25/i)).toBeInTheDocument();
    });

    it("renders 'Showing 11-20 of 25' on page 2", () => {
      render(<Pagination {...defaultProps} page={2} />);
      expect(screen.getByText(/Showing 11.*20 of 25/i)).toBeInTheDocument();
    });

    it("renders 'Showing 21-25 of 25' on the last page", () => {
      render(<Pagination {...defaultProps} page={3} />);
      expect(screen.getByText(/Showing 21.*25 of 25/i)).toBeInTheDocument();
    });

    it("renders 'Showing 0-0 of 0' when there are no items", () => {
      render(<Pagination {...defaultProps} totalItems={0} />);
      expect(screen.getByText(/Showing 0.*0 of 0/i)).toBeInTheDocument();
    });

    it("renders the current page and total pages as 'Page X of Y'", () => {
      render(<Pagination {...defaultProps} page={2} />);
      expect(screen.getByText(/Page 2 of 3/i)).toBeInTheDocument();
    });

    it("shows the rows-per-page value in the select", () => {
      render(<Pagination {...defaultProps} rowsPerPage={25} />);
      const select = screen.getByDisplayValue("25");
      expect(select).toBeInTheDocument();
    });
  });

  describe("navigation buttons", () => {
    it("disables Previous button on page 1", () => {
      render(<Pagination {...defaultProps} page={1} />);
      const prevButton = screen.getByRole("button", { name: /previous/i });
      expect(prevButton).toBeDisabled();
    });

    it("enables Previous button on page 2", () => {
      render(<Pagination {...defaultProps} page={2} />);
      const prevButton = screen.getByRole("button", { name: /previous/i });
      expect(prevButton).not.toBeDisabled();
    });

    it("disables Next button on the last page", () => {
      render(<Pagination {...defaultProps} page={3} />);
      const nextButton = screen.getByRole("button", { name: /next/i });
      expect(nextButton).toBeDisabled();
    });

    it("enables Next button when not on the last page", () => {
      render(<Pagination {...defaultProps} page={1} />);
      const nextButton = screen.getByRole("button", { name: /next/i });
      expect(nextButton).not.toBeDisabled();
    });

    it("calls onPageChange with page-1 when Previous is clicked", () => {
      const onPageChange = vi.fn();
      render(<Pagination {...defaultProps} page={2} onPageChange={onPageChange} />);
      fireEvent.click(screen.getByRole("button", { name: /previous/i }));
      expect(onPageChange).toHaveBeenCalledWith(1);
    });

    it("calls onPageChange with page+1 when Next is clicked", () => {
      const onPageChange = vi.fn();
      render(<Pagination {...defaultProps} page={1} onPageChange={onPageChange} />);
      fireEvent.click(screen.getByRole("button", { name: /next/i }));
      expect(onPageChange).toHaveBeenCalledWith(2);
    });
  });

  describe("rows-per-page select", () => {
    it("calls onRowsPerPageChange with the new value when changed", () => {
      const onRowsPerPageChange = vi.fn();
      render(<Pagination {...defaultProps} onRowsPerPageChange={onRowsPerPageChange} />);
      const select = screen.getByDisplayValue("10");
      fireEvent.change(select, { target: { value: "25" } });
      expect(onRowsPerPageChange).toHaveBeenCalledWith(25);
    });

    it("renders 10, 25, 50 options", () => {
      render(<Pagination {...defaultProps} />);
      const select = screen.getByDisplayValue("10");
      expect(select).toBeInTheDocument();
      // The select should have 3 options
      const options = select.querySelectorAll("option");
      expect(options).toHaveLength(3);
      expect(options[0]).toHaveValue("10");
      expect(options[1]).toHaveValue("25");
      expect(options[2]).toHaveValue("50");
    });
  });

  describe("accessibility", () => {
    it("has role=navigation", () => {
      const { container } = render(<Pagination {...defaultProps} />);
      const nav = container.querySelector('[role="navigation"]');
      expect(nav).toBeInTheDocument();
    });

    it("has aria-label='Pagination'", () => {
      const { container } = render(<Pagination {...defaultProps} />);
      const nav = container.querySelector('[aria-label="Pagination"]');
      expect(nav).toBeInTheDocument();
    });
  });
});
