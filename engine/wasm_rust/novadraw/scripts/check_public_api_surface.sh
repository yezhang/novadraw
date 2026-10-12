#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target_dir="${CARGO_TARGET_DIR:-$root/target/verification/public-api}"
expected="$root/verification/public-api/novadraw-symbols.txt"
actual="$(mktemp)"
trap 'rm -f "$actual"' EXIT

cargo doc -p novadraw --no-deps --target-dir "$target_dir"

doc_root="$target_dir/doc/novadraw"
runtime_html="$doc_root/runtime/runtime/struct.Runtime.html"
scopes=(
  "root:index.html"
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

if [[ ! -f "$runtime_html" ]]; then
  echo "missing Runtime rustdoc page: $runtime_html" >&2
  exit 1
fi

runtime_method_count="$(
  perl -0777 - "$runtime_html" <<'PERL'
use strict;
use warnings;

my ($path) = @ARGV;
open my $handle, '<', $path or die "cannot read $path: $!\n";
local $/;
my $html = <$handle>;
$html =~ m{<div id="implementations-list">(.*?)<h2 id="trait-implementations"}s
    or die "cannot locate inherent Runtime implementations in $path\n";
my $implementations = $1;
my %methods;
while ($implementations =~ /id="method\.([^"]+)"/g) {
    $methods{$1} = 1;
}
print scalar keys %methods;
PERL
)"

runtime_method_budget=25
if (( runtime_method_count > runtime_method_budget )); then
  echo "Runtime exposes $runtime_method_count inherent methods; budget is $runtime_method_budget" >&2
  exit 1
fi

unexpected_root="$(awk '$1 == "root" && $2 != "mod" { print }' "$actual")"
if [[ -n "$unexpected_root" ]]; then
  echo "crate root must expose modules only; found:" >&2
  echo "$unexpected_root" >&2
  exit 1
fi

if rg -n 'novadraw::prelude' \
  "$root/novadraw" \
  "$root/novadraw-editor" \
  "$root/novadraw-inspector" \
  "$root/novadraw-backend-vello" \
  "$root/novadraw-platform-winit" \
  "$root/novadraw-platform-web" \
  "$root/examples" \
  "$root/benchmarks" \
  --glob '*.rs'; then
  echo "library-level novadraw prelude references are forbidden" >&2
  exit 1
fi

if rg -n '^pub mod prelude;' "$root/novadraw/src/lib.rs"; then
  echo "novadraw must not publish a library-level prelude module" >&2
  exit 1
fi

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

echo "PASS module-only root, domain module, advanced API symbols, and Runtime method budget ($runtime_method_count/$runtime_method_budget)"
