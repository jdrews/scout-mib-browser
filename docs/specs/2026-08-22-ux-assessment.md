# UX Assessment Plan

**Date:** 2026-08-22  
**Status:** Proposed

## Purpose

Define how to assess the user experience of Scout MIB Browser: what dimensions
matter, which methods we use, and which tools let us actually *see* the UI.
Primary vehicle: extend the existing WebdriverIO + Tauri embedded-driver harness
(`npm run test:e2e`) into a repeatable **UX probe suite**, complemented by
manual visual review with devtools/screenshots/recording.

## What "UX" means for this app

A desktop tool for SNMP engineers. The UX that matters, mapped to UI areas:

| Dimension | What to assess | Where |
|-----------|----------------|-------|
| Core task flow | Selection → Operation → Execution → reading Result Set; how many steps, how much friction | `TargetBar`, `MIBPanel`, `ResultsPane` |
| Feedback & visibility | Every action produces visible feedback (footer status, progress during streaming walks, connection indicator); nothing silently happens | `AppShell` footer, `status-text`, `conn-indicator` |
| Error tolerance UX | Quality of messaging when things go wrong: unknown OID → warnings + partial badge, broken MIB → fallback banner, failed connection → actionable error. Message must say what happened / why / what to do next | `warnings-banner`, `partial-badge`, `fallback-banner`, `ConnectionModal` |
| Discoverability | Menus, gear button, placeholders, node titles (`name (oid)`), tooltips; can a new user find Test Connection, Manage MIBs, export? | `MenuBar`, `TargetBar`, `ContextMenu` |
| Keyboard operability | Can the core flow run with zero mouse? Autocomplete arrows/Enter/Esc, Escape cancels walk, Tab order sane, focus visible | `TargetBar` keydown handler, global Esc |
| Visual clarity & consistency | Layout balance, value truncation vs Wrap toggle, grid column alignment, dark/light theme contrast, terminology matches CONTEXT.md (Target, not "device/host/agent") | all components, `app.css`, daisyUI themes |
| Visual appeal (beauty) | Do users find the app beautiful? Layout, whitespace, typography, color palette, iconography, theme coherence — scored explicitly, not as an afterthought | whole app, both themes, `app.css`, daisyUI tokens |
| Perceived performance | Time-to-Ready on launch, tree lazy-expand lag per level, autocomplete delay (150 ms debounce + search), Go → first binding, walk completion for 31-binding subtree, grid render for ifTable | measured via WDIO wall-clock + in-page `performance.now()` |
| Accessibility basics | Accessible names on all interactive elements, label↔input association, focus visibility, color contrast, roles on tree/grid | DOM audit per state |

### The aesthetic-usability effect (why "beauty" is a first-class dimension)

