import { execFileSync } from "child_process";
import { expandTo, go, selectTreeNode, waitForAppReady, waitForStatus } from "../support/helpers";

/**
 * Sends a REAL mouse-wheel notch to the X server via XTEST on the display
 * inherited from xvfb-run. Untrusted WheelEvents dispatched from JS never
 * trigger the browser's default scroll behavior, so only hardware-equivalent
 * input exercises the actual scroll chain (the bug under test).
 */
function xwheel(x: number, y: number, direction: "up" | "down", notches = 1, label = ""): void {
  const button = direction === "up" ? 4 : 5;
  const py = `
from Xlib import X, display as xd
from Xlib.ext import xtest
d = xd.Display()
xtest.fake_input(d, X.MotionNotify, x=${x}, y=${y})
d.sync()
q = d.screen().root.query_pointer()
print(f"pointer at root_x={q.root_x} root_y={q.root_y}")
for _ in range(${notches}):
    xtest.fake_input(d, X.ButtonPress, detail=${button})
    xtest.fake_input(d, X.ButtonRelease, detail=${button})
d.sync()
`;
  const out = execFileSync("python3", ["-c", py], { stdio: "pipe" }).toString().trim();
  if (label) console.log(`[syslog-scroll] xwheel ${label} @(${x},${y}): ${out}`);
  // The X server clamps pointer warps to the screen edge. If we didn't land
  // where we aimed, every scroll assertion below would measure the wrong pane.
  const m = out.match(/root_x=(\d+) root_y=(\d+)/);
  if (!m || Math.abs(Number(m[1]) - x) > 1 || Math.abs(Number(m[2]) - y) > 1) {
    throw new Error(
      `X pointer did not land at (${x},${y}) — got "${out}". The Xvfb screen is too small for the 1200x800 window.`,
    );
  }
}

interface ScrollState {
  logTop: number;
  logMax: number;
  resultsTop: number;
  resultsMax: number;
}

const LOG_SEL = "[data-testid='syslog-pane'] [aria-label='System log entries']";

/** Reads scroll positions of the log container and the results body. */
async function scrollState(): Promise<ScrollState> {
  const s = await browser.execute((sel: string) => {
    const log = document.querySelector(sel) as HTMLElement | null;
    const results = document.querySelector("[data-testid='results-body']") as HTMLElement | null;
    return {
      logTop: log ? log.scrollTop : -1,
      logMax: log ? log.scrollHeight - log.clientHeight : -1,
      resultsTop: results ? results.scrollTop : -1,
      resultsMax: results ? results.scrollHeight - results.clientHeight : -1,
    };
  }, LOG_SEL);
  return s as ScrollState;
}

/** Page-space center of the log entries container (window is at 0,0 in Xvfb). */
async function logCenter(): Promise<{ x: number; y: number }> {
  const r = await browser.execute((sel: string) => {
    const log = document.querySelector(sel) as HTMLElement | null;
    if (!log) return null;
    const b = log.getBoundingClientRect();
    return { x: Math.round(b.left + b.width / 2), y: Math.round(b.top + b.height / 2) };
  }, LOG_SEL);
  if (!r) throw new Error("syslog entries container not found");
  return r as { x: number; y: number };
}

/** What the page sees at the given point — verifies the X pointer landed where we think. */
async function elementAt(x: number, y: number): Promise<{ tag: string; inSyslog: boolean }> {
  const r = await browser.execute((px: number, py: number) => {
    const el = document.elementFromPoint(px, py);
    if (!el) return { tag: "none", inSyslog: false };
    return {
      tag: `${el.tagName.toLowerCase()}.${(el.getAttribute("data-testid") || "").slice(0, 24)}`,
      inSyslog: !!el.closest("[data-testid='syslog-pane']"),
    };
  }, x, y);
  return r as { tag: string; inSyslog: boolean };
}

