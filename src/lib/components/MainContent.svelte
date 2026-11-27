<script lang="ts">
  import MIBPanel from "./MIBPanel.svelte";
  import ResultsPane from "./ResultsPane.svelte";
  import ManageMibsPage from "./ManageMibsPage.svelte";
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

  // Keyboard resize for the focusable handle (launch-beta/13): the handle is
  // the panel's right edge, so ArrowRight grows — same direction as dragging.
  const KEY_RESIZE_STEP = 16;
  function onResizeKeydown(e: KeyboardEvent) {
    if (e.key === "ArrowRight") {
      e.preventDefault();
      S.mibPanelWidth = Math.min(MAX_WIDTH, S.mibPanelWidth + KEY_RESIZE_STEP);
    } else if (e.key === "ArrowLeft") {
      e.preventDefault();
      S.mibPanelWidth = Math.max(MIN_WIDTH, S.mibPanelWidth - KEY_RESIZE_STEP);
    }
  }
</script>

{#if S.manageMibsOpen}
  <!-- The Manage MIBs page fills the main content region in place of the
       MIB Browser + Results Pane. -->
  <ManageMibsPage />
{:else}
  <div class="flex flex-1 overflow-hidden" bind:this={mainContentEl}>
    <MIBPanel />
    {#if S.mibPanelOpen}
    <div
      class="resize-handle-h w-1.5 cursor-col-resize flex-shrink-0 bg-base-200 hover:bg-primary/30 transition-colors"
      onmousedown={onResizeStart}
      onkeydown={onResizeKeydown}
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize MIB panel (ArrowRight grows, ArrowLeft shrinks)"
      aria-valuenow={S.mibPanelWidth}
      aria-valuemin={MIN_WIDTH}
      aria-valuemax={MAX_WIDTH}
      tabindex="0"
    >
      <div class="resize-grip-h mx-auto"></div>
    </div>
    {/if}
    <main class="flex flex-col flex-1 overflow-hidden min-w-0">
      <ResultsPane />
    </main>
  </div>
{/if}

<style>
.resize-handle-h {
  position: relative;
}
/* daisyUI 5.7 exposes theme colors as full --color-* values; alpha tints
   go through color-mix — the legacy oklch(var(--p) / a) form references
   variables that no longer exist (see TreeNode.svelte). */
.resize-handle-h:hover,
.resize-handle-h:active {
  background-color: color-mix(in oklab, var(--color-primary) 15%, transparent);
}
.resize-grip-h {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  width: 2px;
  height: 32px;
  border-radius: 1px;
  background-color: color-mix(in oklab, var(--color-base-content) 20%, transparent);
}
.resize-handle-h:hover .resize-grip-h {
  background-color: color-mix(in oklab, var(--color-primary) 60%, transparent);
}
</style>
