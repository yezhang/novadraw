#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target_dir="${CARGO_TARGET_DIR:-$root/target/verification/public-api}"
host_target="$(rustc -vV | sed -n 's/^host: //p')"
forbidden_pattern='title="(struct|enum|trait|type|fn|constant|union) (kurbo|vello|winit)::'
crate_docs=(
  novadraw
  novadraw_editor
  novadraw_inspector
)

cargo clean --doc --target-dir "$target_dir"

check_target() {
  local target="$1"
  local doc_root="$target_dir/$target/doc"
  local findings

  cargo doc \
    -p novadraw \
    -p novadraw-editor \
    -p novadraw-inspector \
    --all-features \
    --no-deps \
    --target "$target" \
    --target-dir "$target_dir"

  findings="$(
    find "${crate_docs[@]/#/$doc_root/}" -type f -name '*.html' -print0 |
      xargs -0 perl -0777 -ne '
        while (/<(pre|h[34])\b[^>]*class="[^"]*(?:item-decl|code-header)[^"]*"[^>]*>.*?<\/\1>/sg) {
          $signature = $&;
          if ($signature =~ /'"$forbidden_pattern"'/) {
            $signature =~ s/<[^>]+>/ /g;
            $signature =~ s/&gt;/>/g;
            $signature =~ s/&lt;/</g;
            $signature =~ s/&amp;/&/g;
            $signature =~ s/\s+/ /g;
            print "$ARGV: $signature\n";
          }
        }
      '
  )"

  if [[ -n "$findings" ]]; then
    printf '%s\n' "$findings" >&2
    echo "public API exposes a forbidden implementation type for target $target" >&2
    return 1
  fi
}

check_target "$host_target"
check_target wasm32-unknown-unknown

echo "PASS public API signatures do not expose kurbo, vello, or winit types"