/** Fingerprint of the rendered window. The pane caps at 200 rows, so once
 *  full, fresh entries shift the window instead of growing it: the oldest row
 *  drops out (first-row timestamp changes) and the newest lands last. Row
 *  messages can repeat between operations ("Get completed ..."), so the
 *  millisecond timestamps in both ends are what make this unique. */
async function logWindowFingerprint(): Promise<string> {
  const t = await browser.execute((sel: string) => {
    const log = document.querySelector(sel) as HTMLElement | null;
    if (!log || !log.firstElementChild) return "";
    const first = (log.firstElementChild.textContent ?? "").slice(0, 40);
    const last = log.lastElementChild?.textContent ?? "";
    return `${first}|${last}`;
  }, LOG_SEL);
  return (t as string) ?? "";
}

/** Wheels in `direction` until `ok` holds (or gives up). Returns the final state. */
async function wheelUntil(
  c: { x: number; y: number },
  direction: "up" | "down",
  ok: (s: ScrollState) => boolean,
  maxNotches = 80,
): Promise<ScrollState> {
  let s = await scrollState();
  for (let i = 0; i < maxNotches && !ok(s); i++) {
    xwheel(c.x, c.y, direction, 1);
    await browser.pause(60);
    s = await scrollState();
  }
  return s;
}

