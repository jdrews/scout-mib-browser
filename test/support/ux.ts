import fs from "fs";
import path from "path";
import { execFileSync } from "child_process";
import { createRequire } from "module";
import { fileURLToPath } from "url";
import { waitForAppReady } from "./helpers";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "..", "..");

/** Artifact directory for this run: docs/ux/<date>/ (harness exports UX_RUN_DATE). */
export function uxDir(): string {
  const date = process.env.UX_RUN_DATE || new Date().toISOString().slice(0, 10);
  const dir = path.join(REPO_ROOT, "docs", "ux", date);
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

/** Writes a JSON artifact next to the screenshots. */
export function writeJson(name: string, data: unknown): string {
  const p = path.join(uxDir(), name);
  fs.writeFileSync(p, JSON.stringify(data, null, 2));
  console.log(`[ux] wrote ${p}`);
  return p;
}

/**
 * Captures the current screen and saves it as <name>.png in the run dir.
 *
 * The embedded WebKitGTK driver's takeScreenshot() returns torn/stale GL
 * composites (ghost content from other states bleeding through) even after a
 * settle+discard, so we capture the real X server framebuffer directly with
 * ImageMagick `import` against the Xvfb display inherited from xvfb-run. This
 * reads ground truth, not the driver's lagging grab. Falls back to the driver
 * capture if DISPLAY is unset or `import` fails.
 */
export async function shot(name: string): Promise<string> {
  const p = path.join(uxDir(), `${name}.png`);
  await browser.pause(400); // let in-flight repaints finish
  const display = process.env.DISPLAY;
  if (display) {
    try {
      const full = `${p}.full.png`;
      execFileSync("import", ["-display", display, "-window", "root", full], { stdio: "ignore" });
      // Crop to the Tauri window (1200x800 at 0,0 in a WM-less Xvfb) so we drop
      // the black margin around the app and get a tight capture.
      execFileSync("convert", [full, "-crop", "1200x800+0+0", "+repage", p], { stdio: "ignore" });
      fs.rmSync(full, { force: true });
      console.log(`[ux] screenshot ${path.basename(p)} (import ${display}, cropped)`);
      return p;
    } catch (e) {
      console.warn(`[ux] import capture failed (${String(e).split("\n")[0]}); falling back to driver`);
    }
  } else {
    console.warn("[ux] DISPLAY unset — using driver capture (may be torn)");
  }
  await browser.takeScreenshot(); // discard — may be a mid-repaint frame
  await browser.pause(250);
  const b64 = await browser.takeScreenshot();
  fs.writeFileSync(p, Buffer.from(b64, "base64"));
  console.log(`[ux] screenshot ${path.basename(p)} (driver)`);
  return p;
}

/**
 * Rewrites the isolated scout/config.toml back to the seeded target (v2c,
 * public, 127.0.0.1:11611). Earlier specs persist host/port/version edits via
 * the app itself (e.g. toggling SNMP V3 in the connection modal), and a stale
 * v3 version poisons every later spec against the v2c-only mock agent with
 * AuthFailure(SecurityNotReady) — so every fresh window starts from the seed.
 */
export function resetTargetConfig(): void {
  const base = process.env.XDG_CONFIG_HOME || path.join(process.env.HOME || "", ".config");
  const dir = path.join(base, "scout");
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(
    path.join(dir, "config.toml"),
    `[mib]
directories = ["${REPO_ROOT}/test/mibs"]

[target]
community = "public"
host = "127.0.0.1"
port = 11611
version = "v2c"
`,
  );
}

/** Reloads the window and waits for startup to finish (fresh clean baseline). */
export async function freshWindow(): Promise<void> {
  resetTargetConfig();
  await browser.url("http://localhost:5173");
  await waitForAppReady(60000);
}

// ── Theme ─────────────────────────────────────────────────────────────────────

/** Reads the active theme from the shell's data-theme attribute. */
export async function currentTheme(): Promise<string> {
  const el = await $("[data-testid='status-text']");
  await el.waitForExist({ timeout: 15000 });
  return (await browser.execute(() => {
    const d = document.querySelector("div[data-theme]");
    return d ? d.getAttribute("data-theme") : "";
  })) ?? "";
}

/** Toggles the theme until the requested one is active (guard: at most 3 clicks). */
export async function setTheme(theme: "dark" | "light"): Promise<void> {
  for (let i = 0; i < 3; i++) {
    if ((await currentTheme()) === theme) return;
    await (await $("[data-testid='theme-toggle']")).click();
    await browser.pause(250);
  }
  const got = await currentTheme();
  if (got !== theme) throw new Error(`could not switch to ${theme} theme (now: ${got})`);
}

// ── Status / feedback polling ────────────────────────────────────────────────

export async function statusText(): Promise<string> {
  const el = await $("[data-testid='status-text']");
  return (await el.getText()) ?? "";
}

/**
 * After an action, polls the footer status for up to `timeoutMs` and returns
 * the first text that differs from `before` (or "no change"). Used by the
 * feedback audit (A4).
 */
export async function pollFeedback(before: string, timeoutMs = 2000): Promise<string> {
  const t0 = Date.now();
  let last = before;
  while (Date.now() - t0 < timeoutMs) {
    last = await statusText();
    if (last !== before && last.trim() !== "") return last;
    await browser.pause(100);
  }
  return "no change";
}

// ── Timing (A2) ───────────────────────────────────────────────────────────────

export interface Metric {
  name: string;
  unit: string;
  samples: number[];
  min: number;
  median: number;
  max: number;
  notes?: string;
}

/** Collects wall-clock samples and reduces them to min/median/max. */
export function reduceMetric(name: string, unit: string, samples: number[], notes?: string): Metric {
  const sorted = [...samples].sort((a, b) => a - b);
  const median =
    sorted.length % 2 === 1
      ? sorted[(sorted.length - 1) / 2]
      : (sorted[sorted.length / 2 - 1] + sorted[sorted.length / 2]) / 2;
  return { name, unit, samples, min: sorted[0], median, max: sorted[sorted.length - 1], notes };
}

/** In-page navigation-timing snapshot (frontend-only latency baseline). */
export async function pageTiming(): Promise<Record<string, number>> {
  return (
    (await browser.execute(() => {
      const t = performance.getEntriesByType("navigation")[0] as PerformanceNavigationTiming;
      if (!t) return {};
      return {
        domContentLoaded: Math.round(t.domContentLoadedEventEnd - t.startTime),
        load: Math.round(t.loadEventEnd - t.startTime),
      };
    })) ?? {}
  );
}

/** performance.now() in the page context (for frontend-only latency probes). */
export async function pageNow(): Promise<number> {
  return (await browser.execute(() => performance.now())) ?? 0;
}

// ── axe-core (A6) ─────────────────────────────────────────────────────────────

// axe-core is a devDependency; resolve its bundled browser build from node_modules.
const require_ = createRequire(import.meta.url);
const AXE_SRC = fs.readFileSync(require_.resolve("axe-core/axe.min.js"), "utf8");

/** Injects axe-core into the page (idempotent; re-inject after reloads). */
export async function injectAxe(): Promise<boolean> {
  return (
    (await browser.execute((code: string) => {
      const w = window as unknown as { axe?: unknown };
      if (w.axe) return true;
      const s = document.createElement("script");
      s.textContent = code;
      document.documentElement.appendChild(s);
      return !!w.axe;
    }, AXE_SRC)) ?? false
  );
}

export interface AxeViolation {
  id: string;
  impact: string | null;
  description: string;
  help: string;
  nodes: number;
  targets: string[];
}

/**
 * Runs axe on the current state and returns a compact violation summary.
 * Uses axe's promise API through async browser.execute — verified working on
 * the embedded driver (the callback API delivered an undefined result).
 */
export async function runAxe(): Promise<AxeViolation[]> {
  if (!(await injectAxe())) throw new Error("axe-core injection failed");
  return await browser.execute(async () => {
    const w = window as unknown as {
      axe: { run: (el: Node) => Promise<{ violations: Array<Record<string, unknown>> }> };
    };
    const r = await w.axe.run(document);
    return (r.violations || []).map((v) => ({
      id: String(v.id),
      impact: v.impact === null ? null : String(v.impact),
      description: String(v.description),
      help: String(v.help),
      nodes: Array.isArray(v.nodes) ? v.nodes.length : 0,
      targets: Array.isArray(v.nodes) ? v.nodes.slice(0, 8).map((n) => JSON.stringify(n.target)) : [],
    }));
  });
}

// ── DOM audit (A6 manual checks + A7 strings) ────────────────────────────────

export interface TabStop {
  index: number;
  tag: string;
  testid: string | null;
  text: string;
  outlineStyle: string;
  outlineWidth: string;
  focusVisible: boolean;
}

/**
 * Tab-order evidence. NOTE: the embedded WebKit driver does not move focus on
 * Tab key events (verified: browser.keys/performActions with "Tab" are accepted
 * but focus never changes), so this walks the DOM in order, programmatically
 * focuses each natively-focusable element, and records its computed focus
 * outline. The ORDER is the real tab order (no tabindex overrides exist in this
 * app); the keypress itself is emulated via el.focus().
 */
export async function tabOrderWalk(maxStops = 60): Promise<TabStop[]> {
  const ids: string[] = await browser.execute(() => {
    const sel = 'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), summary, [tabindex]:not([tabindex="-1"])';
    return Array.from(document.querySelectorAll(sel))
      .filter((el) => {
        const r = (el as HTMLElement).getBoundingClientRect();
        return r.width > 0 && r.height > 0; // skip hidden controls
      })
      .map((el, i) => {
        el.setAttribute("data-ux-stop", String(i));
        return `[data-ux-stop="${i}"]`;
      });
  }) ?? [];

  const stops: TabStop[] = [];
  for (let i = 0; i < Math.min(ids.length, maxStops); i++) {
    const info = await browser.execute((sel: string) => {
      const el = document.querySelector(sel) as HTMLElement | null;
      if (!el) return null;
      el.focus();
      const cs = getComputedStyle(el);
      const r = el.getBoundingClientRect();
      return {
        tag: el.tagName.toLowerCase(),
        testid: el.getAttribute("data-testid") || "",
        text: ((el.textContent || "").replace(/\s+/g, " ").trim() || el.getAttribute("placeholder") || "").slice(0, 60),
        outlineStyle: cs.outlineStyle,
        outlineWidth: cs.outlineWidth,
        focusVisible: r.width > 0 && (cs.outlineStyle !== "none" || cs.boxShadow !== "none"),
      };
    }, ids[i]);
    if (!info) continue;
    stops.push({ index: i, ...info });
  }

  // Clean up the temporary markers and restore focus to body.
  await browser.execute(() => {
    document.querySelectorAll("[data-ux-stop]").forEach((n) => n.removeAttribute("data-ux-stop"));
    (document.activeElement as HTMLElement | null)?.blur?.();
  });
  return stops;
}

/** Programmatically focuses an element by data-testid (Tab stand-in — see tabOrderWalk note). */
export async function focusByTestid(testid: string): Promise<boolean> {
  return (
    (await browser.execute((t: string) => {
      const el = document.querySelector(`[data-testid='${t}']`) as HTMLElement | null;
      if (!el) return false;
      el.focus();
      return document.activeElement === el;
    }, testid)) ?? false
  );
}

export interface NameAuditRow {
  tag: string;
  testid: string | null;
  text: string;
  name: string;
  hasName: boolean;
  kind: "button" | "link" | "input" | "select" | "summary" | "checkbox";
}

/**
 * Finds interactive elements and computes whether each has an accessible name
 * (text content, aria-label/labelledby, title, or — for inputs — an associated
 * <label>). Also flags inputs with no label association at all.
 */
export async function accessibleNameAudit(): Promise<NameAuditRow[]> {
  return (
    (await browser.execute(() => {
      const norm = (s: string | null) => (s ?? "").replace(/\s+/g, " ").trim();
      const rows: Array<{
        tag: string;
        testid: string | null;
        text: string;
        name: string;
        hasName: boolean;
        kind: string;
      }> = [];

      function labelFor(el: HTMLElement): string {
        if (el.id) {
          const l = document.querySelector(`label[for="${CSS.escape(el.id)}"]`);
          if (l) return norm(l.textContent);
        }
        const wrapping = el.closest("label");
        if (wrapping && wrapping !== el) return norm(wrapping.textContent);
        return "";
      }

      function nameOf(el: HTMLElement): string {
        const ariaLabel = norm(el.getAttribute("aria-label"));
        if (ariaLabel) return ariaLabel;
        const labelledby = el.getAttribute("aria-labelledby");
        if (labelledby) {
          const parts = labelledby.split(/\s+/).map((id) => norm(document.getElementById(id)?.textContent));
          if (parts.some(Boolean)) return parts.join(" ");
        }
        const ownText = norm(el.textContent);
        if (ownText) return ownText;
        const title = norm(el.getAttribute("title"));
        if (title) return title;
        if (el instanceof HTMLInputElement || el instanceof HTMLSelectElement) {
          const l = labelFor(el);
          if (l) return l;
          const ph = norm(el.getAttribute("placeholder"));
          if (ph) return `placeholder:${ph}`;
        }
        return "";
      }

      const push = (el: HTMLElement, kind: string) => {
        const name = nameOf(el);
        rows.push({
          tag: el.tagName.toLowerCase(),
          testid: el.getAttribute("data-testid"),
          text: norm(el.textContent).slice(0, 60),
          name,
          hasName: name !== "" && !name.startsWith("placeholder:"),
          kind,
        });
      };

      document.querySelectorAll("button").forEach((el) => push(el as HTMLElement, "button"));
      document.querySelectorAll("a").forEach((el) => push(el as HTMLElement, "link"));
      document.querySelectorAll("select").forEach((el) => push(el as HTMLElement, "select"));
      document.querySelectorAll("summary").forEach((el) => push(el as HTMLElement, "summary"));
      document.querySelectorAll("input[type='checkbox']").forEach((el) => push(el as HTMLElement, "checkbox"));
      document.querySelectorAll("input:not([type='checkbox'])").forEach((el) => push(el as HTMLElement, "input"));

      return rows;
    })) ?? []
  );
}

/** Collects all rendered user-facing strings for the terminology pass (A7). */
export async function collectUserStrings(): Promise<Record<string, string[]>> {
  return (
    (await browser.execute(() => {
      const norm = (s: string | null) => (s ?? "").replace(/\s+/g, " ").trim();
      const out: Record<string, string[]> = {
        status: [],
        buttons: [],
        menuItems: [],
        labels: [],
        placeholders: [],
        headers: [],
        banners: [],
        dialogs: [],
        treeNodes: [],
        other: [],
      };
      const add = (arr: string[], s: string) => {
        if (s && !arr.includes(s)) arr.push(s);
      };

      const st = document.querySelector("[data-testid='status-text']");
      add(out.status, norm(st?.textContent));
      const ind = document.querySelector("[data-testid='conn-indicator']");
      add(out.status, norm(ind?.textContent));
      const nc = document.querySelector("[data-testid='node-count']");
      add(out.status, norm(nc?.textContent));

      document.querySelectorAll("button").forEach((b) => add(out.buttons, norm(b.textContent)));
      document.querySelectorAll("nav a, .menu a, [data-testid^='menu-']").forEach((a) =>
        add(out.menuItems, norm(a.textContent)),
      );
      document.querySelectorAll("label").forEach((l) => add(out.labels, norm(l.textContent)));
      document.querySelectorAll("[placeholder]").forEach((i) => add(out.placeholders, i.getAttribute("placeholder") || ""));
      document.querySelectorAll("h1, h2, h3, h4, [data-testid='mib-panel-header'], [data-testid='results-header']").forEach(
        (h) => add(out.headers, norm(h.textContent)),
      );
      document.querySelectorAll("[role='alert'], .alert").forEach((b) => add(out.banners, norm(b.textContent)));
      document.querySelectorAll("dialog, .modal-box").forEach((d) => add(out.dialogs, norm(d.textContent).slice(0, 400)));
      document.querySelectorAll("[data-tree-node]").forEach((n) => add(out.treeNodes, n.getAttribute("title") || ""));

      return out;
    })) ?? { status: [], buttons: [], menuItems: [], labels: [], placeholders: [], headers: [], banners: [], dialogs: [], treeNodes: [], other: [] }
  );
}
