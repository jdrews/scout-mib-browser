import { describe, it, expect } from "vitest";
import type { MibNodeDetails, SnmpValue } from "$lib/types";
import {
  isWritable,
  isConfirmedWritable,
  setGuard,
  effectiveSetOid,
  mibSyntaxToSetValueType,
  setDetailsFor,
  parseIntegerRange,
  parseSizeBounds,
  validateInteger,
  validateUnsigned32,
  validateCounter64,
  validateOctetString,
  validateIpv4,
  isValidOidString,
  parseHexPairs,
  bitsToBytes,
  bytesToBits,
  prefillFromValue,
  defvalPrefill,
  pduErrorName,
} from "$lib/setLogic";

function details(overrides: Partial<MibNodeDetails> = {}): MibNodeDetails {
  return {
    oid: "1.3.6.1.2.1.1.5",
    name: "sysName",
    mibName: "SNMPv2-MIB",
    syntaxType: "OctetString",
    ...overrides,
  };
}

describe("isWritable", () => {
  it("true for read-write and read-create", () => {
    expect(isWritable("read-write")).toBe(true);
    expect(isWritable("read-create")).toBe(true);
  });

  it("false for confirmed non-writable access", () => {
    expect(isWritable("read-only")).toBe(false);
    expect(isWritable("not-accessible")).toBe(false);
    expect(isWritable("accessible-for-notify")).toBe(false);
  });

  it("unknown (undefined) is not false — settable unless we know it isn't", () => {
    expect(isWritable(undefined)).toBe(true);
  });
});

describe("isConfirmedWritable", () => {
  it("true only for read-write and read-create", () => {
    expect(isConfirmedWritable("read-write")).toBe(true);
    expect(isConfirmedWritable("read-create")).toBe(true);
    expect(isConfirmedWritable("read-only")).toBe(false);
    expect(isConfirmedWritable(undefined)).toBe(false);
  });
});

describe("setGuard", () => {
  it("refuses table nodes", () => {
    const msg = setGuard(
      "1.3.6.1.2.1.2.2",
      details({ oid: "1.3.6.1.2.1.2.2", name: "ifTable", syntaxType: "TABLE", isTable: true }),
    );
    expect(msg).toBe("ifTable is a table — Set targets a column instance");
  });

  it("refuses row entries", () => {
    const msg = setGuard(
      "1.3.6.1.2.1.2.2.1",
      details({ oid: "1.3.6.1.2.1.2.2.1", name: "ifEntry", syntaxType: "ROW" }),
    );
    expect(msg).toBe("ifEntry is a row entry — Set targets a column instance");
  });

  it("refuses confirmed non-writable access", () => {
    const msg = setGuard(
      "1.3.6.1.2.1.1.1",
      details({ oid: "1.3.6.1.2.1.1.1", name: "sysDescr", access: "read-only" }),
    );
    expect(msg).toBe("sysDescr is read-only");
  });

  it("proceeds for writable scalars and unknown nodes", () => {
    expect(
      setGuard("1.3.6.1.2.1.1.5", details({ access: "read-write" })),
    ).toBeNull();
    expect(setGuard("1.3.6.1.2.1.9999", null)).toBeNull();
  });
});

describe("effectiveSetOid", () => {
  it("appends .0 on exact scalar match", () => {
    expect(effectiveSetOid("1.3.6.1.2.1.1.5", details())).toBe(
      "1.3.6.1.2.1.1.5.0",
    );
  });

  it("passes through Table, TableRow, and ObjectIdentifier nodes", () => {
    expect(
      effectiveSetOid(
        "1.3.6.1.2.1.2.2",
        details({ oid: "1.3.6.1.2.1.2.2", syntaxType: "TABLE", isTable: true }),
      ),
    ).toBe("1.3.6.1.2.1.2.2");
    expect(
      effectiveSetOid(
        "1.3.6.1.2.1.2.2.1",
        details({ oid: "1.3.6.1.2.1.2.2.1", syntaxType: "ROW" }),
      ),
    ).toBe("1.3.6.1.2.1.2.2.1");
    expect(
      effectiveSetOid(
        "1.3.6.1.2.1.1.2",
        details({ oid: "1.3.6.1.2.1.1.2", syntaxType: "ObjectIdentifier" }),
      ),
    ).toBe("1.3.6.1.2.1.1.2");
  });

  it("passes through already-suffixed instance OIDs and unknown nodes", () => {
    // Instance OID resolves to the base node, so details.oid !== oid.
    expect(effectiveSetOid("1.3.6.1.2.1.1.5.0", details())).toBe(
      "1.3.6.1.2.1.1.5.0",
    );
    expect(effectiveSetOid("1.3.6.1.2.1.9999", null)).toBe("1.3.6.1.2.1.9999");
  });
});

