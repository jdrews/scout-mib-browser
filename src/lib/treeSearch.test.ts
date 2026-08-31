import { describe, it, expect } from "vitest";
import { parentOid, compareOids, matchesQuery, searchOids, findChain } from "./treeSearch";

describe("parentOid", () => {
  it("removes the last numeric segment", () => {
    expect(parentOid("1.3.6.1.2.1")).toBe("1.3.6.1.2");
    expect(parentOid("1.3")).toBe("1");
  });

  it("returns empty for single-segment OIDs", () => {
    expect(parentOid("1")).toBe("");
    expect(parentOid("")).toBe("");
  });
});

describe("compareOids", () => {
  it("compares sub-identifiers numerically, not lexicographically", () => {
    expect(compareOids("1.3.6.1.2.1.9", "1.3.6.1.2.1.10") < 0).toBe(true);
    expect(compareOids("1.3.6.1.2.1.10", "1.3.6.1.2.1.9") > 0).toBe(true);
  });

  it("orders a prefix before its descendants", () => {
    expect(compareOids("1.3.6.1", "1.3.6.1.2") < 0).toBe(true);
  });

  it("treats equal OIDs as equal", () => {
    expect(compareOids("1.3.6", "1.3.6")).toBe(0);
  });
});

describe("matchesQuery", () => {
  const oid = "1.3.6.1.2.1.2.2.1.3";
  const name = "ifType";

  it("matches names case-insensitively as a substring", () => {
    expect(matchesQuery(oid, name, "ifType")).toBe(true);
    expect(matchesQuery(oid, name, "IFTYPE")).toBe(true);
    expect(matchesQuery(oid, name, "type")).toBe(true);
    expect(matchesQuery(oid, name, "descr")).toBe(false);
  });

  it("matches the exact OID", () => {
    expect(matchesQuery(oid, name, oid)).toBe(true);
  });

  it("matches an OID segment prefix (subtree search)", () => {
    expect(matchesQuery(oid, name, "1.3.6.1.2.1")).toBe(true);
    expect(matchesQuery(oid, name, "1.3.6.1.2.1.2.2.1")).toBe(true);
  });

  it("does not match a partial trailing segment", () => {
    // "1.3.6" must not match an OID starting "1.3.61".
    expect(matchesQuery("1.3.61.2.1", "otherOid", "1.3.6")).toBe(false);
    expect(matchesQuery(oid, name, "1.3.6.1.2.1.2.2.1")).toBe(true);
    expect(matchesQuery(oid, name, "1.3.6.1.2.1.2.2.1.30")).toBe(false);
  });

  it("does not match unrelated queries", () => {
    expect(matchesQuery(oid, name, "zzz")).toBe(false);
    expect(matchesQuery(oid, name, "9.9.9")).toBe(false);
  });
});

describe("searchOids", () => {
  const map = new Map<string, string>([
    ["1.3.6.1.2.1.1.1", "sysDescr"],
    ["1.3.6.1.2.1.2.2.1.3", "ifType"],
    ["1.3.6.1.2.1.2.2.1.4", "ifDescr"],
    ["1.3.6.1.2.1.31.1.1.1", "ipAdEntAddr"],
  ]);

  it("returns empty for a blank query", () => {
    expect(searchOids(map, "")).toEqual([]);
    expect(searchOids(map, "   ")).toEqual([]);
  });

  it("finds by name and trims the query", () => {
    expect(searchOids(map, " ifType ")).toEqual(["1.3.6.1.2.1.2.2.1.3"]);
  });

  it("finds by OID prefix and returns numeric OID order", () => {
    expect(searchOids(map, "1.3.6.1.2.1.2")).toEqual([
      "1.3.6.1.2.1.2.2.1.3",
      "1.3.6.1.2.1.2.2.1.4",
    ]);
  });

  it("combines name and OID hits without duplicates", () => {
    // "if" matches ifType and ifDescr by name; the OID form matches nothing extra.
    expect(searchOids(map, "if")).toEqual([
      "1.3.6.1.2.1.2.2.1.3",
      "1.3.6.1.2.1.2.2.1.4",
    ]);
  });

  it("sorts numerically across many hits", () => {
    const big = new Map<string, string>([
      ["1.3.6.1.2.1.10", "ten"],
      ["1.3.6.1.2.1.9", "nine"],
      ["1.3.6.1.2.1.2", "two"],
    ]);
    expect(searchOids(big, "1.3.6.1.2.1")).toEqual([
      "1.3.6.1.2.1.2",
      "1.3.6.1.2.1.9",
      "1.3.6.1.2.1.10",
    ]);
  });
});

describe("findChain", () => {
  const map = new Map<string, string>([
    ["1", "internet"],
    ["1.3", "dot-3"],
    ["1.3.6", "dot-6"],
    ["1.3.6.1", "iso"],
    ["1.3.6.1.2", "dod"],
    ["1.3.6.1.2.1", "mib-2"],
    ["1.3.6.1.2.1.2", "interfaces"],
    ["1.3.6.1.2.1.2.2", "ifTable"],
    ["1.3.6.1.2.1.2.2.1", "ifEntry"],
    ["1.3.6.1.2.1.2.2.1.3", "ifType"],
  ]);

  it("walks up through indexed parents to the root", () => {
    expect(findChain("1.3.6.1.2.1.2.2.1.3", map)).toEqual([
      "1",
      "1.3",
      "1.3.6",
      "1.3.6.1",
      "1.3.6.1.2",
      "1.3.6.1.2.1",
      "1.3.6.1.2.1.2",
      "1.3.6.1.2.1.2.2",
      "1.3.6.1.2.1.2.2.1",
      "1.3.6.1.2.1.2.2.1.3",
    ]);
  });

  it("stops at an orphan whose parent is not indexed (itself a root)", () => {
    const sparse = new Map<string, string>([
      ["1", "internet"],
      ["1.9.9", "orphaned"],
    ]);
    expect(findChain("1.9.9", sparse)).toEqual(["1.9.9"]);
  });

  it("returns a single-segment OID as its own chain", () => {
    expect(findChain("2", map)).toEqual(["2"]);
  });

  it("includes the target as the last element", () => {
    const chain = findChain("1.3.6.1.2.1", map);
    expect(chain[chain.length - 1]).toBe("1.3.6.1.2.1");
    expect(chain).toEqual(["1", "1.3", "1.3.6", "1.3.6.1", "1.3.6.1.2", "1.3.6.1.2.1"]);
  });
});
