#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target="x86_64-pc-windows-msvc"
package="node-editor-demo"
host="$(rustc -vV | sed -n 's/^host: //p')"

case "$host" in
  *-pc-windows-msvc)
    exec cargo check --locked --target "$target" -p "$package"
    ;;
esac

cargo_xwin_version="0.23.1"
sdk_version="10.0.26100"
crt_version="14.44.17.14"
local_cargo_xwin="$root/target/xwin-tools/bin/cargo-xwin"

if [[ -n "${CARGO_XWIN:-}" ]]; then
  cargo_xwin="$CARGO_XWIN"
elif [[ -x "$local_cargo_xwin" ]]; then
  cargo_xwin="$local_cargo_xwin"
elif command -v cargo-xwin >/dev/null 2>&1; then
  cargo_xwin="$(command -v cargo-xwin)"
else
  echo "cargo-xwin $cargo_xwin_version is required for non-Windows hosts." >&2
  echo "Install it with:" >&2
  echo "  cargo install cargo-xwin --version $cargo_xwin_version --locked --root target/xwin-tools" >&2
  exit 1
fi

actual_version="$("$cargo_xwin" --version)"
if [[ "$actual_version" != "cargo-xwin $cargo_xwin_version" ]]; then
  echo "cargo-xwin $cargo_xwin_version is required; found: $actual_version" >&2
  exit 1
fi

target_libdir="$(rustc --print target-libdir --target "$target")"
if [[ ! -d "$target_libdir" ]]; then
  echo "Rust target $target is not installed." >&2
  echo "Install it with: rustup target add $target" >&2
  exit 1
fi

host_libdir="$(rustc --print target-libdir)"
rust_bin_dir="$(dirname "$host_libdir")/bin"
if ! command -v llvm-ar >/dev/null 2>&1 && [[ ! -x "$rust_bin_dir/llvm-ar" ]]; then
  echo "Rust LLVM tools are required for llvm-lib and llvm-dlltool." >&2
  echo "Install them with: rustup component add llvm-tools-preview" >&2
  exit 1
fi

export XWIN_ARCH="x86_64"
export XWIN_VARIANT="desktop"
export XWIN_VERSION="17"
export XWIN_SDK_VERSION="$sdk_version"
export XWIN_CRT_VERSION="$crt_version"
export XWIN_CACHE_DIR="${XWIN_CACHE_DIR:-$root/target/xwin-cache}"

echo "Windows cross-check host: $host"
echo "Windows cross-check target: $target"
echo "cargo-xwin: $actual_version"
echo "xwin SDK/CRT: $sdk_version / $crt_version"
echo "xwin cache: $XWIN_CACHE_DIR"

exec "$cargo_xwin" xwin check \
  --locked \
  --target "$target" \
  -p "$package"
