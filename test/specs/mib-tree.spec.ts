import { expandTo, findTreeNode, oidInputValue, selectTreeNode, waitForAppReady, waitForTreeNode } from "../support/helpers";

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

  it("keyboard-only selection: arrows move focus, Enter selects", async () => {
    // Fresh collapsed tree so the visible item list is deterministic (roots only).
    await browser.url("http://localhost:5173");
    await waitForAppReady();

    const first = await browser.execute(() => {
      const items = Array.from(document.querySelectorAll("[role='treeitem']"));
      if (!items.length) throw new Error("no treeitems rendered");
      (items[0] as HTMLElement).focus();
      return { oid: items[0].getAttribute("data-oid"), title: items[0].getAttribute("title") };
    });

    // Roving tabindex: exactly one treeitem is in the tab order.
    expect(
      await browser.execute(() =>
        Array.from(document.querySelectorAll("[role='treeitem']")).filter(
          (el) => el.getAttribute("tabindex") === "0"
        ).length
      )
    ).toBe(1);

    // ArrowDown moves focus to the next visible node.
    await browser.keys(["ArrowDown"]);
    const second = await browser.execute(() => {
      const el = document.activeElement as HTMLElement | null;
      return el ? { oid: el.getAttribute("data-oid"), title: el.getAttribute("title") } : null;
    });
    expect(second?.oid).not.toBe(first.oid);

    // ArrowRight expands the focused branch (children load lazily).
    await browser.keys(["ArrowRight"]);
    await browser.pause(500);
    expect(
      await browser.execute(() => {
        const el = document.activeElement as HTMLElement | null;
        return el?.getAttribute("aria-expanded") === "true";
      })
    ).toBe(true);

    // ArrowRight again moves into the first child.
    await browser.keys(["ArrowRight"]);
    await browser.pause(300);
    const child = await browser.execute(() => {
      const el = document.activeElement as HTMLElement | null;
      return el ? el.getAttribute("data-oid") : null;
    });
    expect(child).not.toBe(second?.oid);

    // Enter selects the focused node: aria-selected and the address bar agree.
    await browser.keys(["Enter"]);
    await browser.pause(300);
    expect(
      await browser.execute(() => {
        const el = document.activeElement as HTMLElement | null;
        return el?.getAttribute("aria-selected") === "true";
      })
    ).toBe(true);
    const bar = (await oidInputValue()) ?? "";
    expect(bar.length).toBeGreaterThan(0);

    // ArrowLeft on the child moves focus back to its parent branch.
    await browser.keys(["ArrowLeft"]);
    await browser.pause(200);
    const backOnParent = await browser.execute(() => {
      const el = document.activeElement as HTMLElement | null;
      return el ? el.getAttribute("data-oid") : null;
    });
    expect(backOnParent).toBe(second?.oid);
  });

  it("context menu offers copy actions", async () => {
    // Self-contained: earlier tests may have reloaded the page and collapsed
    // the tree, so expand down to sysDescr explicitly (children load lazily).
    await expandTo(["iso", "org", "dod", "internet", "mgmt", "mib-2", "system"]);
    await waitForTreeNode("sysDescr");

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
