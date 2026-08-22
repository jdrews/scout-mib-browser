import { expandTo, findTreeNode, go, selectTreeNode, waitForAppReady, waitForStatus } from "../support/helpers";

// Recorded against the pinned linux-full-walk.snmprec recording: ifTable has
// 2 rows x 22 columns with no gaps.
const IF_TABLE_ROWS = 2;
const IF_TABLE_COLS = 22;

describe("Table retrieval (grid view)", () => {
  before(async () => {
    await waitForAppReady();
  });

  it("walking a table node produces the grid", async () => {
    // ifTable (1.3.6.1.2.1.2.2) is a child of the interfaces subtree, not of
    // mib-2 directly — interfaces must be expanded too.
    await expandTo(["iso", "org", "dod", "internet", "mgmt", "mib-2", "interfaces"]);
    await selectTreeNode("ifTable");
    await go("walk");

    const status = await waitForStatus(/^Table complete: \d+ row\(s\), \d+ column\(s\)$/);
    expect(status).toBe(`Table complete: ${IF_TABLE_ROWS} row(s), ${IF_TABLE_COLS} column(s)`);

    // Grid view is auto-enabled for table results.
    const grid = await $("[data-testid='grid-table']");
    await expect(grid).toBeExisting();

    // Instance column plus column headers resolved to MIB names.
    const headerText = (await (await $("thead").getText())) ?? "";
    expect(headerText).toContain("Instance");
    expect(headerText).toContain("ifIndex");
    expect(headerText).toContain("ifDescr");
  });

  it("grid footer reports row count", async () => {
    const footer = (await (await $("[data-testid='grid-footer']").getText())) ?? "";
    expect(footer).toContain(`${IF_TABLE_ROWS} of ${IF_TABLE_ROWS} rows`);
  });

  it("missing cells are flagged (conditional on recording)", async () => {
    // The pinned recording has no gaps in ifTable — assert the footer stays
    // clean. If a future recording yields gaps, missing cells render in accent
    // style and the footer shows "M missing cell(s)".
    const footer = (await (await $("[data-testid='grid-footer']").getText())) ?? "";
    expect(footer).not.toContain("missing cell");
  });

  it("filter applies to grid rows", async () => {
    // Deviation from spec: instance ids are "1"/"2", which also occur inside
    // cell values, so an instance fragment cannot uniquely narrow. Filter by a
    // value fragment ("lo") that matches exactly one row in the pinned recording.
    await (await $("[data-testid='filter-input']").setValue("lo"));
    await browser.pause(300);

    const visibleRows = await $$("[data-testid='grid-table'] tbody tr");
    expect(visibleRows.length).toBe(1);
    const footer = (await (await $("[data-testid='grid-footer']").getText())) ?? "";
    expect(footer).toContain(`1 of ${IF_TABLE_ROWS} rows`);

    // Clearing restores all rows.
    await (await $("[data-testid='filter-input']").setValue(""));
    await browser.pause(300);
    expect((await $$("[data-testid='grid-table'] tbody tr")).length).toBe(IF_TABLE_ROWS);
  });
});
