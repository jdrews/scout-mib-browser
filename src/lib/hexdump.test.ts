import { describe, it, expect } from "vitest";
import {
  hexDumpLines,
  hexPairs,
  looksLikeText,
  decodeText,
  formatMac,
  formatIpv4,
  formatIpv6,
  interpretBytes,
  interpretationLabel,
  asn1TypeCodeText,
  ipv4ToBytes,
  oidBerBytes,
  BYTES_PER_ROW,
} from "./hexdump";

describe("hexDumpLines", () => {
  it("returns no rows for an empty byte stream", () => {
    expect(hexDumpLines([])).toEqual([]);
  });

  it("renders one row per 16 bytes with offset, hex and ascii", () => {
    const bytes = Array.from({ length: 16 }, (_, i) => 0x30 + i); // "0".."F"
    const rows = hexDumpLines(bytes);
    expect(rows).toHaveLength(1);
    expect(rows[0].offset).toBe("0000");
    expect(rows[0].hex).toBe("30 31 32 33 34 35 36 37 38 39 3a 3b 3c 3d 3e 3f");
    expect(rows[0].ascii).toBe("0123456789:;<=>?");
  });

  it("continues offsets on subsequent rows and pads the last row", () => {
    const bytes = Array.from({ length: 20 }, (_, i) => i);
    const rows = hexDumpLines(bytes);
    expect(rows).toHaveLength(2);
    expect(rows[1].offset).toBe("0010");
    expect(rows[1].hex.split(" ")).toHaveLength(4);
    expect(rows[1].ascii).toHaveLength(4);
  });

  it("shows non-printable bytes as dots in the ascii column", () => {
    const rows = hexDumpLines([0x00, 0x01, 0x41, 0xff]);
    expect(rows[0].hex).toBe("00 01 41 ff");
    expect(rows[0].ascii).toBe("..A.");
  });

  it("uses four-digit hex offsets past 0xffff", () => {
    const rows = hexDumpLines(new Array(BYTES_PER_ROW * 4096 + 1).fill(0x41));
    expect(rows[rows.length - 1].offset).toBe("10000");
  });
});

describe("hexPairs", () => {
  it("joins bytes as space-separated lowercase hex", () => {
    expect(hexPairs([0xde, 0xad, 0xbe, 0xef])).toBe("de ad be ef");
    expect(hexPairs([])).toBe("");
  });
});

describe("looksLikeText", () => {
  it("rejects an empty stream", () => {
    expect(looksLikeText([])).toBe(false);
  });

  it("accepts plain ASCII text", () => {
    expect(looksLikeText([...new TextEncoder().encode("Linux cray 2.6.21.5-smp")])).toBe(true);
  });

  it("accepts text with whitespace and newlines", () => {
    expect(looksLikeText([...new TextEncoder().encode("a\tb\nc\rd")])).toBe(true);
  });

  it("tolerates up to 10% non-printable bytes", () => {
    const bytes = new Array(9).fill(0x41);
    bytes.push(0x01); // 9/10 = 0.9 is still text
    expect(looksLikeText(bytes)).toBe(true);
    bytes.push(0x02); // 9/11 < 0.9 and not valid UTF-8 either
    expect(looksLikeText(bytes)).toBe(false);
  });

  it("rejects dense binary", () => {
    expect(looksLikeText([0x00, 0x12, 0x79, 0x62, 0xf9, 0x40])).toBe(false);
  });

  it("accepts valid multibyte UTF-8 text", () => {
    expect(looksLikeText([...new TextEncoder().encode("café")])).toBe(true);
  });

  it("rejects short all-multibyte blobs that happen to be valid UTF-8", () => {
    // [0xde, 0xad] decodes as one exotic character — binary, not text.
    expect(looksLikeText([0xde, 0xad])).toBe(false);
    expect(looksLikeText([...new TextEncoder().encode("€")])).toBe(false);
  });

  it("rejects invalid UTF-8 with control bytes", () => {
    expect(looksLikeText([0xff, 0xfe, 0x01])).toBe(false);
  });
});

describe("decodeText", () => {
  it("decodes UTF-8", () => {
    expect(decodeText([...new TextEncoder().encode("café")])).toBe("café");
  });
});

describe("formatMac", () => {
  it("formats six bytes as colon-separated lowercase hex", () => {
    expect(formatMac([0x00, 0x12, 0x79, 0x62, 0xf9, 0x40])).toBe("00:12:79:62:f9:40");
  });
});

describe("formatIpv4", () => {
  it("formats four bytes as dotted decimal", () => {
    expect(formatIpv4([192, 168, 1, 1])).toBe("192.168.1.1");
  });
});

/** Expands a textual IPv6 address (including "::") to its 16 bytes. */
function v6(s: string): number[] {
  const [head = "", tail = ""] = s.split("::");
  const h = head ? head.split(":").map((g) => parseInt(g, 16)) : [];
  const t = tail ? tail.split(":").map((g) => parseInt(g, 16)) : [];
  const pad = new Array(8 - h.length - t.length).fill(0);
  return [...h, ...pad, ...t].flatMap((n) => [(n >> 8) & 0xff, n & 0xff]);
}

