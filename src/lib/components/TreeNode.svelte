<script lang="ts">
  import { FileText, Folder, FolderOpen } from "lucide-svelte";
  import type { TreeNode as TreeNodeType } from "$lib/types";
  import { S } from "$lib/stores.svelte";
  import Self from "./TreeNode.svelte";

  let { node }: { node: TreeNodeType } = $props();

  let hasChildren = $derived(!!node.hasChildren);
  let isSelected = $derived(S.selectedNode?.oid === node.oid);
  let isFallbackNode = $derived(S.fallbackMibs.includes(node.mibName));
  let childrenList = $state<TreeNodeType[]>(node.children ?? []);
  let loaded = $state(!!node.children && node.children.length > 0);
  let loading = $state(false);
  let expanded = $state(false);

  // Roving tabindex: exactly one treeitem in the tree holds tabindex=0.
  let tabIndex = $derived(S.treeFocusOid === node.oid ? 0 : -1);

  // When the tree is rebuilt (MIB unloaded / directory added), this branch's
  // cached children may be stale or gone entirely — refetch so ghost nodes
  // don't linger under an expanded branch. Plain (non-reactive) guard: the
  // effect re-runs when loading/loaded change mid-fetch, but only a version
  // bump should trigger work.
  let lastSeenVersion = 0;
  $effect(() => {
    const version = S.treeVersion;
    if (version === lastSeenVersion) return;
    lastSeenVersion = version;
    if (!hasChildren) {
      expanded = false;
      childrenList = [];
      loaded = false;
      return;
    }
    if (loaded && !loading) {
      loaded = false;
      loadChildren();
    }
  });

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

  function expandNode() {
    expanded = true;
    loadChildren();
  }

  function collapseNode() {
    expanded = false;
  }

  function selectNode() {
    S.selectedNode = node;
    S.targetOidFromTree = node.oid;
    S.treeFocusOid = node.oid;
    // Tree selections carry no live value — clear any from a prior result pick.
    S.inspectorOid = node.oid;
    S.inspectorValue = null;
  }

  /** Row click selects; branch nodes also expand (never collapse — selecting a
   *  branch must not hide its subtree). Collapsing is the icon toggle or ArrowLeft. */
  function onClick() {
    if (hasChildren && !expanded) expandNode();
    selectNode();
  }

  /** Dedicated expand/collapse affordance (pointer only; keyboard uses arrows). */
  function onToggleClick(e: MouseEvent) {
    e.stopPropagation();
    if (!hasChildren) return;
    if (expanded) collapseNode();
    else expandNode();
  }

  function showContextMenu(e: MouseEvent) {
    e.preventDefault();
    selectNode();
    S.contextMenuTarget = { node, x: e.clientX, y: e.clientY };
  }

  let nodeEl: HTMLDivElement;

  /** All rendered treeitems in document order (children render only when expanded). */
  function visibleTreeItems(): HTMLElement[] {
    return Array.from(document.querySelectorAll("[role='treeitem']")) as HTMLElement[];
  }

  function focusItem(el: Element | null) {
    if (!(el instanceof HTMLElement)) return;
    const oid = el.getAttribute("data-oid");
    if (oid) S.treeFocusOid = oid;
    el.focus();
  }

  /** ARIA tree pattern keyboard navigation. */
  function onKeydown(e: KeyboardEvent) {
    const items = visibleTreeItems();
    const idx = items.indexOf(nodeEl);
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        focusItem(items[idx + 1] ?? null);
        break;
      case "ArrowUp":
        e.preventDefault();
        focusItem(items[idx - 1] ?? null);
        break;
      case "ArrowRight":
        e.preventDefault();
        if (hasChildren && !expanded) {
          expandNode();
        } else if (hasChildren && expanded) {
          // First child is the next treeitem in document order.
          focusItem(items[idx + 1] ?? null);
        }
        break;
      case "ArrowLeft":
        e.preventDefault();
        if (hasChildren && expanded) {
          collapseNode();
        } else {
          const parentLi = nodeEl.closest("ul[role='group']")?.closest("li");
          focusItem(parentLi ? parentLi.querySelector(":scope > [role='treeitem']") : null);
        }
        break;
      case "Enter":
      case " ":
        e.preventDefault();
        selectNode();
        break;
    }
  }
</script>

<li role="presentation">
  <div
    bind:this={nodeEl}
    role="treeitem"
    data-tree-node
    data-oid={node.oid}
    title="{node.name} ({node.oid})"
    tabindex={tabIndex}
    aria-selected={isSelected}
    aria-expanded={hasChildren ? expanded : undefined}
    class="tree-row flex items-center gap-1.5 px-2 py-1 text-sm cursor-pointer select-none whitespace-nowrap rounded hover:bg-base-300/60 focus:outline-none focus-visible:ring-2 focus-visible:ring-primary"
    class:selected={isSelected}
    class:fallback-node={isFallbackNode}
    onclick={onClick}
    onkeydown={onKeydown}
    oncontextmenu={showContextMenu}
  >
    {#if hasChildren}
      <span
        class="tree-toggle inline-flex cursor-pointer rounded hover:bg-base-300"
        onclick={onToggleClick}
        title={expanded ? "Collapse" : "Expand"}
      >
        {#if expanded}
          <FolderOpen class="h-4 w-4 shrink-0" />
        {:else}
          <Folder class="h-4 w-4 shrink-0" />
        {/if}
      </span>
    {:else}
      <FileText class="h-4 w-4 shrink-0" />
    {/if}
    <span class="truncate">{node.name}</span>
    {#if isFallbackNode}
      <span class="badge badge-outline badge-warning badge-xs">unresolved</span>
    {/if}
    {#if loading}
      <span class="ml-auto pl-2 text-[10px] opacity-40">loading...</span>
    {:else}
      <span class="ml-auto pl-2 font-mono text-[10px] opacity-60">{truncatedOid}</span>
    {/if}
  </div>
  {#if hasChildren && expanded}
    <ul role="group" class="ml-3 border-l border-base-300 pl-1">
      {#each childrenList as child (child.oid)}
        <Self node={child} />
      {/each}
    </ul>
  {/if}
</li>

<style>
  .tree-row.selected {
    background-color: oklch(var(--p) / 0.2);
    color: var(--fallback-pc, oklch(var(--pc)));
  }

  .tree-row.fallback-node {
    opacity: 0.55;
  }
</style>
