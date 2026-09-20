# SNMP Set Capabilities

**Date:** 2026-09-16
**Status:** Draft — pending review
**Branch:** `feat/snmp-set-buildout`

## Purpose

The Operation dropdown already lists **Set**, but selecting it and clicking Go freezes the entire UI: `handleSet` calls `window.prompt()`, which Tauri's webview does not implement, so the JS main thread blocks forever with no way to cancel. Beyond that bug, Set is a first-class SNMP operation in name only — there is no writability gating, no type-aware value editing, no validation against MIB constraints or enums, and agent-side rejections (wrongValue, readOnly, …) are silently swallowed as "Set complete: 0 binding(s)".

This spec makes Set a full capability with three entry points that converge on one shared dialog:

1. **Address bar** — type/select an OID, set the dropdown to Set, click Go → a modal opens where the user enters a value. The editor understands the OID's MIB type (INTEGER with enums, BITS, OCTET STRING with SIZE constraints, IpAddress, ObjectIdentifier, …).
2. **Inspector** — selecting a writable MIB Node shows a **Set value** button that opens the same modal, pre-filled with the live value when one is loaded.
3. **Results** — confirmed-writable result rows (flat list and table grid cells) show a pencil (edit) affordance, and every value cell offers right-click → "Set value…"; both open the same modal, pre-filled with the row's current value; a successful write updates that row in place instead of replacing the Result Set.

This completes the deferred "Set a cell" item from `docs/specs/2026-08-23-snmp-table-support.md` Phase 4 for both flat results and grid cells.

Settled design decisions (proposed, pending review):

- **One dialog, three doors.** All entry points open the same `SetValueDialog`; none of them sends a request directly. The dialog is the only place a Set is composed and confirmed.
- **Writability rule:** an OID is settable unless we *know* it isn't — guarded when MIB access is `read-only`, `not-accessible`, or `accessible-for-notify`. Affordances scale with confidence: the results **pencil** (at-a-glance edit icon) appears only on *confirmed writable* rows; the **right-click context menu** offers "Set value…" whenever we don't know it's read-only — writable *or* unknown, where unknown lands in the dialog's type-picker fallback; the **address bar** path is always available. The MIB claim is a schema hint, not an agent guarantee — the safety net for "MIB says writable but agent refuses" is surfacing the PDU error (G3), never a second confirmation step.
- **Set merges into the Result Set.** A successful write replaces the matching binding in place (or appends it); it never discards the rest of the current results, unlike today's replace-everything behavior.
- **Rejection keeps the dialog open.** When the agent refuses a write (wrongValue, readOnly, …), the dialog stays open with the error shown inline and the entered value preserved for fast retry — closing is the user's choice, not the app's.

## Domain language

Terms from `CONTEXT.md` apply: **Target**, **MIB Node**, **Variable Binding**, **Selection**, **Operation**, **Result Set**, **Execution**.

The **Operation** definition already lists Set ("Walk, BulkWalk, Get, GetNext, Get Table, or Set"); no new operation term is needed. One proposed glossary addition (fold in during Phase 1):

**Writable**:
A MIB Node whose MAX-ACCESS permits writes — `read-write` or `read-create`. Writability is a property of the schema; whether a Target actually accepts a write is decided by the agent and reported as a PDU error.
_Avoid_: Settable (ambiguous with the UI affordance), editable, mutable

## Investigation findings

### What exists today

| Layer | Piece | Location |
|-------|-------|----------|
| Engine | `SnmpEngine::set()` — connect, single-varbind Set PDU, retry/backoff on network errors | `crates/scout-snmp/src/engine.rs:319` |
| Types | `SetValue` enum (Integer, OctetString, Unsigned32, Counter32, Counter64, IpAddress, TimeTicks, ObjectIdentifier) | `crates/scout-snmp/src/types.rs:398` |
| IPC | `snmp_set` Tauri command + `parse_set_value` JSON→SetValue | `src-tauri/src/main.rs:629`, `:784` |
| Frontend wrapper | `snmpSet()` | `src/lib/tauriCommands.ts:196` |
| Frontend flow | `handleGo` → `handleSet` — **blocking `window.prompt()`**, type guessed from tree selection, replaces whole Result Set | `src/lib/components/TargetBar.svelte:213`, `:517` |
| MIB access parsing | `MibNode.access` / `NodeDetails.access` (`"read-only"`, `"read-write"`, …) via mib-rs; fallback parser also extracts MAX-ACCESS | `crates/scout-mib/src/lib.rs:161`, `loader.rs:118`, `fallback.rs:337` |
| Tree nodes | `TreeNode` — **no access field** | `crates/scout-mib/src/lib.rs:296` |
| Inspector | Fetches `mibNodeDetails`, renders Access row; resolves instance OIDs to base nodes for metadata | `src/lib/components/InspectorPane.svelte:99` |
| Results | Flat rows (OID \| Value \| Type) + grid cells, both carry full instance OID and value | `src/lib/components/ResultsPane.svelte:873`, `:711` |
| Scalar addressing | `scalar_instance_oid` appends `.0` for scalar MIB nodes — **applied to Get only** | `src-tauri/src/main.rs:525`, used at `:515` |
| Tests | Mock-agent Set roundtrip (`engine_set_roundtrip`); mock server speaks SET; no e2e Set case | `crates/scout-snmp/tests/engine_mock.rs:475`, `src/mock.rs:211` |

