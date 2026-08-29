import fs from "fs";
import path from "path";

export const AGENT_HOST = "127.0.0.1";
/** Primary mock agent (pinned linux-full-walk recording). */
export const AGENT_PORT = Number(process.env.E2E_AGENT_PORT || 11611);
/** Synthetic ifStackTable agent (multi-component index, 600 rows). */
export const SYNTH_AGENT_PORT = Number(process.env.E2E_SYNTH_AGENT_PORT || 11612);

/** Switches the target port in the address bar (returns the previous value). */
export async function setTargetPort(port: number): Promise<number> {
  const portEl = await $("[data-testid='port-input']");
  const before = (await portEl.getValue()) ?? "";
  await portEl.setValue(String(port));
  return Number(before);
}

/** Restores the target port to the primary mock agent. */
export async function restoreTargetPort(): Promise<void> {
  await setTargetPort(AGENT_PORT);
}

/** Path to the isolated config file (the harness exports XDG_CONFIG_HOME). */
export function configFilePath(): string {
  const base = process.env.XDG_CONFIG_HOME || path.join(process.env.HOME || "", ".config");
  return path.join(base, "scout", "config.toml");
}

/** Reads the isolated scout/config.toml written by the app. */
export function readConfigFile(): string {
  return fs.readFileSync(configFilePath(), "utf8");
}

/** Reads today's app log file from the isolated config dir ("" if absent). */
export function readAppLog(): string {
  const dir = path.dirname(configFilePath());
  let newest: string | null = null;
  for (const f of fs.readdirSync(dir)) {
    if (f.startsWith("scout.log.") && (!newest || f > newest)) newest = f;
  }
  return newest ? fs.readFileSync(path.join(dir, newest), "utf8") : "";
}

/**
 * Waits until the app has finished its startup sequence. The Tauri app process
 * persists across spec files (one shared session), so the footer status may be
 * a stale message from an earlier file ("Connection failed: ...", "Get complete:
 * ...") instead of "Ready". Any non-empty, non-loading status means the app is
 * up and the MIB tree is loaded. Returns the current status text.
 */
export async function waitForAppReady(timeoutMs = 45000): Promise<string> {
  const el = await $('[data-testid="status-text"]');
  await el.waitForExist({ timeout: timeoutMs });
  let last = "";
  try {
    await browser.waitUntil(
      async () => {
        last = (await el.getText()) ?? "";
        return last.trim() !== "" && !last.startsWith("Loading");
      },
      { timeout: timeoutMs, interval: 250 }
    );
  } catch {
    // waitUntil's own timeout message is built before the first poll, so it
    // can't include `last`; rethrow with the real captured value.
    throw new Error(`app never finished loading (last: "${last}")`);
  }
  return last;
}

/** Waits until the footer status text matches `pattern`; returns the text. */
export async function waitForStatus(pattern: RegExp, timeoutMs = 30000): Promise<string> {
  const el = await $('[data-testid="status-text"]');
  await el.waitForExist({ timeout: timeoutMs });
  let last = "";
  try {
    await browser.waitUntil(
      async () => {
        last = (await el.getText()) ?? "";
        return last.match(pattern) !== null;
      },
      { timeout: timeoutMs, interval: 250 }
    );
  } catch {
    // waitUntil's own timeout message is built before the first poll, so it
    // can't include `last`; rethrow with the real captured value.
    throw new Error(`status-text never matched ${pattern} (last: "${last}")`);
  }
  return last;
}

// NOTE: browser.execute callbacks run in the PAGE context — they must be fully
// self-contained (no references to helpers defined in this file). The
// normalize-space logic below is therefore inlined into each callback.

/**
 * True when a leaf element (no children) inside the results body has exactly
 * the given text. The embedded driver cannot chain selectors (`>>`) or match
 * by text, so specs assert on row cell contents through this execute helper.
 */
export async function resultsBodyHasText(text: string): Promise<boolean> {
  return await browser.execute((t: string) => {
    const norm = (s: string | null) => (s ?? "").replace(/\s+/g, " ").trim();
    const body = document.querySelector("[data-testid='results-body']");
    if (!body) return false;
    const leaves = Array.from(body.querySelectorAll("*")).filter((n) => n.children.length === 0);
    return leaves.some((n) => norm(n.textContent) === t);
  }, text);
}

/** True when the connection panel has a <label> with exactly the given text. */
export async function panelLabelExists(text: string): Promise<boolean> {
  return await browser.execute((t: string) => {
    const norm = (s: string | null) => (s ?? "").replace(/\s+/g, " ").trim();
    const panel = document.querySelector("[data-connection-panel]");
    if (!panel) return false;
    return Array.from(panel.querySelectorAll("label")).some((l) => norm(l.textContent) === t);
  }, text);
}

