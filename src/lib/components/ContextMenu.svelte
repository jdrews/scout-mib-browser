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
    if (!target) return;
    const node = target.node;
    hide();

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
    <li><a onclick={() => handleAction("copy-oid")}>Copy OID</a></li>
    <li><a onclick={() => handleAction("copy-name")}>Copy Name</a></li>
  </ul>
{/if}
