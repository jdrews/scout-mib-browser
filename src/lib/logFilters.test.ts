import { describe, it, expect } from "vitest";
import { passesLogFilter } from "./logFilters";

// The System Log's "Info+" filter must show INFO and above — the shipped
// predicate returned true for everything, leaking DEBUG/TRACE into Info+
// (launch-beta/10).

describe("passesLogFilter (launch-beta/10)", () => {
  it("all shows every level", () => {
    for (const level of ["TRACE", "DEBUG", "INFO", "WARN", "ERROR"]) {
      expect(passesLogFilter(level, "all")).toBe(true);
    }
  });

  it("info shows INFO and above, excluding DEBUG and TRACE", () => {
    expect(passesLogFilter("INFO", "info")).toBe(true);
    expect(passesLogFilter("WARN", "info")).toBe(true);
    expect(passesLogFilter("ERROR", "info")).toBe(true);
    expect(passesLogFilter("DEBUG", "info")).toBe(false);
    expect(passesLogFilter("TRACE", "info")).toBe(false);
  });

  it("warn shows only WARN and ERROR", () => {
    expect(passesLogFilter("WARN", "warn")).toBe(true);
    expect(passesLogFilter("ERROR", "warn")).toBe(true);
    expect(passesLogFilter("INFO", "warn")).toBe(false);
    expect(passesLogFilter("DEBUG", "warn")).toBe(false);
  });

  it("error shows only ERROR", () => {
    expect(passesLogFilter("ERROR", "error")).toBe(true);
    expect(passesLogFilter("WARN", "error")).toBe(false);
    expect(passesLogFilter("INFO", "error")).toBe(false);
  });

  it("level matching is case-insensitive", () => {
    expect(passesLogFilter("debug", "info")).toBe(false);
    expect(passesLogFilter("info", "info")).toBe(true);
    expect(passesLogFilter("error", "warn")).toBe(true);
  });

  it("an unknown level ranks as INFO (visible at Info+)", () => {
    expect(passesLogFilter("NOTICE", "info")).toBe(true);
    expect(passesLogFilter("NOTICE", "warn")).toBe(false);
  });
});
