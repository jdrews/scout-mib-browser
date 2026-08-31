<script lang="ts">
  import { X, Search, ChevronUp, ChevronDown } from "lucide-svelte";
  import { tick } from "svelte";
  import TreeNode from "./TreeNode.svelte";
  import InspectorPane from "./InspectorPane.svelte";
  import { S } from "$lib/stores.svelte";
  import { pluralize } from "$lib/format";
  import { searchOids, findChain } from "$lib/treeSearch";
  import { getTreeNode } from "$lib/treeRegistry";

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
    void S.treeVersion;
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

  // ── Find in tree ───────────────────────────────────────────────────────────
  // Searches the OIDs already loaded (oidNameMap), so it works even when the
  // tree is fully collapsed: navigating a hit expands the branch to it.

  let findInput: HTMLInputElement | undefined;
  let findIndex = $state(-1);
  let lastMatches: string[] | null = null;
  // Bumped on every navigation so a slow reveal chain can't clobber a newer one.
  let revealSeq = 0;

  let findMatches = $derived(S.treeFindQuery.trim() ? searchOids(S.oidNameMap, S.treeFindQuery) : []);

  let findCount = $derived(
    !S.treeFindQuery.trim()
      ? ""
      : findMatches.length === 0
        ? "No matches"
        : `${findIndex + 1}/${findMatches.length}`
  );

  // A new match list (typing, or the loaded MIBs changing) jumps to the first hit.
  $effect(() => {
    const matches = findMatches;
    if (matches === lastMatches) return;
    lastMatches = matches;
    if (matches.length === 0) {
      findIndex = -1;
      S.treeFindOid = null;
      return;
    }
    findIndex = 0;
    void goToMatch(matches[0]);
  });

  $effect(() => {
    if (S.treeFindOpen) findInput?.focus();
  });

  function toggleFind() {
    if (S.treeFindOpen) {
      // Hide the bar and clear the highlight, but leave scroll/selection alone.
      // The query resets so a reopened find starts fresh.
      S.treeFindOpen = false;
      S.treeFindQuery = "";
      S.treeFindOid = null;
      findIndex = -1;
    } else {
      S.treeFindOpen = true;
    }
  }

  function nextMatch() {
    const m = findMatches;
    if (m.length === 0) return;
    findIndex = (findIndex + 1) % m.length;
    void goToMatch(m[findIndex]);
  }

  function prevMatch() {
    const m = findMatches;
    if (m.length === 0) return;
    findIndex = (findIndex - 1 + m.length) % m.length;
    void goToMatch(m[findIndex]);
  }

  function onFindKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      if (e.shiftKey) prevMatch();
      else nextMatch();
    } else if (e.key === "Escape") {
      e.preventDefault();
      toggleFind();
    }
  }

  /** Expands every ancestor of `oid` (top-down), then scrolls the row into
   *  view. Never collapses anything, and doesn't change selection/focus. */
  async function goToMatch(oid: string) {
    const seq = ++revealSeq;
    S.treeFindOid = oid;
    const chain = findChain(oid, S.oidNameMap);
    for (let i = 0; i < chain.length - 1; i++) {
      if (seq !== revealSeq) return;
      let handle = getTreeNode(chain[i]);
      if (!handle && i === 0) {
        // Single-segment leaf roots render inside the "other" folder.
        const other = getTreeNode("__other__");
        if (other) {
          await other.expand();
          if (seq !== revealSeq) return;
          await tick();
        }
        handle = getTreeNode(chain[i]);
      }
      if (!handle) return;
      await handle.expand();
      if (seq !== revealSeq) return;
      await tick();
    }
    if (seq !== revealSeq) return;
    getTreeNode(oid)?.el.scrollIntoView({ block: "center" });
  }
</script>

<nav aria-label="MIB tree" class="flex flex-col bg-base-200 border-r border-base-300 flex-shrink-0" style="width: {width}px">
  <div data-testid="mib-panel-header" class="px-4 py-3 text-sm font-semibold uppercase tracking-wide text-base-content/60 bg-base-100 border-b border-base-300 flex items-center">
    MIB Browser
    <span class="ml-auto flex items-center gap-1">
      {#if S.fallbackMibs.length > 0 && S.fallbackBannerDismissed}
        <!-- Compact amber indicator (UX-18): click reopens the banner. -->
        <button
          data-testid="fallback-indicator"
          class="flex items-center gap-1.5 text-xs font-normal normal-case tracking-normal hover:opacity-80"
          aria-label="{S.fallbackMibs.length} {S.fallbackMibs.length === 1 ? 'MIB' : 'MIBs'} loaded with reduced information — show details"
          onclick={() => (S.fallbackBannerDismissed = false)}
        >
          <span class="w-2 h-2 rounded-full bg-warning inline-block"></span>
          {S.fallbackMibs.length}
        </button>
      {/if}
      <button
        data-testid="mib-find-toggle"
        aria-label="Find in MIB tree"
        aria-expanded={S.treeFindOpen}
        title="Find in MIB tree"
        class="btn btn-ghost btn-sm text-base-content/60 hover:text-base-content {S.treeFindOpen ? 'text-primary' : ''}"
        onclick={toggleFind}
      >
        <Search class="w-4 h-4" />
      </button>
    </span>
  </div>

  {#if S.treeFindOpen}
    <div data-testid="mib-find-bar" role="search" class="px-3 py-2 border-b border-base-300 bg-base-100 flex items-center gap-1.5">
      <input
        bind:this={findInput}
        bind:value={S.treeFindQuery}
        type="text"
        placeholder="Find OID or name…"
        aria-label="Find in MIB tree"
        data-testid="mib-find-input"
        class="input input-xs flex-1 min-w-0"
        onkeydown={onFindKeydown}
      />
      <span data-testid="mib-find-count" class="text-xs font-mono text-base-content/60 whitespace-nowrap min-w-14 text-right">
        {findCount}
      </span>
      <button
        data-testid="mib-find-prev"
        aria-label="Previous match"
        title="Previous match (Shift+Enter)"
        class="btn btn-ghost btn-xs"
        onclick={prevMatch}
      >
        <ChevronUp class="w-4 h-4" />
      </button>
      <button
        data-testid="mib-find-next"
        aria-label="Next match"
        title="Next match (Enter)"
        class="btn btn-ghost btn-xs"
        onclick={nextMatch}
      >
        <ChevronDown class="w-4 h-4" />
      </button>
    </div>
  {/if}

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

  <!-- The inspector owns the bottom-left corner, even when the banner shows. -->
  <InspectorPane />
</nav>
