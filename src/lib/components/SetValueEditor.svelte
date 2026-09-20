<script lang="ts">
  import { TriangleAlert } from "lucide-svelte";
  import { S, mergeSetIntoResultSet } from "$lib/stores.svelte";
  import { snmpSet, mibSearch } from "$lib/tauriCommands";
  import { markConnected, markDisconnected } from "$lib/connectionLogic";
  import * as setL from "$lib/setLogic";
  import { valueDisplay } from "$lib/export";
  import type { SnmpValue } from "$lib/types";

  interface Props {
    target: NonNullable<(typeof S)["setValueTarget"]>;
    onClose: () => void;
  }
  let { target, onClose }: Props = $props();

  // An ancestor-subtree resolution means the object itself is unknown — the
  // type picker is the honest editor for it.
  const details = setL.setDetailsFor(target.oid, target.details);
  const syntax = details?.syntaxType;
  const wireType = setL.mibSyntaxToSetValueType(syntax);

  // SYNTAX labels with a dedicated editor; anything else (or no details)
  // falls back to the type picker.
  const RECOGNIZED_SYNTAX = new Set([
    "Integer32",
    "TruthValue",
    "Counter32",
    "Counter64",
    "Gauge32",
    "Unsigned32",
    "TimeTicks",
    "IpAddress",
    "ObjectIdentifier",
    "OctetString",
    "BITS",
  ]);

  type Mode =
    | "integer"
    | "unsigned"
    | "counter64"
    | "octetstring"
    | "ipaddress"
    | "oid"
    | "bits"
    | "picker";

  let mode: Mode = $derived.by(() => {
    if (!details || !RECOGNIZED_SYNTAX.has(syntax ?? "")) return "picker";
    switch (wireType) {
      case "integer":
        return "integer";
      case "counter32":
      case "gauge32":
      case "timeticks":
        return "unsigned";
      case "counter64":
        return "counter64";
      case "ipaddress":
        return "ipaddress";
      case "objectidentifier":
        return "oid";
      default:
        return syntax === "BITS" && (details.bits?.length ?? 0) > 0 ? "bits" : "octetstring";
    }
  });

  const CUSTOM = "__custom__";
  let text = $state("");
  let hexMode = $state(false);
  let enumChoice = $state("");
  let truthValue = $state("1");
  let pickerType = $state<setL.SetValueType>("integer");
  let checkedBits = $state<Set<number>>(new Set());
  let submitting = $state(false);
  let errorText = $state<string | null>(null);
  let resolvedOidHint: string | null = null;

  // Prefill order: live value → DEFVAL → empty. The keyed remount per target
  // means this runs once per opened dialog.
  const initialPrefill = (() => {
    if (target.currentRaw !== undefined) {
      const p = setL.prefillFromValue(target.currentRaw, syntax);
      if (p.text !== "") return p;
    }
    if (target.currentValue !== undefined && target.currentValue !== "") {
      return { text: target.currentValue, hex: false };
    }
    return setL.defvalPrefill(details, syntax) ?? { text: "", hex: false };
  })();
  text = initialPrefill.text;
  hexMode = initialPrefill.hex;

  if (mode === "integer" && syntax !== "TruthValue" && details?.enums && details.enums.length > 0) {
    const hit = details.enums.find((e) => String(e.value) === text);
    enumChoice = hit ? String(hit.value) : text !== "" ? CUSTOM : "";
  }
  if (mode === "integer" && syntax === "TruthValue") {
    truthValue = text === "0" ? "0" : "1";
  }
  if (mode === "bits" && target.currentRaw !== undefined && typeof target.currentRaw === "object" && "OctetString" in target.currentRaw) {
    checkedBits = new Set(setL.bytesToBits(target.currentRaw.OctetString));
  }

  const intRange = setL.parseIntegerRange(details?.constraints);
  const sizeBounds = setL.parseSizeBounds(details?.constraints);

  let validationError = $derived.by<string | null>(() => {
    if (submitting) return null;
    switch (mode) {
      case "integer": {
        if (syntax === "TruthValue") return null;
        const hasEnums = (details?.enums?.length ?? 0) > 0;
        if (hasEnums && enumChoice !== CUSTOM && enumChoice !== "") return null;
        return setL.validateInteger(text, intRange);
      }
      case "unsigned":
        return setL.validateUnsigned32(text, intRange);
      case "counter64":
        return setL.validateCounter64(text);
      case "octetstring":
        return setL.validateOctetString(text, sizeBounds, hexMode);
      case "ipaddress":
        return setL.validateIpv4(text);
      case "oid": {
        const t = text.trim();
        if (!t) return "Enter a dotted OID or MIB object name";
        if (setL.isValidOidString(t)) return null;
        return resolvedOidHint ? null : "Not a valid dotted OID or known MIB object name";
      }
      case "bits":
        return null;
      case "picker": {
        const t = text.trim();
        switch (pickerType) {
          case "integer":
            return setL.validateInteger(t);
          case "counter32":
          case "gauge32":
          case "timeticks":
            return setL.validateUnsigned32(t);
          case "counter64":
            return setL.validateCounter64(t);
          case "octetstring":
            return setL.validateOctetString(t, undefined, hexMode);
          case "ipaddress":
            return setL.validateIpv4(t);
          case "objectidentifier":
            return setL.isValidOidString(t) ? null : "Enter a dotted OID";
        }
      }
    }
  });

  let hintLine = $derived.by(() => {
    if (!details) return null;
    const parts: string[] = [];
    if (details.constraints) parts.push(`constraints ${details.constraints}`);
    if (details.defaultValue) parts.push(`DEFVAL ${details.defaultValue}`);
    if (details.units) parts.push(details.units);
    return parts.length > 0 ? parts.join(" · ") : null;
  });

  // MIB object names resolve exactly, like the address bar does.
  let oidSearchTimer: ReturnType<typeof setTimeout> | null = null;
  $effect(() => {
    const t = text.trim();
    if (mode !== "oid" || !t || setL.isValidOidString(t)) {
      resolvedOidHint = null;
      return;
    }
    if (oidSearchTimer) clearTimeout(oidSearchTimer);
    oidSearchTimer = setTimeout(async () => {
      try {
        const res = await mibSearch(t);
        const exact = res.find((r) => r.name.toLowerCase() === t.toLowerCase());
        resolvedOidHint = exact?.oid ?? null;
      } catch (err) {
        console.error("OID name resolution failed:", err);
        resolvedOidHint = null;
      }
    }, 200);
  });

  function toggleBit(pos: number) {
    const next = new Set(checkedBits);
    if (next.has(pos)) next.delete(pos);
    else next.add(pos);
    checkedBits = next;
  }

  function numericWire(v: number): SnmpValue {
    switch (wireType) {
      case "counter32":
        return { Counter32: v };
      case "timeticks":
        return { TimeTicks: v };
      default:
        return { Unsigned: v };
    }
  }

  function pickerWire(type: setL.SetValueType, v: number): SnmpValue {
    switch (type) {
      case "counter32":
        return { Counter32: v };
      case "counter64":
        return { Counter64: v };
      case "timeticks":
        return { TimeTicks: v };
      default:
        return { Unsigned: v };
    }
  }

  function buildPayload(): { valueType: string; value: unknown; snmpValue: SnmpValue } {
    switch (mode) {
      case "integer": {
        const v =
          syntax === "TruthValue"
            ? Number(truthValue)
            : (details?.enums?.length ?? 0) > 0 && enumChoice !== CUSTOM
              ? Number(enumChoice)
              : Number(text.trim());
        return { valueType: "integer", value: v, snmpValue: { Integer: v } };
      }
      case "unsigned": {
        const v = Number(text.trim());
        return { valueType: wireType, value: v, snmpValue: numericWire(v) };
      }
      case "counter64": {
        const v = Number(text.trim());
        return { valueType: "counter64", value: v, snmpValue: { Counter64: v } };
      }
      case "octetstring": {
        if (hexMode) {
          const bytes = setL.parseHexPairs(text) ?? [];
          return { valueType: "octetstring", value: bytes, snmpValue: { OctetString: bytes } };
        }
        return {
          valueType: "octetstring",
          value: text,
          snmpValue: { OctetString: [...new TextEncoder().encode(text)] },
        };
      }
      case "bits": {
        const bytes = setL.bitsToBytes([...checkedBits]);
        return { valueType: "octetstring", value: bytes, snmpValue: { OctetString: bytes } };
      }
      case "ipaddress": {
        const s = text.trim();
        return { valueType: "ipaddress", value: s, snmpValue: { IpAddress: s } };
      }
      case "oid": {
        const s = resolvedOidHint ?? text.trim();
        return { valueType: "objectidentifier", value: s, snmpValue: { ObjectIdentifier: s } };
      }
      case "picker": {
        const t = text.trim();
        switch (pickerType) {
          case "integer":
            return { valueType: "integer", value: Number(t), snmpValue: { Integer: Number(t) } };
          case "counter32":
          case "gauge32":
          case "timeticks": {
            const v = Number(t);
            return { valueType: pickerType, value: v, snmpValue: pickerWire(pickerType, v) };
          }
          case "counter64":
            return { valueType: "counter64", value: Number(t), snmpValue: { Counter64: Number(t) } };
          case "octetstring": {
            if (hexMode) {
              const bytes = setL.parseHexPairs(t) ?? [];
              return { valueType: "octetstring", value: bytes, snmpValue: { OctetString: bytes } };
            }
            return {
              valueType: "octetstring",
              value: t,
              snmpValue: { OctetString: [...new TextEncoder().encode(t)] },
            };
          }
          case "ipaddress":
            return { valueType: "ipaddress", value: t, snmpValue: { IpAddress: t } };
          case "objectidentifier":
            return { valueType: "objectidentifier", value: t, snmpValue: { ObjectIdentifier: t } };
        }
      }
    }
  }

  async function submit() {
    if (validationError !== null || submitting) return;
    const cfg = S.targetConfig;
    if (!cfg.host) {
      errorText = "No target configured";
      return;
    }

    submitting = true;
    errorText = null;
    S.isExecuting = true;
    S.statusText = `Setting ${target.oid}...`;
    const payload = buildPayload();
    try {
      const result = await snmpSet(cfg, target.oid, payload.valueType, payload.value);
      const pduError = (result.warnings ?? []).find((w) => w.kind === "pdu-error");
      if (pduError) {
        // A definitive agent answer: the dialog stays open with the value
        // preserved for an immediate retry.
        markConnected();
        errorText = pduError.message;
        S.statusText = `Set failed: ${setL.pduErrorName(pduError)}`;
        return;
      }
      // A successful Set response may carry no varbinds (some agents); in
      // that case synthesize the binding from the request so the row still
      // reflects the write. Never adopt a varbind for a different OID.
      const found = result.bindings.find((b) => b.oid === target.oid);
      const binding = found
        ? { oid: target.oid, value: found.value, warning: found.warning }
        : { oid: target.oid, value: payload.snmpValue };
      mergeSetIntoResultSet(target.oid, binding.value, binding.warning, result);
      markConnected();
      S.statusText = `Set complete: ${target.name ?? target.oid} = ${valueDisplay(binding.value)}`;
      onClose();
    } catch (err) {
      console.error("SNMP Set failed:", err);
      markDisconnected();
      errorText = String(err);
      S.statusText = `Set failed: ${err}`;
    } finally {
      S.isExecuting = false;
      submitting = false;
    }
  }
