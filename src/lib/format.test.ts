import { describe, it, expect } from "vitest";
import { pluralize, dedent } from "./format";

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

describe("dedent", () => {
  it("strips the source indent from continuation lines", () => {
    // The shape MIB DESCRIPTION clauses arrive in: the first line starts
    // right after the opening quote, the rest carry the file indentation.
    const mibDesc =
      "A textual description of the entity.  This value should\n" +
      "        include the full name and version identification of\n" +
      "        the system's hardware type, software operating-system,\n" +
      "        and networking software.";
    expect(dedent(mibDesc)).toBe(
      "A textual description of the entity.  This value should\n" +
        "include the full name and version identification of\n" +
        "the system's hardware type, software operating-system,\n" +
        "and networking software.",
    );
  });

  it("strips a uniform indent as well", () => {
    expect(dedent("    a\n    b")).toBe("a\nb");
  });

  it("keeps interior spacing and blank lines", () => {
    expect(dedent("a  b\n\n    c")).toBe("a  b\n\nc");
  });

  it("handles a single line and empty string", () => {
    expect(dedent("    one")).toBe("one");
    expect(dedent("")).toBe("");
  });
});
