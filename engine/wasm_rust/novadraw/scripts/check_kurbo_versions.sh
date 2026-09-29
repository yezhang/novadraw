#!/usr/bin/env bash
set -euo pipefail

dependency_tree="$(
  cargo tree \
    --workspace \
    --all-features \
    --target all \
    --edges normal \
    --prefix none
)"
versions="$(
  printf '%s\n' "$dependency_tree" |
    sed -nE 's/^kurbo v([^ ]+).*/\1/p' |
    sort -u
)"
version_count="$(printf '%s\n' "$versions" | sed '/^$/d' | wc -l | tr -d ' ')"

if [[ "$version_count" -eq 0 ]]; then
  echo "workspace dependency graph does not contain kurbo" >&2
  exit 1
fi

if [[ "$version_count" -ne 1 ]]; then
  echo "workspace dependency graph contains multiple kurbo versions:" >&2
  printf '  %s\n' $versions >&2
  exit 1
fi

echo "PASS workspace dependency graph uses kurbo $versions"
