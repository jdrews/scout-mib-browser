<script lang="ts">
  import { Moon, PanelBottom, PanelBottomClose, Sun } from "lucide-svelte";
  import MenuBar from "./MenuBar.svelte";
  import TargetBar from "./TargetBar.svelte";
  import MainContent from "./MainContent.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import ManageMibsDialog from "./ManageMibsDialog.svelte";
  import ConnectionModal from "./ConnectionModal.svelte";
  import SystemLogPane from "./SystemLogPane.svelte";
  import { onMount } from "svelte";
  import { S } from "$lib/stores.svelte";
  import { pluralize } from "$lib/format";

  let connState = $derived(S.connectionState);
  let theme = $derived(S.currentTheme);

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

      S.saveCredentials = config.ui?.save_credentials ?? true;

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
      S.treeVersion++;
    } catch (err) {
      console.error("Failed to load tree:", err);
    }
  }
</script>

<div class="flex flex-col h-screen bg-base-100 text-base-content overflow-hidden" data-theme={S.currentTheme}>
  <h1 class="sr-only">Scout MIB Browser</h1>
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
    <div class="flex items-center gap-3">
      <span data-testid="status-text">{S.statusText}</span>
    </div>
    <div class="flex items-center gap-3 ml-auto">
      <button
        data-testid="syslog-toggle"
        aria-label={S.systemLogOpen ? "Close system log" : "Open system log"}
        aria-pressed={S.systemLogOpen}
        title={S.systemLogOpen ? "Close system log" : "Open system log"}
        class="btn btn-ghost btn-circle btn-sm {S.systemLogOpen ? 'btn-active' : ''}"
        onclick={() => (S.systemLogOpen = !S.systemLogOpen)}
      >
        {#if S.systemLogOpen}
          <PanelBottomClose class="w-4 h-4" />
        {:else}
          <PanelBottom class="w-4 h-4" />
        {/if}
      </button>

      <button data-testid="theme-toggle" aria-label={S.currentTheme === "dark" ? "Switch to light mode" : "Switch to dark mode"} class="btn btn-ghost btn-circle btn-sm" onclick={() => S.currentTheme = S.currentTheme === "dark" ? "light" : "dark"} title={S.currentTheme === "dark" ? "Switch to light mode" : "Switch to dark mode"}>
        {#if S.currentTheme === "dark"}
          <Sun class="w-4 h-4" />
        {:else}
          <Moon class="w-4 h-4" />
        {/if}
      </button>

      <span data-testid="conn-indicator" class="flex items-center gap-1.5">
        <!-- Neutral until an attempt is made; red only after a real failure (UX-12). -->
        <span class="w-2 h-2 rounded-full inline-block {connState === 'connected' ? 'bg-success' : connState === 'connecting' ? 'bg-warning' : connState === 'disconnected' ? 'bg-error' : 'bg-base-content/30'}"></span>
        {connState === "connected" ? "Connected" : connState === "connecting" ? "Connecting..." : connState === "disconnected" ? "Disconnected" : "Not connected"}
      </span>
      <span data-testid="node-count">{S.nodeCount ? `${pluralize(S.nodeCount, "node")} loaded` : ""}</span>
    </div>
  </footer>
</div>
