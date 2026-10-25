// Centralizes the MIB lifecycle so the refresh sequence is defined once and
// the menu (File → Add MIB Directory), the Manage MIBs page, and startup
// (AppShell) all share the same add/unload/refresh code paths.

import { S } from "$lib/stores.svelte";
import {
  configRead,
  configWrite,
  mibLoad,
  mibUnloadMany,
  openDirectory,
  openFiles,
  mibOidNameMap,
  mibTree,
} from "$lib/tauriCommands";
import type { LoadDirectoriesStatus } from "./types";

/** Sets nodeCount + fallbackMibs, re-fetches the OID name map and the MIB
 * tree (bumping treeVersion so expanded branches refetch children). The one
 * shared refresh path for every MIB state change. */
export async function refreshMibState(status: LoadDirectoriesStatus): Promise<void> {
  S.nodeCount = status.nodeCount;
  S.fallbackMibs.length = 0;
  S.fallbackMibs.push(...status.fallbackMibs);

  try {
    const pairs = await mibOidNameMap();
    S.oidNameMap = new Map(pairs);
  } catch (e) {
    console.error("Failed to refresh OID name map:", e);
  }

  const data = await mibTree();
  S.treeData.length = 0;
  S.treeData.push(...data);
  S.treeVersion++;
}

/** Picks a MIB directory, appends it to the config (dedup), loads the full
 * configured set (directories + explicit files), and refreshes the MIB state. */
export async function addMibDirectory(): Promise<void> {
  try {
    const selected = await openDirectory();
    if (!selected) return;

    S.statusText = "Loading MIBs...";
    const config = await configRead();
    const dirs = config.mib?.directories ?? [];
    const files = config.mib?.files ?? [];
    if (!dirs.includes(selected)) {
      dirs.push(selected);
      await configWrite("mib.directories", dirs);
    }

    const status = await mibLoad(dirs, files);
    await refreshMibState(status);
    S.statusText =
      status.filesCached !== undefined
        ? `Loaded ${status.nodeCount} nodes (${status.filesParsed} parsed, ${status.filesCached} cached)`
        : `Loaded ${status.nodeCount} nodes`;
  } catch (err) {
    S.statusText = `Error: ${err}`;
    console.error("Failed to add MIB directory:", err);
  }
}

/** Picks MIB files, appends them to the config (dedup), loads the full
 * configured set, and refreshes the MIB state. */
export async function addMibFiles(): Promise<void> {
  try {
    const selected = await openFiles();
    if (!selected || selected.length === 0) return;

    S.statusText = "Loading MIBs...";
    const config = await configRead();
    const dirs = config.mib?.directories ?? [];
    const files = config.mib?.files ?? [];
    for (const f of selected) {
      if (!files.includes(f)) files.push(f);
    }
    await configWrite("mib.files", files);

    const status = await mibLoad(dirs, files);
    await refreshMibState(status);
    S.statusText =
      status.filesCached !== undefined
        ? `Loaded ${status.nodeCount} nodes (${status.filesParsed} parsed, ${status.filesCached} cached)`
        : `Loaded ${status.nodeCount} nodes`;
  } catch (err) {
    S.statusText = `Error: ${err}`;
    console.error("Failed to add MIB files:", err);
  }
}

/** Unloads the given file paths in a single backend call and refreshes the
 * MIB state. The caller sets the status text (Unload Selected vs. Unload
 * All). */
export async function unloadMibs(paths: string[]): Promise<void> {
  const status = await mibUnloadMany(paths);
  await refreshMibState(status);
}
