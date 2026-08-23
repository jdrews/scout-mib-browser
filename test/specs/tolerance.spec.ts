import { typeOid, go, waitForAppReady, waitForStatus } from "../support/helpers";

// ifCounterDiscontinuityTime column of ifXTable (IF-MIB) — present in the MIB,
// but instance .9999 is absent from the recording, so the agent answers with a
// noSuchInstance exception value.
const UNKNOWN_INSTANCE_OID = "1.3.6.1.2.1.31.1.1.1.19.9999";

describe("Tolerance (malformed data handling)", () => {
  before(async () => {
    // Fresh window for a clean baseline (results/tree state from earlier specs).
    // Note: since UX-07, a stale tree selection no longer overrides the typed
    // OID at Go time — this reload is about state hygiene, not correctness.
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

  it("fallback MIB banner uses plain language", async () => {
    const banner = await $("[data-testid='fallback-banner']");
    await expect(banner).toBeExisting();
    expect((await banner.getText()) ?? "").toContain(
      "1 MIB couldn't be fully parsed and was loaded with reduced information"
    );

    // Its System Log button opens the syslog pane.
    await (await $("[data-testid='fallback-syslog-btn']")).click();
    await expect(await $("[data-testid='syslog-pane']")).toBeExisting();
  });

  it("fallback banner dismissal is session-scoped and reversible", async () => {
    // Dismiss: banner goes away, a compact indicator appears in the header.
    await (await $("[data-testid='fallback-dismiss-btn']")).click();
    await expect(await $("[data-testid='fallback-banner']")).not.toBeExisting();
    const indicator = await $("[data-testid='fallback-indicator']");
    await expect(indicator).toBeExisting();
    expect((await indicator.getAttribute("aria-label")) ?? "").toContain("1 MIB loaded with reduced information");

    // Clicking the indicator reopens the banner.
    await indicator.click();
    await expect(await $("[data-testid='fallback-banner']")).toBeExisting();
  });
});
