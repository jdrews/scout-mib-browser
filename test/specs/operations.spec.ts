import {
  AGENT_HOST,
  expandTo,
  findTreeNode,
  go,
  oidIsGreater,
  readAppLog,
  resultsBodyHasText,
  selectTreeNode,
  waitForAppReady,
  waitForStatus,
} from "../support/helpers";

const SYSTEM_OID = "1.3.6.1.2.1.1";
// Recorded once against the pinned linux-full-walk.snmprec recording: the
// system subtree yields exactly 31 bindings (7 scalar instances + 24 sysOR cells).
const SYSTEM_WALK_COUNT = 31;

describe("Operations (executions against the mock agent)", () => {
  before(async () => {
    await waitForAppReady();
  });

  it("Get returns a Variable Binding", async () => {
    await expandTo(["iso", "org", "dod", "internet", "mgmt", "mib-2", "system"]);
    await selectTreeNode("sysDescr");
    await go("get");

    const status = await waitForStatus(/^Get complete: \d+ binding\(s\)$/);
    expect(status).toBe("Get complete: 1 binding(s)");

    // One row with the resolved name and OCTET STRING type. The queried OID is
    // the scalar instance (node OID + .0, appended at query time).
    expect(await resultsBodyHasText("sysDescr.0")).toBe(true);
    expect(await resultsBodyHasText("OCTET STRING")).toBe(true);

    const footer = (await (await $("[data-testid='results-footer']").getText())) ?? "";
    expect(footer).toContain("1 of 1 bindings");
  });

  it("Get Next returns the following binding", async () => {
    // sysDescr is still selected; GetNext on the node OID returns its first
    // instance, which is greater than the requested root.
    await go("getNext");
    const status = await waitForStatus(/^GetNext complete: \d+ binding\(s\)$/);
    expect(status).toBe("GetNext complete: 1 binding(s)");

    // Show raw OIDs so the returned OID is visible in the row.
    await (await $("[data-testid='names-toggle']")).click();
    const rows = await $$("[data-testid='result-row']");
    expect(rows.length).toBe(1);
    const oidCell = ((await rows[0].$$("div")) as WebdriverIO.Element[])[0];
    const returnedOid = (await oidCell.getText()) ?? "";
    expect(returnedOid).toBe("1.3.6.1.2.1.1.1.0");
    expect(oidIsGreater(returnedOid, SYSTEM_OID)).toBe(true);

    // Back to resolved names for later specs.
    await (await $("[data-testid='names-toggle']")).click();
  });

  it("Walk streams a subtree", async () => {
    await selectTreeNode("system");
    await go("walk");

    const status = await waitForStatus(/^walk complete: (\d+) binding\(s\)$/, 60000);
    const n = Number(status.match(/^walk complete: (\d+)/)![1]);
    expect(n).toBe(SYSTEM_WALK_COUNT);
    expect(n).toBeGreaterThan(5);

    const footer = (await (await $("[data-testid='results-footer']").getText())) ?? "";
    expect(footer).toContain(`${n} of ${n} bindings`);
  });

  it("Bulk Walk works", async () => {
    await selectTreeNode("system");
    await go("bulkWalk");

    const status = await waitForStatus(/^bulkWalk complete: (\d+) binding\(s\)$/, 60000);
    const n = Number(status.match(/^bulkWalk complete: (\d+)/)![1]);
    // The recording is deterministic — same count as the Walk above.
    expect(n).toBe(SYSTEM_WALK_COUNT);
  });

  it("Stop cancels an active walk", async () => {
    // Deviation from spec: the spec suggests ifTable, but a table node triggers
    // non-cancellable table retrieval. The slowest cancellable path is a
    // GetNext-mode Walk of the largest recorded subtree (mib-2). The local
    // agent still answers in milliseconds, so the window to catch the walk in
    // flight is small; per the spec's risk table we assert on the final state.
    const stopBtn = await $("[data-testid='stop-btn']");

    async function cancelRun(cancelVia: "stop" | "escape"): Promise<boolean> {
      await selectTreeNode("mib-2");
      await go("walk");
      let cancelled = false;
      try {
        await stopBtn.waitForExist({ timeout: 1500 });
        if (cancelVia === "stop") {
          await stopBtn.click();
        } else {
          // The Escape handler lives on the OID input — focus it first.
          await (await $("[data-testid='oid-input']")).click();
          await browser.keys(["Escape"]);
        }
        cancelled = true;
      } catch {
        // Walk finished before the cancellation could be sent.
      }

      if (cancelled) {
        expect(await waitForStatus(/^Walk cancelled$/, 15000)).toBe("Walk cancelled");
      } else {
        await waitForStatus(/complete: \d+ binding\(s\)/, 30000);
      }
      // Go is re-enabled either way.
      const goBtn = await $("[data-testid='go-btn']");
      await expect(goBtn).toBeExisting();
      expect(await goBtn.getAttribute("disabled")).toBeNull();
      return cancelled;
    }

    await cancelRun("stop");
    // Second run: Escape performs the same cancellation.
    await cancelRun("escape");
  });

  it("Go with no host is rejected", async () => {
    const logBefore = readAppLog();

    await (await $("[data-testid='host-input']").setValue(""));
    await go("get");
    expect(await waitForStatus(/^No target configured$/, 10000)).toBe("No target configured");

    // No SNMP request was sent: no new "started on" engine log line.
    await browser.pause(1000);
    const logAfter = readAppLog();
    const newLines = logAfter.slice(logBefore.length).split("\n");
    expect(newLines.some((l) => l.includes("started on"))).toBe(false);

    // Restore the host for later spec files.
    await (await $("[data-testid='host-input']").setValue(AGENT_HOST));
  });
});
