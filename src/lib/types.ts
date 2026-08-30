/** Single node in the hierarchical MIB tree for UI rendering.
 * Field names are camelCase to match the backend's serde(rename_all = "camelCase"). */
export interface TreeNode {
  oid: string;
  name: string;
  syntaxType?: string;
  mibName: string;
  isTable?: boolean;
  hasChildren?: boolean;
  children?: TreeNode[];
}

/** Result of a MIB search query (autocomplete). */
export interface MibSearchResult {
  oid: string;
  name: string;
  syntaxType: string;
  mibName: string;
}

/** Metadata about a loaded MIB file for the Manage MIBs dialog. */
export interface LoadedMib {
  mibName: string;
  filePath: string;
  nodeCount: number;
  isFallback: boolean;
}

/** Status response from MIB loading operations. */
export interface LoadDirectoriesStatus {
  nodeCount: number;
  fallbackMibs: string[];
}

/** SNMP version for Target connections. */
export type SnmpVersion = "v1" | "v2c" | "v3";

/** Authentication protocol for SNMPv3 USM. */
export type V3AuthProtocol = "none" | "md5" | "sha1" | "sha224" | "sha256" | "sha384" | "sha512";

/** Privacy (encryption) protocol for SNMPv3 USM. */
export type V3PrivProtocol = "none" | "des" | "aes128" | "aes192" | "aes256";

/** Security level for SNMPv3. */
export type V3SecurityLevel = "noAuthNoPrivacy" | "authNoPrivacy" | "authPrivacy";

/// Last-used Target connection settings from config.
export interface TargetConfig {
  host: string;
  port: number;
  version: SnmpVersion;
  community: string;
  v3_username: string;
  v3_auth_protocol: V3AuthProtocol;
  v3_auth_passphrase: string;
  v3_priv_protocol: V3PrivProtocol;
  v3_priv_passphrase: string;
  v3_security_level: V3SecurityLevel;
}

/** Connection state for the Target. */
// "unknown" is the neutral startup state (no attempt yet); "disconnected"
// means an actual connection attempt failed.
export type ConnectionState = "unknown" | "disconnected" | "connecting" | "connected";

/** Application configuration read from the backend. */
export interface AppConfig {
  mib?: {
    directories?: string[];
  };
  target?: Omit<TargetConfig, "host" | "port"> & Partial<Pick<TargetConfig, "host" | "port">>;
  ui?: {
    save_credentials?: boolean;
  };
}

// ── SNMP Execution Types ─────────────────────────────────────────────────────

/** SNMP operation mode. */
export type SnmpOperation = "get" | "getNext" | "walk" | "bulkWalk" | "getTable" | "set";

/** A single SNMP data value from the backend (tagged union matching Rust enum).
 * Note: serde serializes unit variants as their name string, so Null arrives as "Null". */
export type SnmpValue =
  | { Integer: number }
  | { Unsigned: number }
  | { Counter32: number }
  | { Counter64: number }
  | { OctetString: number[] }
  | { ObjectIdentifier: string }
  | { IpAddress: string }
  | { TimeTicks: number }
  | { TruthValue: boolean }
  | "Null"
  | { Raw: { type_code: number; data: number[] } };

/** An OID paired with its live value returned from a Target. */
export interface VariableBinding {
  oid: string;
  value: SnmpValue;
  warning?: boolean;
}

/** A non-fatal issue encountered during an SNMP operation. */
export interface SnmpWarning {
  kind: string;
  message: string;
  oid?: string;
}

/** Output of an SNMP execution — bindings plus warnings. */
export interface ResultSet {
  bindings: VariableBinding[];
  warnings?: SnmpWarning[];
  partial: boolean;
  retries?: number;
}

// ── Table Retrieval Types ────────────────────────────────────────────────────

/** How an index component maps to OID sub-identifiers.
 * `FixedString(n)` serializes as the bare number n (serde newtype variant). */
export type IndexEncoding = "Integer" | "IpAddress" | "Variable" | number;

/** One component of a table's INDEX clause, in clause order. */
export interface TableIndexColumn {
  name: string;
  oid: string;
  implied: boolean;
  encoding: IndexEncoding;
}

/** Parsed INDEX/AUGMENTS metadata for a TABLE node (camelCase from serde). */
export interface TableInfo {
  tableOid: string;
  name: string;
  rowEntryOids: string[];
  indexColumns: TableIndexColumn[];
  columnOids: string[];
}

/** A named value from a MIB type definition (INTEGER enum or BITS bit). */
export interface NamedValueInfo {
  label: string;
  value: number;
}

/** Full inspector details for one MIB node (camelCase from serde).
 * Omitted fields arrive absent — the backend skips null/empty values. */
export interface MibNodeDetails {
  oid: string;
  name: string;
  mibName: string;
  /** SYNTAX type label, e.g. "OctetString", "TABLE", "ROW". */
  syntaxType: string;
  isTable?: boolean;
  description?: string;
  /** MAX-ACCESS label, e.g. "read-only". */
  access?: string;
  /** STATUS label, e.g. "current", "deprecated". */
  status?: string;
  units?: string;
  defaultValue?: string;
  reference?: string;
  displayHint?: string;
  /** Value constraints, e.g. "1..255" or "SIZE (0..32)". */
  constraints?: string;
  enums?: NamedValueInfo[];
  bits?: NamedValueInfo[];
  /** Present when the node is a TABLE container. */
  table?: TableInfo;
  /** Present when the node is a row entry of a known table. */
  indexColumns?: TableIndexColumn[];
}

/** A live value captured when a result row/cell drives the inspector. */
export interface InspectorValue {
  text: string;
  typeLabel: string;
}

/** One row of the flat result list: a binding with resolved name and display text. */
export interface ResultRow {
  oid: string;
  displayName: string;
  /** OID plus any instance suffix, e.g. "1.3.6.1.2.1.1.1.0". */
  fullPath: string;
  type: string;
  value: string;
  warning: boolean;
}

/** A single cell in a table grid result. */
export interface TableCell {
  value?: VariableBinding;
  missing: boolean;
}

/** A single row in a table grid, keyed by its instance suffix. */
export interface TableRowData {
  instance_id: string;
  cells: Record<string, TableCell>;
  /** Decoded index component values aligned with TableInfo.indexColumns order
   * (null = IMPLIED component). Empty when the table has no index metadata or
   * the suffix is undecodable — the UI shows the raw instance column instead. */
  index_values: (string | null)[];
}

/** Result of a table retrieval operation — pivoted grid of rows and columns. */
export interface TableResult {
  table_oid: string;
  columns: string[];
  rows: TableRowData[];
  total_rows: number;
  missing_cells: number;
  warnings?: SnmpWarning[];
  partial: boolean;
}

// ── System Log Types ─────────────────────────────────────────────────────────

/** A single log entry from the backend. */
export interface LogEntry {
  timestamp: string;
  level: string;
  target: string;
  message: string;
}

/** Severity filter for the system log pane. */
export type LogLevel = "all" | "error" | "warn" | "info";
