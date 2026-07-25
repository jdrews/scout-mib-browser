import type { VariableBinding, SnmpValue, SnmpWarning, TargetConfig } from "./types";

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

/** Display string for a SnmpValue (matches ResultsPane logic). */
export function valueDisplay(v: SnmpValue): string {
  if ("Integer" in v) return String(v.Integer);
  if ("Unsigned" in v) return String(v.Unsigned);
  if ("Counter32" in v) return `${v.Counter32} (counter32)`;
  if ("Counter64" in v) return `${v.Counter64} (counter64)`;
  if ("OctetString" in v) {
    try {
      const s = new TextDecoder().decode(new Uint8Array(v.OctetString));
      return `"${s}"`;
    } catch {
      return `0x${v.OctetString.map(b => b.toString(16).padStart(2, "0")).join("")}`;
    }
  }
  if ("ObjectIdentifier" in v) return v.ObjectIdentifier;
  if ("IpAddress" in v) return v.IpAddress;
  if ("TimeTicks" in v) return `${v.TimeTicks} (timeticks)`;
  if ("TruthValue" in v) return v.TruthValue ? "true" : "false";
  if ("Null" in v) return "NULL";
  if ("Raw" in v) {
    const r = v.Raw;
    return `<raw type=0x${r.type_code.toString(16).padStart(2, "0")} data=0x${r.data.map(b => b.toString(16).padStart(2, "0")).join("")}>`;
  }
  return String(v);
}

/** Type label for a SnmpValue (matches ResultsPane logic). */
export function typeLabel(v: SnmpValue): string {
  if ("Integer" in v) return "INTEGER";
  if ("Unsigned" in v) return "UNSIGNED32";
  if ("Counter32" in v) return "COUNTER32";
  if ("Counter64" in v) return "COUNTER64";
  if ("OctetString" in v) return "OCTET STRING";
  if ("ObjectIdentifier" in v) return "OBJECT IDENTIFIER";
  if ("IpAddress" in v) return "IPADDRESS";
  if ("TimeTicks" in v) return "TIMETICKS";
  if ("TruthValue" in v) return "TRUTHVALUE";
  if ("Null" in v) return "NULL";
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
function csvQuote(field: string): string {
  if (field.includes(",") || field.includes('"') || field.includes("\n") || field.includes("\r")) {
    return `"${field.replace(/"/g, '""')}"`;
  }
  return field;
}

/** Format rows as CSV with RFC 4180 quoting. */
export function formatCSV(rows: ExportRow[]): string {
  return rows.map(r => [r.oid, r.name, r.type, r.value].map(csvQuote).join(",")).join("\n");
}
