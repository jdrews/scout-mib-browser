<script lang="ts">
  import { selectedNode } from "$lib/stores";

  $: node = $selectedNode;
  $: visible = node !== null;
  $: displayName = node ? node.name : "";
  $: displaySyntax = node ? (node.syntax_type || "OID") : "";
  $: displayMib = node ? node.mib_name : "";
</script>

{#if visible}
  <div class="px-4 py-2 bg-base-200 border-b border-base-300 flex items-center gap-2">
    <span class="text-base-content font-medium text-sm">{displayName}</span>
    <span class="badge badge-success gap-2">{displaySyntax}</span>
    <span class="badge badge-primary gap-2">{displayMib}</span>
  </div>
{/if}
