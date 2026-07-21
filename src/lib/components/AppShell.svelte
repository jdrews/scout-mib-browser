<script lang="ts">
  import MenuBar from "./MenuBar.svelte";
  import TargetBar from "./TargetBar.svelte";
  import MainContent from "./MainContent.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import ManageMibsDialog from "./ManageMibsDialog.svelte";
  import ConnectionModal from "./ConnectionModal.svelte";
  import { onMount } from "svelte";
  import { statusText, nodeCount, fallbackMibs, treeData, targetConfig, connectionState } from "$lib/stores";
  import { configRead, mibLoadDirectories, mibTree } from "$lib/tauriCommands";

  $: connState = $connectionState;

  onMount(async () => {
    $statusText = "Loading configuration...";
    try {
      const config = await configRead();

      // Load persisted Target settings.
      if (config.target) {
        const t = config.target;
        $targetConfig = {
          host: t.host || "",
          port: t.port ?? 161,
          version: t.version || "v2c",
          community: t.community || "public",
          v3_username: t.v3_username || "",
          v3_auth_protocol: t.v3_auth_protocol || "none",
          v3_auth_passphrase: t.v3_auth_passphrase || "",
          v3_priv_protocol: t.v3_priv_protocol || "none",
          v3_priv_passphrase: t.v3_priv_passphrase || "",
          v3_security_level: t.v3_security_level || "noAuthNoPrivacy",
        };
      }

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
  <TargetBar />
  <MainContent />
  <ContextMenu />
  <ManageMibsDialog />
  <ConnectionModal />
  <footer class="flex items-center justify-between px-3 py-1 bg-surface-0 border-t border-base-01 text-overlay text-[11px] flex-shrink-0">
    <span>{$statusText}</span>
    <div class="flex items-center gap-3">
      <span class="flex items-center gap-1.5">
        <span class="w-2 h-2 rounded-full inline-block" class:bg-green={connState === "connected"} class:bg-yellow={connState === "connecting"} class:bg-red={connState !== "connected" && connState !== "connecting"}></span>
        {connState === "connected" ? "Connected" : connState === "connecting" ? "Connecting..." : "Disconnected"}
      </span>
      <span>{$nodeCount ? `${$nodeCount} nodes loaded` : ""}</span>
    </div>
  </footer>
</div>
