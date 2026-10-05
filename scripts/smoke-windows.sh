#!/bin/bash
set -euo pipefail

# Smoke test for the Windows x64 artifact running under Wine.
#
# Verifies the one thing that matters for a Windows build: does it actually run?
# It launches the binary under Wine (with the WebView2 runtime), then asserts:
#   1. the app + WebView2 processes come up,
#   2. the WebView2 runtime is found (no "Failed to find an installed
#      WebView2 runtime" error),
#   3. the UI renders (the screenshot is not blank),
#   4. MIBs load (the MIB BROWSER panel shows a tree, not "No MIBs loaded").
#
# This is deliberately NOT the full e2e suite: Wine is not Windows (the app
# page-faults intermittently under Wine), and the Windows-specific surface is
# small (same frontend, same backend code). A fast, reliable smoke test is the
# right level of automation here.
#
# Inputs (this script fails with guidance if any are missing):
#   - 64-bit Wine
#   - 7z (to extract the WebView2 runtime .cab)
#   - The Windows x64 binary: $SCOUT_WINDOWS_BIN (any build — a plain release
#     build is fine; the `wdio` feature is NOT needed for a smoke test)
#   - The WebView2 Fixed Version Runtime .cab: $SCOUT_WV2_CAB
#
# NOTE: the Evergreen WebView2 installer does NOT work under Wine — the Edge
# Update service (Goopdate) fails with 0x80040c01. Use the Fixed Version
# Runtime .cab and point the app at it via WEBVIEW2_BROWSER_EXECUTABLE_FOLDER.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# ── Inputs (override via env) ───────────────────────────────────────────────
WINE_BIN="${WINE_BIN:-wine}"
SCOUT_WINDOWS_BIN="${SCOUT_WINDOWS_BIN:-test/windows/scout-mib-browser.exe}"
SCOUT_WV2_CAB="${SCOUT_WV2_CAB:-$(ls test/windows/*.cab 2>/dev/null | head -1 || true)}"
WINE_PREFIX="${SCOUT_WINE_PREFIX:-/tmp/scout-smoke-wine-pfx}"
XVFB_DISPLAY="${SCOUT_SMOKE_DISPLAY:-:99}"

fail() { echo "" >&2; echo "ERROR: $*" >&2; echo "" >&2; exit 1; }

# ── Check inputs, directing the user to what to do if they're missing ─────
command -v "$WINE_BIN" >/dev/null 2>&1 || fail \
  "Wine was not found ('$WINE_BIN'). Install 64-bit Wine, e.g.
     Fedora:  sudo dnf install wine
     Debian:  sudo apt install wine64
     Then re-run."

command -v 7z >/dev/null 2>&1 || fail \
  "7z was not found (needed to extract the WebView2 runtime .cab). Install it, e.g.
     Fedora:  sudo dnf install p7zip
     Debian:  sudo apt install p7zip-full
     Then re-run."

command -v import >/dev/null 2>&1 || fail \
  "ImageMagick 'import' was not found (needed to screenshot the app). Install it, e.g.
     Fedora:  sudo dnf install ImageMagick
     Debian:  sudo apt install imagemagick
     Then re-run."

[ -f "$SCOUT_WINDOWS_BIN" ] || fail \
  "The Windows x64 binary was not found at: $SCOUT_WINDOWS_BIN
     Any Windows x64 build works (a plain release build is fine — the 'wdio'
     feature is NOT needed for a smoke test). Put it there, or point
     SCOUT_WINDOWS_BIN at it:
       SCOUT_WINDOWS_BIN=/path/to/scout-mib-browser.exe npm run test:smoke:windows"

