import { expandTo, nodeCount, waitForAppReady, waitForStatus } from "../support/helpers";

describe("Menus & settings (app-level settings)", () => {
  before(async () => {
    await waitForAppReady();
  });

  it("File menu lists MIB actions", async () => {
    await (await $("[data-testid='menu-file']")).click();
    await expect(await $("[data-testid='menu-add-mib-dir']")).toBeExisting();
    await expect(await $("[data-testid='menu-manage-mibs']")).toBeExisting();

    // Clicking outside the menu (empty nav bar area) closes it.
    await (await $("nav")).click();
    await expect(await $("[data-testid='menu-add-mib-dir']")).not.toBeExisting();
  });

  it("File and Settings menus use compact separators", async () => {
    await (await $("[data-testid='menu-file']")).click();
    expect((await $$("[data-menu='file'] li[role='separator']")).length).toBe(1);
    await (await $("nav")).click();

    await (await $("[data-testid='menu-settings']")).click();
    expect((await $$("[data-menu='settings'] li[role='separator']")).length).toBe(1);
    // The old daisyUI .divider elements (with their large default margins) are gone.
    expect((await $$("ul.menu .divider")).length).toBe(0);
    await (await $("nav")).click();
  });

  it("Manage MIBs dialog", async () => {
    await (await $("[data-testid='menu-file']")).click();
    await (await $("[data-testid='menu-manage-mibs']")).click();

    // Dialog opens and lists each seeded MIB with its node count.
    await expect(await $("[data-testid='manage-mibs-dialog']")).toBeExisting();
    for (const name of ["SNMPv2-SMI", "SNMPv2-MIB", "SNMPv2-TC", "IF-MIB", "BROKEN-MIB"]) {
      const rows = await $$("[data-testid='mib-row']");
      let found = false;
      for (const r of rows) {
        if (((await r.getText()) ?? "").includes(name)) found = true;
      }
      expect(found).toBe(true);
    }

    // Regression: DUP-MIB-A and DUP-MIB-B define the same module name. Both
    // rows must render — rows are keyed by file path, so duplicate module
    // names cannot trip Svelte's each_key_duplicate check (which would kill
    // the update flush and hang the dialog on "Loading...").
    let dupCount = 0;
    for (const r of await $$("[data-testid='mib-row']")) {
      if (((await r.getText()) ?? "").includes("DUP-MIB")) dupCount++;
    }
    expect(dupCount).toBe(2);

    const before = await nodeCount();

    // Unload IF-MIB.
    let ifRow: WebdriverIO.Element | null = null;
    let ifRowCount = 0;
    for (const r of await $$("[data-testid='mib-row']")) {
      const t = (await r.getText()) ?? "";
      if (t.includes("IF-MIB")) {
        ifRow = r;
        ifRowCount = Number(t.match(/(\d+) nodes?/)![1]);
      }
    }
    expect(ifRow).not.toBeNull();
    await (await (ifRow!.$("[data-testid='unload-btn']"))).click();
    expect(await waitForStatus(/^Unloaded IF-MIB$/, 15000)).toBe("Unloaded IF-MIB");

    // It disappears from the list.
    let stillThere = false;
    for (const r of await $$("[data-testid='mib-row']")) {
      if (((await r.getText()) ?? "").includes("IF-MIB")) stillThere = true;
    }
    expect(stillThere).toBe(false);

    // Footer node count decreased by IF-MIB's node count.
    expect(await nodeCount()).toBe(before - ifRowCount);

    // ifTable is no longer in the tree.
    await expandTo(["iso", "org", "dod", "internet", "mgmt", "mib-2"]);
    let hasIfTable = false;
    for (const n of await $$("[data-tree-node]")) {
      if (((await n.getAttribute("title")) ?? "").startsWith("ifTable (")) hasIfTable = true;
    }
    expect(hasIfTable).toBe(false);

    // Close the dialog, then restore state for later spec files: reloading the
    // window re-runs startup (config -> mibLoadDirectories), which rebuilds the
    // resolver from disk and brings IF-MIB back.
    await (await $("[data-testid='manage-mibs-close']")).click();
    await browser.url("http://localhost:5173");
    await waitForAppReady();
    expect(await nodeCount()).toBe(before);
  });

  it("View menu toggles System Log", async () => {
    await (await $("[data-testid='menu-view']")).click();
    const item = await $("[data-testid='menu-system-log']");
    await expect(item).toBeExisting();
    // No checkmark while the pane is closed.
    expect((await item.$$("svg")).length).toBe(0);

    await item.click();
    await expect(await $("[data-testid='syslog-pane']")).toBeExisting();

    // The menu stays open after the toggle (the item stops propagation) —
    // close and reopen View to inspect the checkmark.
    await (await $("[data-testid='menu-view']")).click();
    await (await $("[data-testid='menu-view']")).click();
    const item2 = await $("[data-testid='menu-system-log']");
    expect((await item2.$$("svg")).length).toBeGreaterThan(0);

    // Toggle off. The menu stays open after the toggle (stopPropagation) —
    // close it so later tests start from a clean state.
    await item2.click();
    await (await $("[data-testid='syslog-pane']")).waitForExist({ reverse: true, timeout: 5000 });
    await (await $("nav")).click();
  });

  it("Settings log level filter", async () => {
    // Open the syslog pane so filtering is observable.
    await (await $("[data-testid='menu-view']")).click();
    await (await $("[data-testid='menu-system-log']")).click();
    await expect(await $("[data-testid='syslog-pane']")).toBeExisting();

    // Settings -> System Log Level -> Error. Clicking closes the menu (nav
    // click), so reopen to inspect the active mark.
    await (await $("[data-testid='menu-settings']")).click();
    await (await $("[data-testid='log-level-error']")).click();
    await (await $("[data-testid='menu-settings']")).click();
    const errorItem = await $("[data-testid='log-level-error']");
    // Active level shows the Lucide check icon.
    expect((await errorItem.$$("svg.lucide-check")).length).toBe(1);

    // The pane is filtered to error-level entries only.
    const paneText = (await (await $("[data-testid='syslog-pane']").getText())) ?? "";
    expect(paneText).not.toMatch(/\[(INFO|WARN|DEBUG|TRACE)\]/);

    // Restore the default filter. The settings menu is still open from the
    // inspection above — clicking it again would toggle it closed.
    await (await $("[data-testid='log-level-all']")).click();
  });

  it("theme toggle", async () => {
    const shell = await $("div[data-theme]");
    const before = (await shell.getAttribute("data-theme")) ?? "";

    await (await $("[data-testid='theme-toggle']")).click();
    const after = (await shell.getAttribute("data-theme")) ?? "";
    expect(after).not.toBe(before);
    expect(["dark", "light"]).toContain(after);

    // NOTE: the app persists the theme to localStorage, but in this
    // WebKitGTK/Xvfb environment the driver's execute/sync context sees a
    // different storage instance than the app's main world (verified by
    // probe), so the persisted value is not assertable from tests.
  });
});