describe("System log mouse scrolling", () => {
  before(async () => {
    await waitForAppReady();
  });

  it("wheel over the log pane scrolls the log (not the results)", async () => {
    // Spec files share one app session; a previous file may leave the pane
    // open, which would make the View-menu toggle below CLOSE it. Normalize.
    if (await (await $("[data-testid='syslog-pane']")).isExisting()) {
      await (await $("[data-testid='menu-view']")).click();
      await (await $("[data-testid='menu-system-log']")).click();
      await (await $("[data-testid='syslog-pane']")).waitForExist({ reverse: true, timeout: 5000 });
    }
    // The View menu stays open after the toggle (the item stops propagation),
    // and a previous file may have left another menu open — close everything.
    await (await $("nav")).click();

    // Open the system log via the View menu.
    await (await $("[data-testid='menu-view']")).click();
    await (await $("[data-testid='menu-system-log']")).click();
    await expect(await $("[data-testid='syslog-pane']")).toBeExisting();

    // A walk fills both the results pane and the log, like in the bug report.
    await expandTo(["iso", "org", "dod", "internet", "mgmt", "mib-2", "system"]);
    await selectTreeNode("system");
    await go("walk");
    await waitForStatus(/^walk complete: \d+ binding\(s\)$/);

    // Wait until the log actually has more content than fits (scrollable).
    await browser.waitUntil(
      async () => {
        const s = await scrollState();
        return s.logMax > 0;
      },
      { timeout: 15000, interval: 250, timeoutMsg: "log never became scrollable" },
    );

    const c = await logCenter();
    const at = await elementAt(c.x, c.y);
    console.log(`[syslog-scroll] pointer target: (${c.x},${c.y}) -> ${at.tag} inSyslog=${at.inSyslog}`);
    expect(at.inSyslog).toBe(true);

    // CONTROL: wheel over the RESULTS body. If XTEST input reaches WebKit,
    // this must scroll the results — proving the input path before we blame
    // the log pane.
    const rc = await browser.execute(() => {
      const r = document.querySelector("[data-testid='results-body']") as HTMLElement;
      const b = r.getBoundingClientRect();
      return { x: Math.round(b.left + b.width / 2), y: Math.round(b.top + b.height / 2) };
    });
    await browser.execute(() => {
      const r = document.querySelector("[data-testid='results-body']") as HTMLElement;
      r.scrollTop = 0;
    });
    xwheel(rc.x, rc.y, "down", 3, "control-results");
    await browser.pause(400);
    const ctrl = await scrollState();
    console.log(`[syslog-scroll] control (results): results=${ctrl.resultsTop}/${ctrl.resultsMax}`);
    expect(ctrl.resultsTop).toBeGreaterThan(0);

    // Establish an interior reading position (well off the bottom) by wheeling up.
    const interior = await wheelUntil(c, "up", (s) => s.logTop < s.logMax / 2);
    console.log(`[syslog-scroll] interior: log=${interior.logTop}/${interior.logMax} results=${interior.resultsTop}/${interior.resultsMax}`);
    expect(interior.logTop).toBeLessThan(interior.logMax / 2);

    // Wheel DOWN from there: the log must scroll, the results must not move.
    const before = interior;
    xwheel(c.x, c.y, "down", 3, "log-down");
    await browser.pause(400);
    const afterDown = await scrollState();
    console.log(`[syslog-scroll] after down: log=${afterDown.logTop}/${afterDown.logMax} results=${afterDown.resultsTop}/${afterDown.resultsMax}`);
    expect(afterDown.logTop).toBeGreaterThan(before.logTop);
    expect(afterDown.resultsTop).toBe(before.resultsTop);

    // Wheel UP again: the user can scroll back toward older entries.
    xwheel(c.x, c.y, "up", 3, "log-up");
    await browser.pause(400);
    const afterUp = await scrollState();
    console.log(`[syslog-scroll] after up:   log=${afterUp.logTop}/${afterUp.logMax} results=${afterUp.resultsTop}/${afterUp.resultsMax}`);
    expect(afterUp.logTop).toBeLessThan(afterDown.logTop);
    expect(afterUp.resultsTop).toBe(before.resultsTop);
  });

  it("new entries do not yank the log back to the bottom while reading", async () => {
    // The pane is open and scrollable from the previous test. Read older
    // entries by wheeling up until we are well off the bottom.
    const c = await logCenter();
    const reading = await wheelUntil(c, "up", (s) => s.logTop < s.logMax / 2);
    console.log(`[syslog-scroll] reading at: log=${reading.logTop}/${reading.logMax}`);
    expect(reading.logTop).toBeLessThan(reading.logMax / 2);

    // Trigger fresh log activity (a Get produces engine log lines) and wait
    // for the pane's poller to pick it up.
    const winBefore = await logWindowFingerprint();
    await go("get");
    await waitForStatus(/^Get complete: \d+ binding\(s\)$/);

    const stayed = await browser.waitUntil(
      async () => {
        if ((await logWindowFingerprint()) === winBefore) return false; // poller has not run yet
        const s = await scrollState();
        // ... and the user's reading position was preserved.
        return s.logTop === reading.logTop;
      },
      { timeout: 8000, interval: 250, timeoutMsg: "log position not preserved after new entries" },
    );
    expect(stayed).toBe(true);

    // Back at the bottom, fresh entries DO follow (live tail behavior).
    const atBottom = await wheelUntil(c, "down", (s) => s.logTop === s.logMax);
    console.log(`[syslog-scroll] at bottom:  log=${atBottom.logTop}/${atBottom.logMax}`);
    expect(atBottom.logTop).toBe(atBottom.logMax);

    const winBefore2 = await logWindowFingerprint();
    await go("get");
    await waitForStatus(/^Get complete: \d+ binding\(s\)$/);

    const followed = await browser.waitUntil(
      async () => {
        if ((await logWindowFingerprint()) === winBefore2) return false; // poller has not run yet
        const s = await scrollState();
        return s.logTop === s.logMax; // still pinned to the bottom
      },
      { timeout: 8000, interval: 250, timeoutMsg: "log did not follow new entries while at bottom" },
    );
    expect(followed).toBe(true);
  });

  it("wheel over the log scrolls the log even with a large result set", async () => {
    // Regression: with a big flat result set, WebKitGTK registered a phantom
    // scrollable region for the results pane that extended over the syslog
    // area below it — wheeling there scrolled the RESULTS instead of the log
    // (and only in the horizontal band above the results column). A large
    // walk is required to trigger it.
    await selectTreeNode("mib-2");
    await go("walk");
    await waitForStatus(/^walk complete: \d+ binding\(s\)$/);

    const s0 = await scrollState();
    expect(s0.resultsMax).toBeGreaterThan(5000); // large enough to trigger the bug

    // A syslog point that sits horizontally over the results column — exactly
    // where the phantom region used to steal the wheel.
    const p = await browser.execute(() => {
      const syslog = document.querySelector("[data-testid='syslog-pane']") as HTMLElement;
      const results = document.querySelector("[data-testid='results-body']") as HTMLElement;
      const s = syslog.getBoundingClientRect();
      const rb = results.getBoundingClientRect();
      return { x: Math.round(Math.min(rb.left + 60, s.right - 20)), y: Math.round(s.top + s.height / 2) };
    });
    const at = await elementAt(p.x, p.y);
    console.log(`[syslog-scroll] big-walk target: (${p.x},${p.y}) -> ${at.tag} inSyslog=${at.inSyslog}`);
    expect(at.inSyslog).toBe(true);

    // Reach an interior reading position by wheeling up over that point.
    const interior = await wheelUntil(p, "up", (s) => s.logTop < s.logMax / 2);
    console.log(`[syslog-scroll] big-walk interior: log=${interior.logTop}/${interior.logMax} results=${interior.resultsTop}/${interior.resultsMax}`);
    expect(interior.logTop).toBeLessThan(interior.logMax / 2);

    // Wheel down: the log scrolls, the results do not move.
    xwheel(p.x, p.y, "down", 3, "big-walk-down");
    await browser.pause(400);
    const afterDown = await scrollState();
    console.log(`[syslog-scroll] big-walk down:   log=${afterDown.logTop}/${afterDown.logMax} results=${afterDown.resultsTop}/${afterDown.resultsMax}`);
    expect(afterDown.logTop).toBeGreaterThan(interior.logTop);
    expect(afterDown.resultsTop).toBe(interior.resultsTop);

    // Wheel up again: back toward older entries, results still unmoved.
    xwheel(p.x, p.y, "up", 3, "big-walk-up");
    await browser.pause(400);
    const afterUp = await scrollState();
    console.log(`[syslog-scroll] big-walk up:     log=${afterUp.logTop}/${afterUp.logMax} results=${afterUp.resultsTop}/${afterUp.resultsMax}`);
    expect(afterUp.logTop).toBeLessThan(afterDown.logTop);
    expect(afterUp.resultsTop).toBe(interior.resultsTop);
  });
});

