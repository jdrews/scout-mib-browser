<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { logEntries, systemLogOpen, logLevelFilter } from "$lib/stores";
  import { logRead, logClear } from "$lib/tauriCommands";
  import { listen } from "@tauri-apps/api/event";
  import type { LogEntry, LogLevel } from "$lib/types";

  let intervalId: ReturnType<typeof setInterval> | null = null;
  let logContainer: HTMLDivElement;
  let unlistenEvent: (() => void) | null = null;

  $: filteredEntries = $logEntries.filter((entry: LogEntry) => {
    const filter = $logLevelFilter;
    if (filter === "all") return true;
    if (filter === "error") return entry.level === "ERROR";
    if (filter === "warn") return entry.level === "WARN" || entry.level === "ERROR";
    if (filter === "info") return true;
    return true;
  });

  async function loadEntries() {
    try {
      const entries = await logRead();
      $logEntries = entries;
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
    $logEntries = [];
  }

  onMount(async () => {
    const unlisten = await listen<LogEntry>("system-log-entry", (event) => {
      if ($systemLogOpen) {
        $logEntries = [...$logEntries, event.payload];
        scrollToBottom();
      }
    });
    unlistenEvent = unlisten;

    loadEntries();
    intervalId = setInterval(loadEntries, 1000);
  });

  onDestroy(() => {
    if (intervalId) clearInterval(intervalId);
    if (unlistenEvent) unlistenEvent();
  });
</script>

<div class="collapse collapse-arrow bg-base-100 border-t border-base-300" class:collapse-open={$systemLogOpen}>
  <input type="checkbox" bind:checked={$systemLogOpen} />
  <div class="collapse-title text-sm font-medium">System Log</div>
  <div class="collapse-content flex flex-col h-full">
    <div class="flex items-center justify-between px-2 py-2 bg-base-200 border-b border-base-300">
      <span class="text-xs font-semibold uppercase tracking-wide text-base-content/60">System Log</span>
      <div class="flex items-center gap-2">
        <select
          class="select select-bordered select-sm text-xs"
          bind:value={$logLevelFilter}
        >
          <option value="all">All</option>
          <option value="info">Info+</option>
          <option value="warn">Warning+</option>
          <option value="error">Error</option>
        </select>
        <button
          class="btn btn-ghost btn-sm"
          on:click={handleClear}
        >
          Clear
        </button>
      </div>
    </div>

    <div
      bind:this={logContainer}
      class="flex-1 overflow-y-auto font-mono text-[13px] py-2"
    >
      {#each filteredEntries as entry (entry.timestamp + entry.message)}
        <div class="flex gap-2 px-4 py-1.5 hover:bg-base-200">
          <span class="text-base-content/60 shrink-0">{entry.timestamp}</span>
          <span class="shrink-0 font-bold w-[46px]" class:text-error={entry.level === "ERROR"} class:text-warning={entry.level === "WARN"} class:text-info={entry.level === "INFO"} class:muted-level={entry.level === "DEBUG" || entry.level === "TRACE"}>
            [{entry.level}]
          </span>
          <span class="text-base-content/60 shrink-0">({entry.target})</span>
          <span class="break-all">{entry.message}</span>
        </div>
      {/each}

      {#if $logEntries.length === 0}
        <div class="px-4 py-6 text-center text-base-content/60 text-sm">No log entries yet.</div>
      {/if}
    </div>

    <div class="flex items-center justify-between px-4 py-2 bg-base-200 border-t border-base-300 text-xs text-base-content/60">
      <span>{filteredEntries.length} / {$logEntries.length} entries</span>
      <button
        class="text-xs hover:text-base-content cursor-pointer"
        on:click={scrollToBottom}
      >
        Scroll to bottom
      </button>
    </div>
  </div>
</div>

<style>
.muted-level {
  color: oklch(var(--bc) / 0.6);
}
</style>
