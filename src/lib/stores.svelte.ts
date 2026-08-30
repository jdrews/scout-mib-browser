import type { TreeNode, MibSearchResult, TargetConfig, ConnectionState, SnmpOperation, ResultSet, VariableBinding, TableInfo, TableResult, LogEntry, LogLevel, InspectorValue } from "./types";

// ── Single reactive app state (Svelte 5 deep reactivity) ──────────────────────

const raw = $state({
  currentTheme: (typeof localStorage !== "undefined" ? localStorage.getItem("scout-theme") : null) || "dark" as string,
  selectedNode: null as TreeNode | null,
  targetOidFromTree: "" as string,
  treeFocusOid: null as string | null,
  // Session-only dismissal of the fallback banner (UX-18): intentionally not
  // persisted — a broken MIB is still broken at next launch.
  fallbackBannerDismissed: false,
  treeData: [] as TreeNode[],
  // Bumped every time the tree is rebuilt (startup, add directory, unload).
  // Expanded branches watch it to refetch children, so nodes that vanished
  // from a previously expanded subtree don't linger as ghosts.
  treeVersion: 0,
  contextMenuTarget: null as { node: TreeNode; x: number; y: number } | null,
  statusText: "Ready",
  nodeCount: 0,
  fallbackMibs: [] as string[],
  autocompleteResults: [] as MibSearchResult[],
  highlightedIndex: -1,
  manageMibsOpen: false,
  fileMenuOpen: false,
  settingsMenuOpen: false,
  viewMenuOpen: false,
  targetConfig: {
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
  } as TargetConfig,
  connectionPanelOpen: false,
  saveCredentials: true,
  connectionState: "unknown" as ConnectionState,
  snmpOperation: "get" as SnmpOperation,
  isExecuting: false,
  executionBindings: [] as VariableBinding[],
  executionResults: null as ResultSet | null,
  // Get Subtree result — all MIB nodes under the queried OID. Null means no
  // subtree has been fetched; an empty array is a valid (leaf) result.
  subtreeNodes: null as TreeNode[] | null,
  walkProgress: "",
  queryRootOid: null as string | null,
  tableInfo: null as TableInfo | null,
  tableResult: null as TableResult | null,
  systemLogOpen: false,
  logLevelFilter: "all" as LogLevel,
  logEntries: [] as LogEntry[],
  oidNameMap: new Map<string, string>(),
  mibPanelWidth: typeof localStorage !== "undefined" ? parseInt(localStorage.getItem("scout-mib-width") || "320", 10) : 320,
  systemLogHeight: typeof localStorage !== "undefined" ? parseInt(localStorage.getItem("scout-log-height") || "200", 10) : 200,
  // Inspector pane: open by default (UX choice), collapsed state persisted so a
  // user who closed it gets it back closed on next launch.
  inspectorOpen: typeof localStorage === "undefined" || localStorage.getItem("scout-inspector-open") !== "false",
  inspectorHeight: typeof localStorage !== "undefined" ? parseInt(localStorage.getItem("scout-inspector-height") || "240", 10) : 240,
  // The OID the inspector reports on. Set by tree selection, address-bar
  // autocomplete picks, and result row/cell clicks (the latter also set
  // inspectorValue with the live value from the Result Set).
  inspectorOid: null as string | null,
  inspectorValue: null as InspectorValue | null,
});

// ── Persistence proxy (avoids $effect which can't run at module level) ─────────

const persistKeys: Record<string, string> = {
  currentTheme: "scout-theme",
  mibPanelWidth: "scout-mib-width",
  systemLogHeight: "scout-log-height",
  inspectorOpen: "scout-inspector-open",
  inspectorHeight: "scout-inspector-height",
};

export function clearResults() {
  raw.executionBindings.length = 0;
  raw.executionResults = null;
  raw.subtreeNodes = null;
  raw.tableInfo = null;
  raw.tableResult = null;
}

export const S = new Proxy(raw, {
  get(target, prop) {
    return target[prop as keyof typeof target];
  },
  set(target, prop, value) {
    const key = String(prop);
    const oldVal = target[key as keyof typeof target];
    Reflect.set(target, prop, value);
    if (key in persistKeys && typeof localStorage !== "undefined") {
      localStorage.setItem(persistKeys[key], String(value));
    }
    return true;
  },
});
