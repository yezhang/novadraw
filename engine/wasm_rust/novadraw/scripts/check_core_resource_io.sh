#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if rg -n \
  'std::fs|std::path|decode_file|complete_image_file|File::open|read_to_end' \
  "$root/novadraw/src/render" \
  "$root/novadraw/src/runtime"; then
  echo "Core render/runtime must not perform platform resource I/O" >&2
  exit 1
fi

echo "PASS Core render/runtime contain no platform resource I/O"
