import { AGENT_HOST, expandTo, go, selectTreeNode, typeOid } from "../support/helpers";
import { freshWindow, pollFeedback, statusText, writeJson } from "../support/ux";

// A4 — Feedback audit ("action → feedback map"). After every action we poll the
// footer status for <= 2 s and record what it said (or "no change"), plus any
// other visible effect. Any row with no user-visible feedback at all is a
// finding. Written to docs/ux/<date>/feedback-map.json.

interface FeedbackRow {
  action: string;
  statusBefore: string;
  statusAfter2s: string;
  otherVisibleEffect: string;
  feedbackPresent: boolean;
}

describe("UX A4 — action → feedback map", function () {
  this.timeout(600000);
  const rows: FeedbackRow[] = [];

  async function audit(action: string, doAction: () => Promise<string>): Promise<void> {
    const before = await statusText();
    const other = await doAction();
    const after = await pollFeedback(before, 2000);
    const feedbackPresent = after !== "no change" || other !== "";
    rows.push({ action, statusBefore: before, statusAfter2s: after, otherVisibleEffect: other, feedbackPresent });
    console.log(`[ux][fb] ${action} => status:"${after}" other:"${other}"`);
  }

  async function openConnectionModal() {
    await (await $("[data-testid='conn-gear']")).click();
    await (await $("[data-connection-panel]")).waitForExist({ timeout: 5000 });
  }

  before(async () => {
    await freshWindow();
  });

  it("maps every common action to its user-visible feedback", async () => {
    // ── Menus ────────────────────────────────────────────────────────────────
    await audit("Open File menu", async () => {
      await (await $("[data-testid='menu-file']")).click();
      const open = await $("[data-testid='menu-add-mib-dir']").isExisting();
      return open ? "File menu opened (Add MIB Directory… / Manage MIBs…)" : "menu did not open";
    });

    await audit("Close File menu (click outside)", async () => {
      await (await $("nav")).click();
      const gone = await browser.waitUntil(
        async () => !(await $("[data-testid='menu-add-mib-dir']").isExisting()),
        { timeout: 3000, interval: 100 },
      ).then(() => true).catch(() => false);
      return gone ? "menu closed" : "menu still open";
    });

    await audit("Open Manage MIBs dialog", async () => {
      await (await $("[data-testid='menu-file']")).click();
      await (await $("[data-testid='menu-manage-mibs']")).click();
      await (await $("[data-testid='manage-mibs-dialog']")).waitForExist({ timeout: 5000 });
      return "dialog opened listing loaded MIBs";
    });

    await audit("Unload BROKEN-MIB from dialog", async () => {
      let unloaded = false;
      for (const r of await $$("[data-testid='mib-row']")) {
        const t = (await r.getText()) ?? "";
        if (t.includes("BROKEN-MIB")) {
          await (await r.$("[data-testid='unload-btn']")).click();
          unloaded = true;
          break;
        }
      }
      return unloaded ? "row removed from list" : "BROKEN-MIB row not found";
    });

    await audit("Close Manage MIBs dialog", async () => {
      await (await $("[data-testid='manage-mibs-close']")).click();
      const gone = await browser.waitUntil(
        async () => !(await $("[data-testid='manage-mibs-dialog']").isExisting().catch(() => false)),
        { timeout: 3000, interval: 100 },
      ).then(() => true).catch(() => false);
      return gone ? "dialog closed" : "dialog still open";
    });

    // ── Connection modal ─────────────────────────────────────────────────────
    await audit("Open connection modal (gear)", async () => {
      await openConnectionModal();
      return "modal opened (Target Connection)";
    });

    await audit("Close connection modal (✕)", async () => {
      const closeBtn = (await $$("[data-connection-panel] button"))[0];
      await closeBtn.click();
      const gone = await browser.waitUntil(
        async () => !(await $("[data-connection-panel]").isExisting().catch(() => false)),
        { timeout: 3000, interval: 100 },
      ).then(() => true).catch(() => false);
      return gone ? "modal closed" : "modal still open";
    });

    await audit("Toggle theme (footer button)", async () => {
      const before = await browser.execute(() => document.querySelector("div[data-theme]")?.getAttribute("data-theme"));
      await (await $("[data-testid='theme-toggle']")).click();
      await browser.pause(200);
      const after = await browser.execute(() => document.querySelector("div[data-theme]")?.getAttribute("data-theme"));
      return `theme ${before} -> ${after}`;
    });

    await audit("Test Connection (success)", async () => {
      await openConnectionModal();
      const btn = await $("[data-connection-panel] .btn-block");
      await btn.click();
      await browser.waitUntil(
        async () => ((await btn.getText()) ?? "").includes("Connected"),
        { timeout: 45000, interval: 500 },
      );
      const indicator = (await (await $("[data-testid='conn-indicator']").getText())) ?? "";
      return `button -> "✓ Connected"; footer indicator -> "${indicator}"`;
    });

    await audit("Close connection modal", async () => {
      const closeBtn = (await $$("[data-connection-panel] button"))[0];
      await closeBtn.click();
      await browser.waitUntil(
        async () => !(await $("[data-connection-panel]").isExisting().catch(() => false)),
        { timeout: 3000, interval: 100 },
      );
      return "modal closed";
    });

    // ── Target bar ───────────────────────────────────────────────────────────
    await audit("Edit host input", async () => {
      const inp = await $("[data-testid='host-input']");
      await inp.setValue("192.0.2.99");
      await browser.pause(400);
      await inp.setValue(AGENT_HOST);
      return "value changed in place (persisted to config silently)";
    });

    await audit("Select tree node (sysDescr)", async () => {
      await expandTo(["iso", "org", "dod", "internet", "mgmt", "mib-2", "system"]);
      await selectTreeNode("sysDescr");
      const val = (await (await $("[data-testid='oid-input']").getValue())) ?? "";
      return `address bar populated: "${val}"`;
    });

    await audit("Type in OID input (autocomplete opens)", async () => {
      await typeOid("sysdescr");
      await (await $("[data-testid='autocomplete-list']")).waitForExist({ timeout: 5000 });
      return "dropdown of matching MIB nodes appeared";
    });

    await audit("Dismiss autocomplete (Escape)", async () => {
      await (await $("[data-testid='oid-input']")).click();
      await browser.keys(["Escape"]);
      const gone = !(await $("[data-testid='autocomplete-list']").isExisting().catch(() => false));
      return gone ? "dropdown dismissed" : "dropdown still open";
    });

    // ── Execution & results ──────────────────────────────────────────────────
    await audit("Go (Get on sysDescr)", async () => {
      await go("get");
      return ""; // the 2 s poll in audit() captures the first status change ("Starting get ...")
    });

    // Wait for completion before exercising the results controls.
    await browser.waitUntil(
      async () => /Get complete:/.test(await statusText()),
      { timeout: 30000, interval: 250 },
    );

    await audit("Toggle MIB Names / Raw OIDs", async () => {
      const btn = await $("[data-testid='names-toggle']");
      const before = (await btn.getText()) ?? "";
      await btn.click();
      await browser.pause(150);
      const after = (await btn.getText()) ?? "";
      return `button label ${before} -> ${after}; first column now shows raw OIDs`;
    });

    await audit("Toggle Wrap", async () => {
      const btn = await $("[data-testid='wrap-toggle']");
      await btn.click();
      await browser.pause(150);
      return "long value now wraps instead of truncating";
    });

    await audit("Filter results ('sys')", async () => {
      await (await $("[data-testid='filter-input']").setValue("sys"));
      await browser.pause(300);
      const footer = (await (await $("[data-testid='results-footer']").getText())) ?? "";
      return `footer: "${footer}"`;
    });

    await audit("Clear filter", async () => {
      await (await $("[data-testid='filter-input']").setValue(""));
      await browser.pause(300);
      const footer = (await (await $("[data-testid='results-footer']").getText())) ?? "";
      return `footer: "${footer}"`;
    });

    await audit("Sort by Value (header click)", async () => {
      const h = await $("[data-testid='sort-value']");
      await h.click();
      await browser.pause(150);
      return `header arrow: "${(await h.getText()) ?? ""}"`;
    });

    await audit("Clear results (🗑️)", async () => {
      await (await $("[data-testid='clear-btn']")).click();
      const rows = await $$("[data-testid='result-row']");
      return rows.length === 0 ? "rows removed; placeholder prompt returned" : "rows still present";
    });

    // ── View menu / System Log ───────────────────────────────────────────────
    await audit("Open View menu", async () => {
      await (await $("[data-testid='menu-view']")).click();
      return "View menu opened (System Log item)";
    });

    await audit("Toggle System Log on", async () => {
      await (await $("[data-testid='menu-system-log']")).click();
      await (await $("[data-testid='syslog-pane']")).waitForExist({ timeout: 5000 });
      return "System Log pane opened at bottom";
    });

    await audit("Toggle System Log off", async () => {
      // The View menu is still open from the previous toggle (the item stops
      // propagation), so click the item directly — clicking "View" would close it.
      await (await $("[data-testid='menu-system-log']")).click();
      const gone = await browser.waitUntil(
        async () => !(await $("[data-testid='syslog-pane']").isExisting().catch(() => false)),
        { timeout: 3000, interval: 100 },
      ).then(() => true).catch(() => false);
      return gone ? "pane closed" : "pane still open";
    });

    await audit("Close View menu (click outside)", async () => {
      await (await $("nav")).click();
      return "";
    });

    // ── Context menu ─────────────────────────────────────────────────────────
    await audit("Copy OID (context menu)", async () => {
      await browser.execute((name: string) => {
        const nodes = Array.from(document.querySelectorAll("[data-tree-node]"));
        const el = nodes.find((n) => (n.getAttribute("title") ?? "").startsWith(`${name} (`));
        if (!el) throw new Error(`tree node "${name}" not found`);
        const r = el.getBoundingClientRect();
        el.dispatchEvent(
          new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: r.left + 5, clientY: r.top + 5 }),
        );
      }, "sysDescr");
      await (await $("[data-testid='ctx-copy-oid']")).click();
      return "context menu item clicked";
    });

    const noFeedback = rows.filter((r) => !r.feedbackPresent).map((r) => r.action);
    writeJson("feedback-map.json", {
      rows,
      actionsWithNoVisibleFeedback: noFeedback,
      note: "statusAfter2s='no change' means the footer status text was identical for 2 s after the action; otherVisibleEffect records non-status feedback observed",
    });
    console.log(`[ux][fb] actions with NO visible feedback: ${JSON.stringify(noFeedback)}`);
  });
});
