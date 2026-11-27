<script lang="ts">
  import { Moon, PanelBottom, PanelBottomClose, Sun } from "lucide-svelte";
  import MenuBar from "./MenuBar.svelte";
  import TargetBar from "./TargetBar.svelte";
  import MainContent from "./MainContent.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import HexViewModal from "./HexViewModal.svelte";
  import SetValueDialog from "./SetValueDialog.svelte";
  import ConnectionModal from "./ConnectionModal.svelte";
  import SystemLogPane from "./SystemLogPane.svelte";
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { S } from "$lib/stores.svelte";
  import { pluralize } from "$lib/format";
  import { refreshMibState } from "$lib/mibManager";
  import { configRead, mibLoad, logAppend } from "$lib/tauriCommands";

  let connState = $derived(S.connectionState);
  let theme = $derived(S.currentTheme);
  // Beta feedback triage (launch-beta/12): every bug report needs a build
  // to point at — the footer shows the version, startup records it in the
  // System Log with the platform.
  let appVersion = $state("");

  async function announceBuild() {
    try {
      // The webview's platform string is the triage detail ("X11; Linux
      // x86_64"); @tauri-apps/api has no platform command in v2.
      const [version, platform] = await Promise.all([getVersion(), Promise.resolve(navigator.platform)]);
      appVersion = version;
      await logAppend("INFO", "app", `Scout MIB Browser ${version} on ${platform}`);
    } catch {
      // Outside the packaged app (plain browser dev) there is no app
      // plugin: no version display, never a startup failure.
    }
  }

  onMount(async () => {
    void announceBuild();
    document.documentElement.setAttribute("data-theme", theme);
    S.statusText = "Loading configuration…";
    try {
      const config = await configRead();

      if (config.target) {
        const t = config.target;
        Object.assign(S.targetConfig, {
          host: t.host || "",
          port: t.port ?? 161,
          version: t.version || "v2c",
          community: t.community || "public",
          timeout_secs: t.timeout_secs,
          v3_username: t.v3_username || "",
          v3_auth_protocol: t.v3_auth_protocol || "none",
          v3_auth_passphrase: t.v3_auth_passphrase || "",
          v3_priv_protocol: t.v3_priv_protocol || "none",
          v3_priv_passphrase: t.v3_priv_passphrase || "",
          v3_security_level: t.v3_security_level || "noAuthNoPrivacy",
        });
      }

      S.saveCredentials = config.ui?.save_credentials ?? false;

      // Startup loads directories + explicit files (the full configured set).
      const dirs = config.mib?.directories || [];
      const files = config.mib?.files || [];

      if (dirs.length > 0 || files.length > 0) {
        S.statusText = `Loading MIBs from ${dirs.length} directory(ies)...`;
        const status = await mibLoad(dirs, files);
        await refreshMibState(status);
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
      S.treeError = "";
    } catch (err) {
      // Distinct error state: an empty tree after a failure must not read
      // as "no MIBs loaded" (launch-beta/10).
      S.treeError = String(err);
      S.statusText = `Failed to load MIB tree: ${err}`;
      console.error("Failed to load tree:", err);
    }
  }
</script>

<div class="flex flex-col h-screen bg-base-100 text-base-content overflow-hidden" data-theme={S.currentTheme}>
  <MenuBar />
  <!-- The banner landmark holds the app title and the query bar: both sat
       outside every landmark (axe region, launch-beta/13). -->
  <header class="flex flex-col">
    <h1 class="sr-only">Scout MIB Browser</h1>
    <TargetBar />
  </header>
  <div class="flex flex-col flex-1 overflow-hidden min-h-0">
    <MainContent />
    {#if S.systemLogOpen}
      <SystemLogPane />
    {/if}
  </div>
  <ContextMenu />
  <HexViewModal />
  <SetValueDialog />
  <ConnectionModal />
  <footer class="footer footer-horizontal items-center bg-base-200 border-t border-base-300 text-base-content/60 text-xs tabular-nums flex-shrink-0 px-4 py-2">
    <div class="flex items-center gap-3">
      <span data-testid="status-text" role="status">{S.statusText}</span>
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
        <span class="w-2 h-2 rounded-full inline-block {connState === 'connected' ? 'bg-success' : connState === 'connecting' ? 'bg-warning led-pulse' : connState === 'disconnected' ? 'bg-error' : 'bg-base-content/30'}"></span>
        {connState === "connected" ? "Connected" : connState === "connecting" ? "Connecting…" : connState === "disconnected" ? "Disconnected" : "Not connected"}
      </span>
      <span data-testid="node-count">{S.nodeCount ? `${pluralize(S.nodeCount, "node")} loaded` : ""}</span>
      {#if appVersion}
        <span data-testid="app-version" title="App version">v{appVersion}</span>
      {/if}
    </div>
  </footer>
</div>
