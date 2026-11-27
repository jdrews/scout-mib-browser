<!--
Summary of the change: what does this do, and why? Link the issue it
closes (Fixes #NNN) when there is one.
-->

## Test plan

Gates from [AGENTS.md](../AGENTS.md#pre-commit-checklist); run from the repo
root, tick each after running it:

- [ ] `cargo fmt` (no diff)
- [ ] `cargo test --workspace --all-features`
- [ ] `npx tsc --noEmit`
- [ ] `npx svelte-check --threshold warning` (0 errors)
- [ ] `npx vitest run`
- [ ] `npm run test:e2e` (full suite green; UI changes: add/adjust a spec)

## Screenshots

For UI changes, before/after screenshots.
