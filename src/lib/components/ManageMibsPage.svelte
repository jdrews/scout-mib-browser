<script lang="ts">
  import { ArrowDown, ArrowUp, ArrowUpDown } from "lucide-svelte";
  import { S } from "$lib/stores.svelte";
  import { mibLoadedList } from "$lib/tauriCommands";
  import { addMibDirectory, addMibFiles, unloadMibs } from "$lib/mibManager";
  import type { LoadedMib } from "$lib/types";

  type SortCol = "name" | "path" | "nodes";

  let mibs: LoadedMib[] = $state([]);
  let dataLoaded = $state(false);
  let filterText = $state("");
  // null = default (backend) order; a third click on a sorted column clears.
  let sortColumn: SortCol | null = $state(null);
  let sortAsc = $state(true);
  // An array (not a Set): Svelte 5's state proxy does not track Set.add()/
  // delete() calls, so selection must be mutated by replacement.
  let selected = $state<string[]>([]);
  let addMenuOpen = $state(false);
  let selectAllInput: HTMLInputElement | null = $state(null);

  async function loadMibs() {
    if (dataLoaded) return;
    try {
      mibs = await mibLoadedList();
    } catch (err) {
      S.statusText = `Error: ${err}`;
      console.error("Failed to load MIB list:", err);
    }
    // Mark complete after the fetch so "Loading..." covers the whole window.
    dataLoaded = true;
  }

  $effect(() => {
    if (S.manageMibsOpen && !dataLoaded) {
      loadMibs();
    }
  });

  function back() {
    S.manageMibsOpen = false;
    // Reset transient state so the next open starts fresh.
    dataLoaded = false;
    mibs = [];
    selected = [];
    filterText = "";
    sortColumn = null;
    sortAsc = true;
    addMenuOpen = false;
  }

  async function refetch() {
    try {
      mibs = await mibLoadedList();
    } catch (err) {
      S.statusText = `Error: ${err}`;
      console.error("Failed to refresh MIB list:", err);
    }
  }

  // ── Filtering (display-only; selection operates on the visible set) ──────
  let filterLower = $derived(filterText.toLowerCase());
  let visibleMibs = $derived(
    filterText
      ? mibs.filter(
          (m) =>
            m.mibName.toLowerCase().includes(filterLower) ||
            m.filePath.toLowerCase().includes(filterLower) ||
            (m.isFallback && "fallback".includes(filterLower)),
        )
      : mibs,
  );

  // ── Sorting (Results Pane pattern, with a clearing third click) ─────────
  let sortedMibs = $derived.by(() => {
    if (sortColumn === null) return visibleMibs;
    const col = sortColumn;
    const asc = sortAsc;
    return [...visibleMibs].sort((a, b) => {
      let cmp: number;
      if (col === "nodes") {
        cmp = a.nodeCount - b.nodeCount;
      } else if (col === "name") {
        cmp = a.mibName.toLowerCase().localeCompare(b.mibName.toLowerCase());
      } else {
        cmp = a.filePath.toLowerCase().localeCompare(b.filePath.toLowerCase());
      }
      return asc ? cmp : -cmp;
    });
  });

  function toggleSort(col: SortCol) {
    if (sortColumn === col) {
      if (sortAsc) {
        sortAsc = false;
      } else {
        sortColumn = null;
        sortAsc = true;
      }
    } else {
      sortColumn = col;
      sortAsc = true;
    }
  }

  // ── Selection (keyed by file path; select-all = the visible rows) ───────
  let selectedVisibleCount = $derived(
    visibleMibs.filter((m) => selected.includes(m.filePath)).length,
  );

  $effect(() => {
    if (selectAllInput) {
      selectAllInput.indeterminate =
        selectedVisibleCount > 0 && selectedVisibleCount < visibleMibs.length;
    }
  });

  function toggleSelectAll() {
    if (selectedVisibleCount === visibleMibs.length && visibleMibs.length > 0) {
      selected = [];
    } else {
      selected = visibleMibs.map((m) => m.filePath);
    }
  }

  function toggleSelect(path: string) {
    selected = selected.includes(path)
      ? selected.filter((p) => p !== path)
      : [...selected, path];
  }

  // ── Actions ──────────────────────────────────────────────────────────────
  async function unloadSelected() {
    const paths = [...selected];
    if (paths.length === 0) return;
    try {
      await unloadMibs(paths);
      S.statusText = `Unloaded ${paths.length} MIBs`;
      selected = [];
      await refetch();
    } catch (err) {
      S.statusText = `Error: ${err}`;
      console.error("Failed to unload MIBs:", err);
    }
  }

  async function unloadAll() {
    const paths = mibs.map((m) => m.filePath);
    if (paths.length === 0) return;
    try {
      await unloadMibs(paths);
      S.statusText = "Unloaded all MIBs";
      selected = [];
      await refetch();
    } catch (err) {
      S.statusText = `Error: ${err}`;
      console.error("Failed to unload all MIBs:", err);
    }
  }

  async function addDirectory() {
    addMenuOpen = false;
    await addMibDirectory();
    await refetch();
  }

  async function addFiles() {
    addMenuOpen = false;
    await addMibFiles();
    await refetch();
  }

  function onOutsideClick(e: MouseEvent) {
    if (addMenuOpen && !(e.target instanceof Element && e.target.closest("[data-add-menu]"))) {
      addMenuOpen = false;
    }
  }
</script>

