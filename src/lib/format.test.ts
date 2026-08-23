import { describe, it, expect } from "vitest";
import { pluralize } from "./format";

describe("pluralize", () => {
  it("uses the singular form for exactly one", () => {
    expect(pluralize(1, "node")).toBe("1 node");
  });

  it("uses the plural form for zero and multiple", () => {
    expect(pluralize(0, "node")).toBe("0 nodes");
    expect(pluralize(2, "node")).toBe("2 nodes");
    expect(pluralize(42, "MIB")).toBe("42 MIBs");
  });

  it("accepts an explicit plural form", () => {
    expect(pluralize(1, "binding", "bindings")).toBe("1 binding");
    expect(pluralize(3, "binding", "bindings")).toBe("3 bindings");
  });
});
