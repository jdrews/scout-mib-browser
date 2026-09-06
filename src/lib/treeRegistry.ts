/** Handles to mounted tree rows, so panel-level code (find) can expand
 *  branches and scroll rows without threading props through the tree.
 *  Each TreeNode registers on mount and unregisters on destroy. */

export interface TreeNodeHandle {
  el: HTMLElement;
  /** Expands the node, fetching children if needed. Resolves once loaded;
   *  true when it actually expanded (false if already expanded or a leaf). */
  expand: () => Promise<boolean>;
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

/** Shallowest registered OID that is a proper descendant of `oid`, or null.
 *  A node absorbed by empty-folder collapse has no row of its own — its
 *  rendered representative is the descendant carrying the dot-joined name. */
export function findRenderedDescendant(oid: string): string | null {
  const prefix = `${oid}.`;
  let best: string | null = null;
  for (const o of registry.keys()) {
    if (!o.startsWith(prefix)) continue;
    if (best === null || o.split(".").length < best.split(".").length) best = o;
  }
  return best;
}
