<script lang="ts">
  import MenuBar from "./MenuBar.svelte";
  import MainContent from "./MainContent.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import ManageMibsDialog from "./ManageMibsDialog.svelte";
  import { onMount } from "svelte";
  import { statusText, nodeCount, fallbackMibs, treeData } from "$lib/stores";
  import { configRead, mibLoadDirectories, mibTree } from "$lib/tauriCommands";

  onMount(async () => {
    $statusText = "Loading configuration...";
    try {
      const config = await configRead();
      const dirs = config.mib?.directories || [];

      if (dirs.length > 0) {
        $statusText = `Loading MIBs from ${dirs.length} directory(ies)...`;
        const status = await mibLoadDirectories(dirs);
        $nodeCount = status.nodeCount;
        $fallbackMibs = status.fallbackMibs;
      }

      await refreshTree();
      $statusText = "Ready";
    } catch (err) {
      $statusText = `Error: ${err}`;
      console.error("Failed to load MIBs:", err);
    }
  });

  async function refreshTree() {
    try {
      const data = await mibTree();
      $treeData = data;
    } catch (err) {
      console.error("Failed to load tree:", err);
    }
  }
</script>

<div class="flex flex-col h-screen bg-base-00 text-text overflow-hidden">
  <MenuBar />
  <MainContent />
  <ContextMenu />
  <ManageMibsDialog />
  <footer class="flex items-center justify-between px-3 py-1 bg-surface-0 border-t border-base-01 text-overlay text-[11px] flex-shrink-0">
    <span>{$statusText}</span>
    <span>{$nodeCount ? `${$nodeCount} nodes loaded` : ""}</span>
  </footer>
</div>
