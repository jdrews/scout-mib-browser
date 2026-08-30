<script lang="ts">
  import { ArrowDown, ArrowUp, ArrowUpDown, Trash2, TriangleAlert, WrapText } from "lucide-svelte";
  import { S, clearResults } from "$lib/stores.svelte";
  import type { VariableBinding, SnmpValue, ResultSet, TreeNode, TableResult, TableRowData, TableCell, TableIndexColumn, ResultRow } from "$lib/types";
  import type { ExportFormat } from "$lib/export";
  import * as exportMod from "$lib/export";
  import { saveToFile } from "$lib/tauriCommands";
  import { loadColumnSelection, saveColumnSelection } from "$lib/tableColumns";

  let bindings = $derived(S.executionBindings);
  let results = $derived(S.executionResults);
  let progress = $derived(S.walkProgress);
  let tableResult = $derived(S.tableResult);

  let exportMenuOpen = $state(false);

  let filterText = $state("");
  let sortColumn: "oid" | "value" | "type" = $state("oid");
  let sortAsc = $state(true);

  let showResolvedNames = $state(true);
  let wrapValue = $state(false);

  const COL_MIN_OID = 100;
  const COL_MAX_OID = 500;
  const COL_MIN_TYPE = 70;
  const COL_MAX_TYPE = 200;

  let colOid = $state(180);
  let colType = $state(90);

  function saveColWidths() {
    try {
      localStorage.setItem("scout-results-col-widths", JSON.stringify({ colOid, colType }));
    } catch {}
  }

  function loadColWidths() {
    try {
      const saved = JSON.parse(localStorage.getItem("scout-results-col-widths") || "null");
      if (saved?.colOid) colOid = Math.max(COL_MIN_OID, Math.min(COL_MAX_OID, saved.colOid));
      if (saved?.colType) colType = Math.max(COL_MIN_TYPE, Math.min(COL_MAX_TYPE, saved.colType));
    } catch {}
  }
  loadColWidths();

  $effect(() => {
    saveColWidths();
  });

  let draggingDivider = $state(false);
  let dragStartX = 0;
  let dragStartColOid = 0;
  let dragStartColType = 0;

  function onDivider1MouseDown(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    draggingDivider = true;
    dragStartX = e.clientX;
    dragStartColOid = colOid;
    document.addEventListener("mousemove", onDividerMouseMove);
    document.addEventListener("mouseup", onDividerMouseUp);
  }

  function onDivider2MouseDown(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    draggingDivider = true;
    dragStartX = e.clientX;
    dragStartColType = colType;
    document.addEventListener("mousemove", onDividerMouseMove2);
    document.addEventListener("mouseup", onDividerMouseUp);
  }

  function onDividerMouseMove(e: MouseEvent) {
    if (!draggingDivider) return;
    const newOid = Math.max(COL_MIN_OID, Math.min(COL_MAX_OID, dragStartColOid + (e.clientX - dragStartX)));
    colOid = newOid;
  }

  function onDividerMouseMove2(e: MouseEvent) {
    if (!draggingDivider) return;
    const newType = Math.max(COL_MIN_TYPE, Math.min(COL_MAX_TYPE, dragStartColType + (e.clientX - dragStartX)));
    colType = newType;
  }

  function onDividerMouseUp() {
    draggingDivider = false;
    document.removeEventListener("mousemove", onDividerMouseMove);
    document.removeEventListener("mousemove", onDividerMouseMove2);
    document.removeEventListener("mouseup", onDividerMouseUp);
  }

  const divider1Left = $derived(colOid + 2);
  const divider2Left = $derived(`calc(100% - ${colType}px - 2px)`);

  function resolveOidName(oid: string): { displayName: string; fullPath: string } {
    const parts = oid.split(".");
    for (let i = parts.length; i > 1; i--) {
      const prefix = parts.slice(0, i).join(".");
      if (S.oidNameMap.has(prefix)) {
        const suffix = parts.slice(i).join(".");
        const baseName = S.oidNameMap.get(prefix)!;
        return {
          displayName: suffix ? `${baseName}.${suffix}` : baseName,
          fullPath: oid,
        };
      }
    }
    return { displayName: oid, fullPath: oid };
  }


  function typeLabel(v: SnmpValue): string {
    if (v === "Null") return "NULL";
    if (typeof v !== "object" || v === null) return "UNKNOWN";
    if ("Integer" in v) return "INTEGER";
    if ("Unsigned" in v) return "UNSIGNED32";
    if ("Counter32" in v) return "COUNTER32";
    if ("Counter64" in v) return "COUNTER64";
    if ("OctetString" in v) return "OCTET STRING";
    if ("ObjectIdentifier" in v) return "OBJECT IDENTIFIER";
    if ("IpAddress" in v) return "IPADDRESS";
    if ("TimeTicks" in v) return "TIMETICKS";
    if ("TruthValue" in v) return "TRUTHVALUE";
    if ("Raw" in v) return "RAW";
    return "UNKNOWN";
  }

  let rows = $derived<ResultRow[]>(bindings.map((b): ResultRow => {
    const resolved = resolveOidName(b.oid);
    return {
      oid: b.oid,
      displayName: resolved.displayName,
      fullPath: resolved.fullPath,
      type: typeLabel(b.value),
      value: exportMod.valueDisplay(b.value),
      warning: !!b.warning,
    };
  }));

  // The needle must be lowercased too — the haystacks are, so a mixed-case
  // query like "sysDescr" would otherwise never match.
  let filterLower = $derived(filterText.toLowerCase());

  let filteredRows = $derived(filterText
    ? rows.filter((r: ResultRow) =>
        r.oid.toLowerCase().includes(filterLower) ||
        r.displayName.toLowerCase().includes(filterLower) ||
        r.fullPath.toLowerCase().includes(filterLower) ||
        r.type.toLowerCase().includes(filterLower) ||
        r.value.toLowerCase().includes(filterLower),
      )
    : rows);

  let sortedRows = $derived([...filteredRows].sort((a, b) => {
    const aVal = a[sortColumn];
    const bVal = b[sortColumn];
    const cmp = aVal < bVal ? -1 : aVal > bVal ? 1 : 0;
    return sortAsc ? cmp : -cmp;
  }));

  function toggleSort(col: "oid" | "value" | "type") {
    if (sortColumn === col) {
      sortAsc = !sortAsc;
    } else {
      sortColumn = col;
      sortAsc = true;
    }
  }

  // ── Inspector integration ──────────────────────────────────────────────────
  // Clicking a Variable Binding in the flat list, or a cell in the grid,
  // points the Inspector at that OID and hands it the live value.

  function selectResultRow(row: ResultRow) {
    S.inspectorOid = row.fullPath;
    S.inspectorValue = { text: row.value, typeLabel: row.type };
  }

  function selectGridCell(colOid: string, cell: TableCell) {
    S.inspectorOid = colOid;
    S.inspectorValue = cell.value
      ? { text: exportMod.valueDisplay(cell.value.value), typeLabel: typeLabel(cell.value.value) }
      : null;
  }

  let hasWarnings = $derived(results?.warnings && results.warnings.length > 0);
  let isPartial = $derived(results?.partial || false);

  // A table result is always rendered as the grid — there is no alternative
  // presentation for it.
  let isGridView = $derived(!!tableResult);
  $effect(() => {
    if (tableResult) {
      columnsOpen = false;
      gridSortKey = null;
      gridSortDir = 1;
      gridColWidths = {};
      selectedColumns = loadColumnSelection(tableResult.table_oid, tableResult.columns);
    }
  });
  let tableInfo = $derived(S.tableInfo);
  let gridColumns = $derived(tableResult?.columns || []);
  let gridRows = $derived(tableResult?.rows || []);
  let gridMissingCells = $derived(tableResult?.missing_cells || 0);

  function columnName(oid: string): string {
    const baseName = S.oidNameMap.get(oid) || oid.split(".").pop() || oid;
    return baseName;
  }

  // ── Per-component index columns ────────────────────────────────────────────
  // Rendered when the table has INDEX metadata and the engine decoded the row
  // suffixes. Otherwise a single raw "Instance" column is shown (fallback).
  let gridIndexCols: TableIndexColumn[] = $derived(tableInfo?.indexColumns ?? []);
  let hasDecodedIndexes = $derived(
    gridIndexCols.length > 0 && gridRows.some((r) => r.index_values.length > 0),
  );

  // ── Column selection (immediate display filter) ────────────────────────────
  // The run always fetches every column; toggling a checkbox shows or hides it
  // in the current grid right away. The selection persists per table and is
  // restored as the display state of the next run.
  let columnsOpen = $state(false);
  let selectedColumns = $state<string[]>([]);

  /** The full column set for the selection panel: metadata columns when
   *  available, else whatever the last run fetched. */
  let allSelectableColumns = $derived(
    tableInfo && tableInfo.columnOids.length > 0 ? tableInfo.columnOids : gridColumns,
  );

  // What actually renders: fetched columns ∩ selected columns.
  let visibleGridColumns = $derived(gridColumns.filter((c) => selectedColumns.includes(c)));

  function isIndexColumn(oid: string): boolean {
    return gridIndexCols.some((c) => c.oid === oid);
  }

  function toggleColumn(col: string) {
    const set = new Set(selectedColumns);
    if (set.has(col)) {
      set.delete(col);
    } else {
      set.add(col);
    }
    selectedColumns = [...set];
    if (tableResult) saveColumnSelection(tableResult.table_oid, selectedColumns);
  }

  // Master checkbox: checked = select all, unchecked = select none. With a
  // partial selection it renders indeterminate; clicking then selects all.
  let selectAllInput: HTMLInputElement | null = $state(null);
  function setAllColumns() {
    const total = allSelectableColumns.length;
    selectedColumns =
      selectedColumns.length === total && total > 0 ? [] : [...allSelectableColumns];
    if (tableResult) saveColumnSelection(tableResult.table_oid, selectedColumns);
  }
  $effect(() => {
    const total = allSelectableColumns.length;
    const sel = selectedColumns.length;
    if (selectAllInput) selectAllInput.indeterminate = sel > 0 && sel < total;
  });

  // ── Column resizing (drag a header's right edge; session-scoped) ───────────
  const MIN_COL_W = 48;
  let gridColWidths = $state<Record<string, number>>({});

  function colWidth(key: string, fallback: number): number {
    return gridColWidths[key] ?? fallback;
  }

  /** Fixed-width style for columns that always have a width (identity group). */
  function widthCss(key: string, fallback: number): string {
    const w = colWidth(key, fallback);
    return `width: ${w}px; min-width: ${w}px; max-width: ${w}px;`;
  }

  /** Width style for data columns — auto until the user drags them. */
  function overrideCss(key: string): string {
    const w = gridColWidths[key];
    return w ? `width: ${w}px; min-width: ${w}px; max-width: ${w}px;` : "";
  }

  function startColResize(e: MouseEvent, key: string) {
    e.preventDefault();
    e.stopPropagation();
    const th = (e.currentTarget as HTMLElement).closest("th");
    const startX = e.clientX;
    const startW = gridColWidths[key] ?? Math.round(th?.getBoundingClientRect().width ?? 0);
    if (startW <= 0) return;
    function onMove(ev: MouseEvent) {
      gridColWidths[key] = Math.max(MIN_COL_W, startW + ev.clientX - startX);
    }
    function onUp() {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    }
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  }

  // ── Grid sorting (per-column, numeric-aware; default is walk order) ───────
  type GridSortKey = string; // "instance" | `idx:${i}` | column OID
  let gridSortKey: GridSortKey | null = $state(null);
  let gridSortDir: 1 | -1 = $state(1);

  function toggleGridSort(key: GridSortKey) {
    if (gridSortKey !== key) {
      gridSortKey = key;
      gridSortDir = 1;
    } else if (gridSortDir === 1) {
      gridSortDir = -1;
    } else {
      // Third click restores walk order.
      gridSortKey = null;
      gridSortDir = 1;
    }
  }

  function snmpNumeric(v: SnmpValue): number | null {
    if (v === "Null" || typeof v !== "object" || v === null) return null;
    if ("Integer" in v) return v.Integer;
    if ("Unsigned" in v) return v.Unsigned;
    if ("Counter32" in v) return v.Counter32;
    if ("Counter64" in v) return Number(v.Counter64);
    if ("TimeTicks" in v) return v.TimeTicks;
    return null;
  }

  function gridCellParts(row: TableRowData, key: GridSortKey): { text: string; numeric: number | null } {
    if (key === "instance") {
      return { text: row.instance_id, numeric: /^\d+$/.test(row.instance_id) ? Number(row.instance_id) : null };
    }
    if (key.startsWith("idx:")) {
      const t = row.index_values[Number(key.slice(4))] ?? "";
      return { text: t, numeric: /^\d+$/.test(t) ? Number(t) : null };
    }
    const cell = row.cells[key];
    if (!cell?.value) return { text: "", numeric: null };
    return { text: exportMod.valueDisplay(cell.value.value), numeric: snmpNumeric(cell.value.value) };
  }

  // ── Filtering + sorting + chunked rendering ────────────────────────────────
  let filteredGridRows = $derived(filterText
    ? gridRows.filter((r: TableRowData) => {
        const instMatch = r.instance_id.toLowerCase().includes(filterLower);
        if (instMatch) return true;
        for (const v of r.index_values) {
          if (v !== null && v.toLowerCase().includes(filterLower)) return true;
        }
        for (const cell of Object.values(r.cells)) {
          if (cell.value && exportMod.valueDisplay(cell.value.value).toLowerCase().includes(filterLower)) {
            return true;
          }
        }
        return false;
      })
    : gridRows);

  let sortedGridRows = $derived(
    gridSortKey === null
      ? filteredGridRows // walk order (stable)
      : [...filteredGridRows].sort((a, b) => {
          const pa = gridCellParts(a, gridSortKey!);
          const pb = gridCellParts(b, gridSortKey!);
          const cmp =
            pa.numeric !== null && pb.numeric !== null
              ? pa.numeric - pb.numeric
              : pa.text < pb.text
                ? -1
                : pa.text > pb.text
                  ? 1
                  : 0;
          return gridSortDir * cmp;
        }),
  );

  // Row-cap rendering: show CHUNK rows, append more when the sentinel scrolls
  // into view. The footer always reports the true total.
  const GRID_CHUNK = 500;
  let visibleCount = $state(GRID_CHUNK);
  $effect(() => {
    void sortedGridRows.length;
    visibleCount = GRID_CHUNK;
  });
  let visibleGridRows = $derived(sortedGridRows.slice(0, visibleCount));

  // Footer suffix as a plain expression (not an {#if} block): Svelte trims
  // whitespace at block boundaries, which would glue "(N match filter)" onto
  // "rows".
  let gridFilterSuffix = $derived(
    filterText && sortedGridRows.length !== tableResult?.total_rows
      ? ` (${sortedGridRows.length} match filter)`
      : "",
  );

  let sentinelEl: HTMLDivElement | null = $state(null);
  $effect(() => {
    if (!isGridView || !sentinelEl) return;
    if (typeof IntersectionObserver === "undefined") {
      visibleCount = Number.MAX_SAFE_INTEGER; // no chunking without IO support
      return;
    }
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) visibleCount += GRID_CHUNK;
      },
      { threshold: 0 },
    );
    observer.observe(sentinelEl);
    return () => observer.disconnect();
  });

  // Sticky left offsets for the row-identity column group (# + index columns,
  // or # + Instance in raw mode). Offsets follow each column's current width —
  // the default until the user drags a header.
  const HASH_W = 40;
  const IDX_W = 96;
  const INST_W = 120;
  function stickyLeft(i: number): string {
    // i = 1 is the first identity column after "#".
    let left = HASH_W;
    for (let k = 1; k < i; k++) {
      left += hasDecodedIndexes ? colWidth(`idx:${k - 1}`, IDX_W) : INST_W;
    }
    return `${left}px`;
  }

  async function handleExport(format: ExportFormat) {
    exportMenuOpen = false;
    if (isGridView && tableResult) {
      const nameOf = (oid: string) => columnName(oid);
      // Export what is on screen: the fetched result restricted to the
      // currently displayed columns.
      const visible = { ...tableResult, columns: visibleGridColumns };
      const content =
        format === "json"
          ? exportMod.gridToJson(visible, nameOf)
          : exportMod.gridDelimited(
              visible,
              gridIndexCols,
              nameOf,
              format === "csv" ? "," : "\t",
              format === "csv",
            );
      const name = tableInfo?.name || tableResult.table_oid.split(".").pop() || "table";
      await saveToFile(content, `${name}.${format}`);
      return;
    }

    if (bindings.length === 0) return;

    const rowsExport = exportMod.bindingsToRows(bindings, S.oidNameMap);
    let content: string;

    switch (format) {
      case "tsv":
        content = exportMod.formatTSV(rowsExport);
        break;
      case "json":
        content = exportMod.formatJSON(S.targetConfig, S.queryRootOid, rowsExport, results?.warnings);
        break;
      case "csv":
        content = exportMod.formatCSV(rowsExport);
        break;
    }

    const filename = exportMod.defaultFilename(S.targetConfig, S.queryRootOid, format);
    await saveToFile(content, filename);
  }

  function toggleExportMenu(e: MouseEvent) {
    e.stopPropagation();
    exportMenuOpen = !exportMenuOpen;
  }

  function hideExportOnOutsideClick(e: MouseEvent) {
    const target = e.target as HTMLElement;
    if (!target.closest("[data-export-menu]")) {
      exportMenuOpen = false;
    }
  }

  function clearAll() {
    clearResults();
    filterText = "";
    columnsOpen = false;
    gridSortKey = null;
    gridSortDir = 1;
    sortColumn = "oid";
    sortAsc = true;
  }

  $effect(() => {
    return () => onDividerMouseUp();
  });
