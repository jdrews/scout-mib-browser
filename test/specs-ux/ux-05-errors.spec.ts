import { AGENT_HOST, AGENT_PORT, expandTo, go, selectTreeNode, typeOid, waitForStatus } from "../support/helpers";
import { freshWindow, setTheme, shot, statusText, writeJson } from "../support/ux";

// A5 — Error-path walkthrough. Drives the known failure scenarios against the
// mock agent and captures exact wording + screenshots for scoring (what
// happened? cause named? next action suggested? domain language per CONTEXT.md?).
// Written to docs/ux/<date>/error-messages.json.
const UNKNOWN_INSTANCE_OID = "1.3.6.1.2.1.31.1.1.1.19.9999"; // noSuchInstance on the mock agent
const BAD_PORT = "11699";

describe("UX A5 — error-path wording", function () {
  this.timeout(600000);
  const captured: Record<string, unknown> = {};

  async function openConnectionModal() {
    await (await $("[data-testid='conn-gear']")).click();
    await (await $("[data-connection-panel]")).waitForExist({ timeout: 5000 });
  }

  before(async () => {
    await freshWindow();
    await setTheme("dark"); // earlier specs may have toggled the theme
  });

  it("test connection fails with an actionable message", async () => {
    await (await $("[data-testid='port-input']").setValue(BAD_PORT));
    await openConnectionModal();
    const btn = await $("[data-connection-panel] .btn-block");
    await btn.click();
    await browser.waitUntil(
      async () => ((await btn.getText()) ?? "").includes("Failed"),
      { timeout: 60000, interval: 500 },
    );

    let errMsg = "";
    for (const p of await $$("[data-connection-panel] p")) {
      const t = (await p.getText()) ?? "";
      if (t && !t.startsWith("Credentials are not persisted")) errMsg = t;
    }
    captured.testConnectionFailed = {
      button: (await btn.getText()) ?? "",
      errorMessage: errMsg,
      statusText: await statusText(),
      connIndicator: (await (await $("[data-testid='conn-indicator']").getText())) ?? "",
    };
    await shot("err-01-conn-failed");
    await (await $$("[data-connection-panel] button"))[0].click();
    await (await $("[data-connection-panel]")).waitForExist({ timeout: 5000, reverse: true });
    await (await $("[data-testid='port-input']").setValue(String(AGENT_PORT)));
  });

  it("unknown OID yields warnings banner + partial badge", async () => {
    // Fresh window so no tree selection overrides the typed OID at Go time.
    await freshWindow();
    await setTheme("dark");
    await typeOid(UNKNOWN_INSTANCE_OID);
    await go("get");
    await waitForStatus(/^Get complete: \d+ binding\(s\)$/, 30000);

    const banner = (await (await $("[data-testid='warnings-banner']").getText())) ?? "";
    const badge = (await (await $("[data-testid='partial-badge']").getText())) ?? "";
    const bodyLeaves = await browser.execute(() => {
      const body = document.querySelector("[data-testid='results-body']");
      if (!body) return [];
      return Array.from(body.querySelectorAll("*"))
        .filter((n) => n.children.length === 0)
        .map((n) => (n.textContent || "").trim())
        .filter(Boolean);
    });
    captured.unknownOid = {
      statusText: await statusText(),
      warningsBanner: banner,
      partialBadge: badge,
      resultRowCells: (bodyLeaves ?? []).slice(0, 12),
    };
    await shot("err-02-warnings-partial");
  });

  it("BROKEN-MIB fallback banner + its System Log trail", async () => {
    const banner = (await (await $("[data-testid='fallback-banner']").getText())) ?? "";
    captured.fallbackBanner = { text: banner };

    await (await $("[data-testid='fallback-syslog-btn']")).click();
    await (await $("[data-testid='syslog-pane']")).waitForExist({ timeout: 5000 });
    await browser.pause(800); // let the pane poll in log entries
    const logLines = await browser.execute(() => {
      const pane = document.querySelector("[data-testid='syslog-pane']");
      if (!pane) return [];
      return Array.from(pane.querySelectorAll("div"))
        .map((d) => (d.textContent || "").trim())
        .filter((t) => t && /BROKEN|fallback|regex/i.test(t))
        .slice(0, 10);
    });
    captured.fallbackBanner.logLines = logLines ?? [];
    await shot("err-03-fallback-syslog");

    // Close the pane again for later specs.
    await (await $("[data-testid='menu-view']")).click();
    await (await $("[data-testid='menu-system-log']")).click();
    await browser.waitUntil(
      async () => !(await $("[data-testid='syslog-pane']").isExisting().catch(() => false)),
      { timeout: 3000, interval: 100 },
    ).catch(() => {});
  });

  it("Go with no host is rejected with a message", async () => {
    await (await $("[data-testid='host-input']").setValue(""));
    await go("get");
    captured.goNoHost = { statusText: await statusText(), connIndicator: (await (await $("[data-testid='conn-indicator']").getText())) ?? "" };
    await shot("err-04-no-host");
    await (await $("[data-testid='host-input']").setValue(AGENT_HOST));
  });

  it("Test Connection with empty host: guidance message reachability", async () => {
    await (await $("[data-testid='host-input']").setValue(""));
    await openConnectionModal();
    const btn = await $("[data-connection-panel] .btn-block");
    const disabled = (await btn.getAttribute("disabled")) !== null;
    // ConnectionModal.testConnection() would set statusText to "Enter a Target host
    // to test connection" — but only if the click can land. Record whether it can.
    captured.emptyHostTest = {
      buttonDisabled: disabled,
      note: disabled
        ? "button is disabled with an empty host, so the 'Enter a Target host to test connection' guidance is unreachable; no inline hint explains why the button is dead"
        : "button clickable",
    };
    await shot("err-05-empty-host-test");
    const closeBtn = (await $$("[data-connection-panel] button"))[0];
    await closeBtn.click();
    await (await $("[data-connection-panel]")).waitForExist({ timeout: 5000, reverse: true });
    await (await $("[data-testid='host-input']").setValue(AGENT_HOST));
  });

  it("walk cancellation wording", async () => {
    await expandTo(["iso", "org", "dod", "internet", "mgmt", "mib-2"]);
    await selectTreeNode("mib-2");
    await go("walk");
    let cancelled = false;
    try {
      const stopBtn = await $("[data-testid='stop-btn']");
      await stopBtn.waitForExist({ timeout: 5000 });
      await stopBtn.click();
      cancelled = true;
    } catch {}
    if (cancelled) {
      captured.walkCancelled = { statusText: await waitForStatus(/^Walk cancelled$/, 15000) };
    } else {
      const done = await waitForStatus(/complete: \d+ binding\(s\)/, 60000);
      captured.walkCancelled = { statusText: done, note: "walk finished before Stop could be clicked (local agent too fast)" };
    }
  });

  after(() => {
    writeJson("error-messages.json", captured);
  });
});
