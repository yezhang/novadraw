#!/usr/bin/env bash
set -euo pipefail

forbidden_pattern='^(novadraw-editor|novadraw-inspector|novadraw-backend-vello|novadraw-platform-winit|novadraw-platform-web|vello|winit|web-sys|raw-window-handle|objc2-core-graphics|objc2-quartz-core) v'
dependency_tree="$(cargo tree -p novadraw --edges normal --prefix none)"

if printf '%s\n' "$dependency_tree" | grep -E "$forbidden_pattern"; then
  echo "novadraw core unexpectedly enables an optional package or platform dependency" >&2
  exit 1
fi

echo "PASS novadraw core has no editor, inspector, backend, or platform dependencies"
