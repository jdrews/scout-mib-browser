<script lang="ts">
  import { mibSearch } from "$lib/tauriCommands";
  import { selectedNode, autocompleteResults as acStore, highlightedIndex as hiStore, statusText, treeData } from "$lib/stores";
  import type { MibSearchResult, TreeNode } from "$lib/types";

  let inputValue = "";
  let searchTimer: ReturnType<typeof setTimeout> | null = null;
  $: results = $acStore;
  $: highlighted = $hiStore;

  function onInput(e: Event) {
    const target = e.target as HTMLInputElement;
    const val = target.value.trim();
    inputValue = target.value;

    if (val.length < 1) {
      $acStore = [];
      return;
    }
    $hiStore = -1;

    if (searchTimer) clearTimeout(searchTimer);
    searchTimer = setTimeout(() => performSearch(val), 150);
  }

  async function performSearch(query: string) {
    try {
      const res = await mibSearch(query);
      $acStore = res;
    } catch (err) {
      console.error("Search failed:", err);
    }
  }

  function onKeyDown(e: KeyboardEvent) {
    if (!results.length && e.key !== "Enter") return;

    if (e.key === "ArrowDown") {
      e.preventDefault();
      $hiStore = Math.min($hiStore + 1, results.length - 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      $hiStore = Math.max($hiStore - 1, 0);
    } else if (e.key === "Enter" && $hiStore >= 0) {
      e.preventDefault();
      selectItem(results[$hiStore]);
    } else if (e.key === "Escape") {
      hideAutocomplete();
    } else if (e.key === "Enter") {
      e.preventDefault();
      handleGo();
    }
  }

  function selectItem(item: MibSearchResult) {
    inputValue = `${item.oid}  ${item.name}`;
    hideAutocomplete();
    trySelectInTree(item.oid);
  }

  function trySelectInTree(oid: string) {
    const data = $treeData;
    const found = findNode(data, oid);
    if (found) {
      $selectedNode = found;
    }
  }

  function findNode(nodes: TreeNode[], oid: string): TreeNode | null {
    for (const n of nodes) {
      if (n.oid === oid) return n;
      if (n.children) {
        const found = findNode(n.children, oid);
        if (found) return found;
      }
    }
    return null;
  }

  function hideAutocomplete() {
    $acStore = [];
    $hiStore = -1;
  }

  async function handleGo() {
    const val = inputValue.trim();
    if (!val) return;

    let oid = val.split(/\s{2,}/)[0].trim();
    if (!oid) oid = val.trim();

    $statusText = `Executing operation for ${oid}...`;

    const found = findNode($treeData, oid);
    if (found) {
      $selectedNode = found;
      $statusText = "Navigated to selected node";
      return;
    }

    $statusText = "Ready";
  }

  function hideOnOutsideClick(e: MouseEvent) {
    const target = e.target as HTMLElement;
    if (!target.closest("[data-address-bar]")) {
      hideAutocomplete();
    }
  }
</script>

<div data-address-bar class="px-3 py-2 bg-base-00 border-b border-base-01 relative" on:click|self={hideOnOutsideClick}>
  <div class="flex gap-2">
    <input
      type="text"
      placeholder="Enter OID or MIB name (e.g., 1.3.6.1.2.1.1.1 or sysDescr)"
      autocomplete="off"
      class="flex-1 bg-surface-0 border border-base-01 text-text px-3 py-1.5 text-[13px] font-mono rounded outline-none focus:border-blue"
      bind:value={inputValue}
      on:input={onInput}
      on:keydown={onKeyDown}
    />
    <button class="bg-blue text-base-00 border-none px-4 py-1.5 text-[13px] font-semibold rounded cursor-pointer hover:bg-sapphire" on:click={handleGo}>
      Go
    </button>
  </div>

  {#if results.length > 0}
    <div class="absolute top-full left-2 right-2 bg-base-00 border border-base-01 rounded-lg max-h-[240px] overflow-y-auto z-[500] shadow-lg">
      {#each results as item, i (item.oid)}
        <div
          class="px-3 py-1.5 text-[13px] cursor-pointer flex justify-between items-center hover:bg-base-01"
          class:bg-base-01={i === $hiStore}
          on:click={() => selectItem(item)}
        >
          <span class="text-text">{item.name}</span>
          <span class="text-[11px] text-overlay font-mono">{item.oid}</span>
        </div>
      {/each}
    </div>
  {/if}
</div>


