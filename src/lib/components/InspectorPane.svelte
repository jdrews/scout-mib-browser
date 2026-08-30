<script lang="ts">
  import { ChevronDown, ChevronUp } from "lucide-svelte";
  import { S } from "$lib/stores.svelte";
  import { mibNodeDetails } from "$lib/tauriCommands";
  import type { MibNodeDetails, NamedValueInfo, TableIndexColumn } from "$lib/types";

  const MIN_HEIGHT = 120;
  const MAX_HEIGHT = 800;

  let isResizing = $state(false);
  let startY = $state(0);
  let startHeight = $state(0);

  let open = $derived(S.inspectorOpen);
  let height = $derived(S.inspectorHeight);
  let oid = $derived(S.inspectorOid);
  let liveValue = $derived(S.inspectorValue);

  let details: MibNodeDetails | null = $state(null);
  let loading = $state(false);
  // Monotonic request guard: a slow response for an older selection must not
  // clobber the details of a newer one.
  let requestSeq = 0;

  $effect(() => {
    const target = oid;
    // Re-run when the MIB set changes: a selected node may have vanished with
    // an unloaded MIB, in which case the refetch resolves to "not found".
    void S.treeVersion;
    if (!target) {
      details = null;
      loading = false;
      return;
    }
    const seq = ++requestSeq;
    loading = true;
    mibNodeDetails(target)
      .then((d) => {
        if (seq === requestSeq) details = d;
      })
      .catch((err) => {
        console.error("Failed to load node details:", err);
        if (seq === requestSeq) details = null;
      })
      .finally(() => {
        if (seq === requestSeq) loading = false;
      });
  });

  function toggle() {
    S.inspectorOpen = !S.inspectorOpen;
  }

  function onResizeStart(e: MouseEvent) {
    e.preventDefault();
    isResizing = true;
    startY = e.clientY;
    startHeight = S.inspectorHeight;
    document.addEventListener("mousemove", onResizeMove);
    document.addEventListener("mouseup", onResizeEnd);
    document.body.style.cursor = "row-resize";
    document.body.style.userSelect = "none";
  }

  function onResizeMove(e: MouseEvent) {
    if (!isResizing) return;
    const delta = startY - e.clientY;
    S.inspectorHeight = Math.max(MIN_HEIGHT, Math.min(MAX_HEIGHT, startHeight + delta));
  }

  function onResizeEnd() {
    isResizing = false;
    document.removeEventListener("mousemove", onResizeMove);
    document.removeEventListener("mouseup", onResizeEnd);
    document.body.style.cursor = "";
    document.body.style.userSelect = "";
  }

  // Keyboard resize for the focusable handle: the handle is the pane's top
  // edge, so ArrowUp grows (same direction as dragging it up).
  const KEY_RESIZE_STEP = 16;
  function onResizeKeydown(e: KeyboardEvent) {
    if (e.key === "ArrowUp") {
      e.preventDefault();
      S.inspectorHeight = Math.min(MAX_HEIGHT, S.inspectorHeight + KEY_RESIZE_STEP);
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      S.inspectorHeight = Math.max(MIN_HEIGHT, S.inspectorHeight - KEY_RESIZE_STEP);
    }
  }

  let attrRows = $derived.by(() => {
    if (!details) return [];
    const rows: [string, string][] = [];
    if (details.access) rows.push(["Access", details.access]);
    if (details.status) rows.push(["Status", details.status]);
    if (details.units) rows.push(["Units", details.units]);
    if (details.defaultValue) rows.push(["Default value", details.defaultValue]);
    if (details.displayHint) rows.push(["Display hint", details.displayHint]);
    if (details.constraints) rows.push(["Constraints", details.constraints]);
    if (details.reference) rows.push(["Reference", details.reference]);
    return rows;
  });

  function indexColumnLabel(c: TableIndexColumn): string {
    return c.implied ? `${c.name} (implied)` : c.name;
  }
</script>

