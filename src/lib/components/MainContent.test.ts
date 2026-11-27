import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, fireEvent, cleanup } from "@testing-library/svelte";
import MainContent from "./MainContent.svelte";
import { S } from "$lib/stores.svelte";
import { invoke } from "@tauri-apps/api/core";

const mockedInvoke = invoke as ReturnType<typeof vi.fn>;

// axe (launch-beta/13): a focusable separator (tabindex + role="separator")
// must expose aria-valuenow, and a resizable split is only keyboard-operable
// if the handle moves the pane. The Inspector handle is the established
// pattern (label + keydown resize); the MIB panel handle had neither.

describe("MainContent resize handle (launch-beta/13)", () => {
  beforeEach(() => {
    mockedInvoke.mockReset();
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "mib_tree") return Promise.resolve([]);
      if (cmd === "mib_oid_name_map") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    S.mibPanelOpen = true;
    S.manageMibsOpen = false;
    S.mibPanelWidth = 300;
  });

  afterEach(() => {
    cleanup();
  });

  it("exposes the separator value range and a label", () => {
    const { container } = render(MainContent);
    const handle = container.querySelector(".resize-handle-h")!;
    expect(handle.getAttribute("role")).toBe("separator");
    expect(handle.getAttribute("aria-orientation")).toBe("vertical");
    expect(handle.getAttribute("aria-valuenow")).toBe("300");
    expect(handle.getAttribute("aria-valuemin")).toBe("200");
    expect(handle.getAttribute("aria-valuemax")).toBe("600");
    expect(handle.getAttribute("aria-label")).toBeTruthy();
  });

  it("resizes the panel with the arrow keys", async () => {
    const { container } = render(MainContent);
    const handle = container.querySelector(".resize-handle-h")!;
    await fireEvent.keyDown(handle, { key: "ArrowRight" });
    expect(S.mibPanelWidth).toBeGreaterThan(300);
    await fireEvent.keyDown(handle, { key: "ArrowLeft" });
    expect(S.mibPanelWidth).toBe(300);
  });

  it("clamps the keyboard resize to the panel bounds", async () => {
    S.mibPanelWidth = 200;
    const { container } = render(MainContent);
    const handle = container.querySelector(".resize-handle-h")!;
    await fireEvent.keyDown(handle, { key: "ArrowLeft" });
    expect(S.mibPanelWidth).toBe(200);
  });

  it("sits inside a landmark (axe region)", () => {
    // The named handle is page content: between the MIB panel nav and the
    // results main, the split container must itself be a landmark.
    const { container } = render(MainContent);
    const handle = container.querySelector(".resize-handle-h")!;
    const landmark = handle.closest(
      "main, nav, aside, header, footer, form, section[aria-label], [role='main'], [role='region'], [role='search'], [role='navigation'], [role='complementary']",
    );
    expect(landmark).not.toBeNull();
  });
});