describe("setDetailsFor", () => {
  it("keeps null details (type-picker fallback)", () => {
    expect(setDetailsFor("1.3.6.1.2.1.9999", null)).toBeNull();
  });

  it("keeps exact matches, including ObjectIdentifier nodes", () => {
    const oidNode = details({
      oid: "1.3.6.1.4.1",
      name: "enterprises",
      syntaxType: "ObjectIdentifier",
    });
    expect(setDetailsFor("1.3.6.1.4.1", oidNode)).toBe(oidNode);
  });

  it("keeps instance resolutions of known objects", () => {
    // sysName instance resolves to the base node — a real object, not a subtree.
    const d = details();
    expect(setDetailsFor("1.3.6.1.2.1.1.5.0", d)).toBe(d);
  });

  it("nulls ancestor-subtree resolutions of unknown objects", () => {
    // An enterprise instance resolves to `enterprises` (ObjectIdentifier) —
    // the actual object is not in the loaded MIBs.
    const enterprises = details({
      oid: "1.3.6.1.4.1",
      name: "enterprises",
      syntaxType: "ObjectIdentifier",
    });
    expect(setDetailsFor("1.3.6.1.4.1.2021.4.3.0", enterprises)).toBeNull();
  });
});

describe("mibSyntaxToSetValueType", () => {
  it("maps every MIB syntax per the spec table", () => {
    expect(mibSyntaxToSetValueType("Integer32")).toBe("integer");
    expect(mibSyntaxToSetValueType("TruthValue")).toBe("integer");
    expect(mibSyntaxToSetValueType("Counter32")).toBe("counter32");
    expect(mibSyntaxToSetValueType("Counter64")).toBe("counter64");
    expect(mibSyntaxToSetValueType("Gauge32")).toBe("gauge32");
    expect(mibSyntaxToSetValueType("Unsigned32")).toBe("gauge32");
    expect(mibSyntaxToSetValueType("TimeTicks")).toBe("timeticks");
    expect(mibSyntaxToSetValueType("IpAddress")).toBe("ipaddress");
    expect(mibSyntaxToSetValueType("ObjectIdentifier")).toBe("objectidentifier");
    expect(mibSyntaxToSetValueType("OctetString")).toBe("octetstring");
    expect(mibSyntaxToSetValueType("BITS")).toBe("octetstring");
    expect(mibSyntaxToSetValueType(undefined)).toBe("octetstring");
    expect(mibSyntaxToSetValueType("SomethingWeird")).toBe("octetstring");
  });
});

describe("constraint parsing", () => {
  it("parses integer ranges", () => {
    expect(parseIntegerRange("1..255")).toEqual({ min: 1, max: 255 });
    expect(parseIntegerRange(undefined)).toBeUndefined();
    expect(parseIntegerRange("SIZE (0..32)")).toBeUndefined();
  });

  it("parses negative minimums (full Integer32 range from the backend)", () => {
    // The backend reports a plain Integer32's effective range as the full
    // 32-bit span; dropping the minus sign yields min > max and rejects
    // every value.
    const range = parseIntegerRange("-2147483648..2147483647")!;
    expect(range).toEqual({ min: -2147483648, max: 2147483647 });
    expect(validateInteger("0", range)).toBeNull();
    expect(validateInteger("-5", range)).toBeNull();
    expect(validateInteger("2147483647", range)).toBeNull();
  });

  it("parses SIZE bounds", () => {
    expect(parseSizeBounds("SIZE (0..32)")).toEqual({ min: 0, max: 32 });
    expect(parseSizeBounds("1..255")).toBeUndefined();
  });
});

describe("validateInteger", () => {
  it("accepts in-range values with inclusive bounds", () => {
    const range = parseIntegerRange("1..255")!;
    expect(validateInteger("1", range)).toBeNull();
    expect(validateInteger("255", range)).toBeNull();
    expect(validateInteger("-5")).toBeNull();
  });

  it("rejects out-of-range and non-integer input", () => {
    const range = parseIntegerRange("1..255")!;
    expect(validateInteger("0", range)).toBe("Must be 1..255");
    expect(validateInteger("256", range)).toBe("Must be 1..255");
    expect(validateInteger("abc")).toBe("Enter an integer");
    expect(validateInteger("")).toBe("Enter an integer");
    expect(validateInteger("3000000000")).toMatch(/32-bit/);
  });
});

