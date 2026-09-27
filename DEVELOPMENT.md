# Development

How to build, run, and test Scout MIB Browser. See [README.md](README.md) for what the app is.

## Prerequisites

- **Node.js** 18+
- **Rust** (latest stable) [rustup.rs](https://rustup.rs)
- **System dependencies** for Tauri on Linux:

```bash
# Debian/Ubuntu
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file librsvg2-dev

# Fedora
sudo dnf install webkit2gtk4.1-devel gtk3-devel librsvg2-devel \
  fuse fuse-libs gstreamer-plugins-base desktop-file-utils
```

## Development

```bash
npm install          # install JS/Rust deps (Rust compiles on first run)

npm run dev          # full app: Svelte + Rust with hot reload
```

The Tauri CLI handles orchestration: it starts the Vite dev server, then launches the Rust backend. Changes to either layer trigger a reload.

## Build Commands

| Script | Command | Description |
|--------|---------|-------------|
| `dev` | `npm run dev` | Full app in development mode (hot reload) |
| `build` | `npm run build` | Release build, generates installers/bundles |
| `tauri:debug` | `npm run tauri:debug` | Unoptimized full build for fast iteration |
| `dev:web` | `npm run dev:web` | Frontend-only (Vite dev server) |
| `build:web` | `npm run build:web` | Frontend-only production bundle |

### Fedora AppImage Workaround

On Fedora, the AppImage bundler may fail with "More than one architectures were found". Set these environment variables to work around it ([tauri#13258](https://github.com/tauri-apps/tauri/issues/13258)):

```bash
ARCH=x86_64 NO_STRIP=true npm run build
```

## Bundled MIBs

`src-tauri/mibs/` holds ~57 standard IETF MIBs (core, network, security, DISMAN) taken from [net-snmp's mibs tree](https://github.com/net-snmp/net-snmp/tree/master/mibs) (BSD-licensed; each file keeps its original copyright header). They ship as Tauri resources (`bundle.resources` in `tauri.conf.json`) and are loaded automatically **only when none of the configured MIB directories contain files** — on systems with net-snmp installed, the system MIBs win, so no module is loaded twice. Dev/source builds read `src-tauri/mibs/` directly (the app falls back to the checkout path when the resource dir has no `mibs/`).

## Checks

```bash
npm run check        # TypeScript + Svelte type checking
npm run check:rust   # Rust compilation check (no linking, fast)
```

## Running CI Locally with act

Run the GitHub Actions workflow (`.github/workflows/ci.yml`) locally with [act](https://github.com/nektos/act).

### Prerequisites

- **act** installed (e.g. `brew install act`, or a binary from the [releases](https://github.com/nektos/act/releases)).
- **A container engine act can reach.** A directly-launched rootless engine (docker/podman) often can't complete the user-namespace handshake in a restricted/sandboxed shell. The reliable option is the **rootless podman systemd socket service**, which runs outside that process tree:

  ```bash
  systemctl --user enable --now podman.socket
  export DOCKER_HOST=unix:///run/user/$(id -u)/podman/podman.sock
  ```

  (With a normal Docker daemon, just ensure it's running and set `DOCKER_HOST` if it's non-default.)

### Building the runner image

Build a one-time wrapper image (in a scratch dir). `act` runs each job as the image's default user, so the image is shaped to mirror a real GitHub Actions runner:

```dockerfile
FROM catthehacker/ubuntu:act-latest
# podman: /var/run must be a real directory, not a symlink to /run, or
# podman's CopyToContainer fails ("path escapes from parent",
# nektos/act#6092). Drop this line when the engine is Docker.
RUN rm -f /var/run && mkdir -p /var/run
# Non-privileged job user (mirrors the GH Actions `runner` user, uid 1001),
# with passwordless sudo for the workflow's `sudo apt-get` steps. Without
# this, act runs the job as root and snmpsim refuses to start ("Must drop
# privileges to a non-privileged user&group").
RUN useradd -u 1001 -m -s /bin/bash runner && \
    echo 'runner ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/runner && \
    chmod 440 /etc/sudoers.d/runner
# Pre-install the e2e Python deps as root so the binaries land in
# /usr/local/bin (on PATH). A pip install run as 1001 lands in
# ~/.local/bin (not on PATH) and snmpsim-test.py can't find
# snmpsim-command-responder.
RUN python3 -m pip install --break-system-packages snmpsim pysmi python-xlib
USER 1001
```

```bash
docker build -f Dockerfile -t act-ubuntu .
```

### Running

```bash
# all jobs
act -P ubuntu-latest=act-ubuntu --pull=false

# a single job
act -j verify-web -P ubuntu-latest=act-ubuntu --pull=false
act -j verify-rust -P ubuntu-latest=act-ubuntu --pull=false
act -j e2e -P ubuntu-latest=act-ubuntu --pull=false
```

- `--pull=false` is required: the wrapper image is local, not on a registry.
- This act version has no `-I` flag — select the image with `-P <platform>=<image>` (the workflow uses `runs-on: ubuntu-latest`).
- All three jobs pass with this image (verified: `verify-rust`, `verify-web`, and `e2e` — 16/16 spec files).

## E2E Testing

End-to-end tests use [WebdriverIO](https://webdriver.io/) with the embedded Tauri WebDriver provider. The test runner starts a Vite dev server, launches the app headless via Xvfb, and drives the UI through WebDriver.

The WebDriver server is embedded in the app binary itself (cargo feature
`scout-mib-browser/wdio`, built by `test:e2e:build`), so no external driver
package is required. In particular `webkit2gtk-driver` is NOT needed (it is
not packaged on Fedora 44). The WebKitGTK library from Prerequisites
(`webkit2gtk4.1`) is sufficient; verified working on Fedora 44 with
`webkit2gtk4.1-2.52.5`.

### System Dependencies

Install these before running E2E tests:

**Ubuntu/Debian:**
```bash
sudo apt install xvfb libgbm1 libasound2-data \
  libatk-bridge2.0-0 libcups2 libdrm2 libxkbcommon0 \
  libxcomposite1 libxdamage1 libxrandr2
```

**Fedora:**
```bash
sudo dnf install xorg-x11-server-Xvfb mesa-libgbm \
  alsa-lib atkmm cups-libs libdrm libxkbcommon \
  libXcomposite libXdamage libXrandr
```

### Running Tests

| Script | Command | Description |
|--------|---------|-------------|
| `test:e2e` | `npm run test:e2e` | Full e2e lifecycle: mock agent + isolated config + Vite + WDIO under Xvfb |
| `test:e2e:build` | `npm run test:e2e:build` | Build Rust first, then run E2E tests |
| `test:e2e:agent` | `npm run test:e2e:agent` | Run only the mock SNMP agent (snmpsim) on port 11611, for manual testing |

The `scripts/test-e2e.sh` wrapper handles the full lifecycle: prepares a temp
`XDG_CONFIG_HOME` pre-seeded with a config pointing at the curated MIBs in
`test/mibs/` (regenerated by `scripts/prepare-test-mibs.sh` if missing), starts
the mock SNMP agent on port 11611, starts Vite on port 5173, runs WDIO under
Xvfb with the isolated environment, and cleans up all processes and temp files
on both success and failure.
