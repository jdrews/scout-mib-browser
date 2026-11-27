# UX Assessment Findings — Scout MIB Browser

**Date:** 2026-08-22
**Commit:** `28acd8faf8b8cce17a83478a9961cab50c76f517`
**Method:** Automated WDIO + Tauri embedded-driver probe suite (`npm run ux:assess`) under Xvfb, against a snmpsim mock agent (`127.0.0.1:11611`, v2c, deterministic data). Covers plan methods A1–A7 + heuristic/aesthetic pass (Approach C).
**Artifacts:** `docs/ux/2026-08-22/` — 30+ screenshots (all A1 states × dark/light) + JSON probes (`timings`, `keyboard-log`, `error-messages`, `feedback-map`, `axe`, `dom-audit`, `strings`).

---

## TL;DR

The app is a **functionally solid, fast, well-instrumented** SNMP tool with genuinely good feedback coverage and consistent domain terminology. The core flow (select node → pick operation → Go → read Result Set) works end-to-end and performs well under the mock agent.

The findings cluster in three areas:
1. **Error/edge-state communication** — the one hard failure path (Test Connection) shows a raw, non-actionable string; a broken MIB leaks a garbage node into the tree.
2. **Accessibility & keyboard operability** — inputs lack accessible names, tree leaves and menu items are not keyboard-reachable, scrollable regions aren't focusable, and there's no `<h1>`/landmark structure. These are the largest cluster.
3. **A trust issue** — the connection modal tells users their V3 credentials are *not* persisted, but they actually are (plaintext config).

**Beauty is a genuine strength** (see the separate aesthetic section) and should not be read as evidence that usability is equally strong — several high-severity keyboard/a11y gaps sit behind an attractive shell.

### Top 5 by criticality
| # | Severity | Finding | Area |
|---|----------|---------|------|
| UX-01 | Critical | Test Connection failure shows only "Receive" — no cause, no next step | Error tolerance |
| UX-02 | High | "Credentials are not persisted" disclaimer is false (V3 passphrases ARE written to plaintext config) | Trust / security |
| UX-03 | High | MIB tree leaf nodes aren't keyboard-focusable — node selection is mouse-only | Keyboard operability |
| UX-04 | High | host/port/OID/filter inputs + modal protocol selects have no accessible name / label association | Accessibility |
| UX-07 | Medium | A stale tree selection silently overrides the typed OID when Go runs | Core task flow |

---

## Methodology & evidence notes (read before trusting screenshots)

