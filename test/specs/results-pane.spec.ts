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
    await waitForStatus(new RegExp(`^Walk complete: ${SYSTEM_WALK_COUNT} bindings$`), 60000);
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
    // Re-query the header fresh for every operation: the sort click makes
    // Svelte re-render the header (the arrow is a conditional child), and a
    // reference taken before the re-render can go stale in the webview
    // driver, where queries on the detached node keep seeing the old icon.
    const header = (col: string) => $(`[data-testid='sort-${col}']`);

    // The header icons and the virtualized row list re-render on separate
    // frames, so every check below waits for the rendered state it asserts on
    // instead of assuming the click's effect is already in the DOM.
    const rowValues = async () => {
      const rs = await $$("[data-testid='result-row']");
      const out: string[] = [];
      for (const r of rs) out.push((await ((await r.$$("div"))[1]).getText()) ?? "");
      return out;
    };
    const isSorted = (v: string[], ascending: boolean) =>
      v.every((x, i) => i === 0 || (ascending ? v[i - 1] <= x : v[i - 1] >= x));

    // Default is OID ascending; switch to Value. Sort direction shows as a
    // Lucide arrow icon in the header.
    await (await header("value")).click();
    await expect(await (await header("value")).$("svg.lucide-arrow-up")).toBeExisting();
    await browser.waitUntil(async () => isSorted(await rowValues(), true), {
      timeout: 5000,
      interval: 100,
    });

    // Click again — descending.
    await (await header("value")).click();
    await expect(await (await header("value")).$("svg.lucide-arrow-down")).toBeExisting();
    await browser.waitUntil(async () => isSorted(await rowValues(), false), {
      timeout: 5000,
      interval: 100,
    });

    // Clicking the OID header switches the sort column; the now-inactive
    // Value header drops its arrow entirely (I2: active column only).
    await (await header("oid")).click();
    await expect(await (await header("oid")).$("svg.lucide-arrow-up")).toBeExisting();
    // The value header's icon lags the OID one by a frame — wait for it
    // explicitly; a bare length check raced the Svelte update in CI.
    await (await (await header("value")).$("svg")).waitForExist({ reverse: true, timeout: 5000 });
  });

  it("MIB Names / Raw OIDs toggle", async () => {
    // Default shows resolved names; first row (OID ascending) is sysDescr.0.
    // The previous test ended on the OID header, but the virtualized list can
    // re-sort a frame late, so wait for the expected order first.
    let rows = await $$("[data-testid='result-row']");
    await browser.waitUntil(async () => {
      rows = await $$("[data-testid='result-row']");
      return ((await ((await rows[0].$$("div"))[0]).getText()) ?? "") === "sysDescr.0";
    }, { timeout: 5000, interval: 100 });
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
