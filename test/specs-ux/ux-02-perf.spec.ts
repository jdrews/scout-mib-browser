import { AGENT_PORT, expandTo, selectTreeNode, setOperation, typeOid, waitForStatus, waitForTreeNode } from "../support/helpers";
import { freshWindow, pageNow, reduceMetric, statusText, writeJson, type Metric } from "../support/ux";

// A2 — Perceived-performance probes. N=5 per metric; wall-clock around WDIO
// actions plus in-page performance.now() where we want frontend-only latency.
// Numbers are relative baselines under Xvfb (see plan Risks), not absolute
// performance claims. Written to docs/ux/<date>/timings.json.
const N = 5;
const CHAIN = ["iso", "org", "dod", "internet", "mgmt", "mib-2", "system"];

describe("UX A2 — perceived-performance probes (N=5)", function () {
  this.timeout(600000);

  const samples: Record<string, number[]> = {
    launch_to_ready_wall: [],
    launch_to_ready_page: [],
    autocomplete_latency: [],
    go_to_first_feedback: [],
    walk_time_to_first_binding: [],
    walk_go_to_complete: [],
    table_go_to_complete: [],
    table_status_to_grid: [],
  };
  const expandLevelSamples: number[][] = []; // [iteration][level]

  before(async () => {
    await freshWindow();
  });

  async function timedExpandChain(): Promise<number[]> {
    // Times each level: click summary (if collapsed) -> next-level node visible.
    const times: number[] = [];
    for (let i = 0; i < CHAIN.length - 1; i++) {
      const parent = await waitForTreeNode(CHAIN[i]);
      const open = await browser.execute(
        (el: Element) => el.getAttribute("aria-expanded") === "true",
        parent
      );
      const t0 = Date.now();
      if (!open) await parent.click();
      await waitForTreeNode(CHAIN[i + 1]);
      times.push(Date.now() - t0);
    }
    await waitForTreeNode(CHAIN[CHAIN.length - 1]);
    return times;
  }

  it("collects N=5 samples for every metric", async () => {
    for (let i = 0; i < N; i++) {
      // ── Launch → Ready (wall + in-page navigation-relative) ───────────────
      const tLaunch = Date.now();
      await freshWindow();
      samples.launch_to_ready_wall.push(Date.now() - tLaunch);
      samples.launch_to_ready_page.push(Math.round(await pageNow()));

      // ── Tree expand lag per level ─────────────────────────────────────────
      expandLevelSamples.push(await timedExpandChain());
      // Expand the final level too (untimed) so sysDescr is selectable below.
      await expandTo(CHAIN);

      // ── Autocomplete latency: type -> dropdown visible ────────────────────
      await selectTreeNode("sysDescr"); // deterministic address-bar state first
      const tAuto = Date.now();
      await typeOid("sysdescr");
      await (await $("[data-testid='autocomplete-list']")).waitForExist({ timeout: 5000 });
      samples.autocomplete_latency.push(Date.now() - tAuto);
      // Dismiss without selecting (keep the tree selection intact).
      await (await $("[data-testid='oid-input']")).click();
      await browser.keys(["Escape"]);

      // ── Go → first feedback ────────────────────────────────────────────────
      const before = await statusText();
      await setOperation("get");
      const tGo = Date.now();
      await (await $("[data-testid='go-btn']")).click();
      await browser.waitUntil(
        async () => (await statusText()) !== before && (await statusText()).trim() !== "",
        { timeout: 15000, interval: 25 },
      );
      samples.go_to_first_feedback.push(Date.now() - tGo);
      await waitForStatus(/^Get complete: \d+ binding\(s\)$/, 30000);

      // ── Walk: time-to-first-binding and go-to-complete (31-binding subtree) ─
      await selectTreeNode("system");
      await setOperation("walk");
      const tWalk = Date.now();
      await (await $("[data-testid='go-btn']")).click();
      await browser.waitUntil(
        async () => (await $$("[data-testid='result-row']")).length > 0,
        { timeout: 30000, interval: 25 },
      );
      samples.walk_time_to_first_binding.push(Date.now() - tWalk);
      await waitForStatus(/^walk complete: \d+ binding\(s\)$/, 60000);
      samples.walk_go_to_complete.push(Date.now() - tWalk);

      // ── Grid render: go-to-complete and status-to-rows (ifTable) ──────────
      await expandTo(["iso", "org", "dod", "internet", "mgmt", "mib-2", "interfaces"]);
      await selectTreeNode("ifTable");
      await setOperation("walk");
      const tTable = Date.now();
      await (await $("[data-testid='go-btn']")).click();
      await waitForStatus(/^Table complete: \d+ row\(s\), \d+ column\(s\)$/, 60000);
      samples.table_go_to_complete.push(Date.now() - tTable);
      const tStatus = Date.now();
      const grid = await $("[data-testid='grid-table']");
      await grid.waitForExist({ timeout: 15000 });
      samples.table_status_to_grid.push(Date.now() - tStatus);
    }

    const metrics: Metric[] = [
      reduceMetric("launch_to_ready_wall", "ms", samples.launch_to_ready_wall, "browser.url -> footer leaves 'Loading…' (includes webview startup + MIB load)"),
      reduceMetric("launch_to_ready_page", "ms", samples.launch_to_ready_page, "in-page performance.now() at Ready (navigation-start relative; excludes native process start)"),
      reduceMetric("autocomplete_latency", "ms", samples.autocomplete_latency, "oid-input setValue('sysdescr') -> autocomplete-list visible (150 ms debounce + mibSearch IPC); flag if median > 300 ms"),
      reduceMetric("go_to_first_feedback", "ms", samples.go_to_first_feedback, "Go click -> status-text first change off 'Ready'"),
      reduceMetric("walk_time_to_first_binding", "ms", samples.walk_time_to_first_binding, "Go click (walk system) -> first result-row rendered (100 ms flush timer in play)"),
      reduceMetric("walk_go_to_complete", "ms", samples.walk_go_to_complete, "Go click -> 'walk complete: 31 binding(s)' on the pinned subtree"),
      reduceMetric("table_go_to_complete", "ms", samples.table_go_to_complete, "Go click (ifTable) -> 'Table complete' status"),
      reduceMetric("table_status_to_grid", "ms", samples.table_status_to_grid, "'Table complete' status -> grid-table rows in DOM"),
    ];
    const levels = CHAIN.slice(0, -1).map((name, i) =>
      reduceMetric(`tree_expand_${name}_to_${CHAIN[i + 1]}`, "ms", expandLevelSamples.map((s) => s[i]), "summary click -> child node visible (iteration 1 is cold: IPC + parse; later iterations warm)"),
    );

    writeJson("timings.json", {
      n: N,
      environment: "Xvfb headless, embedded WebKitGTK driver, snmpsim mock agent on 127.0.0.1:" + AGENT_PORT,
      caveat: "relative baselines under Xvfb, not absolute performance claims",
      metrics,
      treeExpandPerLevel: levels,
    });

    for (const m of [...metrics, ...levels]) {
      console.log(`[ux] ${m.name}: min=${m.min} median=${m.median} max=${m.max} ms`);
    }
  });
});
