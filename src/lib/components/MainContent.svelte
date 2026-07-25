<script lang="ts">
  import MIBPanel from "./MIBPanel.svelte";
  import SelectionInfo from "./SelectionInfo.svelte";
  import ResultsPane from "./ResultsPane.svelte";
</script>

<div class="flex flex-1 overflow-hidden">
  <MIBPanel />
  <main class="flex flex-col flex-1 overflow-hidden">
    <SelectionInfo />
    <ResultsPane />
  </main>
</div>
