# Backend Crate Workspace Split

**Date:** 2026-08-20
**Status:** Implemented (2026-08-21) on branch `crate-workspace-split` from `main`

## Implementation Notes (2026-08-21)

- **MockSnmpServer needed a protocol rewrite, not just wiring.** Its hand-rolled BER handling could never interoperate with a real snmp2 client: response PDU tag was 0xA1 (GetNextRequest) instead of 0xA2 (Response), the request-id was hardcoded instead of echoed, the v2c tag table was wrong (Set is 0xA3, GetBulk 0xA5), and fixed-offset parsing broke on BulkPDU (which has no error-status/error-index fields) and variable-width INTEGERs. It now walks proper TLVs per RFC 3416, with parser unit tests driven by datagrams captured from a real snmp2 client.
- **The stack-size concern is confirmed empirically:** the new engine integration tests overflow the default 2MB test-thread stack when run directly, and pass only when spawned on an 8MB-stack runtime — exactly the failure mode `main.rs:289` documented. Tests mirror the app's runtime configuration for this reason.
- **Remaining verification (needs a GUI host):** e2e (`npm run test:e2e`) and a manual `npm run dev` walk of the streaming path. Everything else in the pre-commit checklist passes.

## Context / Why This Shape

The design spec (2026-07-15) promises a "pure Rust crate with zero UI dependency, fully testable independently." Today `lib.rs` claims this but the claim is violated: `snmp/engine.rs:75-76` takes `tauri::ipc::Channel` directly. Splitting the backend into a Cargo workspace makes the boundary structural — the pure crates *cannot* import tauri, so the coupling becomes a compile error instead of a TODO.

Findings that justify the split:

- `mib/` (~1.9k LOC) and `snmp/` (~2.5k LOC) have **zero cross-dependencies** — `main.rs` is the only place they meet. The seam already exists; this is not an artificial boundary.
- Exactly two impurities keep `snmp/` from being tauri-free: the `Channel` params in `engine.rs` and a private tokio runtime + `block_on` (code-review items 3–4). Everything else (`tolerant`, `table`, `types`, `mock`) is already pure.
- The dead `MockSnmpServer` (613 lines, code-review item 2) lives in the snmp module; once it sits in a pure crate with a pure async API, wiring it into real engine tests falls out naturally.

Runtime situation (worse than the code review noted) — **three** runtimes exist today:

1. The engine's private 2-thread multi-thread runtime (`engine.rs:31`), used via `block_on` for get/getnext/set/walk-table and `spawn` for streaming walks.
2. An ad-hoc OS thread + current-thread runtime in `snmp_connect` (`main.rs:289-299`) with a deliberate 8MB stack, because snmp2's connection code can recurse deeply and overflow default tokio worker stacks (2MB).
3. Tauri's own runtime, on which async commands execute.

