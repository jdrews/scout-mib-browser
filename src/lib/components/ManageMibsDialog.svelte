<script lang="ts">
  import { manageMibsOpen } from "$lib/stores";
  import { mibLoadedList, mibUnload, mibTree } from "$lib/tauriCommands";
  import { statusText, nodeCount, fallbackMibs, treeData } from "$lib/stores";
  import type { LoadedMib } from "$lib/types";

  let mibs: LoadedMib[] = [];
  let loading = false;

  async function openDialog() {
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
    $manageMibsOpen = false;
  }

  $: if ($manageMibsOpen && mibs.length === 0) {
    openDialog();
  }
</script>

{#if $manageMibsOpen}
  <div class="fixed inset-0 bg-black/60 flex items-center justify-center z-[3000]" on:click={close}>
    <div class="bg-base-00 border border-base-01 rounded-lg w-[560px] max-h-[70vh] flex flex-col shadow-xl" on:click|stopPropagation>
      <div class="flex items-center justify-between px-4 py-3 border-b border-base-01">
        <h2 class="text-sm font-semibold">Manage MIBs</h2>
        <button class="bg-transparent border-none text-overlay text-lg cursor-pointer leading-none hover:text-red" on:click={close}>&times;</button>
      </div>

      <div class="flex-1 overflow-y-auto p-2">
        {#if loading}
          <p class="text-overlay text-[13px] text-center mt-8">Loading...</p>
        {:else if mibs.length === 0}
          <p class="text-overlay text-[13px] text-center mt-8">No MIBs currently loaded.</p>
        {:else}
          {#each mibs as mib (mib.mibName)}
            <div class="flex items-center px-3 py-2 rounded gap-3 hover:bg-base-01">
              <span class="flex-1 text-[13px] text-text">{mib.mibName}</span>
              <span class="text-[11px] text-overlay font-mono max-w-[240px] overflow-hidden text-ellipsis whitespace-nowrap" title="{mib.filePath}">
                {mib.filePath}
              </span>
              <div class="flex gap-2 items-center text-[11px]">
                {#if mib.isFallback}
                  <span class="bg-base-02 text-yellow px-1.5 py-0.5 rounded-[3px]">FALLBACK</span>
                {/if}
                <span>{mib.nodeCount} nodes</span>
              </div>
              <button class="bg-transparent border border-base-02 text-red px-2 py-0.5 text-[11px] rounded cursor-pointer hover:bg-red hover:text-base-00" on:click={() => unloadMib(mib.mibName)}>
                Unload
              </button>
            </div>
          {/each}
        {/if}
      </div>

      <div class="px-4 py-2 border-t border-base-01 flex justify-end">
        <button class="bg-blue text-base-00 border-none px-4 py-1.5 text-[13px] font-semibold rounded cursor-pointer hover:bg-sapphire" on:click={close}>
          Close
        </button>
      </div>
    </div>
  </div>
{/if}
