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
