<script lang="ts">
  import { S } from "$lib/stores.svelte";
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
  let sortColumn: "oid" | "name" | "type" | "value" = $state("oid");
  let sortAsc = $state(true);

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

  let nameMap = $derived(buildNameMap(S.treeData));

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

  let rows = $derived(bindings.map(b => ({
    oid: b.oid,
    name: nameMap.get(b.oid) || "",
    type: typeLabel(b.value),
    value: valueDisplay(b.value),
    warning: !!b.warning,
  })));

  let filteredRows = $derived(filterText
    ? rows.filter((r: typeof rows[number]) =>
        r.oid.toLowerCase().includes(filterText) ||
        r.name.toLowerCase().includes(filterText) ||
        r.type.toLowerCase().includes(filterText) ||
        r.value.toLowerCase().includes(filterText),
      )
    : rows);

  let sortedRows = $derived([...filteredRows].sort((a, b) => {
    const aVal = a[sortColumn];
    const bVal = b[sortColumn];
    const cmp = aVal < bVal ? -1 : aVal > bVal ? 1 : 0;
    return sortAsc ? cmp : -cmp;
  }));

  function toggleSort(col: "oid" | "name" | "type" | "value") {
    if (sortColumn === col) {
      sortAsc = !sortAsc;
    } else {
      sortColumn = col;
      sortAsc = true;
    }
  }

  function sortIcon(col: string): string {
    if (sortColumn !== col) return "\u2195";
    return sortAsc ? "\u2191" : "\u2193";
  }

  let hasWarnings = $derived(results?.warnings && results.warnings.length > 0);
  let isPartial = $derived(results?.partial || false);

  let isGridView = $derived(!!tableResult);
  let gridColumns = $derived(tableResult?.columns || []);
  let gridRows = $derived(tableResult?.rows || []);
  let gridMissingCells = $derived(tableResult?.missing_cells || 0);
  let gridWarnings = $derived(tableResult?.warnings && tableResult.warnings.length > 0);

  function columnName(oid: string): string {
    const baseName = nameMap.get(oid) || oid.split(".").pop() || oid;
    return baseName;
  }

  let filteredGridRows = $derived(filterText
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

    const rowsExport = exportMod.bindingsToRows(bindings, nameMap);
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
</script>

<div class="flex flex-col flex-1 overflow-hidden">
  <div class="px-4 py-3 text-sm font-semibold uppercase tracking-wide text-base-content/60 bg-base-100 border-b border-base-300 flex items-center justify-between gap-3" onclick={(e) => { if (e.target === e.currentTarget) hideExportOnOutsideClick(e); }}>
    <span>Results</span>
    <div class="flex items-center gap-3">
      {#if progress}
        <span class="text-xs text-primary font-mono">{progress}</span>
      {/if}
      {#if isPartial}
        <span class="text-xs text-accent">\u26a0 partial results</span>
      {/if}
      {#if bindings.length > 0 || isGridView}
        <div data-export-menu class="dropdown dropdown-end relative">
          <button
            class="btn btn-sm"
            onclick={toggleExportMenu}
          >
            Save Results
          </button>
          {#if exportMenuOpen}
            <ul class="absolute top-full right-0 menu menu-sm bg-base-100 rounded-box w-40 p-2 shadow-lg z-[1000] mt-1">
              <li><a onclick={() => handleExport("tsv")}>Save as TSV</a></li>
              <li><a onclick={() => handleExport("json")}>Save as JSON</a></li>
              <li><a onclick={() => handleExport("csv")}>Save as CSV</a></li>
            </ul>
          {/if}
        </div>
      {/if}
      <input
        type="text"
        placeholder="Filter..."
        class="input input-bordered input-sm w-40 font-mono"
        bind:value={filterText}
      />
    </div>
  </div>

  {#if hasWarnings && results?.warnings}
    <div role="alert" class="alert alert-warning px-4 py-2 text-xs max-h-24 overflow-y-auto">
      {#each results.warnings as w}
        <div class="flex gap-1">
          <span>\u26a0</span>
          <span class="font-semibold">{w.kind}</span>
          <span>: {w.message}</span>
          {#if w.oid}<span class="font-mono opacity-70">({w.oid})</span>{/if}
        </div>
      {/each}
    </div>
  {/if}

  {#if isGridView && bindings.length > 0}
    <div class="px-4 py-1 flex items-center gap-2 border-b border-base-300">
      <label class="cursor-pointer flex items-center gap-2 text-xs">
        <input type="checkbox" class="toggle toggle-sm toggle-primary" bind:value={gridView} />
        Grid view
      </label>
    </div>
  {/if}

  <div class="flex-1 overflow-auto">
    {#if isGridView && gridView}
      {#if filteredGridRows.length === 0 && gridRows.length === 0}
        <p class="text-base-content/60 text-sm text-center mt-12">No table data returned.</p>
      {:else if filteredGridRows.length === 0}
        <p class="text-base-content/60 text-sm text-center mt-8">No results match filter.</p>
      {:else}
        <div class="overflow-x-auto">
          <table class="table table-zebra table-sm">
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
                    {#if row.cells[colOid]}
                      {@const cell = row.cells[colOid]}
                      <td class="{cell.missing ? 'text-accent' : ''}">
                        {#if cell.missing}
                          <span class="text-base-content/60 italic">\u2014 missing \u26a0</span>
                        {:else if cell.value}
                          <span>{valueDisplay(cell.value.value)}</span>
                        {:else}
                          <span class="text-base-content/60">\u2014</span>
                        {/if}
                      </td>
                    {:else}
                      <td class="text-accent italic">\u2014 missing \u26a0</td>
                    {/if}
                  {/each}
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    {:else if sortedRows.length === 0 && bindings.length === 0}
      <p class="text-base-content/60 text-sm text-center mt-12">Select a MIB node and click Go to query the Target.</p>
    {:else if sortedRows.length === 0}
      <p class="text-base-content/60 text-sm text-center mt-8">No results match filter.</p>
    {:else}
      <div class="overflow-x-auto">
        <table class="table table-sm">
          <thead>
            <tr>
              <th class="cursor-pointer" onclick={() => toggleSort("oid")}>\u2116 {sortIcon("oid")}</th>
              <th class="cursor-pointer max-w-[200px]" onclick={() => toggleSort("oid")}>OID {sortIcon("oid")}</th>
              <th class="cursor-pointer" onclick={() => toggleSort("name")}>Name {sortIcon("name")}</th>
              <th class="cursor-pointer w-28" onclick={() => toggleSort("type")}>Type {sortIcon("type")}</th>
              <th class="cursor-pointer flex-1" onclick={() => toggleSort("value")}>Value {sortIcon("value")}</th>
            </tr>
          </thead>
          <tbody>
            {#each sortedRows as row, i (row.oid + i)}
              <tr class="{row.warning ? 'text-accent' : ''}">
                <td class="text-base-content/60">{i + 1}</td>
                <td class="break-all max-w-[250px]">{row.oid}</td>
                <td class="text-primary whitespace-nowrap">{row.name || "\u2014"}</td>
                <td class="text-base-content/60 w-28">{row.type}</td>
                <td class="break-all flex-1">
                  {row.value}
                  {#if row.warning} <span class="text-accent">\u26a0</span>{/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>

  {#if isGridView && tableResult}
    <div class="px-4 py-2 text-xs text-base-content/60 border-t border-base-300 bg-base-100 flex justify-between">
      <span>{filteredGridRows.length} of {tableResult.total_rows} rows</span>
      {#if gridMissingCells > 0}
        <span class="text-accent">{gridMissingCells} missing cell(s)</span>
      {/if}
    </div>
  {:else if bindings.length > 0}
    <div class="px-4 py-2 text-xs text-base-content/60 border-t border-base-300 bg-base-100 flex justify-between">
      <span>{sortedRows.length} of {bindings.length} bindings</span>
      {#if results?.retries && results.retries > 0}
        <span>{results.retries} retries</span>
      {/if}
    </div>
  {/if}
</div>
