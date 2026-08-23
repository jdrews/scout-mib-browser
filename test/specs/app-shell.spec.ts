import { expandTo, findTreeNode, nodeCount, waitForAppReady } from "../support/helpers";

describe("App shell (launch and layout)", () => {
  before(async () => {
    // First file to touch the UI — acts as a canary: if the agent or seeded
    // MIBs are broken, this fails loudly instead of passing silently.
    await waitForAppReady();
  });

  it("launches with correct title", async () => {
    expect(await browser.getTitle()).toBe("Scout MIB Browser");
  });

  it("renders the full shell", async () => {
    const buttons = await $$("nav button");
    const labels: string[] = [];
    for (const b of buttons) labels.push((await b.getText()) ?? "");
    expect(labels).toEqual(expect.arrayContaining(["File", "View", "Settings"]));

    await expect(await $("[data-address-bar]")).toBeExisting();
    await expect(await $("[data-testid='mib-panel-header']")).toBeExisting();
    await expect(await $("[data-testid='results-header']")).toBeExisting();
    await expect(await $("footer")).toBeExisting();
  });

  it("loads seeded MIBs on startup", async () => {
    expect(await nodeCount()).toBeGreaterThan(0);

    // The curated set reaches the standard roots down to mib-2.
    await expandTo(["iso", "org", "dod", "internet", "mgmt", "mib-2"]);
    await expect(await findTreeNode("internet")).toBeExisting();
    await expect(await findTreeNode("mib-2")).toBeExisting();
  });

  it("shows a neutral indicator before any connection attempt", async () => {
    // UX-12: the indicator is neutral at startup; red only after a real failure.
    const text = (await (await $("[data-testid='conn-indicator']").getText())) ?? "";
    expect(text).toContain("Not connected");
  });

  it("placeholder results prompt", async () => {
    await expect(await $("[data-testid='results-placeholder']")).toBeExisting();
  });

  it("document structure: single h1, landmarks, focusable scroll regions", async () => {
    const structure = await browser.execute(() => {
      const out: Record<string, unknown> = {};
      out.h1Count = document.querySelectorAll("h1").length;
      out.treeIsNav = !!document.querySelector('nav[aria-label="MIB tree"] [role="tree"]');
      out.resultsInMain = !!document.querySelector("main [data-testid='results-body']");
      const body = document.querySelector("[data-testid='results-body']") as HTMLElement | null;
      out.resultsBodyFocusable = body?.getAttribute("tabindex") === "0";
      return out;
    });
    expect(structure.h1Count).toBe(1);
    expect(structure.treeIsNav).toBe(true);
    expect(structure.resultsInMain).toBe(true);
    expect(structure.resultsBodyFocusable).toBe(true);
  });
});
