# E2E Feature Coverage

**Date:** 2026-08-21  
**Status:** Proposed

## Purpose

Expand the e2e suite (currently a single smoke spec) so it exercises the main features of Scout MIB Browser end to end: MIB loading and browsing, Target configuration, SNMP Operations against a live agent, Result Set display and manipulation, tolerance handling, and app-level settings.

Tests run headless via the existing WebdriverIO + Tauri embedded-driver harness (`npm run test:e2e`), driving the real app binary against a local mock SNMP agent.

## Domain language

Test names and assertions use the terms from `CONTEXT.md`: **Target**, **MIB Node**, **Variable Binding**, **Selection**, **Operation** (Get / Get Next / Walk / Bulk Walk / Set), **Result Set**, **Execution**.

## Scope

### In scope (covered by e2e)

| # | Feature | Where |
|---|---------|-------|
| 1 | App shell: launch, layout, status bar, node count, connection indicator | `AppShell` |
| 2 | MIB tree: lazy expansion, Selection populates address bar, context menu | `MIBPanel`, `TreeNode`, `ContextMenu` |
| 3 | Address bar autocomplete: search, keyboard navigation, selection | `TargetBar` |
| 4 | Target configuration: host/port persistence, Connection modal (v1/v2c/v3 fields), Test Connection success + failure | `TargetBar`, `ConnectionModal` |
| 5 | Operations: Get, Get Next, Walk, Bulk Walk against a live agent; progress + completion status; Stop/Esc cancellation | `TargetBar` |
| 6 | Table retrieval: walking a table node produces grid view with rows/columns/missing cells | `TargetBar`, `ResultsPane` |
| 7 | Result Set manipulation: filter, column sort, MIB Names/Raw OIDs toggle, Wrap toggle, Clear | `ResultsPane` |
| 8 | Tolerance handling: unknown OID → warning banner + partial results badge; regex-fallback MIB banner | `ResultsPane`, `MIBPanel` |
| 9 | Menus and settings: File / View / Settings menus, theme toggle, System Log pane + level filter, Manage MIBs dialog (list + unload) | `MenuBar`, `AppShell`, `ManageMibsDialog` |

### Out of scope (with rationale)

| Feature | Why excluded |
|---------|--------------|
| File → Add MIB Directory | Native OS directory picker is not drivable by WebDriver. MIB loading is exercised via config seeding instead (below). |
| Save Results → TSV/JSON/CSV | Native save dialog (`rfd`). Formatting logic is covered by unit tests in `src/lib/export.ts`; e2e asserts only that the export menu appears with three options. |
| Set Operation execution | Value entry goes through a JS `prompt()` dialog, which the embedded driver cannot reliably drive. E2E asserts the Set option exists in the operation selector; execution stays covered by backend tests. |
| SNMP protocol edge cases (retries, timeout tolerance details) | Covered by `cargo test` in `crates/scout-snmp`. E2E only needs one positive and one negative agent interaction. |

## Test environment

### Mock agent

Use the existing `scripts/snmpsim-test.py` helper: `snmpsim-command-responder` replaying `linux-full-walk.snmprec` (bundled with the `snmpsim` pip package), community `public`, SNMPv2c, port **11611**. The recording contains the full `system` subtree and `ifTable`, which anchors every operation test to known data.

Prerequisite: `pip install snmpsim` (documented in README alongside the other e2e system deps).

The helper is exposed as an npm script — `test:e2e:agent`: `python3 scripts/snmpsim-test.py --port 11611` — so developers can start the agent manually alongside `npm run dev` for interactive debugging. The e2e harness (`scripts/test-e2e.sh`) keeps owning its own agent lifecycle and does not use this script.

### Test MIBs

Create a curated, deterministic MIB set at `test/mibs/`:

- `SNMPv2-MIB`, `SNMPv2-SMI`, `SNMPv2-TC` — provides `sysDescr` (1.3.6.1.2.1.1.1.0), `system` subtree
- `IF-MIB` — provides `ifTable` (1.3.6.1.2.1.2.2) for table-retrieval tests
- `BROKEN-MIB` — intentionally malformed file to trigger the regex-fallback path

Copy from `references/ireasoning/mibbrowser/mibs/` via a one-time script (`scripts/prepare-test-mibs.sh`) so the suite does not depend on `/usr/share/snmp/mibs` being present.

### Config isolation

The app reads `~/.config/scout/config.toml` at startup (see `src-tauri/src/config.rs`). To keep tests hermetic:

1. `scripts/test-e2e.sh` creates a temp dir and sets **`XDG_CONFIG_HOME`** to it before launching WDIO (the Tauri service inherits the environment).
2. Pre-seed `$XDG_CONFIG_HOME/scout/config.toml`:
   ```toml
   [mib]
   directories = ["<repo>/test/mibs"]

   [target]
   host = "127.0.0.1"
   port = 11611
   version = "v2c"
   community = "public"
   ```
3. The user's real config is never touched; nothing to restore on failure.

### Harness changes (`scripts/test-e2e.sh`)

Current lifecycle: kill stale vite → start Vite :5173 → wait for readiness → `xvfb-run wdio` → cleanup. Extend it:

1. Prepare temp `XDG_CONFIG_HOME` + seed config (above).
2. Start snmpsim (`python3 scripts/snmpsim-test.py --port 11611`) in the background; wait ~3 s for readiness (UDP has no handshake to poll; a fixed settle is acceptable — note as future improvement: probe with `snmpget`).
3. Run WDIO under Xvfb (unchanged).
4. Cleanup: kill snmpsim and Vite, remove temp config dir (in a trap so cleanup runs on failure too).

`wdio.conf.mjs` stays as-is: single instance, mocha BDD, 60 s timeout, `bail: 0`. Backend/frontend log capture is already on — tests may assert against captured backend logs when a UI assertion is ambiguous (e.g., proving an SNMP request was sent).

## Testability hooks

Selectors today are mostly class- or text-based (fragile under styling changes). Add minimal `data-testid` attributes before writing the specs:

| testid | Element | Component |
|--------|---------|-----------|
| `host-input`, `port-input` | Target host/port fields | `TargetBar` |
| `conn-gear` | Connection settings button | `TargetBar` |
| `oid-input` | OID/name address input | `TargetBar` |
| `op-select` | Operation dropdown | `TargetBar` |
| `go-btn`, `stop-btn` | Execute / cancel buttons | `TargetBar` |
| `autocomplete-list` | Search results dropdown | `TargetBar` |
| `theme-toggle` | Footer theme button | `AppShell` |
| `conn-indicator` | Footer connection status text | `AppShell` |
| `status-text` | Footer status message | `AppShell` |
| `node-count` | Footer node count | `AppShell` |
| `fallback-banner` | Regex-fallback alert | `MIBPanel` |
| `results-header`, `filter-input`, `clear-btn`, `names-toggle`, `wrap-toggle`, `save-btn` | Results controls | `ResultsPane` |
| `warnings-banner` | Tolerance warning alert | `ResultsPane` |
| `partial-badge` | "⚠ partial results" badge | `ResultsPane` |
| `results-footer` | "N of M bindings" bar | `ResultsPane` |
| `grid-table`, `grid-footer` | Table grid + row count | `ResultsPane` |
| `syslog-pane` | System log pane | `SystemLogPane` |

Existing hooks (`data-address-bar`, `data-tree-node`, `data-connection-panel`, `data-export-menu`) remain valid.

## Spec files and test cases

All specs live in `test/specs/`. The existing `example.spec.ts` is renamed into the app-shell spec below. Shared helpers go in `test/support/` (e.g., `selectTreeLeaf(name)`, `go(oid, op)`, `waitForStatus(pattern)`).

### 1. `app-shell.spec.ts` — launch and layout