describe("formatIpv6", () => {

  it("compresses the longest zero run to ::", () => {
    expect(formatIpv6(v6("fe80::212:79ff:fe62:f940"))).toBe("fe80::212:79ff:fe62:f940");
  });

  it("renders ::1 and the unspecified address", () => {
    expect(formatIpv6(v6("::1"))).toBe("::1");
    expect(formatIpv6(new Array(16).fill(0))).toBe("::");
  });

  it("does not compress a single zero group (RFC 5952)", () => {
    expect(formatIpv6(v6("1:0:2:3:4:5:6:7"))).toBe("1:0:2:3:4:5:6:7");
  });

  it("compresses the leftmost run on ties", () => {
    expect(formatIpv6(v6("1:0:0:2:0:0:3:4"))).toBe("1::2:0:0:3:4");
  });

  it("drops leading zeros in each group", () => {
    expect(formatIpv6(v6("2001:db8:0:0:8:800:200c:41da"))).toBe("2001:db8::8:800:200c:41da");
  });
});

describe("interpretBytes", () => {
  it("recognizes a 6-byte MAC address", () => {
    expect(interpretBytes([0x00, 0x12, 0x79, 0x62, 0xf9, 0x40])).toEqual({
      kind: "mac",
      text: "00:12:79:62:f9:40",
    });
  });

  it("recognizes a 4-byte IPv4 address", () => {
    expect(interpretBytes([192, 168, 1, 1])).toEqual({ kind: "ipv4", text: "192.168.1.1" });
  });

  it("recognizes a 16-byte IPv6 address", () => {
    const interp = interpretBytes(v6("fe80::212:79ff:fe62:f940"));
    expect(interp).toEqual({ kind: "ipv6", text: "fe80::212:79ff:fe62:f940" });
  });

  it("prefers text over length-based matches", () => {
    // 4 bytes of ASCII is a short string (e.g. ifDescr "eth0"), not an IP.
    expect(interpretBytes([...new TextEncoder().encode("eth0")])).toEqual({
      kind: "text",
      text: "eth0",
    });
  });

  it("decodes printable streams as text", () => {
    expect(interpretBytes([...new TextEncoder().encode("Linux router")])).toEqual({
      kind: "text",
      text: "Linux router",
    });
  });

  it("returns null for unrecognizable binary and empty input", () => {
    expect(interpretBytes([0x00, 0x12, 0x79, 0x62, 0xf9, 0x40, 0x01])).toBeNull();
    expect(interpretBytes([])).toBeNull();
  });
});

describe("interpretationLabel", () => {
  it("labels each recognized kind", () => {
    expect(interpretationLabel("mac")).toBe("MAC address");
    expect(interpretationLabel("ipv4")).toBe("IPv4 address");
    expect(interpretationLabel("ipv6")).toBe("IPv6 address");
    expect(interpretationLabel("text")).toBe("Text");
  });
});

describe("asn1TypeCodeText", () => {
  it("names known BER type codes", () => {
    expect(asn1TypeCodeText(0x04)).toBe("0x04 (OCTET STRING)");
    expect(asn1TypeCodeText(0x82)).toBe("0x82 (endOfMibView)");
  });

  it("falls back to the bare hex value for unknown codes", () => {
    expect(asn1TypeCodeText(0x41)).toBe("0x41");
  });
});

describe("ipv4ToBytes", () => {
  it("parses dotted decimal addresses", () => {
    expect(ipv4ToBytes("192.168.1.1")).toEqual([192, 168, 1, 1]);
  });

  it("rejects non-address strings", () => {
    expect(ipv4ToBytes("host.name")).toBeNull();
    expect(ipv4ToBytes("1.2.3.256")).toBeNull();
    expect(ipv4ToBytes("1.2.3")).toBeNull();
  });
});

describe("oidBerBytes", () => {
  it("encodes a standard OID (matches the wire bytes in SNMP captures)", () => {
    // 06 09 2b 06 01 02 01 02 02 01 02 — real snmp2 GetBulk capture.
    expect(oidBerBytes("1.3.6.1.2.1.2.2.1.2")).toEqual([43, 6, 1, 2, 1, 2, 2, 1, 2]);
  });

  it("encodes sub-identifiers >= 128 in base-128 with continuation bits", () => {
    // 99997 = 0x86 0x8D 0x1D (continuation on every byte but the last).
    expect(oidBerBytes("1.3.6.1.99997")).toEqual([43, 6, 1, 0x86, 0x8d, 0x1d]);
  });

  it("encodes a zero sub-identifier as a single zero byte", () => {
    // The trailing .0 is a real (non-leading) arc and must encode as 0x00.
    expect(oidBerBytes("1.3.6.1.0")).toEqual([43, 6, 1, 0]);
  });

  it("returns empty for unparseable input", () => {
    expect(oidBerBytes("not.an.oid")).toEqual([]);
    expect(oidBerBytes("1")).toEqual([]);
  });
});