describe("validateUnsigned32", () => {
  it("accepts 0..u32 max and rejects negatives and overflow", () => {
    expect(validateUnsigned32("0")).toBeNull();
    expect(validateUnsigned32("4294967295")).toBeNull();
    expect(validateUnsigned32("-1")).toBe("Enter a non-negative integer");
    expect(validateUnsigned32("4294967296")).toBe("Must be 0..4294967295");
    expect(validateUnsigned32("10", { min: 20, max: 30 })).toBe(
      "Must be 20..30",
    );
  });
});

describe("validateCounter64", () => {
  it("accepts 0..u64 max (BigInt bound, no float precision loss)", () => {
    expect(validateCounter64("0")).toBeNull();
    expect(validateCounter64("18446744073709551615")).toBeNull();
    expect(validateCounter64("9007199254740993")).toBeNull(); // 2^53 + 1
  });

  it("rejects overflow and non-numeric input", () => {
    expect(validateCounter64("18446744073709551616")).toBe(
      "Must be 0..18446744073709551615",
    );
    expect(validateCounter64("-1")).toBe("Enter a non-negative integer");
    expect(validateCounter64("abc")).toBe("Enter a non-negative integer");
  });
});

describe("validateOctetString", () => {
  it("enforces SIZE bounds in text mode", () => {
    const size = parseSizeBounds("SIZE (1..4)")!;
    expect(validateOctetString("abcd", size)).toBeNull();
    expect(validateOctetString("", size)).toBe("SIZE must be 1..4 characters");
    expect(validateOctetString("abcde", size)).toBe("SIZE must be 1..4 characters");
  });

  it("enforces SIZE bounds in hex mode against byte count", () => {
    const size = parseSizeBounds("SIZE (2..3)")!;
    expect(validateOctetString("0a ff", size, true)).toBeNull();
    expect(validateOctetString("0a", size, true)).toBe("SIZE must be 2..3 octets");
  });
});

describe("validateIpv4", () => {
  it("accepts valid dotted quads", () => {
    expect(validateIpv4("192.168.1.1")).toBeNull();
    expect(validateIpv4("0.0.0.0")).toBeNull();
    expect(validateIpv4("255.255.255.255")).toBeNull();
  });

  it("rejects malformed addresses", () => {
    expect(validateIpv4("256.1.1.1")).toMatch(/out of range/);
    expect(validateIpv4("1.2.3")).toBe("Not a valid IPv4 address (a.b.c.d)");
    expect(validateIpv4("1.2.3.4.5")).toBe("Not a valid IPv4 address (a.b.c.d)");
    expect(validateIpv4("a.b.c.d")).toMatch(/Invalid octet/);
  });
});

describe("isValidOidString", () => {
  it("accepts dotted OIDs with at least two arcs", () => {
    expect(isValidOidString("1.3.6.1")).toBe(true);
    expect(isValidOidString("1.3.6.1.4.1.9.1.122")).toBe(true);
  });

  it("rejects single arcs, gaps, and non-numeric parts", () => {
    expect(isValidOidString("1")).toBe(false);
    expect(isValidOidString("1..3")).toBe(false);
    expect(isValidOidString("1.3.x")).toBe(false);
  });
});

describe("parseHexPairs", () => {
  it("parses byte pairs", () => {
    expect(parseHexPairs("0a ff 10")).toEqual([10, 255, 16]);
    expect(parseHexPairs("")).toEqual([]);
  });

  it("rejects odd digit counts and non-hex characters", () => {
    expect(parseHexPairs("abc")).toBeNull();
    expect(parseHexPairs("0a gg")).toBeNull();
  });
});

describe("bitsToBytes (RFC 1065)", () => {
  it("first bit is the MSB of the first octet", () => {
    expect(bitsToBytes([0])).toEqual([0x80]);
    expect(bitsToBytes([7])).toEqual([0x01]);
    expect(bitsToBytes([0, 7])).toEqual([0x81]);
  });

  it("spans octets", () => {
    expect(bitsToBytes([8])).toEqual([0x00, 0x80]);
    expect(bitsToBytes([0, 8, 15])).toEqual([0x80, 0x81]);
  });

  it("empty selection encodes to the empty byte string", () => {
    expect(bitsToBytes([])).toEqual([]);
  });
});

