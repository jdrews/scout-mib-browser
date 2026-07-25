/** Single node in the hierarchical MIB tree for UI rendering. */
export interface TreeNode {
  oid: string;
  name: string;
  syntax_type?: string;
  mib_name: string;
  is_table?: boolean;
  children?: TreeNode[];
}

/** Result of a MIB search query (autocomplete). */
export interface MibSearchResult {
  oid: string;
  name: string;
  syntax_type: string;
  mib_name: string;
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
export type ConnectionState = "disconnected" | "connecting" | "connected";

/** Application configuration read from the backend. */
export interface AppConfig {
  mib?: {
    directories?: string[];
  };
  target?: Omit<TargetConfig, "host" | "port"> & Partial<Pick<TargetConfig, "host" | "port">>;
}

// ── SNMP Execution Types ─────────────────────────────────────────────────────

/** SNMP operation mode. */
export type SnmpOperation = "get" | "getNext" | "walk" | "bulkWalk" | "set";

/** A single SNMP data value from the backend (tagged union matching Rust enum). */
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
  | { Null: null }
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

/** A single cell in a table grid result. */
export interface TableCell {
  value?: VariableBinding;
  missing: boolean;
}

/** A single row in a table grid, keyed by its instance suffix. */
export interface TableRowData {
  instance_id: string;
  cells: Record<string, TableCell>;
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
