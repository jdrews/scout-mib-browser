import {
  CHAIN_TO_SYSTEM,
  SYNTH_AGENT_PORT,
  expandTo,
  findTreeNode,
  go,
  restoreTargetPort,
  resultsBodyHasText,
  selectTreeNode,
  setTargetPort,
  typeOid,
  waitForAppReady,
  waitForStatus,
} from "../support/helpers";

const SYSTEM_WALK_COUNT = 31; // pinned to the linux-full-walk.snmprec recording

describe("Results pane (result set manipulation)", () => {
  before(async () => {
    await waitForAppReady();
    // Deterministic setup: walk the system subtree once.
    await expandTo(CHAIN_TO_SYSTEM);
    await selectTreeNode("system");
    await go("walk");
    await waitForStatus(new RegExp(`^walk complete: ${SYSTEM_WALK_COUNT} binding\\(s\\)$`), 60000);
  });

  it("filter narrows rows", async () => {
    const footer = $("[data-testid='results-footer']");
    expect(((await footer.getText()) ?? "")).toContain(`${SYSTEM_WALK_COUNT} of ${SYSTEM_WALK_COUNT} bindings`);

    await (await $("[data-testid='filter-input']").setValue("sysDescr"));
    await browser.pause(300);

    // Only the sysDescr.0 row matches; e.g. sysName.0 is hidden.
    expect(((await footer.getText()) ?? "")).toContain(`1 of ${SYSTEM_WALK_COUNT} bindings`);
    expect(await resultsBodyHasText("sysDescr.0")).toBe(true);
    expect(await resultsBodyHasText("sysName.0")).toBe(false);

    // Clearing the filter restores all rows.
    await (await $("[data-testid='filter-input']").setValue(""));
    await browser.pause(300);
    expect(((await footer.getText()) ?? "")).toContain(`${SYSTEM_WALK_COUNT} of ${SYSTEM_WALK_COUNT} bindings`);
  });

  it("sorting by column header", async () => {
    const valueHeader = $("[data-testid='sort-value']");
    const oidHeader = $("[data-testid='sort-oid']");

    // Default is OID ascending; switch to Value. Sort direction shows as a
    // Lucide arrow icon in the header.
    await (await valueHeader).click();
    expect((await valueHeader.$$("svg.lucide-arrow-up")).length).toBe(1);

    // Values are sorted ascending.
    let rows = await $$("[data-testid='result-row']");
    const values: string[] = [];
    for (const r of rows) values.push((await ((await r.$$("div"))[1]).getText()) ?? "");
    for (let i = 1; i < values.length; i++) {
      expect(values[i - 1] <= values[i]).toBe(true);
    }

    // Click again — descending.
    await (await valueHeader).click();
    expect((await valueHeader.$$("svg.lucide-arrow-down")).length).toBe(1);
    rows = await $$("[data-testid='result-row']");
    const desc: string[] = [];
    for (const r of rows) desc.push((await ((await r.$$("div"))[1]).getText()) ?? "");
    for (let i = 1; i < desc.length; i++) {
      expect(desc[i - 1] >= desc[i]).toBe(true);
    }

    // Clicking the OID header switches the sort column.
    await (await oidHeader).click();
    expect((await oidHeader.$$("svg.lucide-arrow-up")).length).toBe(1);
    expect((await valueHeader.$$("svg.lucide-arrow-up-down")).length).toBe(1);
  });

  it("MIB Names / Raw OIDs toggle", async () => {
    // Default shows resolved names; first row (OID ascending) is sysDescr.0.
    let rows = await $$("[data-testid='result-row']");
    expect(((await ((await rows[0].$$("div"))[0]).getText()) ?? "")).toBe("sysDescr.0");

    // Raw OIDs — first column shows the instance OID.
    await (await $("[data-testid='names-toggle']")).click();
    rows = await $$("[data-testid='result-row']");
    expect(((await ((await rows[0].$$("div"))[0]).getText()) ?? "")).toBe("1.3.6.1.2.1.1.1.0");

    // Toggle back restores names.
    await (await $("[data-testid='names-toggle']")).click();
    rows = await $$("[data-testid='result-row']");
    expect(((await ((await rows[0].$$("div"))[0]).getText()) ?? "")).toBe("sysDescr.0");
  });

  it("short values keep the previous single-line behavior", async () => {
    // sysName.0 = "tt" — fits on one line, so nothing clamps and a click only
    // inspects (no expand/collapse to toggle).
    await (await $("[data-testid='filter-input']")).setValue("sysName");
    await browser.pause(300);

    const rows = await $$("[data-testid='result-row']");
    expect(rows.length).toBe(1);
    const valueCell = (await ((await rows[0]).$$("div")))[1];
    const valueSpan = (await valueCell.$$("span"))[0];
    const m = await browser.execute(
      (n: HTMLElement) => ({ scroll: n.scrollHeight, client: n.clientHeight }),
      valueSpan,
    );
    expect(m.scroll).toBe(m.client);

    // Click still selects the row in the Inspector. The instance OID resolves
    // back to the base node, so the inspector shows ...1.5 (without ".0").
    await valueSpan.click();
    await browser.waitUntil(
      async () => {
        const t = (await (await $("[data-testid='inspector-oid']")).getText()) ?? "";
        return t.includes("1.3.6.1.2.1.1.5");
      },
      { timeout: 5000, interval: 100 },
    );

    await (await $("[data-testid='filter-input']")).setValue("");
    await browser.pause(300);
  });

  it("long values wrap, clamp at three lines, and expand on click", async () => {
    // Get synthLongNote.0 from the synthetic agent — a ~400-char OctetString,
    // long enough to wrap past three lines at the default pane width.
    await setTargetPort(SYNTH_AGENT_PORT);
    try {
      await typeOid("1.3.6.1.2.1.15432.1.4.0");
      await go("get");
      // Wait for the value itself — a status match could be stale text from
      // an earlier spec file (the app process persists across files).
      let rows = await $$("[data-testid='result-row']");
      await browser.waitUntil(
        async () => {
          rows = await $$("[data-testid='result-row']");
          if (rows.length !== 1) return false;
          const t = (await rows[0].getText()) ?? "";
          return t.includes("synthetic long octet string");
        },
        { timeout: 15000, interval: 200 },
      );
      expect(rows.length).toBe(1);
      const valueCell = (await ((await rows[0]).$$("div")))[1];
      const valueSpan = (await valueCell.$$("span"))[0];

      // Wraps instead of truncating: no nowrap, long runs break anywhere.
      expect((await valueSpan.getCSSProperty("white-space")).value).not.toBe("nowrap");
      expect((await valueSpan.getCSSProperty("word-break")).value).toBe("break-all");

      const heights = (el: WebdriverIO.Element) =>
        browser.execute(
          (n: HTMLElement) => ({ scroll: n.scrollHeight, client: n.clientHeight }),
          el,
        );

      // Clamped to three lines: the full content is taller than the clamped box.
      const clamped = await heights(valueSpan);
      expect(clamped.scroll).toBeGreaterThan(clamped.client);

      // Clicking the value cell expands it AND points the Inspector at the row
      // (the instance OID resolves back to the base node, ...1.4).
      await valueSpan.click();
      await browser.pause(200);
      const expanded = await heights(valueSpan);
      expect(expanded.client).toBeGreaterThanOrEqual(clamped.scroll);
      await browser.waitUntil(
        async () => {
          const t = (await (await $("[data-testid='inspector-oid']")).getText()) ?? "";
          return t.includes("1.3.6.1.2.1.15432.1.4");
        },
        { timeout: 5000, interval: 100 },
      );

      // Clicking again collapses back to the three-line clamp.
      await valueSpan.click();
      await browser.pause(200);
      const collapsed = await heights(valueSpan);
      expect(collapsed.client).toBeLessThan(clamped.scroll);
    } finally {
      // Restore the primary agent for later spec files, even on failure.
      await restoreTargetPort();
    }
  });

  it("Clear resets the Result Set", async () => {
    // Put a filter in first so we can prove it is cleared too.
    await (await $("[data-testid='filter-input']").setValue("sysDescr"));
    await browser.pause(300);

    await (await $("[data-testid='clear-btn']")).click();

    expect((await $$("[data-testid='result-row']")).length).toBe(0);
    await expect(await $("[data-testid='results-placeholder']")).toBeExisting();
    expect(((await (await $("[data-testid='filter-input']").getValue())) ?? "")).toBe("");
  });
});