### Gaps

**G1 — The UI lock-up.** `handleSet` calls `window.prompt()` (`TargetBar.svelte:526`). Tauri webviews do not implement `prompt()`, so the call blocks the JS main thread indefinitely: the whole app freezes, the Go button stays enabled (isExecuting is set only after the prompt "returns"), and there is no cancellation path.

**G2 — No writability gating anywhere.** `handleSet` fires for any node — tables, table rows, `not-accessible` index columns, read-only scalars. The MIB parser already captures MAX-ACCESS on `MibNode`/`NodeDetails`, but `TreeNode` doesn't carry it and no UI path consults it.

**G3 — Agent-side Set errors are silently swallowed.** snmp2 returns the response PDU as `Ok(pdu)` even when `error_status != 0` (asyncsession.rs:307); `extract_bindings` (`engine.rs:504`) ignores `pdu.error_status`. A wrongValue rejection therefore reports "Set complete: 0 binding(s)" — a write failure presented as success. No RFC 3416 error-status name mapping exists in the codebase.

**G4 — Value editing is type-blind.** One text prompt for everything: no enum selection, no BITS checkboxes, no constraint validation (ranges, SIZE), no DEFVAL or current-value prefill, no IP/OID format checking. BITS has no path at all — `parse_set_value` has no bits case (BITS must be sent as an Octet String of encoded bits).

**G5 — Set replaces the entire Result Set.** `handleSet` clears `executionBindings` and pushes only the response binding (`TargetBar.svelte:573`). Editing one value out of a 500-row walk destroys the other 499.

**G6 — Type inference breaks for typed OIDs.** The value type comes from `S.selectedNode?.syntaxType` (`TargetBar.svelte:525`) — null when the OID was typed into the bar rather than selected in the tree, so it silently defaults to OctetString (setting sysUpTime would send an octet string).

**G7 — Set lacks the scalar `.0` instance fixup.** `snmp_get` appends `.0` when the OID exactly matches a scalar MIB node (`main.rs:515`); `snmp_set` does not. Selecting `sysName` in the tree and setting it sends the bare type OID, which agents reject (noSuchInstance/wrongInstance).

## Design

### Phase 1 — Writability metadata + PDU error surfacing

**`scout-mib`:** add `access: Option<String>` to `TreeNode` (`skip_serializing_if = Option::is_none`, matching the struct's existing style) and populate it from `MibNode.access` wherever tree nodes are built (shallow build + children). Frontend `TreeNode` type in `src/lib/types.ts` gains `access?: string`. The fallback parser already extracts MAX-ACCESS, so both loaders get this for free.

**`scout-snmp`:** check the PDU error status where bindings are extracted:

- In `extract_bindings`, when `pdu.error_status != 0`, emit one `SnmpWarning { kind: "pdu-error", message: "<name> (status <n>) at varbind <error_index>" }` and mark the set partial. Name mapping per RFC 3416's PDU ASN.1 (`noError(0) tooBig(1) noSuchName(2) badValue(3) readOnly(4) genErr(5) noAccess(6) wrongType(7) wrongLength(8) wrongEncoding(9) wrongValue(10) noCreation(11) inconsistentValue(12) resourceUnavailable(13) commitFailed(14) undoFailed(15) authorizationError(16) notWritable(17) inconsistentName(18)`; anything else is `unknown`). Note: `noSuchObject`/`noInstance`/`endOfMibView` are *varbind* exception values, not PDU error-status codes. (v1's low numbers reuse the same codes with v1 meanings; the message includes the raw number so nothing is ever uninterpretable.)
- A PDU error is a definitive agent answer — **not** retryable. `set()` must return it immediately instead of burning the 3-attempt backoff loop on a deterministic rejection. (Get/Walk paths also benefit: an error PDU mid-walk currently ends the walk silently; now it lands in warnings, consistent with the tolerance principle.)

