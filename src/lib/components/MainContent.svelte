<script lang="ts">
  import MIBPanel from "./MIBPanel.svelte";
  import SelectionInfo from "./SelectionInfo.svelte";
  import ResultsPane from "./ResultsPane.svelte";
  import { S } from "$lib/stores.svelte";

  const MIN_WIDTH = 200;
  const MAX_WIDTH = 600;

  let isResizing = $state(false);
  let startX = $state(0);
  let startWidth = $state(0);
  let mainContentEl: HTMLDivElement;

  function onResizeStart(e: MouseEvent) {
    e.preventDefault();
    isResizing = true;
    startX = e.clientX;
    startWidth = S.mibPanelWidth;
    document.addEventListener("mousemove", onResizeMove);
    document.addEventListener("mouseup", onResizeEnd);
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
  }

  function onResizeMove(e: MouseEvent) {
    if (!isResizing) return;
    const delta = e.clientX - startX;
    const newWidth = Math.max(MIN_WIDTH, Math.min(MAX_WIDTH, startWidth + delta));
    S.mibPanelWidth = newWidth;
  }

  function onResizeEnd() {
    isResizing = false;
    document.removeEventListener("mousemove", onResizeMove);
    document.removeEventListener("mouseup", onResizeEnd);
    document.body.style.cursor = "";
    document.body.style.userSelect = "";
  }
</script>

<div class="flex flex-1 overflow-hidden" bind:this={mainContentEl}>
  <MIBPanel />
  <div
    class="resize-handle-h w-[6px] cursor-col-resize flex-shrink-0 bg-base-200 hover:bg-primary/30 transition-colors"
    onmousedown={onResizeStart}
    role="separator"
    aria-orientation="vertical"
    tabindex="0"
  >
    <div class="resize-grip-h mx-auto"></div>
  </div>
  <main class="flex flex-col flex-1 overflow-hidden min-w-0">
    <SelectionInfo />
    <ResultsPane />
  </main>
</div>

<style>
.resize-handle-h {
  position: relative;
}
.resize-handle-h:hover,
.resize-handle-h:active {
  background-color: oklch(var(--p) / 0.15);
}
.resize-grip-h {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  width: 2px;
  height: 32px;
  border-radius: 1px;
  background-color: oklch(var(--bc) / 0.2);
}
.resize-handle-h:hover .resize-grip-h {
  background-color: oklch(var(--p) / 0.6);
}
</style>
