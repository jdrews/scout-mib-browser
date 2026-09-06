<script lang="ts">
  import { X, Search, ChevronUp, ChevronDown } from "lucide-svelte";
  import { tick } from "svelte";
  import TreeNode from "./TreeNode.svelte";
  import InspectorPane from "./InspectorPane.svelte";
  import { S } from "$lib/stores.svelte";
  import { pluralize } from "$lib/format";
  import { searchOids, findChain } from "$lib/treeSearch";
  import { getTreeNode, findRenderedDescendant } from "$lib/treeRegistry";

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
  // Max passes of the reveal walk: one per tree level, capped well beyond any
  // realistic MIB tree depth.
  const MAX_REVEAL_PASSES = 20;

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

  /** True when `oid` is a single-segment leaf root — the only rows that render
   *  inside the "other" folder. A single-segment root with indexed descendants
   *  renders as its own (possibly absorbed) top-level row instead. */
  function isOtherFolderRoot(oid: string): boolean {
    if (oid.includes(".")) return false;
    const prefix = `${oid}.`;
    for (const o of S.oidNameMap.keys()) {
      if (o.startsWith(prefix)) return false;
    }
    return true;
  }

  /** Expands every rendered ancestor of `oid` (top-down), then scrolls the row
   *  into view. Nodes absorbed by empty-folder collapse have no row of their
   *  own — the rendered representative is a descendant carrying the dot-joined
   *  name — so an unrendered target is re-aimed at that descendant and the walk
   *  repeats until it renders. Never collapses anything; doesn't change
   *  selection/focus. */
  async function goToMatch(oid: string) {
    const seq = ++revealSeq;
    S.treeFindOid = oid;
    let target = oid;

    // One pass per tree level at most: each redirect re-aims at a deeper
    // rendered representative, and each expansion loads one more level.
    for (let pass = 0; pass < MAX_REVEAL_PASSES; pass++) {
      if (seq !== revealSeq) return;
      if (getTreeNode(target)) break;
      let expanded = false;
      const chain = findChain(target, S.oidNameMap);
      // Single-segment leaf roots render inside the "other" folder — expand it
      // so their rows exist before the ancestor walk (also covers a target
      // that is itself such a root, whose chain has no ancestors to walk).
      if (isOtherFolderRoot(chain[0])) {
        const other = getTreeNode("__other__");
        if (other) {
          if (await other.expand()) expanded = true;
          if (seq !== revealSeq) return;
          await tick();
        }
      }
      for (let i = 0; i < chain.length - 1; i++) {
        if (seq !== revealSeq) return;
        const handle = getTreeNode(chain[i]);
        // Absorbed ancestors have no row — their representative is a later
        // chain element, so skip instead of aborting.
        if (!handle) continue;
        if (await handle.expand()) expanded = true;
        if (seq !== revealSeq) return;
        await tick();
      }
      const rep = findRenderedDescendant(target);
      if (rep && rep !== target) {
        // The match sits in an absorbed node — follow the hit tint to the row.
        target = rep;
        S.treeFindOid = rep;
        continue;
      }
      if (!expanded) break; // nothing new to expand — give up
    }
    if (seq !== revealSeq) return;
    getTreeNode(target)?.el.scrollIntoView({ block: "center" });
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
