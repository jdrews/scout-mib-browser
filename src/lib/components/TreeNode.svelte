<script lang="ts">
  import type { TreeNode as TreeNodeType } from "$lib/types";
  import { S } from "$lib/stores.svelte";
  import Self from "./TreeNode.svelte";

  let { node }: { node: TreeNodeType } = $props();

  let hasChildren = $derived(!!(node.children && node.children.length > 0));
  let isSelected = $derived(S.selectedNode?.oid === node.oid);
  let childrenList = $derived(node.children ?? []);
  let truncatedOid = $derived(truncateOid(node.oid));

  function truncateOid(oid: string): string {
    const segments = oid.split(".");
    if (segments.length <= 5) return oid;
    const tail = segments.slice(-6).join(".");
    return `...${tail}`;
  }

  function selectNode() {
    S.selectedNode = node;
    S.targetOidFromTree = node.oid;
  }

  function showContextMenu(e: MouseEvent) {
    e.preventDefault();
    selectNode();
    S.contextMenuTarget = { node, x: e.clientX, y: e.clientY };
  }
</script>

<li>
  {#if hasChildren}
    <details open>
      <summary
        data-tree-node
        title="{node.name} ({node.oid})"
        onclick={selectNode}
        oncontextmenu={showContextMenu}
      >
        <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="h-4 w-4"><path stroke-linecap="round" stroke-linejoin="round" d="M2.25 12.75V12A2.25 2.25 0 014.5 9.75h15A2.25 2.25 0 0121.75 12v.75m-8.69-6.44l-2.12-2.12a1.5 1.5 0 00-1.061-.44H4.5A2.25 2.25 0 002.25 6v12a2.25 2.25 0 002.25 2.25h15A2.25 2.25 0 0021.75 18V9a2.25 2.25 0 00-2.25-2.25h-5.379a1.5 1.5 0 01-1.06-.44z" /></svg>
        <span class="truncate">{node.name}</span>
        <span class="ml-auto pl-2 font-mono text-[10px] opacity-60">{truncatedOid}</span>
      </summary>
      <ul>
        {#each childrenList as child (child.oid)}
          <Self node={child} />
        {/each}
      </ul>
    </details>
  {:else}
    <a
      data-tree-node
      title="{node.name} ({node.oid})"
      onclick={(e) => { e.preventDefault(); selectNode(); }}
      oncontextmenu={showContextMenu}
    >
      <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="h-4 w-4"><path stroke-linecap="round" stroke-linejoin="round" d="M19.5 14.25v-2.625a3.375 3.375 0 00-3.375-3.375h-1.5A1.125 1.125 0 0113.5 7.125v-1.5a3.375 3.375 0 00-3.375-3.375H8.25m0 12.75h7.5m-7.5 3H12M10.5 2.25H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 00-9-9z" /></svg>
      <span class="truncate">{node.name}</span>
      <span class="ml-auto pl-2 font-mono text-[10px] opacity-60">{truncatedOid}</span>
    </a>
  {/if}
</li>

<style>
  summary {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    white-space: nowrap;
    user-select: none;
  }

  a {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    white-space: nowrap;
    user-select: none;
  }
</style>
