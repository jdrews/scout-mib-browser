<script lang="ts">
  import MenuBar from "./MenuBar.svelte";
  import TargetBar from "./TargetBar.svelte";
  import MainContent from "./MainContent.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import ManageMibsDialog from "./ManageMibsDialog.svelte";
  import ConnectionModal from "./ConnectionModal.svelte";
  import SystemLogPane from "./SystemLogPane.svelte";
  import { onMount } from "svelte";
  import { S } from "$lib/stores.svelte";

  let connState = $derived(S.connectionState);
  let theme = $derived(S.currentTheme);

  let isLightMode = $state(S.currentTheme === "light");

  function toggleSystemLog() {
    S.systemLogOpen = !S.systemLogOpen;
  }

  function toggleTheme() {
    S.currentTheme = theme === "dark" ? "light" : "dark";
  }

  onMount(async () => {
    document.documentElement.setAttribute("data-theme", theme);
    S.statusText = "Loading configuration...";
    try {
      const config = await (await import("$lib/tauriCommands")).configRead();

      if (config.target) {
        const t = config.target;
        Object.assign(S.targetConfig, {
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
        });
      }

      const dirs = config.mib?.directories || [];

      if (dirs.length > 0) {
        S.statusText = `Loading MIBs from ${dirs.length} directory(ies)...`;
        const cmds = await import("$lib/tauriCommands");
        const status = await cmds.mibLoadDirectories(dirs);
        S.nodeCount = status.nodeCount;
        S.fallbackMibs.length = 0;
        S.fallbackMibs.push(...status.fallbackMibs);

        // Refresh OID→name map for frontend resolution.
        try {
          const pairs = await cmds.mibOidNameMap();
          S.oidNameMap = new Map(pairs);
        } catch (e) {
          console.error("Failed to load OID name map:", e);
        }
      }

      await refreshTree();
      S.statusText = "Ready";
    } catch (err) {
      S.statusText = `Error: ${err}`;
      console.error("Failed to load MIBs:", err);
    }
  });

  async function refreshTree() {
    try {
      const { mibTree } = await import("$lib/tauriCommands");
      S.treeData = await mibTree();
    } catch (err) {
      console.error("Failed to load tree:", err);
    }
  }
</script>

<div class="flex flex-col h-screen bg-base-100 text-base-content overflow-hidden" data-theme={S.currentTheme}>
  <MenuBar />
  <TargetBar />
  <div class="flex flex-col flex-1 overflow-hidden min-h-0">
    <MainContent />
    {#if S.systemLogOpen}
      <SystemLogPane />
    {/if}
  </div>
  <ContextMenu />
  <ManageMibsDialog />
  <ConnectionModal />
  <footer class="footer footer-horizontal items-center bg-base-200 border-t border-base-300 text-base-content/60 text-xs flex-shrink-0 px-4 py-2">
    <aside class="flex items-center gap-3">
      <span>{S.statusText}</span>
    </aside>
    <aside class="flex items-center gap-3 ml-auto">
      <button
        class="btn btn-xs normal-case bg-base-300 text-base-content/60 border-base-300 hover:text-base-content hover:bg-base-200"
        onclick={toggleSystemLog}
      >
        System Log
      </button>

      <label class="swap swap-flip btn btn-ghost btn-circle btn-sm">
        <input type="checkbox" bind:checked={isLightMode} onchange={() => { S.currentTheme = isLightMode ? "light" : "dark"; }} />
        <svg class="swap-on fill-current w-4 h-4" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M5.64,17l-.71.71a1,1,0,0,0,0,1.41,1,1,0,0,0,1.41,0l.71-.71A1,1,0,0,0,5.64,17ZM5,12a1,1,0,0,0-1-1H3a1,1,0,0,0,0,2H4A1,1,0,0,0,5,12Zm7-7a1,1,0,0,0,1-1V3a1,1,0,0,0-2,0V4A1,1,0,0,0,12,5ZM5.64,7.05a1,1,0,0,0,.7.29,1,1,0,0,0,.71-.29,1,1,0,0,0,0-1.41l-.71-.71A1,1,0,0,0,4.93,6.34Zm12,.29a1,1,0,0,0,.7-.29l.71-.71a1,1,0,1,0-1.41-1.41L17,5.64a1,1,0,0,0,0,1.41A1,1,0,0,0,17.66,7.34ZM21,11H20a1,1,0,0,0,0,2h1a1,1,0,0,0,0-2Zm-9,8a1,1,0,0,0-1,1v1a1,1,0,0,0,2,0V20A1,1,0,0,0,12,19ZM18.36,17A1,1,0,0,0,17,17.05a1,1,0,0,0,0,1.41l.71.71a1,1,0,0,0,1.41,0,1,1,0,0,0,0-1.41ZM12,6.5A5.5,5.5,0,1,0,17.5,12,5.5,5.5,0,0,0,12,6.5Zm0,9A3.5,3.5,0,1,1,15.5,12,3.5,3.5,0,0,1,12,15.5Z"/></svg>
        <svg class="swap-off fill-current w-4 h-4" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M21.64,13a1,1,0,0,0-1.05-.14,8.05,8.05,0,0,1-3.37.73A8.15,8.15,0,0,1,9.08,5.49a8.59,8.59,0,0,1,.25-2A1,1,0,0,0,8,2.36,10.14,10.14,0,1,0,22,14.05,1,1,0,0,0,21.64,13Z"/></svg>
      </label>

      <span class="flex items-center gap-1.5">
        <span class="w-2 h-2 rounded-full inline-block" class:bg-success={connState === "connected"} class:bg-warning={connState === "connecting"} class:bg-error={connState !== "connected" && connState !== "connecting"}></span>
        {connState === "connected" ? "Connected" : connState === "connecting" ? "Connecting..." : "Disconnected"}
      </span>
      <span>{S.nodeCount ? `${S.nodeCount} nodes loaded` : ""}</span>
    </aside>
  </footer>
</div>
