<script lang="ts">
  import { executionBindings, executionResults, walkProgress, treeData } from "$lib/stores";
  import type { VariableBinding, SnmpValue, ResultSet, TreeNode } from "$lib/types";

  $: bindings = $executionBindings;
  $: results = $executionResults;
  $: progress = $walkProgress;

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

  /** Enriched row data. */
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
</script>

<div class="flex flex-col flex-1 overflow-hidden">
  <!-- Header bar -->
  <div class="px-3 py-2 text-xs font-semibold uppercase tracking-wide text-overlay bg-base-00 border-b border-base-01 flex items-center justify-between gap-3">
    <span>Results</span>
    <div class="flex items-center gap-3">
      {#if progress}
        <span class="text-[11px] text-blue font-mono">{progress}</span>
      {/if}
      {#if isPartial}
        <span class="text-[11px] text-peach">⚠ partial results</span>
      {/if}
      <input
        type="text"
        placeholder="Filter..."
        class="bg-surface-0 border border-base-01 text-text px-2 py-0.5 text-[11px] font-mono rounded outline-none focus:border-blue w-36"
        bind:value={filterText}
      />
    </div>
  </div>

  <!-- Warnings section -->
  {#if hasWarnings && results?.warnings}
    <div class="bg-peach/10 border-b border-base-01 px-3 py-2 text-[11px] max-h-24 overflow-y-auto">
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
    {#if sortedRows.length === 0 && bindings.length === 0}
      <p class="text-overlay text-[13px] text-center mt-8">Select a MIB node and click Go to query the Target.</p>
    {:else if sortedRows.length === 0}
      <p class="text-overlay text-[13px] text-center mt-4">No results match filter.</p>
    {:else}
      <table class="w-full text-[12px] font-mono border-collapse">
        <thead class="sticky top-0 z-10 bg-base-00">
          <tr class="border-b border-base-01 text-overlay uppercase text-[10px] tracking-wide">
            <th class="text-left px-3 py-1.5 font-semibold cursor-pointer select-none w-8" on:click={() => toggleSort("oid")}>
              #{sortIcon("oid")}
            </th>
            <th class="text-left px-3 py-1.5 font-semibold cursor-pointer select-none" on:click={() => toggleSort("oid")}>
              OID {sortIcon("oid")}
            </th>
            <th class="text-left px-3 py-1.5 font-semibold cursor-pointer select-none" on:click={() => toggleSort("name")}>
              Name {sortIcon("name")}
            </th>
            <th class="text-left px-3 py-1.5 font-semibold cursor-pointer select-none w-28" on:click={() => toggleSort("type")}>
              Type {sortIcon("type")}
            </th>
            <th class="text-left px-3 py-1.5 font-semibold cursor-pointer select-none flex-1" on:click={() => toggleSort("value")}>
              Value {sortIcon("value")}
            </th>
          </tr>
        </thead>
        <tbody>
          {#each sortedRows as row, i (row.oid + i)}
            <tr class="border-b border-base-01/50 hover:bg-base-01 transition-colors" class:text-peach={row.warning}>
              <td class="px-3 py-1 text-overlay whitespace-nowrap">{i + 1}</td>
              <td class="px-3 py-1 text-text whitespace-nowrap">{row.oid}</td>
              <td class="px-3 py-1 text-sky whitespace-nowrap">{row.name || "—"}</td>
              <td class="px-3 py-1 text-overlay whitespace-nowrap w-28">{row.type}</td>
              <td class="px-3 py-1 text-text break-all">
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
  {#if bindings.length > 0}
    <div class="px-3 py-1 text-[10px] text-overlay border-t border-base-01 bg-base-00 flex justify-between">
      <span>{sortedRows.length} of {bindings.length} bindings</span>
      {#if results?.retries && results.retries > 0}
        <span>{results.retries} retries</span>
      {/if}
    </div>
  {/if}
</div>

