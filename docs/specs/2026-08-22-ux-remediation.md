# UX Remediation — Scout MIB Browser

**Date:** 2026-08-22  
**Status:** Proposed  
**Source:** `docs/scratch/ux-findings-2026-08-22.md` (findings UX-01…UX-17, evidence in `docs/ux/2026-08-22/`) plus new finding UX-18 added to that doc.

## Problem Statement

The 2026-08-22 UX assessment found the app functionally solid and fast, with three clusters of user-facing problems:

1. **Error/edge-state communication** — a failed Test Connection shows only "Receive" (no cause, no next step); a broken MIB leaks a garbage node into the tree presented as real; the fallback banner for unparseable MIBs is stuck on screen for the whole session with no way to dismiss it.
2. **Accessibility & keyboard operability** — tree leaves and menu items are mouse-only, inputs lack accessible names, scrollable regions aren't focusable, there's no `<h1>`/landmark structure, and the connection modal has no focus trap or Escape-to-close. The single most important action (choosing a MIB node) is impossible with keyboard alone.
3. **A trust issue** — the connection modal tells users their credentials are *not* persisted, but they actually are (plaintext config file).

## Solution

Remediate all 18 findings. Decisions made during triage of the findings doc:

| # | Severity | Finding | Decision |
|---|----------|---------|----------|
| UX-01 | Critical | Test Connection failure non-actionable | **Agree** with finding's fix |
| UX-02 | High | False "credentials not persisted" disclaimer | **Decided:** keep persistence (convenience), make UI truthful, add opt-out toggle + scrub existing creds on disable |
| UX-03 | High | Tree leaves not keyboard-focusable | **Agree** with finding's fix |
| UX-04 | High | Inputs/selects lack accessible names | **Agree** with finding's fix |
| UX-05 | High | Menu items not keyboard-reachable | **Agree** with finding's fix |
| UX-06 | Medium | Broken MIB leaks garbage node | **Decided:** dimmed + "unresolved" badge; nodes stay queryable but results are annotated |
| UX-07 | Medium | Stale tree selection overrides typed OID | **Agree** with finding's fix |
| UX-08 | Medium | Scroll regions not focusable | **Agree** with finding's fix |
| UX-09 | Medium | No `<h1>` / weak landmarks | **Agree** with finding's fix |
| UX-10 | Medium | Modal: no focus trap / Esc / aria-labelledby | **Agree** with finding's fix |
| UX-11 | Low | "regex fallback" jargon in banner | **Agree** with finding's fix |
| UX-12 | Low | Red "Disconnected" dot at startup | **Agree** with finding's fix |
| UX-13 | Low | Icon inconsistency (emoji vs SVG) | **Decided:** standardize on Lucide (`lucide-svelte`) — rationale below |
| UX-14 | Low | "↳ Wrap" label cryptic | **Agree** with finding's fix |
| UX-15 | Low | "1 nodes" grammar | **Agree** with finding's fix |
| UX-16 | Low | Light-theme low contrast | **Agree** with finding's fix |
| UX-17 | Low | PARTIAL badge reads as positive | **Agree** with finding's fix |
| UX-18 | Medium (new) | Fallback banner not dismissable | **Decided:** session-scoped dismiss + compact header indicator to reopen |

## User Stories

1. As an SNMP engineer, I want a failed Test Connection to tell me why (no response from host:port) and what to check, so that I can fix the connection instead of guessing.
2. As a user entering V3 passphrases, I want the UI to state honestly whether they will be saved to disk, so that I can decide knowingly where I'm typing secrets.
3. As a user on a shared or sensitive machine, I want a toggle to stop credentials from being saved to the config file, so that no secrets persist in plaintext.
4. As a user who turns off credential saving, I want already-saved credentials removed from the config file immediately, so that nothing secret remains on disk.
5. As a keyboard-only user, I want to move focus through MIB tree nodes and select one with arrow keys + Enter, so that the core flow works with zero mouse.
6. As a screen-reader user, I want every input (host, port, OID, filter) and protocol select announced with a proper name, so that I always know which field I'm in.
7. As a keyboard-only user, I want to open menus and reach their items (Add MIB Directory, Manage MIBs, log levels) without a mouse, so that no feature is mouse-gated.
8. As a user loading a MIB that fails to parse, I want its fallback nodes visually marked as unresolved (dimmed + badge), so that I don't mistake them for real entries.
9. As a user who queries an unresolved fallback node, I want the result annotated as coming from a parse artifact, so that a no-such-instance doesn't look like an agent or network problem.
10. As a power user, I want the OID I typed in the address bar to be the one that runs when I click Go, so that the visible input matches the executed action.
11. As a keyboard user, I want to focus and scroll the results list, grid, and System Log panes with keys, so that off-screen rows and columns are reachable.
12. As a screen-reader user, I want a proper document structure (one `<h1>`, landmarks for tree/results/log), so that I can build a page outline and jump between regions.
13. As a keyboard user, I want the connection modal to trap focus, close on Escape, and be announced as a dialog with its title, so that I don't tab into the background or lose context.
14. As a non-expert, I want MIB load warnings in plain language ("couldn't be fully parsed"), so that I'm not confronted with implementation jargon like "regex fallback".
15. As a user at launch, I want the connection indicator neutral until a connection attempt actually fails, so that red always means something went wrong.
16. As a user scanning the UI, I want one consistent icon system (no emoji mixed with SVG), so that the tool looks professional and icons are predictable.
17. As a first-time user, I want the Wrap control to say what it does ("Wrap long values"), so that I can find value wrapping without guessing.
18. As a user reading counts, I want correct pluralization ("1 node", not "1 nodes").
19. As a user in the light theme, I want all text to meet WCAG AA contrast, so that nothing is unreadable.
20. As a user getting partial results, I want the PARTIAL badge in a warning (amber) hue rather than teal/green, so that it reads as caution, not success.
21. As a user with a broken MIB loaded, I want to dismiss the fallback banner after reading it, so that it doesn't sit permanently in my panel.
22. As a user who dismissed the fallback banner, I want a small indicator showing it can be reopened, so that the information isn't lost.

