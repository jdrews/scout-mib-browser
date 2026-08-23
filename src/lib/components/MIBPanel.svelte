<script lang="ts">
  import { X } from "lucide-svelte";
  import TreeNode from "./TreeNode.svelte";
  import { S } from "$lib/stores.svelte";
  import { pluralize } from "$lib/format";

  let hasTree = $derived(S.treeData.length > 0);
  let showFallback = $derived(S.fallbackMibs.length > 0 && !S.fallbackBannerDismissed);
  let width = $derived(S.mibPanelWidth);

  // Plain-language copy (UX-11).
  let fallbackCopy = $derived(
    S.fallbackMibs.length === 1
      ? "1 MIB couldn't be fully parsed and was loaded with reduced information."
      : `${pluralize(S.fallbackMibs.length, "MIB")} couldn't be fully parsed and were loaded with reduced information.`
  );

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

<nav aria-label="MIB tree" class="flex flex-col bg-base-200 border-r border-base-300 flex-shrink-0" style="width: {width}px">
  <div data-testid="mib-panel-header" class="px-4 py-3 text-sm font-semibold uppercase tracking-wide text-base-content/60 bg-base-100 border-b border-base-300 flex items-center">
    MIB Browser
    {#if S.fallbackMibs.length > 0 && S.fallbackBannerDismissed}
      <!-- Compact amber indicator (UX-18): click reopens the banner. -->
      <button
        data-testid="fallback-indicator"
        class="ml-auto flex items-center gap-1.5 text-xs font-normal normal-case tracking-normal hover:opacity-80"
        aria-label="{S.fallbackMibs.length} {S.fallbackMibs.length === 1 ? 'MIB' : 'MIBs'} loaded with reduced information — show details"
        onclick={() => (S.fallbackBannerDismissed = false)}
      >
        <span class="w-2 h-2 rounded-full bg-warning inline-block"></span>
        {S.fallbackMibs.length}
      </button>
    {/if}
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
      <span class="flex-1">{fallbackCopy}</span>
      <button data-testid="fallback-syslog-btn" class="btn btn-sm btn-ghost" onclick={toggleSystemLog}>
        System Log
      </button>
      <button
        data-testid="fallback-dismiss-btn"
        aria-label="Dismiss fallback warning"
        class="btn btn-sm btn-ghost"
        onclick={() => (S.fallbackBannerDismissed = true)}
      >
        <X class="w-3.5 h-3.5" />
      </button>
    </div>
  {/if}
</nav>
