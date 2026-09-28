# Installing Scout MIB Browser

Release artifacts are published as a GitHub Release.
Verify any download against the release's `SHA256SUMS` file:

```sh
sha256sum -c SHA256SUMS
```

Artifact naming: `scout-mib-browser-<version>-<platform>-<arch><ext>`, where the
version is the one from `src-tauri/tauri.conf.json`.

| Artifact | Platform |
|----------|----------|
| `…-linux-amd64.AppImage` | Linux x86-64 |
| `…-linux-aarch64.AppImage` | Linux ARM64 |
| `…-darwin-x64.dmg` / `…-darwin-x64.app.tar.gz` | macOS Intel |
| `…-darwin-aarch64.dmg` / `…-darwin-aarch64.app.tar.gz` | macOS Apple Silicon |
| `…-windows-x64-setup.exe` / `…-windows-x64.zip` | Windows x64 |
| `…-windows-arm64-setup.exe` / `…-windows-arm64.zip` | Windows ARM64 |

## Linux (AppImage)

The AppImage is **self-contained**: it bundles WebKitGTK, GTK, and all native
dependencies. No packages need to be installed before running.

- **FUSE** is required to run the AppImage normally. If FUSE is unavailable
  (e.g. some containers), run with `--appimage-extract-and-run` instead.
- **glibc floor:** the AppImage is built on Ubuntu 24.04, so it requires
  glibc 2.39 or newer (Ubuntu 24.04+, Debian 13+). On older systems,
  `--appimage-extract-and-run` still works if the system libraries are present.

If you run the **bare binary** instead of the AppImage (not a release
artifact), these runtime libraries must be installed:
`libwebkit2gtk-4.1-0`, `libgtk-3-0`, `librsvg-2-0`, `libsoup-3.0-0`,
`libjavascriptcoregtk-4.1-0`.

## macOS

The `.app` (in the `.dmg` or the `.app.tar.gz`) is **self-contained**; no
external dependencies.

**Code signing (v1):** the app is ad-hoc signed, not Developer-ID signed and
not notarized (a follow-up). Gatekeeper will warn on first launch:

- Right-click (or control-click) the app → **Open** → **Open**, or
- `xattr -d com.apple.quarantine /path/to/Scout\ MIB\ Browser.app`

## Windows

The NSIS installer (`-setup.exe`) bundles the binary, the bundled MIBs, and the
WebView2 bootstrapper; the portable `.zip` contains the same binary + MIBs
without an installer.

- **WebView2:** Windows 10 (1804+) and Windows 11 ship the WebView2 runtime;
  on older systems the bundled bootstrapper downloads it on first install.
- **Code signing (v1):** the installer is unsigned, so SmartScreen may show
  "Windows protected your PC" on first run — choose **More info** →
  **Run anyway** (a follow-up adds an OV/EV certificate).

## Bundled MIBs

~57 IETF MIBs ship with every artifact as bundled resources; no separate
download is needed.
