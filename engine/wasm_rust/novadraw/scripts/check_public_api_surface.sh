#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target_dir="${CARGO_TARGET_DIR:-$root/target/verification/public-api}"
expected="$root/verification/public-api/novadraw-symbols.txt"
actual="$(mktemp)"
trap 'rm -f "$actual"' EXIT

cargo doc -p novadraw --no-deps --target-dir "$target_dir"

doc_root="$target_dir/doc/novadraw"
scopes=(
  "root:index.html"
  "prelude:prelude/index.html"
  "advanced:advanced/index.html"
  "module.animation:animation/index.html"
  "module.connection:connection/index.html"
  "module.container:container/index.html"
  "module.event:event/index.html"
  "module.figure:figure/index.html"
  "module.geometry:geometry/index.html"
  "module.graphics:graphics/index.html"
  "module.host:host/index.html"
  "module.layout:layout/index.html"
  "module.render:render/index.html"
  "module.runtime:runtime/index.html"
  "module.text:text/index.html"
  "module.tree:tree/index.html"
)

for entry in "${scopes[@]}"; do
  scope="${entry%%:*}"
  relative="${entry#*:}"
  html="$doc_root/$relative"
  if [[ ! -f "$html" ]]; then
    echo "missing rustdoc module page: $html" >&2
    exit 1
  fi
  perl -0777 - "$scope" "$html" >>"$actual" <<'PERL'
use strict;
use warnings;

my ($scope, $path) = @ARGV;
open my $handle, '<', $path or die "cannot read $path: $!\n";
local $/;
my $html = <$handle>;

while ($html =~ m{<dl class="item-table(?: [^"]*)?">(.*?)</dl>}sg) {
    my $table = $1;
    while ($table =~ m{<dt([^>]*)>(.*?)</dt>}sg) {
        my ($attributes, $item) = ($1, $2);
        next unless $item =~ m{<a class="(constant|enum|fn|macro|mod|struct|trait|type|union)"};
        my $kind = $1;
        my $name;
        if ($attributes =~ /id="reexport\.([^"]+)"/) {
            $name = $1;
        } elsif ($item =~ /title="\Q$kind\E novadraw(?:::[^":]+)*::([^":]+)"/) {
            $name = $1;
        } else {
            next;
        }
        print "$scope $kind $name\n";
    }
}
PERL
done

LC_ALL=C sort -u -o "$actual" "$actual"

if [[ "${1:-}" == "--update" ]]; then
  mkdir -p "$(dirname "$expected")"
  cp "$actual" "$expected"
  echo "UPDATED ${expected#$root/}"
  exit 0
fi

if [[ ! -f "$expected" ]]; then
  echo "missing public API snapshot: $expected" >&2
  echo "run scripts/check_public_api_surface.sh --update after reviewing the API" >&2
  exit 1
fi

if ! cmp -s "$expected" "$actual"; then
  diff -u "$expected" "$actual" || true
  echo "public API symbols changed; review the diff and update the snapshot intentionally" >&2
  exit 1
fi

echo "PASS root, prelude, domain module, and advanced API symbols match the snapshot"
