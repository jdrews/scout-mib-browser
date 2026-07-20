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
      >{expanded ? "▼" : "▶"}</span>
    {/if}

    <span class="w-4 h-4 flex items-center justify-center mr-1 text-xs flex-shrink-0">
      {hasChildren ? "📁" : "📄"}
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
