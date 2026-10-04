#!/usr/bin/env bash
#
# fix-appimage-wayland.sh — strip the bundled libwayland from a Tauri AppImage.
#
# Why: the Tauri AppImage bundler (linuxdeploy) auto-bundles libwayland-*
# alongside webkit2gtk. That copy is frozen at the build host's version
# (e.g. 1.22.0 on the ubuntu-24.04 CI runner) and is ABI-incompatible with a
# newer host (e.g. wayland 1.26.0 on Fedora 44). webkit2gtk's
# eglGetDisplay(EGL_DEFAULT_DISPLAY) then fails with EGL_BAD_PARAMETER and the
# app aborts before the window appears:
#
#   Could not create default EGL display: EGL_BAD_PARAMETER. Aborting...
#
# libwayland must match the host's compositor / GL stack and must not be
# bundled. This script removes the bundled libwayland-* and re-packs the
# AppImage, so the dynamic loader resolves libwayland from the system at
# runtime. The rest of the bundle (including webkit2gtk, which already uses the
# system libEGL/libGL/libgbm) is left untouched.
#
# Usage: fix-appimage-wayland.sh <path-to-AppImage>
# The AppImage is replaced in place.
#
# Requires: appimagetool (self-contained: it bundles mksquashfs and its
# AppRun puts it on PATH). Extraction uses the AppImage's own
# --appimage-extract, so no FUSE or unsquashfs is needed.

set -euo pipefail

APPIMAGE="${1:?usage: fix-appimage-wayland.sh <path-to-AppImage>}"
[ -f "$APPIMAGE" ] || { echo "error: $APPIMAGE not found" >&2; exit 1; }
# Resolve to an absolute path so it still resolves after we `cd` into the
# workdir below (the caller may pass a relative path).
APPIMAGE="$(cd "$(dirname "$APPIMAGE")" && pwd)/$(basename "$APPIMAGE")"

command -v appimagetool >/dev/null || { echo "error: appimagetool not on PATH" >&2; exit 1; }

workdir="$(mktemp -d)"
trap 'rm -rf "$workdir"' EXIT
cd "$workdir"

# 1. Extract the AppImage payload -> ./squashfs-root/
"$APPIMAGE" --appimage-extract >/dev/null

# 2. Remove the bundled libwayland-*. They live in usr/lib/.
#    If there is nothing to remove, the image is already correct: verify and
#    exit without re-packing (keeps the step idempotent / a no-op when clean).
if ls squashfs-root/usr/lib/libwayland-*.so* >/dev/null 2>&1; then
  rm -v squashfs-root/usr/lib/libwayland-*.so*
  # 3. Re-pack. appimagetool names the output after the input dir basename and
  #    writes it to the current working directory (-> ./squashfs-root.AppImage).
  #    Detect the result defensively: prefer the expected name, else the newest
  #    .AppImage that appeared in cwd.
  appimagetool squashfs-root >/dev/null
  NEW="$(find . -maxdepth 1 -name 'squashfs-root.AppImage' | head -1)"
  if [ -z "$NEW" ]; then
    NEW="$(ls -1t *.AppImage 2>/dev/null | head -1)"
    [ -n "$NEW" ] && NEW="./$NEW"
  fi
  [ -n "$NEW" ] && [ -f "$NEW" ] || { echo "error: re-packed AppImage not found" >&2; exit 1; }
  # 4. Replace the original in place.
  mv "$NEW" "$APPIMAGE"
  echo "OK: stripped bundled libwayland and re-packed $APPIMAGE (resolves libwayland from system)."
else
  echo "OK: no bundled libwayland found in $APPIMAGE; nothing to do."
fi
