<script lang="ts">
  import { X } from "lucide-svelte";
  import { tick } from "svelte";
  import { S } from "$lib/stores.svelte";
  import { mibLoadedList, mibUnload, mibTree } from "$lib/tauriCommands";
  import { pluralize } from "$lib/format";
  import { trapFocus } from "$lib/focusTrap";
  import type { LoadedMib } from "$lib/types";

  let mibs: LoadedMib[] = $state([]);
  let loading = $state(false);
  let dataLoaded = $state(false);

  let panelEl: HTMLDialogElement | undefined;
  let lastTrigger: HTMLElement | null = null;

  // Dialog pattern (UX-10): same treatment as the connection modal.
  $effect(() => {
    if (!S.manageMibsOpen) return;
    lastTrigger = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    let cleanup: (() => void) | undefined;
    void tick().then(() => {
      if (panelEl) cleanup = trapFocus(panelEl, close);
    });
    return () => {
      cleanup?.();
      if (lastTrigger instanceof HTMLElement) lastTrigger.focus();
    };
  });

  async function loadMibs() {
    if (dataLoaded) return;
    dataLoaded = true;
    loading = true;
    try {
      mibs = await mibLoadedList();
    } catch (err) {
      S.statusText = `Error: ${err}`;
      console.error("Failed to load MIB list:", err);
    }
    loading = false;
  }

  async function unloadMib(mibName: string) {
    try {
      const status = await mibUnload(mibName);
      S.nodeCount = status.nodeCount;
      S.fallbackMibs.length = 0;
      S.fallbackMibs.push(...status.fallbackMibs);

      try {
        const pairs = await import("$lib/tauriCommands").then(m => m.mibOidNameMap());
        S.oidNameMap = new Map(pairs);
      } catch (e) {
        console.error("Failed to refresh OID name map:", e);
      }

      const data = await mibTree();
      S.treeData.length = 0;
      S.treeData.push(...data);
      S.treeVersion++;

      mibs = mibs.filter(m => m.mibName !== mibName);
      S.statusText = `Unloaded ${mibName}`;
    } catch (err) {
      S.statusText = `Error: ${err}`;
      console.error("Failed to unload MIB:", err);
    }
  }

  function close() {
    dataLoaded = false;
    mibs = [];
    S.manageMibsOpen = false;
  }

  $effect(() => {
    if (S.manageMibsOpen && !dataLoaded) {
      loadMibs();
    }
  });
</script>

{#if S.manageMibsOpen}
  <dialog role="dialog" aria-modal="true" aria-labelledby="manage-mibs-dialog-title" bind:this={panelEl} class="modal modal-open" onclick={close}>
    <div data-testid="manage-mibs-dialog" class="modal-box max-w-[560px] max-h-[70vh] flex flex-col" onclick={(e) => e.stopPropagation()}>
      <form method="dialog">
        <button data-autofocus aria-label="Close Manage MIBs dialog" class="btn btn-sm btn-circle btn-ghost absolute right-2 top-2 hover:text-error"><X class="w-4 h-4" /></button>
      </form>
      <h3 id="manage-mibs-dialog-title" class="text-lg font-bold">Manage MIBs</h3>

      <div class="flex-1 overflow-y-auto mt-4">
        {#if loading}
          <p class="text-base-content/60 text-sm text-center mt-12">Loading...</p>
        {:else if mibs.length === 0}
          <p class="text-base-content/60 text-sm text-center mt-12">No MIBs currently loaded.</p>
        {:else}
          {#each mibs as mib (mib.filePath)}
            <div data-testid="mib-row" class="flex items-center px-4 py-2.5 rounded gap-3 hover:bg-base-200">
              <span class="flex-1 text-sm">{mib.mibName}</span>
              <span class="text-xs text-base-content/60 font-mono max-w-[240px] overflow-hidden text-ellipsis whitespace-nowrap" title="{mib.filePath}">
                {mib.filePath}
              </span>
              <div class="flex gap-2 items-center text-xs">
                {#if mib.isFallback}
                  <span class="badge badge-warning badge-sm">FALLBACK</span>
                {/if}
                <span data-testid="mib-node-count">{pluralize(mib.nodeCount, "node")}</span>
              </div>
              <button data-testid="unload-btn" class="btn btn-error btn-xs" onclick={() => unloadMib(mib.mibName)}>
                Unload
              </button>
            </div>
          {/each}
        {/if}
      </div>

      <div class="modal-action">
        <button data-testid="manage-mibs-close" class="btn btn-primary" onclick={close}>
          Close
        </button>
      </div>
    </div>
  </dialog>
{/if}