{#if S.manageMibsOpen}
  <div data-testid="manage-mibs-page" class="flex flex-col flex-1 overflow-hidden" onclick={onOutsideClick}>
    <div class="px-4 py-3 border-b border-base-300 flex items-center gap-2 flex-wrap">
      <button data-testid="back-btn" aria-label="Back to MIB Browser" class="btn btn-sm btn-ghost" onclick={back}>
        ← Back
      </button>
      <span class="text-sm font-semibold uppercase tracking-wide text-base-content/60">Manage MIBs</span>
      <div class="flex items-center gap-2 ml-auto flex-wrap">
        <div data-add-menu class="dropdown dropdown-end relative">
          <button data-testid="add-menu-btn" class="btn btn-sm" onclick={() => (addMenuOpen = !addMenuOpen)}>Add…</button>
          {#if addMenuOpen}
            <ul class="absolute top-full right-0 menu menu-sm bg-base-100 rounded-box w-40 p-2 shadow-lg z-[1000] mt-1">
              <li><a data-testid="add-dir-item" onclick={addDirectory}>Directory…</a></li>
              <li><a data-testid="add-files-item" onclick={addFiles}>File(s)…</a></li>
            </ul>
          {/if}
        </div>
        <button data-testid="unload-selected-btn" class="btn btn-sm" disabled={selected.length === 0} onclick={unloadSelected}>Unload Selected</button>
        <button data-testid="unload-all-btn" class="btn btn-sm btn-error" onclick={unloadAll}>Unload All</button>
        <input data-testid="mibs-filter-input" aria-label="Filter MIBs" type="text" placeholder="Filter..." class="input input-bordered input-sm w-40 font-mono" bind:value={filterText} />
      </div>
    </div>

    <div class="flex-1 overflow-auto">
      {#if !dataLoaded}
        <p class="text-base-content/60 text-sm text-center mt-12">Loading...</p>
      {:else if mibs.length === 0}
        <p class="text-base-content/60 text-sm text-center mt-12">No MIBs loaded.<br/>Use Add MIB Directory to get started.</p>
      {:else if visibleMibs.length === 0}
        <p data-testid="mibs-no-match" class="text-base-content/60 text-sm text-center mt-12">No MIBs match filter.</p>
      {:else}
        <table data-testid="mibs-table" class="table table-sm w-full">
          <thead>
            <tr>
              <th class="w-[40px] min-w-[40px] max-w-[40px]">
                <input
                  type="checkbox"
                  bind:this={selectAllInput}
                  data-testid="select-all-mibs"
                  aria-label="Select all MIBs"
                  checked={selectedVisibleCount === visibleMibs.length && visibleMibs.length > 0}
                  onchange={toggleSelectAll}
                />
              </th>
              <th
                data-testid="sort-name"
                class="cursor-pointer select-none"
                aria-sort={sortColumn === "name" ? (sortAsc ? "ascending" : "descending") : undefined}
                onclick={() => toggleSort("name")}
              >
                <span class="flex items-center gap-1">
                  Name
                  {#if sortColumn === "name"}{#if sortAsc}<ArrowUp class="w-3 h-3 shrink-0" />{:else}<ArrowDown class="w-3 h-3 shrink-0" />{/if}{:else}<ArrowUpDown class="w-3 h-3 shrink-0 opacity-40" />{/if}
                </span>
              </th>
              <th
                data-testid="sort-path"
                class="cursor-pointer select-none"
                aria-sort={sortColumn === "path" ? (sortAsc ? "ascending" : "descending") : undefined}
                onclick={() => toggleSort("path")}
              >
                <span class="flex items-center gap-1">
                  File Path
                  {#if sortColumn === "path"}{#if sortAsc}<ArrowUp class="w-3 h-3 shrink-0" />{:else}<ArrowDown class="w-3 h-3 shrink-0" />{/if}{:else}<ArrowUpDown class="w-3 h-3 shrink-0 opacity-40" />{/if}
                </span>
              </th>
              <th
                data-testid="sort-nodes"
                class="text-right cursor-pointer select-none"
                aria-sort={sortColumn === "nodes" ? (sortAsc ? "ascending" : "descending") : undefined}
                onclick={() => toggleSort("nodes")}
              >
                <span class="flex items-center gap-1 justify-end">
                  Nodes
                  {#if sortColumn === "nodes"}{#if sortAsc}<ArrowUp class="w-3 h-3 shrink-0" />{:else}<ArrowDown class="w-3 h-3 shrink-0" />{/if}{:else}<ArrowUpDown class="w-3 h-3 shrink-0 opacity-40" />{/if}
                </span>
              </th>
              <th>Status</th>
            </tr>
          </thead>
          <tbody>
            {#each sortedMibs as m (m.filePath)}
              <tr data-testid="mib-row">
                <td>
                  <input type="checkbox" data-testid="mib-select" aria-label={m.mibName} checked={selected.includes(m.filePath)} onchange={() => toggleSelect(m.filePath)} />
                </td>
                <td class="text-sm">{m.mibName}</td>
                <td class="text-xs text-base-content/60 font-mono max-w-[400px] overflow-hidden text-ellipsis whitespace-nowrap" title={m.filePath}>{m.filePath}</td>
                <td class="text-right font-mono" data-testid="mib-node-count">{m.nodeCount.toLocaleString()}</td>
                <td>
                  {#if m.isFallback}
                    <span class="badge badge-warning badge-sm">FALLBACK</span>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>

    <div data-testid="mibs-footer" class="px-4 py-2 border-t border-base-300 text-xs text-base-content/60">
      {selected.length} of {mibs.length} MIBs selected · {S.nodeCount} nodes loaded
    </div>
  </div>
{/if}
