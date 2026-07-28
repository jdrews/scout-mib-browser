#!/usr/bin/env bash
# Local compilation check — mirrors the CI workflow.
set -euo pipefail

echo "==> Building frontend..."
npm run build:web
echo "==> Running cargo check..."
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
echo "==> Check passed."