## Implementation Decisions

### UX-01 — Actionable Test Connection failure

Map transport-level failures to one actionable message that names the host:port and suggests checks (agent listening, host/port correct). Example wording: "Connection failed — no SNMP response from 127.0.0.1:11699. Check the host/port and that the agent is listening." Preserve the raw error string in the System Log for debugging.

### UX-02 — Truthful credential persistence with opt-out (decided)

- Keep persisting connection settings by default (convenience); make the UI honest instead of changing the behavior.
- Replace the false disclaimer in the Connection modal with a truthful note: connection settings, including credentials, are saved to the local config file for convenience. When the toggle is off, state that credentials will not be saved and must be re-entered on each launch.
- New setting `save_credentials` (boolean, default `true`) in the `[ui]` config section, exposed as a toggle beside the note in the Connection modal. The toggle's own state is persisted so the opt-out survives restarts.
- Scope of "credentials" when the toggle is off: **community string and all V3 credential fields** (username, auth passphrase, priv passphrase). Host, port, and SNMP version continue to be saved for convenience.
- **Scrub on disable:** turning the toggle off immediately removes existing community/V3 credential values from the config file on disk — not just stops future writes.

### UX-03 — Keyboard-operable MIB tree

ARIA tree pattern: leaf nodes become focusable `treeitem`s with roving tabindex within the tree; ArrowUp/Down move between nodes, ArrowRight expands or moves into a child, ArrowLeft collapses or moves to parent, Enter/Space selects. Selection remains the single source that populates the address bar.

### UX-04 — Accessible names on inputs & selects

Associate each input (host, port, OID, filter) with a real `<label for>` or `aria-label`. Give the V3 Auth/Priv protocol selects visible labels ("Auth protocol", "Priv protocol") wired to the controls. Visual labels already exist in the modal — wire them up programmatically.

### UX-05 — Keyboard-reachable menus

Accessible menu-button pattern: top-level buttons open on Enter/Space/ArrowDown; items are focusable `menuitem`s navigable by arrow keys; Escape closes and returns focus to the trigger button.

### UX-06 — Marked fallback nodes (decided)

Fallback-derived tree nodes render **dimmed with an "unresolved" badge** next to the name. They remain selectable and queryable; when Go runs against one, the status text/result is annotated that the node came from an unresolved fallback parse, so the expected no-such-instance reads as a parse artifact rather than an agent problem. Keep the existing FALLBACK indicator on the affected module in Manage MIBs.

### UX-07 — Typed OID authoritative over stale tree selection

Track whether the address bar has been edited since the last tree selection ("dirty" state); when dirty, the typed value wins at Go time. Typing into the address bar clears the stale tree selection so the two can't silently diverge. The effective OID that will run must be unambiguous before execution.

### UX-08 / UX-09 — Document structure & focusable scroll regions

One `<h1>` (visually-hidden if needed). Panes exposed as landmarks: `navigation` for the MIB tree, a labelled region/`main` for results, `complementary` for the System Log; no nested or duplicated landmarks. Each scroll container (results body, grid, log) gets `tabindex="0"` plus an accessible name so it is focusable and keyboard-scrollable.

### UX-10 — Connection modal a11y

On open: move focus into the modal and trap the Tab cycle; return focus to the trigger on close. Close on Escape. Add `role="dialog"`, `aria-modal="true"`, and `aria-labelledby` pointing at the "Target Connection" title. Apply the same treatment to the Manage MIBs dialog for consistency.

### UX-11 — Plain-language fallback banner copy

Replace "N MIB(s) loaded via regex fallback" with plain language: "N MIB(s) couldn't be fully parsed and were loaded with reduced information." Implement together with the UX-18 rework of the same banner.

