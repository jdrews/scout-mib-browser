import { AGENT_HOST, AGENT_PORT, oidInputValue } from "../support/helpers";
import { focusByTestid, freshWindow, shot, statusText, tabOrderWalk, writeJson } from "../support/ux";

// A3 — Keyboard-only task script. The full core flow (Selection → Operation →
// Execution → cancel) driven by REAL key events: typing, ArrowDown/ArrowUp,
// Enter, Escape via browser.keys (W3C Actions — verified working on this
// driver).
//
// ENVIRONMENT LIMITATION (verified 3 ways): the embedded WebKit driver accepts
// Tab key events but never moves focus on them. Focus movement is therefore
// emulated with el.focus() (a documented stand-in for Tab), and the tab order
// itself is established from a DOM-order walk of natively focusable elements
// (tabOrderWalk) — which is exactly what a keyboard user would traverse, since
// this app uses no tabindex overrides. Any step that cannot be expressed with
// keys + focus() at all is a finding.

interface KbStop {
  index: number;
  tag: string;
  testid: string | null;
  text: string;
  outlineStyle: string;
  focusVisible: boolean;
}

interface KbStep {
  step: string;
  action: string;
  result: "ok" | "fail" | "info";
  detail: string;
  screenshot?: string;
}

describe("UX A3 — keyboard-only core flow (zero mouse)", function () {
  this.timeout(600000);

  const steps: KbStep[] = [];
  let tabOrder: KbStop[] = [];
  const findings: string[] = [];

  function log(step: string, action: string, result: KbStep["result"], detail: string) {
    steps.push({ step, action, result, detail });
    console.log(`[ux][kb] ${step}: ${action} -> ${result} (${detail})`);
  }

  async function activeStop(): Promise<{ tag: string; testid: string | null; focusVisible: boolean }> {
    const info = await browser.execute(() => {
      const el = document.activeElement as HTMLElement | null;
      if (!el || el === document.body) return { tag: "body", testid: null, focusVisible: false };
      const cs = getComputedStyle(el);
      const r = el.getBoundingClientRect();
      return {
        tag: el.tagName.toLowerCase(),
        testid: el.getAttribute("data-testid"),
        focusVisible: r.width > 0 && (cs.outlineStyle !== "none" || cs.boxShadow !== "none"),
      };
    });
    return info as { tag: string; testid: string | null; focusVisible: boolean };
  }

  async function selectOp(op: string): Promise<boolean> {
    for (let i = 0; i < 6; i++) {
      const v = await browser.execute(() => (document.querySelector("[data-testid='op-select']") as HTMLSelectElement)?.value);
      if (v === op) return true;
      await browser.keys(["ArrowDown"]);
      await browser.pause(100);
    }
    return (await browser.execute(() => (document.querySelector("[data-testid='op-select']") as HTMLSelectElement)?.value)) === op;
  }

  it("runs the core flow with no mouse", async () => {
    // ── Setup: fresh window (navigation, not a mouse action) ────────────────
    await freshWindow();
    log("0", "setup: fresh window", "info", `status=${await statusText()}`);

    // ── Tab order evidence (DOM-order focusable walk; Tab key unsupported by driver) ──
    tabOrder = await tabOrderWalk(60);
    const orderStr = tabOrder.map((s) => s.testid || `${s.tag}:${s.text.slice(0, 12)}`).join(" > ");
    log("1", "tab-order walk (DOM focusables, emulated focus)", "ok", orderStr);

    const noOutline = tabOrder.filter((s) => !s.focusVisible).map((s) => s.testid || s.tag);
    if (noOutline.length) findings.push(`No visible focus indicator on: ${noOutline.join(", ")}`);

    // ── 2. Focus the OID input (Tab stand-in), type a MIB name ──────────────
    const focused = await focusByTestid("oid-input");
    if (!focused) {
      log("2", "focus oid-input", "fail", "el.focus() did not land on oid-input");
      writeJson("keyboard-log.json", { steps, tabOrder, findings });
      return;
    }
    await browser.keys(["s", "y", "s", "d", "e", "s", "c", "r"]);
    let dropdown = false;
    try {
      await (await $("[data-testid='autocomplete-list']")).waitForExist({ timeout: 3000 });
      dropdown = true;
    } catch {}
    log("2", `type "sysdescr"`, dropdown ? "ok" : "fail", dropdown ? "autocomplete dropdown appeared from typing alone" : "no dropdown within 3 s");
    await shot("kb-02-autocomplete-open");

    // ── 3. ArrowDown + Enter selects the highlighted item ────────────────────
    await browser.keys(["ArrowDown"]);
    await browser.pause(150);
    await browser.keys(["Enter"]);
    await browser.pause(300);
    const val = await oidInputValue();
    const selOk = val === "1.3.6.1.2.1.1.1  sysDescr";
    log("3", "ArrowDown, Enter (select highlighted result)", selOk ? "ok" : "fail", `address bar now: "${val}"`);
    await shot("kb-03-node-selected");

    // ── 4. Focus the Operation select (Tab stand-in), set Walk with arrows ───
    await focusByTestid("op-select");
    const walkOk = await selectOp("walk");
    log("4", "focus op-select, ArrowDown until Walk", walkOk ? "ok" : "fail", walkOk ? "operation=walk via arrow keys on native <select>" : "could not reach walk");
    await shot("kb-04-op-walk");

    // ── 5. Focus Go (Tab stand-in), check focus ring, press Enter ────────────
    await focusByTestid("go-btn");
    const goStop = await activeStop();
    if (!goStop.focusVisible) findings.push("No visible focus indicator on the Go button when focused.");
    log("5", "focus Go, Enter (start walk)", "ok", `focus ring visible=${goStop.focusVisible}`);
    await shot("kb-05-focus-go");

    // ── 6. Escape-to-cancel while focus is on the Go button ─────────────────
    await browser.keys(["Enter"]);
    let sawRunning = false;
    try {
      await browser.waitUntil(async () => /bindings\.\.\.|complete:/.test(await statusText()), { timeout: 20000, interval: 50 });
      sawRunning = /bindings\.\.\./.test(await statusText());
    } catch {}
    const st1 = await statusText();

    if (!sawRunning) {
      // The sysDescr subtree walk finished before we could act — retry against
      // the larger mib-2 subtree using a raw OID (no tree selection involved,
      // so Go uses the typed OID).
      log("6a", "walk of sysDescr subtree completed before Escape test", "info", st1);
      await freshWindow();
      tabOrder = await tabOrderWalk(60);
      await focusByTestid("oid-input");
      await browser.keys(["1", ".", "3", ".", "6", ".", "1", ".", "2", ".", "1"]);
      await browser.pause(700); // let any autocomplete settle; Enter runs Go regardless
      await focusByTestid("go-btn");
      await browser.keys(["Enter"]);
      try {
        await browser.waitUntil(async () => /bindings\.\.\./.test(await statusText()), { timeout: 30000, interval: 50 });
        sawRunning = true;
      } catch {}
    }

    if (sawRunning) {
      const focusNow = await activeStop();
      await browser.keys(["Escape"]);
      await browser.pause(1500);
      const st2 = await statusText();
      const cancelledFromCurrentFocus = st2 === "Walk cancelled";
      log("6", `Escape while focus on ${focusNow.testid || focusNow.tag} (walk running)`, cancelledFromCurrentFocus ? "ok" : "fail", `status after Escape: "${st2}"`);
      await shot("kb-06-escape-attempt");

      if (!cancelledFromCurrentFocus && focusNow.testid !== "oid-input") {
        findings.push(
          `Escape-to-cancel did nothing while focus was on ${focusNow.testid || focusNow.tag}; the Escape handler is bound to the OID input's keydown, not globally. A keyboard user who just pressed Enter on Go cannot cancel the walk without first moving focus back to the OID field.`,
        );
        // Emulate Shift+Tab x2 (go-btn -> op-select -> oid-input) and retry.
        await focusByTestid("oid-input");
        await browser.keys(["Escape"]);
        await browser.pause(1500);
        const st3 = await statusText();
        log("7", "focus back on oid-input (Shift+Tab x2 stand-in), Escape", st3 === "Walk cancelled" ? "ok" : "fail", `status: "${st3}"`);
        if (st3 === "Walk cancelled") {
          findings.push("Escape-to-cancel only works when the OID input holds focus — extra focus juggling required after pressing Go.");
        }
      }
      await shot("kb-07-after-cancel");
    } else {
      log("6", "catch a running walk to test Escape", "fail", `walk never observable in flight (status: "${st1}") — local agent answers too fast; see timings.json`);
    }

    // ── 8. Go re-enabled after cancel/completion ─────────────────────────────
    const goBtn = await $("[data-testid='go-btn']");
    const goDisabled = (await goBtn.getAttribute("disabled")) !== null;
    log("8", "Go button state after walk ends", goDisabled ? "fail" : "ok", goDisabled ? "still disabled" : "re-enabled");

    // ── Static keyboard-reachability analysis (evidence for findings) ────────
    const reach = await browser.execute(() => {
      const norm = (s: string | null) => (s ?? "").replace(/\s+/g, " ").trim();
      const out: Record<string, unknown> = {};
      // Tree nodes are role=treeitem with a roving tabindex; exactly one should
      // be tabbable at a time.
      const treeitems = Array.from(document.querySelectorAll("[role='treeitem']"));
      out.treeItems = treeitems.length;
      out.treeItemTabstops = treeitems.filter((el) => el.getAttribute("tabindex") === "0").length;
      out.treeLeafExample = treeitems[0] ? norm(treeitems[0].getAttribute("title")) : null;
      // Menu items are role=menuitem (focusable via arrow keys when open).
      out.menuItems = document.querySelectorAll("[role='menuitem']").length;
      const menuTriggers = Array.from(document.querySelectorAll("nav button[aria-haspopup='menu']"));
      out.menuTriggersWithHaspopup = menuTriggers.length;
      // Sort headers are plain <div onclick> -> not focusable.
      out.sortHeaderDivs = document.querySelectorAll("[data-testid^='sort-']").length;
      return out;
    });
    log("9", "static keyboard-reachability analysis", "info", JSON.stringify(reach));

    writeJson("keyboard-log.json", {
      environment: `Xvfb, embedded WebKitGTK driver, agent ${AGENT_HOST}:${AGENT_PORT}`,
      zeroMouse: true,
      driverLimitation:
        "Tab key events are accepted by the embedded driver but never move focus (verified via browser.keys and performActions); focus movement emulated with el.focus(). All other keys (typing, arrows, Enter, Escape) are real W3C key events.",
      steps,
      tabOrder,
      reachability: reach ?? {},
      findings,
    });
  });
});
