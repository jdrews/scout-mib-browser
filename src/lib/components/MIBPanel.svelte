<script lang="ts">
  import TreeNode from "./TreeNode.svelte";
  import { treeData, fallbackMibs } from "$lib/stores";

  $: hasTree = $treeData.length > 0;
  $: showFallback = $fallbackMibs.length > 0;
</script>

<aside class="w-[320px] min-w-[200px] max-w-[600px] flex flex-col bg-surface-0 border-r border-base-01 flex-shrink-0">
  <div class="px-3 py-2 text-xs font-semibold uppercase tracking-wide text-overlay bg-base-00 border-b border-base-01">
    MIB Browser
  </div>

  <div class="flex-1 overflow-y-auto overflow-x-hidden py-1">
    {#if !hasTree}
      <p class="text-overlay text-[13px] text-center mt-8">No MIBs loaded.<br/>Use File → Add MIB Directory to get started.</p>
    {:else}
      {#each $treeData as node (node.oid)}
        <TreeNode {node} />
      {/each}
    {/if}
  </div>

  {#if showFallback}
    <div class="bg-base-01 border-t border-base-02 px-2 py-1.5 text-[11px] text-yellow flex items-center gap-2">
      <span>{$fallbackMibs.length} MIB(s) loaded via regex fallback</span>
      <button class="ml-auto bg-transparent border border-base-02 text-yellow px-2 py-0.5 text-[11px] rounded cursor-pointer hover:bg-base-02">
        System Log
      </button>
    </div>
  {/if}
</aside>
