import { describe, it, expect } from "vitest";
import { CURRENCY, CURRENCY_SYMBOL, formatMoney } from "@/lib/utils";

describe("CURRENCY constants", () => {
  it("CURRENCY is PKR", () => {
    expect(CURRENCY).toBe("PKR");
  });

  it("CURRENCY_SYMBOL is Rs", () => {
    expect(CURRENCY_SYMBOL).toBe("Rs");
  });
});

describe("formatMoney", () => {
  it("formats a whole number", () => {
    expect(formatMoney(500)).toBe("Rs 500");
  });

  it("formats a decimal number", () => {
    expect(formatMoney(1500.5)).toBe("Rs 1,500.5");
  });

  it("formats a string number", () => {
    expect(formatMoney("123.45")).toBe("Rs 123.45");
  });

  it("formats zero", () => {
    expect(formatMoney(0)).toBe("Rs 0");
  });

  it("formats null as dash", () => {
    expect(formatMoney(null)).toBe("—");
  });

  it("formats NaN as dash", () => {
    expect(formatMoney(NaN)).toBe("—");
  });

  it("formats Infinity as dash", () => {
    expect(formatMoney(Infinity)).toBe("—");
  });

  it("formats negative Infinity as dash", () => {
    expect(formatMoney(-Infinity)).toBe("—");
  });

  it("formats undefined as dash", () => {
    expect(formatMoney(undefined as unknown as null)).toBe("—");
  });

  it("formats a large number with commas", () => {
    const result = formatMoney(1234567);
    expect(result).toContain("Rs");
    expect(result).toContain("1,234,567");
  });

  it("formats a string with decimal", () => {
    expect(formatMoney("99.99")).toBe("Rs 99.99");
  });

  it("formats an empty string as dash", () => {
    expect(formatMoney("")).toBe("—");
  });
});