- **launches with correct title** — page title is "Scout MIB Browser".
- **renders the full shell** — menu bar (File/View/Settings), `[data-address-bar]`, MIB panel, Results pane, footer all present.
- **loads seeded MIBs on startup** — `node-count` matches a known non-zero value; status text reaches "Ready"; MIB tree shows root nodes including `internet` / `mib-2`.
- **shows disconnected state before any test connection** — `conn-indicator` reads "Disconnected".
- **placeholder results prompt** — Results pane shows the "Select a MIB node and click Go" hint before any Execution.

### 2. `mib-tree.spec.ts` — browsing and Selection

- **expands a subtree lazily** — expand `internet` → `1.3.6.1.2.1` appears; expand to `system`; children load on demand (no full tree in initial DOM).
- **selecting a leaf populates the address bar** — click `sysDescr` → `oid-input` value is `1.3.6.1.2.1.1.1.0  sysDescr`.
- **context menu offers copy actions** — right-click a node → menu with "Copy OID" and "Copy Name"; clicking "Copy OID" updates `status-text` (accept either "Copied OID: …" or "Failed to copy" — see Risks).

### 3. `address-bar.spec.ts` — autocomplete

- **typing shows search results** — type `sysdescr` into `oid-input`, wait past the 150 ms debounce → `autocomplete-list` visible with a `sysDescr` entry showing name + OID.
- **keyboard navigation selects an item** — ArrowDown highlights first result; Enter populates `oid-input` and selects the matching tree node.
- **Escape dismisses the dropdown** — results list disappears, input value unchanged.
- **Go is disabled with empty input** — clear `oid-input` → `go-btn` disabled.

### 4. `connection.spec.ts` — Target configuration

- **host/port inputs persist to config** — type host `127.0.0.1`, port `11611`; after the test, read `$XDG_CONFIG_HOME/scout/config.toml` and assert both values were written (validates `persistTargetConfig`).
- **Connection modal opens from gear and Settings menu** — click `conn-gear` → `[data-connection-panel]` visible; close; open via Settings → "Connection…".
- **version toggle swaps credential fields** — v2c shows Community String; switch to v3 → community hidden, Username/Auth/Priv protocol + passphrase fields visible.
- **Test Connection succeeds against snmpsim** — with seeded target, click Test Connection → button reads "✓ Connected"; `conn-indicator` reads "Connected".
- **Test Connection fails with a message** — set port to an unused one (e.g., 11699), Test Connection → button reads "✕ Failed" and an error message paragraph is visible.

### 5. `operations.spec.ts` — Executions against the agent

Setup for this file: target = snmpsim (seeded config).

- **Get returns a Variable Binding** — select `sysDescr`, op Get, Go → status "Get complete: 1 binding(s)"; one result row with resolved name `sysDescr`, type `OCTET STRING`; `results-footer` reads "1 of 1 bindings".
- **Get Next returns the following binding** — op Get Next on `sysDescr.0` → exactly one binding, OID greater than the requested root.
- **Walk streams a subtree** — select `system` (1.3.6.1.2.1.1), op Walk, Go → status reaches "walk complete: N binding(s)" with N > 5; row count in `results-footer` equals N.
- **Bulk Walk works** — same subtree, op Bulk Walk → completes with the same binding count as Walk (recording is deterministic).
- **Stop cancels an active walk** — start a Bulk Walk on the full `ifTable` subtree (large), wait until progress counter shows > 0 bindings, click `stop-btn` → status "Walk cancelled"; `go-btn` re-enabled. Also verify Escape key performs the same cancellation (second run).
- **Go with no host is rejected** — clear host, Go → status "No target configured", no request sent (assert via captured backend logs: no new SNMP log line).

### 6. `table-retrieval.spec.ts` — grid view

