import { oidInputValue, typeOid, waitForAppReady } from "../support/helpers";

describe("Address bar (autocomplete)", () => {
  before(async () => {
    await waitForAppReady();
  });

  it("typing shows search results", async () => {
    await typeOid("sysdescr");
    await browser.pause(600); // past the 150 ms debounce

    await expect(await $("[data-testid='autocomplete-list']")).toBeExisting();

    const rows = await $$("[data-testid='autocomplete-list'] > div");
    let matched = "";
    for (const r of rows) {
      const t = (await r.getText()) ?? "";
      if (t.includes("sysDescr")) matched = t;
    }
    expect(matched).toContain("1.3.6.1.2.1.1.1");
  });

  it("keyboard navigation selects an item", async () => {
    await typeOid("sysdescr");
    await browser.pause(600);
    await expect(await $("[data-testid='autocomplete-list']")).toBeExisting();

    // ArrowDown highlights the first result, Enter selects it. (browser.keys
    // uses the W3C Actions API, which the embedded driver implements; click
    // first so the input holds DOM focus.)
    await (await $("[data-testid='oid-input']")).click();
    await browser.keys(["ArrowDown"]);
    await browser.keys(["Enter"]);

    await expect(await $("[data-testid='autocomplete-list']")).not.toBeExisting();
    expect(await oidInputValue()).toBe("1.3.6.1.2.1.1.1  sysDescr");
  });

  it("Escape dismisses the dropdown", async () => {
    await typeOid("sysdescr");
    await browser.pause(600);
    await expect(await $("[data-testid='autocomplete-list']")).toBeExisting();

    const before = await oidInputValue();
    await (await $("[data-testid='oid-input']")).click();
    await browser.keys(["Escape"]);

    await expect(await $("[data-testid='autocomplete-list']")).not.toBeExisting();
    expect(await oidInputValue()).toBe(before);
  });

  it("Go is disabled with empty input", async () => {
    await typeOid("");
    const goBtn = await $("[data-testid='go-btn']");
    await expect(goBtn).toBeExisting();
    expect(await goBtn.getAttribute("disabled")).not.toBeNull();
  });
});
