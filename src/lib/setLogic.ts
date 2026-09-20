import type { MibNodeDetails, SnmpValue, SnmpWarning } from "./types";

// ── Writability ──────────────────────────────────────────────────────────────

/** MAX-ACCESS values that permit writes. */
const WRITABLE_ACCESS = new Set(["read-write", "read-create"]);
/** MAX-ACCESS values known not to permit writes. */
const NON_WRITABLE_ACCESS = new Set([
  "read-only",
  "not-accessible",
  "accessible-for-notify",
]);

/** Settable unless we *know* it isn't: unknown (undefined) access is *not*
 * false — the MIB claim is a schema hint, and the agent's PDU error is the
 * safety net for "MIB says writable but agent refuses". */
export function isWritable(access: string | undefined): boolean {
  return !NON_WRITABLE_ACCESS.has(access ?? "");
}

/** True only when the MIB confirms writability — the bar for at-a-glance
 * affordances (results pencil, inspector Set button). */
export function isConfirmedWritable(access: string | undefined): boolean {
  return WRITABLE_ACCESS.has(access ?? "");
}

/** Status-bar message when Set must be refused; null = proceed. Unknown nodes
 * (details === null) always proceed — they land in the dialog's type picker. */
export function setGuard(oid: string, details: MibNodeDetails | null): string | null {
  if (!details) return null;
  const name = details.name || oid;
  if (details.isTable || details.syntaxType === "TABLE") {
    return `${name} is a table — Set targets a column instance`;
  }
  if (details.syntaxType === "ROW") {
    return `${name} is a row entry — Set targets a column instance`;
  }
  if (!isWritable(details.access)) {
    return `${name} is ${details.access}`;
  }
  return null;
}

/** Details that drive the Set dialog. Longest-prefix resolution of an unknown
 * instance OID lands on an ancestor ObjectIdentifier subtree (e.g. `enterprises`)
 * — that is not the object being set, so return null and let the dialog offer
 * its type-picker fallback instead of editing the parent's type. */
export function setDetailsFor(
  oid: string,
  details: MibNodeDetails | null,
): MibNodeDetails | null {
  if (!details) return null;
  if (details.oid !== oid && details.syntaxType === "ObjectIdentifier") return null;
  return details;
}

/** Mirrors the backend's `scalar_instance_oid`: when the OID exactly matches a
 * scalar MIB node (not Table/TableRow/ObjectIdentifier), append `.0`. Instance
 * OIDs (which resolve to their base node, so `details.oid !== oid`) pass
 * through untouched. */
export function effectiveSetOid(oid: string, details: MibNodeDetails | null): string {
  if (!details || details.oid !== oid) return oid;
  const t = details.syntaxType;
  if (t === "TABLE" || t === "ROW" || t === "ObjectIdentifier") return oid;
  return `${oid}.0`;
}

// ── Wire type mapping ────────────────────────────────────────────────────────

/** The eight wire value types the backend's `parse_set_value` accepts. */
export type SetValueType =
  | "integer"
  | "octetstring"
  | "gauge32"
  | "counter32"
  | "counter64"
  | "ipaddress"
  | "timeticks"
  | "objectidentifier";

/** Single source of truth: MIB SYNTAX label → wire value type. OctetString,
 * BITS, and anything unrecognized all send as octet strings (BITS as encoded
 * bytes). */
export function mibSyntaxToSetValueType(syntax: string | undefined): SetValueType {
  switch (syntax) {
    case "Integer32":
    case "TruthValue":
      return "integer";
    case "Counter32":
      return "counter32";
    case "Counter64":
      return "counter64";
    case "Gauge32":
    case "Unsigned32":
      return "gauge32";
    case "TimeTicks":
      return "timeticks";
    case "IpAddress":
      return "ipaddress";
    case "ObjectIdentifier":
      return "objectidentifier";
    default:
      return "octetstring";
  }
}

// ── Constraint parsing ───────────────────────────────────────────────────────

export interface ValueRange {
  min: number;
  max: number;
}

/** Extracts an integer range from constraints text, e.g. `"1..255"`. A SIZE
 * clause is not an integer range and yields undefined. */
export function parseIntegerRange(
  constraints: string | undefined,
): ValueRange | undefined {
  if (!constraints || /SIZE/i.test(constraints)) return undefined;
  // The minimum may be negative (e.g. the full Integer32 range the backend
  // reports as "-2147483648..2147483647").
  const m = /(-?\d+)\.\.(\d+)/.exec(constraints);
  if (!m) return undefined;
  return { min: Number(m[1]), max: Number(m[2]) };
}

