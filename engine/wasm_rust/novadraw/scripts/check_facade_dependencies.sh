#!/usr/bin/env bash
set -euo pipefail

forbidden_pattern='^(vello|winit|web-sys|raw-window-handle|objc2-core-graphics|objc2-quartz-core) v'
dependency_tree="$(cargo tree -p novadraw --edges normal --prefix none)"

if printf '%s\n' "$dependency_tree" | grep -E "$forbidden_pattern"; then
  echo "core facade unexpectedly enables a platform rendering dependency" >&2
  exit 1
fi

echo "PASS novadraw core facade has no platform rendering dependencies"
