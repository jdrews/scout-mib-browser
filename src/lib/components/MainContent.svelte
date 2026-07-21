<script lang="ts">
  import MIBPanel from "./MIBPanel.svelte";
  import AddressBar from "./AddressBar.svelte";
  import SelectionInfo from "./SelectionInfo.svelte";
  import ResultsPane from "./ResultsPane.svelte";
</script>

<div class="flex flex-1 overflow-hidden">
  <MIBPanel />
  <main class="flex flex-col flex-1 overflow-hidden">
    <AddressBar />
    <SelectionInfo />
    <ResultsPane />
  </main>
</div>