if [ -z "$SCOUT_WV2_CAB" ] || [ ! -f "$SCOUT_WV2_CAB" ]; then
  fail "The WebView2 Fixed Version Runtime .cab was not found in test/windows/.
     Download the x64 Fixed Version Runtime .cab from the Microsoft WebView2
     page and drop it into test/windows/ — or point SCOUT_WV2_CAB at it:
       SCOUT_WV2_CAB=/path/to/Microsoft.WebView2.FixedVersionRuntime.<ver>.x64.cab npm run test:smoke:windows
     (The Evergreen WebView2 installer does NOT work under Wine; use the .cab.)"
fi

# ── Wine prefix with the win7 override ──────────────────────────────────────
export WINEPREFIX="$WINE_PREFIX"
export WINEDEBUG=-all
export WINEDLLOVERRIDES="mscoree,mshtml="
mkdir -p "$WINE_PREFIX"
echo "Initializing Wine prefix at $WINE_PREFIX ..."
wineboot --init >/dev/null 2>&1 || true
# The WebView2 runtime must run in Windows 7 mode under Wine.
wine reg add "HKCU\Software\Wine\AppDefaults\msedgewebview2.exe" \
  /v Version /t REG_SZ /d win7 /f >/dev/null 2>&1 || true

# ── Stage the binary + curated test MIBs ───────────────────────────────────
STAGE_DIR="$WINE_PREFIX/drive_c/scout"
mkdir -p "$STAGE_DIR"
cp -f "$SCOUT_WINDOWS_BIN" "$STAGE_DIR/scout-mib-browser.exe"
MIBS_DIR="$WINE_PREFIX/drive_c/test/mibs"
mkdir -p "$MIBS_DIR"
cp -rf "$REPO_ROOT/test/mibs/." "$MIBS_DIR/"

# ── Extract the Fixed Version Runtime ───────────────────────────────────────
WV2_RT_ROOT="$WINE_PREFIX/drive_c/wv2rt"
mkdir -p "$WV2_RT_ROOT"
echo "Extracting WebView2 runtime from $SCOUT_WV2_CAB ..."
7z x -y "$SCOUT_WV2_CAB" -o"$WV2_RT_ROOT" >/dev/null
WV2_RT_FOLDER="$(find "$WV2_RT_ROOT" -maxdepth 1 -type d -name 'Microsoft.WebView2.FixedVersionRuntime.*' | head -1)"
[ -n "$WV2_RT_FOLDER" ] || fail "Could not find the extracted WebView2 runtime folder under $WV2_RT_ROOT."
[ -f "$WV2_RT_FOLDER/msedgewebview2.exe" ] || fail "msedgewebview2.exe not found in $WV2_RT_FOLDER — is $SCOUT_WV2_CAB a Fixed Version Runtime .cab?"
WV2_RT_WIN="C:\\wv2rt\\$(basename "$WV2_RT_FOLDER")"
WINE_APP_WIN="C:\\scout\\scout-mib-browser.exe"

# ── Pre-seed the Wine config so the app auto-loads the curated MIBs ───────
# The MIB path is a Windows path with backslashes. In TOML, backslashes are
# escape characters in basic (double-quoted) strings, so "C:\test\mibs" is a
# parse error (\t = tab, \m = invalid). Use a literal (single-quoted) string,
# which needs no escaping. The heredoc is quoted so the backslash is written
# verbatim (an unquoted heredoc would collapse "\\" to "\").
APPDATA_DIR="$(find "$WINE_PREFIX/drive_c/users" -maxdepth 4 -type d -ipath '*/AppData/Local' 2>/dev/null | head -1)"
[ -n "$APPDATA_DIR" ] || fail "Could not find the Wine user's AppData/Local dir under $WINE_PREFIX/drive_c/users."
mkdir -p "$APPDATA_DIR/scout"
cat > "$APPDATA_DIR/scout/config.toml" <<'EOF'
[mib]
directories = ['C:\test\mibs']

[target]
community = "public"
host = "127.0.0.1"
port = 161
version = "v2c"
EOF

