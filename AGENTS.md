## Agent skills

### Issue tracker

Issues live as Markdown files under `~/git/scout-tickets/` — outside the repo to keep the workspace clean. See `docs/agents/issue-tracker.md`.

### Triage labels

Five canonical roles: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context repo — `CONTEXT.md` at root, ADRs in `docs/adr/`. See `docs/agents/domain.md`.

## Pre-commit checklist

Run these before considering work done:

1. **Rust format**: `cargo fmt --manifest-path src-tauri/Cargo.toml`
2. **Rust tests**: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
3. **TypeScript check**: `npx tsc --noEmit`
4. **Svelte check**: `npx svelte-check --threshold warning` (no errors)
