<script lang="ts">
  import { S } from "$lib/stores.svelte";

  let node = $derived(S.selectedNode);
  let visible = $derived(node !== null);
  let displayName = $derived(node ? node.name : "");
  let displaySyntax = $derived(node ? (node.syntax_type || "OID") : "");
  let displayMib = $derived(node ? node.mib_name : "");
</script>

{#if visible}
  <div class="px-4 py-2 bg-base-200 border-b border-base-300 flex items-center gap-2">
    <span class="text-base-content font-medium text-sm">{displayName}</span>
    <span class="badge badge-success gap-2">{displaySyntax}</span>
    <span class="badge badge-primary gap-2">{displayMib}</span>
  </div>
{/if}
