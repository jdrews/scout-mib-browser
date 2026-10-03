# Cross-Platform Release Artifacts

**Date:** 2026-09-27
**Status:** Draft — pending review
**Branch:** `feat/cross-platform-release`

## Purpose

Publish **runnable** artifacts for Scout MIB Browser across six architecture targets, produced and released by GitHub Actions. The release is **user-driven**: a maintainer triggers the workflow from the GitHub web UI (Actions → *Release* → *Run workflow*), types a tag name, and the pipeline builds every target, attaches the artifacts to a GitHub Release, and publishes a `SHA256SUMS` file — the same shape as `goreleaser` and the `logstation` release workflow.

Settled design decisions (proposed, pending review):

- **Use [`tauri-apps/tauri-action`](https://github.com/tauri-apps/tauri-action) as the build/release backbone.** It is the official Tauri GitHub Action: it sets up each platform's toolchain + native deps, runs `tauri build`, handles asset naming (`releaseAssetNamePattern`), and creates the GitHub Release with auto-generated notes (`generateReleaseNotes`). Its recommended workflow already builds Linux arm64 on the **native** `ubuntu-24.04-arm` runner (not QEMU). We extend it in three places it does not cover: a **Windows arm64** leg (the ARM64 MSVC build tools must be installed by a custom step — the action only runs `tauri build --target …`), a **`SHA256SUMS`** job (the action does not emit checksums), and a **`tag` input** for the logstation-style "name your tag" model.
- **Run-on-the-native-runner, not cross-compile, wherever a native runner exists.** Cross-compiling a WebKitGTK app is fragile; GitHub now has free ARM64 Linux runners, which removes the one genuinely hard target (Linux arm64). We cross-compile only where no native runner exists (Windows arm64, and optionally macOS x64).
- **One "runnable" artifact per platform, chosen to be self-contained:** AppImage (Linux), `.app`/`.dmg` (macOS), NSIS installer + portable `.zip` (Windows). No `.deb`/`.rpm`/`.msi`/flatpak in v1 (deferred, per the "no packages right now" constraint) — though `tauri build` already emits them, so they can be attached later without rework.
- **The release is a `workflow_dispatch` with a required `tag` input** (mirrors `logstation`). The pipeline builds the current HEAD and anchors the Release + tag to that commit.
- **Code signing is a known, documented gap in v1:** ad-hoc sign on macOS so the app runs locally; unsigned on Windows/Linux with the OS trust warnings (Gatekeeper / SmartScreen) documented. Full signing + notarization is a follow-up, not a blocker.

## Target matrix — recommendations

The six requested targets are the right core set. Recommendations on "any others":

| Target | Runner | Build approach | Runnable artifact |
|--------|--------|----------------|-------------------|
| Linux x64 | `ubuntu-24.04` | Native | `…-linux-x64.AppImage` |
| Linux arm64 | `ubuntu-24.04-arm` (ARM64 hosted) | **Native on ARM** | `…-linux-arm64.AppImage` |
| macOS x64 | `macos-latest` (Apple Silicon) | Cross-compile `--target x86_64-apple-darwin` | `…-darwin-x64.dmg` (+ `.app`) |
| macOS arm64 | `macos-latest` (Apple Silicon) | Native | `…-darwin-arm64.dmg` (+ `.app`) |
| Windows x64 | `windows-latest` (x64) | Native | `…-windows-x64-setup.exe` + `.zip` |
| Windows arm64 | `windows-latest` (x64) | Cross-compile `--target aarch64-pc-windows-msvc` | `…-windows-arm64-setup.exe` + `.zip` |

**Why Linux arm64 is built on an ARM runner, not cross-compiled.** Tauri's AppImage tooling (`linuxdeploy`) does **not** support cross-compiling ARM AppImages — ARM AppImages can only be built on an ARM host (or a slow QEMU emulator). GitHub's `ubuntu-24.04-arm` hosted runners (free for public repos) make a native arm64 build the clean path: install the arm64 `libwebkit2gtk-4.1-dev` etc. natively, run `tauri build`, done in ~10 min. This sidesteps the fragile "aarch64 native deps in a cross toolchain" problem entirely.

**Why macOS x64 is cross-compiled from the arm64 runner.** On macOS Tauri uses the system `WKWebView` (no native C dependency to cross-compile, unlike Linux's WebKitGTK), so `rustup target add x86_64-apple-darwin` + `tauri build --target x86_64-apple-darwin` on a single Apple Silicon runner produces both arches. This is one runner, two builds. (Alternative: `macos-13` Intel runner for a native x64 build — more runners, no benefit for this app.)

**Recommended NOT to add (v1):** Linux i686 (32-bit x86), Windows x86 (32-bit), Linux armv7 (32-bit ARM), and musl/Alpine builds. These are legacy/niche for a modern GUI app and each adds a distinct toolchain burden. **Optional nicety:** a macOS *universal* binary (merge x64 + arm64 into one `.app`/`.dmg` via `lipo`) to cut the macOS artifact count from two to one — deferred; shipping two arches separately is simpler and fine.

**Deferred (packages, "later"):** `.deb`, `.rpm`, `.msi`, flatpak. `tauri build` with `bundle.targets: "all"` already produces `deb`/`rpm`/`msi` alongside the runnable artifacts, so attaching them in a future release is a one-line change to the release job, not a re-architecture.

## Investigation findings

### What exists today

| Layer | Piece | Location |
|-------|-------|----------|
| App | Tauri v2 (Svelte + Rust), `bundle.targets: "all"`, `productName` "Scout MIB Browser", `version` `0.1.0` | `src-tauri/tauri.conf.json` |
| Build | `npm run build` → `tauri build` (release); `beforeBuildCommand: npm run build:web` | `package.json:7`, `tauri.conf.json:10` |
| CI | Three jobs on `ubuntu-latest` (verify-rust, verify-web, e2e) with the Linux system-dep install list + `Swatinem/rust-cache` + `sccache` | `.github/workflows/ci.yml` |
| Release | **None.** No release workflow, no artifact publishing, no checksums, no signing. | — |
| Docs | Dev/build docs; **no** runtime-dependency / install docs. | `DEVELOPMENT.md` |
| MIBs | ~57 bundled IETF MIBs ship as Tauri resources (`bundle.resources`). | `src-tauri/mibs/`, `tauri.conf.json:30` |

### Gaps

- **G1 — No release pipeline.** Nothing builds for non-host platforms, nothing publishes a GitHub Release, no checksums, no release notes.
- **G2 — No cross-architecture builds.** `tauri build` builds for the host arch only. arm64 targets (Linux, Windows) and macOS x64-on-arm64 need explicit `--target` / runner selection.
- **G3 — No runtime-dependency documentation.** The user's constraint ("if dependencies must be installed prior to running, document that") is unmet — there is no install/runtime-deps doc.
- **G4 — No code signing.** macOS ad-hoc signing, Windows/Linux signing are all absent; the OS trust warnings are undocumented.
- **G5 — Bundle `targets: "all"` on non-Windows hosts.** `tauri build` on a Linux host with `targets: "all"` also emits `deb`/`rpm`; on a non-Windows host the `msi`/`nsis` are skipped. For a clean "runnable-only" v1 we select explicit `--bundles` per platform so we don't ship package formats we've deferred.

## Design

### Phase 1 — The build matrix

One matrix leg per target. Each leg: install the platform's native build deps (and, for Windows arm64, the ARM64 MSVC build tools) → `npm ci` → tauri-action runs `tauri build` (with the leg's `--target`), names the assets via `releaseAssetNamePattern`, and uploads them to the (draft) Release + as workflow artifacts.

Per-target specifics:

- **Linux x64 / arm64** (`ubuntu-24.04` / `ubuntu-24.04-arm`): install `libwebkit2gtk-4.1-dev libgtk-3-dev libdbus-1-dev libssl-dev libsoup-3.0-dev librsvg2-dev pkg-config` (the arm64 runner installs the arm64 variants natively). Build base is Ubuntu 24.04 (or 22.04 for a lower glibc floor — see Risk R1). `tauri build --bundles appimage`. Output: `src-tauri/target/release/bundle/appimage/*.AppImage`.
- **macOS** (`macos-latest`, one runner, two builds): `rustup target add x86_64-apple-darwin`. Build arm64 natively, then `tauri build --target x86_64-apple-darwin`. Ad-hoc sign each `.app` (`codesign --sign -`) so it runs locally. Output: `…/bundle/macos/*.app` and `…/bundle/dmg/*.dmg`.
- **Windows** (`windows-latest`, one runner, two builds): native x64 build, then `rustup target add aarch64-pc-windows-msvc` + the **C++ ARM64 build tools** (VS 2022 `MSVC v143 C++ ARM64 build tools`) + `tauri build --target aarch64-pc-windows-msvc`. The NSIS installer is x86 (runs via emulation on ARM); the app binary is native arm64. Output: `…/bundle/nsis/*-setup.exe`. Also produce a portable `.zip` of the release binary + resources for both arches.

Caching mirrors `ci.yml`: `Swatinem/rust-cache` + `sccache` (`RUSTC_WRAPPER`) + `actions/setup-node` npm cache, per job.

### Phase 2 — The release workflow (`.github/workflows/release.yml`)

`workflow_dispatch` with a required `tag` input (the logstation model), `contents: write`, a `verify` gate, a `tauri-action` matrix, and a final `checksum` job that adds the goreleaser-style `SHA256SUMS` and publishes the draft.

```yaml
name: Release

on:
  workflow_dispatch:
    inputs:
      tag: { description: 'New tag name (e.g. v0.1.0)', required: true }

permissions:
  contents: write

jobs:
  verify:                       # gate: pre-commit checks, one host (reuses ci.yml steps)
    runs-on: ubuntu-latest
    # cargo fmt --check, cargo clippy, cargo test --workspace --all-features,
    # npx tsc --noEmit, npx svelte-check --threshold warning

  build:
    needs: [verify]
    runs-on: ${{ matrix.platform }}
    strategy:
      fail-fast: false
      matrix:
        include:
          - { platform: 'ubuntu-24.04',     args: '' }                                # linux x64
          - { platform: 'ubuntu-24.04-arm', args: '' }                                # linux arm64 (native ARM)
          - { platform: 'macos-latest',     args: '--target x86_64-apple-darwin' }    # macos x64
          - { platform: 'macos-latest',     args: '--target aarch64-apple-darwin' }   # macos arm64
          - { platform: 'windows-latest',   args: '' }                                # windows x64
          - { platform: 'windows-latest',   args: '--target aarch64-pc-windows-msvc', arm: true } # windows arm64
    steps:
      - uses: actions/checkout@<pinned>
      - name: Linux system deps
        if: runner.os == 'Linux'
        run: sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libdbus-1-dev libssl-dev libsoup-3.0-dev librsvg2-dev pkg-config
      - name: Windows ARM64 MSVC build tools
        if: matrix.arm
        run: |  # install "C++ ARM64 build tools" (MSVC v143) — tauri-action does NOT do this
          # e.g. add the VS component via the Visual Studio Installer / vswhere
      - uses: actions/setup-node@<pinned>   # node 20, cache npm
      - run: npm ci
      - uses: dtolnay/rust-toolchain@<pinned>   # + macos targets
      - uses: Swatinem/rust-cache@<pinned>
      - uses: tauri-apps/tauri-action@<pinned>
        env: { GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }} }
        with:
          tagName: ${{ inputs.tag }}        # verbatim (no __VERSION__) → the user's tag is used
          releaseName: 'Scout MIB Browser ${{ inputs.tag }}'
          generateReleaseNotes: true        # GitHub-generated notes (PRs since last tag)
          releaseDraft: true               # don't publish on a partial matrix failure
          uploadWorkflowArtifacts: true    # so the checksum job can re-fetch all bundles
          releaseAssetNamePattern: '[mainBinaryName]-[version]-[platform]-[arch][ext]'
          workflowArtifactNamePattern: '[mainBinaryName]-[version]-[platform]-[arch][ext]'  # match release names so SHA256SUMS is consistent
          args: ${{ matrix.args }}

  checksum:                      # goreleaser-style SHA256SUMS + publish the draft
    needs: [build]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/download-artifact@<pinned>   # all workflow artifacts → one dir
      - run: sha256sum * > SHA256SUMS
      - run: |  # upload the checksum file and publish the draft (gh CLI)
          gh release upload ${{ inputs.tag }} SHA256SUMS
          gh release edit ${{ inputs.tag }} --draft false
```

Notes:

- **Windows arm64 is the one leg tauri-action cannot set up for you** — the ARM64 MSVC build tools must be installed by a custom step; the action only runs `tauri build --target aarch64-pc-windows-msvc`.
- **`SHA256SUMS` is the one goreleaser-style artifact tauri-action does not produce** — the `checksum` job adds it.
- **`releaseDraft: true`** means a partial matrix failure leaves an unpublished draft; `checksum` (which `needs: build`, i.e. all legs) publishes only when every target succeeded.
- **`workflowArtifactNamePattern` is set equal to `releaseAssetNamePattern`** so the workflow artifacts the `checksum` job downloads carry the same names as the published release assets — otherwise `SHA256SUMS` would reference names users can't match to their downloads.
- `verify` gates the matrix so a broken commit never burns six build runners. Because it is a `workflow_dispatch`, the release is anchored to the dispatch HEAD and the maintainer names the tag — the `logstation` model.

### Phase 3 — Artifact naming, checksums, publishing

**Naming** comes from tauri-action's `releaseAssetNamePattern: '[mainBinaryName]-[version]-[platform]-[arch][ext]'`, where `[mainBinaryName]` = `scout-mib-browser` (the Cargo package name) and `[version]` is read from `tauri.conf.json` (single source of truth) — no separate rename step. (Verify the exact `[platform]`/`[arch]` strings tauri-action emits per OS during the dry run; adjust the pattern if they differ from the table.)

```
scout-mib-browser-<ver>-linux-x64.AppImage
scout-mib-browser-<ver>-linux-arm64.AppImage
scout-mib-browser-<ver>-darwin-x64.dmg
scout-mib-browser-<ver>-darwin-arm64.dmg
scout-mib-browser-<ver>-windows-x64-setup.exe
scout-mib-browser-<ver>-windows-arm64-setup.exe
scout-mib-browser-<ver>-windows-x64.zip        (optional — custom step; tauri-action emits no portable zip)
scout-mib-browser-<ver>-windows-arm64.zip      (optional — custom step)
SHA256SUMS
```

**Release + notes** are created by tauri-action (`generateReleaseNotes: true` → GitHub's generated description, PRs since the last tag; a `releaseBody` is pre-pended). **`SHA256SUMS`** is generated by the `checksum` job over the canonical-named files (goreleaser-style) and uploaded to the release. The optional portable `.zip` is produced by a small custom step after the Windows build (zip the release binary + resources), since tauri-action does not emit portable zips.

### Phase 4 — Code signing (v1 = ad-hoc / unsigned, documented)

- **macOS:** `codesign --sign -` (ad-hoc) on each `.app` so it launches locally. Full Developer-ID signing + notarization requires an Apple Developer account → **follow-up**. Until then, a downloaded unsigned/ad-hoc app triggers Gatekeeper; the user right-clicks → Open (or `xattr -d com.apple.quarantine`).
- **Windows:** unsigned in v1 → SmartScreen "Windows protected your PC" on first run. A code-signing cert (OV/EV) is a **follow-up**.
- **Linux:** AppImage unsigned in v1 (optionally GPG-sign with `appimage-sign` later). No OS gate.

### Phase 5 — Runtime-dependency documentation (`docs/INSTALL.md`)

New `docs/INSTALL.md`, linked from `README.md` and this spec, documenting per-platform runtime requirements (the user's "document the dependencies" constraint):

- **Linux (AppImage):** self-contained — bundles WebKitGTK/GTK and all native deps. Needs **FUSE** on the host, or run with `--appimage-extract-and-run`. (If the bare binary is used instead of the AppImage: `libwebkit2gtk-4.1-0`, `libgtk-3-0`, `librsvg-2-0`, `libsoup-3.0-0`, `libjavascriptcoregtk-4.1-0`.)
- **macOS:** self-contained `.app`; no external deps. Unsigned/ad-hoc → Gatekeeper (right-click → Open).
- **Windows:** NSIS installer bundles the binary, resources, and the WebView2 bootstrapper. Win10 (1804+) / Win11 ship WebView2; older needs the bootstrapper (default `downloadBootstrapper` mode). Unsigned → SmartScreen.

## Test plan

- **Dry runs.** Trigger the workflow for each single build job (via a `workflow_dispatch` that accepts a `target` override, or by running one job) and confirm the artifact is produced and correctly named.
- **Artifact smoke tests.** On a clean VM/container per platform, run the produced artifact:
  - Linux x64 + arm64 AppImage: launch, confirm the window opens and a bundled MIB loads.
  - macOS x64 + arm64 `.app`: launch on matching-arch hardware/VM, confirm it opens (ad-hoc-signed).
  - Windows x64 + arm64: run the NSIS installer (x64 natively; arm64 on an ARM VM or via emulation), confirm the app launches.
- **Checksum verification.** Re-run `sha256sum -c SHA256SUMS` against the downloaded Release assets.
- **Idempotency / re-dispatch.** Re-running the workflow with the same tag overwrites cleanly (or is blocked with a clear message — pick one and document it).

## Risks and mitigations

| Risk | Mitigation |
|------|------------|
| R1 — glibc floor: building on a too-new base raises the min glibc, breaking older Linux (`GLIBC_2.xx not found`) | Build Linux on Ubuntu 22.04 (or 24.04, documented floor); document the supported baseline in `docs/INSTALL.md`. |
| R2 — `ubuntu-24.04-arm` runner availability (free for public repos; billed + possibly rate-limited for private) | Primary path is the ARM runner; document the QEMU-emulation fallback (slow) and the "build on a self-hosted ARM box" option for private repos. |
| R3 — Windows arm64 cross-compile needs the ARM64 MSVC build tools installed on the runner | Install `MSVC v143 C++ ARM64 build tools` in the job (or a small setup script); the NSIS installer is x86 so only the app binary needs the arm64 toolchain. |
| R4 — macOS Gatekeeper / Windows SmartScreen on unsigned v1 artifacts | Ad-hoc sign macOS; document the OS trust warnings in `docs/INSTALL.md`; full signing/notarization is a tracked follow-up. |
| R5 — Six slow release builds (full Rust compile each) | Reuse `ci.yml` caching (rust-cache + sccache + npm cache); builds run in parallel; `verify` gates before any build starts. |
| R6 — `bundle.targets: "all"` emits deferred package formats | Select explicit `--bundles` per platform (appimage / app+dmg / nsis) so v1 ships only runnable artifacts; packages remain a one-line later addition. |
| R7 — Tag/version drift (tag name vs `tauri.conf.json` version) | The tag is the single source of truth for the release version: validated up front in the gate job, then injected into `tauri.conf.json` by each build leg before the build. The repo file holds a dev marker (`0.1.0-dev`) for local builds and is never bumped for releases. |
| R8 — `tauri-action` is a moving target (`@v1`); behavior can shift under a floating tag | Pin `tauri-apps/tauri-action` to a specific version/SHA in the workflow; bump deliberately. |
| R9 — tauri-action creates the Release per matrix leg; a partial failure could leave a half-populated release | `releaseDraft: true` keeps it unpublished on failure; the `checksum` job (which `needs` the whole matrix) publishes only when every target succeeded. |

## Definition of done

1. `.github/workflows/release.yml` exists, `workflow_dispatch` with a required `tag` input, `contents: write`, and a `verify` gate that runs the pre-commit checks.
2. All six targets build on their designated runners (native where a runner exists; cross-compile only for Windows arm64 and macOS x64) and produce the canonical-named runnable artifacts.
3. tauri-action (pinned) creates the GitHub Release (tag on HEAD + auto-generated notes) and uploads all artifacts; the `checksum` job generates `SHA256SUMS` and publishes the draft.
4. macOS artifacts are ad-hoc signed; the OS trust warnings (Gatekeeper / SmartScreen) and the signing follow-up are documented.
5. `docs/INSTALL.md` documents per-platform runtime dependencies (FUSE for AppImage, self-contained macOS/Windows, WebView2) and is linked from `README.md`.
6. Each produced artifact is smoke-tested on a clean target (launches, window opens, a bundled MIB loads) and `sha256sum -c SHA256SUMS` passes against the published assets.
7. Pre-commit checklist green: `cargo fmt`, `cargo test --workspace --all-features`, `npx tsc --noEmit`, `npx svelte-check --threshold warning`.

## Open questions

- **Public vs private repo:** the ARM64 Linux runner is free for public repos but billed for private — confirm the repo's visibility so we know whether the `ubuntu-24.04-arm` path is cost-free or needs a self-hosted ARM fallback.
- **Re-dispatch semantics:** should re-running with an existing tag overwrite the Release, or fail fast? (Proposed: overwrite, since releases are re-cuttable.)
- **macOS universal binary:** ship one merged x64+arm64 `.dmg` instead of two? (Proposed: keep two separate for v1; universal is a later nicety.)
- **Release notes source:** GitHub auto-generated (PRs since last tag) vs a maintained `CHANGELOG.md`? (Proposed: auto-generated for v1.)
- **Signing timeline:** when to add Developer-ID + notarization (macOS) and an OV/EV cert (Windows)? Tracked as a follow-up spec.
