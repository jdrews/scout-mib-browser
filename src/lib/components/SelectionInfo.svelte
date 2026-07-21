<script lang="ts">
  import { selectedNode } from "$lib/stores";

  $: node = $selectedNode;
  $: visible = node !== null;
  $: displayName = node ? node.name : "";
  $: displaySyntax = node ? (node.syntax_type || "OID") : "";
  $: displayMib = node ? node.mib_name : "";
</script>

{#if visible}
  <div class="px-3 py-1.5 bg-surface-0 border-b border-base-01 text-xs flex items-center gap-2">
    <span class="text-text font-medium">{displayName}</span>
    <span class="px-1.5 py-0.5 rounded-[3px] text-[11px] bg-base-01 text-green">
      {displaySyntax}
    </span>
    <span class="px-1.5 py-0.5 rounded-[3px] text-[11px] bg-base-01 text-blue">
      {displayMib}
    </span>
  </div>
{/if}
