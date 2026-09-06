/** One row of a Wireshark-style dump: byte offset, hex bytes, ASCII column. */
export interface HexRow {
  /** Zero-padded hex offset of the first byte in the row, e.g. "0010". */
  offset: string;
  /** Space-separated lowercase hex byte pairs, e.g. "48 65 6c 6c 6f". */
  hex: string;
  /** ASCII rendering; non-printable bytes shown as ".". */
  ascii: string;
}

export const BYTES_PER_ROW = 16;

/** Strictly printable for the dump's ASCII column (space..tilde). */
function isDumpPrintable(b: number): boolean {
  return b >= 0x20 && b <= 0x7e;
}

/** True when the byte is acceptable in a text value (printable + whitespace). */
export function isTextByte(b: number): boolean {
  return isDumpPrintable(b) || b === 0x09 || b === 0x0a || b === 0x0d;
}

/** Wireshark-style dump: 16 bytes per row, offset | hex | ascii. */
export function hexDumpLines(bytes: number[]): HexRow[] {
  const rows: HexRow[] = [];
  for (let i = 0; i < bytes.length; i += BYTES_PER_ROW) {
    const chunk = bytes.slice(i, i + BYTES_PER_ROW);
    rows.push({
      offset: i.toString(16).padStart(4, "0"),
      hex: chunk.map((b) => b.toString(16).padStart(2, "0")).join(" "),
      ascii: chunk.map((b) => (isDumpPrintable(b) ? String.fromCharCode(b) : ".")).join(""),
    });
  }
  return rows;
}

/** Space-separated lowercase hex byte pairs ("de ad be ef"). */
export function hexPairs(bytes: number[]): string {
  return bytes.map((b) => b.toString(16).padStart(2, "0")).join(" ");
}

/** Heuristic: is this byte stream text rather than binary?
 *  Mostly printable ASCII (>=90%), or valid UTF-8 with no C0 control bytes. */
export function looksLikeText(bytes: number[]): boolean {
  if (bytes.length === 0) return false;
  let printable = 0;
  for (const b of bytes) {
    if (isTextByte(b)) printable++;
  }
  if (printable / bytes.length >= 0.9) return true;
  // Multibyte UTF-8 text: a strict decode that succeeds, no C0 control bytes
  // (in valid UTF-8 every byte below 0x20 is an ASCII control), and at least
  // four characters — short all-multibyte blobs like [0xde, 0xad] are valid
  // UTF-8 but almost certainly binary.
  let decoded: string;
  try {
    decoded = new TextDecoder("utf-8", { fatal: true }).decode(new Uint8Array(bytes));
  } catch {
    return false;
  }
  if ([...decoded].length < 4) return false;
  for (const b of bytes) {
    if (b < 0x20 && b !== 0x09 && b !== 0x0a && b !== 0x0d) return false;
  }
  return true;
}

/** Decodes text bytes as UTF-8 (invalid sequences become U+FFFD). */
export function decodeText(bytes: number[]): string {
  return new TextDecoder("utf-8").decode(new Uint8Array(bytes));
}

/** MAC address, e.g. "00:12:79:62:f9:40". */
export function formatMac(bytes: number[]): string {
  return bytes.map((b) => b.toString(16).padStart(2, "0")).join(":");
}

/** IPv4 dotted decimal, e.g. "192.168.1.1". */
export function formatIpv4(bytes: number[]): string {
  return bytes.join(".");
}

/** IPv6 compressed form per RFC 5952 (longest zero run becomes "::",
 *  leftmost on ties, runs shorter than two groups are not compressed). */
export function formatIpv6(bytes: number[]): string {
  const groups: number[] = [];
  for (let i = 0; i < 16; i += 2) {
    groups.push((bytes[i] << 8) | bytes[i + 1]);
  }
  let bestStart = -1;
  let bestLen = 0;
  let curStart = -1;
  let curLen = 0;
  for (let i = 0; i < groups.length; i++) {
    if (groups[i] === 0) {
      if (curStart === -1) curStart = i;
      curLen++;
      if (curLen > bestLen) {
        bestLen = curLen;
        bestStart = curStart;
      }
    } else {
      curStart = -1;
      curLen = 0;
    }
  }
  const fmt = (g: number) => g.toString(16);
  if (bestLen >= 2) {
    const head = groups.slice(0, bestStart).map(fmt).join(":");
    const tail = groups.slice(bestStart + bestLen).map(fmt).join(":");
    return `${head}::${tail}`;
  }
  return groups.map(fmt).join(":");
}

