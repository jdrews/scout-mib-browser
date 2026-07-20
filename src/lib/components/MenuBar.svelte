<script lang="ts">
  import { fileMenuOpen, statusText, nodeCount, fallbackMibs, treeData, manageMibsOpen } from "$lib/stores";
  import { configRead, configWrite, mibLoadDirectories, openDirectory, mibTree } from "$lib/tauriCommands";

  function toggleMenu(e: MouseEvent) {
    e.stopPropagation();
    $fileMenuOpen = !$fileMenuOpen;
  }

  async function handleAction(action: string) {
    $fileMenuOpen = false;
    switch (action) {
      case "add-mib-directory":
        await addMibDirectory();
        break;
      case "manage-mibs":
        $manageMibsOpen = true;
        break;
    }
  }

  async function addMibDirectory() {
    try {
      const selected = await openDirectory();
      if (!selected) return;

      $statusText = "Loading MIBs...";
      const config = await configRead();
      const dirs = config.mib?.directories || [];
      if (!dirs.includes(selected)) {
        dirs.push(selected);
        await configWrite("mib.directories", dirs);
      }

      const status = await mibLoadDirectories(dirs);
      $nodeCount = status.nodeCount;
      $fallbackMibs = status.fallbackMibs;

      const data = await mibTree();
      $treeData = data;
      $statusText = `Loaded ${status.nodeCount} nodes`;
    } catch (err) {
      $statusText = `Error: ${err}`;
      console.error("Failed to add MIB directory:", err);
    }
  }
</script>

<nav class="flex items-center bg-surface-0 border-b border-base-01 px-2 h-[28px] flex-shrink-0 select-none relative">
  <div
    class="px-3 py-1 text-[13px] text-subtext-1 cursor-pointer rounded hover:bg-base-01 hover:text-text"
    class:active={$fileMenuOpen}
    on:click={toggleMenu}
  >
    File
  </div>

  {#if $fileMenuOpen}
    <div class="absolute top-[28px] left-0 bg-base-00 border border-base-01 rounded-lg py-1 min-w-[200px] z-[1000] shadow-lg">
      <div class="px-4 py-1.5 text-[13px] text-subtext-1 cursor-pointer hover:bg-base-01 hover:text-text" on:click={() => handleAction("add-mib-directory")}>
        Add MIB Directory...
      </div>
      <div class="h-px bg-base-01 my-1"></div>
      <div class="px-4 py-1.5 text-[13px] text-subtext-1 cursor-pointer hover:bg-base-01 hover:text-text" on:click={() => handleAction("manage-mibs")}>
        Manage MIBs...
      </div>
    </div>
  {/if}
</nav>
