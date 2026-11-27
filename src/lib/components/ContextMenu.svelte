<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { S, requestTreeReveal } from "$lib/stores.svelte";
  import { mibNodeDetails } from "$lib/tauriCommands";
  import * as exportMod from "$lib/export";
  import { setDetailsFor } from "$lib/setLogic";
  import type { MibNodeDetails, SnmpValue } from "$lib/types";

  let target = $derived(S.contextMenuTarget);
  let visible = $derived(target !== null);
  let posX = $derived(target ? target.x : 0);
  let posY = $derived(target ? target.y : 0);

  function hide() {
    S.contextMenuTarget = null;
  }

  /** Opens the Set value dialog for an instance OID (value and row targets). */
  async function openSetDialog(oid: string, displayName: string, value: SnmpValue) {
    // Longest-prefix resolution maps instance OIDs to their base node;
    // null (or an ancestor-subtree resolution) lands the dialog in its
    // type-picker fallback.
    let d: MibNodeDetails | null = null;
    try {
      d = setDetailsFor(oid, await mibNodeDetails(oid));
    } catch (err) {
      console.error("Set node lookup failed:", err);
    }
    S.setValueTarget = {
      oid,
      name: displayName,
      details: d,
      currentValue: exportMod.valueDisplay(value),
      currentRaw: value,
    };
  }

  async function handleAction(action: string) {
    // Read the target exactly once: hide() nulls the store, and a second
    // read of the lazy $derived below would see null.
    const t = target;
    if (!t) return;
    hide();

    // Value targets offer the hex view and/or the Set value dialog — no
    // clipboard involved.
    if (t.kind === "value") {
      if (action === "hex-view") {
        S.hexViewTarget = { oid: t.oid, displayName: t.displayName, value: t.value };
      } else if (action === "set-value") {
        await openSetDialog(t.oid, t.displayName, t.value);
      }
      return;
    }

    // Row targets (results rows) combine the node clipboard actions with the
    // value actions and the reveal.
    if (t.kind === "row") {
      try {
        switch (action) {
          case "copy-oid":
            await navigator.clipboard.writeText(t.oid);
            S.statusText = `Copied OID: ${t.oid}`;
            break;
          case "copy-name":
            await navigator.clipboard.writeText(t.displayName);
            S.statusText = `Copied name: ${t.displayName}`;
            break;
          case "reveal":
            if (t.baseOid) requestTreeReveal(t.baseOid);
            else S.statusText = "Not in any loaded MIB";
            break;
          case "hex-view":
            if (t.value) S.hexViewTarget = { oid: t.oid, displayName: t.displayName, value: t.value };
            break;
          case "set-value":
            if (t.value) await openSetDialog(t.oid, t.displayName, t.value);
            break;
        }
      } catch (err) {
        console.error("Clipboard error:", err);
        S.statusText = "Failed to copy";
      }
      return;
    }

    const node = t.node;
    try {
      switch (action) {
        case "copy-oid":
          await navigator.clipboard.writeText(node.oid);
          S.statusText = `Copied OID: ${node.oid}`;
          break;
        case "copy-name":
          await navigator.clipboard.writeText(node.name);
          S.statusText = `Copied name: ${node.name}`;
          break;
      }
    } catch (err) {
      console.error("Clipboard error:", err);
      S.statusText = "Failed to copy";
    }
  }

  const handleClick = () => hide();
  const handleContextMenu = (e: MouseEvent) => {
    if (!(e.target as HTMLElement).closest("[data-tree-node]")) {
      hide();
    }
  };

  onMount(() => {
    document.addEventListener("click", handleClick);
    document.addEventListener("contextmenu", handleContextMenu);
  });

  onDestroy(() => {
    document.removeEventListener("click", handleClick);
    document.removeEventListener("contextmenu", handleContextMenu);
  });
</script>

{#if visible}
  <ul
    class="menu-in fixed menu p-2 bg-base-100 rounded-box w-48 shadow-lg z-[2000]"
    style="left: {posX}px; top: {posY}px;"
  >
    {#if target?.kind === "node"}
      <li><a data-testid="ctx-copy-oid" onclick={() => handleAction("copy-oid")}>Copy OID</a></li>
      <li><a data-testid="ctx-copy-name" onclick={() => handleAction("copy-name")}>Copy Name</a></li>
    {:else if target?.kind === "value"}
      {#if target.writable !== false}
        <li><a data-testid="ctx-set-value" onclick={() => handleAction("set-value")}>Set value…</a></li>
      {/if}
      {#if exportMod.isByteValue(target.value)}
        <li><a data-testid="ctx-hex-view" onclick={() => handleAction("hex-view")}>Hex View</a></li>
      {/if}
    {:else if target?.kind === "row"}
      {#if target.baseOid}
        <li><a data-testid="ctx-reveal" onclick={() => handleAction("reveal")}>Reveal in MIB Browser</a></li>
      {/if}
      <li><a data-testid="ctx-copy-oid" onclick={() => handleAction("copy-oid")}>Copy OID</a></li>
      <li><a data-testid="ctx-copy-name" onclick={() => handleAction("copy-name")}>Copy Name</a></li>
      {#if target.value && target.writable !== false}
        <li><a data-testid="ctx-set-value" onclick={() => handleAction("set-value")}>Set value…</a></li>
      {/if}
      {#if target.value && exportMod.isByteValue(target.value)}
        <li><a data-testid="ctx-hex-view" onclick={() => handleAction("hex-view")}>Hex View</a></li>
      {/if}
    {/if}
  </ul>
{/if}
