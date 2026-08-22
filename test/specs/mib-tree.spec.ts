import { expandTo, findTreeNode, oidInputValue, selectTreeNode, waitForAppReady } from "../support/helpers";

describe("MIB tree (browsing and selection)", () => {
  before(async () => {
    await waitForAppReady();
    // Earlier spec files leave subtrees expanded in the shared app instance;
    // reload for a fresh, fully-collapsed tree.
    await browser.url("http://localhost:5173");
    await waitForAppReady();
  });

  it("expands a subtree lazily", async () => {
    // Initial DOM holds only the top-level roots — not the full tree.
    const initial = await $$("[data-tree-node]");
    expect(initial.length).toBeGreaterThan(0);
    expect(initial.length).toBeLessThan(10);

    let systemVisible = false;
    for (const n of initial) {
      if (((await n.getAttribute("title")) ?? "").startsWith("system (")) systemVisible = true;
    }
    expect(systemVisible).toBe(false);

    // Expand down to the system subtree; children load on demand.
    await expandTo(["iso", "org", "dod", "internet", "mgmt", "mib-2", "system"]);
    await expect(await findTreeNode("system")).toBeExisting();
  });

  it("selecting a leaf populates the address bar", async () => {
    await selectTreeNode("sysDescr");
    // Deviation from spec: the address bar shows the node OID; the scalar
    // instance suffix (.0) is appended at query time, not in the UI.
    expect(await oidInputValue()).toBe("1.3.6.1.2.1.1.1  sysDescr");
  });

  it("context menu offers copy actions", async () => {
    // element.dispatchEvent() is not part of this WDIO build — dispatch the
    // contextmenu event directly in page context (locating the node there too,
    // to avoid cross-context element argument serialization).
    await browser.execute((name: string) => {
      const nodes = Array.from(document.querySelectorAll("[data-tree-node]"));
      const el = nodes.find((n) => (n.getAttribute("title") ?? "").startsWith(`${name} (`));
      if (!el) throw new Error(`tree node "${name}" not found`);
      const r = el.getBoundingClientRect();
      el.dispatchEvent(
        new MouseEvent("contextmenu", {
          bubbles: true,
          cancelable: true,
          clientX: r.left + 5,
          clientY: r.top + 5,
        })
      );
    }, "sysDescr");

    await expect(await $("[data-testid='ctx-copy-oid']")).toBeExisting();
    await expect(await $("[data-testid='ctx-copy-name']")).toBeExisting();

    await (await $("[data-testid='ctx-copy-oid']")).click();
    // Headless WebKit may deny clipboard writes — either outcome proves the
    // menu mechanics work (see spec Risks).
    const status = (await (await $("[data-testid='status-text']").getText())) ?? "";
    expect(status).toMatch(/^(Copied OID: .+|Failed to copy)$/);
  });
});
