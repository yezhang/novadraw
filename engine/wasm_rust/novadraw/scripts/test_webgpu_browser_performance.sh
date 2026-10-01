#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NODE_BIN="${NODE_BIN:-node}"

if ! command -v "$NODE_BIN" >/dev/null 2>&1; then
  echo "Node.js executable not found: $NODE_BIN" >&2
  exit 4
fi

exec "$NODE_BIN" --test "$ROOT/scripts/run_webgpu_browser_performance.test.mjs"
