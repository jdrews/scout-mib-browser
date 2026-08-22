#!/bin/bash
set -euo pipefail

# UX assessment harness (docs/specs/2026-08-22-ux-assessment.md).
#
# Reuses the e2e lifecycle (mock agent, Vite, Xvfb, isolated config) via
# scripts/test-e2e.sh, but runs the separate UX probe suite
# (wdio.ux.conf.mjs -> test/specs-ux/**) so it stays out of the CI feature
# baseline and can be slow/screenshot-heavy. Artifacts land in
# docs/ux/<date>/ — screenshots plus JSON probe results.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export UX_RUN_DATE="${UX_RUN_DATE:-$(date +%F)}"
mkdir -p "$REPO_ROOT/docs/ux/$UX_RUN_DATE"

# The Tauri window is 1200x800; give the Xvfb screen headroom so the UX suite's
# full-window `import` captures aren't clipped at the right/bottom edge.
export E2E_XVFB_SCREEN="${E2E_XVFB_SCREEN:-1280x900}"

exec bash "$REPO_ROOT/scripts/test-e2e.sh" wdio.ux.conf.mjs
