<script lang="ts">
  import { FileText, Folder, FolderOpen } from "lucide-svelte";
  import type { TreeNode as TreeNodeType } from "$lib/types";
  import { S } from "$lib/stores.svelte";
  import Self from "./TreeNode.svelte";

  let { node }: { node: TreeNodeType } = $props();

  let hasChildren = $derived(!!node.hasChildren);
  let isSelected = $derived(S.selectedNode?.oid === node.oid);
  let childrenList = $state<TreeNodeType[]>(node.children ?? []);
  let loaded = $state(!!node.children && node.children.length > 0);
  let loading = $state(false);
  let expanded = $state(false);

  let truncatedOid = $derived(truncateOid(node.oid));

  function truncateOid(oid: string): string {
    const segments = oid.split(".");
    if (segments.length <= 5) return oid;
    const tail = segments.slice(-6).join(".");
    return `...${tail}`;
  }

  async function loadChildren() {
    if (loading || loaded || !hasChildren) return;
    loading = true;
    try {
      const { mibChildren } = await import("$lib/tauriCommands");
      const data = await mibChildren(node.oid);
      childrenList = data;
      loaded = true;
    } catch (err) {
      console.error("Failed to load children for", node.oid, err);
    } finally {
      loading = false;
    }
  }

  function onToggle(e: Event) {
    const isNowOpen = (e.target as HTMLDetailsElement).open;
    expanded = isNowOpen;
    if (isNowOpen) {
      loadChildren();
    }
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
    <details ontoggle={onToggle}>
      <summary
        data-tree-node
        title="{node.name} ({node.oid})"
        onclick={selectNode}
        oncontextmenu={showContextMenu}
      >
        {#if expanded}
          <FolderOpen class="h-4 w-4 shrink-0" />
        {:else}
          <Folder class="h-4 w-4 shrink-0" />
        {/if}
        <span class="truncate">{node.name}</span>
        {#if loading}
          <span class="ml-auto pl-2 text-[10px] opacity-40">loading...</span>
        {:else}
          <span class="ml-auto pl-2 font-mono text-[10px] opacity-60">{truncatedOid}</span>
        {/if}
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
      <FileText class="h-4 w-4 shrink-0" />
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
