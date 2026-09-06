<script lang="ts">
  import { Check, Copy, X } from "lucide-svelte";
  import { tick } from "svelte";
  import { S } from "$lib/stores.svelte";
  import { trapFocus } from "$lib/focusTrap";
  import { hexDumpLines, interpretBytes, interpretationLabel, asn1TypeCodeText, hexPairs } from "$lib/hexdump";
  import * as exportMod from "$lib/export";

  let target = $derived(S.hexViewTarget);
  let open = $derived(target !== null);

  let panelEl: HTMLDialogElement | undefined;
  let lastTrigger: HTMLElement | null = null;
  let copied = $state(false);

  // Dialog pattern (UX-10): focus moves into the modal on open and back to
  // the trigger on close; Tab cycles inside; Escape closes.
  $effect(() => {
    if (!open) return;
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

  // A new value starts un-copied even if the previous copy feedback lingers.
  $effect(() => {
    void target;
    copied = false;
  });

  function close() {
    S.hexViewTarget = null;
  }

  async function copyHex() {
    if (!target) return;
    const bytes = exportMod.byteData(target.value);
    try {
      await navigator.clipboard.writeText(hexPairs(bytes));
      S.statusText = `Copied hex (${bytes.length} bytes)`;
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch (err) {
      console.error("Clipboard error:", err);
      S.statusText = "Failed to copy";
    }
  }

  let bytes = $derived(target ? exportMod.byteData(target.value) : []);
  let typeCode = $derived(target ? exportMod.rawTypeCode(target.value) : undefined);
  let interp = $derived(bytes.length > 0 ? interpretBytes(bytes) : null);
</script>

{#if open && target}
  <dialog
    role="dialog"
    aria-modal="true"
    aria-labelledby="hex-view-title"
    bind:this={panelEl}
    class="modal modal-open"
    onclick={(e) => { if (e.target === panelEl) close(); }}
  >
    <div data-testid="hex-view-modal" class="modal-box w-full max-w-3xl max-h-[85vh] flex flex-col">
      <div class="flex items-start gap-2">
        <div class="min-w-0 flex-1">
          <h2 id="hex-view-title" data-testid="hex-view-name" class="font-semibold text-sm break-all">{target.displayName}</h2>
          <p class="font-mono text-xs break-all text-base-content/70 mt-0.5">{target.oid}</p>
        </div>
        <button
          data-testid="hex-copy-btn"
          class="btn btn-ghost btn-sm shrink-0"
          title="Copy hex bytes"
          aria-label="Copy hex bytes"
          onclick={copyHex}
        >
          {#if copied}<Check class="w-4 h-4 text-success" />{:else}<Copy class="w-4 h-4" />{/if}
        </button>
        <button data-testid="hex-close-btn" class="btn btn-ghost btn-sm btn-circle shrink-0" aria-label="Close hex view" onclick={close}>
          <X class="w-4 h-4" />
        </button>
      </div>

      <div class="flex flex-wrap gap-x-4 gap-y-1 text-[11px] font-mono text-base-content/60 mt-2 mb-2">
        {#if typeCode !== undefined}
          <span>type: {asn1TypeCodeText(typeCode)}</span>
        {/if}
        {#if interp && interp.kind !== "text"}
          <span>{interpretationLabel(interp.kind)}: {interp.text}</span>
        {/if}
        <span>{bytes.length} bytes</span>
      </div>

      <!-- Uncapped dump — the modal exists precisely because the inline and
           inspector dumps are row-capped. -->
      <div data-testid="hex-view-dump" class="flex-1 min-h-0 overflow-auto border border-base-300 rounded-box bg-base-200/50 p-2 font-mono text-[11px] leading-tight">
        {#each hexDumpLines(bytes) as r (r.offset)}
          <div class="flex gap-2 whitespace-pre">
            <span class="w-8 shrink-0 text-right text-base-content/40">{r.offset}</span>
            <span>{r.hex}</span>
            <span class="text-base-content/70">{r.ascii}</span>
          </div>
        {/each}
      </div>
    </div>
  </dialog>
{/if}
