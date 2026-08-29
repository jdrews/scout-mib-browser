import type { TableResult } from "../../src/lib/types";
import {
  expandTo,
  go,
  restoreTargetPort,
  selectTreeNode,
  setTargetPort,
  SYNTH_AGENT_PORT,
  waitForAppReady,
  waitForStatus,
} from "../support/helpers";

// Pinned linux-full-walk.snmprec (primary agent): ifTable has 2 rows x 22
// columns with no gaps — 44 instances total under 1.3.6.1.2.1.2.2.
const IF_TABLE_ROWS = 2;
const IF_TABLE_COLS = 22;
const IF_TABLE_FLAT_BINDINGS = IF_TABLE_ROWS * IF_TABLE_COLS;

// Synthetic ifStack agent (test/snmprec/synthetic-ifstack.snmprec): a table
// with a two-component integer index and only one accessible column
// (ifStackStatus); the index objects are not-accessible.
const IFSTACK_ROWS = 600;
const GRID_CHUNK = 500;

describe("Table retrieval (Get Table)", () => {
  before(async () => {
    await waitForAppReady();
  });

  // A failed test must not leak an active filter into the next one — it would
  // hide grid rows (and flat-walk bindings) from later assertions.
  afterEach(async () => {
    const input = await $("[data-testid='filter-input']");
    if (await input.isExisting()) {
      await input.setValue("");
    }
  });

  it("Get Table on a table node produces the grid", async () => {
    // ifTable (1.3.6.1.2.1.2.2) is a child of the interfaces subtree, not of
    // mib-2 directly — interfaces must be expanded too.
    await expandTo(["iso", "org", "dod", "internet", "mgmt", "mib-2", "interfaces"]);
    await selectTreeNode("ifTable");
    await go("getTable");

    const status = await waitForStatus(/^Table complete: \d+ row\(s\), \d+ column\(s\)$/);
    expect(status).toBe(`Table complete: ${IF_TABLE_ROWS} row(s), ${IF_TABLE_COLS} column(s)`);

    // Grid view is auto-enabled for table results.
    const grid = await $("[data-testid='grid-table']");
    await expect(grid).toBeExisting();

    // Index component header (ifIndex) plus data columns resolved to MIB names.
    const headerText = (await (await $("thead").getText())) ?? "";
    expect(headerText).toContain("ifIndex");
    expect(headerText).toContain("ifDescr");
  });

  it("grid footer reports the true row count", async () => {
    const footer = (await (await $("[data-testid='grid-footer']").getText())) ?? "";
    expect(footer).toContain(`Showing ${IF_TABLE_ROWS} of ${IF_TABLE_ROWS} rows`);
    // The pinned recording has no gaps in ifTable — the footer stays clean.
    expect(footer).not.toContain("missing cell");
  });

  it("filter applies to grid rows", async () => {
    // "lo" matches exactly one row's ifDescr in the pinned recording; instance
    // ids are "1"/"2", which also occur inside cell values, so a value fragment
    // is needed to narrow uniquely.
    await (await $("[data-testid='filter-input']").setValue("lo"));
    await browser.pause(300);

    const visibleRows = await $$("[data-testid='grid-table'] tbody tr");
    expect(visibleRows.length).toBe(1);
    const footer = (await (await $("[data-testid='grid-footer']").getText())) ?? "";
    expect(footer).toContain(`Showing 1 of ${IF_TABLE_ROWS} rows (1 match filter)`);

    // Clearing restores all rows.
    await (await $("[data-testid='filter-input']").setValue(""));
    await browser.pause(300);
    expect((await $$("[data-testid='grid-table'] tbody tr")).length).toBe(IF_TABLE_ROWS);
  });

  it("Walk on a table node stays a flat walk", async () => {
    // Deliberate behavior change: only Get Table produces a grid. A plain walk
    // of the same subtree returns flat bindings.
    await go("walk");

    const status = await waitForStatus(/^walk complete: \d+ binding\(s\)$/);
    expect(status).toBe(`walk complete: ${IF_TABLE_FLAT_BINDINGS} binding(s)`);

    const footer = (await (await $("[data-testid='results-footer']").getText())) ?? "";
    expect(footer).toContain(`${IF_TABLE_FLAT_BINDINGS} of ${IF_TABLE_FLAT_BINDINGS} bindings`);

    const gridExists = await (await $("[data-testid='grid-table']")).isExisting();
    expect(gridExists).toBe(false);
  });

  it("Get Table on a non-table is rejected with guidance", async () => {
    await expandTo(["iso", "org", "dod", "internet", "mgmt", "mib-2", "system"]);
    await selectTreeNode("sysDescr");
    await go("getTable");

    expect(await waitForStatus(/is not a table/)).toBe("sysDescr is not a table — use Walk");

    const gridExists = await (await $("[data-testid='grid-table']")).isExisting();
    expect(gridExists).toBe(false);
  });

  it("Get and Get Next on a table node are rejected with guidance", async () => {
    await selectTreeNode("ifTable");

    await go("get");
    expect(await waitForStatus(/is a table/)).toBe(
      "ifTable is a table — use Get Table or Walk"
    );

    await go("getNext");
    expect(await waitForStatus(/is a table/)).toBe(
      "ifTable is a table — use Get Table or Walk"
    );
  });

  it("column selection applies to the next Get Table run", async () => {
    // Start from a full grid.
    await go("getTable");
    await waitForStatus(
      new RegExp(`^Table complete: ${IF_TABLE_ROWS} row\\(s\\), ${IF_TABLE_COLS} column\\(s\\)$`),
    );

    // Uncheck ifAdminStatus in the Columns… panel. The embedded driver's
    // synthetic .click() does not reliably fire Svelte's onchange, so set the
    // property and dispatch change explicitly (same pattern as setOperation).
    await (await $("[data-testid='columns-btn']")).click();
    const toggled = await browser.execute(() => {
      const panel = document.querySelector("[data-testid='columns-panel']");
      if (!panel) return false;
      const label = Array.from(panel.querySelectorAll("label")).find((l) =>
        (l.textContent ?? "").includes("ifAdminStatus"),
      );
      const input = label?.querySelector("input") as HTMLInputElement | null;
      if (!input) return false;
      input.checked = !input.checked;
      input.dispatchEvent(new Event("change", { bubbles: true }));
      return true;
    });
    expect(toggled).toBe(true);

    // The selection is a display filter: it applies on the next run. Wait for
    // the NEW count specifically — the previous run's status still matches the
    // generic pattern and would satisfy a stale read.
    await go("getTable");
    const status = await waitForStatus(
      new RegExp(`^Table complete: ${IF_TABLE_ROWS} row\\(s\\), ${IF_TABLE_COLS - 1} column\\(s\\)$`),
    );
    expect(status).toBe(`Table complete: ${IF_TABLE_ROWS} row(s), ${IF_TABLE_COLS - 1} column(s)`);

    const headerText = (await (await $("thead").getText())) ?? "";
    expect(headerText).not.toContain("ifAdminStatus");

    // Restore the default (all columns) so later specs see a clean state. A
    // new result closes the panel, so reopen it first.
    await (await $("[data-testid='columns-btn']")).click();
    await browser.execute(() => {
      const panel = document.querySelector("[data-testid='columns-panel']");
      if (!panel) return;
      const label = Array.from(panel.querySelectorAll("label")).find((l) =>
        (l.textContent ?? "").includes("ifAdminStatus"),
      );
      const input = label?.querySelector("input") as HTMLInputElement | null;
      if (!input || input.checked) return;
      input.checked = true;
      input.dispatchEvent(new Event("change", { bubbles: true }));
    });
  });
});