- **walking a table node produces the grid** — select `ifTable`, op Walk, Go → status "Table complete: R row(s), C column(s)"; `grid-table` visible with an Instance column and column headers resolved to MIB names (e.g., `ifIndex`, `ifDescr`).
- **grid footer reports row count** — reads "R of R rows".
- **missing cells are flagged** — if the recording yields gaps, missing cells render in accent style and `grid-footer` shows "M missing cell(s)" (assert conditionally on the known recording).
- **filter applies to grid rows** — type an instance id fragment into `filter-input` → visible rows decrease, footer updates.

### 7. `results-pane.spec.ts` — Result Set manipulation

Setup: run a Walk of the `system` subtree once (deterministic N bindings).

- **filter narrows rows** — type `sysDescr` → only matching row(s) visible; "N of M" footer reflects the filtered count; clear filter restores all.
- **sorting by column header** — click Value header → rows sorted ascending with ↑ icon; click again → descending ↓; clicking OID header switches sort column.
- **MIB Names / Raw OIDs toggle** — default shows resolved names (`sysDescr`); click "Raw OIDs" → first column shows `1.3.6.1.2.1.1.1.0`; toggle back restores names.
- **Wrap toggle changes value rendering** — long OctetString values truncate by default; enabling Wrap removes truncation (assert computed style `word-break`/`white-space` change).
- **Clear resets the Result Set** — click `clear-btn` → rows gone, placeholder prompt returns, filter cleared.

### 8. `tolerance.spec.ts` — malformed data handling

- **unknown OID produces warnings, not a crash** — Get an OID that is in the MIB but absent from the recording (e.g., `ifName.9999`) → `warnings-banner` visible with kind + message; `partial-badge` ("⚠ partial results") shown; app remains responsive (status bar updates).
- **regex-fallback MIB banner** — with `BROKEN-MIB` seeded, `fallback-banner` in the MIB panel reads "1 MIB(s) loaded via regex fallback"; its System Log button opens `syslog-pane`.

### 9. `menus-settings.spec.ts` — app-level settings

- **File menu lists MIB actions** — File → "Add MIB Directory…" and "Manage MIBs…" present; clicking outside closes the menu.
- **Manage MIBs dialog** — open via File → Manage MIBs… → lists each seeded MIB with node count; click Unload on one (e.g., `IF-MIB`) → it disappears from the list, `node-count` in footer decreases, and `ifTable` is no longer expandable in the tree.
- **View menu toggles System Log** — View → "System Log" shows/hides `syslog-pane`; checkmark reflects state.
- **Settings log level filter** — Settings → System Log Level → selecting "Error" marks it active (✓) and filters pane entries to error-level.
- **theme toggle** — click `theme-toggle` → root `data-theme` flips dark↔light; preference persists in localStorage (`scout-theme`).

## Risks and mitigations

| Risk | Mitigation |
|------|------------|
| Clipboard write may fail in headless WebKit (no permission grant) | Context-menu test accepts the "Failed to copy" status as a pass for menu mechanics; revisit if the Tauri driver exposes clipboard permissions. |
| Walk-cancel timing is inherently racy | Use the largest available subtree (`ifTable`) so the walk runs long enough; assert on final state ("Walk cancelled"), not intermediate progress. Mark the spec file with a note if it flakes >10% of runs. |
| snmpsim recording content varies by package version | Pin expected values to ranges (N > 5 bindings) rather than exact counts where possible; record exact counts for `system` subtree once and assert equality there. |
| UDP agent readiness has no handshake | Fixed 3 s settle in the harness; first spec (`app-shell`) acts as a canary — if the agent is down, operation specs fail loudly with connection errors, not silent passes. |
| Test writes to user's real config | Eliminated by `XDG_CONFIG_HOME` isolation (see Config isolation). |

## Definition of done

1. `test/mibs/` + `scripts/prepare-test-mibs.sh` committed; suite runs without `/usr/share/snmp/mibs`.
2. All nine spec files above pass in a clean environment: fresh clone, system deps per README, `pip install snmpsim`, `npm run test:e2e:build`.
3. `data-testid` hooks from the table exist and are used by the specs (no class-based selectors for new assertions).
4. Harness cleans up snmpsim, Vite, and temp config on both success and failure paths.
5. Existing pre-commit checklist (`cargo fmt`, `cargo test --workspace --all-features`, `npx tsc --noEmit`, `npx svelte-check`) still passes — the testid additions must not break type checking.

