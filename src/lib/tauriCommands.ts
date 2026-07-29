import { invoke } from "@tauri-apps/api/core";

/** Tauri 2.x event listener — uses core.registerListener. */
export async function tauriListen<T = any>(
  event: string,
  handler: (payload: T) => void,
): Promise<() => void | Promise<void>> {
  const eventId = await invoke<number>("registerListener", { event });
  return () => invoke("unregisterListener", { eventId }).then(() => {});
}

import type {
  TreeNode,
  MibSearchResult,
  LoadedMib,
  LoadDirectoriesStatus,
  AppConfig,
  ResultSet,
  VariableBinding,
  TargetConfig,
  TableResult,
  LogEntry,
} from "./types";

/** Reads the application configuration. */
export async function configRead(): Promise<AppConfig> {
  return invoke("config_read");
}

/** Writes a value to the application configuration. */
export async function configWrite(
  path: string,
  value: unknown,
): Promise<void> {
  return invoke("config_write", { path, value });
}

/** Returns the hierarchical MIB tree for rendering in the UI. */
export async function mibTree(): Promise<TreeNode[]> {
  return invoke("mib_tree");
}

/** Returns direct children of the given OID for lazy loading. */
export async function mibChildren(oid: string): Promise<TreeNode[]> {
  return invoke("mib_children", { oid });
}

/** Searches for MIB nodes matching the given query (autocomplete). */
export async function mibSearch(query: string): Promise<MibSearchResult[]> {
  return invoke("mib_search", { query });
}

/** Resolves a dotted-decimal OID to its MIB node. */
export async function mibResolveOid(oid: string): Promise<{ oid: string; name: string; syntax_type?: string; mib_name: string } | null> {
  return invoke("mib_resolve_oid", { oid });
}

/** Loads all MIB files from the given directories. */
export async function mibLoadDirectories(
  directories: string[],
): Promise<LoadDirectoriesStatus> {
  return invoke("mib_load_directories", { directories });
}

/** Unloads all nodes from the given MIB module. */
export async function mibUnload(mibName: string): Promise<LoadDirectoriesStatus> {
  return invoke("mib_unload", { mibName });
}

/** Returns metadata about all currently loaded MIB modules. */
export async function mibLoadedList(): Promise<LoadedMib[]> {
  return invoke("mib_loaded_list");
}

/** Opens a native directory picker dialog. */
export async function openDirectory(): Promise<string | null> {
  const result = (await invoke("dialog_open_directory")) as string | null;
  return result;
}

/// Tests connectivity to the Target by performing a simple SNMP Get.
export async function snmpConnect(params: {
  host: string;
  port: number;
  version: string;
  community?: string;
  v3_username?: string;
  v3_auth_protocol?: string;
  v3_auth_passphrase?: string;
  v3_priv_protocol?: string;
  v3_priv_passphrase?: string;
}): Promise<{ bindings: unknown[]; warnings?: unknown[] }> {
  return invoke("snmp_connect", { params });
}

/// Persists all Target connection settings to config at once.
export async function persistTargetConfig(config: {
  host: string;
  port: number;
  version: string;
  community: string;
  v3_username: string;
  v3_auth_protocol: string;
  v3_auth_passphrase: string;
  v3_priv_protocol: string;
  v3_priv_passphrase: string;
}): Promise<void> {
  await invoke("config_write_target", { config });
}

// ── SNMP Execution Commands ──────────────────────────────────────────────────

/** Builds the params object from target config for SNMP commands. */
function buildSnmpParams(config: TargetConfig) {
  return {
    host: config.host,
    port: config.port,
    version: config.version,
    community: config.community || undefined,
    v3_username: config.v3_username || undefined,
    v3_auth_protocol: config.v3_auth_protocol !== "none" ? config.v3_auth_protocol : undefined,
    v3_auth_passphrase: config.v3_auth_passphrase || undefined,
    v3_priv_protocol: config.v3_priv_protocol !== "none" ? config.v3_priv_protocol : undefined,
    v3_priv_passphrase: config.v3_priv_passphrase || undefined,
  };
}

/** Executes a Get operation for the given OIDs. */
export async function snmpGet(
  targetConfig: TargetConfig,
  oids: string[],
): Promise<ResultSet> {
  return invoke("snmp_get", { params: buildSnmpParams(targetConfig), oids });
}