describe("Table retrieval (multi-component index, synthetic agent)", () => {
  before(async () => {
    await setTargetPort(SYNTH_AGENT_PORT);
  });

  after(async () => {
    await restoreTargetPort();
  });

  it("retrieves a two-index table as a grid", async () => {
    // ifStackTable (1.3.6.1.2.1.31.1.2) — INDEX { ifStackHigherLayer, ifStackLowerLayer }.
    await expandTo(["iso", "org", "dod", "internet", "mgmt", "mib-2", "ifMIB", "ifMIBObjects"]);
    await selectTreeNode("ifStackTable");
    await go("getTable");

    // Only ifStackStatus is accessible — the index objects are not-accessible.
    // Wait for this run's specific count: a stale "Table complete" status from
    // the previous describe would satisfy the generic pattern.
    const status = await waitForStatus(
      new RegExp(`^Table complete: ${IFSTACK_ROWS} row\\(s\\), 1 column\\(s\\)$`),
    );
    expect(status).toBe(`Table complete: ${IFSTACK_ROWS} row(s), 1 column(s)`);
  });

  it("renders each index component as its own column in index-correct order", async () => {
    const headerText = (await (await $("thead").getText())) ?? "";
    expect(headerText).toContain("ifStackHigherLayer");
    expect(headerText).toContain("ifStackLowerLayer");

    // Walk order must be numeric per index component, not lexicographic: the
    // higher=1 block holds lower values 1, 31, 61, ... (20 rows), so row 2 is
    // (1, 31) — a string sort would put "121" there — and row 21 starts the
    // higher=2 block at (2, 2). td[0] is "#".
    const rows = await browser.execute(() => {
      const trs = Array.from(document.querySelectorAll("[data-testid='grid-table'] tbody tr"));
      const pick = (tr: Element | undefined) => {
        if (!tr) return null;
        const tds = Array.from(tr.querySelectorAll("td"));
        return [tds[1]?.textContent?.trim() ?? "", tds[2]?.textContent?.trim() ?? ""];
      };
      return { first: pick(trs[0]), second: pick(trs[1]), twentyFirst: pick(trs[20]) };
    });
    expect(rows.first).toEqual(["1", "1"]);
    expect(rows.second).toEqual(["1", "31"]);
    expect(rows.twentyFirst).toEqual(["2", "2"]);
  });

  it("chunks rendering while the footer keeps the true total", async () => {
    let rows = await $$("[data-testid='grid-table'] tbody tr");
    expect(rows.length).toBe(GRID_CHUNK);
    let footer = (await (await $("[data-testid='grid-footer']").getText())) ?? "";
    expect(footer).toContain(`Showing ${GRID_CHUNK} of ${IFSTACK_ROWS} rows`);

    // Scrolling the sentinel into view loads the remaining rows.
    await browser.execute(() => {
      const body = document.querySelector("[data-testid='results-body']");
      if (body) body.scrollTop = body.scrollHeight;
    });
    await browser.waitUntil(
      async () => (await $$("[data-testid='grid-table'] tbody tr")).length === IFSTACK_ROWS,
      { timeout: 15000, timeoutMsg: "grid never rendered all rows after scrolling" },
    );
    footer = (await (await $("[data-testid='grid-footer']").getText())) ?? "";
    expect(footer).toContain(`Showing ${IFSTACK_ROWS} of ${IFSTACK_ROWS} rows`);
  });

  it("sorts index columns numerically", async () => {
    // Two clicks on the higher-layer header: ascending, then descending.
    const th = await $("[data-grid-col='idx:0']");
    await th.click();
    await browser.pause(300);
    await th.click();
    await browser.pause(300);

    // Descending puts the higher=30 block first; its first row is (30, 30).
    const firstRow = await browser.execute(() => {
      const tr = document.querySelector("[data-testid='grid-table'] tbody tr");
      if (!tr) return null;
      const tds = Array.from(tr.querySelectorAll("td"));
      return [tds[1]?.textContent?.trim() ?? "", tds[2]?.textContent?.trim() ?? ""];
    });
    expect(firstRow).toEqual(["30", "30"]);
  });

  it("Stop cancels a Get Table run", async () => {
    await selectTreeNode("ifStackTable");
    await go("getTable");

    // Wait until the run is visibly in flight, then cancel it.
    await waitForStatus(/^Get Table: \d+ bindings\.\.\.$/);
    await (await $("[data-testid='stop-btn']")).click();

    const status = await waitForStatus(/Table retrieval cancelled/);
    expect(status).toBe("Table retrieval cancelled");
  });

  it("decodes an Integer + IpAddress index and flags missing cells", async () => {
    // synthIpTable (SYNTH-TABLE-MIB) — INDEX { synthIpRow, synthIpAddr };
    // synthIpNote is absent on row 7 in the recording.
    await expandTo(["iso", "org", "dod", "internet", "mgmt", "mib-2", "synthTableMib", "synthObjects"]);
    await selectTreeNode("synthIpTable");
    await go("getTable");

    const status = await waitForStatus(
      /^Table complete: 12 row\(s\), 2 column\(s\) \(1 missing cell\(s\)\)$/,
    );
    expect(status).toBe("Table complete: 12 row(s), 2 column(s) (1 missing cell(s))");

    const headerText = (await (await $("thead").getText())) ?? "";
    expect(headerText).toContain("synthIpRow");
    expect(headerText).toContain("synthIpAddr");

    // td order: #, synthIpRow, synthIpAddr, synthIpStatus, synthIpNote.
    const rows = await browser.execute(() => {
      const trs = Array.from(document.querySelectorAll("[data-testid='grid-table'] tbody tr"));
      if (trs.length !== 12) return null;
      const pick = (tr: Element) =>
        Array.from(tr.querySelectorAll("td")).map((td) => td.textContent?.trim() ?? "");
      return { first: pick(trs[0]), seventh: pick(trs[6]) };
    });
    expect(rows).not.toBeNull();
    expect(rows!.first.slice(1, 3)).toEqual(["1", "10.0.1.1"]);
    // Row 7's note cell is missing — flagged in place.
    expect(rows!.seventh[4]).toContain("missing");
  });

  it("renders an IMPLIED index component as a blank column", async () => {
    // synthImpTable — INDEX { synthImpKey, IMPLIED synthImpIp }; the address is
    // absent from the instance OID entirely.
    await selectTreeNode("synthImpTable");
    await go("getTable");

    const status = await waitForStatus(/^Table complete: 5 row\(s\), 1 column\(s\)$/);
    expect(status).toBe("Table complete: 5 row(s), 1 column(s)");

    const headerText = (await (await $("thead").getText())) ?? "";
    expect(headerText).toContain("synthImpKey");
    expect(headerText).toContain("synthImpIp");

    // First row: key "1"; the implied address is absent from the instance
    // OID, so its cell renders as a dash with an "(implied)" tooltip.
    const firstRow = await browser.execute(() => {
      const tr = document.querySelector("[data-testid='grid-table'] tbody tr");
      if (!tr) return null;
      const tds = Array.from(tr.querySelectorAll("td"));
      return {
        key: tds[1]?.textContent?.trim() ?? "",
        implied: tds[2]?.textContent?.trim() ?? "",
        title: tds[2]?.getAttribute("title") ?? "",
      };
    });
    expect(firstRow).toEqual({ key: "1", implied: "—", title: "(implied)" });
  });

  it("grid JSON export produces the spec'd shape", async () => {
    // The save dialog is native and unreachable from the webview driver, so the
    // serializer is exercised directly (same code path handleExport uses). A
    // grid must be showing for the export menu to exist — retrieve one.
    await selectTreeNode("ifStackTable");
    await go("getTable");
    await waitForStatus(/^Table complete: \d+ row\(s\), 1 column\(s\)$/);

    const { gridToJson } = await import("../../src/lib/export");
    const result: TableResult = {
      table_oid: "1.3.6.1.2.1.15432.1.1",
      columns: ["1.3.6.1.2.1.15432.1.1.1.3", "1.3.6.1.2.1.15432.1.1.1.4"],
      rows: [
        {
          instance_id: "1.10.0.1.1",
          cells: {
            "1.3.6.1.2.1.15432.1.1.1.3": { value: { oid: "", value: { Integer: 1 } }, missing: false },
            "1.3.6.1.2.1.15432.1.1.1.4": { value: undefined, missing: true },
          },
          index_values: ["1", "10.0.1.1"],
        },
      ],
      total_rows: 1,
      missing_cells: 1,
      partial: false,
    };
    const json = JSON.parse(gridToJson(result, (oid) => `name-${oid.split(".").pop()}`));
    expect(json.table_oid).toBe("1.3.6.1.2.1.15432.1.1");
    expect(json.columns).toEqual([
      { oid: "1.3.6.1.2.1.15432.1.1.1.3", name: "name-3" },
      { oid: "1.3.6.1.2.1.15432.1.1.1.4", name: "name-4" },
    ]);
    expect(json.rows).toEqual([
      {
        instance_id: "1.10.0.1.1",
        cells: { "1.3.6.1.2.1.15432.1.1.1.3": "1", "1.3.6.1.2.1.15432.1.1.1.4": null },
      },
    ]);

    // The grid's export menu offers the three formats.
    await (await $("[data-testid='save-btn']")).click();
    const menuText = (await (await $("[data-export-menu]")).getText()) ?? "";
    expect(menuText).toContain("Save as TSV");
    expect(menuText).toContain("Save as JSON");
    expect(menuText).toContain("Save as CSV");
  });
});
