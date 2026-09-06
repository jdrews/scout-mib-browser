import type {
  VariableBinding,
  SnmpValue,
  SnmpWarning,
  TargetConfig,
  TableIndexColumn,
  TableRowData,
  TableResult,
  InspectorValue,
} from "./types";
import { interpretBytes, hexPairs, ipv4ToBytes, oidBerBytes } from "./hexdump";

export type ExportFormat = "tsv" | "json" | "csv";

/** Enriched row ready for export. */
export interface ExportRow {
  oid: string;
  name: string;
  type: string;
  value: string;
}

/** Metadata envelope for JSON export. */
export interface JsonExportEnvelope {
  target: {
    host: string;
    port: number;
    version: string;
  };
  timestamp: string;
  root_oid: string | null;
  entries: ExportRow[];
  errors: SnmpWarning[];
}

/** Display for a byte payload: recognized patterns (MAC, IP, text) render
 *  nicely; anything else is binary and falls back to space-separated hex. */
function displayBytes(bytes: number[]): string {
  if (bytes.length === 0) return '""';
  const interp = interpretBytes(bytes);
  if (!interp) return hexPairs(bytes);
  return interp.kind === "text" ? `"${interp.text}"` : interp.text;
}

/** Display string for a SnmpValue (matches ResultsPane logic). The type is
 *  never repeated in the value — the Type column carries it. */
export function valueDisplay(v: SnmpValue): string {
  if (v === "Null") return "NULL";
  if (typeof v !== "object" || v === null) return String(v);
  if ("Integer" in v) return String(v.Integer);
  if ("Unsigned" in v) return String(v.Unsigned);
  if ("Counter32" in v) return String(v.Counter32);
  if ("Counter64" in v) return String(v.Counter64);
  if ("OctetString" in v) return displayBytes(v.OctetString);
  if ("ObjectIdentifier" in v) return v.ObjectIdentifier;
  if ("IpAddress" in v) return v.IpAddress;
  if ("TimeTicks" in v) return String(v.TimeTicks);
  if ("TruthValue" in v) return v.TruthValue ? "true" : "false";
  // Unknown ASN.1 type — present the bytes like any other binary value; the
  // type code stays visible in raw mode and the inspector.
  if ("Raw" in v) return displayBytes(v.Raw.data);
  return String(v);
}

/** Inspector payload for a live value: display text plus the raw bytes when
 *  the value carries them (OctetString/Raw), so the inspector can show a
 *  hex dump. */
export function inspectorValueOf(v: SnmpValue): InspectorValue {
  const base: InspectorValue = { text: valueDisplay(v), typeLabel: typeLabel(v) };
  if (typeof v === "object" && v !== null) {
    if ("OctetString" in v) return { ...base, bytes: v.OctetString };
    if ("Raw" in v) return { ...base, bytes: v.Raw.data, typeCode: v.Raw.type_code };
  }
  return base;
}

/** True when the value carries raw bytes (rendered as a hex dump in raw mode). */
export function isByteValue(v: SnmpValue): boolean {
  return typeof v === "object" && v !== null && ("OctetString" in v || "Raw" in v);
}

/** The byte payload of a byte-carrying value. */
export function byteData(v: SnmpValue): number[] {
  if (typeof v !== "object" || v === null) return [];
  if ("OctetString" in v) return v.OctetString;
  if ("Raw" in v) return v.Raw.data;
  return [];
}

/** The ASN.1 type code when the value is a Raw variant, else undefined. */
export function rawTypeCode(v: SnmpValue): number | undefined {
  return typeof v === "object" && v !== null && "Raw" in v ? v.Raw.type_code : undefined;
}

/** Raw-mode display for scalar values: the decoded value plus its wire
 *  representation (hex bytes / BER encoding). Byte-carrying values are
 *  rendered as a hex dump by the caller instead. */
export function rawValueDisplay(v: SnmpValue): string {
  if (v === "Null") return "NULL";
  if (typeof v !== "object" || v === null) return String(v);
  if ("Integer" in v) return `${rawIntHex(v.Integer)} (${v.Integer})`;
  if ("Unsigned" in v) return `0x${v.Unsigned.toString(16).padStart(8, "0")} (${v.Unsigned})`;
  if ("Counter32" in v) return `0x${v.Counter32.toString(16).padStart(8, "0")} (${v.Counter32})`;
  if ("Counter64" in v) return `0x${v.Counter64.toString(16)} (${v.Counter64})`;
  if ("TimeTicks" in v) return `0x${v.TimeTicks.toString(16).padStart(8, "0")} (${v.TimeTicks})`;
  if ("TruthValue" in v) return v.TruthValue ? "true" : "false";
  if ("IpAddress" in v) {
    const bytes = ipv4ToBytes(v.IpAddress);
    return bytes ? `${v.IpAddress} (${hexPairs(bytes)})` : v.IpAddress;
  }
  if ("ObjectIdentifier" in v) {
    const ber = oidBerBytes(v.ObjectIdentifier);
    return ber.length > 0 ? `${v.ObjectIdentifier} [${hexPairs(ber)}]` : v.ObjectIdentifier;
  }
  return valueDisplay(v);
}

function rawIntHex(n: number): string {
  if (n < 0) return `-0x${(-n).toString(16)}`;
  return `0x${n.toString(16).padStart(8, "0")}`;
}