/** Executes a GetNext operation for the given OIDs. */
export async function snmpGetNext(
  targetConfig: TargetConfig,
  oids: string[],
): Promise<ResultSet> {
  return invoke("snmp_get_next", { params: buildSnmpParams(targetConfig), oids });
}

/** Executes a streaming walk (Walk or BulkWalk). Returns immediately — results stream via callbacks. */
async function snmpStreamingWalk(
  command: string,
  targetConfig: TargetConfig,
  rootOid: string,
  onBatch: (bindings: VariableBinding[]) => void,
  onComplete: (result: ResultSet) => void,
): Promise<{ unlisten: () => void }> {
  const params = buildSnmpParams(targetConfig);

  const batchUnlisten = await tauriListen<{ bindings: VariableBinding[] }>("snmp-walk-batch", (payload) => {
    onBatch(payload.bindings);
  });

  const completeUnlisten = await tauriListen<ResultSet>("snmp-walk-complete", (payload) => {
    onComplete(payload);
  });

  await invoke(command, { params, rootOid });

  return {
    unlisten: async () => {
      await batchUnlisten();
      await completeUnlisten();
    },
  };
}

/** Executes a Walk operation from the given root OID (blocking). */
export async function snmpWalk(
  targetConfig: TargetConfig,
  rootOid: string,
  onBatch: (bindings: VariableBinding[]) => void,
  onComplete: (result: ResultSet) => void,
): Promise<{ unlisten: () => void }> {
  const result = await invoke<ResultSet>("snmp_walk", { params: buildSnmpParams(targetConfig), rootOid: rootOid });
  onBatch(result.bindings);
  onComplete(result);
  return { unlisten: () => {} };
}

/** Executes a BulkWalk operation from the given root OID (blocking). */
export async function snmpBulkWalk(
  targetConfig: TargetConfig,
  rootOid: string,
  onBatch: (bindings: VariableBinding[]) => void,
  onComplete: (result: ResultSet) => void,
): Promise<{ unlisten: () => void }> {
  const result = await invoke<ResultSet>("snmp_bulk_walk", { params: buildSnmpParams(targetConfig), rootOid });
  onBatch(result.bindings);
  onComplete(result);
  return { unlisten: () => {} };
}

/** Executes a Set operation to write a value at the given OID. */
export async function snmpSet(
  targetConfig: TargetConfig,
  oid: string,
  valueType: string,
  value: unknown,
): Promise<ResultSet> {
  return invoke("snmp_set", {
    params: buildSnmpParams(targetConfig),
    oid,
    valueType,
    value,
  });
}

/** Walks all columns of a table and returns results as a pivoted grid. */
export async function snmpWalkTable(
  targetConfig: TargetConfig,
  tableOid: string,
  columnOids: string[],
): Promise<TableResult> {
  return invoke("snmp_walk_table", {
    params: buildSnmpParams(targetConfig),
    tableOid,
    columnOids,
  });
}

/** Returns column OIDs for a TABLE node. */
export async function mibTableColumns(tableOid: string): Promise<string[]> {
  return invoke("mib_table_columns", { tableOid });
}

/** Returns all OID → name pairs from the loaded MIB index. */
export async function mibOidNameMap(): Promise<[string, string][]> {
  return invoke("mib_oid_name_map");
}

// ── File System Commands ─────────────────────────────────────────────────────

/** Writes a string to the given file path via backend. */
export async function fsWriteFile(path: string, content: string): Promise<void> {
  return invoke("fs_write_file", { path, content });
}

/** Opens a native save dialog and writes content to the selected path. Returns the saved path or null if cancelled. */
export async function saveToFile(
  content: string,
  defaultFilename: string,
): Promise<string | null> {
  const path = (await invoke("dialog_save_file", { defaultPath: defaultFilename })) as string | null;
  if (!path) return null;
  await fsWriteFile(path, content);
  return path;
}

// ── System Log Commands ──────────────────────────────────────────────────────

/** Reads all log entries from the backend. */
export async function logRead(): Promise<LogEntry[]> {
  return invoke("log_read");
}

/** Clears the in-memory log buffer. */
export async function logClear(): Promise<void> {
  return invoke("log_clear");
}

/** Returns the path to the log file on disk. */
export async function logPath(): Promise<string> {
  return invoke("log_path");
}