Users perceive attractive interfaces as *more usable* — the **aesthetic-usability
effect** ([NN/g](https://www.nngroup.com/articles/aesthetic-usability-effect/)).
People believe things that look better will work better, even when they aren't.
Two consequences for this assessment:

1. **Beauty is a feature, not decoration.** A polished, attractive tool earns
   user trust and goodwill; it is part of the product we are assessing, so it
   gets its own explicit dimension (above) and its own scoring in Approach C —
   not folded into "visual clarity" as an afterthought.
2. **Beauty masks real problems.** The same effect makes users *more tolerant*
   of minor usability flaws in an attractive UI. If the app looks great, users
   will under-report friction and rate the experience higher than the evidence
   warrants. So:
   - In human testing (Approach D), **watch what users do** (hesitations,
     mis-clicks, workarounds) as the ground truth, not just what they *say* —
     a "I loved it" from someone who fumbled the Operation selector is still a
     finding.
   - Treat a high subjective-usability score that contradicts observed task
     errors as a signal of masking, and log both.

## Assets we already have

- **WDIO 9 harness** (`wdio.conf.mjs`, `scripts/test-e2e.sh`): embedded
  `@wdio/tauri-service` driver (WebKitWebDriver, port 4445), Xvfb headless,
  snmpsim mock agent on 11611 with deterministic data (31-binding `system`
  walk, 2×22 `ifTable`), isolated `XDG_CONFIG_HOME`, backend+frontend log
  capture. Nine feature specs pass in ~37 s.
- **`data-testid` hooks** on every key element (see
  `docs/specs/2026-08-21-e2e-feature-coverage.md`) — stable anchors for probes.
- **Shared helpers** (`test/support/helpers.ts`): `waitForStatus`,
  `selectTreeNode`, `expandTo`, `go(op)`, `resultsBodyHasText`, etc.
- **Known driver limitations** (from e2e phase 1, still true): no `text=`
  selectors, no `>>` chaining, no `element.keys`/`dispatchEvent`,
  `selectByAttribute` broken on closed `<select>`, page console forwarding
  does not work (use DOM `dataset` markers + `browser.execute`), localStorage
  is separate between execute context and app main world.
- **Screenshot capture is supported** on Linux by the Tauri driver
  (`@wdio/tauri-service` platform-support docs) — `browser.takeScreenshot()`
  returns base64 PNG from inside a spec. This is the backbone of the visual pass.

## Approach A — Automated UX probes via WDIO (primary)

New spec(s), e.g. `test/specs/ux-probes.spec.ts`, run under a **separate config**
(`wdio.ux.conf.mjs` + `npm run ux:assess`) so the assessment suite stays out of
the CI feature suite and can be slow/screenshot-heavy without breaking the 37 s
baseline. Reuse `scripts/test-e2e.sh` lifecycle (agent, Vite, Xvfb, config
isolation) — factor the harness so both configs share it.

### A1. State screenshot pass

Script every key state once per theme (dark + light via `theme-toggle`), save
`browser.takeScreenshot()` base64 to `docs/ux/<date>/<state>-<theme>.png`:

1. Launch, Ready (empty results placeholder)
2. Tree expanded to `system`, node selected (address bar populated)
3. Autocomplete dropdown open mid-typing
4. Walk running (mid-stream progress visible)
5. Result Set list view (full 31 rows), filtered
6. Grid view (ifTable, missing-cell styling if present)
7. Warnings banner + partial badge (unknown OID Get)
8. Fallback banner (BROKEN-MIB)
9. Connection modal — v2c and v3 field sets
10. Test Connection failed state (bad port)
11. Manage MIBs dialog, System Log pane open
12. Footer in each: status text, node count, connection indicator

These images are the evidence base for every other method below.

### A2. Perceived-performance probes

Wall-clock with `Date.now()` around WDIO actions (and in-page
`performance.now()` via `browser.execute` where we want frontend-only latency):

| Metric | How |
|--------|-----|
| Launch → "Ready" | spec start to `waitForAppReady` |
| Tree expand lag per level | click summary → child node visible (`expandTo` steps timed individually) |
| Autocomplete latency | type into `oid-input` → `autocomplete-list` visible (expect ~150 ms debounce + search; flag if > 300 ms) |
| Go → first feedback | click `go-btn` → `status-text` changes off idle |
| Walk time-to-first-binding / complete | status "Walking…" → first row in `results-body` → "walk complete" (pin to the deterministic 31-binding subtree) |
| Grid render | "Table complete" status → `grid-table` rows visible |

Run each N=5×, record min/median; store results as JSON next to screenshots.
These numbers become before/after baselines for any UX change.

### A3. Keyboard-only task script

Perform the full core flow with **zero mouse** (`browser.keys()` only — note:
`element.keys` doesn't exist on this driver): Tab to `oid-input`, type
`sysdescr`, ArrowDown, Enter, Enter/Tab to Go, then Escape-to-cancel a walk.
Log each step + screenshot; any step requiring a click is a finding. Also verify
focus outline is visible at each stop (screenshot evidence).

### A4. Feedback audit ("action → feedback map")

Scripted session where after *every* action we poll `status-text` for ≤ 2 s and
record what it said (or "no change"). Produces a table of
action → user-visible feedback; any row with no feedback is a finding
(e.g. Clear results, toggles, menu opens).

### A5. Error-path walkthrough

Drive the known failure scenarios against the mock agent and capture exact
wording + screenshots: bad port (Test Connection fail), unknown OID
(noSuchInstance → warnings banner + partial badge), BROKEN-MIB fallback banner,
Go with no host ("No target configured"). Score each message on: states what
happened? names the cause? suggests next action? Uses domain language from
CONTEXT.md?

### A6. Accessibility / DOM audit per state

- Vendor `axe-core` (`test/support/axe.min.js`) — environment may be offline, so
  no CDN. Inject in-page: read file Node-side, pass source string to
  `browser.execute`, append `<script>`, then `axe.run()` on each A1 state; save
  JSON violations per state.
- Manual DOM checks via `browser.execute` (self-contained callbacks — driver
  limitation): interactive elements missing accessible name; inputs without
  associated `<label>` (extend the existing `panelLabelExists` pattern);
  tab-order walk recording; focus-visible outline computed style at each stop.

### A7. Terminology consistency pass

Collect all rendered user-facing strings (status texts, labels, banners, menu
items) via execute-walks and diff against CONTEXT.md vocabulary: "Target" not
device/host/agent/endpoint; "MIB Node", "Variable Binding", "Result Set",
"Operation". Inconsistencies are cheap findings with real onboarding impact.

## Approach B — Tools for actually viewing the UI

| Tool | How to use it | What it's best for |
|------|---------------|--------------------|
| **Live dev window** (`npm run dev`) | Run on a real X11/Wayland display, click through manually. Debug builds ship WebKitGTK devtools: right-click → Inspect Element | Layout, spacing, theme CSS variables, console errors, hover/animation feel — anything needing live manipulation |
| **WDIO `takeScreenshot()`** (A1) | From inside the probe spec; base64 → PNG on disk, state-anchored via testids | Reproducible, diffable evidence tied to exact app states; works headless in CI |
| **Xvfb + ImageMagick `import`** | Run app under a known Xvfb display (`xvfb-run -a` or fixed `:99`), then `DISPLAY=:99 import -window root shot.png` between manual/scripted steps | Full-window captures including OS chrome; fallback if driver screenshots misbehave. `import` is already installed here |
| **Xvfb + ffmpeg x11grab** | `ffmpeg -f x11grab -video_size 1280x800 -i :99+0,0 out.mp4` while the scripted flow runs (add deliberate pauses between states) | Motion review: streaming result updates, dropdown open/close, progress feedback rhythm. Scrubbing a video catches issues still frames miss |
| **Standalone web mode** (`npm run dev:web`) | Vite-only frontend in a real browser (Chrome/Firefox) — full DevTools, axe DevTools extension, Lighthouse, zoom/responsive checks | Pure presentation review (themes, daisyUI tokens, CSS). Caveat: Tauri IPC (`invoke`) has no backend here, so MIB loading and Executions won't populate data-dependent views; useful for shell/layout/theme only until we add a web-mode mock |
| **Reference comparison** | `references/ireasoning/mibbrowser` (iReasoning MIB Browser) side by side in screenshots | Benchmarking conventions: tree layout, results table ergonomics, what power users expect from an SNMP tool |

## Approach C — Heuristic scoring

Structured walkthrough of the A1 screenshot set against Nielsen's 10
heuristics plus tool-specific extras (OID readability at default column width;
MIB Names vs Raw OIDs default; grid row/column density for a 22-column table;
progress visibility on long walks). Score each UI area 0–4 per heuristic with
the screenshot as evidence. Output: findings list, severity-ranked, each with
repro steps (which spec/state) and suggested fix.

**Aesthetic pass (separate from the heuristics).** A dedicated visual-appeal
review of the same A1 set — this is what answers "do users find the app
beautiful?": layout balance and whitespace rhythm, type scale and hierarchy,
color palette + contrast in *both* themes, iconography consistency, density vs.
breathing room, and coherence against the reference app (iReasoning MIB Browser)
and general desktop-tool polish. Score each state 0–4 on appeal with the
screenshot as evidence; report an overall "beauty" score alongside the
usability findings. Because of the aesthetic-usability effect, keep this score
**distinct** from the usability scores — a high beauty score is not evidence
that usability is good, and vice versa.

## Approach D — Task-based testing (optional, later)

Define 3–5 tasks (first-timer: connect + walk `system`; power user: Bulk Walk
`ifTable`, filter, toggle Raw OIDs; recovery: fix a failed connection), run with
2–3 real users, record time/errors/confusion points. The A2/A3 automated
numbers give an objective baseline to compare human runs against. Defer until
A–C findings are triaged.

Capture the aesthetic-usability effect explicitly here: after tasks, ask each
user a short subjective block — "How visually appealing is this tool?" and
"How easy was it to use?" (separate 1–5 scales). Then **reconcile**: if appeal
is high but observed task errors/hesitations are also high, that gap *is* the
aesthetic-usability effect in action — the beauty is masking real friction.
Log observed behavior as ground truth and note where a positive verbal rating
contradicted it. This is the one place in the whole plan where the effect can
be measured directly rather than only guarded against.

## Phased plan

1. **Phase 0 — setup**: ~~confirm `webkit2gtk-driver` installed~~ RESOLVED —
   not needed at all: the harness uses the **embedded** driver provider
   (`driverProvider: 'embedded'`), which ships the WebDriver server inside the
   app binary (cargo feature `scout-mib-browser/wdio`). Verified live on this
   Fedora 44 box (no `webkit2gtk-driver` package exists here; only
   `webkit2gtk4.1-2.52.5` installed): smoke spec launched the app under Xvfb,
   reached "Ready" (179 nodes loaded), and `browser.takeScreenshot()` saved a
   30 KB PNG of the full UI (`/tmp/opencode/ux-smoke/launch.png`). Remaining:
   vendor `axe.min.js`; factor harness so `wdio.ux.conf.mjs` reuses
   agent/Vite/Xvfb lifecycle (reference: `/tmp/opencode/ux-smoke/run.sh`).
2. **Phase 1 — screenshot pass** (A1): all states, both themes. Review in a
   sitting with the reference app open; jot observations.
3. **Phase 2 — probes** (A2–A5): timings, keyboard-only script, feedback map,
   error-path wording.
4. **Phase 3 — audits** (A6–A7 + C): axe results per state, DOM checks,
   terminology diff, heuristic scoring + the separate aesthetic ("beauty") pass.
5. **Phase 4 — findings doc**: severity-ranked list with screenshot refs and
   repro steps; file tickets under `~/git/scout-tickets/` with triage labels
   (`needs-triage` → …) per the repo's issue-tracker convention.

### Definition of done (for the assessment itself)

- Screenshot set covers all A1 states in both themes, committed to `docs/ux/<date>/`.
- Timing JSON baselines recorded (N=5 per metric).
- Keyboard-only core flow result: pass/fail with step log.
- Feedback map complete; every action has a feedback row or an explicit finding.
- Axe + DOM audit results per state; terminology diff produced.
- Aesthetic ("beauty") score recorded per state and overall, kept distinct from usability scores.
- Findings doc written; actionable items filed as tickets.

## Risks / known limitations

- **One shared app instance** across spec files (maxInstances 1, state
  persists) — UX specs must either run in their own WDIO invocation or be
  order-aware (the feature suite already has this property).
- **Driver gaps** (no text selectors, no `element.keys`, broken console
  forwarding) mean all probes go through `browser.execute` self-contained
  callbacks + testids — same pattern as existing helpers.
- **Timing noise** under Xvfb: numbers are relative baselines, not absolute
  performance claims; note the environment in the report.
- **Web mode is data-blind** without a backend mock (Approach B caveat).
- ~~Screenshot capture unverified~~ VERIFIED working (embedded provider,
  `browser.takeScreenshot()` → base64 PNG) — see Phase 0 note above.
