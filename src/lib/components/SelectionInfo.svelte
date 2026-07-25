<script lang="ts">
  import { selectedNode } from "$lib/stores";

  $: node = $selectedNode;
  $: visible = node !== null;
  $: displayName = node ? node.name : "";
  $: displaySyntax = node ? (node.syntax_type || "OID") : "";
  $: displayMib = node ? node.mib_name : "";
</script>

{#if visible}
  <div class="px-4 py-2 bg-surface-0 border-b border-base-01 flex items-center gap-2">
    <span class="text-text font-medium text-sm">{displayName}</span>
    <span class="px-2 py-1 rounded-[3px] text-xs bg-base-01 text-green">
      {displaySyntax}
    </span>
    <span class="px-2 py-1 rounded-[3px] text-xs bg-base-01 text-blue">
      {displayMib}
    </span>
  </div>
{/if}
