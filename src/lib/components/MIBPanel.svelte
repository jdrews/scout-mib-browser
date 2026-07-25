<script lang="ts">
  import TreeNode from "./TreeNode.svelte";
  import { S } from "$lib/stores.svelte";

  let hasTree = $derived(S.treeData.length > 0);
  let showFallback = $derived(S.fallbackMibs.length > 0);
  let width = $derived(S.mibPanelWidth);

  function toggleSystemLog() {
    S.systemLogOpen = !S.systemLogOpen;
  }

  function showMibLoadDetails() {
    S.logLevelFilter = "all";
    S.systemLogOpen = true;
  }
</script>

<aside class="flex flex-col bg-base-200 border-r border-base-300 flex-shrink-0" style="width: {width}px">
  <div class="px-4 py-3 text-sm font-semibold uppercase tracking-wide text-base-content/60 bg-base-100 border-b border-base-300">
    MIB Browser
  </div>

  <div class="flex-1 overflow-y-auto overflow-x-hidden py-2">
    {#if !hasTree}
      <p class="text-base-content/60 text-sm text-center mt-12">No MIBs loaded.<br/>Use File → Add MIB Directory to get started.</p>
    {:else}
      <ul class="menu menu-xs bg-base-200 rounded-box w-full p-0">
        {#each S.treeData as node (node.oid)}
          <TreeNode {node} />
        {/each}
      </ul>
    {/if}
  </div>

  {#if showFallback}
    <div role="alert" class="alert alert-warning px-3 py-2 text-xs">
      <span class="cursor-pointer hover:text-base-content underline">{S.fallbackMibs.length} MIB(s) loaded via regex fallback</span>
      <button class="btn btn-sm btn-ghost ml-auto" onclick={toggleSystemLog}>
        System Log
      </button>
    </div>
  {/if}
</aside>
