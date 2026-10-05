# Manage MIBs Page (Redesign)

**Date:** 2026-10-04
**Status:** Proposed

## Purpose

Manage MIBs is currently a small modal (`ManageMibsDialog.svelte`, capped at `max-w-[560px] max-h-[70vh]`) that lists loaded MIB modules with a per-row Unload button. It does not scale to the number of MIBs a real system MIB directory loads, and it offers no way to sort, filter, bulk-unload, or add MIBs. This spec reworks it into a **full-page view** built on the same table pattern as the Results Pane, addressing the following:

1. **Too small** — the modal caps at 560px wide / 70vh tall; a system MIB directory (e.g. `/usr/share/snmp/mibs`, ~100–200 files) does not fit.
2. **No sort / filter** — rows render in backend order (name ascending) with no way to reorder or narrow the list.
3. **No bulk actions** — unloading is one row at a time; there is no select-many and no "unload all."
4. **Can't add from here** — adding a MIB directory requires closing the dialog and using File → Add MIB Directory.
5. **Can't add individual files** — only whole directories can be added; there is no way to add one or more specific MIB files.

## Current state

- `ManageMibsDialog.svelte` — modal; lists `LoadedMib` rows (name, path, node count, `FALLBACK` badge) with a per-row Unload.
- `LoadedMib` = `{ mibName, filePath, nodeCount, isFallback }` (from the `mib_loaded_list` command).
- Unload is **by module name** (`mib_unload(mib_name)`); rows are keyed by **file path** (so duplicate module names render as separate rows).
- The add-directory flow (picker → config → `mib_load_directories` → refresh) lives in `MenuBar.svelte`.
- The "refresh MIB state" sequence (nodeCount, fallbackMibs, oidNameMap, tree) is duplicated across `MenuBar.addMibDirectory`, `ManageMibsDialog.unloadMib`, and `AppShell.onMount`.

## Proposed design

A **full-page view** that occupies the main content region — the same area the Results Pane fills — shown in place of the MIB Browser + Results when active. It is a daisyUI `table` with a header of actions + filter, sortable columns, row selection, and bulk actions.

### Layout

```
┌────────────────────────────────────────────────────────────────────┐
│ Manage MIBs   [Add…] [Unload Selected] [Unload All]  [Filter…] │
├──┬───────────┬──────────────────────────────┬────────┬───────────┤
│ ☐│ Name      │ File Path                    │ Nodes  │ Status    │  ← sortable headers
├──┼───────────┼──────────────────────────────┼────────┼───────────┤
│ ☐│ IF-MIB    │ /usr/share/snmp/mibs/IF-…   │ 1,204  │           │
│ ☐│ SNMPv2-TC │ /usr/share/snmp/mibs/SNMP…  │ 312    │ FALLBACK  │
├──┴───────────┴──────────────────────────────┴────────┴───────────┤
│ 3 of 142 MIBs selected · 1,516 nodes loaded                        │
└────────────────────────────────────────────────────────────────────┘
```

- **Header row**: title; `Add…` (dropdown: `Directory…` / `File(s)…`, matching the `Save Results` dropdown); `Unload Selected` (disabled at 0 selection); `Unload All`; a `Filter…` input (mirrors the Results Pane filter).
- **Table columns**:
  - **Select** — per-row checkbox; the header checkbox is select-all (indeterminate on partial selection, same as the Results Pane column panel).
  - **Name** — module name (`mibName`). Sortable.
  - **File Path** — `filePath`, truncated with a full-path tooltip. Sortable.
  - **Nodes** — `nodeCount`, numeric, right-aligned. Sortable (numeric compare).
  - **Status** — `FALLBACK` badge when `isFallback`, else blank.
- **Footer**: `N of M MIBs selected · K nodes loaded` (K = total node count, matching the app footer).
- **Empty state**: "No MIBs loaded. Use Add MIB Directory to get started." (matches the MIB Browser empty state).
- **Loading state**: "Loading…" while `mib_loaded_list` is in flight.
- **No-match state**: "No MIBs match filter."

### Sorting

Follow the Results Pane pattern (`sortColumn`, `sortAsc`, `toggleSort(col)`, `ArrowUp`/`ArrowDown`/`ArrowUpDown` icons):

- Click a header to sort ascending; click again to descend; a third click clears back to the default.
- Default order = backend order (name ascending), so the initial view is stable.
- `Nodes` sorts numerically; `Name` / `File Path` sort lexicographically (case-insensitive).
- Set `aria-sort` on the active header (`ascending` / `descending`), omitted on the rest.

### Filtering

- A single `Filter…` input (case-insensitive substring) matching **Name**, **File Path**, and **Status** — so typing `fallback` isolates the broken MIBs.
- Filtering is display-only; selection and bulk actions operate on the **visible (filtered)** set (select-all selects the visible rows, consistent with a filtered view).

