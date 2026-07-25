<script lang="ts">
  import { S } from "$lib/stores.svelte";
  import { configRead, configWrite, mibLoadDirectories, openDirectory, mibTree } from "$lib/tauriCommands";

  function toggleMenu(e: MouseEvent) {
    e.stopPropagation();
    S.fileMenuOpen = !S.fileMenuOpen;
  }

  async function handleAction(action: string) {
    S.fileMenuOpen = false;
    switch (action) {
      case "add-mib-directory":
        await addMibDirectory();
        break;
      case "manage-mibs":
        S.manageMibsOpen = true;
        break;
    }
  }

  async function addMibDirectory() {
    try {
      const selected = await openDirectory();
      if (!selected) return;

      S.statusText = "Loading MIBs...";
      const config = await configRead();
      const dirs = config.mib?.directories || [];
      if (!dirs.includes(selected)) {
        dirs.push(selected);
        await configWrite("mib.directories", dirs);
      }

      const status = await mibLoadDirectories(dirs);
      S.nodeCount = status.nodeCount;
      S.fallbackMibs.length = 0;
      S.fallbackMibs.push(...status.fallbackMibs);

      const data = await mibTree();
      S.treeData.length = 0;
      S.treeData.push(...data);
      S.statusText = `Loaded ${status.nodeCount} nodes`;
    } catch (err) {
      S.statusText = `Error: ${err}`;
      console.error("Failed to add MIB directory:", err);
    }
  }
</script>

<nav class="flex items-center bg-base-200 border-b border-base-300 px-3 h-[36px] flex-shrink-0 select-none relative">
  <div class="dropdown dropdown-right dropdown-hover">
    <button tabindex="0" role="button" class="btn btn-ghost btn-sm">File</button>
    <ul class="dropdown-content menu bg-base-100 rounded-box w-52 p-2 shadow-lg z-[1000]">
      <li><a onclick={() => handleAction("add-mib-directory")}>Add MIB Directory...</a></li>
      <div class="divider divider-my-1"></div>
      <li><a onclick={() => handleAction("manage-mibs")}>Manage MIBs...</a></li>
    </ul>
  </div>
</nav>