**Frontend:** new pure module `src/lib/setLogic.ts` (vitest-tested, same shape as `connectionLogic.ts`):

- `isWritable(access: string | undefined): boolean` — true for `read-write`, `read-create`; false for `read-only`, `not-accessible`, `accessible-for-notify`; **unknown** (`undefined`) is *not* false (see Writability rule).
- `setGuard(oid, details): string | null` — returns a status-bar message when Set must be refused: table node ("X is a table — Set targets a column instance"), row entry, or confirmed non-writable access ("sysDescr is read-only"). Null = proceed.
- `effectiveSetOid(oid, details): string` — mirrors `scalar_instance_oid`: when the OID exactly matches a scalar MIB node (not Table/TableRow/ObjectIdentifier), append `.0`. Frontend-owned so the dialog can display the exact instance that will be written (G7); the backend applies the same fixup in `snmp_set` as defense in depth.
- Per-type value validation + parsing (see Phase 2 for editors): integer range from constraints text (`"1..255"`), SIZE bounds, IPv4 format, dotted-OID format, hex-pair parsing, BITS checkboxes → octet-string bytes.

### Phase 2 — `SetValueDialog`

New component `src/lib/components/SetValueDialog.svelte`, following the established modal pattern exactly (always mounted in `AppShell.svelte` beside ConnectionModal/ManageMibsDialog/HexViewModal; gated by a store flag; native `<dialog>` + daisyUI `modal modal-open`; shared focus trap). Store gains:

```ts
setValueTarget: null as {
  oid: string;            // effective instance OID (after .0 fixup)
  name?: string;          // MIB node name, when known
  details: MibNodeDetails | null;   // null = not in loaded MIBs
  currentValue?: string;  // live value text, when opened from results/inspector
} | null
```

**Layout:**

- Header: node name (or OID), syntax-type badge, and a hint line with constraints / DEFVAL / units when present.
- Body — one editor per MIB type, driven by `details`:
  - **Integer32 / TruthValue** — named-value `<select>` when the node has enums (value → label), with a trailing "Custom…" option revealing a free numeric input; plain numeric input otherwise. TruthValue renders as True/False select. Range-validated from constraints.
  - **Counter32 / Gauge32 / Unsigned32 / TimeTicks** — numeric input, 0..u32 max, constraint range shown as hint and enforced.
  - **OctetString** — text input; SIZE (min..max) validated; a "hex" toggle switches the field to space-separated byte pairs for binary values (MACs, opaque data).
  - **IpAddress** — text input with strict IPv4 validation (`a.b.c.d`, octets 0–255).
  - **ObjectIdentifier** — text input accepting a dotted OID or a MIB object name; names resolve via `mibSearch` (exact match, as the address bar does) and the resolved OID is shown as a hint before submit.
  - **Bits** — checkbox list of named bits (position + label); encodes to octet-string bytes for wire transmission. Nodes typed Bits without bit metadata fall back to the OctetString editor.
  - **Unknown** (`details === null` or unrecognized syntax) — an explicit type picker (the eight `SetValue` types) plus a matching generic input. This is the escape hatch for OIDs not present in loaded MIBs; it makes the dialog honest about what it will send.
- Prefill order: `currentValue` (results row / inspector live value) → DEFVAL when it parses cleanly for the type → empty.
- Footer: **Cancel** and **Set**. Set is disabled while the value fails validation, with the specific problem shown inline under the field (e.g. "must be 1..255", "not a valid IPv4 address"). No request fires on an invalid value — client-side validation is the first gate; the agent's PDU error is the second.

On submit: `snmpSet(cfg, oid, valueType, parsedValue)` with the frontend→backend type mapping below; status bar shows `Setting {oid}…`; `isExecuting` true while in flight.

| MIB syntax | Sent as (`valueType`) |
|------------|----------------------|
| Integer32, TruthValue | `Integer` (0/1 for TruthValue) |
| Counter32 | `Counter32` |
| Counter64 | `Counter64` |
| Gauge32, Unsigned32 | `Gauge32` (→ `SetValue::Unsigned32`) |
| TimeTicks | `TimeTicks` |
| IpAddress | `IpAddress` |
| ObjectIdentifier | `ObjectIdentifier` |
| OctetString, Bits, Unknown | `OctetString` (Bits as encoded bytes) |

This mapping is the single source of truth in `setLogic.ts`, replacing the ad-hoc switch inside today's `handleSet`.