### Selection, bulk unload, and unload-all

- Per-row checkboxes + a header select-all checkbox (indeterminate when partially selected, matching the Results Pane column panel). **No per-row Unload button** — selection + `Unload Selected` covers the single-row case too.
- **Unload Selected** — enabled when ≥1 row is selected; unloads the selected modules.
- **Unload All** — unloads every loaded module; visually distinct (destructive styling).
- Both reuse the shared refresh path (below). Status text: `Unloaded N MIBs` / `Unloaded all MIBs`.
- Selection is keyed by **file path** (row identity); unload is dispatched **by file path** (path-based API, see backend) — see the duplicate-name edge case.
- **No confirmation.** The app has no confirmation dialogs today (a single Unload is immediate); `Unload Selected` / `Unload All` are immediate. Unloading is reversible by re-adding the directory, and the status text confirms the action.

### Add MIBs from the page

An `Add…` dropdown in the page header (matching the `Save Results` dropdown) with two actions:

- **`Directory…`** — `dialog_open_directory` → append to `mib.directories` config (dedup) → load → refresh. Same flow as the menu's File → Add MIB Directory.
- **`File(s)…`** — `dialog_open_files` (new multi-file picker, `rfd::pick_files()`) → append the chosen paths to a new `mib.files` config entry (dedup) → load → refresh.

Both:
- After a load, the table re-fetches `mib_loaded_list` so the new modules appear immediately.
- Call the same shared functions as the menu (File → Add MIB Directory keeps working).

**Source model — directories + explicit files.** Directories remain **live** sources (a new file dropped into a configured directory is picked up on reload); explicit files are a second, **persistent** source type. The resolver loads from *directories ∪ explicit files*, and its cache-retention step keeps explicit files (not just directory files) so they survive reloads and startup. *(Alternative considered: a files-only model where "add a directory" expands to its files — rejected because it loses the live-directory property.)*

- (Future) multi-directory picker and per-source (directory/file) removal are out of scope.

### State & navigation

- Reuse `S.manageMibsOpen` as "the Manage MIBs page is the active main view."
- `MainContent.svelte` renders `<ManageMibsPage />` when `manageMibsOpen`, else the MIB Browser + Results Pane — so the page fills the exact region the Results Pane fills.
- File → Manage MIBs… sets `manageMibsOpen = true`; a **Back** control (and/or re-selecting the browser) sets it false.
- The page holds local state: `mibs`, `loading`, `filterText`, `sortColumn`, `sortAsc`, `selected: Set<filePath>`.

### Component structure & refactor

- **New** `src/lib/components/ManageMibsPage.svelte` (replaces `ManageMibsDialog.svelte`; remove the dialog).
- **New** `src/lib/mibManager.ts` — centralizes the MIB lifecycle so the triplicated logic is gone:
  - `refreshMibState(status)` — sets `nodeCount` + `fallbackMibs`, re-fetches `oidNameMap` and `mibTree` (bumps `treeVersion`).
  - `addMibDirectory()` — picker → config dedup → load → `refreshMibState` (moved out of `MenuBar`).
  - `addMibFiles()` — multi-file picker → `mib.files` config dedup → load → `refreshMibState`.
  - `unloadMibs(paths: string[])` — dispatches `mib_unload_many(paths)` → `refreshMibState`.
- `MenuBar.svelte` and `AppShell.svelte` (startup) call the shared functions.
- Store: no new flags required beyond reusing `manageMibsOpen`.

### Backend

- **New command** `mib_unload_many(file_paths: Vec<String>) -> MibLoadStatus` — unloads the given files in a **single write-lock** (one index swap, one status) instead of N separate calls. For each path it resolves `loaded_files[path] → module_name` and unloads that module (reusing the existing per-module removal). The resolver already takes one write-lock per operation; a bulk command avoids N lock acquisitions and N frontend tree rebuilds.
  - **Path-based, not name-based.** Rows are keyed by file path, so the API is keyed by file path too. For the common case (one file per module) this is identical to name-based. For a true duplicate-module pair it unloads the whole merged module — see the edge case.
  - Frontend `unloadMibs(paths)` calls `mib_unload_many` for both `Unload Selected` and `Unload All` (all = every loaded file path). The old name-based `mib_unload` can be retired.
- **File loading + persistence.** The resolver loads from *directories ∪ explicit files*. Its `file_cache.retain` step must keep explicit files (today it drops any file not in a configured directory, which would silently lose individually-added files on reload/startup). A new `mib.files` config entry persists the explicit file paths; startup loads directories + explicit files.
- **New command** `dialog_open_files` — multi-file picker (`rfd::pick_files()`), mirroring `dialog_open_directory`.
- **Load command** — `mib_load_directories` gains an explicit-files parameter (or is replaced by `mib_load(directories, files)`); the frontend always passes the full configured set. A deleted explicit file drops out on the next load (consistent with directory behavior).
- No changes to `mib_loaded_list` / `LoadedMibInfo` — the columns map directly onto the existing fields.

