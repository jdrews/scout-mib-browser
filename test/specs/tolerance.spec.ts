import { typeOid, go, waitForAppReady, waitForStatus } from "../support/helpers";

// ifCounterDiscontinuityTime column of ifXTable (IF-MIB) — present in the MIB,
// but instance .9999 is absent from the recording, so the agent answers with a
// noSuchInstance exception value.
const UNKNOWN_INSTANCE_OID = "1.3.6.1.2.1.31.1.1.1.19.9999";

describe("Tolerance (malformed data handling)", () => {
  before(async () => {
    // Fresh window: a tree selection left by an earlier spec would override
    // the typed OID when Go is clicked. Reloading clears it.
    await browser.url("http://localhost:5173");
    await waitForAppReady();
  });

  it("unknown OID produces warnings, not a crash", async () => {
    await typeOid(UNKNOWN_INSTANCE_OID);
    await go("get");

    // The Get itself "completes" — with one warned binding.
    expect(await waitForStatus(/^Get complete: \d+ binding\(s\)$/)).toBe("Get complete: 1 binding(s)");

    // Warnings banner shows kind + message; partial badge is shown.
    const banner = await $("[data-testid='warnings-banner']");
    await expect(banner).toBeExisting();
    const bannerText = (await banner.getText()) ?? "";
    expect(bannerText).toContain("no-such-instance");
    expect(bannerText).toContain("No such instance currently exists at this OID on the Target");

    await expect(await $("[data-testid='partial-badge']")).toBeExisting();

    // The app remains responsive — status bar shows a completed state.
    const status = (await (await $("[data-testid='status-text']").getText())) ?? "";
    expect(status).toMatch(/^Get complete: 1 binding\(s\)$/);
  });

  it("regex-fallback MIB banner", async () => {
    const banner = await $("[data-testid='fallback-banner']");
    await expect(banner).toBeExisting();
    expect((await banner.getText()) ?? "").toContain("1 MIB(s) loaded via regex fallback");

    // Its System Log button opens the syslog pane.
    await (await $("[data-testid='fallback-syslog-btn']")).click();
    await expect(await $("[data-testid='syslog-pane']")).toBeExisting();
  });
});
