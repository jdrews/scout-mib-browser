// Run-identity guard for cancellable operations (launch-beta/09).
//
// A cancelled Get/GetNext keeps its promise chain alive: the engine reports
// the cancel asynchronously, so the late result or rejection arrives after
// the user has already moved on. Applying that outcome unconditionally
// overwrites the new run's status, resets the Get Next sequence cursor, and
// clears the Result Set on the next run.
//
// Each run takes an identity at start; a run is current until a newer run
// starts (start supersedes) or Stop retires it. Stale outcomes must be
// dropped, not applied. Ids are never reused.

export class RunGuard {
  private current = 0;

  /** Begins a run, superseding any in flight, and returns its identity. */
  start(): number {
    return ++this.current;
  }

  /** Retires the current run (Stop pressed): its outcome is now stale. */
  retire(): void {
    this.current++;
  }

  /** True while `id` is still the current run. */
  isCurrent(id: number): boolean {
    return id === this.current;
  }
}
