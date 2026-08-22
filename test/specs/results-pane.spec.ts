import { expandTo, findTreeNode, go, resultsBodyHasText, selectTreeNode, waitForAppReady, waitForStatus } from "../support/helpers";

const SYSTEM_WALK_COUNT = 31; // pinned to the linux-full-walk.snmprec recording

describe("Results pane (result set manipulation)", () => {
  before(async () => {
    await waitForAppReady();
    // Deterministic setup: walk the system subtree once.
    await expandTo(["iso", "org", "dod", "internet", "mgmt", "mib-2", "system"]);
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

    // Default is OID ascending; switch to Value.
    await (await valueHeader).click();
    expect(((await valueHeader.getText()) ?? "")).toContain("\u2191"); // ↑

    // Values are sorted ascending.
    let rows = await $$("[data-testid='result-row']");
    const values: string[] = [];
    for (const r of rows) values.push((await ((await r.$$("div"))[1]).getText()) ?? "");
    for (let i = 1; i < values.length; i++) {
      expect(values[i - 1] <= values[i]).toBe(true);
    }

    // Click again — descending.
    await (await valueHeader).click();
    expect(((await valueHeader.getText()) ?? "")).toContain("\u2193"); // ↓
    rows = await $$("[data-testid='result-row']");
    const desc: string[] = [];
    for (const r of rows) desc.push((await ((await r.$$("div"))[1]).getText()) ?? "");
    for (let i = 1; i < desc.length; i++) {
      expect(desc[i - 1] >= desc[i]).toBe(true);
    }

    // Clicking the OID header switches the sort column.
    await (await oidHeader).click();
    expect(((await oidHeader.getText()) ?? "")).toContain("\u2191");
    expect(((await valueHeader.getText()) ?? "")).toContain("\u2195");
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

  it("Wrap toggle changes value rendering", async () => {
    // First row is sysDescr.0 — a long OctetString, truncated by default.
    let rows = await $$("[data-testid='result-row']");
    const valueCell = (await ((await rows[0]).$$("div")))[1];
    expect((await valueCell.getCSSProperty("white-space")).value).toBe("nowrap");

    await (await $("[data-testid='wrap-toggle']")).click();
    expect((await valueCell.getCSSProperty("word-break")).value).toBe("break-all");

    // Toggle back.
    await (await $("[data-testid='wrap-toggle']")).click();
    expect((await valueCell.getCSSProperty("white-space")).value).toBe("nowrap");
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