/** Extracts SIZE bounds from constraints text, e.g. `"SIZE (0..32)"`. */
export function parseSizeBounds(
  constraints: string | undefined,
): ValueRange | undefined {
  if (!constraints) return undefined;
  const m = /SIZE\s*\((\d+)\.\.(\d+)\)/i.exec(constraints);
  if (!m) return undefined;
  return { min: Number(m[1]), max: Number(m[2]) };
}

// ── Per-type validation (error message, or null when valid) ─────────────────

export function validateInteger(text: string, range?: ValueRange): string | null {
  const t = text.trim();
  if (!/^[+-]?\d+$/.test(t)) return "Enter an integer";
  const v = Number(t);
  if (v < -2147483648 || v > 2147483647) {
    return "Must be a 32-bit integer (-2147483648..2147483647)";
  }
  if (range && (v < range.min || v > range.max)) {
    return `Must be ${range.min}..${range.max}`;
  }
  return null;
}

export function validateUnsigned32(text: string, range?: ValueRange): string | null {
  const t = text.trim();
  if (!/^\d+$/.test(t)) return "Enter a non-negative integer";
  const v = Number(t);
  if (v > 4294967295) return "Must be 0..4294967295";
  if (range && (v < range.min || v > range.max)) {
    return `Must be ${range.min}..${range.max}`;
  }
  return null;
}

/** u64 bound checked with BigInt — Number loses precision past 2^53. */
export function validateCounter64(text: string): string | null {
  const t = text.trim();
  if (!/^\d+$/.test(t)) return "Enter a non-negative integer";
  if (BigInt(t) > 18446744073709551615n) return "Must be 0..18446744073709551615";
  return null;
}

export function validateOctetString(
  text: string,
  size?: ValueRange,
  hexMode = false,
): string | null {
  if (hexMode) {
    const bytes = parseHexPairs(text);
    if (!bytes) return "Enter space-separated byte pairs (e.g. 0a ff)";
    if (size && (bytes.length < size.min || bytes.length > size.max)) {
      return `SIZE must be ${size.min}..${size.max} octets`;
    }
    return null;
  }
  if (size && (text.length < size.min || text.length > size.max)) {
    return `SIZE must be ${size.min}..${size.max} characters`;
  }
  return null;
}

export function validateIpv4(text: string): string | null {
  const t = text.trim();
  const parts = t.split(".");
  if (parts.length !== 4) return "Not a valid IPv4 address (a.b.c.d)";
  for (const p of parts) {
    if (!/^\d{1,3}$/.test(p)) return `Invalid octet "${p}"`;
    if (Number(p) > 255) return `Octet ${p} out of range (0-255)`;
  }
  return null;
}

/** Dotted-decimal OID: at least two arcs, each a non-negative integer. */
export function isValidOidString(text: string): boolean {
  const t = text.trim();
  if (!/^\d+(\.\d+)*$/.test(t)) return false;
  return t.split(".").length >= 2;
}

/** Parses space-separated hex byte pairs. Returns null for non-hex characters
 * or an odd digit count; empty input is the (valid) empty byte string. */
export function parseHexPairs(text: string): number[] | null {
  const t = text.trim();
  if (!t) return [];
  if (!/^[0-9a-fA-F ]+$/.test(t)) return null;
  const digits = t.replace(/ /g, "");
  if (digits.length % 2 !== 0) return null;
  const bytes: number[] = [];
  for (let i = 0; i < digits.length; i += 2) {
    bytes.push(parseInt(digits.slice(i, i + 2), 16));
  }
  return bytes;
}

/** Encodes checked bit positions to octet-string bytes. RFC 1065: the first
 * declared bit is the most significant bit of the first octet. */
export function bitsToBytes(checkedPositions: number[]): number[] {
  if (checkedPositions.length === 0) return [];
  const max = Math.max(...checkedPositions);
  const bytes = new Array<number>(Math.floor(max / 8) + 1).fill(0);
  for (const pos of checkedPositions) {
    bytes[Math.floor(pos / 8)] |= 0x80 >> (pos % 8);
  }
  return bytes;
}

/** Inverse of {@link bitsToBytes}: the set bit positions in encoded octet
 * string bytes, in ascending order. */
