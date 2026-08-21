#!/usr/bin/env bash
# Local compilation check — mirrors the CI workflow.
set -euo pipefail

echo "==> Building frontend..."
npm run build:web
echo "==> Running cargo check..."
cargo check --workspace --all-targets
echo "==> Check passed."
