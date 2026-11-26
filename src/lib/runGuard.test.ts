import { describe, it, expect } from "vitest";
import { RunGuard } from "./runGuard";

// A cancelled Get/GetNext keeps its promise chain alive: the engine reports
// the cancel asynchronously, so the late result/rejection can land after the
// user already started the next run. Applying that outcome unconditionally
// overwrites the new run's status, resets the Get Next sequence cursor, and
// clears the Result Set on the next run (the race CI caught in cancel.spec).
// RunGuard is the run-identity seam TargetBar uses to drop stale outcomes.

describe("RunGuard (launch-beta/09)", () => {
  it("a freshly started run is current", () => {
    const g = new RunGuard();
    const id = g.start();
    expect(g.isCurrent(id)).toBe(true);
  });

  it("starting a new run supersedes the previous one", () => {
    const g = new RunGuard();
    const first = g.start();
    const second = g.start();
    expect(g.isCurrent(first)).toBe(false);
    expect(g.isCurrent(second)).toBe(true);
  });

  it("retiring (Stop pressed) makes the current run stale", () => {
    const g = new RunGuard();
    const id = g.start();
    g.retire();
    expect(g.isCurrent(id)).toBe(false);
  });

  it("a run started after a retire is current", () => {
    const g = new RunGuard();
    g.start();
    g.retire();
    const next = g.start();
    expect(g.isCurrent(next)).toBe(true);
  });

  it("retire with no run started is safe", () => {
    const g = new RunGuard();
    g.retire();
    expect(g.isCurrent(0)).toBe(false);
  });

  it("ids are never reused, so old ids can never look current", () => {
    const g = new RunGuard();
    const ids = [g.start(), g.start(), g.start()];
    g.retire();
    ids.push(g.start());
    // Only the last started run is current.
    expect(ids.filter((id) => g.isCurrent(id))).toEqual([ids[ids.length - 1]]);
  });
});
