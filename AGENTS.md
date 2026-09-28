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

## Commit messages

Commit messages must follow the [7 rules](https://chris.beams.io/posts/git-commit/):

1. Separate the subject from the body with a blank line
2. Limit the subject line to 50 characters
3. Capitalize the subject line
4. Do not end the subject line with a period
5. Use the imperative mood in the subject line
6. Wrap the body at 72 characters
7. Use the body to explain what and why vs. how

## Running CI locally

Run the CI workflow locally with `act`. See [DEVELOPMENT.md → Running CI Locally with act](DEVELOPMENT.md#running-ci-locally-with-act) for prerequisites (rootless podman socket + `DOCKER_HOST`) and the one-time `act-ubuntu` runner image build (podman `/var/run` fix, non-privileged `runner` user, pre-installed e2e Python deps). Quick form:

```bash
systemctl --user enable --now podman.socket
export DOCKER_HOST=unix:///run/user/$(id -u)/podman/podman.sock
act -P ubuntu-latest=act-ubuntu --pull=false
```

## Backend layout

The Rust backend is a Cargo workspace:

- `crates/scout-mib` — MIB parsing and OID resolution. Pure, no UI dependency.
- `crates/scout-snmp` — SNMP engine with a pure async API (`WalkBatchSender` trait for streaming). No tauri imports; the tokio runtime is owned by the caller.
- `src-tauri/` — app crate: Tauri commands, config, logging. Owns one multi-threaded tokio runtime with 8MB worker stacks (snmp2 recurses deeply and overflows default stacks).
