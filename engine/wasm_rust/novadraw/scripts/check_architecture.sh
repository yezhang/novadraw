#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target_dir="${CARGO_TARGET_DIR:-$root/target/verification/public-api}"
expected="$root/verification/public-api/novadraw-symbols.txt"
actual="$(mktemp)"
trap 'rm -f "$actual"' EXIT

if [[ $# -gt 1 || ( $# -eq 1 && "$1" != "--update-api" ) ]]; then
  echo "usage: $0 [--update-api]" >&2
  exit 2
fi

forbidden_dependency_pattern='^(novadraw-editor|novadraw-inspector|novadraw-backend-vello|novadraw-platform-winit|novadraw-platform-web|vello|winit|web-sys|raw-window-handle|objc2-core-graphics|objc2-quartz-core) v'
core_tree="$(cargo tree -p novadraw --edges normal --prefix none)"
if printf '%s\n' "$core_tree" | grep -E "$forbidden_dependency_pattern"; then
  echo "novadraw core unexpectedly depends on an editor, backend, or platform package" >&2
  exit 1
fi

workspace_tree="$(
  cargo tree \
    --workspace \
    --all-features \
    --target all \
    --edges normal \
    --prefix none
)"
kurbo_versions="$(
  printf '%s\n' "$workspace_tree" |
    sed -nE 's/^kurbo v([^ ]+).*/\1/p' |
    sort -u
)"
kurbo_version_count="$(
  printf '%s\n' "$kurbo_versions" |
    sed '/^$/d' |
    wc -l |
    tr -d ' '
)"
if [[ "$kurbo_version_count" -ne 1 ]]; then
  echo "workspace must contain exactly one Kurbo version; found:" >&2
  printf '  %s\n' $kurbo_versions >&2
  exit 1
fi

logging_pattern='(debug_render|trace_render|info_render|warn_render|error_render|debug|trace|info|warn|error|println|eprintln)!\s*\(|tracing::'
if rg -n --glob '*.rs' "$logging_pattern" \
  "$root/novadraw/src/graph/render_recursive.rs" \
  "$root/novadraw/src/layout" \
  "$root/novadraw-backend-vello/src"; then
  echo "protected render, layout, and backend hot paths must not log" >&2
  exit 1
fi

if rg -n \
  'std::fs|std::path|decode_file|complete_image_file|File::open|read_to_end' \
  "$root/novadraw/src/render" \
  "$root/novadraw/src/runtime"; then
  echo "Core render/runtime must not perform platform resource I/O" >&2
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

crate_docs=(
  novadraw
  novadraw_editor
  novadraw_inspector
)
for doc_root in \
  "$target_dir/doc" \
  "$target_dir/wasm32-unknown-unknown/doc"; do
  for crate in "${crate_docs[@]}"; do
    rm -rf "$doc_root/$crate"
  done
done

cargo doc \
  -p novadraw \
  -p novadraw-editor \
  -p novadraw-inspector \
  --all-features \
  --no-deps \
  --target-dir "$target_dir"

scan_forbidden_types() {
  local doc_root="$1"
  local findings
  findings="$(
    find "${crate_docs[@]/#/$doc_root/}" -type f -name '*.html' -print0 |
      xargs -0 perl -0777 -ne '
        while (/<(pre|h[34])\b[^>]*class="[^"]*(?:item-decl|code-header)[^"]*"[^>]*>.*?<\/\1>/sg) {
          $signature = $&;
          if ($signature =~ /title="(?:struct|enum|trait|type|fn|constant|union) (?:kurbo|vello|winit)::/) {
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
    echo "public API exposes a forbidden implementation type" >&2
    exit 1
  fi
}

host_doc_root="$target_dir/doc"
scan_forbidden_types "$host_doc_root"

novadraw_doc_root="$host_doc_root/novadraw"
runtime_html="$novadraw_doc_root/runtime/runtime/struct.Runtime.html"
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
  html="$novadraw_doc_root/$relative"
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

if [[ "${1:-}" == "--update-api" ]]; then
  cp "$actual" "$expected"
else
  if [[ ! -f "$expected" ]]; then
    echo "missing public API snapshot: $expected" >&2
    exit 1
  fi
  if ! cmp -s "$expected" "$actual"; then
    diff -u "$expected" "$actual" || true
    echo "public API symbols changed; review and run $0 --update-api" >&2
    exit 1
  fi
fi

cargo doc \
  -p novadraw \
  -p novadraw-editor \
  -p novadraw-inspector \
  --all-features \
  --no-deps \
  --target wasm32-unknown-unknown \
  --target-dir "$target_dir"
scan_forbidden_types "$target_dir/wasm32-unknown-unknown/doc"

echo "PASS architecture boundaries, Kurbo $kurbo_versions, and Runtime methods $runtime_method_count/$runtime_method_budget"