/** Type label for a SnmpValue (matches ResultsPane logic). */
export function typeLabel(v: SnmpValue): string {
  if (v === "Null") return "NULL";
  if (typeof v !== "object" || v === null) return "UNKNOWN";
  if ("Integer" in v) return "INTEGER";
  if ("Unsigned" in v) return "UNSIGNED32";
  if ("Counter32" in v) return "COUNTER32";
  if ("Counter64" in v) return "COUNTER64";
  if ("OctetString" in v) return "OCTET STRING";
  if ("ObjectIdentifier" in v) return "OBJECT IDENTIFIER";
  if ("IpAddress" in v) return "IPADDRESS";
  if ("TimeTicks" in v) return "TIMETICKS";
  if ("TruthValue" in v) return "TRUTHVALUE";
  if ("Raw" in v) return "RAW";
  return "UNKNOWN";
}

/** Convert bindings to enriched export rows. */
export function bindingsToRows(
  bindings: VariableBinding[],
  nameMap: Map<string, string>,
): ExportRow[] {
  return bindings.map(b => ({
    oid: b.oid,
    name: nameMap.get(b.oid) || "",
    type: typeLabel(b.value),
    value: valueDisplay(b.value),
  }));
}

/** Generate a default filename for export. */
export function defaultFilename(
  target: TargetConfig,
  rootOid: string | null,
  format: ExportFormat,
): string {
  const host = target.host || "unknown";
  const shortOid = rootOid ? rootOid.replace(/\./g, "_") : "query";
  const ts = new Date()
    .toISOString()
    .replace(/[:.]/g, "")
    .replace(/T/, "_")
    .replace(/Z$/, "");
  return `${host}_${shortOid}_${ts}.${format}`;
}

/** Format rows as TSV: tab-delimited, no header, `oid\tname\ttype\thuman_value`. */
export function formatTSV(rows: ExportRow[]): string {
  return rows.map(r => [r.oid, r.name, r.type, r.value].join("\t")).join("\n");
}

/** Format as JSON with metadata envelope. */
export function formatJSON(
  target: TargetConfig,
  rootOid: string | null,
  rows: ExportRow[],
  warnings: SnmpWarning[] | undefined,
): string {
  const envelope: JsonExportEnvelope = {
    target: {
      host: target.host,
      port: target.port,
      version: target.version,
    },
    timestamp: new Date().toISOString(),
    root_oid: rootOid,
    entries: rows,
    errors: warnings || [],
  };
  return JSON.stringify(envelope, null, 2);
}

/** RFC 4180 CSV quoting for a single field. */
export function quoteCsvField(field: string): string {
  if (field.includes(",") || field.includes('"') || field.includes("\n") || field.includes("\r")) {
    return `"${field.replace(/"/g, '""')}"`;
  }
  return field;
}

/** Format rows as CSV with RFC 4180 quoting. */
export function formatCSV(rows: ExportRow[]): string {
  return rows.map(r => [r.oid, r.name, r.type, r.value].map(quoteCsvField).join(",")).join("\n");
}

// ── Grid (table result) export ───────────────────────────────────────────────

/** True when the result carries decoded index values for the given columns. */
function hasDecodedIndexes(result: TableResult, indexColumns: TableIndexColumn[]): boolean {
  return indexColumns.length > 0 && result.rows.some((r) => r.index_values.length > 0);
}

/** Grid export header: Instance + decoded index components + data columns. */
export function gridHeaderNames(
  result: TableResult,
  indexColumns: TableIndexColumn[],
  nameOf: (oid: string) => string,
): string[] {
  const names = ["Instance"];
  if (hasDecodedIndexes(result, indexColumns)) names.push(...indexColumns.map((c) => c.name));
  names.push(...result.columns.map(nameOf));
  return names;
}

/** One grid row as display strings, aligned with gridHeaderNames(). */
export function gridRowValues(
  row: TableRowData,
  result: TableResult,
  indexColumns: TableIndexColumn[],
): string[] {
  const vals = [row.instance_id];
  if (hasDecodedIndexes(result, indexColumns)) {
    for (const v of row.index_values) vals.push(v ?? "");
  }
  for (const colOid of result.columns) {
    const cell = row.cells[colOid];
    vals.push(cell?.value ? valueDisplay(cell.value.value) : "");
  }
  return vals;
}

/** Grid JSON: { table_oid, columns: [{oid, name}], rows: [...] }. */
export function gridToJson(
  result: TableResult,
  nameOf: (oid: string) => string,
): string {
  return JSON.stringify(
    {
      table_oid: result.table_oid,
      columns: result.columns.map((oid) => ({ oid, name: nameOf(oid) })),
      rows: result.rows.map((r) => ({
        instance_id: r.instance_id,
        cells: Object.fromEntries(
          Object.entries(r.cells).map(([k, c]) => [k, c.value ? valueDisplay(c.value.value) : null]),
        ),
      })),
    },
    null,
    2,
  );
}

/** Grid TSV/CSV: header + rows; CSV fields get RFC 4180 quoting. */
export function gridDelimited(
  result: TableResult,
  indexColumns: TableIndexColumn[],
  nameOf: (oid: string) => string,
  delimiter: string,
  quote: boolean,
): string {
  const lines = [gridHeaderNames(result, indexColumns, nameOf)];
  for (const row of result.rows) lines.push(gridRowValues(row, result, indexColumns));
  return lines
    .map((fields) => fields.map((f) => (quote ? quoteCsvField(f) : f)).join(delimiter))
    .join("\n");
}
