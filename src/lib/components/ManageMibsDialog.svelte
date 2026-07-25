<script lang="ts">
  import { manageMibsOpen } from "$lib/stores";
  import { mibLoadedList, mibUnload, mibTree } from "$lib/tauriCommands";
  import { statusText, nodeCount, fallbackMibs, treeData } from "$lib/stores";
  import type { LoadedMib } from "$lib/types";

  let mibs: LoadedMib[] = [];
  let loading = false;
  let dataLoaded = false;

  async function loadMibs() {
    if (dataLoaded) return;
    dataLoaded = true;
    loading = true;
    try {
      mibs = await mibLoadedList();
    } catch (err) {
      $statusText = `Error: ${err}`;
      console.error("Failed to load MIB list:", err);
    }
    loading = false;
  }

  async function unloadMib(mibName: string) {
    try {
      const status = await mibUnload(mibName);
      $nodeCount = status.nodeCount;
      $fallbackMibs = status.fallbackMibs;

      const data = await mibTree();
      $treeData = data;

      mibs = mibs.filter(m => m.mibName !== mibName);
      $statusText = `Unloaded ${mibName}`;
    } catch (err) {
      $statusText = `Error: ${err}`;
      console.error("Failed to unload MIB:", err);
    }
  }

  function close() {
    dataLoaded = false;
    mibs = [];
    $manageMibsOpen = false;
  }

  $: if ($manageMibsOpen && !dataLoaded) {
    loadMibs();
  }
</script>

{#if $manageMibsOpen}
  <dialog class="modal modal-open" on:click={close}>
    <div class="modal-box max-w-[560px] max-h-[70vh] flex flex-col" on:click|stopPropagation>
      <form method="dialog">
        <button class="btn btn-sm btn-circle btn-ghost absolute right-2 top-2 hover:text-error">✕</button>
      </form>
      <h3 class="text-lg font-bold">Manage MIBs</h3>

      <div class="flex-1 overflow-y-auto mt-4">
        {#if loading}
          <p class="text-base-content/60 text-sm text-center mt-12">Loading...</p>
        {:else if mibs.length === 0}
          <p class="text-base-content/60 text-sm text-center mt-12">No MIBs currently loaded.</p>
        {:else}
          {#each mibs as mib (mib.mibName)}
            <div class="flex items-center px-4 py-2.5 rounded gap-3 hover:bg-base-200">
              <span class="flex-1 text-sm">{mib.mibName}</span>
              <span class="text-xs text-base-content/60 font-mono max-w-[240px] overflow-hidden text-ellipsis whitespace-nowrap" title="{mib.filePath}">
                {mib.filePath}
              </span>
              <div class="flex gap-2 items-center text-xs">
                {#if mib.isFallback}
                  <span class="badge badge-warning badge-sm">FALLBACK</span>
                {/if}
                <span>{mib.nodeCount} nodes</span>
              </div>
              <button class="btn btn-error btn-xs" on:click={() => unloadMib(mib.mibName)}>
                Unload
              </button>
            </div>
          {/each}
        {/if}
      </div>

      <div class="modal-action">
        <button class="btn btn-primary" on:click={close}>
          Close
        </button>
      </div>
    </div>
  </dialog>
{/if}
