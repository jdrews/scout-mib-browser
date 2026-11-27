<script lang="ts">
  import MIBPanel from "./MIBPanel.svelte";
  import ResultsPane from "./ResultsPane.svelte";
  import ManageMibsPage from "./ManageMibsPage.svelte";
  import { S } from "$lib/stores.svelte";
</script>

{#if S.manageMibsOpen}
  <!-- The Manage MIBs page fills the main content region in place of the
       MIB Browser + Results Pane. -->
  <ManageMibsPage />
{:else}
  <!-- The MIB panel resize handle lives on the panel's right edge, inside
       MIBPanel's nav landmark: a named separator is page content, and as a
       sibling between the nav and main it sat outside every landmark
       (axe region, launch-beta/13). -->
  <div class="flex flex-1 overflow-hidden">
    <MIBPanel />
    <main class="flex flex-col flex-1 overflow-hidden min-w-0">
      <ResultsPane />
    </main>
  </div>
{/if}
