#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target_dir="${CARGO_TARGET_DIR:-$root/target}"
host_target="$(rustc -vV | sed -n 's/^host: //p')"
binary="$target_dir/$host_target/release/shape-app"
report="$target_dir/verification/reports/native-vello-binary-size.json"

cargo build \
  -p shape-app \
  --release \
  --target "$host_target" \
  --target-dir "$target_dir"

bytes="$(wc -c < "$binary" | tr -d ' ')"
sha256="$(shasum -a 256 "$binary" | cut -d ' ' -f 1)"
mkdir -p "$(dirname "$report")"
printf '{\n  "artifact": "%s",\n  "target": "%s",\n  "bytes": %s,\n  "sha256": "%s"\n}\n' \
  "${binary#"$root/"}" \
  "$host_target" \
  "$bytes" \
  "$sha256" > "$report"

echo "PASS native Vello binary size: $bytes bytes ($report)"
