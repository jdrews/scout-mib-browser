/** Handles to mounted tree rows, so panel-level code (find) can expand
 *  branches and scroll rows without threading props through the tree.
 *  Each TreeNode registers on mount and unregisters on destroy. */

export interface TreeNodeHandle {
  el: HTMLElement;
  /** Expands the node, fetching children if needed. Resolves once loaded. */
  expand: () => Promise<void>;
}

const registry = new Map<string, TreeNodeHandle>();

/** Registers a handle under its OID. Returns the unregister function to call
 *  on destroy (identity-checked, so duplicate OIDs from orphan nodes don't
 *  evict each other's entries). */
export function registerTreeNode(oid: string, handle: TreeNodeHandle): () => void {
  registry.set(oid, handle);
  return () => {
    if (registry.get(oid) === handle) registry.delete(oid);
  };
}

export function getTreeNode(oid: string): TreeNodeHandle | undefined {
  return registry.get(oid);
}
