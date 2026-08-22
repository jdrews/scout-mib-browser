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

  it("shows disconnected state before any test connection", async () => {
    const text = (await (await $("[data-testid='conn-indicator']").getText())) ?? "";
    expect(text).toContain("Disconnected");
  });

  it("placeholder results prompt", async () => {
    await expect(await $("[data-testid='results-placeholder']")).toBeExisting();
  });
});
