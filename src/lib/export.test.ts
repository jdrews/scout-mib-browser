import { describe, it, expect } from "vitest";
import {
  valueDisplay,
  typeLabel,
  rawValueDisplay,
  inspectorValueOf,
  isByteValue,
  byteData,
  rawTypeCode,
} from "./export";

describe("valueDisplay", () => {
  it("renders scalars as before", () => {
    expect(valueDisplay({ Integer: -42 })).toBe("-42");
    // The Type column carries the counter/timeticks label — no suffix here.
    expect(valueDisplay({ Counter32: 7 })).toBe("7");
    expect(valueDisplay({ Counter64: 8 })).toBe("8");
    expect(valueDisplay({ TimeTicks: 100 })).toBe("100");
    expect(valueDisplay({ TruthValue: true })).toBe("true");
    expect(valueDisplay("Null")).toBe("NULL");
    expect(valueDisplay({ IpAddress: "10.0.0.1" })).toBe("10.0.0.1");
    expect(valueDisplay({ ObjectIdentifier: "1.3.6.1.4.1.9" })).toBe("1.3.6.1.4.1.9");
  });

  it("renders text octet strings quoted", () => {
    expect(valueDisplay({ OctetString: [...new TextEncoder().encode("Linux router")] })).toBe(
      '"Linux router"',
    );
  });

  it("recognizes MAC addresses (6 bytes)", () => {
    expect(valueDisplay({ OctetString: [0x00, 0x12, 0x79, 0x62, 0xf9, 0x40] })).toBe(
      "00:12:79:62:f9:40",
    );
  });

  it("recognizes IPv4 (4 bytes) but not 4-byte text", () => {
    expect(valueDisplay({ OctetString: [192, 168, 1, 1] })).toBe("192.168.1.1");
    expect(valueDisplay({ OctetString: [...new TextEncoder().encode("eth0")] })).toBe('"eth0"');
  });

  it("recognizes IPv6 (16 bytes)", () => {
    const bytes = [0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0x02, 0x12, 0x79, 0xff, 0xfe, 0x62, 0xf9, 0x40];
    expect(valueDisplay({ OctetString: bytes })).toBe("fe80::212:79ff:fe62:f940");
  });

  it("renders unrecognizable binary as space-separated hex", () => {
    // 5 bytes: too short for an address, not text.
    expect(valueDisplay({ OctetString: [0xde, 0xad, 0xbe, 0xef, 0x01] })).toBe(
      "de ad be ef 01",
    );
  });

  it("renders an empty octet string as an empty quoted string", () => {
    expect(valueDisplay({ OctetString: [] })).toBe('""');
  });

  it("renders Raw values like any other binary value", () => {
    // Unknown ASN.1 types go through the same presentation as OctetStrings:
    // space-separated hex for unrecognizable bytes...
    expect(valueDisplay({ Raw: { type_code: 0x44, data: [0xde, 0xad] } })).toBe("de ad");
    // ...pattern recognition for shaped ones...
    expect(
      valueDisplay({ Raw: { type_code: 0x44, data: [0x00, 0x12, 0x79, 0x62, 0xf9, 0x40] } }),
    ).toBe("00:12:79:62:f9:40");
    // ...and an empty quoted string when there is no data.
    expect(valueDisplay({ Raw: { type_code: 0x41, data: [] } })).toBe('""');
  });
});

describe("typeLabel", () => {
  it("labels every variant", () => {
    expect(typeLabel({ Integer: 1 })).toBe("INTEGER");
    expect(typeLabel({ OctetString: [] })).toBe("OCTET STRING");
    expect(typeLabel({ Raw: { type_code: 0, data: [] } })).toBe("RAW");
    expect(typeLabel("Null")).toBe("NULL");
  });
});

describe("rawValueDisplay", () => {
  it("shows scalars with their wire encoding", () => {
    expect(rawValueDisplay({ Integer: 123456789 })).toBe("0x075bcd15 (123456789)");
    expect(rawValueDisplay({ Integer: -1 })).toBe("-0x1 (-1)");
    expect(rawValueDisplay({ Unsigned: 255 })).toBe("0x000000ff (255)");
    expect(rawValueDisplay({ Counter32: 65536 })).toBe("0x00010000 (65536)");
    expect(rawValueDisplay({ Counter64: 4294967296 })).toBe("0x100000000 (4294967296)");
    expect(rawValueDisplay({ TimeTicks: 1234567 })).toBe("0x0012d687 (1234567)");
    expect(rawValueDisplay({ TruthValue: false })).toBe("false");
    expect(rawValueDisplay("Null")).toBe("NULL");
  });

  it("shows IP addresses with their packed bytes", () => {
    expect(rawValueDisplay({ IpAddress: "10.0.0.1" })).toBe("10.0.0.1 (0a 00 00 01)");
    // Non-IP strings pass through untouched.
    expect(rawValueDisplay({ IpAddress: "not-an-ip" })).toBe("not-an-ip");
  });

  it("shows OIDs with their BER bytes", () => {
    expect(rawValueDisplay({ ObjectIdentifier: "1.3.6.1.2.1" })).toBe(
      "1.3.6.1.2.1 [2b 06 01 02 01]",
    );
  });
});

describe("byte helpers", () => {
  it("isByteValue / byteData cover OctetString and Raw only", () => {
    expect(isByteValue({ OctetString: [1] })).toBe(true);
    expect(isByteValue({ Raw: { type_code: 0x04, data: [2] } })).toBe(true);
    expect(isByteValue({ Integer: 1 })).toBe(false);
    expect(isByteValue("Null")).toBe(false);

    expect(byteData({ OctetString: [1, 2] })).toEqual([1, 2]);
    expect(byteData({ Raw: { type_code: 0x04, data: [3] } })).toEqual([3]);
    expect(byteData({ Integer: 1 })).toEqual([]);
  });

  it("rawTypeCode is present only for Raw values", () => {
    expect(rawTypeCode({ Raw: { type_code: 0x82, data: [] } })).toBe(0x82);
    expect(rawTypeCode({ OctetString: [1] })).toBeUndefined();
    expect(rawTypeCode("Null")).toBeUndefined();
  });

  it("inspectorValueOf carries bytes for byte values", () => {
    const mac = inspectorValueOf({ OctetString: [0x00, 0x12, 0x79, 0x62, 0xf9, 0x40] });
    expect(mac.text).toBe("00:12:79:62:f9:40");
    expect(mac.typeLabel).toBe("OCTET STRING");
    expect(mac.bytes).toEqual([0x00, 0x12, 0x79, 0x62, 0xf9, 0x40]);

    const raw = inspectorValueOf({ Raw: { type_code: 0x04, data: [0xde] } });
    expect(raw.bytes).toEqual([0xde]);
    expect(raw.typeCode).toBe(0x04);

    const int = inspectorValueOf({ Integer: 5 });
    expect(int.bytes).toBeUndefined();
    expect(int.typeCode).toBeUndefined();
  });
});
