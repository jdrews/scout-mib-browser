/** Per-table display-column selection.
 *
 * Under the single-pass walk the selection is a display filter only — it
 * never reduces what is fetched — so it applies to the next Get Table run.
 * The default (no saved value) is all columns.
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

/** Returns the persisted column subset for a table, in `allColumns` order.
 * Falls back to all columns when nothing is saved (or the saved selection no
 * longer intersects the current column set). */
export function loadColumnSelection(tableOid: string, allColumns: string[]): string[] {
  const cached = sessionSelections.get(tableOid);
  if (cached) {
    const selected = intersect(cached, allColumns);
    if (selected.length > 0) return selected;
  }
  try {
    const saved: unknown = JSON.parse(localStorage.getItem(storageKey(tableOid)) || "null");
    if (Array.isArray(saved)) {
      const selected = intersect(saved as string[], allColumns);
      if (selected.length > 0) return selected;
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
