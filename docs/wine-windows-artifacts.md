# Running the Windows Artifacts under Wine

Scout is a Tauri app, so it needs the **WebView2 runtime** (Microsoft Edge / Chromium).
Wine ships no WebView2 runtime, and the **Evergreen** WebView2 installer fails under Wine
(the Edge Update service errors with `0x80040c01`). The working path is to feed the app a
**Fixed Version Runtime** directly via the `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER` env var.

Only the **x64** artifacts can be tested this way (Wine here is 64-bit x86; the arm64
builds need a real Windows-on-ARM host).

## Prerequisites

- Wine (64-bit), `7z`, `unzip`
- The Windows x64 artifact: `scout-mib-browser-<ver>-windows-x64.zip` (or the `-setup.exe`)
- A WebView2 **Fixed Version Runtime** `.cab` (x64) — from the
  [Microsoft WebView2 download page](https://developer.microsoft.com/en-us/microsoft-edge/webview2)
  (Fixed Version → select version + x64). e.g. `Microsoft.WebView2.FixedVersionRuntime.154.0.4258.53.x64.cab`

## 1. Create a Wine prefix with a win7 override

The WebView2 runtime must run in Windows 7 mode under Wine:

```sh
export WINEPREFIX="$HOME/wine-wv2"
wineboot --init
wine reg add "HKCU\Software\Wine\AppDefaults\msedgewebview2.exe" /v Version /t REG_SZ /d win7 /f
```

## 2. Stage the app

```sh
cd "$WINEPREFIX/drive_c"
mkdir scout
unzip /path/to/scout-mib-browser-<ver>-windows-x64.zip -d scout
# For the NSIS installer, extract scout-mib-browser.exe and mibs/ into scout/ instead.
```

## 3. Extract the Fixed Version Runtime

```sh
mkdir -p "$WINEPREFIX/drive_c/wv2rt"
7z x /path/to/Microsoft.WebView2.FixedVersionRuntime.<ver>.x64.cab -o"$WINEPREFIX/drive_c/wv2rt"
```

This yields `wv2rt/Microsoft.WebView2.FixedVersionRuntime.<ver>.x64/` containing
`msedgewebview2.exe`.

## 4. Launch

```sh
RT="C:\\wv2rt\\Microsoft.WebView2.FixedVersionRuntime.<ver>.x64"
wine cmd /c "set WEBVIEW2_BROWSER_EXECUTABLE_FOLDER=$RT&& C:\\scout\\scout-mib-browser.exe"
```

The app starts, finds the runtime, and renders the full UI.

## Notes

- **MIBs:** the app only auto-loads MIBs on startup if the config
  (`%LOCALAPPDATA%\scout\config.toml`) has a `[mib] directories` entry. Otherwise use
  **File → Add MIB Directory**.
- **Headless / remote:** run under `xvfb-run -a wine cmd /c "..."`, or serve the display
  with `x11vnc -display :N -forever -nopw` and connect to `localhost:5900`. On a Wayland
  host, unset `WAYLAND_DISPLAY` when starting `x11vnc` so it targets the X display.
- The runtime can crash intermittently (page faults); just relaunch if the window vanishes.
- **Automated smoke test:** `npm run test:smoke:windows` automates the above and
  asserts the app comes up, finds the runtime, renders, and loads MIBs. It
  expects the binary and the runtime `.cab` in `test/windows/` (or
  `SCOUT_WINDOWS_BIN` / `SCOUT_WV2_CAB`) and fails with guidance if they're
  missing.
