import type { TreeNode, MibSearchResult, TargetConfig, ConnectionState, SnmpOperation, ResultSet, VariableBinding, TableResult, LogEntry, LogLevel } from "./types";

// ── Single reactive app state (Svelte 5 deep reactivity) ──────────────────────

const raw = $state({
  currentTheme: (typeof localStorage !== "undefined" ? localStorage.getItem("scout-theme") : null) || "dark" as string,
  selectedNode: null as TreeNode | null,
  targetOidFromTree: "" as string,
  treeData: [] as TreeNode[],
  contextMenuTarget: null as { node: TreeNode; x: number; y: number } | null,
  statusText: "Ready",
  nodeCount: 0,
  fallbackMibs: [] as string[],
  autocompleteResults: [] as MibSearchResult[],
  highlightedIndex: -1,
  manageMibsOpen: false,
  fileMenuOpen: false,
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
  connectionState: "disconnected" as ConnectionState,
  snmpOperation: "get" as SnmpOperation,
  isExecuting: false,
  executionBindings: [] as VariableBinding[],
  executionResults: null as ResultSet | null,
  walkProgress: "",
  queryRootOid: null as string | null,
  tableResult: null as TableResult | null,
  systemLogOpen: false,
  logLevelFilter: "all" as LogLevel,
  logEntries: [] as LogEntry[],
  mibPanelWidth: typeof localStorage !== "undefined" ? parseInt(localStorage.getItem("scout-mib-width") || "320", 10) : 320,
  systemLogHeight: typeof localStorage !== "undefined" ? parseInt(localStorage.getItem("scout-log-height") || "200", 10) : 200,
});

// ── Persistence proxy (avoids $effect which can't run at module level) ─────────

const persistKeys: Record<string, string> = {
  currentTheme: "scout-theme",
  mibPanelWidth: "scout-mib-width",
  systemLogHeight: "scout-log-height",
};

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