</script>

<div class="flex items-start gap-2 flex-wrap">
  <h3 id="set-value-dialog-title" data-testid="set-dialog-title" class="text-lg font-bold break-all">
    {target.name ?? target.oid}
  </h3>
  {#if syntax}
    <span data-testid="set-syntax-badge" class="badge badge-outline badge-xs font-mono mt-1">{syntax}</span>
  {/if}
</div>
<p data-testid="set-dialog-oid" class="font-mono text-xs break-all mt-1 text-base-content/80">{target.oid}</p>
{#if hintLine}
  <p data-testid="set-hint" class="text-xs text-base-content/60 mt-2">{hintLine}</p>
{/if}

<div class="mt-4">
  {#if mode === "integer"}
    {#if syntax === "TruthValue"}
      <select data-testid="set-truth-select" data-autofocus class="select select-bordered select-sm w-full" bind:value={truthValue}>
        <option value="1">True (1)</option>
        <option value="0">False (0)</option>
      </select>
    {:else if details?.enums && details.enums.length > 0}
      <select data-testid="set-enum-select" data-autofocus class="select select-bordered select-sm w-full" bind:value={enumChoice}>
        {#each details.enums as e (e.value)}
          <option value={String(e.value)}>{e.label} ({e.value})</option>
        {/each}
        <option value={CUSTOM}>Custom…</option>
      </select>
      {#if enumChoice === CUSTOM}
        <input data-testid="set-custom-input" type="text" inputmode="numeric" class="input input-bordered input-sm w-full mt-2 font-mono" bind:value={text} placeholder="Numeric value" />
      {/if}
    {:else}
      <input data-testid="set-int-input" data-autofocus type="text" inputmode="numeric" class="input input-bordered input-sm w-full font-mono" bind:value={text} placeholder="Integer value" />
    {/if}
  {:else if mode === "unsigned" || mode === "counter64"}
    <input data-testid="set-numeric-input" data-autofocus type="text" inputmode="numeric" class="input input-bordered input-sm w-full font-mono" bind:value={text} placeholder="Numeric value" />
  {:else if mode === "octetstring"}
    <div class="flex items-center gap-3">
      <input data-testid="set-octet-input" data-autofocus type="text" class="input input-bordered input-sm flex-1 font-mono" bind:value={text} placeholder={hexMode ? "Byte pairs, e.g. 0a ff" : "Text value"} />
      <label class="flex items-center gap-1.5 text-xs cursor-pointer whitespace-nowrap">
        <input type="checkbox" data-testid="set-hex-toggle" checked={hexMode} onchange={() => (hexMode = !hexMode)} />
        hex
      </label>
    </div>
  {:else if mode === "ipaddress"}
    <input data-testid="set-ip-input" data-autofocus type="text" class="input input-bordered input-sm w-full font-mono" bind:value={text} placeholder="a.b.c.d" />
  {:else if mode === "oid"}
    <input data-testid="set-oid-input" data-autofocus type="text" class="input input-bordered input-sm w-full font-mono" bind:value={text} placeholder="Dotted OID or MIB object name" />
    {#if resolvedOidHint}
      <p data-testid="set-oid-hint" class="text-xs text-base-content/60 mt-1 font-mono break-all">→ {resolvedOidHint}</p>
    {/if}
  {:else if mode === "bits"}
    <div data-testid="set-bits-list" class="space-y-1 max-h-48 overflow-y-auto pr-1">
      {#each details!.bits as b (b.value)}
        <label class="flex items-center gap-2 text-sm cursor-pointer py-0.5">
          <input type="checkbox" checked={checkedBits.has(b.value)} onchange={() => toggleBit(b.value)} />
          <span class="font-mono">{b.label}</span>
          <span class="text-xs text-base-content/60 font-mono">({b.value})</span>
        </label>
      {/each}
    </div>
  {:else}
    <select data-testid="set-type-picker" data-autofocus class="select select-bordered select-sm w-full" bind:value={pickerType}>
      <option value="integer">Integer</option>
      <option value="octetstring">OctetString</option>
      <option value="gauge32">Gauge32 / Unsigned32</option>
      <option value="counter32">Counter32</option>
      <option value="counter64">Counter64</option>
      <option value="ipaddress">IpAddress</option>
      <option value="timeticks">TimeTicks</option>
      <option value="objectidentifier">ObjectIdentifier</option>
    </select>
    <div class="flex items-center gap-3 mt-2">
      <input data-testid="set-picker-input" type="text" class="input input-bordered input-sm flex-1 font-mono" bind:value={text} placeholder="Value" />
      {#if pickerType === "octetstring"}
        <label class="flex items-center gap-1.5 text-xs cursor-pointer whitespace-nowrap">
          <input type="checkbox" data-testid="set-picker-hex-toggle" checked={hexMode} onchange={() => (hexMode = !hexMode)} />
          hex
        </label>
      {/if}
    </div>
  {/if}

  {#if validationError}
    <p data-testid="set-validation-error" class="text-xs text-error mt-2">{validationError}</p>
  {/if}

  {#if errorText}
    <div data-testid="set-error" role="alert" class="alert alert-error text-xs mt-3 gap-2">
      <TriangleAlert class="w-4 h-4 shrink-0" />
      <span class="break-all">{errorText}</span>
    </div>
  {/if}
</div>

<div class="modal-action">
  <button data-testid="set-cancel" class="btn btn-ghost btn-sm" onclick={onClose} disabled={submitting}>Cancel</button>
  <button data-testid="set-submit" class="btn btn-primary btn-sm" onclick={submit} disabled={validationError !== null || submitting}>
    {submitting ? "Setting…" : "Set"}
  </button>
</div>
