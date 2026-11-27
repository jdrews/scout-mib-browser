import { describe, expect, it } from "vitest";
import { alignWindows, entryKey } from "./logWindow";
import type { LogEntry } from "./types";

/** Entries keyed by their visible identity (the pane has no per-entry id). */
function e(ts: string, msg = "m"): LogEntry {
  return { timestamp: ts, level: "INFO", target: "t", message: msg };
}

/** Window of entries `from..to` inclusive, timestamps zero-padded. */
function span(from: number, to: number): LogEntry[] {
  const out: LogEntry[] = [];
  for (let i = from; i <= to; i++) out.push(e(String(i).padStart(4, "0")));
  return out;
}

describe("alignWindows", () => {
  it("reports no change for an identical window", () => {
    const w = span(1, 5);
    expect(alignWindows(w, w, entryKey)).toEqual({ dropped: 0, appended: 0 });
  });

  it("counts appends when the window has room", () => {
    expect(alignWindows(span(1, 5), span(1, 8), entryKey)).toEqual({
      dropped: 0,
      appended: 3,
    });
  });

  it("counts leading rows lost when the capped window slides", () => {
    // 200-row window slides by 3: the reader's first 3 rendered entries are
    // gone from the top and the view must be compensated by their height.
    expect(alignWindows(span(5, 204), span(8, 207), entryKey)).toEqual({
      dropped: 3,
      appended: 3,
    });
  });

  it("counts a full drop when the window slides past its own length", () => {
    expect(alignWindows(span(1, 4), span(9, 12), entryKey)).toEqual({
      dropped: 4,
      appended: 4,
    });
  });

  it("treats a cleared log as a full reset", () => {
    expect(alignWindows(span(1, 3), [], entryKey)).toEqual({ dropped: 3, appended: 0 });
    expect(alignWindows([], span(1, 3), entryKey)).toEqual({ dropped: 0, appended: 3 });
  });

  it("takes the largest overlap when identical rows repeat", () => {
    // The pane renders TRACE spam where consecutive rows share a
    // millisecond timestamp, so the suffix/prefix overlap is ambiguous —
    // the largest one is the only reading that never invents a drop.
    const prev = [e("1"), e("2", "x"), e("3", "x"), e("4", "y")];
    const next = [e("2", "x"), e("3", "x"), e("4", "y"), e("5", "z")];
    expect(alignWindows(prev, next, entryKey)).toEqual({ dropped: 1, appended: 1 });
  });

  it("handles a poll that arrives with no overlap at all", () => {
    expect(alignWindows([e("1"), e("2")], [e("9")], entryKey)).toEqual({
      dropped: 2,
      appended: 1,
    });
  });

  it("treats a window that shrank at the end as a reset, not an append", () => {
    // prev [1..6] → next [1,2] cannot be an append (the tail moved
    // backwards), so the largest valid overlap is empty: every rendered row
    // is replaced, and compensating by the old leading rows would be wrong.
    expect(alignWindows(span(1, 6), span(1, 2), entryKey)).toEqual({
      dropped: 6,
      appended: 2,
    });
  });
});

describe("entryKey", () => {
  it("distinguishes entries that differ in any visible field", () => {
    const base = e("1");
    expect(entryKey(base)).not.toBe(entryKey({ ...base, timestamp: "2" }));
    expect(entryKey(base)).not.toBe(entryKey({ ...base, level: "ERROR" }));
    expect(entryKey(base)).not.toBe(entryKey({ ...base, target: "other" }));
    expect(entryKey(base)).not.toBe(entryKey({ ...base, message: "other" }));
    expect(entryKey(base)).toBe(entryKey({ ...base }));
  });
});
