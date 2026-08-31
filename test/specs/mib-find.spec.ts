import { findTreeNode, oidInputValue, waitForAppReady } from "../support/helpers";

/** OID of the currently highlighted (find-hit) row, or null. */
async function highlightedOid(): Promise<string | null> {
  return await browser.execute(
    () => document.querySelector("[data-tree-node].find-hit")?.getAttribute("data-oid") ?? null
  );
}

/** Text of the "n/m" (or "No matches") count label. */
async function findCountText(): Promise<string> {
  return (await (await $("[data-testid='mib-find-count']")).getText()) ?? "";
}

/** Waits until the count label reads exactly `text`. */
async function waitForCount(text: string): Promise<void> {
  await browser.waitUntil(async () => (await findCountText()) === text, {
    timeout: 10000,
    interval: 100,
    timeoutMsg: `find count never read "${text}"`,
  });
}

/** Waits until a find-hit row is rendered (reveal expands branches lazily). */
async function waitForHighlight(oid?: string): Promise<void> {
  await browser.waitUntil(
    async () => {
      const cur = await highlightedOid();
      return oid ? cur === oid : cur !== null;
    },
    { timeout: 20000, interval: 200, timeoutMsg: `find highlight never appeared${oid ? ` for ${oid}` : ""}` }
  );
}

async function openFind(): Promise<void> {
  await (await $("[data-testid='mib-find-toggle']")).click();
  await expect(await $("[data-testid='mib-find-bar']")).toBeExisting();
}

/** Clicks the find input (so it holds DOM focus) and sets its value. */
async function typeFind(query: string): Promise<void> {
  const input = await $("[data-testid='mib-find-input']");
  await input.click();
  await input.setValue(query);
}

/** Reloads the page for a fresh, fully-collapsed tree (frontend state resets). */
async function freshTree(): Promise<void> {
  await browser.url("http://localhost:5173");
  await waitForAppReady();
}

describe("MIB tree find", () => {
  before(async () => {
    await waitForAppReady();
  });

  after(async () => {
    // Leave the shared app instance with find closed for later spec files.
    const barOpen = await browser.execute(
      () => !!document.querySelector("[data-testid='mib-find-bar']")
    );
    if (barOpen) await (await $("[data-testid='mib-find-toggle']")).click();
  });

  it("the header icon opens and closes the find bar", async () => {
    await freshTree();

    await expect(await $("[data-testid='mib-find-bar']")).not.toBeExisting();
    await (await $("[data-testid='mib-find-toggle']")).click();
    await expect(await $("[data-testid='mib-find-bar']")).toBeExisting();
    await expect(await $("[data-testid='mib-find-input']")).toBeExisting();

    // Clicking the icon again hides the bar.
    await (await $("[data-testid='mib-find-toggle']")).click();
    await expect($("[data-testid='mib-find-bar']")).not.toBeExisting();
  });

  it("finds by name in a fully collapsed tree and expands to the hit", async () => {
    await freshTree();
    // Fully collapsed: only the top-level roots are rendered.
    const before = await $$("[data-tree-node]");
    expect(before.length).toBeGreaterThan(0);
    expect(before.length).toBeLessThan(10);

    await openFind();
    await typeFind("ifType");
    await waitForCount("1/1");

    // Reveal expands the collapsed branch down to the hit and highlights it.
    await waitForHighlight("1.3.6.1.2.1.2.2.1.3");
    const after = await $$("[data-tree-node]");
    expect(after.length).toBeGreaterThan(before.length);

    // The matched substring is marked like a text-search hit.
    const mark = await browser.execute(
      () => document.querySelector("[data-tree-node].find-hit mark.find-mark")?.textContent ?? null
    );
    expect(mark).toBe("ifType");
  });

  it("Enter steps to the next finding, Shift+Enter back", async () => {
    await freshTree();

    await openFind();
    await typeFind("if");
    let count = "";
    await browser.waitUntil(async () => /^\d+\/\d+$/.test((count = await findCountText())), {
      timeout: 10000,
      interval: 100,
    });
    const denom = Number(count.split("/")[1]);
    expect(denom).toBeGreaterThan(1);

    await waitForHighlight();
    const firstOid = await highlightedOid();
    expect(firstOid).not.toBeNull();

    // Every rendered entry whose name contains the query is marked, not just
    // the current hit: the chain to ifNumber renders "interfaces" (a sibling
    // branch), which also carries a mark.
    const marks = await browser.execute(() => {
      const all = Array.from(document.querySelectorAll("[data-tree-node] mark.find-mark"));
      return {
        total: all.length,
        onNonHitRow: all.some(
          (m) => !m.closest("[data-tree-node]")?.classList.contains("find-hit")
        ),
      };
    });
    expect(marks.total).toBeGreaterThan(1);
    expect(marks.onNonHitRow).toBe(true);

    // Enter → 2/N on a different row.
    await browser.keys(["Enter"]);
    await browser.waitUntil(async () => (await highlightedOid()) !== firstOid, {
      timeout: 10000,
      interval: 200,
    });
    expect(await findCountText()).toBe(`2/${denom}`);

    // Previous-match button → back to 1/N on the original row. (The embedded
    // driver doesn't deliver modifier+key combos with shiftKey set, so the
    // Shift+Enter shortcut is exercised by the button equivalent instead.)
    await (await $("[data-testid='mib-find-prev']")).click();
    await browser.waitUntil(async () => (await highlightedOid()) === firstOid, {
      timeout: 10000,
      interval: 200,
    });
    expect(await findCountText()).toBe(`1/${denom}`);
  });

  it("finds by OID number", async () => {
    await freshTree();

    await openFind();
    await typeFind("1.3.6.1.2.1.2.2.1.3");
    await waitForCount("1/1");
    await waitForHighlight("1.3.6.1.2.1.2.2.1.3");

    // Ancestors expanded on the way down stay visible (find never collapses).
    await expect(await findTreeNode("ifEntry")).toBeExisting();
    await expect(await findTreeNode("ifTable")).toBeExisting();
  });

  it("closing removes the highlight but keeps the tree where you left it", async () => {
    await freshTree();

    await openFind();
    await typeFind("ifType");
    await waitForHighlight("1.3.6.1.2.1.2.2.1.3");

    const visibleBefore = (await $$("[data-tree-node]")).length;
    const barBefore = await oidInputValue();

    // Close via the header icon.
    await (await $("[data-testid='mib-find-toggle']")).click();
    await expect($("[data-testid='mib-find-bar']")).not.toBeExisting();

    // Highlight gone, nothing collapsed, selection untouched.
    expect(
      await browser.execute(() => document.querySelectorAll("[data-tree-node].find-hit").length)
    ).toBe(0);
    expect((await $$("[data-tree-node]")).length).toBe(visibleBefore);
    expect(await oidInputValue()).toBe(barBefore);
  });

  it("a query with no hits shows a message and highlights nothing", async () => {
    await freshTree();

    await openFind();
    await typeFind("zzz-no-such-node");
    await waitForCount("No matches");
    expect(await highlightedOid()).toBeNull();
  });
});
