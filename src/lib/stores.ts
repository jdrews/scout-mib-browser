import { writable } from "svelte/store";
import type { TreeNode, MibSearchResult, TargetConfig, ConnectionState, SnmpOperation, ResultSet, VariableBinding, TableResult, LogEntry, LogLevel } from "./types";

/** Currently selected MIB node (null = no selection). */
export const selectedNode = writable<TreeNode | null>(null);

/** Full hierarchical tree data. */
export const treeData = writable<TreeNode[]>([]);

/** Target node for the context menu. */
export const contextMenuTarget = writable<{ node: TreeNode; x: number; y: number } | null>(null);

/** Status bar text. */
export const statusText = writable("Ready");

/** Total number of loaded MIB nodes. */
export const nodeCount = writable(0);

/** Names of MIB modules loaded via regex fallback. */
export const fallbackMibs = writable<string[]>([]);

/** Current autocomplete search results. */
export const autocompleteResults = writable<MibSearchResult[]>([]);

/** Index of the highlighted autocomplete item (-1 = none). */
export const highlightedIndex = writable(-1);

/** Whether the Manage MIBs dialog is open. */
export const manageMibsOpen = writable(false);

/** Whether the File menu dropdown is open. */
export const fileMenuOpen = writable(false);

/// Current Target connection configuration (from config + user edits).
export const targetConfig = writable<TargetConfig>({
  host: "",
  port: 161,
  version: "v2c",
  community: "public",
  v3_username: "",
  v3_auth_protocol: "none",
  v3_auth_passphrase: "",
  v3_priv_protocol: "none",
  v3_priv_passphrase: "",
  v3_security_level: "noAuthNoPrivacy",
});

/** Whether the Connection Panel modal is open. */
export const connectionPanelOpen = writable(false);

/// Current connection state to the Target.
export const connectionState = writable<ConnectionState>("disconnected");

// ── SNMP Execution stores ────────────────────────────────────────────────────

/** Currently selected SNMP operation mode. */
export const snmpOperation = writable<SnmpOperation>("get");

/** Whether an SNMP operation is currently executing. */
export const isExecuting = writable(false);

/** Accumulated variable bindings from the current execution (streamed for walks). */
export const executionBindings = writable<VariableBinding[]>([]);

/** Final result set from the last completed execution. */
export const executionResults = writable<ResultSet | null>(null);

/** Walk progress indicator (e.g., "100/1234 bindings"). */
export const walkProgress = writable("");

/** Root OID of the last query (for export default filename). */
export const queryRootOid = writable<string | null>(null);

// ── Table Retrieval stores ───────────────────────────────────────────────────

/** Result from a table retrieval operation (grid view data). */
export const tableResult = writable<TableResult | null>(null);

// ── System Log stores ────────────────────────────────────────────────────────

/** Whether the system log pane is visible. */
export const systemLogOpen = writable(false);

/** Current severity filter for the log pane. */
export const logLevelFilter = writable<LogLevel>("all");

/** Accumulated log entries from the backend. */
export const logEntries = writable<LogEntry[]>([]);
