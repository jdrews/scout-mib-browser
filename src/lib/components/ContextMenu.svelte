<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { contextMenuTarget, statusText } from "$lib/stores";

  $: target = $contextMenuTarget;
  $: visible = target !== null;
  $: posX = target ? target.x : 0;
  $: posY = target ? target.y : 0;

  function hide() {
    $contextMenuTarget = null;
  }

  async function handleAction(action: string) {
    if (!target) return;
    const node = target.node;
    hide();

    try {
      switch (action) {
        case "copy-oid":
          await navigator.clipboard.writeText(node.oid);
          $statusText = `Copied OID: ${node.oid}`;
          break;
        case "copy-name":
          await navigator.clipboard.writeText(node.name);
          $statusText = `Copied name: ${node.name}`;
          break;
      }
    } catch (err) {
      console.error("Clipboard error:", err);
      $statusText = "Failed to copy";
    }
  }

  const handleClick = () => hide();
  const handleContextMenu = (e: MouseEvent) => {
    if (!(e.target as HTMLElement).closest(".tree-node")) {
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
  <div
    class="fixed bg-base-00 border border-base-01 rounded-lg py-1 min-w-[140px] z-[2000] shadow-lg"
    style="left: {posX}px; top: {posY}px;"
  >
    <div class="px-4 py-1.5 text-[13px] text-subtext-1 cursor-pointer hover:bg-base-01 hover:text-text" on:click={() => handleAction("copy-oid")}>
      Copy OID
    </div>
    <div class="px-4 py-1.5 text-[13px] text-subtext-1 cursor-pointer hover:bg-base-01 hover:text-text" on:click={() => handleAction("copy-name")}>
      Copy Name
    </div>
  </div>
{/if}