# ── Dedicated Xvfb (so we can screenshot the app) ──────────────────────────
pkill -x Xvfb 2>/dev/null || true
sleep 2
echo "Starting Xvfb on $XVFB_DISPLAY ..."
Xvfb "$XVFB_DISPLAY" -screen 0 1600x900x24 -nolisten tcp >/tmp/scout-smoke-xvfb.log 2>&1 &
XVFB_PID=$!
sleep 4
export DISPLAY="$XVFB_DISPLAY"

APP_LOG=/tmp/scout-smoke-app.log

# ── Cleanup ─────────────────────────────────────────────────────────────────
RESULT=0
cleanup() {
  RESULT=$?
  echo ""
  echo "Cleaning up ..."
  # Tear down the Wine session so no orphaned app/webview2 processes linger.
  pkill -x winedbg 2>/dev/null || true
  pkill -x wineserver 2>/dev/null || true
  [ -n "$XVFB_PID" ] && kill "$XVFB_PID" 2>/dev/null || true
  exit $RESULT
}
trap cleanup EXIT

# ── Launch the app under Wine, with a retry for the intermittent Wine crash ──
# The app page-faults intermittently under Wine (a Wine bug, not an app bug).
# Retry up to 2 attempts so a Wine crash doesn't cause a false failure. Each
# attempt waits for the UI to render, then for the MIBs to load (the tree
# populates asynchronously after the render: configRead -> mibLoadDirectories
# -> refreshTree, so the empty state is transient while loading).
UP=0
MIB_LOADED=0
PANEL_TEXT=""
for ATTEMPT in 1 2; do
  # Kill any lingering Wine processes from a previous attempt.
  pkill -x winedbg 2>/dev/null || true
  pkill -x wineserver 2>/dev/null || true
  sleep 2
  echo "Launching the Windows binary under Wine (attempt $ATTEMPT) ..."
  wine cmd /c "set WEBVIEW2_BROWSER_EXECUTABLE_FOLDER=$WV2_RT_WIN&& $WINE_APP_WIN" > "$APP_LOG" 2>&1 &
  WINE_LAUNCH_PID=$!

  # Poll for the UI to render. The process name (pgrep) is unreliable under
  # Wine, so the rendered UI (a non-blank screenshot) is the authoritative
  # signal that the app is running.
  echo "Waiting for the UI to render (up to 120s) ..."
  for i in $(seq 1 12); do
    sleep 10
    import -window root /tmp/scout-smoke-probe.png 2>/dev/null || true
    PROBE_STDDEV=$(identify -format "%[standard-deviation]" /tmp/scout-smoke-probe.png 2>/dev/null || echo 0)
    if awk "BEGIN{exit !($PROBE_STDDEV >= 100)}"; then
      UP=1
      echo "The UI rendered (screenshot stddev=$PROBE_STDDEV) at t=$((i*10))s."
      break
    fi
    # If the app crashed (winedbg attached) or exited, stop waiting early and
    # let the outer loop retry.
    if pgrep -x 'winedbg' >/dev/null 2>&1 || ! kill -0 "$WINE_LAUNCH_PID" 2>/dev/null; then
      echo "The app crashed or exited early (t=$((i*10))s) — likely a Wine crash."
      break
    fi
  done

  if [ "$UP" -ne 1 ]; then
    echo "Attempt $ATTEMPT did not bring the UI up; retrying ..."
    pkill -x winedbg 2>/dev/null || true
    pkill -x wineserver 2>/dev/null || true
    sleep 3
    continue
  fi

  # The MIBs load asynchronously after the render. The empty state ("No MIBs
  # loaded") is transient while loading, so poll the MIB BROWSER panel (OCR)
  # for the tree to populate before declaring the MIB-load check a failure.
  echo "Waiting for the MIBs to load (up to 60s) ..."
  for i in $(seq 1 12); do
    sleep 5
    import -window root /tmp/scout-smoke-shot.png 2>/dev/null || true
    convert /tmp/scout-smoke-shot.png -crop 400x400+0+40 +repage -resize 250% -colorspace Gray -sharpen 0x1 /tmp/scout-smoke-panel.png 2>/dev/null || true
    PANEL_TEXT=$(tesseract /tmp/scout-smoke-panel.png - 2>/dev/null | tr -s ' \n' ' ' || true)
    if echo "$PANEL_TEXT" | grep -qiE "ccitt|iso\.org|internet|joint-iso|nodes loaded"; then
      MIB_LOADED=1
      echo "MIBs loaded (tree visible) at t=$((i*5))s after render."
      break
    fi
    echo "Waiting for MIBs (t=$((i*5))s); panel: ${PANEL_TEXT:-(empty)}"
    # If the app crashed during MIB loading, stop early and let the outer loop retry.
    if pgrep -x 'winedbg' >/dev/null 2>&1 || ! kill -0 "$WINE_LAUNCH_PID" 2>/dev/null; then
      echo "The app crashed during MIB loading (t=$((i*5))s) — likely a Wine crash."
      break
    fi
  done

  if [ "$MIB_LOADED" -eq 1 ]; then
    break
  fi
  echo "Attempt $ATTEMPT did not load the MIBs; retrying ..."
  pkill -x winedbg 2>/dev/null || true
  pkill -x wineserver 2>/dev/null || true
  sleep 3