### Phase 3 — Wire the three entry points + Result Set merge

**Entry 1 — Address bar.** The `set` branch of `handleGo` becomes: resolve node metadata via `mibNodeDetails(effectiveOid)` (instance OIDs resolve to their base node, same as the Inspector) → run `setGuard` (table / read-only / not-accessible → status message, no dialog, mirroring the existing table guard at `TargetBar.svelte:232`) → compute `effectiveSetOid` → open the dialog. The old `handleSet` (prompt + type switch + replace-everything) is deleted.

**Entry 2 — Inspector.** A **Set value** button in the identity block, rendered only when access is *confirmed writable* (hidden for read-only / not-accessible / tables / rows and for nodes whose access we couldn't determine — the context menu and address bar remain available for those). Opens the dialog with the live value pre-filled when one is loaded.

**Entry 3 — Results.** Two affordances on value cells (flat rows and grid cells):
- **Pencil icon** (at-a-glance): rendered only when writability is *confirmed* (`read-write`/`read-create`). Writability is resolved lazily per distinct base OID via `mibNodeDetails` and cached in a local map for the lifetime of the Result Set (a 500-row walk of one table = one lookup per column, not per row).
- **Right-click context menu**: today a value cell's right-click shows a menu only for byte values (single "Hex View" item, `ResultsPane.svelte:223`); extend it to show on every value cell with two items — **"Set value…"** (omitted when the base node is confirmed non-writable; unknown access keeps it, landing in the dialog's type-picker fallback) and **"Hex View"** (byte values only, preserving current behavior). The menu target already carries the instance OID + `SnmpValue`, so the dialog opens pre-filled.
- Grid cells (Get Table): same two affordances; the instance OID is column OID + row instance suffix — this is the "Set a cell" item deferred by the table-support spec.
- Clicking the pencil (or choosing the menu item) opens the dialog with the row's current value pre-filled; the row click-to-inspect behavior is preserved (the pencil stops propagation, like the existing hex-view button).

**Result Set merge.** After a successful Set: replace the binding whose OID matches in `S.executionBindings` (value + warning updated from the response); append if absent. If the agent's response carries no varbinds (some agents do), synthesize the binding from the request (OID + sent value) so the row still reflects the write. Warnings from the response join the Result Set's warnings; status bar: `Set complete: {name} = {value}`.

**Agent errors.** On a PDU-level rejection the dialog **stays open**: an inline error box shows the RFC 3416 name ("wrongValue (status 10) at varbind 0"), the value field keeps what was entered, and Set is re-enabled for immediate retry. The status bar mirrors the failure (`Set failed: wrongValue`); no binding changes, so the Result Set and its warnings are untouched — the previous value stays in place. The user closes the dialog (Cancel/Esc/backdrop) to give up; nothing is added to the warnings banner while the dialog owns the error.

### Out of scope

Multi-varbind atomic Sets (v2c/v3 allow several varbinds per PDU; agents vary — one binding per request is sufficient for v1), read-create row creation via index-column writes, SET from context menus, and any undo/rollback concepting. Notifications/traps remain out of scope as before.

## Test plan

**`scout-mib` unit tests**
- Extend the `TABLE-TEST-MIB` fixture pattern with a `read-write` scalar (and a `read-create` one); assert `TreeNode.access` is populated through both shallow build and children, and stays `None` when the MIB omits MAX-ACCESS.

**`scout-snmp` tests**
- Mock server: add a canned-response mode that answers a Set PDU with a non-zero error status (e.g. 10 wrongValue, error-index 0). `engine.set()` must return immediately (no backoff sleep) with one `pdu-error` warning naming "wrongValue" and the raw status; bindings empty; partial true.
- Existing `engine_set_roundtrip` stays green (success path unchanged).

**Frontend unit tests (`setLogic.test.ts`, vitest)**
- `isWritable`: read-write/read-create true; read-only/not-accessible/accessible-for-notify false; undefined unknown (not false).
- `setGuard`: table, row entry, read-only each produce a message; writable scalar and unknown node produce null.
- `effectiveSetOid`: exact scalar match appends `.0`; Table/TableRow/ObjectIdentifier and already-suffixed OIDs pass through.
- Validation: integer range from `"1..255"` constraints (bounds inclusive, out-of-range rejected), SIZE bounds on octet strings, IPv4 accept/reject cases, dotted-OID format, hex-pair parsing (odd digit count, non-hex chars), BITS checkboxes → bytes (bit ordering per RFC 1065: first bit = MSB of first octet).
- Prefill conversion: SnmpValue → editor text per type.

