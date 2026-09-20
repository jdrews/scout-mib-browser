<script lang="ts">
  import { X } from "lucide-svelte";
  import { tick } from "svelte";
  import { S } from "$lib/stores.svelte";
  import { trapFocus } from "$lib/focusTrap";
  import SetValueEditor from "./SetValueEditor.svelte";

  let target = $derived(S.setValueTarget);
  let panelEl: HTMLDialogElement | undefined;
  let lastTrigger: HTMLElement | null = null;

  // Dialog pattern (UX-10): same treatment as the connection modal.
  $effect(() => {
    if (!target) return;
    lastTrigger = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    let cleanup: (() => void) | undefined;
    void tick().then(() => {
      if (panelEl) cleanup = trapFocus(panelEl, close);
    });
    return () => {
      cleanup?.();
      if (lastTrigger instanceof HTMLElement) lastTrigger.focus();
    };
  });

  function close() {
    S.setValueTarget = null;
  }
</script>

{#if target}
  <dialog role="dialog" aria-modal="true" aria-labelledby="set-value-dialog-title" bind:this={panelEl} class="modal modal-open" onclick={close}>
    <div data-testid="set-value-dialog" class="modal-box max-w-[560px] max-h-[80vh] flex flex-col overflow-y-auto" onclick={(e) => e.stopPropagation()}>
      <form method="dialog">
        <button aria-label="Close Set value dialog" class="btn btn-sm btn-circle btn-ghost absolute right-2 top-2 hover:text-error"><X class="w-4 h-4" /></button>
      </form>
      <!-- Keyed by target: a fresh editor instance (and its state) per opened target. -->
      {#key target}
        <SetValueEditor target={target} onClose={close} />
      {/key}
    </div>
  </dialog>
{/if}
