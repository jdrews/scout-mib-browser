<script lang="ts">
  import TreeNode from "./TreeNode.svelte";
  import { S } from "$lib/stores.svelte";

  let hasTree = $derived(S.treeData.length > 0);
  let showFallback = $derived(S.fallbackMibs.length > 0);
  let width = $derived(S.mibPanelWidth);

  // Keep the roving tabindex target valid: exactly one rendered treeitem must
  // hold tabindex=0. If the focused oid is gone (tree replaced, MIB unloaded)
  // or nothing is focused yet, fall back to the first root.
  $effect(() => {
    const focus = S.treeFocusOid;
    void S.treeData.length;
    if (S.treeData.length === 0) return;
    let found = false;
    for (const el of document.querySelectorAll("[role='treeitem']")) {
      if (el.getAttribute("data-oid") === focus) {
        found = true;
        break;
      }
    }
    if (!found) S.treeFocusOid = S.treeData[0].oid;
  });

  function toggleSystemLog() {
    S.systemLogOpen = !S.systemLogOpen;
  }

  function showMibLoadDetails() {
    S.logLevelFilter = "all";
    S.systemLogOpen = true;
  }
</script>

<aside class="flex flex-col bg-base-200 border-r border-base-300 flex-shrink-0" style="width: {width}px">
  <div data-testid="mib-panel-header" class="px-4 py-3 text-sm font-semibold uppercase tracking-wide text-base-content/60 bg-base-100 border-b border-base-300">
    MIB Browser
  </div>

  <div class="flex-1 overflow-y-auto overflow-x-hidden py-2">
    {#if !hasTree}
      <p class="text-base-content/60 text-sm text-center mt-12">No MIBs loaded.<br/>Use File → Add MIB Directory to get started.</p>
    {:else}
      <ul role="tree" aria-label="MIB tree" class="w-full p-0 list-none">
        {#each S.treeData as node (node.oid)}
          <TreeNode {node} />
        {/each}
      </ul>
    {/if}
  </div>

  {#if showFallback}
    <div data-testid="fallback-banner" role="alert" class="alert alert-warning px-3 py-2 text-xs">
      <span class="cursor-pointer hover:text-base-content underline">{S.fallbackMibs.length} MIB(s) loaded via regex fallback</span>
      <button data-testid="fallback-syslog-btn" class="btn btn-sm btn-ghost ml-auto" onclick={toggleSystemLog}>
        System Log
      </button>
    </div>
  {/if}
</aside>
