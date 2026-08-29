/** Per-table display-column selection.
 *
 * Under the single-pass walk every column is always fetched; the selection is
 * a pure display filter that shows/hides grid columns immediately and is
 * restored as the display state of subsequent runs. The default (no saved
 * value) is all columns; an explicitly empty selection persists as "none".
 *
 * Storage: a session-scoped map is the source of truth; localStorage mirrors
 * it best-effort so the selection also survives app restarts where storage is
 * available. (In some embedded webview environments localStorage writes do not
 * survive past the current task, which is why the map comes first.) */

const sessionSelections = new Map<string, string[]>();

function storageKey(tableOid: string): string {
  return `scout-table-cols-${tableOid}`;
}

/** Intersects a candidate selection with the current column set, preserving
 *  `allColumns` order. Empty when nothing intersects. */
function intersect(candidate: string[], allColumns: string[]): string[] {
  return allColumns.filter((c) => candidate.includes(c));
}

/** Resolves a saved candidate against the current column set.
 * - explicit empty selection → `[]` (the user chose "none")
 * - non-empty with a live intersection → that subset
 * - otherwise (nothing usable) → null, meaning "fall back to all columns" */
function resolve(candidate: string[] | undefined, allColumns: string[]): string[] | null {
  if (!candidate) return null;
  if (candidate.length === 0) return [];
  const selected = intersect(candidate, allColumns);
  return selected.length > 0 ? selected : null;
}

/** Returns the persisted column subset for a table, in `allColumns` order.
 * Falls back to all columns when nothing is saved (or the saved selection no
 * longer intersects the current column set). An explicit empty selection
 * persists as "none". */
export function loadColumnSelection(tableOid: string, allColumns: string[]): string[] {
  const fromSession = resolve(sessionSelections.get(tableOid), allColumns);
  if (fromSession !== null) return fromSession;
  try {
    const saved: unknown = JSON.parse(localStorage.getItem(storageKey(tableOid)) || "null");
    if (Array.isArray(saved)) {
      const fromStorage = resolve(saved as string[], allColumns);
      if (fromStorage !== null) return fromStorage;
    }
  } catch {
    // Corrupt storage — fall through to the default.
  }
  return [...allColumns];
}

/** Persists the column subset for a table. */
export function saveColumnSelection(tableOid: string, selected: string[]): void {
  sessionSelections.set(tableOid, [...selected]);
  try {
    localStorage.setItem(storageKey(tableOid), JSON.stringify(selected));
  } catch {
    // Storage full/unavailable — selection still persists for the session.
  }
}