**E2E (`test/specs/set.spec.ts`, new)**
- New synthetic recording `test/snmprec/set-capable.snmprec` (second snmpsim instance, same config-isolation pattern as the table spec): a read-write scalar with a recorded successful Set echo, and a recorded wrongValue error response for a read-only object. If hand-authoring `.snmprec` proves brittle (known risk from the table spec), record against a local agent instead.
- Cases:
  - Go → Set on a writable OID opens the dialog; entering a valid value and submitting updates the result row in place with the written value; status reads "Set complete".
  - Enum editor: a named-value select is offered for an INTEGER-with-enums node; choosing a name sends its numeric value.
  - Go → Set on a read-only OID shows the guard status, opens no dialog, fires no request.
  - Inspector: Set button visible on a writable node, absent on a read-only one; opens pre-filled from the live value.
  - Results pencil: present on confirmed-writable rows, absent on confirmed read-only *and* unknown-access rows; opens pre-filled; after submit, sibling rows are preserved (G5 regression).
  - Right-click context menu: "Set value…" present on writable and unknown-access rows, absent on confirmed read-only rows; right-clicking a byte value still offers "Hex View"; choosing "Set value…" on an unknown-access row opens the dialog in type-picker mode.
  - Agent rejection: setting a value the recorded agent rejects keeps the dialog open with an inline "wrongValue" error and the entered value preserved (G3 regression); submitting a second, accepted value then succeeds and updates the row; the previous value was in the Result Set throughout.

## Risks and mitigations

| Risk | Mitigation |
|------|------------|
| snmpsim replay of Set requests requires the recording to contain matching request varbinds; hand-authoring `.snmprec` is undocumented territory | Same approach as the table spec: verify against the bundled recording first; fall back to recording from a live local agent. Mock-server tests cover the error path deterministically regardless. |
| v1 vs v3 error-status numbers collide (v1 status 2 = noSuchName, v3 status 2 = badValue) | Message always includes the raw status number; mapping table documents both. The app already knows the Target's version, so the right name can be chosen per version. |
| MIB access metadata is a schema claim — vendor MIBs may mislabel, and agents enforce their own rules | Writability only gates the UI affordance; PDU error surfacing (G3) is the real safety net. Unknown access never blocks. |
| Fallback-parser MIBs have sparser access data | Missing access = unknown → dialog still reachable via address bar and right-click context menu with the type-picker fallback; pencil and inspector button shown only on confirmed writable. |
| `TreeNode` payload grows (access string per node) | Negligible: short strings, omitted when absent (`skip_serializing_if`). |
| Lazy writability lookups in ResultsPane add IPC calls for large walks | Deduplicated per base OID and cached per Result Set; one lookup per column of a table, not per row. |

## Definition of done

1. `window.prompt()` is gone; Set never blocks the UI (G1).
2. All three entry points open the single type-aware `SetValueDialog` with per-type editors, constraint/enum/bits validation, and prefill from live value or DEFVAL (G4).
3. Writability gating follows the settled rule: confirmed read-only / not-accessible / tables / rows are guarded in the address bar path and hide their Set affordances in the Inspector, results pencil, and context menu; unknown access keeps the context-menu and address-bar paths open with the type-picker fallback (G2).
4. Agent PDU errors surface by RFC 3416 name with raw status and error index, without retry backoff; "Set complete: 0 binding(s)" on a rejection is impossible (G3). On rejection the dialog stays open with the inline error and value preserved for retry.
5. Set response merges into the existing Result Set — matching binding replaced in place, siblings preserved (G5); typed OIDs get correct types from MIB metadata, not a tree-selection side channel (G6); scalar nodes are addressed at their `.0` instance (G7).
6. `TreeNode.access` flows end to end (loader → tree command → frontend type); CONTEXT.md gains **Writable**.
7. Pre-commit checklist green: `cargo fmt`, `cargo test --workspace --all-features`, `npx tsc --noEmit`, `npx svelte-check --threshold warning`; full e2e suite (`npm run test:e2e`) green including the new set spec.

## Open questions

Settled during review (2026-09-16): dialog stays open on agent rejection with the value preserved for retry; the results pencil appears only on confirmed-writable rows, while right-click → "Set value…" remains available for unknown-access rows (type-picker fallback), since agents commonly expose more than the loaded MIBs describe.

Remaining:

- Multi-varbind atomic Set (e.g. set an interface's adminStatus + description in one PDU) — worth a follow-up spec, or YAGNI?