The plan consolidates to **one app-owned runtime** with 8MB worker stacks. This preserves the documented stack-size safety while moving runtime ownership out of the pure crate. (Naively awaiting engine calls on Tauri's 2MB-stack workers would reintroduce the overflow the comment at `main.rs:289` warns about.)

## Target Layout

```
Cargo.toml            # workspace (already exists) — members += crates/*
crates/scout-mib/     # mib/{mod,loader,fallback}.rs — deps: mib-rs, walkdir, regex, serde, tracing
crates/scout-snmp/    # snmp/{mod,engine,tolerant,table,types,mock}.rs — deps: snmp2, tokio, serde, tracing
src-tauri/            # app crate: main.rs commands, config.rs, log.rs — depends on both
```

## Key Design Decisions

1. **`WalkBatchSender` trait** (in `scout-snmp`) replaces `Option<tauri::ipc::Channel>`:

   ```rust
   pub trait WalkBatchSender: Send + Sync {
       fn send_binding(&self, binding: &VariableBinding);
       fn send_complete(&self, result: &ResultSet);
   }
   ```

   Typed values (serde) instead of pre-serialized strings. The app-side adapter does `serde_json::to_string` + `Channel::send`, which also eliminates the silent `unwrap_or_default()` serialization failure (code-review item 6).

   *Approved deviation:* per-binding `send_binding` rather than a `Vec`-based `WalkBatch`. The frontend receives one `VariableBinding` per channel message (`src/lib/tauriCommands.ts:158`); serializing a batch as an array would change the wire format and force frontend changes. Per-binding sends keep the wire byte-identical while still gaining typed values and surfaced serialization errors.

2. **Engine becomes a pure async API**: `SnmpEngine` methods become `pub async fn`s; streaming fns take `&tokio::runtime::Handle` + `Arc<dyn WalkBatchSender>` and return `tokio::task::JoinHandle<()>`. The runtime itself moves to app-crate state (`SnmpEngineState` → holds `Arc<Runtime>` with 8MB worker stacks).

3. **Sync Tauri commands** (`snmp_get`, `snmp_get_next`, `snmp_set`, `snmp_walk_table`) become `async` commands. Frontend `invoke()` is unaffected — command names and signatures are preserved.

   *Approved deviation:* they do not `.await` engine calls directly on Tauri's runtime. They spawn the engine work onto the app-owned 8MB-stack runtime (`handle.spawn(...).await`). Tauri's own workers run with default 2MB stacks, so direct awaits would reintroduce exactly the snmp2 recursion overflow the comment at `main.rs:289` warns about (see decision 2's rationale).

4. **`MockSnmpServer`** moves into `scout-snmp` (stays public) and gets wired into real engine integration tests (code-review item 2).

5. **Vestigial `lib.rs`** (`pub mod mib; pub mod snmp;`) is deleted once the modules move.

## Steps (each step leaves the tree green)

1. **Move `mib/` → `crates/scout-mib`** — pure relocation, path fixes, workspace member, app dependency. Zero behavior change.
2. **Decouple + move `snmp/` → `crates/scout-snmp`** — introduce `WalkBatchSender`, async-ify the engine, take a `Handle` param. (Inseparable from the move: the crate cannot import tauri.) All 81 existing unit tests must pass unchanged. Also: drop the unused `futures` dependency (verified no usages in src/), and un-gate `mock.rs` (`#[cfg(test)]` → regular public module) so step 4's integration tests can use it.
3. **App-crate rewiring** — app-owned runtime state, `Channel → WalkBatchSender` adapter, sync→async command conversion, delete `lib.rs`. Verify with the full pre-commit checklist plus e2e (`npm run test:e2e`) — the streaming path is the riskiest change.
4. **Engine integration tests via mock** — start `MockSnmpServer` on an ephemeral localhost port; cover get, walk, bulkwalk, and cancellation. Gate behind a cargo feature if CI proves flaky with localhost UDP. Also move the existing `src-tauri/tests/snmpsim_integration.rs` → `crates/scout-snmp/tests/` — it uses `snmp2` directly (no app-crate imports) and would break once snmp2 leaves the app crate.
5. **Housekeeping** — CI paths (`cargo fmt/clippy/test --manifest-path src-tauri/Cargo.toml` → `--workspace`, in `.github/workflows/ci.yml:42,45,48`), same fix in `scripts/check.sh:8` and the `test:e2e:build` script in package.json, update the AGENTS.md pre-commit checklist to match (`cargo test --lib` → `cargo test --workspace --lib`), trim now-unused deps from the app Cargo.toml (mib-rs, snmp2, walkdir, regex, tokio, futures…).

If step 4's mock proves insufficient for snmp2's protocol expectations (it is currently only self-tested), land steps 1–3 + 5 first and defer step 4 to a follow-up.

## Effort & Risk

~1.5 days. Steps 1 and 5 are mechanical; step 3 is the only real risk (streaming + command conversion), mitigated by e2e plus a manual `npm run dev` walk before merge. Frontend: no changes expected.

## Out of Scope

- Structured error types (`Result<_, String>` → `thiserror`, code-review item 1) — natural follow-up once crate boundaries are fixed; error types can then live in the pure crates.
- Splitting oversized frontend components (TargetBar, ResultsPane).
- Removing the `tauriListen` no-op stub in `tauriCommands.ts`.
