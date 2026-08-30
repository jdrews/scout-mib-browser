<script lang="ts">
  import { Settings } from "lucide-svelte";
  import { mibSearch, mibResolveOid } from "$lib/tauriCommands";
  import { S } from "$lib/stores.svelte";
  import { persistTargetConfig } from "$lib/tauriCommands";
  import type { MibSearchResult, TreeNode, SnmpOperation, VariableBinding, ResultSet, TableInfo, TableResult } from "$lib/types";

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

  // Track active walk state for Go/Stop toggle and Esc key cancellation
  let isWalkActive = $state(false);
  // True while a Get Table run is in flight (drives the Stop status message).
  let tableRunActive = $state(false);

  const operations: SnmpOperation[] = ["get", "getNext", "walk", "bulkWalk", "getTable", "set", "getSubtree"];

  const operationLabels: Record<SnmpOperation, string> = {
    get: "Get",
    getNext: "Get Next",
    walk: "Walk",
    bulkWalk: "Bulk Walk",
    getTable: "Get Table",
    set: "Set",
    getSubtree: "Get Subtree",
  };

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

    // UX-07: a manual edit makes the typed value authoritative — drop the tree
    // selection so the two can't silently diverge at Go time.
    if (S.selectedNode) S.selectedNode = null;

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
    // Cancel active walk with Escape
    if (e.key === "Escape" && isWalkActive) {
      e.preventDefault();
      handleStop();
      return;
    }

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
    // An autocomplete pick is a selection — report it in the inspector.
    S.inspectorOid = item.oid;
    S.inspectorValue = null;
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

  /** Resolves the typed value to an unambiguous OID: numeric OIDs pass
   *  through; MIB names must match exactly (case-insensitive). */
  async function resolveEffectiveOid(val: string): Promise<string | null> {
    let oid = extractOid(val);
    if (!oid) oid = val.trim();
    if (!oid) return S.selectedNode?.oid ?? null;

    if (/^\d+(\.\d+)*$/.test(oid)) return oid;

    const res = await mibSearch(oid);
    const exact = res.find((r) => r.name.toLowerCase() === oid.toLowerCase());
    return exact?.oid ?? null;
  }

  async function handleGo() {
    const val = inputValue.trim();
    if (executing) return;

    // UX-07: the typed value is authoritative; a tree selection only fills in
    // when the bar is untouched. The effective OID must be unambiguous.
    let effectiveOid = "";
    try {
      effectiveOid = (await resolveEffectiveOid(val)) ?? "";
    } catch (err) {
      console.error("OID resolution failed:", err);
    }

    if (!effectiveOid) {
      S.statusText = val.trim()
        ? `No MIB object named "${extractOid(val) || val.trim()}" — type a full name or OID`
        : "Enter an OID or select a tree node";
      return;
    }

    if (operation === "set") {
      handleSet(effectiveOid);
      return;
    }

    if (operation === "getTable") {
      await executeGetTable(effectiveOid);
      return;
    }

    // Get Subtree is a local MIB query — no Target required.
    if (operation === "getSubtree") {
      await executeGetSubtree(effectiveOid);
      return;
    }

    // Table Get guard: getting a table's raw OID only surfaces noSuchObject
    // noise — point the user at the operations that make sense. Covers both
    // tree selections and typed OIDs/names (resolved via mib_resolve_oid).
    if (operation === "get" || operation === "getNext") {
      let tableName: string | null = S.selectedNode?.isTable === true ? S.selectedNode.name : null;
      if (tableName === null) {
        try {
          const resolved = await mibResolveOid(effectiveOid);
          if (resolved?.isTable) tableName = resolved.name;
        } catch (err) {
          console.error("Table guard lookup failed:", err);
        }
      }
      if (tableName !== null) {
        S.statusText = `${tableName} is a table — use Get Table or Walk`;
        return;
      }
    }

    await executeOperation(operation, effectiveOid);
  }

  async function handleStop() {
    if (!isWalkActive) return;
    const wasTableRun = tableRunActive;
    isWalkActive = false;
    tableRunActive = false;

    const cmds = await import("$lib/tauriCommands");
    await cmds.snmpCancelWalk();

    S.isExecuting = false;
    S.walkProgress = "";
    S.statusText = wasTableRun ? "Table retrieval cancelled" : "Walk cancelled";
  }

  async function executeOperation(op: SnmpOperation, oid: string) {
    const cfg = S.targetConfig;
    if (!cfg.host) {
      S.statusText = "No target configured";
      return;
    }

    S.isExecuting = true;
    S.executionBindings.length = 0;
    S.executionResults = null;
    S.tableResult = null;
    S.subtreeNodes = null;
    S.walkProgress = "";
    S.queryRootOid = oid;
    isWalkActive = false;

    // Walk/Bulk Walk on a table node behaves like anywhere else — a flat
    // subtree walk. Get Table (executeGetTable) is the only grid path.
    S.statusText = `Starting ${op} on ${oid}...`;

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
        isWalkActive = true;

        // Buffer bindings and flush to reactive store periodically.
        // Larger batches reduce Svelte $derived recalculation frequency (which is O(n) per row).
        const buffer: VariableBinding[] = [];
        let flushTimer: ReturnType<typeof setTimeout> | null = null;
        function scheduleFlush() {
          if (flushTimer) clearTimeout(flushTimer);
          flushTimer = setTimeout(() => {
            flushTimer = null;
            if (buffer.length > 0) {
              S.executionBindings.push(...buffer);
              buffer.length = 0;
            }
          }, 100);
        }

        const fn = op === "walk" ? cmds.snmpWalk : cmds.snmpBulkWalk;
        await fn(cfg, oid,
          (batch: VariableBinding[]) => {
            if (!isWalkActive) return;
            count += batch.length;
            buffer.push(...batch);
            S.walkProgress = `${count} bindings`;
            S.statusText = `${op}: ${count} bindings...`;
            // Flush every 200 bindings or on timer, whichever comes first.
            if (buffer.length >= 200) {
              S.executionBindings.push(...buffer);
              buffer.length = 0;
              if (flushTimer) {
                clearTimeout(flushTimer);
                flushTimer = null;
              }
            } else {
              scheduleFlush();
            }
          },
          (result: ResultSet) => {
            if (!isWalkActive) return;
            // Flush remaining buffered bindings
            if (buffer.length > 0) {
              S.executionBindings.push(...buffer);
              buffer.length = 0;
            }
            if (flushTimer) {
              clearTimeout(flushTimer);
              flushTimer = null;
            }
            isWalkActive = false;
            S.executionResults = result;
            S.walkProgress = "";
            S.statusText = `${op} complete: ${count} binding(s)`;
          }
        );
      }
    } catch (err) {
      console.error("SNMP operation failed:", err);
      isWalkActive = false;
      S.statusText = `Error: ${err}`;
      S.executionResults = { bindings: [], partial: true, warnings: [{ kind: "error", message: String(err) }] };
    } finally {
      S.isExecuting = false;
    }
  }

  /** Get Table: the only path to grid retrieval. Fetches every column —
   *  display-column selection is a client-side filter in ResultsPane. */
  async function executeGetTable(oid: string) {
    const cfg = S.targetConfig;
    if (!cfg.host) {
      S.statusText = "No target configured";
      return;
    }

    const cmds = await import("$lib/tauriCommands");

    let info: TableInfo | null = null;
    try {
      info = await cmds.mibTableInfo(oid);
    } catch (err) {
      console.error("Table info lookup failed:", err);
    }

    if (!info) {
      const label = S.selectedNode?.name ?? oid;
      S.statusText = `${label} is not a table — use Walk`;
      return;
    }

    S.isExecuting = true;
    S.executionBindings.length = 0;
    S.executionResults = null;
    S.tableInfo = info;
    S.tableResult = null;
    S.subtreeNodes = null;
    S.walkProgress = "";
    S.queryRootOid = oid;
    isWalkActive = false;
    tableRunActive = true;

    const name = info.name || oid;
    S.statusText = `Detecting table columns for ${name}...`;

    try {
      const allColumns = await cmds.mibTableColumns(oid);
      if (allColumns.length === 0) {
        tableRunActive = false;
        S.statusText = `No columns found for table ${name}`;
        return;
      }

      S.statusText = `Fetching table ${name}...`;
      isWalkActive = true;

      await cmds.snmpGetTable(cfg, oid, allColumns,
        (count: number) => {
          if (!isWalkActive) return;
          S.walkProgress = `${count} bindings`;
          S.statusText = `Get Table: ${count} bindings...`;
        },
        (result: TableResult) => {
          if (!isWalkActive) return;
          isWalkActive = false;
          tableRunActive = false;
          S.tableResult = result;
          S.walkProgress = "";
          let msg = `Table complete: ${result.total_rows} row(s), ${result.columns.length} column(s)`;
          if (result.missing_cells > 0) {
            msg += ` (${result.missing_cells} missing cell(s))`;
          }
          S.statusText = msg;
        }
      );
    } catch (err) {
      console.error("Table retrieval failed:", err);
      isWalkActive = false;
      tableRunActive = false;
      S.statusText = `Table error: ${err}`;
      S.tableResult = null;
    } finally {
      S.isExecuting = false;
    }
  }

  /** Get Subtree: a local query of the MIB tree hierarchy — lists every node
   *  under the OID in tree order. No Target involved. */
  async function executeGetSubtree(oid: string) {
    S.isExecuting = true;
    S.executionBindings.length = 0;
    S.executionResults = null;
    S.tableResult = null;
    S.walkProgress = "";
    S.queryRootOid = oid;
    S.statusText = `Loading subtree for ${oid}...`;

    try {
      const cmds = await import("$lib/tauriCommands");
      const nodes = await cmds.mibSubtree(oid);
      S.subtreeNodes = nodes;
      S.statusText = `Get Subtree complete: ${nodes.length} node(s) under ${oid}`;
    } catch (err) {
      console.error("Subtree retrieval failed:", err);
      S.subtreeNodes = [];
      S.statusText = `Subtree error: ${err}`;
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
    const syntaxType = node?.syntaxType || "OctetString";
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
  <label for="target-host" class="text-xs font-semibold uppercase tracking-wide text-base-content/60 whitespace-nowrap">Target</label>

  <div class="join">
    <input
      id="target-host"
      data-testid="host-input"
      type="text"
      placeholder="Host or IP"
      value={cfg.host}
      oninput={onHostInput}
      class="input input-bordered input-sm w-[160px] font-mono join-item"
    />

    <!-- /60 (not /40): 4.5:1 AA on base-200 in the light theme. -->
    <span class="text-base-content/60 text-sm flex items-center join-item">:</span>

    <input
      data-testid="port-input"
      aria-label="Port"
      type="text"
      placeholder="Port"
      value={cfg.port}
      oninput={onPortInput}
      class="input input-bordered input-sm w-[60px] font-mono join-item text-center"
    />
  </div>

  <button
    data-testid="conn-gear"
    aria-label="Connection settings"
    title="Connection settings"
    onclick={openConnectionPanel}
    class="btn btn-ghost btn-circle btn-sm"
  >
    <Settings class="w-4 h-4" />
  </button>

  <input
    id="oid-input"
    data-testid="oid-input"
    aria-label="OID or MIB name"
    type="text"
    placeholder="Enter OID or MIB name (e.g., sysDescr)"
    autocomplete="off"
    class="flex-1 input input-bordered input-sm font-mono"
    bind:value={inputValue}
    oninput={onInput}
    onkeydown={onKeyDown}
  />

  <select
    data-testid="op-select"
    aria-label="SNMP operation"
    class="select select-bordered select-sm w-[90px]"
    bind:value={S.snmpOperation}
  >
    {#each operations as op (op)}
      <option value={op}>{operationLabels[op]}</option>
    {/each}
  </select>

  {#if isWalkActive}
    <button
      data-testid="stop-btn"
      class="btn btn-error btn-sm"
      onclick={handleStop}
    >
      Stop
    </button>
  {:else}
    <button
      data-testid="go-btn"
      class="btn btn-primary btn-sm"
      onclick={handleGo}
      disabled={executing || !inputValue.trim()}
    >
      {executing ? "..." : "Go"}
    </button>
  {/if}

  {#if results.length > 0}
    <div data-testid="autocomplete-list" class="absolute top-full left-4 right-[200px] bg-base-100 border border-base-300 rounded-box max-h-[240px] overflow-y-auto z-[500] shadow-lg">
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
