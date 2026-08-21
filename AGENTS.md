## Agent skills

### Issue tracker

Issues live as Markdown files under `~/git/scout-tickets/` — outside the repo to keep the workspace clean. See `docs/agents/issue-tracker.md`.

### Triage labels

Five canonical roles: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context repo — `CONTEXT.md` at root, ADRs in `docs/adr/`. See `docs/agents/domain.md`.

## Pre-commit checklist

Run these from the repo root before considering work done:

1. **Rust format**: `cargo fmt`
2. **Rust tests**: `cargo test --workspace --all-features`
3. **TypeScript check**: `npx tsc --noEmit`
4. **Svelte check**: `npx svelte-check --threshold warning` (no errors)

## Backend layout

The Rust backend is a Cargo workspace:

- `crates/scout-mib` — MIB parsing and OID resolution. Pure, no UI dependency.
- `crates/scout-snmp` — SNMP engine with a pure async API (`WalkBatchSender` trait for streaming). No tauri imports; the tokio runtime is owned by the caller.
- `src-tauri/` — app crate: Tauri commands, config, logging. Owns one multi-threaded tokio runtime with 8MB worker stacks (snmp2 recurses deeply and overflows default stacks).
