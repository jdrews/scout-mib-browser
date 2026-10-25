import { CHAIN_TO_MIB2, expandTo, nodeCount, waitForAppReady, waitForStatus } from "../support/helpers";

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

  // ── Manage MIBs page (full-page view, Results Pane table pattern) ────────

  /** Reads the visible table rows as (name, node count) pairs. */
  async function readMibRows(): Promise<{ name: string; count: number }[]> {
    const rows = await $$("[data-testid='mib-row']");
    const out: { name: string; count: number }[] = [];
    for (const r of rows) {
      const tds = await r.$$("td");
      const name = ((await tds[1].getText()) ?? "").trim();
      const countText = (await (await r.$("[data-testid='mib-node-count']")).getText()) ?? "";
      out.push({ name, count: Number(countText.replace(/\D/g, "")) });
    }
    return out;
  }

  async function openManageMibsPage(): Promise<void> {
    await (await $("[data-testid='menu-file']")).click();
    await (await $("[data-testid='menu-manage-mibs']")).click();
    await (await $("[data-testid='manage-mibs-page']")).waitForExist({ timeout: 15000 });
    // Wait for the list to finish loading (the footer renders immediately,
    // so the rows are the load-complete signal).
    await browser.waitUntil(
      async () => (await $$("[data-testid='mib-row']")).length > 0,
      { timeout: 15000, interval: 250 },
    );
  }

  it("Manage MIBs page opens and lists seeded MIBs", async () => {
    await openManageMibsPage();

    // Lists each seeded MIB (directory files + the explicit file).
    const rows = await readMibRows();
    for (const name of ["SNMPv2-SMI", "SNMPv2-MIB", "SNMPv2-TC", "IF-MIB", "BROKEN-MIB", "EXTRA-MIB"]) {
      expect(rows.some((r) => r.name === name)).toBe(true);
    }
    // MIBs that define OIDs carry a positive node count. (SNMPv2-TC is
    // textual-conventions only, so it legitimately indexes 0 nodes.)
    for (const name of ["SNMPv2-SMI", "SNMPv2-MIB", "IF-MIB", "BROKEN-MIB", "EXTRA-MIB"]) {
      const row = rows.find((r) => r.name === name);
      expect(row!.count).toBeGreaterThan(0);
    }

    // Regression: DUP-MIB-A and DUP-MIB-B define the same module name. Both
    // rows must render — rows are keyed by file path, so duplicate module
    // names cannot trip Svelte's each_key_duplicate check (which would kill
    // the update flush and hang the page on "Loading...").
    expect(rows.filter((r) => r.name === "DUP-MIB").length).toBe(2);

    // BROKEN-MIB carries the FALLBACK badge.
    let brokenRow: WebdriverIO.Element | null = null;
    for (const r of await $$("[data-testid='mib-row']")) {
      if (((await r.getText()) ?? "").includes("BROKEN-MIB")) brokenRow = r;
    }
    expect(brokenRow).not.toBeNull();
    expect((await (brokenRow!.$$("span.badge"))).length).toBe(1);

    // Footer: nothing selected, node count matches the app footer.
    const footer = (await (await $("[data-testid='mibs-footer']")).getText()) ?? "";
    expect(footer).toMatch(/^0 of \d+ MIBs selected · \d+ nodes loaded$/);

    // Back returns to the MIB Browser view.
    await (await $("[data-testid='back-btn']")).click();
    await (await $("[data-testid='manage-mibs-page']")).waitForExist({ reverse: true, timeout: 5000 });
  });

  it("Manage MIBs page sorts by Name and Nodes", async () => {
    await openManageMibsPage();
    const defaults = await readMibRows();
    expect(defaults.length).toBeGreaterThan(0);

    const byName = (a: { name: string; count: number }, b: { name: string; count: number }) =>
      a.name.toLowerCase().localeCompare(b.name.toLowerCase());
    const minName = defaults.reduce((m, r) => (byName(r, m) < 0 ? r : m));
    const maxName = defaults.reduce((m, r) => (byName(r, m) > 0 ? r : m));
    const minCount = defaults.reduce((m, r) => (r.count < m.count ? r : m));
    const maxCount = defaults.reduce((m, r) => (r.count > m.count ? r : m));

    // Name header: click = ascending, click again = descending.
    await (await $("[data-testid='sort-name']")).click();
    let sorted = await readMibRows();
    expect(sorted[0].name.toLowerCase()).toBe(minName.name.toLowerCase());
    expect(sorted[sorted.length - 1].name.toLowerCase()).toBe(maxName.name.toLowerCase());
    await (await $("[data-testid='sort-name']")).click();
    sorted = await readMibRows();
    expect(sorted[0].name.toLowerCase()).toBe(maxName.name.toLowerCase());
    expect(sorted[sorted.length - 1].name.toLowerCase()).toBe(minName.name.toLowerCase());

    // Nodes header: numeric compare, ascending then descending.
    await (await $("[data-testid='sort-nodes']")).click();
    sorted = await readMibRows();
    expect(sorted[0].count).toBe(minCount.count);
    expect(sorted[sorted.length - 1].count).toBe(maxCount.count);
    await (await $("[data-testid='sort-nodes']")).click();
    sorted = await readMibRows();
    expect(sorted[0].count).toBe(maxCount.count);
    expect(sorted[sorted.length - 1].count).toBe(minCount.count);

    // A third click clears back to the default (backend) order.
    await (await $("[data-testid='sort-nodes']")).click();
    sorted = await readMibRows();
    expect(sorted.map((r) => r.name)).toEqual(defaults.map((r) => r.name));
  });

  it("Manage MIBs page filters", async () => {
    // The page is still open from the sort case.
    const filter = await $("[data-testid='mibs-filter-input']");
    const all = await readMibRows();

    // "fallback" isolates the broken MIB.
    await filter.setValue("fallback");
    let rows = await readMibRows();
    expect(rows.length).toBe(1);
    expect(rows[0].name).toBe("BROKEN-MIB");

    // No-match state.
    await filter.setValue("zzz-no-such-mib");
    await expect(await $("[data-testid='mibs-no-match']")).toBeExisting();

    // Clearing restores the full list.
    await filter.setValue("");
    expect((await readMibRows()).length).toBe(all.length);
  });

  it("Manage MIBs page select-all + Unload Selected", async () => {
    // The page is still open from the filter case, unfiltered.
    const before = await nodeCount();
    const all = await readMibRows();

    // Select-all selects every visible row.
    await (await $("[data-testid='select-all-mibs']")).click();
    const footer = (await (await $("[data-testid='mibs-footer']")).getText()) ?? "";
    expect(footer).toMatch(new RegExp(`^${all.length} of ${all.length} MIBs selected`));

    // Unload Selected removes them; the footer node count drops by the sum of
    // the distinct modules' node counts (duplicate-module rows share one
    // module's count, so dedup by module name).
    const distinctSum = Object.values(
      Object.fromEntries(all.map((r) => [r.name, r.count])),
    ).reduce((a, b) => a + b, 0);

    await (await $("[data-testid='unload-selected-btn']")).click();
    expect(await waitForStatus(/^Unloaded \d+ MIBs$/, 30000)).toMatch(/^Unloaded \d+ MIBs$/);

    // The list is empty and the app footer shows no loaded nodes.
    await browser.waitUntil(
      async () => (await $$("[data-testid='mib-row']")).length === 0,
      { timeout: 15000, interval: 250 },
    );
    const nodeCountText = (await (await $("[data-testid='node-count']")).getText()) ?? "";
    expect(nodeCountText).toBe("");
    expect(before).toBe(distinctSum);
  });

  it("Manage MIBs page Unload All + reload restores", async () => {
    // Reload the window: startup re-runs (config -> mibLoad), rebuilding the
    // resolver from disk (directories + explicit files).
    await browser.url("http://localhost:5173");
    await waitForAppReady();
    expect(await nodeCount()).toBeGreaterThan(0);

    await openManageMibsPage();
    await (await $("[data-testid='unload-all-btn']")).click();
    expect(await waitForStatus(/^Unloaded all MIBs$/, 30000)).toBe("Unloaded all MIBs");
    await browser.waitUntil(
      async () => (await $$("[data-testid='mib-row']")).length === 0,
      { timeout: 15000, interval: 250 },
    );

    // Reload again: the explicit file (EXTRA-MIB) persisted in config and is
    // loaded alongside the directory MIBs.
    await browser.url("http://localhost:5173");
    await waitForAppReady();
    expect(await nodeCount()).toBeGreaterThan(0);
    await openManageMibsPage();
    const rows = await readMibRows();
    expect(rows.some((r) => r.name === "EXTRA-MIB")).toBe(true);
  });

  it("Manage MIBs page Add dropdown", async () => {
    // The page is still open from the previous case.
    await (await $("[data-testid='add-menu-btn']")).click();
    await expect(await $("[data-testid='add-dir-item']")).toBeExisting();
    await expect(await $("[data-testid='add-files-item']")).toBeExisting();

    // Restore state for later spec files: reload re-runs startup and brings
    // the MIBs back.
    await browser.url("http://localhost:5173");
    await waitForAppReady();
    expect(await nodeCount()).toBeGreaterThan(0);
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
