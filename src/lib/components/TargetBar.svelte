<script lang="ts">
  import { Settings } from "lucide-svelte";
  import { mibSearch, mibResolveOid, mibNodeDetails } from "$lib/tauriCommands";
  import { S } from "$lib/stores.svelte";
  import { schedulePersistTargetConfig } from "$lib/persistTarget";
  import { markConnected, markDisconnected, hasErrorWarning, streamingOutcome } from "$lib/connectionLogic";
  import { RunGuard } from "$lib/runGuard";
  import { setGuard, effectiveSetOid, setDetailsFor } from "$lib/setLogic";
  import { pluralize } from "$lib/format";
  import type { MibSearchResult, TreeNode, SnmpOperation, VariableBinding, ResultSet, TableInfo, TableResult, MibNodeDetails } from "$lib/types";

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
  // True while a Get / GetNext / Set run is in flight (launch-beta/08):
  // Stop and Escape cancel it via the same shared token as walks.
  let cancellableRun = $state(false);
  // True while a Get Table run is in flight (drives the Stop status message).
  let tableRunActive = $state(false);
  // Get Next stepping cursor: the OID the last Get Next run returned. A new
  // run that starts exactly there continues the sequence — its bindings are
  // appended to the Result Set instead of replacing it. Any other start (new
  // selection, typed OID, or a different operation) begins a fresh sequence.
  let getNextCursor = $state<string | null>(null);
  // Drop outcomes of superseded or stopped runs (see $lib/runGuard): a
  // cancelled GetNext resolves late by design, and applying its empty
  // result wipes the sequence cursor and the Result Set of the run that
  // replaced it.
  const runGuard = new RunGuard();

  // Dropdown order is alphabetical by label.
  const operations: SnmpOperation[] = ["get", "getNext", "getSubtree", "getTable", "set", "walk", "bulkWalk"];

  const operationLabels: Record<SnmpOperation, string> = {
    get: "Get",
    getNext: "Get Next",
    walk: "Walk",
    bulkWalk: "Walk (Bulk)",
    getTable: "Get Table",
    set: "Set",
    getSubtree: "Get Subtree",
  };

  function onHostInput(e: Event) {
    const val = (e.target as HTMLInputElement).value;
    const next = { ...cfg, host: val };
    Object.assign(S.targetConfig, next);
    // Debounced + error-reporting: a keystroke burst is one write, and a
    // failed write reaches the status bar (launch-beta/10).
    schedulePersistTargetConfig(next);
  }

  function onPortInput(e: Event) {
    const val = parseInt((e.target as HTMLInputElement).value, 10);
    if (!isNaN(val) && val > 0 && val < 65536) {
      const next = { ...cfg, port: val };
      Object.assign(S.targetConfig, next);
      schedulePersistTargetConfig(next);
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
    // Cancel an active walk or Get/GetNext/Set run with Escape
    if (e.key === "Escape" && (isWalkActive || cancellableRun)) {
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

  /** Longest-prefix MIB name for an OID (e.g. `…sysDescr.0` -> "sysDescr.0"). */
  function displayNameForOid(oid: string): string {
    const parts = oid.split(".");
    for (let i = parts.length; i > 1; i--) {
      const prefix = parts.slice(0, i).join(".");
      if (S.oidNameMap.has(prefix)) {
        const suffix = parts.slice(i).join(".");
        const baseName = S.oidNameMap.get(prefix)!;
        return suffix ? `${baseName}.${suffix}` : baseName;
      }
    }
    return oid;
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
      await openSetValueDialog(effectiveOid);
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
    if (!isWalkActive && !cancellableRun) return;
    const wasTableRun = tableRunActive;
    const wasSimpleRun = cancellableRun && !isWalkActive;
    isWalkActive = false;
    tableRunActive = false;
    cancellableRun = false;
    // Retire the run: its late result/rejection must not touch state that
    // now belongs to whatever the user starts next.
    runGuard.retire();

    const cmds = await import("$lib/tauriCommands");
    await cmds.snmpCancelWalk();

    S.isExecuting = false;
    S.walkProgress = "";
    S.statusText = wasTableRun
      ? "Table retrieval cancelled"
      : wasSimpleRun
        ? "Cancelled…"
        : "Walk cancelled";
  }

  async function executeOperation(op: SnmpOperation, oid: string) {
    const cfg = S.targetConfig;
    if (!cfg.host) {
      S.statusText = "No target configured";
      return;
    }

    // This run owns the Result Set and status bar from here on; outcomes of
    // any run it supersedes (or that Stop retires) are dropped as stale.
    const myRun = runGuard.start();
    const stale = () => !runGuard.isCurrent(myRun);

    S.isExecuting = true;
    // A Get Next run that starts where the last one ended continues the
    // sequence (appends); every other start replaces the Result Set.
    const isNextContinuation = op === "getNext" && getNextCursor === oid;
    // Get/GetNext never touch the Result Set at run start: the previous
    // result stays on screen while the run is in flight, and a cancelled
    // or retired run — whose outcome is dropped — discards nothing. Only
    // a real outcome commits (replace for fresh runs, append for a
    // continuation). A cancelled GetNext's kept bindings replace the
    // whole set: the engine's sequence is authoritative.
    const commitFresh = (bindings: VariableBinding[]) => {
      if (op === "getNext" && !isNextContinuation) getNextCursor = null;
      if (op === "get" || (op === "getNext" && !isNextContinuation)) {
        S.executionBindings.length = 0;
      }
      S.executionBindings.push(...bindings);
    };
    if (op === "walk" || op === "bulkWalk") {
      S.executionBindings.length = 0;
    }
    S.tableResult = null;
    S.subtreeNodes = null;
    S.walkProgress = "";
    S.queryRootOid = oid;
    isWalkActive = false;
    // Get/GetNext are cancellable through the shared token (08); Set goes
    // through the SetValueEditor dialog, which cancels on close.
    cancellableRun = op === "get" || op === "getNext";

    // Walk/Bulk Walk on a table node behaves like anywhere else — a flat
    // subtree walk. Get Table (executeGetTable) is the only grid path.
    S.statusText = `Starting ${operationLabels[op]} on ${oid}…`;

    try {
      if (op === "get") {
        const cmds = await import("$lib/tauriCommands");
        const result = await cmds.snmpGet(cfg, [oid]);
        if (stale()) return;
        // An empty answer (cancel semantics — the real cancel rejects, 08)
        // must not wipe the pane: keep what is on screen.
        if (result.bindings.length > 0) commitFresh(result.bindings);
        S.executionResults = result;
        markConnected();
        S.statusText = `Get complete: ${pluralize(result.bindings.length, "binding")}`;
      } else if (op === "getNext") {
        const cmds = await import("$lib/tauriCommands");
        const result = await cmds.snmpGetNext(cfg, [oid]);
        if (stale()) return;
        // A cancelled GetNext returns Ok with the bindings kept so far plus a
        // cancelled warning (08) — report the cancel, not a plain completion.
        const nextCancelled = result.warnings?.some((w) => w.kind === "cancelled");
        if (nextCancelled) {
          // Cancel never discards data (08): append the engine's kept
          // bindings to what is on screen, deduped by OID. For a
          // continuation the UI's set is the sequence so far and the kept
          // bindings are this run's steps; for a fresh run the kept
          // bindings belong to the same cursor the UI already rendered,
          // so dedup keeps the screen intact.
          const kept = [
            ...S.executionBindings,
            ...result.bindings.filter((b) => !S.executionBindings.some((p) => p.oid === b.oid)),
          ];
          S.executionBindings.length = 0;
          S.executionBindings.push(...kept);
          S.executionResults = { ...result, bindings: [...kept] };
        } else {
          // Real answer: replace (fresh) or append (continuation).
          if (isNextContinuation) S.executionBindings.push(...result.bindings);
          else commitFresh(result.bindings);
          S.executionResults = result;
        }
        // Advance the address bar and cursor to the returned OID: a subsequent
        // Go with Get Next then starts from here, appending the following OID
        // as another row. A fresh selection (tree click, autocomplete pick, or
        // typed edit) replaces the bar and restarts the sequence.
        const lastOid = result.bindings[result.bindings.length - 1]?.oid;
        if (lastOid) {
          getNextCursor = lastOid;
          inputValue = `${lastOid}  ${displayNameForOid(lastOid)}`;
        } else {
          getNextCursor = null;
        }
        markConnected();
        S.statusText = nextCancelled
          ? `Get Next cancelled: ${pluralize(S.executionBindings.length, "binding")} kept`
          : `Get Next complete: ${pluralize(result.bindings.length, "binding")}`;
      } else if (op === "walk" || op === "bulkWalk") {
        const cmds = await import("$lib/tauriCommands");
        let count = 0;
        isWalkActive = true;
        // First streamed binding proves the target answered — flip the
        // indicator early instead of waiting for completion on long walks.
        let sawData = false;

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
            if (!sawData && batch.length > 0) {
              sawData = true;
              markConnected();
            }
            count += batch.length;
            buffer.push(...batch);
            S.walkProgress = pluralize(count, "binding");
            S.statusText = `${operationLabels[op]}: ${pluralize(count, "binding")}…`;
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
            // A retired walk's late completion must not touch the Result
            // Set the user has moved on to.
            if (!stale()) S.executionResults = result;
            S.walkProgress = "";
            const outcome = streamingOutcome(count, result.partial, hasErrorWarning(result.warnings));
            if (outcome === "connected") markConnected();
            else if (outcome === "disconnected") markDisconnected();
            S.statusText = `${operationLabels[op]} complete: ${pluralize(count, "binding")}`;
          }
        );
      }
    } catch (err) {
      if (stale()) return;
      console.error("SNMP operation failed:", err);
      isWalkActive = false;
      const msg = String(err);
      // A cancelled Get/Set arrives as a rejection (task abort or engine
      // token check, 08) — a user action, not a communication failure.
      if (msg.includes("Cancelled by user")) {
        S.statusText = `${operationLabels[op]} cancelled`;
      } else {
        // Get/GetNext reject only when the backend operation fails — a
        // communication problem. Walk/BulkWalk report failures via their
        // complete channel instead, so an invoke rejection there is a local
        // validation error (e.g. BulkWalk on v1) and leaves state untouched.
        if (op === "get" || op === "getNext") markDisconnected();
        S.statusText = `Error: ${msg}`;
        // The failed run's answer is the empty set — commit it as a replace
        // (fresh) instead of clearing the Result Set at run start.
        if (op === "get" || (op === "getNext" && !isNextContinuation)) {
          commitFresh([]);
          S.executionResults = { bindings: [], partial: true, warnings: [{ kind: "error", message: msg }] };
        }
      }
    } finally {
      // Only the current run may release the run flags: a retired run's
      // late rejection must not clear the flags of the run that replaced
      // it (that hides the Stop button mid-run).
      if (!stale()) {
        S.isExecuting = false;
        cancellableRun = false;
      }
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
    S.statusText = `Detecting table columns for ${name}…`;

    try {
      const allColumns = await cmds.mibTableColumns(oid);
      if (allColumns.length === 0) {
        tableRunActive = false;
        S.statusText = `No columns found for table ${name}`;
        return;
      }

      S.statusText = `Fetching table ${name}…`;
      isWalkActive = true;
      // First progress tick proves the target answered — flip the indicator
      // early instead of waiting for completion on large tables.
      let sawData = false;

      await cmds.snmpGetTable(cfg, oid, allColumns,
        (count: number) => {
          if (!isWalkActive) return;
          if (!sawData && count > 0) {
            sawData = true;
            markConnected();
          }
          S.walkProgress = pluralize(count, "binding");
          S.statusText = `Get Table: ${pluralize(count, "binding")}…`;
        },
        (result: TableResult) => {
          if (!isWalkActive) return;
          isWalkActive = false;
          tableRunActive = false;
          S.tableResult = result;
          S.walkProgress = "";
          const outcome = streamingOutcome(result.total_rows, result.partial, hasErrorWarning(result.warnings));
          if (outcome === "connected") markConnected();
          else if (outcome === "disconnected") markDisconnected();
          let msg = `Table complete: ${pluralize(result.total_rows, "row")}, ${pluralize(result.columns.length, "column")}`;
          if (result.missing_cells > 0) {
            msg += ` (${result.missing_cells} missing ${result.missing_cells === 1 ? "cell" : "cells"})`;
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
    S.statusText = `Loading subtree for ${oid}…`;

    try {
      const cmds = await import("$lib/tauriCommands");
      const nodes = await cmds.mibSubtree(oid);
      S.subtreeNodes = nodes;
      S.statusText = `Get Subtree complete: ${pluralize(nodes.length, "node")} under ${oid}`;
    } catch (err) {
      console.error("Subtree retrieval failed:", err);
      S.subtreeNodes = [];
      S.statusText = `Subtree error: ${err}`;
    } finally {
      S.isExecuting = false;
    }
  }

  /** Set entry point (address bar): resolve node metadata, run the guard,
   *  apply the scalar .0 fixup, and open the shared Set value dialog. */
  async function openSetValueDialog(oid: string) {
    let resolved: MibNodeDetails | null = null;
    try {
      resolved = await mibNodeDetails(oid);
    } catch (err) {
      console.error("Set node lookup failed:", err);
    }
    // An ancestor-subtree resolution means the object itself is unknown.
    const details = setDetailsFor(oid, resolved);

    const guard = setGuard(oid, details);
    if (guard !== null) {
      S.statusText = guard;
      return;
    }

    S.setValueTarget = {
      oid: effectiveSetOid(oid, details),
      name: details?.name,
      details,
    };
  }

  let oidInputEl: HTMLInputElement;

  // While the dropdown is visible, a press anywhere except the OID input and
  // the result list dismisses it. (The old handler only fired on clicks
  // landing in the bar's own padding, so clicking elsewhere left it stuck open.)
  // mousedown covers real pointer use; click covers synthetic drivers that
  // dispatch click without a preceding mousedown.
  $effect(() => {
    if (!results.length) return;
    function onDocPress(e: MouseEvent) {
      const target = e.target as HTMLElement;
      if (target === oidInputEl || target.closest("[data-testid='autocomplete-list']")) return;
      hideAutocomplete();
    }
    document.addEventListener("mousedown", onDocPress);
    document.addEventListener("click", onDocPress);
    return () => {
      document.removeEventListener("mousedown", onDocPress);
      document.removeEventListener("click", onDocPress);
    };
  });
</script>

<div data-address-bar class="flex items-center gap-2 px-4 py-2 bg-base-200 border-b border-base-300 flex-shrink-0 relative">
  <label for="target-host" class="text-xs font-medium text-base-content/60 whitespace-nowrap">Target</label>

  <div class="join">
    <input
      id="target-host"
      data-testid="host-input"
      type="text"
      placeholder="Host or IP"
      value={cfg.host}
      oninput={onHostInput}
      class="input input-bordered input-sm w-40 font-mono join-item"
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
      class="input input-bordered input-sm w-16 font-mono join-item text-center"
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
    placeholder="OID or MIB name"
    autocomplete="off"
    bind:this={oidInputEl}
    class="flex-1 input input-bordered input-sm font-mono"
    bind:value={inputValue}
    oninput={onInput}
    onkeydown={onKeyDown}
  />

  <select
    data-testid="op-select"
    aria-label="SNMP operation"
    class="select select-bordered select-sm w-24"
    bind:value={S.snmpOperation}
  >
    {#each operations as op (op)}
      <option value={op}>{operationLabels[op]}</option>
    {/each}
  </select>

  {#if isWalkActive || cancellableRun}
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
      {executing ? "…" : "Go"}
    </button>
  {/if}

  {#if results.length > 0}
    <div data-testid="autocomplete-list" class="absolute top-full left-4 right-50 bg-base-100 border border-base-300 rounded-box max-h-60 overflow-y-auto z-[500] shadow-lg">
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
