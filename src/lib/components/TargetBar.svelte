<script lang="ts">
  import { mibSearch } from "$lib/tauriCommands";
  import { S } from "$lib/stores.svelte";
  import { persistTargetConfig } from "$lib/tauriCommands";
  import type { MibSearchResult, TreeNode, SnmpOperation, VariableBinding, ResultSet } from "$lib/types";

  let cfg = $derived(S.targetConfig);

  let inputValue = $state("");
  let treeOidTrigger = $derived(S.targetOidFromTree);

  $effect(() => {
    const oid = treeOidTrigger;
    if (oid && S.selectedNode) {
      inputValue = `${oid}  ${S.selectedNode.name}`;
    }
    return;
  });

  let searchTimer: ReturnType<typeof setTimeout> | null = null;
  let results = $derived(S.autocompleteResults);
  let highlighted = $derived(S.highlightedIndex);
  let operation = $derived(S.snmpOperation);
  let executing = $derived(S.isExecuting);

  const operations: SnmpOperation[] = ["get", "getNext", "walk", "bulkWalk", "set"];

  function onHostInput(e: Event) {
    const val = (e.target as HTMLInputElement).value;
    const next = { ...cfg, host: val };
    Object.assign(S.targetConfig, next);
    persistTargetConfig(next);
  }

  function onPortInput(e: Event) {
    const val = parseInt((e.target as HTMLInputElement).value, 10);
    if (!isNaN(val) && val > 0 && val < 65536) {
      const next = { ...cfg, port: val };
      Object.assign(S.targetConfig, next);
      persistTargetConfig(next);
    }
  }

  function openConnectionPanel() {
    S.connectionPanelOpen = true;
  }

  function onInput(e: Event) {
    const target = e.target as HTMLInputElement;
    const val = target.value.trim();
    inputValue = target.value;

    if (val.length < 1) {
      S.autocompleteResults.length = 0;
      return;
    }
    S.highlightedIndex = -1;

    if (searchTimer) clearTimeout(searchTimer);
    searchTimer = setTimeout(() => performSearch(val), 150);
  }

  async function performSearch(query: string) {
    try {
      const res = await mibSearch(query);
      S.autocompleteResults.length = 0;
      S.autocompleteResults.push(...res);
    } catch (err) {
      console.error("Search failed:", err);
    }
  }

  function onKeyDown(e: KeyboardEvent) {
    if (!results.length && e.key !== "Enter") return;

    if (e.key === "ArrowDown") {
      e.preventDefault();
      S.highlightedIndex = Math.min(S.highlightedIndex + 1, results.length - 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      S.highlightedIndex = Math.max(S.highlightedIndex - 1, 0);
    } else if (e.key === "Enter" && S.highlightedIndex >= 0) {
      e.preventDefault();
      selectItem(results[S.highlightedIndex]);
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
    const data = S.treeData;
    const found = findNode(data, oid);
    if (found) {
      S.selectedNode = found;
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
    S.autocompleteResults.length = 0;
    S.highlightedIndex = -1;
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

    const targetNode = S.selectedNode;
    const effectiveOid = targetNode?.oid || oid;

    if (operation === "set") {
      handleSet(effectiveOid);
      return;
    }

    await executeOperation(operation, effectiveOid);
  }

  async function executeOperation(op: SnmpOperation, oid: string) {
    const cfg = S.targetConfig;
    if (!cfg.host) {
      S.statusText = "No target configured";
      return;
    }

    const targetNode = S.selectedNode;
    const isTableNode = targetNode?.is_table === true;

    S.isExecuting = true;
    S.executionBindings.length = 0;
    S.executionResults = null;
    S.tableResult = null;
    S.walkProgress = "";
    S.queryRootOid = oid;

    if (isTableNode && (op === "walk" || op === "bulkWalk")) {
      await executeTableRetrieval(op, oid);
      return;
    }

    S.statusText = `Starting ${op} on ${oid}...`;

    let unlisten: (() => void) | undefined;
    try {
      if (op === "get") {
        const cmds = await import("$lib/tauriCommands");
        const result = await cmds.snmpGet(cfg, [oid]);
        S.executionBindings.length = 0;
        S.executionBindings.push(...result.bindings);
        S.executionResults = result;
        S.statusText = `Get complete: ${result.bindings.length} binding(s)`;
      } else if (op === "getNext") {
        const cmds = await import("$lib/tauriCommands");
        const result = await cmds.snmpGetNext(cfg, [oid]);
        S.executionBindings.length = 0;
        S.executionBindings.push(...result.bindings);
        S.executionResults = result;
        S.statusText = `GetNext complete: ${result.bindings.length} binding(s)`;
      } else if (op === "walk" || op === "bulkWalk") {
        const cmds = await import("$lib/tauriCommands");
        let count = 0;
        const fn = op === "walk" ? cmds.snmpWalk : cmds.snmpBulkWalk;
        const handle = await fn(cfg, oid,
          (batch: VariableBinding[]) => {
            count += batch.length;
            S.executionBindings.push(...batch);
            S.walkProgress = `${count} bindings`;
            S.statusText = `${op}: ${count} bindings...`;
          },
          (result: ResultSet) => {
            S.executionResults = result;
            S.walkProgress = "";
            S.statusText = `${op} complete: ${result.bindings.length} binding(s)`;
            handle.unlisten();
          }
        );
        unlisten = handle.unlisten;
      }
    } catch (err) {
      console.error("SNMP operation failed:", err);
      S.statusText = `Error: ${err}`;
      S.executionResults = { bindings: [], partial: true, warnings: [{ kind: "error", message: String(err) }] };
    } finally {
      if (unlisten) unlisten();
      S.isExecuting = false;
    }
  }

  async function executeTableRetrieval(op: SnmpOperation, tableOid: string) {
    const cfg = S.targetConfig;
    S.statusText = `Detecting table columns for ${tableOid}...`;

    try {
      const cmds = await import("$lib/tauriCommands");
      const columnOids = await cmds.mibTableColumns(tableOid);

      if (columnOids.length === 0) {
        S.statusText = `No columns found for table ${tableOid}`;
        S.isExecuting = false;
        return;
      }

      S.statusText = `Walking ${columnOids.length} column(s) for table...`;

      const result = await cmds.snmpWalkTable(cfg, tableOid, columnOids);
      S.tableResult = result;
      S.walkProgress = "";
      S.statusText = `Table complete: ${result.total_rows} row(s), ${result.columns.length} column(s)`;

      if (result.missing_cells > 0) {
        S.statusText += ` (${result.missing_cells} missing cell(s))`;
      }
    } catch (err) {
      console.error("Table retrieval failed:", err);
      S.statusText = `Table error: ${err}`;
      S.tableResult = null;
    } finally {
      S.isExecuting = false;
    }
  }

  async function handleSet(oid: string) {
    const cfg = S.targetConfig;
    if (!cfg.host) {
      S.statusText = "No target configured";
      return;
    }

    const node = S.selectedNode;
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
      S.isExecuting = true;
      S.statusText = `Setting ${oid}...`;
      const result = await import("$lib/tauriCommands").then(m => m.snmpSet(cfg, oid, valueType, parsedValue));
      S.executionBindings.length = 0;
      S.executionBindings.push(...result.bindings);
      S.executionResults = result;
      S.statusText = `Set complete: ${result.bindings.length} binding(s)`;
    } catch (err) {
      console.error("SNMP Set failed:", err);
      S.statusText = `Set error: ${err}`;
      S.executionResults = { bindings: [], partial: true, warnings: [{ kind: "error", message: String(err) }] };
    } finally {
      S.isExecuting = false;
    }
  }

  function hideOnOutsideClick(e: MouseEvent) {
    const target = e.target as HTMLElement;
    if (!target.closest("[data-address-bar]")) {
      hideAutocomplete();
    }
  }
</script>

<div data-address-bar class="flex items-center gap-2 px-4 py-2 bg-base-200 border-b border-base-300 flex-shrink-0 relative" onclick={(e) => { if (e.target === e.currentTarget) hideOnOutsideClick(e); }}>
  <label class="text-xs font-semibold uppercase tracking-wide text-base-content/60 whitespace-nowrap">Target</label>

  <div class="join">
    <input
      type="text"
      placeholder="Host or IP"
      value={cfg.host}
      oninput={onHostInput}
      class="input input-bordered input-sm w-[160px] font-mono join-item"
    />

    <span class="bg-base-300 text-base-content/60 text-sm px-2 flex items-center join-item">:</span>

    <input
      type="text"
      placeholder="Port"
      value={cfg.port}
      oninput={onPortInput}
      class="input input-bordered input-sm w-[60px] font-mono join-item text-center"
    />
  </div>

  <button
    title="Connection settings"
    onclick={openConnectionPanel}
    class="btn btn-ghost btn-circle btn-sm"
  >
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <circle cx="12" cy="12" r="3"/>
      <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>
    </svg>
  </button>

  <input
    type="text"
    placeholder="Enter OID or MIB name (e.g., sysDescr)"
    autocomplete="off"
    class="flex-1 input input-bordered input-sm font-mono"
    bind:value={inputValue}
    oninput={onInput}
    onkeydown={onKeyDown}
  />

  <select
    class="select select-bordered select-sm w-[90px]"
    bind:value={S.snmpOperation}
  >
    {#each operations as op (op)}
      <option value={op}>{op}</option>
    {/each}
  </select>

  <button
    class="btn btn-primary btn-sm"
    onclick={handleGo}
    disabled={executing || !inputValue.trim()}
  >
    {executing ? "..." : "Go"}
  </button>

  {#if results.length > 0}
    <div class="absolute top-full left-4 right-[200px] bg-base-100 border border-base-300 rounded-box max-h-[240px] overflow-y-auto z-[500] shadow-lg">
      {#each results as item, i (item.oid)}
        <div
          class="px-3 py-2 text-sm cursor-pointer flex justify-between items-center hover:bg-base-200"
          class:bg-base-200={i === S.highlightedIndex}
          onclick={() => selectItem(item)}
        >
          <span class="text-base-content">{item.name}</span>
          <span class="text-xs text-base-content/60 font-mono">{item.oid}</span>
        </div>
      {/each}
    </div>
  {/if}
</div>
