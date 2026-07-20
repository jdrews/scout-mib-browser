<script lang="ts">
  import type { TreeNode } from "$lib/types";
  import { selectedNode, contextMenuTarget } from "$lib/stores";

  export let node: TreeNode;

  let expanded = false;
  $: hasChildren = !!(node.children && node.children.length > 0);
  $: isSelected = $selectedNode?.oid === node.oid;
  $: childrenList = node.children ?? [];

  function toggleExpand(e: MouseEvent) {
    e.stopPropagation();
    expanded = !expanded;
  }

  function selectNode() {
    $selectedNode = node;
  }

  function showContextMenu(e: MouseEvent) {
    e.preventDefault();
    $contextMenuTarget = { node, x: e.clientX, y: e.clientY };
  }
</script>

<div class="tree-node">
  <div
    class="flex items-center px-1 py-[2px] cursor-default rounded text-[13px] whitespace-nowrap select-none hover:bg-base-01"
    class:selected={isSelected}
    on:click={selectNode}
    on:contextmenu={showContextMenu}
  >
    {#if hasChildren}
      <span
        class="w-4 h-4 flex items-center justify-center text-[10px] text-overlay cursor-pointer flex-shrink-0 hover:text-subtext-1"
        on:click={toggleExpand}
      >
        <svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" class:rotate-90={expanded}><polyline points="9 18 15 12 9 6"/></svg>
      </span>
    {/if}

    <span class="w-4 h-4 flex items-center justify-center mr-1 text-xs flex-shrink-0">
      {#if hasChildren}
        <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
      {:else}
        <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>
      {/if}
    </span>

    <span class="overflow-hidden text-ellipsis text-text" title="{node.name} ({node.oid})">
      {node.name}
    </span>

    <span class="ml-auto pl-2 text-[11px] text-overlay font-mono">{node.oid}</span>
  </div>

  {#if hasChildren && expanded}
    <div class="pl-3">
      {#each childrenList as child (child.oid)}
        <svelte:self {child} />
      {/each}
    </div>
  {/if}
</div>
