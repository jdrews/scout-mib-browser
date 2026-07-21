import { writable } from "svelte/store";
import type { TreeNode, MibSearchResult, TargetConfig, ConnectionState } from "./types";

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
