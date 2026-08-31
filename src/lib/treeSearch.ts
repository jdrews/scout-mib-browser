/** Pure in-memory search over the loaded OID→name map (S.oidNameMap).
 *  The map is refreshed whenever MIBs load or unload, so results always
 *  reflect the OIDs already loaded — no backend round-trip per keystroke. */

/** Parent OID with the last numeric segment removed; "" for single-segment OIDs. */
export function parentOid(oid: string): string {
  const i = oid.lastIndexOf(".");
  return i <= 0 ? "" : oid.slice(0, i);
}

/** Numeric sub-identifier comparison so `2` sorts before `10`. */
export function compareOids(a: string, b: string): number {
  const as = a.split(".");
  const bs = b.split(".");
  const len = Math.min(as.length, bs.length);
  for (let i = 0; i < len; i++) {
    const x = Number.parseInt(as[i], 10);
    const y = Number.parseInt(bs[i], 10);
    if (x !== y) return x - y;
  }
  return as.length - bs.length;
}

/** True when the query matches a node: case-insensitive name substring, or a
 *  segment-aware OID prefix (the exact OID, or an ancestor whose segments all
 *  match — `1.3.6` must not match `1.3.61`). */
export function matchesQuery(oid: string, name: string, query: string): boolean {
  const q = query.toLowerCase();
  if (name.toLowerCase().includes(q)) return true;
  if (oid === query) return true;
  return oid.toLowerCase().startsWith(q + ".");
}

/** All loaded OIDs matching the query, in numeric OID order (parents before
 *  children, subtrees contiguous) — a stable order for stepping through hits. */
export function searchOids(map: Map<string, string>, query: string): string[] {
  const q = query.trim();
  if (!q) return [];
  const hits: string[] = [];
  for (const [oid, name] of map) {
    if (matchesQuery(oid, name, q)) hits.push(oid);
  }
  hits.sort(compareOids);
  return hits;
}

/** Top-down chain of OIDs from the rendered tree root to `oid` (inclusive).
 *
 *  Follows how the backend builds the tree: a node whose parent OID is not
 *  indexed is itself a root (orphan), and single-segment OIDs are roots.
 *  Expanding every element except the last guarantees the target row renders. */
export function findChain(oid: string, map: Map<string, string>): string[] {
  const chain = [oid];
  let cur = oid;
  for (;;) {
    const p = parentOid(cur);
    if (!p || !map.has(p)) break;
    chain.push(p);
    cur = p;
  }
  return chain.reverse();
}