/** A recognized interpretation of a byte stream. */
export type ByteInterpretation =
  | { kind: "mac"; text: string }
  | { kind: "ipv4"; text: string }
  | { kind: "ipv6"; text: string }
  | { kind: "text"; text: string };

/** Recognizes well-known binary patterns in a byte stream. Text wins over
 *  length-based matches (a 4-byte OctetString holding "eth0" is text, not an
 *  IP address). Returns null when nothing matches — the caller falls back to
 *  a hex presentation. */
export function interpretBytes(bytes: number[]): ByteInterpretation | null {
  if (bytes.length === 0) return null;
  if (looksLikeText(bytes)) return { kind: "text", text: decodeText(bytes) };
  switch (bytes.length) {
    case 4:
      return { kind: "ipv4", text: formatIpv4(bytes) };
    case 6:
      return { kind: "mac", text: formatMac(bytes) };
    case 16:
      return { kind: "ipv6", text: formatIpv6(bytes) };
    default:
      return null;
  }
}

/** Human label for a recognized interpretation, e.g. "MAC address". */
export function interpretationLabel(kind: ByteInterpretation["kind"]): string {
  switch (kind) {
    case "mac":
      return "MAC address";
    case "ipv4":
      return "IPv4 address";
    case "ipv6":
      return "IPv6 address";
    case "text":
      return "Text";
  }
}

/** Packed 4-byte form of a dotted-decimal IPv4 address (null if not one). */
export function ipv4ToBytes(ip: string): number[] | null {
  const parts = ip.split(".");
  if (parts.length !== 4) return null;
  const bytes = parts.map((p) => Number(p));
  if (bytes.some((b) => !Number.isInteger(b) || b < 0 || b > 255)) return null;
  return bytes;
}

/** BER content bytes of a dotted-decimal OID (base-128 sub-identifiers),
 *  e.g. "1.3.6.1.2.1" -> [43, 6, 1, 2, 1]. Empty when unparseable. */
export function oidBerBytes(oid: string): number[] {
  const parts = oid.split(".").map((s) => (/^\d+$/.test(s) ? BigInt(s) : null));
  if (parts.length < 2 || parts.some((p) => p === null)) return [];
  const bytes: number[] = [Number(parts[0]) * 40 + Number(parts[1])];
  for (let i = 2; i < parts.length; i++) {
    let val = parts[i]!;
    // Base-128 groups, lowest first; the continuation bit is set on every
    // group except the lowest (the last byte on the wire).
    const groups: number[] = [];
    do {
      groups.push(Number(val & 0x7fn));
      val >>= 7n;
    } while (val > 0n);
    for (let g = groups.length - 1; g >= 0; g--) {
      bytes.push(g === 0 ? groups[g] : groups[g] | 0x80);
    }
  }
  return bytes;
}

const ASN1_TYPE_NAMES: Record<number, string> = {
  0x02: "INTEGER",
  0x04: "OCTET STRING",
  0x05: "NULL",
  0x06: "OBJECT IDENTIFIER",
  0x30: "SEQUENCE",
  0x80: "noSuchObject",
  0x81: "noSuchInstance",
  0x82: "endOfMibView",
};

/** Name for a BER type code when known, else its hex value. */
export function asn1TypeLabel(code: number): string {
  return ASN1_TYPE_NAMES[code] ?? `0x${code.toString(16).padStart(2, "0")}`;
}

/** "0x04 (OCTET STRING)" for known codes, plain "0x41" for unknown ones. */
export function asn1TypeCodeText(code: number): string {
  const hex = `0x${code.toString(16).padStart(2, "0")}`;
  const name = ASN1_TYPE_NAMES[code];
  return name ? `${hex} (${name})` : hex;
}
