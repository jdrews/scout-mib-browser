# Scout MIB Browser — Code Quality & Architecture Review

**Date:** 2026-08-15 (revised 2026-08-21)
**Scope:** Full read-only analysis of `src-tauri` (~6,400 LOC Rust) and `src` (~2,800 LOC Svelte/TS)

## Strengths

- **Architecture matches the design docs** (`docs/specs/`): clean two-tier split — pure-Rust `mib` + `snmp` modules behind Tauri commands, Svelte 5 frontend with a single reactive `$state` object (`src/lib/stores.svelte.ts:5`) and thin invoke wrappers.
- **Strong documentation culture**: CONTEXT.md, ADRs, dated specs, doc comments on all public Rust APIs.
- **Good CI**: fmt + clippy + `--all-features` tests for Rust; typecheck + build for web (`.github/workflows/ci.yml`). Lockfiles committed.
- **Error tolerance is a real, tested feature**: `snmp/tolerant.rs` has 22 unit tests; 91 Rust unit tests total across the crate.
- **Walk cancellation** via `AtomicBool` token and streaming via Tauri `Channel`s — solid design for long operations.

## Issues (prioritized improvements)

1. **`Result<_, String>` everywhere (45 sites)** — no error types. Frontend can't distinguish "device unreachable" from "bad OID" from "parse failure". Introduce `thiserror` types in the backend and structured error payloads in commands. *Not addressed by the workspace-split spec; the main remaining gap to A-.*
2. **`MockSnmpServer` (613 lines) is dead code** — exported from `snmp/mod.rs:19` but its listen/respond loop is referenced by no test (the 9 tests inside `mock.rs` only cover BER encoding helpers). Worse, the 9 engine tests only cover pure helpers (`is_subtree_of`, `Target` constructors); **no get/walk/bulk path is actually tested** in CI (only the opt-in live snmpsim test at `src-tauri/tests/snmpsim_integration.rs`). Wire the mock into real engine tests.
3. **"Pure Rust backend" claim is violated** — `lib.rs` advertises zero UI dependency, but `snmp/engine.rs:75-76` (also `128-129`, `398`) takes `tauri::ipc::Channel` directly. Abstract to a generic sender/callback trait.
4. **Three tokio runtimes exist** — worse than a single smell:
   - The engine's private 2-thread multi-thread runtime (`engine.rs:31-51`), used via `block_on` for get/getnext/set/walk-table and `spawn` for streaming walks.
   - An ad-hoc OS thread (8MB stack) + current-thread runtime spawned **per call** in `snmp_connect` (`main.rs:289-299`) to avoid snmp2's deep recursion overflowing default 2MB tokio worker stacks.
   - Tauri's own runtime, on which async commands execute.

   Consolidate to one app-owned runtime with 8MB worker stacks. Note: naively awaiting engine calls on Tauri's 2MB-stack workers would reintroduce the overflow — "just run snmp2 on Tauri's runtime" is wrong as stated.
5. **`tauriListen` is a no-op stub** (`tauriCommands.ts:4-9`) — it carries a doc comment explaining why (event plugin unavailable), but it's still dead API; remove it or implement it.
6. **Silent failure**: `serde_json::to_string(&rs).unwrap_or_default()` at 4 sites (`engine.rs:98,114,151,167`) sends an empty string on serialization error.
7. **Large components**: `TargetBar.svelte` (478 lines) and `ResultsPane.svelte` (432 lines) should be split into subcomponents.
8. **Shallow e2e**: 4 smoke assertions (title + element existence) in `test/specs/example.spec.ts`. A mock-device walk verifying rendered results would be high-value.
9. **Repo hygiene**: `.sandcastle/` scaffolding is tracked; `package-lock.json` is both gitignored and committed (contradiction); `csp: null` + `withGlobalTauri: true` in `tauri.conf.json` (acceptable for local desktop, but a CSP is cheap to add).

## Post-review developments

- The uncommitted SystemLogPane WIP (former item 10) was committed as `bdae683` — resolved.
- `docs/specs/2026-08-20-crate-workspace-split.md` (proposed) structurally addresses items 2–4 + 6: workspace split into `crates/scout-mib` + `crates/scout-snmp`, a `WalkBatchSender` trait replacing `Channel`, pure-async engine, app-owned runtime, and the mock wired into integration tests. The root `Cargo.toml` workspace already exists.

## Grade: B-

Strong foundation — clear architecture, real domain docs, good CI, and a well-tested tolerance layer. Held back by the missing end-to-end engine tests (the mock server exists but is unused), stringly-typed errors, and the runtime/channel coupling that breaks the stated "pure Rust" boundary. With the workspace-split spec landing, items 2–4 + 6 become compile-enforced; item 1 (structured errors) is the remaining gap to a solid A-.