### UX-12 — Neutral connection indicator at startup

The status-bar connection indicator is neutral until a connection attempt has been made; it turns red only after an actual failure.

### UX-13 — Single icon system: Lucide (decided)

Standardize on **Lucide** via `lucide-svelte` components. Rationale:

- Stroke-based, single visual weight — reads consistently at the small sizes this dense tool uses.
- First-class Svelte 5 components with per-icon tree-shaking; MIT licensed; the de-facto pairing for Tailwind/daisyUI apps.
- Alternatives considered: **Phosphor** (multiple weights — more choice surface than this app needs), **Heroicons** (matches the currently hand-inlined paths, but its outline/solid duality invites exactly the mixing we're fixing), **Tabler** (consistent stroke set, less Svelte/Tailwind ecosystem momentum).

Migrate every emoji/text glyph and hand-inlined SVG to Lucide components: 🗑️ → `Trash2`, ⚠ → `TriangleAlert`, ✓ → `Check`, ✕ → `X`, ↳ Wrap → `WrapText` (plus the UX-14 label); tree folder/file icons, theme sun/moon, and menu check become their Lucide equivalents. No raw emoji or hand-pasted SVG paths in components thereafter.

### UX-14 — Wrap control discoverability

The wrap toggle gets a tooltip ("Wrap long values") alongside its icon + text label.

### UX-15 — Pluralization

Correct pluralization for node counts ("1 node", "N nodes") in Manage MIBs and anywhere counts render.

### UX-16 — Light-theme contrast

Audit the light theme against WCAG AA (4.5:1 for normal text); fix faint menu-bar and muted-value colors via daisyUI theme tokens rather than one-off overrides.

### UX-17 — Partial-results badge hue

The "PARTIAL RESULTS" badge moves from the teal/accent tone to an amber/warning hue consistent with the warnings banner.

### UX-18 — Dismissable fallback banner (new finding, decided)

Add a close (X) button to the fallback banner; dismissal lasts for the **session only**. When dismissed, show a compact amber indicator (dot with count) in the MIB panel header that reopens the banner on click. Rejected alternatives: auto-dismiss after N seconds (warnings shouldn't vanish silently) and persisting dismissal across launches (if broken MIBs are still loaded next launch, the warning is still true and should reappear).

## Testing Decisions

- **What makes a good test here:** assert external behavior only — rendered copy and ARIA attributes, config-file contents after an action, status text, axe results. Never component internals.
- **Rust (prior art: the config module's existing test suite):** `save_credentials = false` keeps credential fields out of the written TOML while host/port/version still persist; toggling off scrubs already-saved credential values from disk; default (`true`) round-trips unchanged.
- **Frontend unit (prior art: the connection-logic vitest tests with testing-library):** modal note/toggle states and their copy; fallback node renders dimmed + "unresolved" badge; banner dismiss → header indicator appears and reopens the banner; pluralization helper.
- **E2E (prior art: the WDIO feature suite on `data-testid` hooks + the UX probe suite):** Test Connection failure shows the actionable host:port message; keyboard-only core flow including tree selection via arrow keys + Enter; per-state axe pass free of the `select-name`, `label`, `scrollable-region-focusable`, and `page-has-heading-one` violations found in the assessment; typed OID wins over a stale tree selection; querying a fallback node annotates the result; banner dismiss/reopen works.
- **Before/after baseline:** re-run the UX assessment suite (`npm run ux:assess`) after remediation — its A1 screenshots, A3 keyboard script, and A6 axe probes are the recorded baselines to diff against.

## Out of Scope

- **Encrypted secret storage / OS keychain.** Saving-on remains plaintext in the local config; the toggle is an opt-out, not encryption.
- **Saved connection profiles / multiple targets.** The single last-used-target model is unchanged.
- **Performance work.** The ~30 s cold-start variance noted in the findings doc is a separate investigation, not part of this remediation.
- **MIB parsing changes.** Fallback parser behavior is unchanged; only how its artifacts are surfaced is affected.
- **Human task-based testing** (Approach D of the assessment plan) remains deferred.

## Further Notes

- Existing tickets in `scout-tickets/ux-assessment/` (#01–#09, status needs-triage) map to UX-01…UX-10; this spec resolves their open decision points — notably #02's direction (truthful copy + opt-out toggle) and #06's approach (mark, don't omit; allow query with annotation). Findings UX-11…UX-17 and new UX-18 are not yet ticketed; file them if tracker coverage is wanted.
- **Sequencing suggestion:** UX-02, UX-13, and UX-18 touch the same components (Connection modal, MIB panel) as several a11y items — batch those component edits to avoid churn. The a11y cluster (UX-03/04/05/08/09/10) is the largest body of work.
- **Preserve cited strengths:** feedback coverage, the unknown-OID error path, and terminology consistency are called out as model behavior in the findings doc — remediation must not regress them.
