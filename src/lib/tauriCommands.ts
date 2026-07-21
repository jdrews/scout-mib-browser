import { invoke } from "@tauri-apps/api/core";
import { open as tauriOpen } from "@tauri-apps/plugin-dialog";
import type {
  TreeNode,
  MibSearchResult,
  LoadedMib,
  LoadDirectoriesStatus,
  AppConfig,
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
