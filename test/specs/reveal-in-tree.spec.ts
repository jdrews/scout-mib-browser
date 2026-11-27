import {
  CHAIN_TO_INTERFACES,
  CHAIN_TO_SYSTEM,
  expandTo,
  findTreeNode,
  go,
  selectTreeNode,
  waitForAppReady,
  waitForStatus,
  waitForTreeNode,
} from "../support/helpers";

const SYSTEM_WALK_COUNT = 31; // pinned to the linux-full-walk.snmprec recording

// Double-click (and the row menu's Reveal) on a result row must reveal the
// row's base MIB node in the tree, expanding collapsed branches. Right-click
// on the row body must open the app's menu — leaking through showed
// WebKitGTK's dev-build menu (Back/Reload/Inspect).

/** Dispatches `type` on a cell of the first result row containing `needle`
 *  (element.dispatchEvent is not in this WDIO build — locate and dispatch in
 *  page context, the established contextmenu pattern). */
async function dispatchOnRow(needle: string, type: string, cellIndex: number): Promise<void> {
  await browser.execute(
    (t: string, evType: string, cell: number) => {
      const norm = (s: string | null) => (s ?? "").replace(/\s+/g, " ").trim();
      const rows = Array.from(document.querySelectorAll("[data-testid='result-row']"));
      const row = rows.find((r) => norm(r.textContent).includes(t));
      if (!row) throw new Error(`result row "${t}" not found among ${rows.length} mounted rows`);
      const el = (row.children[cell] ?? row) as HTMLElement;
      const r = el.getBoundingClientRect();
      el.dispatchEvent(
        new MouseEvent(evType, {
          bubbles: true,
          cancelable: true,
          clientX: r.left + 24,
          clientY: r.top + 6,
        })
      );
    },
    needle,
    type,
    cellIndex
  );
}

/** Right-click in the row's OID cell — no cell-level menu there, so the row
 *  handler owns it. */
const rightClickRow = (needle: string) => dispatchOnRow(needle, "contextmenu", 0);
const dblClickRow = (needle: string) => dispatchOnRow(needle, "dblclick", 0);

const treeRowExists = (namePrefix: string) =>
  browser.execute(
    (p: string) =>
      Array.from(document.querySelectorAll("[data-tree-node]")).some((n) =>
        (n.getAttribute("title") ?? "").startsWith(`${p} (`)
      ),
    namePrefix
  );

/** Collapse a branch via its icon toggle — a row click selects and expands
 *  but never collapses (TreeNode's onClick contract). */
async function collapseBranch(name: string): Promise<void> {
  const node = await findTreeNode(name);
  const toggle = await node.$(".tree-toggle");
  if (toggle && (await toggle.getAttribute("title")) === "Collapse") {
    await toggle.click();
  }
}

/** Collapse the mib-2 branch (it is expanded from setup) so a reveal must
 *  re-expand it, and wait until sysDescr's row is gone. */
async function collapseMib2(): Promise<void> {
  await collapseBranch("mgmt.mib-2");
  await browser.waitUntil(async () => !(await treeRowExists("sysDescr")), {
    timeout: 5000,
    interval: 100,
  });
}

describe("Reveal result in MIB Browser", () => {
  before(async () => {
    await waitForAppReady();
    await expandTo(CHAIN_TO_SYSTEM);
    await selectTreeNode("system");
    await go("walk");
    await waitForStatus(new RegExp(`^Walk complete: ${SYSTEM_WALK_COUNT} bindings$`), 60000);
  });

  it("right-click on the row body opens the app menu with Reveal", async () => {
    await rightClickRow("sysDescr.0");
    await expect(await $("[data-testid='ctx-reveal']")).toBeExisting();
    await expect(await $("[data-testid='ctx-copy-oid']")).toBeExisting();
    await expect(await $("[data-testid='ctx-copy-name']")).toBeExisting();

    // Dismiss via the document click listener.
    await browser.execute(() => {
      document.body.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    await expect(await $("[data-testid='ctx-reveal']")).not.toBeExisting();
  });

  it("double-click reveals the node, re-expanding a collapsed branch", async () => {
    await collapseMib2();

    await dblClickRow("sysDescr.0");
    await waitForTreeNode("sysDescr");
  });

  it("the menu's Reveal item reveals the node", async () => {
    await collapseMib2();

    await rightClickRow("sysDescr.0");
    await (await $("[data-testid='ctx-reveal']")).click();
    await waitForTreeNode("sysDescr");
  });

  it("double-click on a grid row reveals the table node", async () => {
    await expandTo(CHAIN_TO_INTERFACES);
    await selectTreeNode("ifTable");
    await go("getTable");
    await waitForStatus(/^Table complete: \d+ rows?, \d+ columns?$/, 60000);

    // Collapse the branch so the reveal has to walk down to the table's entry.
    await collapseBranch("interfaces");

    // Double-click the first grid row (the handler sits on the <tr>).
    await browser.execute(() => {
      const tr = document.querySelector("[data-testid='grid-table'] tbody tr");
      if (!tr) throw new Error("no grid row mounted");
      const r = tr.getBoundingClientRect();
      tr.dispatchEvent(
        new MouseEvent("dblclick", { bubbles: true, cancelable: true, clientX: r.left + 30, clientY: r.top + 6 })
      );
    });

    // Empty-folder collapse renders the run as a dot-joined row
    // ("ifTable.ifEntry …"); the reveal must bring it into view.
    await browser.waitUntil(
      async () =>
        await browser.execute(() =>
          Array.from(document.querySelectorAll("[data-tree-node]")).some((n) =>
            (n.getAttribute("title") ?? "").includes("ifEntry (")
          )
        ),
      { timeout: 15000, interval: 200, timeoutMsg: "ifEntry never revealed in the tree" }
    );
  });
});
