# SNMP Table Support

**Date:** 2026-08-23
**Status:** Proposed — investigation complete, not yet implemented
**Branch:** `snmp-table-support`

## Purpose

Scout already has a first-generation table mode (ticket `scout-mib-browser-mvp/09-table-retrieval.md`, merged): selecting a TABLE node with Walk/Bulk Walk pivots per-column walks into a grid. This spec investigates what "properly supporting SNMP tables" requires on top of that, and specifies the work in four phases:

1. **Table metadata** — parse INDEX/AUGMENTS so rows are identified by named index values, not opaque suffixes.
2. **Single-pass table walk** — one subtree walk instead of one walk per column, with streaming progress and Stop.
3. **Get Table operation + grid UI upgrade** — an explicit **Get Table** option in the Operation dropdown (replacing today's implicit Walk-triggered grid), decoded per-component index columns, column selection, row-cap rendering, per-column sort, honest export formats.
4. **Cell-level operations** (deferred) — Get/Set on specific rows and cells.

Settled design decisions (agreed 2026-08-23):

- **Get Table is the only path to the grid.** Walk/Bulk Walk on a table node behaves like everywhere else — flat subtree walk of column-major bindings. One operation, one behavior, regardless of node type. This is a deliberate behavior change (see Phase 3 and the e2e migration note in the test plan).
- **Default column set is all columns.** Under the single-pass walk, column selection is a display filter — it never reduces what is fetched (SNMP cannot skip subtrees mid-walk), so there is no network argument for a reduced default.
- **Multi-attribute indexes render as separate narrow columns**, one per index component, not a composite cell — so they stay individually sortable and filterable.

## Domain language

Terms from `CONTEXT.md` apply: **Target**, **MIB Node**, **Variable Binding**, **Selection**, **Operation**, **Result Set**, **Execution**.

The existing **Operation** term gains a sixth mode, **Get Table**: fetch an entire Table from the Target and display it as a pivoted grid of Table Rows. Unlike Walk (which returns flat Variable Bindings for any subtree), Get Table is only meaningful on a Table node and always produces the grid view. The `CONTEXT.md` Operation definition ("Walk, BulkWalk, Get, GetNext, or Set") should be extended accordingly during Phase 3 implementation.

This work also introduces table-specific concepts. Proposed additions to the `CONTEXT.md` glossary (fold in during Phase 1 implementation):

**Table**:
An SMI object type with `SYNTAX SEQUENCE OF <Entry>` — a set of rows, each identified by an index. What the user selects and walks.
_Avoid_: Grid, dataset, list

**Table Row**:
One instance of a Table, identified by its index values. A row in the grid is one Table Row; its cells are Variable Bindings.
_Avoid_: Record, entry (ambiguous with SMI ROW entry), line

**Index Column**:
A column named in the table's `INDEX`/`AUGMENTS` clause whose value participates in identifying a Table Row. May be marked `IMPLIED`.
_Avoid_: Key, primary key, row id

## Investigation findings

### What exists today (works end to end)

| Layer | Piece | Location |
|-------|-------|----------|
| MIB parsing | `SyntaxType::Table` / `TableRow`; `MibNode.is_table`; detected via mib-rs `obj.is_table()` / `is_row()` | `crates/scout-mib/src/lib.rs:39`, `loader.rs:92` |
| MIB tree | Table/ROW nodes sort as subtrees; lazy children work | `lib.rs:594` (`sort_nodes`) |
| Column detection | `Resolver::get_table_columns()` — all leaf objects under the table subtree | `lib.rs:655` |
| Engine | `SnmpEngine::walk_table()` — BulkWalks each column, pivots via `assemble_table_grid()` | `crates/scout-snmp/src/engine.rs:354`, `table.rs:69` |
| IPC | `mib_table_columns`, `snmp_walk_table` commands | `src-tauri/src/main.rs:320,511` |
| Frontend | Table auto-detection on Walk/Bulk Walk; grid view with Instance column, missing-cell flags, filter, TSV export, footer counts | `TargetBar.svelte:314`, `ResultsPane.svelte:356` |
| Tests | 7 Rust unit tests for grid assembly/columns; e2e `table-retrieval.spec.ts` (ifTable, 2 rows × 22 cols) | `table.rs:162`, `test/specs/table-retrieval.spec.ts` |

### Gaps

**G1 — Index metadata is discarded.** The loader reads `is_table()`/`is_row()` but never the INDEX clause. mib-rs 0.8 (already a dependency) exposes exactly what's needed: `obj.effective_indexes()` (follows AUGMENTS chains, works for columns too), and per index entry `name()`, `object()`, `implied()`, `encoding()` (Integer / IpAddress / FixedString / variable-length), and `fixed_size()` — plus `is_column()` and `is_index()` predicates. Consequences today: the Instance column is an opaque suffix (`"1"`, `"192.168.1.1"`, `"1.5"`); multi-attribute indexes can't be decomposed; nothing marks which columns are index columns.

**G2 — One walk per column.** `walk_table` calls `do_walk_loop` once per column OID, and `do_walk_loop` opens a fresh connection per call (`engine.rs:446`). A 30-column table means 30 connections + 30 walks where one connection + one subtree walk suffices. Worse, the table path has **no streaming, no progress, no Stop**: `snmp_walk_table` is a plain await (`main.rs:511`), and the frontend only arms the Stop button for Walk/Bulk Walk (`TargetBar.svelte:249`). A large table walk gives zero feedback and can't be cancelled.

**G3 — Row ordering is lexicographic-string.** `assemble_table_grid` collects instance IDs in a `BTreeSet<String>` (`table.rs:76`) → rows sort as *strings*: index `10` renders before `2`. The e2e recording (rows `"1"`, `"2"`) masks this.

**G4 — Column detection is over-broad.** `get_table_columns` takes every leaf under the table subtree. A nested sub-table inside a table (legal SMI, common in vendor MIBs) contributes its leaves as "columns" of the outer table; walking those drags the entire nested table into the grid with wrong row alignment. mib-rs `is_column()` gives the exact set (including columns added by AUGMENTS).

**G5 — Fallback parser has no table awareness.** `fallback.rs` always emits `is_table: false` and never looks at INDEX clauses, so any vendor MIB that fails mib-rs loses its tables entirely (flat walk, no grid).

**G6 — Grid UI limits.**
- Table mode is implicit: Walk/Bulk Walk on a table node silently switches to grid retrieval (`TargetBar.svelte:224`) — there is no explicit operation for it, so the user has no idea what will happen until results arrive.
- All columns are always walked — no column selection. Heavy for ifTable-class tables when the user wants three columns.
- No row cap: a 10k-row table renders 10k DOM rows in WebKitGTK (the flat binding view has the same theoretical issue but walks are usually smaller).
- No per-column sort (filter only); the Instance column can't be sorted numerically.
- Export is dishonest for grids: `handleExport`'s grid branch always writes TSV and returns, even when JSON or CSV was chosen (`ResultsPane.svelte:228`).
- Get on a table node passes the raw OID through (`scalar_instance_oid` skips Table/TableRow, `main.rs:392`) → confusing noSuchObject results.

**G7 — Test coverage is thin.** One table (ifTable), 2 rows, single snmpsim recording with no gaps: no multi-attribute index, no missing-cell path exercised, no ordering case with ≥10 rows, no cancel/progress.

## Design

### Phase 1 — Table metadata (`scout-mib`)

New public types in `crates/scout-mib`:

```rust
pub enum IndexEncoding { Integer, IpAddress, FixedString(usize), Variable }

pub struct IndexColumn {
    pub name: String,          // e.g. "ifIndex"
    pub oid: String,           // column OID
    pub implied: bool,
    pub encoding: IndexEncoding,
}

pub struct TableInfo {
    pub table_oid: String,
    pub name: String,                    // e.g. "ifTable"
    pub row_entry_oids: Vec<String>,     // base entry + augmented entries
    pub index_columns: Vec<IndexColumn>, // in INDEX clause order
    pub column_oids: Vec<String>,        // all columns incl. augmented, OID order
}
```

- `MibRsLoader` builds a `TableInfo` for every object where `obj.is_table()`: rows from the table's entry objects (including those reached via `augments()`), index columns from the base row's `effective_indexes()` (name via `index.object()`/`index.name()`, implied flag, encoding from `encoding()`/`fixed_size()`), columns as every object with `is_column()` whose parent row belongs to this table.
- `Resolver` gains `table_index: HashMap<String, TableInfo>` and `get_table_info(&self, table_oid) -> Option<&TableInfo>`.
- `get_table_columns` keeps its signature but returns `table_info.column_oids` when metadata exists, falling back to the current leaf heuristic otherwise (fallback-MIB tables).
- New Tauri command `mib_table_info(table_oid) -> Option<TableInfo>` (camelCase serde, matching existing conventions); frontend type `TableInfo` in `src/lib/types.ts`.
- **Fallback extractor:** best-effort table detection — an OBJECT-TYPE with `SYNTAX SEQUENCE OF X` whose corresponding `XEntry OBJECT-TYPE` carries an `INDEX { … }` clause marks the table node (`is_table: true`, `SyntaxType::Table`) and records index *names* only (encoding `Variable`). No encoding info means instance decoding degrades to the raw suffix — acceptable for a tolerance path.
- CONTEXT.md glossary gains **Table**, **Table Row**, **Index Column** (above).

### Phase 2 — Single-pass streaming table walk (`scout-snmp`)

Replace per-column walks with one BulkWalk of the table subtree, backing the new Get Table operation:

- New `SnmpEngine::get_table_streaming(runtime, target, table_oid, column_oids, sender, cancel_token) -> JoinHandle<()>`, mirroring `walk_streaming`'s shape. One connection; walk from the table root OID; terminate on subtree exit (existing `is_subtree_of` logic).
- New `TableRowSender` trait (parallel to `WalkBatchSender`): `send_progress(count: usize) -> bool` (false = client gone, stop), plus `send_complete(result: &TableResult)`. The app crate bridges it to Tauri IPC channels exactly like `ChannelWalkSender` (`main.rs:126`).
- **Pivot on known columns only:** a binding is assigned to a row iff its OID starts with one of the requested column OIDs. This excludes nested sub-table data (G4) from the grid. Note that `column_oids` here is the *display* selection: the walk fetches the whole subtree regardless, so selecting fewer columns saves rendering and pivot work, not network traffic — SNMP cannot skip subtrees mid-walk. (Nested sub-table data is still fetched over the wire — accepted cost for v1; see Risks.)
- **Row order = encounter order.** Walk order is numeric per sub-identifier, i.e. correct index order (`2` before `10`). Rows are emitted in first-encounter order, which fixes G3 without any string sorting. Drop the `BTreeSet` from `assemble_table_grid`; take an ordered row list.
- The `snmp_walk_table` command is renamed `snmp_get_table`, becomes channel-based (batch/complete channels) like `snmp_walk_streaming`, and reuses `WalkCancelToken` — Stop button and Esc work for table retrieval (G2). Frontend: `snmpWalkTable` → `snmpGetTable`.
- Progress message: binding count ("N bindings"), same status-bar pattern as Walk.
- Keep tolerance behavior: per-error retry with backoff, partial flag + warnings on incomplete columns (existing `inconsistent-rows` warning stays).

Why not keep per-column walks over a single session? Still N× the round trips for no benefit — a subtree walk returns everything in one pass. Why not stream *partial rows* (row appears after column 1, cells fill in as later columns arrive)? Walk order is column-major (all of col 1, then all of col 2), so a row only completes at the very end; partial-row streaming would need a new IPC message type and frontend merge logic for no correctness gain. Deferred to Phase 4 if ever wanted.

### Phase 3 — Get Table operation + grid UI upgrade (`src/lib`)

- **Get Table operation.** Add `getTable` to the `TargetBar` operations list (label "Get Table"). Remove the implicit branch at `TargetBar.svelte:224`: Walk/Bulk Walk on a table node now performs an ordinary flat subtree walk, and Get Table is the only entry point to grid retrieval. Get Table on a non-table selection (tree or typed OID — resolved via `mib_resolve_oid` before firing) shows "X is not a table — use Walk" instead of executing. Progress, Stop button, and Esc follow the existing walk pattern, driven by the Phase 2 channels.
- **Per-component index columns.** With `TableInfo`, split the instance suffix into one narrow grid column per index component, placed leftmost with visually distinct headers (subtle background + "index" marker); the raw full suffix stays available in a row tooltip. Decoding consumes sub-identifiers left-to-right — Integer takes 1, IpAddress takes 4, FixedString(n) takes n; **IMPLIED components are skipped** (their value is not present in the instance OID at all) and render blank with an "(implied)" tooltip. Stop at the first Variable component and treat the remainder as opaque (correct only when it is the final component; otherwise show raw). Tables without metadata (fallback MIBs) or with undecodable indexes fall back to a single raw "Instance" column — today's behavior. Per-component layout (rather than one composite cell) keeps each index attribute individually sortable and filterable, and makes row grouping scannable vertically.
- **Sticky header + sticky index columns.** `position: sticky` on the header row and the leftmost index column(s), so column names and row identity remain visible while scrolling wide tables in either direction.
- **Column selection.** A "Columns…" control above the grid opens a checkbox list of `TableInfo.column_oids` (index columns marked). Selection persists per table OID; default is all columns — under the single-pass walk this is display-only, so there is no network argument for a reduced default. The run always fetches every column; the selection is a client-side filter that shows/hides grid columns immediately and is restored as the display state of subsequent runs.
- **Row-cap rendering.** Render rows in chunks (500) with a scroll sentinel that appends the next chunk (IntersectionObserver); footer always shows the true total ("showing 500 of 12,344 rows"). Filter applies before chunking.
- **Per-column sort.** Clicking a grid header sorts that column (numeric-aware: integers/counters compare numerically; index columns included — a key advantage of per-component layout over a composite cell); default order is walk order. Re-click toggles direction; third click restores walk order.
- **Honest export.** Grid export honors the chosen format: TSV as today; JSON as `{ table_oid, columns: [{oid, name}], rows: [{ instance_id, cells: { [colOid]: value } }] }; CSV like TSV with proper quoting via the existing `export.ts` helpers.
- **Table Get guard.** Get/GetNext on a node with `isTable === true` shows a status hint ("ifTable is a table — use Get Table or Walk") instead of sending the raw OID, rather than surfacing noSuchObject noise.

### Phase 4 — Cell-level operations (deferred, recorded for completeness)

- Get a specific row: address-bar syntax like `ifTable.<index-values>` or a "Get row" context-menu action on grid rows.
- Set a cell: right-click a grid cell → existing Set flow with the full instance OID pre-filled.
- Partial-row streaming (incremental cell fill) if large-table UX demands it.

Out of scope by design: SNMP notifications/traps, table diffing across Executions, editing index values.

## Test plan

**`scout-mib` unit tests**
- Extend the `TABLE-TEST-MIB` fixture pattern (`loader.rs:408`) with: a multi-attribute index (Integer + IpAddress), an `IMPLIED` index, and an AUGMENTS table; assert `TableInfo` fields (index order, implied flag, encodings, augmented columns included).
- Fallback heuristic: vendor-style MIB text with SEQUENCE OF + INDEX → `is_table` true, index names captured.
- `get_table_columns` returns metadata columns when present; falls back to leaf heuristic otherwise; excludes nested sub-table leaves when metadata is present (G4 regression test).

**`scout-snmp` tests**
- Mock-agent integration (`tests/engine_mock.rs`): serve a table subtree with ≥12 rows on an integer index → assert row order `2` before `10` (G3 regression), single connection used (count server-side requests), bindings for a nested sub-table are excluded from the grid, cancel mid-walk yields partial + no hang.
- Unit: ordered-row assembly (no string sort); instance-suffix decoding per encoding rules — Integer/IpAddress/FixedString/Variable cases, multi-attribute splitting, IMPLIED components skipped (blank, not consumed), undecodable input falls back to raw suffix.

**E2E (`test/specs/table-retrieval.spec.ts`)**
- **Operation migration:** existing cases that do `go("walk")` on `ifTable` and expect the grid switch to `go("getTable")`. Add a regression case: Walk on `ifTable` produces flat bindings (binding list view, no grid) — locking in the one-operation-one-behavior decision.
- **Get Table gating:** Get Table on a non-table node (e.g. `sysDescr`) shows the "not a table" status hint and fires no query.
- The pinned `linux-full-walk.snmprec` ifTable data (2 rows) stays as the smoke case.
- Add a synthetic deterministic recording (small script in `scripts/`, committed output under `test/`) with: ≥12-row integer index (ordering), a two-attribute index (Integer + IpAddress, decoding), an IMPLIED index component, and one column with a missing row (missing-cell path). Point an additional spec file or new cases at it via the existing harness (second snmpsim instance on another port, same config-isolation pattern).
- Assert: per-component index columns render as separate sortable headers; Stop button cancels a Get Table run; Columns… selection reduces rendered columns (display-only); JSON export of a grid produces JSON.

## Risks and mitigations

| Risk | Mitigation |
|------|------------|
| mib-rs 0.8 API drift (`effective_indexes`, `Index` handles are new-ish public API) | Version already pinned to `0.8`; all Phase 1 extraction sits behind `MibRsLoader` so a future upgrade is one file. If a needed accessor is missing, degrade: metadata absent → today's heuristic behavior (tolerance principle). |
| Vendor MIBs with malformed INDEX/AUGMENTS clauses | Per-object tolerance: failure to build one table's `TableInfo` logs and leaves that table on the heuristic path; never blocks loading other MIBs. |
| Single-pass walk changes network profile (one long walk vs many short) — some agents time out or rate-limit differently | Cancellation + existing retry/backoff; per-column mode remains available as a fallback behind a setting if field reports emerge (do not build it speculatively). |
| Large tables (100k+ rows) exhaust memory before the grid is sent | v1 accepts full-grid-in-Rust (same as today); Phase 3 row cap bounds DOM cost. If memory becomes real, stream partial rows (Phase 4). |
| Synthetic e2e recording format is undocumented territory (snmpsim `.snmprec`) | Verify format against the bundled `linux-full-walk.snmprec` during implementation; if hand-authoring proves brittle, record from a local agent instead. |
| Column-major walk order means no incremental row streaming in v1 | Accepted: progress = binding count, which is still live feedback; Stop works. |

## Definition of done

1. `TableInfo` exposed end to end (loader → resolver → Tauri command → frontend type); CONTEXT.md glossary updated with **Get Table**, **Table**, **Table Row**, **Index Column**.
2. Get Table is in the Operation dropdown and the only path to the grid; Walk/Bulk Walk on a table node produces flat bindings (e2e regression case).
3. Table retrieval is single-pass with streaming progress and working Stop/Esc; row order is index-correct for integer indexes ≥ 10 rows (unit + e2e).
4. Grid renders per-component decoded index columns (IMPLIED blank, undecodable → raw Instance fallback), sticky header/index columns, column selection (persisted, default all), chunked row rendering, per-column sort, and TSV/JSON/CSV export that matches the chosen format.
5. Fallback parser marks tables best-effort; a malformed-INDEX MIB degrades without breaking load.
6. Pre-commit checklist green: `cargo fmt`, `cargo test --workspace --all-features`, `npx tsc --noEmit`, `npx svelte-check --threshold warning`; full e2e suite (`npm run test:e2e`) green including the migrated table spec.

## Open questions

- Phase 4 trigger: what row-count/UX pain threshold justifies cell-level Set and partial-row streaming?
