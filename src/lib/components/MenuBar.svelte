<script lang="ts">
  import { tick } from "svelte";
  import { Check } from "lucide-svelte";
  import { S } from "$lib/stores.svelte";
  import { configRead, configWrite, mibLoadDirectories, openDirectory, mibTree, mibOidNameMap } from "$lib/tauriCommands";

  type MenuName = "file" | "view" | "settings";

  const menuOpenKey: Record<MenuName, "fileMenuOpen" | "viewMenuOpen" | "settingsMenuOpen"> = {
    file: "fileMenuOpen",
    view: "viewMenuOpen",
    settings: "settingsMenuOpen",
  };

  function closeAllMenus() {
    S.fileMenuOpen = false;
    S.settingsMenuOpen = false;
    S.viewMenuOpen = false;
  }

  function isMenuOpen(name: MenuName): boolean {
    return S[menuOpenKey[name]];
  }

  async function openMenu(name: MenuName, focusFirstItem: boolean) {
    closeAllMenus();
    S[menuOpenKey[name]] = true;
    if (focusFirstItem) {
      await tick();
      const first = document.querySelector(`[data-menu="${name}"] [role='menuitem']`);
      if (first instanceof HTMLElement) first.focus();
    }
  }

  function toggleFileMenu(e: MouseEvent) {
    e.stopPropagation();
    if (S.fileMenuOpen) closeAllMenus();
    else openMenu("file", false);
  }

  function toggleSettingsMenu(e: MouseEvent) {
    e.stopPropagation();
    if (S.settingsMenuOpen) closeAllMenus();
    else openMenu("settings", false);
  }

  function toggleViewMenu(e: MouseEvent) {
    e.stopPropagation();
    if (S.viewMenuOpen) closeAllMenus();
    else openMenu("view", false);
  }

  /** Top-level trigger: ArrowDown opens the menu and moves focus into it. */
  async function onTriggerKeydown(e: KeyboardEvent, name: MenuName) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      await openMenu(name, true);
    }
  }

  /** Menu items: arrow-key navigation, Escape closes and returns focus to the trigger. */
  function onItemKeydown(e: KeyboardEvent, name: MenuName) {
    const items = Array.from(
      document.querySelectorAll(`[data-menu="${name}"] [role='menuitem']`),
    ) as HTMLElement[];
    const idx = items.indexOf(e.currentTarget as HTMLElement);
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        items[(idx + 1) % items.length].focus();
        break;
      case "ArrowUp":
        e.preventDefault();
        items[(idx - 1 + items.length) % items.length].focus();
        break;
      case "Home":
        e.preventDefault();
        items[0].focus();
        break;
      case "End":
        e.preventDefault();
        items[items.length - 1].focus();
        break;
      case "Escape":
        e.preventDefault();
        closeAllMenus();
        (document.querySelector(`[data-testid="menu-${name}"]`) as HTMLElement | null)?.focus();
        break;
      case "Tab":
        // Close the menu and let Tab continue to the next page control.
        closeAllMenus();
        break;
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
      S.treeVersion++;
      S.statusText = status.filesCached !== undefined
        ? `Loaded ${status.nodeCount} nodes (${status.filesParsed} parsed, ${status.filesCached} cached)`
        : `Loaded ${status.nodeCount} nodes`;
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

<nav aria-label="Application menu" class="flex items-center bg-base-200 border-b border-base-300 px-3 h-[36px] flex-shrink-0 select-none relative" onclick={closeAllMenus}>
  <div class="relative" data-menu="file">
    <button data-testid="menu-file" aria-haspopup="menu" aria-expanded={S.fileMenuOpen} class="btn btn-ghost btn-sm" onclick={toggleFileMenu} onkeydown={(e) => onTriggerKeydown(e, "file")}>File</button>
    {#if S.fileMenuOpen}
      <ul role="menu" aria-label="File" class="absolute top-full left-0 menu bg-base-100 rounded-box w-52 p-2 shadow-lg z-[1000] mt-1">
        <li><a role="menuitem" tabindex="-1" data-testid="menu-add-mib-dir" onkeydown={(e) => onItemKeydown(e, "file")} onclick={() => handleAction("add-mib-directory")}>Add MIB Directory...</a></li>
        <li role="separator" aria-orientation="horizontal" aria-hidden="true" class="my-1 h-px bg-base-300"></li>
        <li><a role="menuitem" tabindex="-1" data-testid="menu-manage-mibs" onkeydown={(e) => onItemKeydown(e, "file")} onclick={() => handleAction("manage-mibs")}>Manage MIBs...</a></li>
      </ul>
    {/if}
  </div>

  <div class="relative" data-menu="view">
    <button data-testid="menu-view" aria-haspopup="menu" aria-expanded={S.viewMenuOpen} class="btn btn-ghost btn-sm" onclick={toggleViewMenu} onkeydown={(e) => onTriggerKeydown(e, "view")}>View</button>
    {#if S.viewMenuOpen}
      <ul role="menu" aria-label="View" class="absolute top-full left-0 menu bg-base-100 rounded-box w-52 p-2 shadow-lg z-[1000] mt-1">
        <li>
          <a role="menuitem" tabindex="-1" data-testid="menu-system-log" onkeydown={(e) => onItemKeydown(e, "view")} onclick={(e) => { e.preventDefault(); e.stopPropagation(); S.systemLogOpen = !S.systemLogOpen; }}>
            <span class="flex items-center gap-2">
              {#if S.systemLogOpen}
                <Check class="w-4 h-4 shrink-0" />
              {/if}
              <span class={S.systemLogOpen ? "text-primary" : ""}>System Log</span>
            </span>
          </a>
        </li>
      </ul>
    {/if}
  </div>

  <div class="relative" data-menu="settings">
    <button data-testid="menu-settings" aria-haspopup="menu" aria-expanded={S.settingsMenuOpen} class="btn btn-ghost btn-sm" onclick={toggleSettingsMenu} onkeydown={(e) => onTriggerKeydown(e, "settings")}>Settings</button>
    {#if S.settingsMenuOpen}
      <ul role="menu" aria-label="Settings" class="absolute top-full left-0 menu bg-base-100 rounded-box w-52 p-2 shadow-lg z-[1000] mt-1">
        <li><a role="menuitem" tabindex="-1" data-testid="menu-connection" onkeydown={(e) => onItemKeydown(e, "settings")} onclick={() => handleAction("connection")}>Connection...</a></li>
        <li role="separator" aria-orientation="horizontal" aria-hidden="true" class="my-1 h-px bg-base-300"></li>
        <li><span class="px-3 py-1 text-xs font-semibold uppercase tracking-wide text-base-content/60">System Log Level</span></li>
        <li><a role="menuitem" tabindex="-1" data-testid="log-level-all" class={S.logLevelFilter === "all" ? "active" : ""} onkeydown={(e) => onItemKeydown(e, "settings")} onclick={() => { S.logLevelFilter = "all"; }}><span class="flex items-center gap-2">{#if S.logLevelFilter === "all"}<Check class="w-3.5 h-3.5 shrink-0" />{/if}<span>All</span></span></a></li>
        <li><a role="menuitem" tabindex="-1" data-testid="log-level-info" class={S.logLevelFilter === "info" ? "active" : ""} onkeydown={(e) => onItemKeydown(e, "settings")} onclick={() => { S.logLevelFilter = "info"; }}><span class="flex items-center gap-2">{#if S.logLevelFilter === "info"}<Check class="w-3.5 h-3.5 shrink-0" />{/if}<span>Info+</span></span></a></li>
        <li><a role="menuitem" tabindex="-1" data-testid="log-level-warn" class={S.logLevelFilter === "warn" ? "active" : ""} onkeydown={(e) => onItemKeydown(e, "settings")} onclick={() => { S.logLevelFilter = "warn"; }}><span class="flex items-center gap-2">{#if S.logLevelFilter === "warn"}<Check class="w-3.5 h-3.5 shrink-0" />{/if}<span>Warning+</span></span></a></li>
        <li><a role="menuitem" tabindex="-1" data-testid="log-level-error" class={S.logLevelFilter === "error" ? "active" : ""} onkeydown={(e) => onItemKeydown(e, "settings")} onclick={() => { S.logLevelFilter = "error"; }}><span class="flex items-center gap-2">{#if S.logLevelFilter === "error"}<Check class="w-3.5 h-3.5 shrink-0" />{/if}<span>Error</span></span></a></li>
      </ul>
    {/if}
  </div>
</nav>
