import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open as tauriOpen, save as tauriSave } from "@tauri-apps/plugin-dialog";
import type {
  TreeNode,
  MibSearchResult,
  LoadedMib,
  LoadDirectoriesStatus,
  AppConfig,
  ResultSet,
  VariableBinding,
  TargetConfig,
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
  const result = await tauriOpen({ directory: true, multiple: false });
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

  const batchUnlisten = await listen<ResultSet>("snmp-walk-batch", (event) => {
    onBatch(event.payload.bindings);
  });

  const completeUnlisten = await listen<ResultSet>("snmp-walk-complete", (event) => {
    onComplete(event.payload);
  });

  await invoke(command, { params, root_oid: rootOid });

  return {
    unlisten: async () => {
      await batchUnlisten();
      await completeUnlisten();
    },
  };
}

/** Executes a Walk operation from the given root OID. Returns immediately — results stream via callbacks. */
export async function snmpWalk(
  targetConfig: TargetConfig,
  rootOid: string,
  onBatch: (bindings: VariableBinding[]) => void,
  onComplete: (result: ResultSet) => void,
): Promise<{ unlisten: () => void }> {
  return snmpStreamingWalk("snmp_walk", targetConfig, rootOid, onBatch, onComplete);
}

/** Executes a BulkWalk operation from the given root OID. Returns immediately — results stream via callbacks. */
export async function snmpBulkWalk(
  targetConfig: TargetConfig,
  rootOid: string,
  onBatch: (bindings: VariableBinding[]) => void,
  onComplete: (result: ResultSet) => void,
): Promise<{ unlisten: () => void }> {
  return snmpStreamingWalk("snmp_bulk_walk", targetConfig, rootOid, onBatch, onComplete);
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
    value_type: valueType,
    value,
  });
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
  filters?: Array<{ name: string; extensions: string[] }>,
): Promise<string | null> {
  const path = await tauriSave({
    title: "Save Results",
    defaultPath: defaultFilename,
    filters: filters ? [{ name: defaultFilename.split(".").pop() || "File", extensions: [defaultFilename.split(".").pop() || "*"] }] : undefined,
  });
  if (!path) return null;
  await fsWriteFile(path, content);
  return path;
}
