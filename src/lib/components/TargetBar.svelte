<script lang="ts">
  import { targetConfig, connectionPanelOpen, statusText } from "$lib/stores";
  import { persistTargetConfig } from "$lib/tauriCommands";

  $: cfg = $targetConfig;

  function onHostInput(e: Event) {
    const val = (e.target as HTMLInputElement).value;
    const next = { ...cfg, host: val };
    $targetConfig = next;
    persistTargetConfig(next);
  }

  function onPortInput(e: Event) {
    const val = parseInt((e.target as HTMLInputElement).value, 10);
    if (!isNaN(val) && val > 0 && val < 65536) {
      const next = { ...cfg, port: val };
      $targetConfig = next;
      persistTargetConfig(next);
    }
  }

  function openConnectionPanel() {
    $connectionPanelOpen = true;
  }
</script>

<div class="flex items-center gap-2 px-3 py-1.5 bg-surface-0 border-b border-base-01 flex-shrink-0">
  <label class="text-[11px] font-semibold uppercase tracking-wide text-overlay whitespace-nowrap">Target</label>

  <input
    type="text"
    placeholder="Host or IP"
    value={cfg.host}
    on:input={onHostInput}
    class="w-[200px] bg-base-00 border border-base-01 text-text px-2 py-1 text-[13px] font-mono rounded outline-none focus:border-blue"
  />

  <span class="text-overlay text-[13px]">:</span>

  <input
    type="text"
    placeholder="Port"
    value={cfg.port}
    on:input={onPortInput}
    class="w-[70px] bg-base-00 border border-base-01 text-text px-2 py-1 text-[13px] font-mono rounded outline-none focus:border-blue text-center"
  />

  <button
    title="Connection settings"
    on:click={openConnectionPanel}
    class="ml-auto text-overlay hover:text-text cursor-pointer p-1 rounded transition-colors"
  >
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <circle cx="12" cy="12" r="3"/>
      <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>
    </svg>
  </button>
</div>
