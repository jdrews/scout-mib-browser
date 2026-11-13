# UX Visual Polish — Scout MIB Browser

**Date:** 2026-11-02 (baseline refreshed 2026-11-03)
**Status:** Direction signed off 2026-11-03 (Phase 0 gate). Phase 1 (token foundation) **merged** to `main` 2026-11-11 via PR #135 (`e63d85e`) — theme (both themes), IBM Plex pair, motion tokens, and the dead-CSS migration, verified against the 2026-11-03 baseline. Phase 2 in progress: pass 1 (spacing) **merged** via PR #137 (2026-11-13); pass 2 (color) **merged** via PR #138 (2026-11-13); pass 3 (icons) done on `feat/ux-visual-polish-icons` (one 16 px Lucide size + 1.5 stroke app-wide via a single `.lucide` CSS rule, ghost sort arrows removed from inactive headers I2 — active column only; harness 6/6, axe at parity, timings within noise).
**Method:** Unslop-style visual audit (see [Credit](#credit)) applied to the app's current UI, using this repo's UX assessment capability (`npm run ux:assess`, `docs/specs/2026-08-22-ux-assessment.md`) as the evidence and verification harness.
**Evidence base:** `docs/ux/2026-11-03/` (fresh baseline, commit `7733df3`, 206 nodes, all 12 states × dark/light + JSON probes — supersedes the 2026-09-12 set, which predated the Manage MIBs page and Set features), current component sources under `src/lib/components/`, and the 2026-08-22 findings/remediation pair (all 18 prior usability findings are closed — this plan is about *visual character*, not usability gaps).

---

## TL;DR

The app is functionally polished and was scored ~3.6/4 on beauty in August, but visually it is **the unthemed default**: daisyUI's stock dark theme (violet primary `oklch(58% 0.233 277°)`), stock neon-saturation warning/error hues, 24 uppercase tracked labels, three off-scale text sizes (10/11/13 px), three icon sizes, 57 arbitrary Tailwind values, and the system font stack nobody chose. The template check fails: swap in another product's name and the screen would still fit.

This is a **polish-mode** run, not a redesign. The layout, panes, interactions, and data treatments (mono OIDs, hex dumps, the tree) are the product and stay. What changes is the token layer and the small decisions: one tinted neutral ramp kept, the accent moved from violet to the product's world, semantic hues toned down, labels to sentence case, one type pairing, one icon size, one spacing scale, short copy, quiet detail, one motion curve.

Each pass below ships as a small PR with before/after screenshot diffs from the `ux:assess` harness, so every change is evidence-backed and regression-checked (axe, DOM, strings, timings all re-run).

---

## Method

Two inputs, combined:

1. **Unslop audit.** Walk every group of the unslop signs list (surfaces/color, type, icons, spacing, copy, components, visuals, motion, template look) against (a) the 2026-11-03 screenshot set — all 12 A1 states in both themes plus the error, keyboard, and log captures — and (b) the component sources, where the signs live in class names and tokens. Every sign below is listed with its evidence (screenshot file and/or code location).
2. **This repo's UX assessment capability.** The WDIO + Tauri embedded-driver probe suite (`scripts/ux-assess.sh` → `wdio.ux.conf.mjs` → `test/specs-ux/**`) is already wired for exactly this: state-anchored screenshots in both themes, rendered-string collection (`strings.json` — the copy-audit diff source), axe + DOM audits (regression guard while restyling), and timing baselines (perceived performance must not move). It runs headless under Xvfb against the deterministic snmpsim mock agent, so before/after comparison is reproducible and cheap.

The August assessment's heuristic/aesthetic scoring is reused as the sign-off instrument (Approach C in the assessment plan): score each state 0–4 on appeal *and* run the template check — "if you swapped the product name for another product's, would the screen still fit?"

---

## What's already good (keep, don't touch)

- **Layout and information architecture.** Three-pane shell (tree / results / inspector), status footer, target bar — correct proportions for a dense desktop tool; the August assessment rated layout 4/4 in the core states.
- **Mono treatment of OIDs, values, paths, and hex.** The single best visual decision in the app; it is the seed of the direction below.
- **Hairline borders and neutral surface steps.** `base-300` borders + `base-100/200/300` steps are quiet and correct; no gradients, glows, gradient text, colored shadows, or emoji remain (the UX-13 Lucide migration cleared the emoji).
- **Feedback coverage** (25/26 actions produce visible feedback) and the **unknown-OID warning path** (banner names the cause, includes the exact OID) — model behavior per the findings doc.
- **Motion restraint.** No looping animation, no `transition-all` on cards; only `transition-colors` on dividers. The motion pass is nearly free.
- **The daisyUI dark neutral ramp** is already a cool graphite (`base-100 oklch(25.3% 0.016 252°)`) — close to the target world. Keep the ramp; change the accents on top of it.

---

## The audit — signs of slop, with evidence

Verdict up front: **light-to-medium slop** — a designed app wearing a stock theme. No hero, no blobs, no gradient text; the tells are the palette, the labels, the sizes, and the copy.

### 1. Color (the dominant tell)

| # | Sign | Evidence |
|---|------|----------|
| C1 | Stock **violet primary** — the default AI-vibe accent, from daisyUI's unmodified dark theme (`node_modules/daisyui/theme/dark.css`: `--color-primary: oklch(58% 0.233 277.117)`) | Go button, SNMP version active tab, MIB Names/Raw active toggle, modal Close button, credential toggle, tree selection tint, find-hit tint, focus rings — every state in `docs/ux/2026-11-03/*` |
| C2 | **Full-saturation semantic hues** in the chrome at the same time: neon amber warning (`oklch(82% 0.189 84°)`), pink-red error, blue info, green success | Fallback banner (`01-*`), PARTIAL RESULTS badge + no-such-instance banner (`09-*`), Unload All (`12a-*`), `[INFO]` log tags (`12b-*`), Disconnected dot (footer, all states) |
| C3 | The **bright amber fallback banner is the loudest element on the launch screen** — pinned bottom-left over the tree, 4 wrapped lines, competing with content the user works with | `01-launch-ready-dark.png`; `MIBPanel.svelte:287` (`alert alert-warning`) |
| C4 | Fourth hue in data cells: teal `text-accent` for missing cells, warning rows, and the missing-cell footer count | `ResultsPane.svelte:966,1150,1174`; `06-grid-view-dark.png` |
| C5 | Stock magenta `secondary` defined in the theme (unused by components, but one token from a magenta surprise) | `dark.css` `--color-secondary: oklch(65% 0.241 354°)` |

### 2. Type

| # | Sign | Evidence |
|---|------|----------|
| T1 | **24 uppercase + tracked labels**: `TARGET`, `MIB BROWSER`, `RESULTS`, `INSPECTOR`, `SYSTEM LOG`, `SNMP VERSION`, `COMMUNITY STRING`, `DESCRIPTION`, `LIVE VALUE`, `OID`/`VALUE`/`TYPE` column headers (`tracking-wider`), `MANAGE MIBS`, `System Log Level` | `grep -rn "uppercase tracking" src/lib` → 24 hits across 10 components; visible in every screenshot |
| T2 | **Off-scale sizes**: `text-[10px]` (inspector section labels ×7, tree OID suffix, tree "loading..."), `text-[11px]` (raw hexdump meta), `text-[13px]` (every table cell, syslog) — three custom sizes layered on `xs`/`sm` | `InspectorPane.svelte`, `TreeNode.svelte:285`, `ResultsPane.svelte:955-960,1118`, `SystemLogPane.svelte:166` |
| T3 | **System font stack nobody chose** (`-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto...`) — the "template look" tell: grey surfaces + default sans + one stock accent | `src/app.css:24` |
| T4 | `font-bold` (700) modal titles; hierarchy by weight where size+color would do | `ConnectionModal.svelte:110`, `SetValueDialog` pattern |
| T5 | No explicit `tabular-nums` on non-mono figures (node counts, binding counts) | footer / `AppShell.svelte:120` |

### 3. Icons

| # | Sign | Evidence |
|---|------|----------|
| I1 | **Three Lucide sizes in circulation**: 12 px (`w-3`), 14 px (`w-3.5`), 16 px (`w-4`) in the same viewports | `ResultsPane.svelte` (arrows `w-3`, alert `w-3.5`, trash `w-4`), `MIBPanel.svelte`, `TreeNode.svelte:264-270` |
| I2 | **Sort-arrow noise**: a faint `ArrowUpDown` (`opacity-40`) rendered in *every* column header, including 22-column grids — half the grid header is ghost arrows that mean nothing until clicked | `ResultsPane.svelte:917,926,942,1068,1072,1076`; `ManageMibsPage.svelte:232,243,254`; `06-grid-view-dark.png` |

### 4. Spacing

| # | Sign | Evidence |
|---|------|----------|
| S1 | **57 arbitrary Tailwind values**: `w-[160px]`, `w-[60px]`, `w-[90px]`, `w-[40px]`, `min-w-[280px]`, `max-w-[400px]`, `h-[6px]`, `w-[5px]`, `w-[46px]`, `right-[200px]`, `max-h-[240px]`, ... | `grep -rn '\-\[' src/lib/*.svelte` → 57 hits |
| S2 | **The operation select clips its own label**: `w-[90px]` cuts "Get Table" to "Get Ta..." — a functional defect caused by a one-off width | `TargetBar.svelte:621`; `06-grid-view-dark.png` (top bar) |
| S3 | **Launch-screen dead zone**: tree rows at the top, the banner at the bottom, ~300 px of empty panel between them in the most-seen state of the app | `01-launch-ready-dark.png` |
| S4 | Fallback banner content column too narrow: message wraps to 4 lines, the "System Log" button wraps to 2 | `01-launch-ready-dark.png`; `MIBPanel.svelte:287-299` |

### 5. Copy

| # | Sign | Evidence |
|---|------|----------|
| Y1 | Wordy placeholder: "Enter OID or MIB name (e.g., sysDescr)" | `TargetBar.svelte:609`; every state screenshot |
| Y2 | **"(s)" pluralization**: "Get complete: 31 binding(s)", "Table complete: 2 row(s), 22 column(s)", "1 of 1 bindings", "5 missing cell(s)" — a `pluralize()` helper already exists and is used in the MIB banner, just not here | `TargetBar.svelte:300,385,475,508`, `ResultsPane.svelte:1174,1182`; `strings.json` |
| Y3 | Inconsistent ellipsis: `Filter...` / `Columns...` / `Loading...` (three dots) vs `Add…` / `Find OID or name…` (real ellipsis char) | `ResultsPane.svelte:852,874`, `ManageMibsPage.svelte:189,199,205`, `MIBPanel.svelte:244` |
| Y4 | Case drift in status strings: "walk complete: 31 binding(s)" vs "Get complete: 1 binding(s)" vs "Table complete: ..." for the same kind of sentence | footer in all state screenshots; `strings.json` |
| Y5 | **Inspector description renders raw MIB indentation**, so `sysDescr`'s description reads as centered text — a data-normalization bug surfacing as a design defect | `03-autocomplete-open-dark.png` (inspector); `InspectorPane.svelte:271` (`whitespace-pre-wrap` on undedented text from RFC1158) |
| Y6 | 10 px "loading..." micro-text in the tree | `TreeNode.svelte:283` |

### 6. Components & details

| # | Sign | Evidence |
|---|------|----------|
| D1 | **Circle-ring X close buttons** in both modals — the ring reads as a target, and the X floats in a white circle | `ConnectionModal.svelte:109`, old Manage MIBs dialog (superseded), `09-conn-modal-v2c-dark.png` |
| D2 | **MIB Names / Raw render as two separate pills**, not one segmented control | `ResultsPane.svelte:846-849`; `05a-*` |
| D3 | `FALLBACK` badge is a solid amber pill; `unresolved` badge is an outline amber pill; both uppercase — badges doing label work that sentence-case text could do quieter | `ManageMibsPage.svelte:271`, `TreeNode.svelte:280` |
| D4 | daisyUI default radii everywhere (8 px `rounded-box`) with pill badges mixed in — radius by size is not applied | throughout |
| D5 | Grid index columns are 96 px fixed, clipping names like `ifPhysAddress` in headers while data cells auto-size | `ResultsPane.svelte:740`; `06-grid-view-dark.png` |
| D6 | Tree OID suffix at 10 px / 60% opacity is below the readable floor for the app's densest column of meta | `TreeNode.svelte:285` |

### 7. Motion

| # | Sign | Evidence |
|---|------|----------|
| M1 | Modals and menus appear with **no transition** — instant pop, which reads as unpolished next to the `transition-colors` dividers | `ConnectionModal.svelte`, `MenuBar.svelte` dropdowns |
| M2 | No single easing/duration system defined (daisyUI defaults vary per component) | `app.css` (no motion tokens) |

### 8. Template look (the deciding check)

- Grey surfaces + system sans + stock violet accent + stock neon semantics: **the screen would fit any other CRUD tool with a different name.** This is not slop in the blobs-and-gradients sense — it is the *next* template unslop warns about: clean, but un-owned.
- The fix is a direction (below) that only this product could have.

---

## The direction

Per the unslop method, derived from what the product is (a MIB browser for SNMP engineers — see `CONTEXT.md`), not from what is trending:

- **Product.** A desktop tool for SNMP engineers: point at a Target, walk its MIB tree, read the values the device reports — and when it doesn't, see exactly why.
- **Character.** *Precise, calm, instrumented.* The register of lab network gear: quiet rack units, monospace readouts, status LEDs you trust.
- **World.** **Network instrumentation.** Dark rooms and rack hardware, terminal monospace, LED status. The world's tint is a cool graphite with a faint teal cast — the surfaces are already close (daisyUI dark's 252° hue); the accents move to the world (teal) and off it (violet, magenta, neon amber).
- **Moments.** Two places carry the richness, both from what the product shows: (1) the **Result Set value column** — the live data in crisp tabular mono is the content people come for; (2) the **Hex View** — a full hex+ASCII dump rendered like a cathode readout (mono, muted grid lines, one accent on the type line). No generated imagery: a desktop tool's moments are its data.
- **Signature.** The **status LED**: the footer connection dot, treated as an instrument indicator — neutral (grey) at rest, amber while a connection is in flight, and a *soft, single* glow only while live. It is the one element allowed to be "on." Pairing rule that travels with it: **MIB names in the sans face, OIDs/values in the mono face** — that two-face pairing is the product's visual identity, used consistently in tree, address bar, results, inspector, and log.

### Consequences for tokens

| Layer | Decision |
|-------|----------|
| Neutrals | Keep the daisyUI dark ramp (already cool graphite); keep the light ramp with the existing `base-content` darkening (`app.css:10`). No surface tinting beyond that — the world shows in the accent, not the chrome. |
| Primary (accent) | Violet → **instrument teal**. Starting values, to be tuned in the screenshot pass: dark `--color-primary: oklch(76% 0.11 196)` / content `oklch(20% 0.05 200)`; light `oklch(45% 0.12 196)`. On dark, the **primary button becomes off-white from the ramp with dark text** (unslop's deliberate-looking dark-theme primary); teal stays for selection, focus, the live LED, and the one key figure. |
| Secondary / accent tokens | `secondary` → neutral (retire magenta); `accent` → the same teal family (so `text-accent` data marks stop being a fourth hue). |
| Semantics | Tone the stock neon down to sit inside the ramp: warning `oklch(80% 0.13 84)` (dark) — the fallback banner becomes a *toned amber strip*, not a light bulb; error `oklch(68% 0.15 15)`; success `oklch(74% 0.14 163)`; info `oklch(72% 0.10 235)`. |
| Type | **IBM Plex Sans** (interface) + **IBM Plex Mono** (OID, value, path, hex, log) — one family, two faces, both fitting "precise, technical, calm"; bundled via `@fontsource` (desktop app, no web-font cost concerns). Fallbacks: `system-ui` / `ui-monospace`. Scale: 12 meta · 13 secondary · 14 body/tree/nav · 15 inputs and table cells · 16 modal titles. Weights 400/500, 600 only for figures. `tabular-nums` on all figures. Sentence case, zero tracking under 16 px. |
| Icons | Lucide only (already true), **16 px, stroke 1.5** (`strokeWidth={1.5}`), colored by text role, 8 px gap to label. Sort arrows: **active column only** — drop the `opacity-40` ghosts from inactive headers. |
| Spacing | 4 px grid. Replace the 57 arbitrary values with scale steps (`w-[160px]`→`w-40`, `w-[60px]`→`w-16`, `w-[90px]`→`w-24` (fixes S2 — "Get Table" fits), `w-[40px]`→`w-10`, `min-w-[280px]`→`min-w-70`, `max-w-[400px]`→`max-w-100`, `w-[46px]`→`w-12` (log level), `h-[6px]`/`w-[5px]` resize handles → `h-1.5`/`w-1.5`). Row heights 32–36; panel padding 16; page padding 24. |
| Radius | Inputs/buttons 8 (keep `rounded-box`), modals 12, tags/badges 4–6 (no pills for badges), segmented joins shared radius. Nested radius respected. |
| Motion | One curve `cubic-bezier(0.22, 1, 0.36, 1)`. Modals/menus: 160–200 ms fade + 6 px rise. Toggles/hover: 120–150 ms color/surface. LED: one 2 s soft pulse **only while `connecting`**. Never animate layout properties. |

---

## The plan

Work runs like a designer's redline pass: one layer at a time, across the whole app, each pass a small PR verified by the harness. Spacing before color-before-copy ordering matters because label case changes text width, which changes what fits.

### Phase 0 — Re-baseline (½ day)

1. ~~Run `npm run ux:assess` → new baseline set~~ **DONE (2026-11-03):** `docs/ux/2026-11-03/` — all 6 spec files passed in 3:36; 12 states × both themes + error/keyboard/log captures + all JSON probes; run-meta: commit `7733df3`, 206 nodes, agent `127.0.0.1:11611`. Timing medians vs the 2026-09-12 set: launch→Ready 745 ms (was 2211), autocomplete 252 (254), Go→feedback 49 (52), walk complete 124 (137), table complete 90 (97), table→grid 20 (24) — at or better across the board, so the 09-12 performance numbers in the August findings doc are safely superseded.
2. Manual `npm run dev` pass on a real display (devtools): confirm hover states, font rendering under WebKitGTK, and the states the harness can't reach (raw view, hex modal, set dialog, column panel).
3. Score the current set with the August aesthetic rubric (0–4 per state) — this is the "before" number.
4. **Gate:** present this plan's direction (above) for sign-off. Changing the direction later means redoing the token work, so everything after Phase 0 waits for the yes.

### Phase 1 — Token foundation (1–2 days) — **DONE**

Everything downstream reads these tokens, so they land first, in one PR.

**DONE (2026-11-11, PR #135 / `e63d85e`):** items 1–3 landed as one PR — theme overrides in `src/app.css` (dark + light), `@fontsource` IBM Plex pair wired through `@theme` font tokens, motion curve + 140/180 ms in `@theme static`, and the dead `oklch(var(--p) / a)` CSS migrated to `color-mix`/`var(--color-*)` in the five components + the `app.css` scrollbar. Verification (item 4) recorded below.

1. `src/app.css`: add a full `@plugin "daisyui"` custom theme (or `[data-theme]` overrides) with the palette from the direction — teal primary, neutral secondary, toned semantics, kept neutrals.
   - **Corrected 2026-11-03 (implementation finding):** daisyUI 5.7.47 theme files define only `--color-*` values — the `oklch(var(--p) ...)`, `oklch(var(--bc) ...)`, and `var(--fallback-pc, oklch(var(--pc)))` references in component `<style>` blocks (`TreeNode`, `InspectorPane`, `ResultsPane`, `SystemLogPane`, `MainContent`, plus the `app.css` scrollbar) are **dead CSS** and do *not* propagate from a theme swap. Phase 1 therefore migrates those to `var(--color-*)` / `color-mix(in oklab, ...)` forms (the pattern already used by `TreeNode`'s find-hit highlight) as part of the same PR.
2. Bundle IBM Plex Sans + IBM Plex Mono (`@fontsource`), set `--font-sans`/`--font-mono` (Tailwind v4 theme vars) so the existing `font-mono` usages — the app's best decision — pick up the new face automatically.
3. Motion tokens: define the single easing curve + duration set as CSS vars; apply to daisyUI `--animation-*` where relevant.
4. Verify: re-run the screenshot pass only (a quick `ux:assess` run); confirm both themes, no contrast regressions (the light-theme `base-content` fix from UX-16 must survive), axe clean.

   **DONE (2026-11-03):** before/after pair in `docs/ux/2026-11-03/` (baseline re-captured from a worktree at `cff02d4` — the first after-run had clobbered the same-date baseline) vs `docs/ux/2026-11-03-phase1-after/`. Results: all 6 specs pass; **axe at parity in 8/9 states, syslog-open improved by −2** (two pre-existing `text-primary` menu-icon violations resolved by the teal — violet failed where teal clears); timing medians within noise (worst ±12 ms, walk metrics faster); both themes render the new palette with the UX-16 light `base-content` fix intact. One contrast correction from the pass: the selected tree row keeps **base-content text on the 20% teal band** — the first cut used `--color-primary-content` (the button-on-teal pairing), which measured 1.4:1 on the tint and failed axe; base-content clears 9.8:1 (dark) / 14.3:1 (light), the 60% OID suffix 4.7:1 in both. Known non-regression, deferred to the color pass: light-theme semantic *text on white* (warning 1.75, error 2.74, success 2.02, info 2.25) — the stock light values fail identically, so this is pre-existing; the light semantic chips need darker values (and the dark find-mark chip, 2.8:1, likewise).

### Phase 2 — The nine passes, in order

Each pass: make the change → `cargo fmt` + `cargo test` + `npx tsc --noEmit` + `npx svelte-check` (repo pre-commit checklist) → screenshot pass → diff against baseline → commit.

1. **Spacing.** Retire the 57 arbitrary values to scale steps (table in the direction); fix the op-select clip (S2); rebalance the fallback banner (S4: message column `flex-1`, single-line button); pull the tree panel's dead zone (S3) by letting the banner sit directly under the tree block with 16 px rhythm. *Files: `TargetBar`, `ResultsPane`, `MIBPanel`, `SystemLogPane`, `ManageMibsPage`.*
2. **Color.** The Phase 1 tokens now *get used consistently*: primary button off-white on dark; teal for selection/focus/LED only; `text-accent` data marks moved to the toned semantic set (C4); the amber banner toned (C3). Confirm no component hard-codes a violet hex/oklch. *Files: `ResultsPane` (`.inspector-selected`), `TreeNode` (`.tree-row.selected`, `.find-hit`), `MIBPanel` (banner), `AppShell` (LED).*
3. **Icons.** One size + stroke app-wide: every `w-3`/`w-3.5` Lucide → 16 px, `strokeWidth={1.5}`; sort-arrow policy — ghost `ArrowUpDown` removed from inactive headers, arrow only on the active column (I2). *Files: all components using `lucide-svelte`.*
4. **Copy.** Short placeholders ("OID or MIB name", "Filter"); `pluralize()` for every count (Y2 — `TargetBar` status strings, `ResultsPane` footers); normalize ellipses to `…` (Y3); one case convention for completion messages ("Walk complete: 31 bindings") (Y4); **dedent MIB description text before display** so `sysDescr` stops reading centered (Y5 — normalize in `mibNodeDetails` consumer or a small `dedent()` in `format.ts`); tree "loading..." → 11–12 px (Y6). *Files: `TargetBar`, `ResultsPane`, `InspectorPane`, `format.ts`, `ManageMibsPage`.*
5. **Type.** Sentence case, zero tracking on all 24 labels (T1) — `text-xs font-medium text-base-content/60` is the new section-label recipe; sizes to the 12/13/14/15/16 scale (T2); modal titles `font-semibold` 16 px (T4); `tabular-nums` on figures (T5). *Files: the 10 components flagged by the grep.*
6. **Layout.** Modal close buttons: circle-ring X → quiet square ghost in the corner (D1, both modals); MIB Names/Raw → one segmented `join` control (D2); grid index columns: min-width from content, names no longer clipped at 96 px (D5); fallback banner placement per pass 1. *Files: `ConnectionModal`, `SetValueDialog`, `HexViewModal`, `ResultsPane`.*
7. **Details.** Radii by size (D4); badges as quiet 4–6 px tags in sentence case ("fallback", "unresolved") (D3); tree OID suffix 11–12 px at 50% (D6); focus ring: 2 px teal + 2 px offset, visible in both themes; hover states stay surface-steps (no new tints). *Files: `TreeNode`, `ManageMibsPage`, `app.css`.*
8. **Visuals.** No generated imagery (desktop app). The two moments get their care here: the value column (mono, tabular, wrapped with the existing 3-line clamp) and the Hex View (muted offsets, one accent on the type line, consistent 11 px mono). Empty states keep their calm single-line treatment.
9. **Motion.** Modal/menu open: 160–200 ms fade + 6 px rise (M1); toggles/hover on the 120–150 ms curve (M2); the LED's single pulse while `connecting` — the one allowed living element; verify nothing animates layout properties. *Files: `app.css`, `ConnectionModal`, `MenuBar`.*

### Phase 3 — Verification and sign-off (½–1 day)

1. Full `npm run ux:assess` re-run → `docs/ux/<date>/` after-set.
2. Side-by-side diff of before/after per state (the 2026-08-22 findings doc is the format precedent: per-state table, screenshot refs).
3. Re-score with the August aesthetic rubric; the target is not a higher "pretty" number — it is passing the **template check**: the screen should no longer fit another product's name.
4. Regression gates: axe violations not up, DOM audit stable, timing medians within noise of the 2026-11-XX baseline (Xvfb-relative), feature e2e suite green (`npm run test:e2e`), pre-commit checklist green.
5. Manual dev pass on a real display for the states the harness can't reach (raw view, hex modal, set dialog, column panel, Manage MIBs page).

---

## Definition of done

- [x] Phase 0 baseline recorded in `docs/ux/2026-11-03/` (2026-11-03) — "before" aesthetic scores still pending (Phase 0.3).
- [x] Direction signed off (Phase 0 gate) and written into this doc's status line (2026-11-03).
- [x] Token foundation merged: custom daisyUI theme (both themes), IBM Plex pair, motion tokens (2026-11-11, PR #135 / `e63d85e`).
- [ ] Nine passes merged, each with a before/after screenshot diff and green pre-commit checklist.
- [ ] No violet, no magenta, no neon amber in any state screenshot; no uppercase tracked label; no `text-[10px]`/`text-[11px]` off-scale sizes; no `w-[..px]` one-offs left (resize handles exempted as `h-1.5`/`w-1.5`); op-select shows "Get Table" unclipped; `sysDescr` description left-aligned.
- [ ] After-set `ux:assess` run: axe not regressed, timings within noise, feature suite green.
- [ ] Template check passed and recorded in the findings table.
- [ ] Findings written up in the `docs/scratch/` style of the 2026-08-22 doc (per-state table, screenshot refs, scores).

## Ticketing

Per the repo's issue-tracker convention, file under `~/git/scout-tickets/` (one ticket per pass, labelled `ready-for-agent` once the direction is signed off): tokens, spacing, color, icons, copy, type, layout, details, motion, verification. The copy and type passes touch the same label strings — batch their tickets per pass, not per string.

## Risks / known limitations

- **daisyUI token coupling.** Components reference `oklch(var(--p) ...)` and `var(--color-warning)` in `<style>` blocks; the theme swap is exactly what makes that safe, but any component with a hard-coded color (none found in the audit, but the pass-2 verification re-checks) would survive the swap invisibly.
- **WebKitGTK font rendering.** IBM Plex under WebKitGTK at 12–13 px needs the manual dev pass to confirm hinting quality before the font decision is locked; the system-stack fallback stays in the font stack either way.
- **Light theme is the weak twin.** August's UX-16 contrast work lives in light-theme tokens; the semantic tone-downs must be re-verified against WCAG AA in light (the harness's axe probe covers this).
- **No pixel-diff tooling.** Before/after comparison is a human side-by-side (the established practice in this repo); the harness makes the inputs reproducible, not the judgment automatic.
- **Timing baselines are Xvfb-relative** — as always in this repo, relative, not absolute.
- **Copy changes touch `strings.json`-verified e2e assertions** — some feature specs assert on status strings; the copy pass updates those assertions in the same PR.

## Credit

Methodology adapted from **unslop** by Damir Sokolovsky ([@heydamir](https://x.com/heydamir)), [github.com/heydamir/unslop](https://github.com/heydamir/unslop), CC BY 4.0 — the signs list, the direction method (product/character/world/moments/signature), and the nine-pass ordering. Evidence and verification: this repo's UX assessment suite per `docs/specs/2026-08-22-ux-assessment.md`.