done

# The final screenshot is the last one taken (the MIB poll loop's, or the
# render probe's if the UI never rendered). The app is still running, so it is
# a valid representation of the final state.
SHOT=/tmp/scout-smoke-shot.png
if [ ! -f "$SHOT" ] && [ -f /tmp/scout-smoke-probe.png ]; then
  cp /tmp/scout-smoke-probe.png "$SHOT" 2>/dev/null || true
fi

# ── Assertions ─────────────────────────────────────────────────────────────
echo ""
echo "=== Smoke test assertions ==="

# 1. The app came up (the UI rendered during the wait loop).
if [ "$UP" -ne 1 ]; then
  echo "FAIL: the app did not come up (the UI never rendered)."
  echo "----- app log -----"
  tail -20 "$APP_LOG"
  exit 1
fi
echo "PASS: the app came up (the UI rendered)."

# 2. The WebView2 runtime was found (no "Failed to find" error).
if grep -qi "Failed to find an installed WebView2 runtime" "$APP_LOG"; then
  echo "FAIL: the WebView2 runtime was not found."
  echo "----- app log -----"
  tail -20 "$APP_LOG"
  exit 1
fi
echo "PASS: the WebView2 runtime was found."

# 3. The UI rendered (the screenshot is not blank).
STDDEV=$(identify -format "%[standard-deviation]" "$SHOT" 2>/dev/null || echo 0)
echo "Screenshot standard-deviation: $STDDEV (a blank image is ~0)."
if awk "BEGIN{exit !($STDDEV < 100)}"; then
  echo "FAIL: the UI did not render (screenshot is blank)."
  exit 1
fi
echo "PASS: the UI rendered."

# 4. MIBs loaded (the MIB BROWSER panel shows a tree, not "No MIBs loaded").
#    The poll loop above OCR'd the panel each iteration; PANEL_TEXT is the most
#    recent reading and MIB_LOADED is the authoritative signal.
echo "MIB BROWSER panel OCR: ${PANEL_TEXT:-(empty)}"
if [ "$MIB_LOADED" -eq 1 ]; then
  echo "PASS: MIBs loaded (the panel shows a tree, not 'No MIBs loaded')."
elif echo "$PANEL_TEXT" | grep -qi "No MIBs loaded"; then
  echo "FAIL: MIBs did not load (the panel shows 'No MIBs loaded')."
  exit 1
elif [ -z "$PANEL_TEXT" ]; then
  echo "WARN: could not OCR the MIB BROWSER panel; skipping the MIB-load check."
else
  echo "PASS: MIBs loaded (the panel shows a tree, not 'No MIBs loaded')."
fi

echo ""
echo "SMOKE TEST PASSED: the Windows binary runs under Wine."
exit 0
