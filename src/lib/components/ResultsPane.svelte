<script lang="ts">
  import { ArrowDown, ArrowUp, ArrowUpDown, Trash2, TriangleAlert, WrapText } from "lucide-svelte";
  import { S, clearResults } from "$lib/stores.svelte";
  import type { VariableBinding, SnmpValue, ResultSet, TreeNode, TableResult, TableRowData, TableCell } from "$lib/types";
  import type { ExportFormat } from "$lib/export";
  import * as exportMod from "$lib/export";
  import { saveToFile } from "$lib/tauriCommands";

  let bindings = $derived(S.executionBindings);
  let results = $derived(S.executionResults);
  let progress = $derived(S.walkProgress);
  let tableResult = $derived(S.tableResult);

  let exportMenuOpen = $state(false);
  let gridView = $state(false);

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

  function valueDisplay(v: SnmpValue): string {
    if (v === "Null") return "NULL";
    if (typeof v !== "object" || v === null) return String(v);
    if ("Integer" in v) return String(v.Integer);
    if ("Unsigned" in v) return String(v.Unsigned);
    if ("Counter32" in v) return `${v.Counter32} (counter32)`;
    if ("Counter64" in v) return `${v.Counter64} (counter64)`;
    if ("OctetString" in v) {
      try {
        const s = new TextDecoder().decode(new Uint8Array(v.OctetString));
        return `"${s}"`;
      } catch {
        return `0x${v.OctetString.map(b => b.toString(16).padStart(2, "0")).join("")}`;
      }
    }
    if ("ObjectIdentifier" in v) return v.ObjectIdentifier;
    if ("IpAddress" in v) return v.IpAddress;
    if ("TimeTicks" in v) return `${v.TimeTicks} (timeticks)`;
    if ("TruthValue" in v) return v.TruthValue ? "true" : "false";
    if ("Raw" in v) {
      const r = v.Raw;
      return `<raw type=0x${r.type_code.toString(16).padStart(2, "0")} data=0x${r.data.map(b => b.toString(16).padStart(2, "0")).join("")}>`;
    }
    return String(v);
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

  let rows = $derived(bindings.map(b => {
    const resolved = resolveOidName(b.oid);
    return {
      oid: b.oid,
      displayName: resolved.displayName,
      fullPath: resolved.fullPath,
      type: typeLabel(b.value),
      value: valueDisplay(b.value),
      warning: !!b.warning,
    };
  }));

  // The needle must be lowercased too — the haystacks are, so a mixed-case
  // query like "sysDescr" would otherwise never match.
  let filterLower = $derived(filterText.toLowerCase());

  let filteredRows = $derived(filterText
    ? rows.filter((r: typeof rows[number]) =>
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

  let hasWarnings = $derived(results?.warnings && results.warnings.length > 0);
  let isPartial = $derived(results?.partial || false);

  let isGridView = $derived(!!tableResult);
  // A table result always starts in grid view; the user can still toggle it off.
  $effect(() => { if (tableResult) gridView = true; });
  let gridColumns = $derived(tableResult?.columns || []);
  let gridRows = $derived(tableResult?.rows || []);
  let gridMissingCells = $derived(tableResult?.missing_cells || 0);

  function columnName(oid: string): string {
    const baseName = S.oidNameMap.get(oid) || oid.split(".").pop() || oid;
    return baseName;
  }

  let filteredGridRows = $derived(filterText
    ? gridRows.filter((r: TableRowData) => {
        const instMatch = r.instance_id.toLowerCase().includes(filterLower);
        if (instMatch) return true;
        for (const cell of Object.values(r.cells)) {
          if (cell.value && valueDisplay(cell.value.value).toLowerCase().includes(filterLower)) {
            return true;
          }
        }
        return false;
      })
    : gridRows);

  async function handleExport(format: ExportFormat) {
    exportMenuOpen = false;
    if (isGridView && tableResult) {
      const header = ["Instance", ...gridColumns.map(c => columnName(c))];
      const lines = [header.join("\t")];
      for (const row of gridRows) {
        const cells = [row.instance_id];
        for (const colOid of gridColumns) {
          const cell = row.cells[colOid];
          if (cell?.value) {
            cells.push(valueDisplay(cell.value.value));
          } else {
            cells.push("");
          }
        }
        lines.push(cells.join("\t"));
      }
      const content = lines.join("\n");
      const filename = `${tableResult.table_oid.split(".").pop() || "table"}.tsv`;
      await saveToFile(content, filename);
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
    gridView = false;
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
      <input data-testid="filter-input" type="text" placeholder="Filter..." class="input input-bordered input-sm w-40 font-mono" bind:value={filterText} />
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

  {#if isGridView}
    <div class="px-4 py-1 flex items-center gap-2 border-b border-base-300">
      <label class="cursor-pointer flex items-center gap-2 text-xs">
        <input type="checkbox" class="toggle toggle-sm toggle-primary" bind:checked={gridView} />
        Grid view
      </label>
    </div>
  {/if}

  <div data-testid="results-body" class="flex-1 overflow-auto">
    {#if isGridView && gridView}
      {#if filteredGridRows.length === 0 && gridRows.length === 0}
        <p class="text-base-content/60 text-sm text-center mt-12">No table data returned.</p>
      {:else if filteredGridRows.length === 0}
        <p class="text-base-content/60 text-sm text-center mt-8">No results match filter.</p>
      {:else}
        <div class="overflow-x-auto">
          <table data-testid="grid-table" class="table table-zebra table-sm">
            <thead>
              <tr>
                <th>#</th>
                <th>Instance</th>
                {#each gridColumns as colOid}
                  <th>{columnName(colOid)}</th>
                {/each}
              </tr>
            </thead>
            <tbody>
              {#each filteredGridRows as row, i (row.instance_id)}
                <tr>
                  <td class="text-base-content/60">{i + 1}</td>
                  <td class="font-semibold">{row.instance_id}</td>
                  {#each gridColumns as colOid (colOid)}
                    {@const cell = row.cells[colOid]}
                    <td class="{cell.missing ? 'text-accent' : ''}">
                      {#if cell.missing}
                        <span class="text-base-content/60 italic flex items-center gap-1">— missing <TriangleAlert class="w-3 h-3 shrink-0" /></span>
                      {:else if cell.value}
                        <span>{valueDisplay(cell.value.value)}</span>
                      {:else}
                        <span class="text-base-content/60">\u2014</span>
                      {/if}
                    </td>
                  {/each}
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    {:else if isGridView}
      <p class="text-base-content/60 text-sm text-center mt-12">Grid view is disabled — enable it above to display the table.</p>
    {:else if sortedRows.length === 0 && bindings.length === 0}
      <p data-testid="results-placeholder" class="text-base-content/60 text-sm text-center mt-12">Select a MIB node and click Go to query the Target.</p>
    {:else if sortedRows.length === 0}
      <p class="text-base-content/60 text-sm text-center mt-8">No results match filter.</p>
    {:else}
      <div class="overflow-x-auto relative" style="cursor: {draggingDivider ? 'col-resize' : ''}">

        <div class="resize-divider absolute top-0 bottom-0 w-[5px] z-20 hover:bg-primary/50 transition-colors" style="left: {divider1Left}px;" onmousedown={onDivider1MouseDown}></div>
        <div class="resize-divider absolute top-0 bottom-0 w-[5px] z-20 hover:bg-primary/50 transition-colors" style="left: {divider2Left};" onmousedown={onDivider2MouseDown}></div>

        <div class="flex bg-base-200 border-b-2 border-base-content/30 sticky top-0 z-10 text-xs font-semibold uppercase tracking-wider" style="min-width: max-content;">
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
          <div data-testid="result-row" class="flex border-b border-base-300 {row.warning ? 'text-accent' : ''}" style="min-width: max-content;">
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
      <span>{filteredGridRows.length} of {tableResult.total_rows} rows</span>
      {#if gridMissingCells > 0}
        <span class="text-accent">{gridMissingCells} missing cell(s)}</span>
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
