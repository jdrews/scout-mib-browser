<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { S } from "$lib/stores.svelte";
  import { logRead, logClear } from "$lib/tauriCommands";
  import type { LogEntry, LogLevel } from "$lib/types";

  const MIN_HEIGHT = 100;
  const MAX_HEIGHT = 800;

  let isResizing = $state(false);
  let startY = $state(0);
  let startHeight = $state(0);

  let intervalId: ReturnType<typeof setInterval> | null = null;
  let logContainer: HTMLDivElement;

  let height = $derived(S.systemLogHeight);
  let filteredEntries = $derived(S.logEntries.filter((entry: LogEntry) => {
    const filter = S.logLevelFilter;
    if (filter === "all") return true;
    if (filter === "error") return entry.level === "ERROR";
    if (filter === "warn") return entry.level === "WARN" || entry.level === "ERROR";
    if (filter === "info") return true;
    return true;
  }));

  function onResizeStart(e: MouseEvent) {
    e.preventDefault();
    isResizing = true;
    startY = e.clientY;
    startHeight = S.systemLogHeight;
    document.addEventListener("mousemove", onResizeMove);
    document.addEventListener("mouseup", onResizeEnd);
    document.body.style.cursor = "row-resize";
    document.body.style.userSelect = "none";
  }

  function onResizeMove(e: MouseEvent) {
    if (!isResizing) return;
    const delta = startY - e.clientY;
    const newHeight = Math.max(MIN_HEIGHT, Math.min(MAX_HEIGHT, startHeight + delta));
    S.systemLogHeight = newHeight;
  }

  function onResizeEnd() {
    isResizing = false;
    document.removeEventListener("mousemove", onResizeMove);
    document.removeEventListener("mouseup", onResizeEnd);
    document.body.style.cursor = "";
    document.body.style.userSelect = "";
  }

  async function loadEntries() {
    try {
      const entries = await logRead();
      S.logEntries.length = 0;
      S.logEntries.push(...entries);
      scrollToBottom();
    } catch (err) {
      console.error("Failed to read logs:", err);
    }
  }

  function scrollToBottom() {
    if (logContainer) {
      logContainer.scrollTop = logContainer.scrollHeight;
    }
  }

  async function handleClear() {
    await logClear();
    S.logEntries.length = 0;
  }

  onMount(() => {
    loadEntries();
    intervalId = setInterval(loadEntries, 1000);
  });

  onDestroy(() => {
    if (intervalId) clearInterval(intervalId);
  });
</script>

<div class="flex flex-col flex-shrink-0 border-t border-base-300 bg-base-100" style="height: {height}px">
  <div
    class="resize-handle-v h-[6px] cursor-row-resize flex-shrink-0 bg-base-200 hover:bg-primary/30 transition-colors"
    onmousedown={onResizeStart}
    role="separator"
    aria-orientation="horizontal"
    tabindex="0"
  >
    <div class="resize-grip-v mx-auto"></div>
  </div>

  <div class="flex items-center justify-between px-2 py-2 bg-base-200 border-b border-base-300">
    <span class="text-xs font-semibold uppercase tracking-wide text-base-content/60">System Log</span>
    <div class="flex items-center gap-2">
      <select
        class="select select-bordered select-sm text-xs"
        bind:value={S.logLevelFilter}
      >
        <option value="all">All</option>
        <option value="info">Info+</option>
        <option value="warn">Warning+</option>
        <option value="error">Error</option>
      </select>
      <button
        class="btn btn-ghost btn-sm"
        onclick={handleClear}
      >
        Clear
      </button>
    </div>
  </div>

  <div
    bind:this={logContainer}
    class="flex-1 overflow-y-auto font-mono text-[13px] py-2"
  >
    {#each filteredEntries as entry, i (i)}
      <div class="flex gap-2 px-4 py-1.5 hover:bg-base-200">
        <span class="text-base-content/60 shrink-0">{entry.timestamp}</span>
        <span class="shrink-0 font-bold w-[46px]" class:text-error={entry.level === "ERROR"} class:text-warning={entry.level === "WARN"} class:text-info={entry.level === "INFO"} class:muted-level={entry.level === "DEBUG" || entry.level === "TRACE"}>
          [{entry.level}]
        </span>
        <span class="text-base-content/60 shrink-0">({entry.target})</span>
        <span class="break-all">{entry.message}</span>
      </div>
    {/each}

    {#if S.logEntries.length === 0}
      <div class="px-4 py-6 text-center text-base-content/60 text-sm">No log entries yet.</div>
    {/if}
  </div>

  <div class="flex items-center justify-between px-4 py-2 bg-base-200 border-t border-base-300 text-xs text-base-content/60">
    <span>{filteredEntries.length} / {S.logEntries.length} entries</span>
    <button
      class="text-xs hover:text-base-content cursor-pointer"
      onclick={scrollToBottom}
    >
      Scroll to bottom
    </button>
  </div>
</div>

<style>
.muted-level {
  color: oklch(var(--bc) / 0.6);
}
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
