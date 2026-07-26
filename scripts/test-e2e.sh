#!/bin/bash
set -e

# Kill any existing processes
pkill -f "vite" 2>/dev/null || true
sleep 1

# Start Vite dev server in background
echo "Starting Vite dev server..."
npx vite --port 5173 &
VITE_PID=$!
echo "Vite PID: $VITE_PID"

# Wait for Vite to be ready
echo "Waiting for Vite on port 5173..."
for i in $(seq 1 30); do
  if curl -s http://localhost:5173 > /dev/null 2>&1; then
    echo "Vite is ready!"
    break
  fi
  sleep 1
done

# Run WDIO tests
echo "Running E2E tests..."
xvfb-run --auto-servernum npx wdio run wdio.conf.mjs
RESULT=$?

# Cleanup
echo "Stopping Vite (PID: $VITE_PID)..."
kill $VITE_PID 2>/dev/null || true

exit $RESULT
