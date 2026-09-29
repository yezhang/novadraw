#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PORT="${PORT:-4173}"

exec python3 -m http.server "$PORT" \
  --bind 127.0.0.1 \
  --directory "$ROOT/examples/web/web-validation/dist"
