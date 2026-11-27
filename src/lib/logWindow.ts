import type { LogEntry } from "./types";

// Scroll anchoring for the system log pane (e2e-stability/02). The pane
// renders the last `maxEntries` entries, so every poll that arrives at the cap
// removes rows from the TOP of the list. The browser then clamps `scrollTop`
// to the shorter content and the text under the reader's eyes jumps — the
// "yanked" symptom the pane is supposed to prevent. To compensate, the pane
// needs to know how many leading rows a poll removes, which is only
// computable by aligning the previous window against the new one: entries have
// no ids, so their visible fields are the identity.

/** Identity of an entry as the pane renders it (the pane has no per-entry id). */
export function entryKey(entry: LogEntry): string {
  return `${entry.timestamp}|${entry.level}|${entry.target}|${entry.message}`;
}

export interface WindowAlignment {
  /** Rows removed from the top of the list by this update. */
  dropped: number;
  /** Rows added at the bottom by this update. */
  appended: number;
}

/**
 * Aligns the rendered window against the incoming one, assuming the log is
 * append-only: the overlap is the longest run that is a suffix of `prev` and a
 * prefix of `next`. The longest such run is the only reading that never
 * invents a drop, because TRACE spam repeats identical rows within a
 * millisecond. A window that cannot be aligned at all (a clear, a log that
 * moved backwards) is a full reset: every row is replaced.
 */
export function alignWindows<T>(
  prev: readonly T[],
  next: readonly T[],
  key: (entry: T) => string,
): WindowAlignment {
  const max = Math.min(prev.length, next.length);
  for (let overlap = max; overlap > 0; overlap--) {
    let matches = true;
    const offset = prev.length - overlap;
    for (let i = 0; i < overlap; i++) {
      if (key(prev[offset + i]) !== key(next[i])) {
        matches = false;
        break;
      }
    }
    if (matches) return { dropped: prev.length - overlap, appended: next.length - overlap };
  }
  return { dropped: prev.length, appended: next.length };
}
