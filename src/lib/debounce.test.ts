import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { debounce } from "./debounce";

// Target settings persisted on every keystroke (un-awaited, un-caught) —
// launch-beta/10 debounces the persist path. These pin the debounce util
// the persister is built on.

describe("debounce (launch-beta/10)", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("does not call before the window elapses", () => {
    const fn = vi.fn();
    const d = debounce(fn, 300);
    d("a");
    vi.advanceTimersByTime(299);
    expect(fn).not.toHaveBeenCalled();
  });

  it("calls once with the latest args after the window", () => {
    const fn = vi.fn();
    const d = debounce(fn, 300);
    d("a");
    d("b");
    d("c");
    vi.advanceTimersByTime(300);
    expect(fn).toHaveBeenCalledTimes(1);
    expect(fn).toHaveBeenCalledWith("c");
  });

  it("each burst gets its own call", () => {
    const fn = vi.fn();
    const d = debounce(fn, 300);
    d(1);
    vi.advanceTimersByTime(300);
    d(2);
    vi.advanceTimersByTime(300);
    expect(fn).toHaveBeenCalledTimes(2);
    expect(fn).toHaveBeenNthCalledWith(1, 1);
    expect(fn).toHaveBeenNthCalledWith(2, 2);
  });

  it("flush invokes a pending call immediately", () => {
    const fn = vi.fn();
    const d = debounce(fn, 300);
    d("x");
    d.flush();
    expect(fn).toHaveBeenCalledWith("x");
    // The pending timer was consumed: nothing fires later.
    vi.advanceTimersByTime(500);
    expect(fn).toHaveBeenCalledTimes(1);
  });

  it("cancel suppresses the pending call", () => {
    const fn = vi.fn();
    const d = debounce(fn, 300);
    d("x");
    d.cancel();
    vi.advanceTimersByTime(500);
    expect(fn).not.toHaveBeenCalled();
  });

  it("flush with nothing pending is a no-op", () => {
    const fn = vi.fn();
    const d = debounce(fn, 300);
    d.flush();
    expect(fn).not.toHaveBeenCalled();
  });
});