<section data-testid="inspector-pane" class="flex flex-col flex-shrink-0 bg-base-100">
  {#snippet indexColumnList(columns: TableIndexColumn[])}
    <ul class="mt-1 space-y-0.5">
      {#each columns as c (c.name)}
        <li class="text-xs font-mono flex items-center gap-1.5">
          {indexColumnLabel(c)}
          {#if c.implied}<span class="badge badge-outline badge-warning badge-xs">implied</span>{/if}
        </li>
      {/each}
    </ul>
  {/snippet}

  {#snippet namedValueList(values: NamedValueInfo[])}
    <!-- value → name rows; long lists (e.g. IANAifType) scroll in place. -->
    <ul class="mt-1 space-y-0.5 max-h-48 overflow-y-auto pr-1">
      {#each values as v (v.label)}
        <li class="flex gap-3 text-xs font-mono">
          <span class="w-12 shrink-0 text-right text-base-content/70">{v.value}</span>
          <span class="break-all min-w-0">{v.label}</span>
        </li>
      {/each}
    </ul>
  {/snippet}

  {#if open}
    <div
      data-testid="inspector-resize"
      class="resize-handle-v h-[6px] cursor-row-resize flex-shrink-0 bg-base-200 hover:bg-primary/30 transition-colors focus-visible:ring-2 focus-visible:ring-primary"
      onmousedown={onResizeStart}
      onkeydown={onResizeKeydown}
      role="separator"
      aria-orientation="horizontal"
      aria-label="Resize inspector pane (ArrowUp grows, ArrowDown shrinks)"
      tabindex="0"
    >
      <div class="resize-grip-v mx-auto"></div>
    </div>
  {/if}

  <button
    data-testid="inspector-toggle"
    class="flex items-center gap-2 px-4 py-2 bg-base-200 border-t border-b border-base-300 text-left hover:bg-base-300/60 cursor-pointer w-full"
    aria-expanded={open}
    aria-controls="inspector-body"
    onclick={toggle}
  >
    <span class="text-xs font-semibold uppercase tracking-wide text-base-content/60">Inspector</span>
    {#if open}
      <ChevronDown class="w-3.5 h-3.5 ml-auto text-base-content/60" />
    {:else}
      <ChevronUp class="w-3.5 h-3.5 ml-auto text-base-content/60" />
    {/if}
  </button>

  {#if open}
    <div id="inspector-body" data-testid="inspector-body" class="overflow-y-auto flex flex-col" style="height: {height}px">
      {#if !oid}
        <p data-testid="inspector-placeholder" class="text-base-content/60 text-sm text-center mt-8 px-4">
          Select a MIB node to inspect it.
        </p>
      {:else if loading && !details}
        <p class="text-base-content/60 text-sm text-center mt-8 px-4">Loading…</p>
      {:else if !details}
        <div data-testid="inspector-not-found" class="px-4 py-3 text-sm">
          <p class="text-base-content/60 mb-1">Not found in loaded MIBs:</p>
          <p class="font-mono break-all">{oid}</p>
        </div>
      {:else}
        <!-- Identity block: pinned above the scroll area. -->
        <div data-testid="inspector-identity" class="px-4 py-3 border-b border-base-300 bg-base-100">
          <div class="flex items-center gap-2 flex-wrap">
            <span data-testid="inspector-name" class="font-semibold text-sm">{details.name}</span>
            <span data-testid="inspector-type" class="badge badge-outline badge-xs font-mono">{details.syntaxType}</span>
          </div>
          <p data-testid="inspector-oid" class="font-mono text-xs break-all mt-1 text-base-content/80">{details.oid}</p>
        </div>

        <!-- Everything else scrolls. -->
        <div class="flex-1 overflow-y-auto">
          {#if liveValue}
            <div data-testid="inspector-live-value" class="px-4 py-2 border-b border-base-300 bg-base-200/60">
              <p class="text-[10px] font-semibold uppercase tracking-wide text-base-content/60 mb-1">Live value</p>
              <p class="font-mono text-xs break-all">{liveValue.text}</p>
              <p class="text-[10px] text-base-content/60 mt-0.5">{liveValue.typeLabel}</p>
            </div>
          {/if}

          {#if details.description}
            <div data-testid="inspector-description" class="px-4 py-2 border-b border-base-300">
              <p class="text-[10px] font-semibold uppercase tracking-wide text-base-content/60 mb-1">Description</p>
              <p class="text-xs whitespace-pre-wrap break-words">{details.description}</p>
            </div>
          {/if}

          {#if attrRows.length > 0}
            <dl data-testid="inspector-attrs" class="px-4 py-2 border-b border-base-300">
              {#each attrRows as [label, value] (label)}
                <div class="flex gap-2 py-0.5 text-xs">
                  <dt class="w-28 shrink-0 text-base-content/60">{label}</dt>
                  <dd class="font-mono break-all min-w-0 flex-1">{value}</dd>
                </div>
              {/each}
            </dl>
          {/if}

          {#if details.enums && details.enums.length > 0}
            <div data-testid="inspector-enums" class="px-4 py-2 border-b border-base-300">
              <p class="text-[10px] font-semibold uppercase tracking-wide text-base-content/60 mb-1">Values ({details.enums.length})</p>
              {@render namedValueList(details.enums)}
            </div>
          {/if}

          {#if details.bits && details.bits.length > 0}
            <div data-testid="inspector-bits" class="px-4 py-2 border-b border-base-300">
              <p class="text-[10px] font-semibold uppercase tracking-wide text-base-content/60 mb-1">Bits ({details.bits.length})</p>
              {@render namedValueList(details.bits)}
            </div>
          {/if}

          {#if details.table}
            <div data-testid="inspector-table-section" class="px-4 py-2 border-b border-base-300">
              <p class="text-[10px] font-semibold uppercase tracking-wide text-base-content/60 mb-1">Table</p>
              <p class="text-xs">
                {details.table.columnOids.length} column(s), indexed by:
              </p>
              {@render indexColumnList(details.table.indexColumns)}
            </div>
          {/if}

          {#if details.indexColumns && details.indexColumns.length > 0}
            <div data-testid="inspector-row-section" class="px-4 py-2">
              <p class="text-[10px] font-semibold uppercase tracking-wide text-base-content/60 mb-1">Row entry — INDEX</p>
              {@render indexColumnList(details.indexColumns)}
            </div>
          {/if}
        </div>
      {/if}
    </div>
  {/if}
</section>

<style>
  .resize-handle-v {
    position: relative;
  }
  .resize-handle-v:hover,
  .resize-handle-v:active {
    background-color: oklch(var(--p) / 0.15);
  }
  .resize-grip-v {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    height: 2px;
    width: 32px;
    border-radius: 1px;
    background-color: oklch(var(--bc) / 0.2);
  }
  .resize-handle-v:hover .resize-grip-v {
    background-color: oklch(var(--p) / 0.6);
  }
</style>