export function bytesToBits(bytes: number[]): number[] {
  const positions: number[] = [];
  for (let i = 0; i < bytes.length; i++) {
    for (let j = 0; j < 8; j++) {
      if (bytes[i] & (0x80 >> j)) positions.push(i * 8 + j);
    }
  }
  return positions;
}

// ── Prefill ──────────────────────────────────────────────────────────────────

/** Editor prefill for one value: text plus a hex-mode flag (OctetString only). */
export interface Prefill {
  text: string;
  hex: boolean;
}

function isPrintableText(bytes: number[]): boolean {
  if (bytes.length === 0) return false;
  return bytes.every((b) => b === 9 || b === 10 || b === 13 || (b >= 32 && b <= 126));
}

function octetPrefill(bytes: number[]): Prefill {
  if (isPrintableText(bytes)) {
    return { text: new TextDecoder().decode(new Uint8Array(bytes)), hex: false };
  }
  return {
    text: bytes.map((b) => b.toString(16).padStart(2, "0")).join(" "),
    hex: true,
  };
}

/** Converts a live SnmpValue to editor text for the node's type. Mismatched
 * variants (e.g. Raw at a typed node) fall back to byte rendering when bytes
 * exist, else empty. */
export function prefillFromValue(
  value: SnmpValue | undefined,
  syntax: string | undefined,
): Prefill {
  if (!value || value === "Null") return { text: "", hex: false };
  const num = (n: number): Prefill => ({ text: String(n), hex: false });
  switch (mibSyntaxToSetValueType(syntax)) {
    case "integer":
      if ("Integer" in value) return num(value.Integer);
      if ("TruthValue" in value) return num(value.TruthValue ? 1 : 0);
      break;
    case "counter32":
      if ("Counter32" in value) return num(value.Counter32);
      break;
    case "counter64":
      if ("Counter64" in value) return num(value.Counter64);
      break;
    case "gauge32":
      if ("Unsigned" in value) return num(value.Unsigned);
      break;
    case "timeticks":
      if ("TimeTicks" in value) return num(value.TimeTicks);
      break;
    case "ipaddress":
      if ("IpAddress" in value) return { text: value.IpAddress, hex: false };
      break;
    case "objectidentifier":
      if ("ObjectIdentifier" in value) return { text: value.ObjectIdentifier, hex: false };
      break;
    case "octetstring":
      if ("OctetString" in value) return octetPrefill(value.OctetString);
      break;
  }
  if ("OctetString" in value) return octetPrefill(value.OctetString);
  if ("Raw" in value && value.Raw.data.length > 0) return octetPrefill(value.Raw.data);
  return { text: "", hex: false };
}

/** DEFVAL as written in the MIB source, when it parses cleanly for the type. */
export function defvalPrefill(
  details: MibNodeDetails | null,
  syntax: string | undefined,
): Prefill | null {
  if (!details?.defaultValue) return null;
  const raw = details.defaultValue.trim();
  switch (mibSyntaxToSetValueType(syntax)) {
    case "integer": {
      // Named enum value, e.g. DEFVAL { up } → its numeric value.
      if (raw.startsWith("{") && raw.endsWith("}")) {
        const hit = details.enums?.find((e) => e.label === raw.slice(1, -1).trim());
        return hit ? { text: String(hit.value), hex: false } : null;
      }
      return /^[+-]?\d+$/.test(raw) ? { text: raw, hex: false } : null;
    }
    case "counter32":
    case "counter64":
    case "gauge32":
    case "timeticks":
      return /^\d+$/.test(raw) ? { text: raw, hex: false } : null;
    case "ipaddress":
      return validateIpv4(raw) === null ? { text: raw, hex: false } : null;
    case "objectidentifier":
      // DEFVAL is usually a MIB object name, not a dotted OID — only clean OIDs prefill.
      return isValidOidString(raw) ? { text: raw, hex: false } : null;
    case "octetstring": {
      const unquoted =
        raw.length >= 2 && raw.startsWith('"') && raw.endsWith('"')
          ? raw.slice(1, -1)
          : raw;
      return { text: unquoted, hex: false };
    }
  }
}

// ── Agent error surfacing ────────────────────────────────────────────────────

/** Extracts the RFC 3416 name from a pdu-error warning message, e.g.
 * `"wrongValue (status 9) at varbind 0"` → `"wrongValue"`. */
export function pduErrorName(warning: SnmpWarning | undefined): string {
  if (!warning) return "agent error";
  const m = /^(\w+) \(/.exec(warning.message);
  return m ? m[1] : warning.kind;
}
