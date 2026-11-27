import {
  restoreTargetPort,
  setTargetPort,
  go,
  waitForAppReady,
  waitForStatus,
  typeOid,
} from "../support/helpers";

// A port with no SNMP agent: every request fails fast with a port-unreachable
// refusal and the engine climbs its retry/backoff ladder (1+2+4 s sleeps), so
// an in-flight Get/GetNext runs for many seconds — long enough to press Stop.
const DEAD_PORT = 11698;

const OID = "1.3.6.1.2.1.1.1.0";

describe("Cancellation (launch-beta/08)", () => {
  before(async () => {
    await browser.url("http://localhost:5173");
    await waitForAppReady();
    await setTargetPort(DEAD_PORT);
  });

  after(async () => {
    await restoreTargetPort();
  });

  it("Stop cancels an in-flight GetNext promptly", async () => {
    await typeOid(OID);
    await go("getNext");

    // The Stop button replaces Go while the run is cancellable.
    const stop = await $("[data-testid='stop-btn']");
    await stop.waitForExist({ timeout: 5000 });
    await stop.click();

    // Cancelled at the first backoff boundary — well inside the ~15 s ladder.
    await waitForStatus(/cancelled/i, 5000);
  });

  it("Stop cancels an in-flight Get and reports it as a cancel, not an error", async () => {
    await typeOid(OID);
    await go("get");

    const stop = await $("[data-testid='stop-btn']");
    await stop.waitForExist({ timeout: 5000 });
    await stop.click();

    const status = await waitForStatus(/cancelled/i, 5000);
    expect(status).not.toMatch(/^Error:/);
  });

  it("the result set survives a cancelled GetNext sequence", async () => {
    // Point back at the live agent: the first Get Next answers, then the
    // target flips to the dead port for the continuation, which gets stopped.
    // The kept binding must remain in the Result Set.
    await restoreTargetPort();
    await typeOid(OID);
    await go("getNext");
    await waitForStatus(/^Get Next complete: 1 binding$/, 15000);

    await setTargetPort(DEAD_PORT);
    // The bar now holds the returned OID; a second Get Next continues there.
    await go("getNext");
    const stop = await $("[data-testid='stop-btn']");
    await stop.waitForExist({ timeout: 5000 });
    await stop.click();
    const status = await waitForStatus(/cancelled/i, 5000);
    expect(status).not.toMatch(/^Error:/);

    // The cancel discarded nothing: the binding from the first run is still
    // rendered in the Result Set (a cancelled continuation never replaces it).
    // Load-dependent timing: the engine's cancelled Ok can land before Stop's
    // handler retires the run (retire drops it) — both paths must keep the row.
    // The PR #167 CI flake was the Ok landing first with a typed-OID fresh
    // start, whose run-start clear wiped the row; the UI now restores the
    // pre-run Result Set on a cancel that kept nothing (TargetBar).
    //
    // The flat view is virtualized: rows mount on the virtualizer's
    // ResizeObserver cycle, which lags the status flush that ends the run, so
    // reading the body the moment the status lands can see the sticky header
    // alone — the CI signature (`Received string: "  OID  Value  Type"`).
    // Wait for the row to mount, then assert its contents (same pattern as
    // address-bar.spec).
    const mounted = await browser
      .waitUntil(async () => (await $$("[data-testid='result-row']")).length > 0, {
        timeout: 5000,
        interval: 100,
      })
      .then(() => true)
      .catch(() => false);
    if (!mounted) {
      const dbg = await browser.execute(() => {
        const body = document.querySelector("[data-testid='results-body']");
        return body ? body.innerHTML.slice(0, 1500) : "(no results-body element)";
      });
      throw new Error(`result row never mounted after the cancel: ${dbg}`);
    }
    const kept = await browser.execute(() => {
      const body = document.querySelector("[data-testid='results-body']");
      return body ? body.textContent ?? "" : "";
    });
    expect(kept).toContain("sysObjectID");
  });
});
