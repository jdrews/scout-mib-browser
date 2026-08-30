import { expandTo, go, selectTreeNode, setOperation, waitForAppReady, waitForStatus } from "../support/helpers";

const SYSTEM_OID = "1.3.6.1.2.1.1";

// Pinned to the committed test/mibs set: `system` owns 9 direct children
// (7 scalars + sysORLastChange + sysORTable) and the table subtree adds
// sysOREntry plus its 4 columns — 14 nodes in total.
const SYSTEM_SUBTREE_COUNT = 14;

describe("Get Subtree (MIB tree hierarchy query)", () => {
  before(async () => {
    await waitForAppReady();
    // Earlier specs leave the tree expanded, but re-expand to be safe —
    // expandTo only clicks collapsed branches.
    await expandTo(["iso", "org", "dod", "internet", "mgmt", "mib-2", "system"]);
  });

  it("lists every MIB node under the selected OID in tree order", async () => {
    await selectTreeNode("system");
    await go("getSubtree");

    expect(await waitForStatus(/^Get Subtree complete: \d+ node\(s\) under .+$/)).toBe(
      `Get Subtree complete: ${SYSTEM_SUBTREE_COUNT} node(s) under ${SYSTEM_OID}`
    );

    const rows = await $$("[data-testid='subtree-row']");
    expect(rows.length).toBe(SYSTEM_SUBTREE_COUNT);

    // First row is the first child in numeric OID order; last row is the
    // deepest leaf (sysORUpTime, sysOREntry's last column).
    const firstCells = (await rows[0].$$("div")) as WebdriverIO.Element[];
    expect((await firstCells[0].getText()) ?? "").toBe("sysDescr");
    expect((await firstCells[1].getText()) ?? "").toBe(`${SYSTEM_OID}.1`);

    const lastCells = (await rows[rows.length - 1].$$("div")) as WebdriverIO.Element[];
    expect((await lastCells[0].getText()) ?? "").toBe("sysORUpTime");
    expect((await lastCells[1].getText()) ?? "").toBe(`${SYSTEM_OID}.9.1.4`);

    // The footer reports the node count.
    const footer = (await (await $("[data-testid='results-footer']").getText())) ?? "";
    expect(footer).toContain(`${SYSTEM_SUBTREE_COUNT} of ${SYSTEM_SUBTREE_COUNT} nodes`);
  });

  it("filter narrows subtree rows", async () => {
    await (await $("[data-testid='filter-input']").setValue("sysOR"));
    await browser.pause(300);

    // sysORLastChange, sysORTable, sysOREntry, and the 4 sysOR columns.
    const footer = (await (await $("[data-testid='results-footer']").getText())) ?? "";
    expect(footer).toContain(`7 of ${SYSTEM_SUBTREE_COUNT} nodes`);
    expect((await $$("[data-testid='subtree-row']")).length).toBe(7);

    await (await $("[data-testid='filter-input']").setValue(""));
    await browser.pause(300);
    expect((await $$("[data-testid='subtree-row']")).length).toBe(SYSTEM_SUBTREE_COUNT);
  });

  it("a table node's subtree lists its entry and columns", async () => {
    await selectTreeNode("sysORTable");
    await go("getSubtree");

    expect(await waitForStatus(/^Get Subtree complete: \d+ node\(s\) under .+$/)).toBe(
      `Get Subtree complete: 5 node(s) under ${SYSTEM_OID}.9`
    );

    const rows = await $$("[data-testid='subtree-row']");
    expect(rows.length).toBe(5);
    const names: string[] = [];
    for (const r of rows) {
      const cells = (await r.$$("div")) as WebdriverIO.Element[];
      names.push((await cells[0].getText()) ?? "");
    }
    // Numeric OID order: sysORID (.9.1.2) precedes sysORDescr (.9.1.3).
    expect(names).toEqual(["sysOREntry", "sysORIndex", "sysORID", "sysORDescr", "sysORUpTime"]);
  });

  it("a leaf node has an empty subtree", async () => {
    await selectTreeNode("sysDescr");
    await go("getSubtree");

    expect(await waitForStatus(/^Get Subtree complete: \d+ node\(s\) under .+$/)).toBe(
      `Get Subtree complete: 0 node(s) under ${SYSTEM_OID}.1`
    );
    await expect(await $("[data-testid='subtree-empty']")).toBeExisting();
    expect((await $$("[data-testid='subtree-row']")).length).toBe(0);
  });

  it("clear removes the subtree result", async () => {
    await (await $("[data-testid='clear-btn']")).click();
    expect((await $$("[data-testid='subtree-row']")).length).toBe(0);
    await expect(await $("[data-testid='results-placeholder']")).toBeExisting();

    // Leave the operation on Get for later spec files.
    await setOperation("get");
  });
});
