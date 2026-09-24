import { waitForAppReady } from "../support/helpers";

describe("MIB browser sidebar (collapse/expand)", () => {
  before(async () => {
    await waitForAppReady();
    // The app process and the browser profile persist across spec files, so
    // a previous file may have left the panel collapsed. Reset to the
    // default (open) and reload for a fresh, open panel.
    await browser.execute(() => localStorage.removeItem("scout-mib-panel-open"));
    await browser.url("http://localhost:5173");
    await waitForAppReady();
  });

  it("opens by default with a collapse button in the header", async () => {
    await expect(await $("[data-testid='mib-panel-header']")).toBeExisting();
    await expect(await $("[data-testid='mib-panel-collapse']")).toBeExisting();
    await expect(await $("[data-testid='mib-panel-rail']")).not.toBeExisting();
    // The resize handle is present while the panel is open.
    await expect(await $("[role='separator'][aria-orientation='vertical']")).toBeExisting();
  });

  it("collapses to a rail and expands back", async () => {
    await (await $("[data-testid='mib-panel-collapse']")).click();
    await expect(await $("[data-testid='mib-panel-rail']")).toBeExisting();
    await expect(await $("[data-testid='mib-panel-header']")).not.toBeExisting();
    // The tree rows and the resize handle are gone while collapsed.
    expect((await $$("[data-tree-node]")).length).toBe(0);
    await expect(await $("[role='separator'][aria-orientation='vertical']")).not.toBeExisting();
    // The rail's label says what it is.
    const railText = (await (await $("[data-testid='mib-panel-rail']").getText())) ?? "";
    expect(railText).toContain("MIB Browser");

    await (await $("[data-testid='mib-panel-expand']")).click();
    await expect(await $("[data-testid='mib-panel-header']")).toBeExisting();
    await expect(await $("[data-testid='mib-panel-rail']")).not.toBeExisting();
    await expect(await $("[role='separator'][aria-orientation='vertical']")).toBeExisting();
  });

  // NOTE: the collapsed state is persisted to localStorage (like the inspector
  // pane), but in this WebKitGTK/Xvfb environment each navigation gives the
  // webview fresh storage, so the persisted value is not assertable from e2e
  // here (same limitation as the theme note in menus-settings.spec.ts).

  it("View menu toggles the panel", async () => {
    // The item shows a Check icon (like the System Log item) while the panel
    // is open.
    const hasCheck = () =>
      browser.execute(() => {
        const a = document.querySelector("[data-testid='menu-mib-panel']");
        return a?.querySelector("svg") !== null;
      });

    // Close any menu a previous file may have left open.
    await (await $("nav")).click();
    await (await $("[data-testid='menu-view']")).click();
    await expect(await $("[data-testid='menu-mib-panel']")).toBeExisting();
    // Checked while the panel is open.
    expect(await hasCheck()).toBe(true);

    // The View menu stays open after the toggle (the item stops propagation).
    await (await $("[data-testid='menu-mib-panel']")).click();
    await expect(await $("[data-testid='mib-panel-rail']")).toBeExisting();
    expect(await hasCheck()).toBe(false);

    // Reopen via the menu for the remaining spec files.
    await (await $("[data-testid='menu-mib-panel']")).click();
    await expect(await $("[data-testid='mib-panel-header']")).toBeExisting();
    await (await $("nav")).click();
  });
});
