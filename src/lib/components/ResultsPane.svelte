<script lang="ts">
  import { executionBindings, executionResults, walkProgress, treeData, targetConfig, queryRootOid, tableResult as tableResultStore } from "$lib/stores";
  import type { VariableBinding, SnmpValue, ResultSet, TreeNode, TableResult, TableRowData, TableCell } from "$lib/types";
  import type { ExportFormat } from "$lib/export";
  import * as exportMod from "$lib/export";
  import { saveToFile } from "$lib/tauriCommands";

  $: bindings = $executionBindings;
  $: results = $executionResults;
  $: progress = $walkProgress;
  $: tableResult = $tableResultStore;

  let exportMenuOpen = false;

  let filterText = "";
  let sortColumn: "oid" | "name" | "type" | "value" = "oid";
  let sortAsc = true;

  /** Build OID -> name map from tree data. */
  function buildNameMap(nodes: TreeNode[]): Map<string, string> {
    const map = new Map<string, string>();
    function walk(n: TreeNode) {
      if (n.name && n.name !== n.oid) {
        map.set(n.oid, n.name);
      }
      for (const child of n.children || []) {
        walk(child);
      }
    }
    for (const n of nodes) walk(n);
    return map;
  }

  $: nameMap = buildNameMap($treeData);

  /** Display string for a SnmpValue. */
  function valueDisplay(v: SnmpValue): string {
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
    if ("Null" in v) return "NULL";
    if ("Raw" in v) {
      const r = v.Raw;
      return `<raw type=0x${r.type_code.toString(16).padStart(2, "0")} data=0x${r.data.map(b => b.toString(16).padStart(2, "0")).join("")}>`;
    }
    return String(v);
  }

  /** Type label for a SnmpValue. */
  function typeLabel(v: SnmpValue): string {
    if ("Integer" in v) return "INTEGER";
    if ("Unsigned" in v) return "UNSIGNED32";
    if ("Counter32" in v) return "COUNTER32";
    if ("Counter64" in v) return "COUNTER64";
    if ("OctetString" in v) return "OCTET STRING";
    if ("ObjectIdentifier" in v) return "OBJECT IDENTIFIER";
    if ("IpAddress" in v) return "IPADDRESS";
    if ("TimeTicks" in v) return "TIMETICKS";
    if ("TruthValue" in v) return "TRUTHVALUE";
    if ("Null" in v) return "NULL";
    if ("Raw" in v) return "RAW";
    return "UNKNOWN";
  }

  /** Enriched row data for flat view. */
  $: rows = bindings.map(b => ({
    oid: b.oid,
    name: nameMap.get(b.oid) || "",
    type: typeLabel(b.value),
    value: valueDisplay(b.value),
    warning: !!b.warning,
  }));

  $: filteredRows = filterText
    ? rows.filter((r: typeof rows[number]) =>
        r.oid.toLowerCase().includes(filterText) ||
        r.name.toLowerCase().includes(filterText) ||
        r.type.toLowerCase().includes(filterText) ||
        r.value.toLowerCase().includes(filterText),
      )
    : rows;

  $: sortedRows = [...filteredRows].sort((a, b) => {
    const aVal = a[sortColumn];
    const bVal = b[sortColumn];
    const cmp = aVal < bVal ? -1 : aVal > bVal ? 1 : 0;
    return sortAsc ? cmp : -cmp;
  });

  function toggleSort(col: "oid" | "name" | "type" | "value") {
    if (sortColumn === col) {
      sortAsc = !sortAsc;
    } else {
      sortColumn = col;
      sortAsc = true;
    }
  }

  function sortIcon(col: string): string {
    if (sortColumn !== col) return "↕";
    return sortAsc ? "↑" : "↓";
  }

  $: hasWarnings = results?.warnings && results.warnings.length > 0;
  $: isPartial = results?.partial || false;

  // Table grid helpers
  $: isGridView = !!tableResult;
  $: gridColumns = tableResult?.columns || [];
  $: gridRows = tableResult?.rows || [];
  $: gridMissingCells = tableResult?.missing_cells || 0;
  $: gridWarnings = tableResult?.warnings && tableResult.warnings.length > 0;

  /** Get column name from OID using nameMap. */
  function columnName(oid: string): string {
    const baseName = nameMap.get(oid) || oid.split(".").pop() || oid;
    return baseName;
  }

  /** Filter grid rows by text. */
  $: filteredGridRows = filterText
    ? gridRows.filter((r: TableRowData) => {
        const instMatch = r.instance_id.toLowerCase().includes(filterText);
        if (instMatch) return true;
        for (const cell of Object.values(r.cells)) {
          if (cell.value && valueDisplay(cell.value.value).toLowerCase().includes(filterText)) {
            return true;
          }
        }
        return false;
      })
    : gridRows;

  async function handleExport(format: ExportFormat) {
    exportMenuOpen = false;
    if (isGridView && tableResult) {
      // Export table as TSV with column headers.
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

    const rows = exportMod.bindingsToRows(bindings, nameMap);
    let content: string;

    switch (format) {
      case "tsv":
        content = exportMod.formatTSV(rows);
        break;
      case "json":
        content = exportMod.formatJSON($targetConfig, $queryRootOid, rows, results?.warnings);
        break;
      case "csv":
        content = exportMod.formatCSV(rows);
        break;
    }

    const filename = exportMod.defaultFilename($targetConfig, $queryRootOid, format);
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
</script>

<div class="flex flex-col flex-1 overflow-hidden">
  <!-- Header bar -->
  <div class="px-4 py-3 text-sm font-semibold uppercase tracking-wide text-overlay bg-base-00 border-b border-base-01 flex items-center justify-between gap-3" on:click|self={hideExportOnOutsideClick}>
    <span>Results</span>
    <div class="flex items-center gap-3">
      {#if progress}
        <span class="text-xs text-blue font-mono">{progress}</span>
      {/if}
      {#if isPartial}
        <span class="text-xs text-peach">⚠ partial results</span>
      {/if}
      {#if bindings.length > 0}
        <div data-export-menu class="relative">
          <button
            class="bg-surface-0 border border-base-01 text-text px-3 py-2 text-[13px] font-mono rounded outline-none hover:border-blue cursor-pointer"
            on:click={toggleExportMenu}
          >
            Save Results ▾
          </button>
          {#if exportMenuOpen}
            <div class="absolute top-full right-0 bg-base-00 border border-base-01 rounded-lg py-1 min-w-[140px] z-[1000] shadow-lg mt-1">
              <div class="px-3 py-2 text-sm cursor-pointer hover:bg-base-01" on:click={() => handleExport("tsv")}>
                Save as TSV
              </div>
              <div class="px-3 py-2 text-sm cursor-pointer hover:bg-base-01" on:click={() => handleExport("json")}>
                Save as JSON
              </div>
              <div class="px-3 py-2 text-sm cursor-pointer hover:bg-base-01" on:click={() => handleExport("csv")}>
                Save as CSV
              </div>
            </div>
          {/if}
        </div>
      {/if}
      <input
        type="text"
        placeholder="Filter..."
        class="bg-surface-0 border border-base-01 text-text px-3 py-2 text-sm font-mono rounded outline-none focus:border-blue w-40"
        bind:value={filterText}
      />
    </div>
  </div>

  <!-- Warnings section -->
  {#if hasWarnings && results?.warnings}
    <div class="bg-peach/10 border-b border-base-01 px-4 py-2 text-xs max-h-24 overflow-y-auto">
      {#each results.warnings as w}
        <div class="text-peach flex gap-1">
          <span>⚠</span>
          <span class="font-semibold">{w.kind}</span>
          <span>: {w.message}</span>
          {#if w.oid}<span class="font-mono opacity-70">({w.oid})</span>{/if}
        </div>
      {/each}
    </div>
  {/if}

  <!-- Results table -->
  <div class="flex-1 overflow-auto">
    {#if isGridView}
      <!-- Grid view for table results -->
      {#if filteredGridRows.length === 0 && gridRows.length === 0}
        <p class="text-overlay text-sm text-center mt-12">No table data returned.</p>
      {:else if filteredGridRows.length === 0}
        <p class="text-overlay text-sm text-center mt-8">No results match filter.</p>
      {:else}
        <table class="w-full text-[13px] font-mono border-collapse">
          <thead class="sticky top-0 z-10 bg-base-00">
            <tr class="border-b border-base-01 text-overlay uppercase text-xs tracking-wide">
              <th class="text-left px-4 py-2.5 font-semibold whitespace-nowrap">#</th>
              <th class="text-left px-4 py-2.5 font-semibold whitespace-nowrap">Instance</th>
              {#each gridColumns as colOid}
                <th class="text-left px-4 py-2.5 font-semibold whitespace-nowrap">{columnName(colOid)}</th>
              {/each}
            </tr>
          </thead>
          <tbody>
            {#each filteredGridRows as row, i (row.instance_id)}
              <tr class="border-b border-base-01/50 hover:bg-base-01 transition-colors min-h-[32px]" class:bg-base-01={i % 2 === 0 && i > 0}>
                <td class="px-4 py-2.5 text-overlay whitespace-nowrap">{i + 1}</td>
                <td class="px-4 py-2.5 text-text whitespace-nowrap font-semibold">{row.instance_id}</td>
                {#each gridColumns as colOid (colOid)}
                  {#if row.cells[colOid]}
                    {@const cell = row.cells[colOid]}
                    <td class="px-4 py-2.5 break-all max-w-[300px]" class:text-peach={cell.missing}>
                      {#if cell.missing}
                        <span class="text-overlay italic">— missing ⚠</span>
                      {:else if cell.value}
                        <span class="text-text">{valueDisplay(cell.value.value)}</span>
                      {:else}
                        <span class="text-overlay">—</span>
                      {/if}
                    </td>
                  {:else}
                    <td class="px-4 py-2.5 text-peach italic">— missing ⚠</td>
                  {/if}
                {/each}
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    {:else if sortedRows.length === 0 && bindings.length === 0}
      <p class="text-overlay text-sm text-center mt-12">Select a MIB node and click Go to query the Target.</p>
    {:else if sortedRows.length === 0}
      <p class="text-overlay text-sm text-center mt-8">No results match filter.</p>
    {:else}
      <!-- Flat view for regular bindings -->
      <table class="w-full text-[13px] font-mono border-collapse">
        <thead class="sticky top-0 z-10 bg-base-00">
          <tr class="border-b border-base-01 text-overlay uppercase text-xs tracking-wide">
            <th class="text-left px-4 py-2.5 font-semibold cursor-pointer select-none w-8" on:click={() => toggleSort("oid")}>
              #{sortIcon("oid")}
            </th>
            <th class="text-left px-4 py-2.5 font-semibold cursor-pointer select-none break-all max-w-[200px]" on:click={() => toggleSort("oid")}>
              OID {sortIcon("oid")}
            </th>
            <th class="text-left px-4 py-2.5 font-semibold cursor-pointer select-none" on:click={() => toggleSort("name")}>
              Name {sortIcon("name")}
            </th>
            <th class="text-left px-4 py-2.5 font-semibold cursor-pointer select-none w-28" on:click={() => toggleSort("type")}>
              Type {sortIcon("type")}
            </th>
            <th class="text-left px-4 py-2.5 font-semibold cursor-pointer select-none flex-1" on:click={() => toggleSort("value")}>
              Value {sortIcon("value")}
            </th>
          </tr>
        </thead>
        <tbody>
          {#each sortedRows as row, i (row.oid + i)}
            <tr class="border-b border-base-01/50 hover:bg-base-01 transition-colors min-h-[32px]" class:text-peach={row.warning} class:bg-base-01={i % 2 === 0 && i > 0}>
              <td class="px-4 py-2.5 text-overlay whitespace-nowrap">{i + 1}</td>
              <td class="px-4 py-2.5 text-text break-all max-w-[250px]">{row.oid}</td>
              <td class="px-4 py-2.5 text-sky whitespace-nowrap">{row.name || "—"}</td>
              <td class="px-4 py-2.5 text-overlay whitespace-nowrap w-28">{row.type}</td>
              <td class="px-4 py-2.5 text-text break-all flex-1">
                {row.value}
                {#if row.warning} <span class="text-peach">⚠</span>{/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>

  <!-- Footer -->
  {#if isGridView && tableResult}
    <div class="px-4 py-2 text-xs text-overlay border-t border-base-01 bg-base-00 flex justify-between">
      <span>{filteredGridRows.length} of {tableResult.total_rows} rows</span>
      {#if gridMissingCells > 0}
        <span class="text-peach">{gridMissingCells} missing cell(s)</span>
      {/if}
    </div>
  {:else if bindings.length > 0}
    <div class="px-4 py-2 text-xs text-overlay border-t border-base-01 bg-base-00 flex justify-between">
      <span>{sortedRows.length} of {bindings.length} bindings</span>
      {#if results?.retries && results.retries > 0}
        <span>{results.retries} retries</span>
      {/if}
    </div>
  {/if}
</div>

