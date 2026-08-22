import { execSync } from "child_process";
import { AGENT_HOST, AGENT_PORT, expandTo, go, selectTreeNode, setOperation, typeOid, waitForStatus } from "../support/helpers";
import { freshWindow, setTheme, shot, statusText, writeJson } from "../support/ux";

// A1 — State screenshot pass. Every key state once per theme (dark first — the
// default — then light), saved to docs/ux/<date>/. One shared app instance:
// states are scripted in order and re-established for the light pass.
const UNKNOWN_INSTANCE_OID = "1.3.6.1.2.1.31.1.1.1.19.9999"; // noSuchInstance on the mock agent
const BAD_PORT = "11699";

interface MidStreamResult {
  captured: boolean;
  goToCompleteMs: number | null;
  statusAtShot: string | null;
}

describe("UX A1 — state screenshot pass (both themes)", function () {
  this.timeout(600000);
  const midStream: MidStreamResult = { captured: false, goToCompleteMs: null, statusAtShot: null };

  before(async () => {
    await freshWindow();
    const nc = await browser.execute(() => {
      const el = document.querySelector("[data-testid='node-count']");
      return el ? el.textContent : "";
    });
    writeJson("run-meta.json", {
      date: process.env.UX_RUN_DATE || new Date().toISOString().slice(0, 10),
      gitSha: execSync("git rev-parse HEAD", { cwd: process.cwd() }).toString().trim(),
      agent: `${AGENT_HOST}:${AGENT_PORT}`,
      nodeCount: String(nc ?? ""),
      statusAtReady: await statusText(),
    });
  });

  async function openConnectionModal() {
    await (await $("[data-testid='conn-gear']")).click();
    await (await $("[data-connection-panel]")).waitForExist({ timeout: 5000 });
  }

  /** Clicks the "SNMP V2C" version button in the open modal (version persists to config). */
  async function restoreV2c() {
    await browser.execute(() => {
      const panel = document.querySelector("[data-connection-panel]");
      const btn = Array.from(panel!.querySelectorAll("button")).find((b) => (b.textContent || "").trim() === "SNMP V2C");
      btn?.click();
    });
    await browser.pause(150);
  }

  async function closeConnectionModal() {
    const closeBtn = (await $$("[data-connection-panel] button"))[0];
    await closeBtn.click();
    await (await $("[data-connection-panel]")).waitForExist({ timeout: 5000, reverse: true });
  }

  async function walkSystem() {
    await selectTreeNode("system");
    await go("walk");
    await waitForStatus(/^walk complete: \d+ binding\(s\)$/, 60000);
  }

  async function walkIfTable() {
    await expandTo(["iso", "org", "dod", "internet", "mgmt", "mib-2", "interfaces"]);
    await selectTreeNode("ifTable");
    await go("walk");
    await waitForStatus(/^Table complete: \d+ row\(s\), \d+ column\(s\)$/, 60000);
  }

  async function warningsState() {
    // A tree selection left over would override the typed OID at Go time, so
    // start from a fresh window (this also documents that interaction).
    await freshWindow();
    // Theme persistence across reloads is unverified in this environment —
    // re-assert whichever theme this pass is shooting in.
    await setTheme(activeTheme);
    await typeOid(UNKNOWN_INSTANCE_OID);
    await go("get");
    await waitForStatus(/^Get complete: \d+ binding\(s\)$/, 30000);
  }

  async function midStreamAttempt(): Promise<void> {
    // Walk the largest cancellable subtree (mib-2) to maximize the window in
    // which "walk running" is visible, and screenshot on first sight.
    await expandTo(["iso", "org", "dod", "internet", "mgmt", "mib-2"]);
    await selectTreeNode("mib-2");
    await setOperation("walk");
    const t0 = Date.now();
    await (await $("[data-testid='go-btn']")).click();
    let sawRunning = false;
    try {
      await browser.waitUntil(
        async () => {
          const s = await statusText();
          if (/bindings\.\.\./.test(s)) {
            sawRunning = true;
            return true;
          }
          return /complete:/.test(s);
        },
        { timeout: 20000, interval: 40 },
      );
    } catch {
      // fell through to completion without a poll hitting the running state
    }
    if (sawRunning) {
      midStream.captured = true;
      midStream.statusAtShot = await statusText();
      await shot(`04-walk-running-${activeTheme}`);
    }
    const done = await waitForStatus(/complete: \d+ binding\(s\)/, 60000);
    midStream.goToCompleteMs = Date.now() - t0;
    console.log(`[ux] mid-stream ${sawRunning ? "captured" : "missed"}; walk mib-2 took ${midStream.goToCompleteMs} ms (${done})`);
    await (await $("[data-testid='clear-btn']")).click();
  }

  let activeTheme = "dark";

  async function darkPass() {
    activeTheme = "dark";
    // 01 — launch, Ready, empty results placeholder (fallback banner visible too)
    await shot("01-launch-ready-dark");

    // 02 — tree expanded to system, node selected (address bar populated)
    await expandTo(["iso", "org", "dod", "internet", "mgmt", "mib-2", "system"]);
    await selectTreeNode("sysDescr");
    await shot("02-tree-selected-dark");

    // 08 — fallback banner (BROKEN-MIB loaded via regex fallback at startup)
    await shot("08-fallback-banner-dark");

    // 03 — autocomplete dropdown open mid-typing
    await typeOid("sysdescr");
    await (await $("[data-testid='autocomplete-list']")).waitForExist({ timeout: 5000 });
    await browser.pause(200);
    await shot("03-autocomplete-open-dark");
    await (await $("[data-testid='oid-input']")).click();
    await browser.keys(["Escape"]);

    // 04 — walk running (mid-stream progress)
    await midStreamAttempt();

    // 05a/05b — Result Set list view, full and filtered
    await walkSystem();
    await shot("05a-results-list-full-dark");
    await (await $("[data-testid='filter-input']").setValue("sys"));
    await browser.pause(300);
    await shot("05b-results-list-filtered-dark");
    await (await $("[data-testid='filter-input']").setValue(""));
    await browser.pause(300);

    // 06 — grid view (ifTable)
    await walkIfTable();
    await shot("06-grid-view-dark");

    // 07 — warnings banner + partial badge (unknown OID Get)
    await warningsState();
    await shot("07-warnings-partial-dark");

    // 09/10 — connection modal, v2c and v3 field sets. The version choice is
    // persisted to config.toml on click, so restore v2c before closing or every
    // later execution in this shared session fails with AuthFailure.
    await openConnectionModal();
    await shot("09-conn-modal-v2c-dark");
    await browser.execute(() => {
      const panel = document.querySelector("[data-connection-panel]");
      const btn = Array.from(panel!.querySelectorAll("button")).find((b) => (b.textContent || "").trim() === "SNMP V3");
      btn?.click();
    });
    await browser.pause(200);
    await shot("10-conn-modal-v3-dark");
    await restoreV2c();
    await closeConnectionModal();

    // 11 — Test Connection failed state (bad port)
    await (await $("[data-testid='port-input']").setValue(BAD_PORT));
    await openConnectionModal();
    const testBtn = await $("[data-connection-panel] .btn-block");
    await testBtn.click();
    await browser.waitUntil(
      async () => ((await testBtn.getText()) ?? "").includes("Failed"),
      { timeout: 60000, interval: 500 },
    );
    await shot("11-conn-test-failed-dark");
    await closeConnectionModal();
    await (await $("[data-testid='port-input']").setValue(String(AGENT_PORT)));

    // 12a — Manage MIBs dialog
    await (await $("[data-testid='menu-file']")).click();
    await (await $("[data-testid='menu-manage-mibs']")).click();
    await (await $("[data-testid='manage-mibs-dialog']")).waitForExist({ timeout: 5000 });
    await browser.pause(300);
    await shot("12a-manage-mibs-dark");
    await (await $("[data-testid='manage-mibs-close']")).click();

    // 12b — System Log pane open
    await (await $("[data-testid='menu-view']")).click();
    await (await $("[data-testid='menu-system-log']")).click();
    await (await $("[data-testid='syslog-pane']")).waitForExist({ timeout: 5000 });
    await browser.pause(300);
    await shot("12b-syslog-pane-dark");
    await (await $("[data-testid='menu-view']")).click();
    await (await $("nav")).click();
  }

  async function lightPass() {
    activeTheme = "light";
    // Re-establish each state; results/grid/warnings persist across the theme
    // toggle, so only re-run what was cleared.
    await setTheme("light");

    // 01 — empty results placeholder again (clear the warned binding)
    await (await $("[data-testid='clear-btn']")).click();
    await shot("01-launch-ready-light");

    // 02/08 — tree selected + fallback banner
    await expandTo(["iso", "org", "dod", "internet", "mgmt", "mib-2", "system"]);
    await selectTreeNode("sysDescr");
    await shot("02-tree-selected-light");
    await shot("08-fallback-banner-light");

    // 03 — autocomplete open
    await typeOid("sysdescr");
    await (await $("[data-testid='autocomplete-list']")).waitForExist({ timeout: 5000 });
    await browser.pause(200);
    await shot("03-autocomplete-open-light");
    await (await $("[data-testid='oid-input']")).click();
    await browser.keys(["Escape"]);

    // 04 — walk running (retry; may be missed again)
    await midStreamAttempt();

    // 05a/05b — list view full + filtered
    await walkSystem();
    await shot("05a-results-list-full-light");
    await (await $("[data-testid='filter-input']").setValue("sys"));
    await browser.pause(300);
    await shot("05b-results-list-filtered-light");
    await (await $("[data-testid='filter-input']").setValue(""));
    await browser.pause(300);

    // 06 — grid view
    await walkIfTable();
    await shot("06-grid-view-light");

    // 07 — warnings + partial badge
    await warningsState();
    await shot("07-warnings-partial-light");

    // 09/10 — connection modal both field sets (restore v2c before closing)
    await openConnectionModal();
    await restoreV2c(); // fresh window may have re-seeded; ensure v2c baseline first
    await shot("09-conn-modal-v2c-light");
    await browser.execute(() => {
      const panel = document.querySelector("[data-connection-panel]");
      const btn = Array.from(panel!.querySelectorAll("button")).find((b) => (b.textContent || "").trim() === "SNMP V3");
      btn?.click();
    });
    await browser.pause(200);
    await shot("10-conn-modal-v3-light");
    await restoreV2c();
    await closeConnectionModal();

    // 11 — Test Connection failed
    await (await $("[data-testid='port-input']").setValue(BAD_PORT));
    await openConnectionModal();
    const testBtn = await $("[data-connection-panel] .btn-block");
    await testBtn.click();
    await browser.waitUntil(
      async () => ((await testBtn.getText()) ?? "").includes("Failed"),
      { timeout: 60000, interval: 500 },
    );
    await shot("11-conn-test-failed-light");
    await closeConnectionModal();
    await (await $("[data-testid='port-input']").setValue(String(AGENT_PORT)));

    // 12a/12b — Manage MIBs + System Log
    await (await $("[data-testid='menu-file']")).click();
    await (await $("[data-testid='menu-manage-mibs']")).click();
    await (await $("[data-testid='manage-mibs-dialog']")).waitForExist({ timeout: 5000 });
    await browser.pause(300);
    await shot("12a-manage-mibs-light");
    await (await $("[data-testid='manage-mibs-close']")).click();

    await (await $("[data-testid='menu-view']")).click();
    await (await $("[data-testid='menu-system-log']")).click();
    await (await $("[data-testid='syslog-pane']")).waitForExist({ timeout: 5000 });
    await browser.pause(300);
    await shot("12b-syslog-pane-light");
    await (await $("[data-testid='menu-view']")).click();
    await (await $("nav")).click();
  }

  it("captures all key states in dark theme", async () => {
    await darkPass();
  });

  it("captures all key states in light theme", async () => {
    await lightPass();
  });

  after(() => {
    writeJson("screens-midstream.json", midStream);
  });
});
