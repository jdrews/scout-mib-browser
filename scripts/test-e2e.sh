#!/bin/bash
set -euo pipefail

# E2E test harness (shared by the feature suite and the UX assessment suite).
#
# Usage: scripts/test-e2e.sh [wdio-config]   (default: wdio.conf.mjs)
#
# Lifecycle (everything torn down via trap on exit, success or failure):
#   1. Prepare a temp XDG_CONFIG_HOME with a pre-seeded scout/config.toml
#      pointing at the curated test/mibs/ set and the local mock agent.
#   2. Start the mock SNMP agent (snmpsim) on the e2e port.
#   3. Start Vite on port 5173.
#   4. Run WDIO under Xvfb with the isolated environment.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"
WDIO_CONFIG="${1:-wdio.conf.mjs}"
AGENT_PORT="${E2E_AGENT_PORT:-11611}"
# Second agent: synthetic ifStackTable (multi-component index, 600 rows) for
# the table-retrieval specs. The app config points at $AGENT_PORT; specs that
# need it switch the port input to this one.
SYNTH_AGENT_PORT="${E2E_SYNTH_AGENT_PORT:-11612}"
VITE_PORT=5173

WORK_DIR="$(mktemp -d /tmp/scout-e2e-XXXXXX)"
export XDG_CONFIG_HOME="$WORK_DIR/config"
mkdir -p "$XDG_CONFIG_HOME/scout"

# Pre-seeded config: curated MIBs + local mock agent. The app reads this at
# startup, so tests never touch the developer's real ~/.config/scout.
cat > "$XDG_CONFIG_HOME/scout/config.toml" <<EOF
[mib]
directories = ["$REPO_ROOT/test/mibs"]

[target]
community = "public"
host = "127.0.0.1"
port = $AGENT_PORT
version = "v2c"
EOF

# test/mibs is committed; regenerate if the checkout is incomplete.
if [ ! -f "$REPO_ROOT/test/mibs/BROKEN-MIB" ]; then
  echo "test/mibs incomplete — running prepare-test-mibs.sh..."
  bash "$REPO_ROOT/scripts/prepare-test-mibs.sh"
fi

VITE_PID=""
AGENT_PID=""
SYNTH_AGENT_PID=""
RESULT=0

cleanup() {
  RESULT=$?
  echo ""
  echo "Cleaning up e2e environment..."
  if [ -n "$AGENT_PID" ] && kill -0 "$AGENT_PID" 2>/dev/null; then
    kill "$AGENT_PID" 2>/dev/null || true
  fi
  if [ -n "$SYNTH_AGENT_PID" ] && kill -0 "$SYNTH_AGENT_PID" 2>/dev/null; then
    kill "$SYNTH_AGENT_PID" 2>/dev/null || true
  fi
  pkill -f "snmpsim-command-responder.*:$AGENT_PORT" 2>/dev/null || true
  pkill -f "snmpsim-command-responder.*:$SYNTH_AGENT_PORT" 2>/dev/null || true
  if [ -n "$VITE_PID" ] && kill -0 "$VITE_PID" 2>/dev/null; then
    kill "$VITE_PID" 2>/dev/null || true
  fi
  rm -rf "$WORK_DIR"
  exit $RESULT
}
trap cleanup EXIT

# ── Mock SNMP agents ─────────────────────────────────────────────────────────
echo "Starting mock SNMP agent on port $AGENT_PORT..."
python3 "$REPO_ROOT/scripts/snmpsim-test.py" --port "$AGENT_PORT" \
  > "$WORK_DIR/agent.log" 2>&1 &
AGENT_PID=$!

# Synthetic ifStackTable agent (multi-component index, 600 rows).
echo "Starting synthetic ifStack agent on port $SYNTH_AGENT_PORT..."
python3 "$REPO_ROOT/scripts/snmpsim-test.py" --port "$SYNTH_AGENT_PORT" \
  "$REPO_ROOT/test/snmprec/synthetic-ifstack.snmprec" \
  > "$WORK_DIR/synth-agent.log" 2>&1 &
SYNTH_AGENT_PID=$!

if command -v snmpget > /dev/null 2>&1; then
  echo "Waiting for agents to answer SNMP..."
  AGENT_UP=0
  for i in $(seq 1 30); do
    if snmpget -v2c -c public "127.0.0.1:$AGENT_PORT" 1.3.6.1.2.1.1.5.0 \
        > /dev/null 2>&1; then
      AGENT_UP=1
      break
    fi
    sleep 1
  done
  if [ "$AGENT_UP" -ne 1 ]; then
    echo "ERROR: mock agent did not come up (see $WORK_DIR/agent.log)" >&2
    exit 1
  fi
  SYNTH_UP=0
  for i in $(seq 1 30); do
    if snmpget -v2c -c public "127.0.0.1:$SYNTH_AGENT_PORT" \
        1.3.6.1.2.1.31.1.2.1.3.1.1 > /dev/null 2>&1; then
      SYNTH_UP=1
      break
    fi
    sleep 1
  done
  if [ "$SYNTH_UP" -ne 1 ]; then
    echo "ERROR: synthetic agent did not come up (see $WORK_DIR/synth-agent.log)" >&2
    exit 1
  fi
else
  sleep 3
fi

# ── Vite dev server ──────────────────────────────────────────────────────────
pkill -f "vite" 2>/dev/null || true
sleep 1

echo "Starting Vite dev server on port $VITE_PORT..."
npx vite --port "$VITE_PORT" > "$WORK_DIR/vite.log" 2>&1 &
VITE_PID=$!
echo "Vite PID: $VITE_PID"

echo "Waiting for Vite on port $VITE_PORT..."
for i in $(seq 1 30); do
  if curl -s "http://localhost:$VITE_PORT" > /dev/null 2>&1; then
    echo "Vite is ready!"
    break
  fi
  sleep 1
done

# ── WDIO under Xvfb ──────────────────────────────────────────────────────────
echo "Running E2E tests (wdio config: $WDIO_CONFIG, app config: $XDG_CONFIG_HOME)..."
set +e
# The Tauri window is 1200x800, so the Xvfb screen must be at least that large:
# pointer-based input (e.g. mouse-wheel tests) is clamped to the screen edge
# otherwise. The UX suite exports a larger value for unclipped screenshots.
XVFB_ARGS=(--auto-servernum -s "-screen 0 ${E2E_XVFB_SCREEN:-1280x900}x24")
xvfb-run "${XVFB_ARGS[@]}" npx wdio run "$WDIO_CONFIG"
RESULT=$?
set -e

exit $RESULT
