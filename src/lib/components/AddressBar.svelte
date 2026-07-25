<script lang="ts">
  import { mibSearch } from "$lib/tauriCommands";
import {
  selectedNode,
  autocompleteResults as acStore,
  highlightedIndex as hiStore,
  statusText,
  treeData,
  snmpOperation,
  isExecuting,
  executionBindings,
  executionResults,
  walkProgress,
  targetConfig,
  queryRootOid,
  tableResult as tableResultStore,
} from "$lib/stores";
  import type { MibSearchResult, TreeNode, SnmpOperation, VariableBinding, ResultSet } from "$lib/types";

  let inputValue = "";
  let searchTimer: ReturnType<typeof setTimeout> | null = null;
  $: results = $acStore;
  $: highlighted = $hiStore;
  $: operation = $snmpOperation;
  $: executing = $isExecuting;

  function onInput(e: Event) {
    const target = e.target as HTMLInputElement;
    const val = target.value.trim();
    inputValue = target.value;

    if (val.length < 1) {
      $acStore = [];
      return;
    }
    $hiStore = -1;

    if (searchTimer) clearTimeout(searchTimer);
    searchTimer = setTimeout(() => performSearch(val), 150);
  }

  async function performSearch(query: string) {
    try {
      const res = await mibSearch(query);
      $acStore = res;
    } catch (err) {
      console.error("Search failed:", err);
    }
  }

  function onKeyDown(e: KeyboardEvent) {
    if (!results.length && e.key !== "Enter") return;

    if (e.key === "ArrowDown") {
      e.preventDefault();
      $hiStore = Math.min($hiStore + 1, results.length - 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      $hiStore = Math.max($hiStore - 1, 0);
    } else if (e.key === "Enter" && $hiStore >= 0) {
      e.preventDefault();
      selectItem(results[$hiStore]);
    } else if (e.key === "Escape") {
      hideAutocomplete();
    } else if (e.key === "Enter") {
      e.preventDefault();
      handleGo();
    }
  }

  function selectItem(item: MibSearchResult) {
    inputValue = `${item.oid}  ${item.name}`;
    hideAutocomplete();
    trySelectInTree(item.oid);
  }

  function trySelectInTree(oid: string) {
    const data = $treeData;
    const found = findNode(data, oid);
    if (found) {
      $selectedNode = found;
    }
  }

  function findNode(nodes: TreeNode[], oid: string): TreeNode | null {
    for (const n of nodes) {
      if (n.oid === oid) return n;
      if (n.children) {
        const found = findNode(n.children, oid);
        if (found) return found;
      }
    }
    return null;
  }

  function hideAutocomplete() {
    $acStore = [];
    $hiStore = -1;
  }

  function extractOid(val: string): string {
    const parts = val.split(/\s{2,}/);
    return parts[0].trim();
  }

  async function handleGo() {
    const val = inputValue.trim();
    if (!val || executing) return;

    let oid = extractOid(val);
    if (!oid) oid = val.trim();

    // If we have a selected node, use its OID
    const targetNode = $selectedNode;
    const effectiveOid = targetNode?.oid || oid;

    if (operation === "set") {
      handleSet(effectiveOid);
      return;
    }

    await executeOperation(operation, effectiveOid);
  }

  async function executeOperation(op: SnmpOperation, oid: string) {
    const cfg = $targetConfig;
    if (!cfg.host) {
      $statusText = "No target configured";
      return;
    }

    // Check if the selected node is a TABLE — if so, use table retrieval mode.
    const targetNode = $selectedNode;
    const isTableNode = targetNode?.is_table === true;

    $isExecuting = true;
    $executionBindings = [];
    $executionResults = null;
    $tableResultStore = null;
    $walkProgress = "";
    $queryRootOid = oid;

    if (isTableNode && (op === "walk" || op === "bulkWalk")) {
      await executeTableRetrieval(op, oid);
      return;
    }

    $statusText = `Starting ${op} on ${oid}...`;

    let unlisten: (() => void) | undefined;
    try {
      if (op === "get") {
        const cmds = await import("$lib/tauriCommands");
        const result = await cmds.snmpGet(cfg, [oid]);
        $executionBindings = result.bindings;
        $executionResults = result;
        $statusText = `Get complete: ${result.bindings.length} binding(s)`;
      } else if (op === "getNext") {
        const cmds = await import("$lib/tauriCommands");
        const result = await cmds.snmpGetNext(cfg, [oid]);
        $executionBindings = result.bindings;
        $executionResults = result;
        $statusText = `GetNext complete: ${result.bindings.length} binding(s)`;
      } else if (op === "walk" || op === "bulkWalk") {
        const cmds = await import("$lib/tauriCommands");
        let count = 0;
        const fn = op === "walk" ? cmds.snmpWalk : cmds.snmpBulkWalk;
        const handle = await fn(cfg, oid,
          (batch: VariableBinding[]) => {
            count += batch.length;
            $executionBindings = [...$executionBindings, ...batch];
            $walkProgress = `${count} bindings`;
            $statusText = `${op}: ${count} bindings...`;
          },
          (result: ResultSet) => {
            $executionResults = result;
            $walkProgress = "";
            $statusText = `${op} complete: ${result.bindings.length} binding(s)`;
            handle.unlisten();
          }
        );
        unlisten = handle.unlisten;
      }
    } catch (err) {
      console.error("SNMP operation failed:", err);
      $statusText = `Error: ${err}`;
      $executionResults = { bindings: [], partial: true, warnings: [{ kind: "error", message: String(err) }] };
    } finally {
      if (unlisten) unlisten();
      $isExecuting = false;
    }
  }

  async function executeTableRetrieval(op: SnmpOperation, tableOid: string) {
    const cfg = $targetConfig;
    $statusText = `Detecting table columns for ${tableOid}...`;

    try {
      // Get column OIDs from the MIB resolver.
      const cmds = await import("$lib/tauriCommands");
      const columnOids = await cmds.mibTableColumns(tableOid);

      if (columnOids.length === 0) {
        $statusText = `No columns found for table ${tableOid}`;
        $isExecuting = false;
        return;
      }

      $statusText = `Walking ${columnOids.length} column(s) for table...`;

      // Execute table walk.
      const result = await cmds.snmpWalkTable(cfg, tableOid, columnOids);
      $tableResultStore = result;
      $walkProgress = "";
      $statusText = `Table complete: ${result.total_rows} row(s), ${result.columns.length} column(s)`;

      if (result.missing_cells > 0) {
        $statusText += ` (${result.missing_cells} missing cell(s))`;
      }
    } catch (err) {
      console.error("Table retrieval failed:", err);
      $statusText = `Table error: ${err}`;
      $tableResultStore = null;
    } finally {
      $isExecuting = false;
    }
  }

  async function handleSet(oid: string) {
    const cfg = $targetConfig;
    if (!cfg.host) {
      $statusText = "No target configured";
      return;
    }

    // For Set, we need a value input — show inline prompt
    const node = $selectedNode;
    const syntaxType = node?.syntax_type || "OctetString";
    const proposedValue = prompt(`Set value for ${oid} (${syntaxType}):`);
    if (proposedValue === null) return;

    let valueType: string;
    let parsedValue: unknown;

    switch (syntaxType.toLowerCase()) {
      case "integer":
      case "integer32":
        valueType = "Integer";
        parsedValue = parseInt(proposedValue, 10);
        break;
      case "counter32":
        valueType = "Counter32";
        parsedValue = parseInt(proposedValue, 10) >>> 0;
        break;
      case "counter64":
        valueType = "Counter64";
        parsedValue = BigInt(proposedValue);
        break;
      case "gauge32":
      case "unsigned32":
        valueType = "Gauge32";
        parsedValue = parseInt(proposedValue, 10) >>> 0;
        break;
      case "ipaddress":
      case "ip address":
        valueType = "IpAddress";
        parsedValue = proposedValue;
        break;
      case "timeticks":
        valueType = "TimeTicks";
        parsedValue = parseInt(proposedValue, 10) >>> 0;
        break;
      case "object identifier":
        valueType = "ObjectIdentifier";
        parsedValue = proposedValue;
        break;
      default:
        valueType = "OctetString";
        parsedValue = proposedValue;
    }

    try {
      $isExecuting = true;
      $statusText = `Setting ${oid}...`;
      const result = await import("$lib/tauriCommands").then(m => m.snmpSet(cfg, oid, valueType, parsedValue));
      $executionBindings = result.bindings;
      $executionResults = result;
      $statusText = `Set complete: ${result.bindings.length} binding(s)`;
    } catch (err) {
      console.error("SNMP Set failed:", err);
      $statusText = `Set error: ${err}`;
      $executionResults = { bindings: [], partial: true, warnings: [{ kind: "error", message: String(err) }] };
    } finally {
      $isExecuting = false;
    }
  }

  function hideOnOutsideClick(e: MouseEvent) {
    const target = e.target as HTMLElement;
    if (!target.closest("[data-address-bar]")) {
      hideAutocomplete();
    }
  }
</script>

<div data-address-bar class="px-4 py-3 bg-base-00 border-b border-base-01 relative" on:click|self={hideOnOutsideClick}>
  <div class="flex gap-3 items-center">
    <select
      class="bg-surface-0 border border-base-01 text-text px-4 py-2 text-sm font-mono rounded outline-none focus:border-blue cursor-pointer"
      bind:value={operation}
    >
      <option value="get">Get</option>
      <option value="getNext">GetNext</option>
      <option value="walk">Walk</option>
      <option value="bulkWalk">BulkWalk</option>
      <option value="set">Set</option>
    </select>
    <input
      type="text"
      placeholder="Enter OID or MIB name (e.g., 1.3.6.1.2.1.1.1 or sysDescr)"
      autocomplete="off"
      class="flex-1 bg-surface-0 border border-base-01 text-text px-4 py-2 text-sm font-mono rounded outline-none focus:border-blue"
      bind:value={inputValue}
      on:input={onInput}
      on:keydown={onKeyDown}
    />
    <button
      class="bg-blue text-base-00 border-none px-5 py-2 text-sm font-semibold rounded cursor-pointer hover:bg-sapphire disabled:opacity-50 disabled:cursor-not-allowed"
      on:click={handleGo}
      disabled={executing || !inputValue.trim()}
    >
      {executing ? "..." : "Go"}
    </button>
  </div>

  {#if results.length > 0}
    <div class="absolute top-full left-3 right-3 bg-base-00 border border-base-01 rounded-lg max-h-[240px] overflow-y-auto z-[500] shadow-lg">
      {#each results as item, i (item.oid)}
        <div
          class="px-3 py-2 text-sm cursor-pointer flex justify-between items-center hover:bg-base-01"
          class:bg-base-01={i === $hiStore}
          on:click={() => selectItem(item)}
        >
          <span class="text-text">{item.name}</span>
          <span class="text-xs text-overlay font-mono">{item.oid}</span>
        </div>
      {/each}
    </div>
  {/if}
</div>

