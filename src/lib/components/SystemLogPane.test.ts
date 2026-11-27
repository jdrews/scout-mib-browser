import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, fireEvent, cleanup } from "@testing-library/svelte";
import SystemLogPane from "./SystemLogPane.svelte";
import { S } from "$lib/stores.svelte";
import { invoke } from "@tauri-apps/api/core";

const mockedInvoke = invoke as ReturnType<typeof vi.fn>;

// axe (launch-beta/13): the pane's top-edge handle is a focusable separator —
// it must expose aria-valuenow (required) and be keyboard-operable, mirroring
// the Inspector handle (ArrowUp grows, ArrowDown shrinks).

describe("SystemLogPane resize handle (launch-beta/13)", () => {
  beforeEach(() => {
    mockedInvoke.mockReset();
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "log_read") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    S.systemLogHeight = 200;
    S.logEntries.length = 0;
  });

  afterEach(() => {
    cleanup();
  });

  it("exposes the separator value range and a label", () => {
    const { container } = render(SystemLogPane);
    const handle = container.querySelector(".resize-handle-v")!;
    expect(handle.getAttribute("role")).toBe("separator");
    expect(handle.getAttribute("aria-orientation")).toBe("horizontal");
    expect(handle.getAttribute("aria-valuenow")).toBe("200");
    expect(handle.getAttribute("aria-valuemin")).toBe("100");
    expect(handle.getAttribute("aria-valuemax")).toBe("800");
    expect(handle.getAttribute("aria-label")).toBeTruthy();
  });

  it("resizes the pane with the arrow keys", async () => {
    const { container } = render(SystemLogPane);
    const handle = container.querySelector(".resize-handle-v")!;
    // The handle is the pane's top edge: ArrowUp grows (same as dragging up).
    await fireEvent.keyDown(handle, { key: "ArrowUp" });
    expect(S.systemLogHeight).toBeGreaterThan(200);
    await fireEvent.keyDown(handle, { key: "ArrowDown" });
    expect(S.systemLogHeight).toBe(200);
  });

  it("clamps the keyboard resize to the pane bounds", async () => {
    S.systemLogHeight = 800;
    const { container } = render(SystemLogPane);
    const handle = container.querySelector(".resize-handle-v")!;
    await fireEvent.keyDown(handle, { key: "ArrowUp" });
    expect(S.systemLogHeight).toBe(800);
  });
});