</script>

<div class="flex flex-col flex-1 overflow-hidden">
  <div data-testid="results-header" class="px-4 py-3 text-sm font-semibold uppercase tracking-wide text-base-content/60 bg-base-100 border-b border-base-300 flex items-center justify-between gap-3" onclick={(e) => { if (e.target === e.currentTarget) hideExportOnOutsideClick(e); }}>
    <span>Results</span>
    <div class="flex items-center gap-2">
      {#if progress}
        <span class="text-xs text-primary font-mono">{progress}</span>
      {/if}
      {#if isPartial}
        <span data-testid="partial-badge" class="badge badge-warning badge-sm gap-1"><TriangleAlert class="w-3 h-3" /> partial results</span>
      {/if}
      {#if bindings.length > 0 || isGridView}
        <div data-export-menu class="dropdown dropdown-end relative">
          <button data-testid="save-btn" class="btn btn-sm" onclick={toggleExportMenu}>Save Results</button>
          {#if exportMenuOpen}
            <ul class="absolute top-full right-0 menu menu-sm bg-base-100 rounded-box w-40 p-2 shadow-lg z-[1000] mt-1">
              <li><a onclick={() => handleExport("tsv")}>Save as TSV</a></li>
              <li><a onclick={() => handleExport("json")}>Save as JSON</a></li>
              <li><a onclick={() => handleExport("csv")}>Save as CSV</a></li>
            </ul>
          {/if}
        </div>
        <button data-testid="clear-btn" aria-label="Clear results" class="btn btn-sm btn-ghost" title="Clear results" onclick={clearAll}><Trash2 class="w-4 h-4" /></button>
        {#if !isGridView}
          <button data-testid="names-toggle" class="btn btn-sm {showResolvedNames ? 'btn-primary' : 'btn-ghost'}" onclick={() => showResolvedNames = !showResolvedNames}>{showResolvedNames ? "MIB Names" : "Raw OIDs"}</button>
          <button data-testid="wrap-toggle" title="Wrap long values" class="btn btn-sm {wrapValue ? 'btn-primary' : 'btn-ghost'}" onclick={() => wrapValue = !wrapValue}>
            <WrapText class="w-4 h-4 inline-block" /> Wrap
          </button>
        {/if}
      {/if}
      <input data-testid="filter-input" aria-label="Filter results" type="text" placeholder="Filter..." class="input input-bordered input-sm w-40 font-mono" bind:value={filterText} />
    </div>
  </div>

  {#if hasWarnings && results?.warnings}
    <div data-testid="warnings-banner" role="alert" class="alert alert-warning px-4 py-2 text-xs max-h-24 overflow-y-auto">
      {#each results.warnings as w}
        <div class="flex gap-1 items-start">
          <TriangleAlert class="w-3.5 h-3.5 shrink-0 mt-0.5" />
          <span class="font-semibold">{w.kind}</span>
          <span>: {w.message}</span>
          {#if w.oid}<span class="font-mono opacity-70">({w.oid})</span>{/if}
        </div>
      {/each}
    </div>
  {/if}

  {#if isGridView && tableResult}
    <div class="px-4 py-1 flex items-center gap-2 border-b border-base-300">
      <button data-testid="columns-btn" class="btn btn-sm btn-ghost" onclick={() => (columnsOpen = !columnsOpen)}>Columns…</button>
    </div>
    {#if columnsOpen}
      <div data-testid="columns-panel" class="px-4 py-2 border-b border-base-300 bg-base-100 max-h-60 overflow-y-auto">
        <p class="text-xs text-base-content/60 mb-1">Display columns — changes apply immediately.</p>
        <label title="Select or clear every column" class="flex items-center gap-2 py-1.5 mb-2 border-b border-base-300 cursor-pointer select-none">
          <input type="checkbox" bind:this={selectAllInput} data-testid="select-all-cols" checked={selectedColumns.length === allSelectableColumns.length && allSelectableColumns.length > 0} onchange={() => setAllColumns()} class="scale-125" />
          <span class="text-sm font-semibold">All columns</span>
        </label>
        {#each allSelectableColumns as col (col)}
          <label class="flex items-center gap-2 text-sm py-0.5 cursor-pointer">
            <input type="checkbox" checked={selectedColumns.includes(col)} onchange={() => toggleColumn(col)} />
            <span>{columnName(col)}</span>
            {#if isIndexColumn(col)}<span class="badge badge-xs badge-outline">index</span>{/if}
          </label>
        {/each}
      </div>
    {/if}
  {/if}

  <div
    data-testid="results-body"
    tabindex="0"
    aria-label="Results"
    class="flex-1 overflow-auto focus:outline-none focus-visible:ring-2 focus-visible:ring-primary"
  >
    {#if isGridView}
      {#if filteredGridRows.length === 0 && gridRows.length === 0}
        <p class="text-base-content/60 text-sm text-center mt-12">No table data returned.</p>
      {:else if filteredGridRows.length === 0}
        <p class="text-base-content/60 text-sm text-center mt-8">No results match filter.</p>
      {:else}
        <!-- results-body is the scroll container so sticky top/left both work. -->
        <table data-testid="grid-table" class="table table-sm w-full min-w-max border-separate border-spacing-0">
          <thead>
            <tr>
              <th class="sticky top-0 left-0 z-30 bg-base-100 px-2 py-1.5 text-xs w-[40px] min-w-[40px] max-w-[40px]">#</th>
              {#if hasDecodedIndexes}
                {#each gridIndexCols as col, i (col.oid)}
                  <th data-grid-col="idx:{i}" title={col.implied ? `${col.name} (implied)` : col.name} class="sticky top-0 z-30 bg-base-200 px-2 py-1.5 text-xs cursor-pointer select-none relative" style="left: {stickyLeft(i + 1)}px; {widthCss(`idx:${i}`, IDX_W)}" onclick={() => toggleGridSort(`idx:${i}`)}>
                    <span class="flex items-center gap-1">
                      <span class="truncate">{col.name}</span>
                      {#if gridSortKey === `idx:${i}`}{#if gridSortDir === 1}<ArrowUp class="w-3 h-3 shrink-0" />{:else}<ArrowDown class="w-3 h-3 shrink-0" />{/if}{:else}<ArrowUpDown class="w-3 h-3 shrink-0 opacity-40" />{/if}
                    </span>
                    <span class="col-resize-handle absolute top-0 right-0 h-full w-[5px] cursor-col-resize z-10 hover:bg-primary/60" onclick={(e) => e.stopPropagation()} onmousedown={(e) => startColResize(e, `idx:${i}`)}></span>
                  </th>
                {/each}
              {:else}
                <th data-grid-col="instance" class="sticky top-0 z-30 bg-base-100 px-2 py-1.5 text-xs cursor-pointer select-none relative" style="left: {stickyLeft(1)}px; {widthCss('instance', INST_W)}" onclick={() => toggleGridSort("instance")}>
                  <span class="flex items-center gap-1">
                    <span class="truncate">Instance</span>
                    {#if gridSortKey === "instance"}{#if gridSortDir === 1}<ArrowUp class="w-3 h-3 shrink-0" />{:else}<ArrowDown class="w-3 h-3 shrink-0" />{/if}{:else}<ArrowUpDown class="w-3 h-3 shrink-0 opacity-40" />{/if}
                  </span>
                  <span class="col-resize-handle absolute top-0 right-0 h-full w-[5px] cursor-col-resize z-10 hover:bg-primary/60" onclick={(e) => e.stopPropagation()} onmousedown={(e) => startColResize(e, "instance")}></span>
                </th>
              {/if}
              {#each visibleGridColumns as colOid (colOid)}
                <th
                  data-grid-col={colOid}
                  class="sticky top-0 z-20 px-2 py-1.5 text-xs cursor-pointer select-none relative"
                  class:bg-base-100={colOid !== S.inspectorOid}
                  class:inspector-col-selected={colOid === S.inspectorOid}
                  style="{overrideCss(colOid)}"
                  onclick={() => toggleGridSort(colOid)}
                >
                  <span class="flex items-center gap-1">
                    <span class="truncate">{columnName(colOid)}</span>
                    {#if gridSortKey === colOid}{#if gridSortDir === 1}<ArrowUp class="w-3 h-3 shrink-0" />{:else}<ArrowDown class="w-3 h-3 shrink-0" />{/if}{:else}<ArrowUpDown class="w-3 h-3 shrink-0 opacity-40" />{/if}
                  </span>
                  <span class="col-resize-handle absolute top-0 right-0 h-full w-[5px] cursor-col-resize z-10 hover:bg-primary/60" onclick={(e) => e.stopPropagation()} onmousedown={(e) => startColResize(e, colOid)}></span>
                </th>
              {/each}
            </tr>
          </thead>
          <tbody>
            {#each visibleGridRows as row, i (row.instance_id)}
              <tr>
                <td class="sticky left-0 z-10 bg-base-200 px-2 text-xs text-base-content/60 w-[40px] min-w-[40px] max-w-[40px]">{i + 1}</td>
                {#if hasDecodedIndexes}
                  {#each row.index_values as val, i (i)}
                    <td class="sticky z-10 bg-base-200 px-2 font-mono text-[13px] truncate" style="left: {stickyLeft(i + 1)}px; {widthCss(`idx:${i}`, IDX_W)}" title={val === null ? "(implied)" : row.instance_id}>
                      {#if val === null}<span class="text-base-content/40 italic">—</span>{:else}{val}{/if}
                    </td>
                  {/each}
                {:else}
                  <td class="sticky z-10 bg-base-200 px-2 font-semibold font-mono text-[13px] truncate" style="left: {HASH_W}px; {widthCss('instance', INST_W)}" title={row.instance_id}>{row.instance_id}</td>
                {/if}
                {#each visibleGridColumns as colOid (colOid)}
                  {@const cell = row.cells[colOid]}
                  <td
                    class="px-2 font-mono text-[13px] cursor-pointer hover:bg-base-200/70 {cell.missing ? 'text-accent' : ''} {gridColWidths[colOid] ? 'overflow-hidden text-ellipsis whitespace-nowrap' : ''}"
                    class:inspector-col-selected={colOid === S.inspectorOid}
                    style="{overrideCss(colOid)}"
                    title="Click to inspect {columnName(colOid)}"
                    onclick={() => selectGridCell(colOid, cell)}
                  >
                    {#if cell.missing}
                      <span class="text-base-content/60 italic flex items-center gap-1">— missing <TriangleAlert class="w-3 h-3 shrink-0" /></span>
                    {:else if cell.value}
                      <span>{exportMod.valueDisplay(cell.value.value)}</span>
                    {:else}
                      <span class="text-base-content/60">\u2014</span>
                    {/if}
                  </td>
                {/each}
              </tr>
            {/each}
          </tbody>
        </table>
        {#if visibleGridRows.length < sortedGridRows.length}
          <div bind:this={sentinelEl} data-testid="grid-sentinel" class="h-8"></div>
        {/if}
      {/if}
    {:else if sortedRows.length === 0 && bindings.length === 0}
      <p data-testid="results-placeholder" class="text-base-content/60 text-sm text-center mt-12">Select a MIB node and click Go to query the Target.</p>
    {:else if sortedRows.length === 0}
      <p class="text-base-content/60 text-sm text-center mt-8">No results match filter.</p>
    {:else}
      <!-- Single scroll container: results-body (overflow-auto) owns both axes.
           A nested overflow-x-auto here makes WebKitGTK register a phantom
           scrollable region over the area below (e.g. the system log pane),
           stealing mouse-wheel input from it once the row set is large. -->
      <div class="relative" style="cursor: {draggingDivider ? 'col-resize' : ''}">

        <div class="resize-divider absolute top-0 bottom-0 w-[5px] z-20 hover:bg-primary/50 transition-colors" style="left: {divider1Left}px;" onmousedown={onDivider1MouseDown}></div>
        <div class="resize-divider absolute top-0 bottom-0 w-[5px] z-20 hover:bg-primary/50 transition-colors" style="left: {divider2Left};" onmousedown={onDivider2MouseDown}></div>

        <!-- /60 (not /30): 3:1 AA for the UI boundary in the light theme. -->
        <div class="flex bg-base-200 border-b-2 border-base-content/60 sticky top-0 z-10 text-xs font-semibold uppercase tracking-wider" style="min-width: max-content;">
          <div data-testid="sort-oid" class="cursor-pointer px-2 py-1.5 truncate select-none flex items-center gap-1" style="width: {colOid}px; min-width: {COL_MIN_OID}px; max-width: {COL_MAX_OID}px;" onclick={() => toggleSort("oid")}>
            <span class="truncate">OID</span>
            {#if sortColumn === "oid"}{#if sortAsc}<ArrowUp class="w-3 h-3 shrink-0" />{:else}<ArrowDown class="w-3 h-3 shrink-0" />{/if}{:else}<ArrowUpDown class="w-3 h-3 shrink-0" />{/if}
          </div>
          <div data-testid="sort-value" class="flex-1 min-w-[120px] cursor-pointer px-2 py-1.5 truncate select-none flex items-center gap-1" onclick={() => toggleSort("value")}>
            <span class="truncate">Value</span>
            {#if sortColumn === "value"}{#if sortAsc}<ArrowUp class="w-3 h-3 shrink-0" />{:else}<ArrowDown class="w-3 h-3 shrink-0" />{/if}{:else}<ArrowUpDown class="w-3 h-3 shrink-0" />{/if}
          </div>
          <div data-testid="sort-type" class="cursor-pointer px-2 py-1.5 truncate select-none flex items-center gap-1" style="width: {colType}px; min-width: {COL_MIN_TYPE}px; max-width: {COL_MAX_TYPE}px;" onclick={() => toggleSort("type")}>
            <span class="truncate">Type</span>
            {#if sortColumn === "type"}{#if sortAsc}<ArrowUp class="w-3 h-3 shrink-0" />{:else}<ArrowDown class="w-3 h-3 shrink-0" />{/if}{:else}<ArrowUpDown class="w-3 h-3 shrink-0" />{/if}
          </div>
        </div>

        {#each sortedRows as row (row.oid)}
          <div
            data-testid="result-row"
            class="flex border-b border-base-300 cursor-pointer hover:bg-base-200/70 {row.warning ? 'text-accent' : ''}"
            class:inspector-selected={row.fullPath === S.inspectorOid}
            style="min-width: max-content;"
            title="Click to inspect"
            onclick={() => selectResultRow(row)}
          >
            <div class="px-2 py-1 truncate font-mono text-[13px] relative" style="width: {colOid}px; min-width: {COL_MIN_OID}px; max-width: {COL_MAX_OID}px;" title="{row.fullPath}\n{row.oid}">
              {showResolvedNames ? row.displayName : row.oid}
            </div>
            <div class="flex-1 min-w-[120px] px-2 py-1 font-mono text-[13px] {wrapValue ? 'break-all' : 'truncate'}">
              {row.value}
              {#if row.warning} <TriangleAlert class="w-3.5 h-3.5 inline-block text-accent" />{/if}
            </div>
            <div class="px-2 py-1 font-mono text-[13px] text-base-content/60" style="width: {colType}px; min-width: {COL_MIN_TYPE}px; max-width: {COL_MAX_TYPE}px;">{row.type}</div>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  {#if isGridView && tableResult}
    <div data-testid="grid-footer" class="px-4 py-2 text-xs text-base-content/60 border-t border-base-300 bg-base-100 flex justify-between">
      <span>Showing {visibleGridRows.length} of {tableResult.total_rows} rows{gridFilterSuffix}</span>
      {#if gridMissingCells > 0}
        <span class="text-accent">{gridMissingCells} missing cell(s)</span>
      {/if}
    </div>
  {:else if bindings.length > 0}
    <div data-testid="results-footer" class="px-4 py-2 text-xs text-base-content/60 border-t border-base-300 bg-base-100 flex justify-between">
      <span>{sortedRows.length} of {bindings.length} bindings</span>
      {#if results?.retries && results.retries > 0}
        <span>{results.retries} retries</span>
      {/if}
    </div>
  {/if}
</div>

<style>
  /* Row/column highlighted while the inspector reports on it. */
  .inspector-selected {
    background-color: oklch(var(--p) / 0.12);
    box-shadow: inset 2px 0 0 oklch(var(--p));
  }
  .inspector-col-selected {
    background-color: oklch(var(--p) / 0.15);
  }
</style>
