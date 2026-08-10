<script lang="ts">
  import { S } from "$lib/stores.svelte";
  import { configRead, configWrite, mibLoadDirectories, openDirectory, mibTree, mibOidNameMap } from "$lib/tauriCommands";

  function closeAllMenus() {
    S.fileMenuOpen = false;
    S.settingsMenuOpen = false;
    S.viewMenuOpen = false;
  }

  function toggleFileMenu(e: MouseEvent) {
    e.stopPropagation();
    if (S.fileMenuOpen) {
      closeAllMenus();
    } else {
      closeAllMenus();
      S.fileMenuOpen = true;
    }
  }

  function toggleSettingsMenu(e: MouseEvent) {
    e.stopPropagation();
    if (S.settingsMenuOpen) {
      closeAllMenus();
    } else {
      closeAllMenus();
      S.settingsMenuOpen = true;
    }
  }

  function toggleViewMenu(e: MouseEvent) {
    e.stopPropagation();
    if (S.viewMenuOpen) {
      closeAllMenus();
    } else {
      closeAllMenus();
      S.viewMenuOpen = true;
    }
  }

  async function handleAction(action: string) {
    closeAllMenus();
    switch (action) {
      case "add-mib-directory":
        await addMibDirectory();
        break;
      case "manage-mibs":
        S.manageMibsOpen = true;
        break;
      case "connection":
        S.connectionPanelOpen = true;
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

      try {
        const pairs = await mibOidNameMap();
        S.oidNameMap = new Map(pairs);
      } catch (e) {
        console.error("Failed to load OID name map:", e);
      }

      const data = await mibTree();
      S.treeData.length = 0;
      S.treeData.push(...data);
      S.statusText = `Loaded ${status.nodeCount} nodes`;
    } catch (err) {
      S.statusText = `Error: ${err}`;
      console.error("Failed to add MIB directory:", err);
    }
  }

  $effect(() => {
    return () => {
      closeAllMenus();
    };
  });
</script>

<nav class="flex items-center bg-base-200 border-b border-base-300 px-3 h-[36px] flex-shrink-0 select-none relative" onclick={closeAllMenus}>
  <div class="relative">
    <button tabindex="0" role="button" class="btn btn-ghost btn-sm" onclick={toggleFileMenu}>File</button>
    {#if S.fileMenuOpen}
      <ul class="absolute top-full left-0 menu bg-base-100 rounded-box w-52 p-2 shadow-lg z-[1000] mt-1">
        <li><a onclick={() => handleAction("add-mib-directory")}>Add MIB Directory...</a></li>
        <div class="divider divider-my-1"></div>
        <li><a onclick={() => handleAction("manage-mibs")}>Manage MIBs...</a></li>
      </ul>
    {/if}
  </div>

  <div class="relative">
    <button tabindex="0" role="button" class="btn btn-ghost btn-sm" onclick={toggleViewMenu}>View</button>
    {#if S.viewMenuOpen}
      <ul class="absolute top-full left-0 menu bg-base-100 rounded-box w-52 p-2 shadow-lg z-[1000] mt-1">
        <li>
          <a onclick={(e) => { e.preventDefault(); e.stopPropagation(); S.systemLogOpen = !S.systemLogOpen; }}>
            <span class="flex items-center gap-2">
              {#if S.systemLogOpen}
                <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><polyline points="20 6 9 17 4 12"/></svg>
              {/if}
              <span class={S.systemLogOpen ? "text-primary" : ""}>System Log</span>
            </span>
          </a>
        </li>
      </ul>
    {/if}
  </div>

  <div class="relative">
    <button tabindex="0" role="button" class="btn btn-ghost btn-sm" onclick={toggleSettingsMenu}>Settings</button>
    {#if S.settingsMenuOpen}
      <ul class="absolute top-full left-0 menu bg-base-100 rounded-box w-52 p-2 shadow-lg z-[1000] mt-1">
        <li><a onclick={() => handleAction("connection")}>Connection...</a></li>
        <div class="divider divider-my-1"></div>
        <li>
          <details open>
            <summary>System Log Level</summary>
            <ul class="menu menu-vertical w-full p-0">
              <li><a class={S.logLevelFilter === "all" ? "active" : ""} onclick={() => { S.logLevelFilter = "all"; }}>{S.logLevelFilter === "all" ? "✓" : ""} All</a></li>
              <li><a class={S.logLevelFilter === "info" ? "active" : ""} onclick={() => { S.logLevelFilter = "info"; }}>{S.logLevelFilter === "info" ? "✓" : ""} Info+</a></li>
              <li><a class={S.logLevelFilter === "warn" ? "active" : ""} onclick={() => { S.logLevelFilter = "warn"; }}>{S.logLevelFilter === "warn" ? "✓" : ""} Warning+</a></li>
              <li><a class={S.logLevelFilter === "error" ? "active" : ""} onclick={() => { S.logLevelFilter = "error"; }}>{S.logLevelFilter === "error" ? "✓" : ""} Error</a></li>
            </ul>
          </details>
        </li>
      </ul>
    {/if}
  </div>
</nav>