/**
 * Clicks a button in the connection panel by exact text. The driver can't
 * select by text, so execute tags the target with a temp attribute and the
 * spec-side click uses a plain CSS selector for it.
 */
export async function clickPanelButton(text: string): Promise<void> {
  const found = await browser.execute((t: string) => {
    const norm = (s: string | null) => (s ?? "").replace(/\s+/g, " ").trim();
    const panel = document.querySelector("[data-connection-panel]");
    if (!panel) return false;
    const btn = Array.from(panel.querySelectorAll("button")).find((b) => norm(b.textContent) === t);
    if (!btn) return false;
    btn.setAttribute("data-e2e-target", "1");
    return true;
  }, text);
  if (!found) throw new Error(`connection panel button "${text}" not found`);
  const el = await $('[data-connection-panel] button[data-e2e-target="1"]');
  await el.click();
  await browser.execute(() => {
    document.querySelectorAll("[data-e2e-target]").forEach((n) => n.removeAttribute("data-e2e-target"));
  });
}

/** Finds a visible tree node by exact name (node title is "name (oid)"). */
export async function findTreeNode(name: string): Promise<WebdriverIO.Element> {
  const nodes = await $$("[data-tree-node]");
  for (const n of nodes) {
    const title = (await n.getAttribute("title")) || "";
    if (title.startsWith(`${name} (`)) return n;
  }
  throw new Error(`tree node "${name}" not found among ${nodes.length} visible nodes`);
}

/** Waits for a tree node to become visible (children load lazily). */
export async function waitForTreeNode(name: string, timeoutMs = 15000): Promise<WebdriverIO.Element> {
  const el = await browser.waitUntil(
    async () => {
      try {
        return await findTreeNode(name);
      } catch {
        return null;
      }
    },
    { timeout: timeoutMs, interval: 200, timeoutMsg: `tree node "${name}" never appeared` }
  );
  if (!el) throw new Error(`tree node "${name}" never appeared`);
  return el;
}

/** Clicks a tree node to select it (waits for it to be visible first). */
export async function selectTreeNode(name: string): Promise<void> {
  await (await waitForTreeNode(name)).click();
}

/** Expands successive tree nodes by name, waiting for lazy children between steps. */
export async function expandTo(names: string[]): Promise<void> {
  for (const name of names) {
    const node = await waitForTreeNode(name);
    // Branch nodes are role=treeitem divs with aria-expanded; the tree state
    // persists across spec files, so only click when collapsed — clicking an
    // expanded branch would collapse it.
    const open = await browser.execute(
      (el: Element) => el.getAttribute("aria-expanded") === "true",
      node
    );
    if (!open) await node.click();
  }
}

/** Sets the operation dropdown to `op` (get/getNext/walk/bulkWalk/set). */
export async function setOperation(op: string): Promise<void> {
  // selectByAttribute clicks the <option>, which this driver does not turn
  // into a value change on a closed <select>. Set the value and fire the
  // change event Svelte's bind:value listens for.
  await browser.execute(
    (sel: string, v: string) => {
      const s = document.querySelector(sel) as HTMLSelectElement;
      s.value = v;
      s.dispatchEvent(new Event("change", { bubbles: true }));
    },
    "[data-testid='op-select']",
    op
  );
}

/** Clicks Go, optionally switching the operation first. */
export async function go(op?: string): Promise<void> {
  if (op) await setOperation(op);
  await $('[data-testid="go-btn"]').click();
}

/** Sets the OID address input. */
export async function typeOid(value: string): Promise<void> {
  await (await $('[data-testid="oid-input"]').setValue(value));
}

/** Reads the OID address input value. */
export async function oidInputValue(): Promise<string> {
  return (await (await $('[data-testid="oid-input"]').getValue())) ?? "";
}

/** Parses "N node(s) loaded" from the footer node count element. */
export async function nodeCount(): Promise<number> {
  const text = (await (await $('[data-testid="node-count"]').getText())) ?? "";
  const m = text.match(/(\d+) nodes? loaded/);
  if (!m) throw new Error(`node-count did not match "N node(s) loaded": "${text}"`);
  return Number(m[1]);
}

/** True when OID `a` sorts strictly after OID `b`. */
export function oidIsGreater(a: string, b: string): boolean {
  const pa = a.split(".").map(Number);
  const pb = b.split(".").map(Number);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const x = pa[i] ?? -1;
    const y = pb[i] ?? -1;
    if (x !== y) return x > y;
  }
  return false;
}