### Edge cases

- **Duplicate module names** (e.g. `DUP-MIB-A` / `DUP-MIB-B`): rows are keyed by file path and both render (existing regression test). The node index merges nodes by OID and tags them only by module name, so the two files' contributions are not separately trackable. Unloading one of a duplicate pair therefore unloads the **whole merged module** (both files) — a documented caveat of the path-based API. **Follow-up:** per-file node provenance for true per-file precision (see Out of scope).
- **Unload All** leaves the resolver empty; the MIB Browser shows its empty state; re-adding a directory reloads (existing startup behavior).
- **Explicit files** persist across reloads and startup (`mib.files`); a deleted explicit file drops out on the next load (consistent with directory behavior). An explicit file that also lives in a configured directory is the same file (deduped by path) — no double-count.
- **Fallback MIBs** keep the `FALLBACK` badge; the separate MIB-Browser fallback banner (driven by `S.fallbackMibs`) is unaffected.
- **Filter + select-all**: select-all selects the currently visible (filtered) rows.

### Accessibility

- The page is a normal page (not a modal), so the dialog focus-trap is removed; standard keyboard access applies.
- Sortable headers expose `aria-sort`; the filter input has `aria-label="Filter MIBs"`; the select-all checkbox has an `aria-label`; row checkboxes are labeled by the module name; the `FALLBACK` badge carries readable text.
- Keep the accessible-name / axe audit coverage (see `ux-06-audit.spec.ts`) pointed at the page.

## Design decisions

- **Page over a larger modal.** The complaint is fundamentally about space and the table pattern; the Results Pane is a full pane, so a full-page view in the same region is the most apt and least surprising. A larger modal would still cap space and keep the "dialog" mental model. *(Alternative considered: an 80% modal — rejected for the space and pattern reasons.)*
- **Path-based unload API.** Rows are keyed by file path, so unload is dispatched by file path (`mib_unload_many(paths)` → resolve to module → unload module). It is a clean improvement over name-based and robust to a module loading from several files; the duplicate-module caveat is documented, and per-file provenance is the follow-up.
- **Immediate, no confirmation; bulk only.** Matches the app's existing no-confirm pattern (a single Unload is immediate) and drops the per-row button in favor of selection + `Unload Selected`.
- **Centralize the MIB lifecycle.** The refresh sequence is triplicated today; extracting `mibManager.ts` makes the page's add/unload correct by construction and removes the duplication.
- **One bulk command.** `mib_unload_many` keeps a bulk unload to a single lock acquisition and a single frontend tree rebuild.
- **`Add…` dropdown, not a separate feature.** `rfd` (like the OS pickers) can't select files and a directory in one dialog, so adding individual files is a second action; grouping it under one `Add…` dropdown (matching `Save Results`) keeps one entry point and a clean header.
- **Directories + explicit files.** Directories stay live sources; explicit files are a second persistent source type. Preserves the "point at a directory" workflow (new files in a dir are picked up on reload) while adding precise per-file control.

## Domain language

No new terms. A **MIB module** (the `mibName`) is the unit of load/unload; a file path is its identity for display. Consistent with `CONTEXT.md` and the existing `LoadedMib`.

## Tests

- **E2E** — update `test/specs/menus-settings.spec.ts` (the "Manage MIBs dialog" case) to the page, and add:
  - Page opens via File → Manage MIBs…; lists each seeded MIB with its node count; both `DUP-MIB` rows render.
  - Sort by Name (asc/desc) and by Nodes (numeric).
  - Filter narrows the list (e.g. `fallback` isolates `BROKEN-MIB`); no-match state.
  - Select-all + Unload Selected removes the selected modules; footer node count drops by their sum.
  - Unload All empties the list; re-adding a directory restores it.
  - Add MIB Directory from the page adds a directory's MIBs.
  - Add MIB Files from the page adds the chosen files; they persist across a reload.
- **Unit** — `mibManager` (directory/file dedup, status → store mapping) where the extracted logic is non-trivial.
- **Backend** — `scout-mib` tests: `unload_mib_many` (by file path: removes the resolved modules, preserves others, returns status; a duplicate-module path unloads the whole module); explicit-file retention across a directory reload; a deleted explicit file drops out.
- **A11y** — point the `ux-06` audit at the page (accessible names + axe).

## Out of scope / follow-ups

- Per-file node provenance (true per-file precision for duplicate module names) — the only way to unload one file's contribution independently of another file's for a shared module.
- Multi-directory picker; per-source (directory/file) removal from config.
- Exporting the MIB list; reordering / priority of modules.
- Hiding the TargetBar on the page (optional chrome cleanup).
