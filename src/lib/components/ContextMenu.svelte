<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { S } from "$lib/stores.svelte";

  let target = $derived(S.contextMenuTarget);
  let visible = $derived(target !== null);
  let posX = $derived(target ? target.x : 0);
  let posY = $derived(target ? target.y : 0);

  function hide() {
    S.contextMenuTarget = null;
  }

  async function handleAction(action: string) {
    // Read the target exactly once: hide() nulls the store, and a second
    // read of the lazy $derived below would see null.
    const t = target;
    if (!t) return;
    hide();

    // Value targets only offer the hex view — no clipboard involved.
    if (t.kind === "value") {
      if (action === "hex-view") {
        S.hexViewTarget = { oid: t.oid, displayName: t.displayName, value: t.value };
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
    class="fixed menu p-2 bg-base-100 rounded-box w-40 shadow-lg z-[2000]"
    style="left: {posX}px; top: {posY}px;"
  >
    {#if target?.kind === "node"}
      <li><a data-testid="ctx-copy-oid" onclick={() => handleAction("copy-oid")}>Copy OID</a></li>
      <li><a data-testid="ctx-copy-name" onclick={() => handleAction("copy-name")}>Copy Name</a></li>
    {:else if target?.kind === "value"}
      <li><a data-testid="ctx-hex-view" onclick={() => handleAction("hex-view")}>Hex View</a></li>
    {/if}
  </ul>
{/if}