## Phase 2: GitHub Actions

**Goal:** run the full e2e suite in CI as a manually triggered workflow (`workflow_dispatch`), Linux only — not on every MR yet (see graduation path below). Public repos get unlimited Actions minutes, so cost is a non-issue; the only real cost is wall-clock build time.

### Workflow shape (`.github/workflows/ci.yml`)

Two parallel jobs:

| Job | Contents | Expected runtime (warm cache) |
|-----|----------|-------------------------------|
| `checks` | `cargo fmt --check`, `cargo test --workspace --all-features`, `npx tsc --noEmit`, `npx svelte-check --threshold warning` | ~5–10 min |
| `e2e` | system deps → build with wdio feature → prepare test MIBs → `npm run test:e2e` | ~10–15 min |

- **Triggers:** `workflow_dispatch` only for now — manual runs from the Actions tab, nothing automatic per MR. Graduation path: once the suite proves stable (see DoD), add a `pull_request` trigger, then consider a required status check. No path filtering when that happens — the suite is cheap relative to the Rust build it rides on, and filtering adds maintenance surface.
- **Runner:** `ubuntu-latest` only. The webkit2gtk-driver/Xvfb harness is Linux-specific; Windows/macOS e2e is out of scope.
- **Caching (required for acceptable runtime):** sccache + `target/` + cargo registry for Rust, npm cache for JS. Cold first run ~30 min; subsequent MRs should land in the 10–15 min range.

### `e2e` job steps

1. Checkout.
2. Install system deps from the README's Ubuntu list (`xvfb webkit2gtk-driver libwebkit2gtk-4.1-dev libgbm1 libasound2-data …`) + `python3-pip`.
3. Toolchains: Rust stable, Node 20, `pip install snmpsim`.
4. `bash scripts/prepare-test-mibs.sh` (from Phase 1).
5. `cargo build --workspace --features scout-mib-browser/wdio` (same as `test:e2e:build`).
6. `npm run test:e2e` — the harness already handles Vite, Xvfb, snmpsim lifecycle, and temp-config isolation; no changes needed for CI beyond what Phase 1's harness rework provides.

### Flakiness policy

- Keep `bail: 0` so one failure never hides the rest of the signal.
- If the walk-cancel spec in `operations.spec.ts` flakes on shared runners, add a single automatic retry (e.g., `nick-fields/retry` or a WDIO spec-level retry) scoped to that file only — not a blanket suite retry, which would mask real regressions.
- While manually triggered, results are advisory and block nothing. Before graduating to per-MR runs or a required status check, the suite must prove itself stable enough to gate (three consecutive green runs).

### Later refinement (not part of Phase 2)

A prebuilt Docker image with all system deps baked in cuts setup from ~4 min to ~1 min and pins the environment. Defer until apt install time is actually painful; a standard runner + sccache is enough to start.

### Phase 2 definition of done

1. `ci.yml` merged with a `workflow_dispatch` trigger; a manual run completes both jobs within ~15 min warm.
2. Three consecutive green manual runs before considering per-MR triggers or a required status check (tracked as a follow-up).
3. Failure artifacts: captured backend/frontend logs (already enabled in `wdio.conf.mjs`) uploaded via `actions/upload-artifact` on failure for triage.

## Open questions

- Phase 2 graduation: what flake-rate threshold and minimum number of green runs justifies moving e2e from manual trigger to per-MR runs?
- Exact binding count for the `system` subtree walk against `linux-full-walk.snmprec` — to be recorded during implementation and pinned in `operations.spec.ts`.
- Is a second, smaller recording (e.g., 1–2 rows of `ifTable`) worth adding for faster table tests, or is the full walk fast enough?