- **Screenshots:** captured via ImageMagick `import` against the Xvfb framebuffer (the driver's own `takeScreenshot()` returned torn/stale GL composites). Captures are 1200×800, tight to the window.
  - **Reliable reference frames** (verified clean): `probe-2-results.png`, `probe-3-syslog.png`.
  - **Known artifacts in some A1 captures:** a few frames show headless-compositing ghosts (a prior state's syslog pane / banner bleeding through), and the light-pass `01-launch-ready-light.png` reuses the dirty app instance from the dark pass, so it shows residual state (open System Log, leftover OID, "Connection failed: Receive") rather than a true fresh launch. Treat per-state *layout/color/typography* as valid; do not read transient content in those specific frames as the real initial state.
  - **Recommendation:** final visual sign-off should include a live `npm run dev` pass (devtools) to confirm states render as the clean probes show.
- **Keyboard:** the embedded WebKit driver accepts Tab/arrow key events but never moves focus and can't drive a closed `<select>`. Keyboard findings combine (a) real key events that do work (typing, autocomplete arrows+Enter, Enter on buttons, Escape on the OID input), (b) an emulated-focus DOM walk for tab order, and (c) static reachability analysis.
- **Walk-in-flight / Escape-to-cancel:** the local mock agent answers in ~135 ms, so a running walk can't be caught to test cancellation — that path is *unverified*, not confirmed broken.
- **Timings** are Xvfb-relative baselines (N=5), not absolute performance claims.

---

## Findings by severity

### CRITICAL

**UX-01 — Test Connection failure is non-actionable ("Receive")** · *Error tolerance*
- **Evidence:** `error-messages.json → testConnectionFailed`: `errorMessage: "Receive"`, `statusText: "Connection failed: Receive"`. Screenshot `11-conn-test-failed-{dark,light}.png`.
- **Repro:** open connection modal (gear), set port to a dead port (e.g. 11699), click Test Connection → button flips to "✕ Failed", status reads "Connection failed: Receive".
- **Problem:** The user is told *that* it failed but not *why* or *what to do*. "Receive" is a truncated raw transport error (likely a snmp2 `Receive`/timeout variant) with no host/port/context. Fails all three of the plan's error-message tests: states what happened? (barely) names the cause? (no) suggests next action? (no).
- **Fix:** Map the underlying error to an actionable message, e.g. "Connection failed — no SNMP response from 127.0.0.1:11699. Check the host/port and that the agent is listening." Preserve the raw string in the System Log for debugging.

### HIGH

**UX-02 — False "credentials not persisted" disclaimer** · *Trust / security*
- **Evidence:** `10-conn-modal-v3-{dark,light}.png` + `strings.json → conn-modal-v3.dialogs`: "Credentials are not persisted beyond the current session. Re-enter on each launch." Cross-checked against `src-tauri/src/config.rs` (`config_write_target`), which persists the target — including V3 username/passphrases — to plaintext `~/.config/scout/config.toml`.
- **Problem:** The UI asserts the opposite of what happens. A user who believes credentials are session-only may enter them casually; in reality they're stored in plaintext. This is a trust and (mild) security issue, and it's factually wrong.
- **Fix:** Either (a) stop persisting V3 secrets to plaintext config and make the statement true, or (b) correct the copy to reflect reality ("Your connection settings, including credentials, are saved to your local config") and consider a note about plaintext storage / an opt-out.

**UX-03 — MIB tree leaf nodes aren't keyboard-focusable (mouse-only selection)** · *Keyboard operability*
- **Evidence:** `keyboard-log.json → steps[9]`: `treeLeafAnchorsWithoutHref: 2` (e.g. `brokenThing (.fallback.brokenThing)`); static read of `TreeNode.svelte` — leaves render `<a>` with no `href`. Tab-order walk shows focus stops at the `<summary>` group headers but never reaches individual leaf nodes.
- **Repro:** Tab through the tree — you can reach and expand/collapse group summaries, but you cannot move focus onto a specific leaf (e.g. `sysDescr`) to select it. Selection requires a mouse click.
- **Problem:** The single most important action in the app — choosing which MIB node to query — is impossible with keyboard alone. This breaks the "core flow with zero mouse" requirement outright.
- **Fix:** Make leaf nodes real focusable controls: `<a href="#">` / `role="treeitem"` with `tabindex`, roving-tabindex within the tree, and ArrowUp/Down/Left/Right + Enter to navigate/select per the ARIA tree pattern.

**UX-04 — Inputs & protocol selects have no accessible name / label association** · *Accessibility*
- **Evidence:** `dom-audit.json → accessibleNames` (missing): `host-input`, `port-input`, `oid-input`, `filter-input` (+ one unnamed modal input). `axe.json`: critical `select-name` in every state (1 node; 3 in conn-modal-v3) and critical `label` in the connection modal (1 in v2c, 3 in v3).
- **Problem:** The four primary inputs rely on placeholders only — placeholders are not accessible names and vanish on input. Screen readers announce them as "edit text" with no label. The V3 Auth/Priv Protocol `<select>`s have no name at all.
- **Fix:** Associate each input with a real `<label for>` (or `aria-label`); give the protocol selects labels ("Auth protocol", "Priv protocol"). Visual labels already exist in the modal — wire them up programmatically.

**UX-05 — Menu items aren't keyboard-reachable** · *Keyboard operability / discoverability*
- **Evidence:** `keyboard-log.json → steps[9]`: `menuAnchorsWithoutHref: 2`; static read of `MenuBar.svelte` — menu entries are `<a onclick>` with no `href`, so they're not in the tab sequence. Only the top-level menu *buttons* (File/View/Settings) are focusable.
- **Problem:** A keyboard user can focus "File" but cannot open it or reach its items (Add MIB Directory, Manage MIBs) without a mouse — so Manage MIBs and Add MIB Directory are keyboard-inaccessible.
- **Fix:** Implement an accessible menu button pattern: top-level `role="menu"` buttons that open on Enter/Space/ArrowDown, with focusable `role="menuitem"` children and proper Escape-to-close.

### MEDIUM

**UX-06 — Broken MIB leaks a garbage node into the tree** · *Error tolerance / visual clarity*
- **Evidence:** `strings.json → treeNodes`: `brokenThing (.fallback.brokenThing)` appears in every state's tree; `12a-manage-mibs-*.png` shows BROKEN-MIB tagged "FALLBACK 1 nodes". Screenshot `08-fallback-banner-*.png`.
- **Problem:** The regex fallback for an unparseable MIB surfaces a nonsense node (`brokenThing`, OID `.fallback.brokenThing`) as if it were a real, queryable entry. Users can select it and get a confusing result. It also reads oddly in the tree next to legitimate nodes.
- **Fix:** Either don't render fallback artifacts as selectable tree nodes, or clearly mark them (e.g. dimmed + "unresolved" badge) and prevent/annotate querying them.

**UX-07 — Stale tree selection silently overrides the typed OID at Go time** · *Core task flow*
- **Evidence:** static read of `TargetBar.svelte` / results logic — `effectiveOid = targetNode?.oid || oid`. The A1 spec even documents it (`ux-01-screens.spec.ts:71`: "A tree selection left over would override the typed OID at Go time").
- **Repro:** select a node in the tree (address bar populates), then type a *different* OID into the address bar, then click Go — the walk/get runs against the *tree-selected* OID, not what you typed.
- **Problem:** The visible address bar lies about what will run. This is a subtle correctness/confusion trap in the primary action.
- **Fix:** Make the address bar authoritative when it's been edited (track "dirty" state), or clear/override the tree selection when the user types, and reflect the effective OID unambiguously before executing.

**UX-08 — Scrollable regions aren't keyboard-focusable** · *Accessibility*
- **Evidence:** `axe.json`: serious `scrollable-region-focusable` in `grid-view` and `syslog-open`.
- **Problem:** The results body, the wide grid (22 columns), and the System Log pane scroll but can't be focused/scrolled by keyboard — a keyboard or screen-reader user can't reach off-screen rows/columns/log lines.
- **Fix:** Add `tabindex="0"` (and an accessible name) to each scroll container so it's focusable and scrollable via keys.

**UX-09 — No `<h1>` and weak landmark structure** · *Accessibility*
- **Evidence:** `axe.json`: moderate `page-has-heading-one` (most states), `region` (4–8 nodes not in landmarks), `landmark-unique`, `landmark-complementary-is-top-level` (an `<aside>` nested in another landmark).
- **Problem:** The document has no top-level heading and its panes aren't cleanly exposed as landmarks, so assistive tech can't build a useful page outline or jump between the tree / results / log regions.
- **Fix:** Add one `<h1>` (visually-hidden if needed), give each pane a landmark role (`navigation` for the tree, `main`/`region` with `aria-label` for results, `complementary` for the log) and ensure landmarks aren't nested/duplicated.

**UX-10 — Connection modal: no focus trap, no Escape-to-close, no `aria-labelledby`** · *Accessibility / keyboard*
- **Evidence:** static read of `ConnectionModal.svelte`; consistent with `axe.json` dialog-state violations. (Contrast: the Manage MIBs dialog does close via its button.)
- **Problem:** When the modal is open, Tab can escape into the background page; Escape doesn't dismiss it; and it's not programmatically tied to a heading for screen readers.
- **Fix:** On open, move focus into the modal and trap it (return focus on close); close on Escape; add `role="dialog"` + `aria-modal="true"` + `aria-labelledby` pointing at the "Target Connection" title.

**UX-18 — Fallback banner is not dismissable (stuck for the whole session)** · *Visual clarity / feedback*
- **Evidence:** static read of `MIBPanel.svelte` — the `fallback-banner` (`role="alert"`, `alert-warning`) renders pinned at the bottom of the MIB panel whenever one or more MIBs loaded via fallback, and has no close control. Screenshot `08-fallback-banner-{dark,light}.png`; the banner is visible in every subsequent A1 state capture for the rest of the session.
- **Problem:** A one-time informational warning about MIB load state stays on screen permanently, occupying panel space next to the tree and competing with content the user actually works with. The only action it offers (open System Log) doesn't answer "I've read this — go away." Visually jarring in a tool used for long sessions; users who load a broken MIB once carry the banner until app restart.
- **Fix (decided 2026-08-22): dismissable + collapsed indicator.** Add a close (X) button; dismissal lasts for the session only. When dismissed, show a compact amber indicator (dot with count) in the MIB panel header that reopens the banner on click. Keeps the warning reachable without permanent visual weight.
- **Rejected alternatives:** auto-dismiss after N seconds (warnings shouldn't vanish silently, and an arbitrary timeout feels flaky); persisting dismissal across launches (if the broken MIBs are still loaded next launch, the warning is still true and should reappear).

### LOW / POLISH

- **UX-11 — "regex fallback" jargon in the banner** · *Error tolerance / terminology.* `strings.json → banners`: "1 MIB(s) loaded via regex fallback". "Regex fallback" is an implementation detail. Use plain language: "1 MIB couldn't be fully parsed and was loaded with reduced information."
- **UX-12 — Red "Disconnected" dot at startup** · *Feedback / visual clarity.* A fresh launch shows the red error-colored indicator before the user has done anything wrong. Neutral until a connection is attempted would read better. (`01-launch-ready-*`, status bar.)
- **UX-13 — Icon inconsistency (emoji vs SVG)** · *Visual consistency.* Emoji glyphs (🗑️ clear, ⚠ warnings, ↳ Wrap) sit alongside SVG/text controls. Pick one icon system.
- **UX-14 — "↳ Wrap" label is cryptic** · *Discoverability.* The `↳` affordance doesn't say what it does. A tooltip or clearer label ("Wrap long values") would help first-timers.
- **UX-15 — "1 nodes" grammar** · *Terminology.* Manage MIBs shows "FALLBACK 1 nodes". Pluralize correctly ("1 node").
- **UX-16 — Light-theme low contrast in places** · *Visual clarity / a11y.* Some light-theme text (e.g. menu bar, muted values) is faint; verify against WCAG AA. (`01-launch-ready-light.png`.)
- **UX-17 — "▲ PARTIAL RESULTS" badge color reads as positive** · *Visual clarity.* The partial/warning state uses a teal/green tone that can read as success rather than caution. Use an amber/warning hue for partial results. (`07-warnings-partial-*`.)

---

## Aesthetic / beauty assessment (kept distinct from usability)

Scored 0–4 per state on layout balance, whitespace rhythm, type hierarchy, color/contrast in both themes, iconography consistency, and coherence as a desktop tool. **This is an appeal score — it does not imply the usability findings above are minor.**

| State | Dark | Light | Notes |
|-------|:---:|:---:|-------|
| Launch / empty results | 4 | 3.5 | Clean, calm; light theme menu bar a touch faint |
| Tree selected | 4 | 3.5 | Good hierarchy; `name (oid)` titles are scannable |
| Autocomplete open | 3.5 | 3.5 | Dropdown is clear; could show match highlighting |
| Results list (full) | 4 | 3.5 | Monospace OIDs, good row rhythm, clear columns |
| Grid view (ifTable) | 3.5 | 3 | Dense but legible; 22-col horizontal scroll expected |
| Warnings / partial | 3.5 | 3.5 | Amber banner is clear; badge color slightly off (UX-17) |
| Fallback banner | 3 | 3 | Present but jargon + garbage node (UX-06/11) |
| Connection modal (v2c/v3) | 3.5 | 3.5 | Segmented version control is nice; labels not wired (UX-04) |
| Manage MIBs dialog | 3.5 | 3.5 | Clear list; "FALLBACK" badge + grammar (UX-15) |
| System Log pane | 3.5 | 3 | Monospace log is right; contrast on some lines (axe color-contrast) |

**Overall beauty: ~3.6 / 4.** A professional, coherent, appropriately dense technical tool with a consistent purple accent and good monospace treatment of OIDs/values. The dark theme is the stronger of the two. Deductions are mostly icon/label consistency (UX-13/14) and light-theme contrast (UX-16), not layout or palette problems.

> Aesthetic-usability note: because this app looks polished, first-time users will likely *under*-report the keyboard/a11y friction (UX-03/04/05/08). Watch observed behavior in any human testing, not just verbal ratings.

---

## What's working well (strengths)

- **Feedback coverage is excellent (A4):** 25 of 26 audited actions produce clear user-visible feedback (footer status and/or an obvious visual effect). The only "no feedback" row — closing the View menu by clicking outside — is acceptable. Examples: Unload → "Unloaded BROKEN-MIB"; Test Connection → "Connected to 127.0.0.1:11611" + "✓ Connected"; Go → "Get complete: 1 binding(s)"; Copy OID → "Copied OID: …".
- **Unknown-OID error handling is a model (A5):** the warning banner states what happened, names the cause, uses domain language, and includes the exact OID — "⚠ no-such-instance : No such instance currently exists at this OID on the Target (1.3.6.1.2.1.31.1.1.1.19.9999)" — with a "▲ partial results" badge and an inline ⚠/NULL row.
- **Terminology is consistent (A7):** "Target" is used throughout (never device/host/agent), matching CONTEXT.md; "MIB Names"/"Raw OIDs", "Result Set", "Operation" are used correctly.
- **Fast perceived performance (A2, medians):** launch→Ready ~895 ms; Go→first feedback ~68 ms; autocomplete ~263 ms (under the 300 ms flag); walk first binding ~105 ms; grid render ~29 ms after table-complete.
- **Keyboard works where the driver allows:** typing + ArrowDown + Enter drives autocomplete selection; Enter activates buttons; focus ring is visible on focused buttons.

---

## Performance baselines (A2, N=5, Xvfb-relative)

| Metric | Median | Min | Max | Note |
|--------|-------:|----:|----:|------|
| Launch → Ready (wall) | 895 ms | 102 ms | 30034 ms | High variance; one ~30 s cold sample — investigate first-run cost |
| Autocomplete latency | 263 ms | 253 ms | 272 ms | Under the 300 ms flag, but close |
| Go → first feedback | 68 ms | 57 ms | 72 ms | Good |
| Walk → first binding | 105 ms | 95 ms | 110 ms | Local agent; not a network figure |
| Walk (Go → complete, 31 bindings) | 135 ms | 128 ms | 144 ms | Too fast to observe in-flight / test cancel |
| Table (Go → complete) | 333 ms | 326 ms | 348 ms | ifTable 2×22 |
| Table-complete → grid visible | 29 ms | 25 ms | 31 ms | Good |

---

## Actionable items → tickets

Filed under `~/git/scout-tickets/ux-assessment/` (Status: needs-triage):
- #01 Test Connection error wording (UX-01)
- #02 Fix false credential-persistence disclaimer (UX-02)
- #03 Keyboard-operable MIB tree (UX-03)
- #04 Accessible names/labels on inputs & selects (UX-04)
- #05 Keyboard-reachable menus (UX-05)
- #06 Handle broken-MIB fallback node gracefully (UX-06)
- #07 Make typed OID authoritative over stale tree selection (UX-07)
- #08 Document structure: h1, landmarks, focusable scroll regions (UX-08/09)
- #09 Connection modal a11y: focus trap, Esc-close, aria-labelledby (UX-10)

---

## Axe closure record (launch-beta/13, 2026-11-27)

`npm run ux:assess` on `feat/axe-violations-cleanup` → `docs/ux/2026-11-27/axe.json`:
**0 violation nodes across all 9 states** (axe-core 4.10.2). The 2026-11-20 run
reported 33 nodes; the five rule families closed:

| Rule (impact) | 2026-11-20 | Fix |
|---|---|---|
| `aria-required-attr` (critical, 8 states) | 4 resize handles focusable with no value | All three panes' handles expose `aria-valuenow`/`valuemin`/`valuemax`; MIB-panel and System-Log handles gained arrow-key resize (Inspector semantics) |
| `aria-required-children` / `aria-required-parent` (critical) + `listitem` (serious) | menus' `ul[role=menu]` owned bare `li`s | Items wrap in `li role="none"`; Settings "System Log Level" is a named `role="group"` |
| `region` (moderate, 9 states) | h1 + target bar outside landmarks; the MIB-panel handle once named | h1 + TargetBar in a `header` (banner); the handle moved onto the panel's right edge, inside the panel's `nav` |
| `landmark-one-main` + Manage-MIBs `region` | page replaced the view that carried `main` | `ManageMibsPage` root is `main` |
| `heading-order` (moderate, conn-modal states) | modal titled `h3` under the app `h1` | `h2`, matching HexViewModal |

Two whack-a-moles worth remembering: a separator gains an accessible name,
becomes page content, and must live in a landmark (the `region` hit on the
named handle); wrapping the split in a named `section` fixes that but nests
`main` in a region (`landmark-main-is-top-level`) — the control belongs in
the landmark of the pane it resizes.
