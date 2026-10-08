#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
pattern='(debug_render|trace_render|info_render|warn_render|error_render|debug|trace|info|warn|error|println|eprintln)!\s*\(|tracing::'
protected_paths=(
  "$root/novadraw/src/graph/render_recursive.rs"
  "$root/novadraw/src/layout"
  "$root/novadraw-backend-vello/src"
)

if rg -n --glob '*.rs' "$pattern" "${protected_paths[@]}"; then
  echo "hot paths must not contain tracing or print macros" >&2
  exit 1
fi

echo "PASS protected render, layout, and backend hot paths contain no logging macros"