describe("System log open/close controls", () => {
  before(async () => {
    await waitForAppReady();
  });

  it("footer button toggles the pane", async () => {
    const toggle = await $("[data-testid='syslog-toggle']");
    // The scrolling specs above leave the pane open; normalize to closed first.
    if (await (await $("[data-testid='syslog-pane']")).isExisting()) {
      await toggle.click();
      await (await $("[data-testid='syslog-pane']")).waitForExist({ reverse: true, timeout: 5000 });
    }

    // Closed -> open; the button reflects state via aria-pressed.
    expect(await toggle.getAttribute("aria-pressed")).toBe("false");
    await toggle.click();
    await expect(await $("[data-testid='syslog-pane']")).toBeExisting();
    expect(await (await $("[data-testid='syslog-toggle']")).getAttribute("aria-pressed")).toBe("true");

    // Open -> closed from the footer.
    await (await $("[data-testid='syslog-toggle']")).click();
    await (await $("[data-testid='syslog-pane']")).waitForExist({ reverse: true, timeout: 5000 });
  });

  it("X button in the pane header closes it", async () => {
    await (await $("[data-testid='syslog-toggle']")).click();
    await expect(await $("[data-testid='syslog-pane']")).toBeExisting();

    await (await $("[data-testid='syslog-close']")).click();
    await (await $("[data-testid='syslog-pane']")).waitForExist({ reverse: true, timeout: 5000 });
  });
});