describe("bytesToBits (RFC 1065 inverse)", () => {
  it("decodes set bits in ascending position order", () => {
    expect(bytesToBits([0x80])).toEqual([0]);
    expect(bytesToBits([0x01])).toEqual([7]);
    expect(bytesToBits([0x81])).toEqual([0, 7]);
    expect(bytesToBits([0x00, 0x80])).toEqual([8]);
    expect(bytesToBits([0x80, 0x81])).toEqual([0, 8, 15]);
  });

  it("round-trips through bitsToBytes", () => {
    for (const positions of [[0], [3, 9], [0, 7, 8, 15], []]) {
      expect(bytesToBits(bitsToBytes(positions))).toEqual(positions);
    }
  });

  it("empty input decodes to no bits", () => {
    expect(bytesToBits([])).toEqual([]);
  });
});

describe("prefillFromValue", () => {
  const v = (x: SnmpValue) => x;

  it("converts numeric types to text", () => {
    expect(prefillFromValue(v({ Integer: -42 }), "Integer32")).toEqual({
      text: "-42",
      hex: false,
    });
    expect(prefillFromValue(v({ TruthValue: true }), "TruthValue")).toEqual({
      text: "1",
      hex: false,
    });
    expect(prefillFromValue(v({ Counter32: 7 }), "Counter32")).toEqual({
      text: "7",
      hex: false,
    });
    expect(prefillFromValue(v({ Unsigned: 99 }), "Gauge32")).toEqual({
      text: "99",
      hex: false,
    });
    expect(prefillFromValue(v({ TimeTicks: 100 }), "TimeTicks")).toEqual({
      text: "100",
      hex: false,
    });
  });

  it("converts string types to text", () => {
    expect(
      prefillFromValue(v({ IpAddress: "10.0.0.1" }), "IpAddress"),
    ).toEqual({ text: "10.0.0.1", hex: false });
    expect(
      prefillFromValue(v({ ObjectIdentifier: "1.3.6.1.4.1" }), "ObjectIdentifier"),
    ).toEqual({ text: "1.3.6.1.4.1", hex: false });
  });

  it("octet strings: printable as text, binary as hex pairs", () => {
    expect(
      prefillFromValue(v({ OctetString: [115, 99, 111, 117, 116] }), "OctetString"),
    ).toEqual({ text: "scout", hex: false });
    expect(
      prefillFromValue(v({ OctetString: [0x00, 0x1a, 0xff] }), "OctetString"),
    ).toEqual({ text: "00 1a ff", hex: true });
  });

  it("Null and undefined prefill empty", () => {
    expect(prefillFromValue(v("Null"), "Integer32")).toEqual({ text: "", hex: false });
    expect(prefillFromValue(undefined, "OctetString")).toEqual({ text: "", hex: false });
  });

  it("mismatched variants fall back to byte rendering", () => {
    expect(
      prefillFromValue(v({ Raw: { type_code: 0x44, data: [1, 2] } }), "Integer32"),
    ).toEqual({ text: "01 02", hex: true });
  });
});

describe("defvalPrefill", () => {
  it("uses numeric DEFVALs for integer types", () => {
    expect(
      defvalPrefill(details({ defaultValue: "5" }), "Integer32"),
    ).toEqual({ text: "5", hex: false });
  });

  it("resolves named enum DEFVALs to their value", () => {
    const d = details({
      syntaxType: "Integer32",
      defaultValue: "{ up }",
      enums: [
        { label: "down", value: 1 },
        { label: "up", value: 2 },
      ],
    });
    expect(defvalPrefill(d, "Integer32")).toEqual({ text: "2", hex: false });
  });

  it("strips quotes from string DEFVALs", () => {
    expect(
      defvalPrefill(details({ defaultValue: '"scout"' }), "OctetString"),
    ).toEqual({ text: "scout", hex: false });
  });

  it("rejects values that do not parse for the type", () => {
    expect(defvalPrefill(details({ defaultValue: "notanip" }), "IpAddress")).toBeNull();
    expect(
      defvalPrefill(details({ defaultValue: "{ unknownEnum }" }), "Integer32"),
    ).toBeNull();
    expect(defvalPrefill(null, "Integer32")).toBeNull();
  });
});

describe("pduErrorName", () => {
  it("extracts the RFC 3416 name from the warning message", () => {
    expect(
      pduErrorName({ kind: "pdu-error", message: "wrongValue (status 9) at varbind 0" }),
    ).toBe("wrongValue");
    expect(pduErrorName(undefined)).toBe("agent error");
  });
});
