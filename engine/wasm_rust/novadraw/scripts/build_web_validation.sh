#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="${CARGO_TARGET_DIR:-$ROOT/target}"
WASM_BINDGEN="${WASM_BINDGEN:-$ROOT/target/wasm-tools/bin/wasm-bindgen}"
WEB_APP="$ROOT/examples/web/web-validation"
DIST="$WEB_APP/dist"

NOVADRAW_GIT_REVISION="${NOVADRAW_GIT_REVISION:-$(git -C "$ROOT" rev-parse HEAD)}"
if [[ -z "${NOVADRAW_GIT_DIRTY:-}" ]]; then
  if [[ -n "$(git -C "$ROOT" status --porcelain)" ]]; then
    NOVADRAW_GIT_DIRTY=true
  else
    NOVADRAW_GIT_DIRTY=false
  fi
fi
export NOVADRAW_GIT_REVISION NOVADRAW_GIT_DIRTY

if [[ ! -x "$WASM_BINDGEN" ]]; then
  echo "wasm-bindgen not found at $WASM_BINDGEN" >&2
  echo "Install it with: cargo install wasm-bindgen-cli --version 0.2.127 --locked --root target/wasm-tools" >&2
  exit 1
fi

cargo build -p web-validation --target wasm32-unknown-unknown --release
rm -rf "$DIST"
mkdir -p "$DIST/pkg"
"$WASM_BINDGEN" \
  "$TARGET/wasm32-unknown-unknown/release/web_validation.wasm" \
  --target web \
  --out-dir "$DIST/pkg"
cp "$WEB_APP/web/index.html" "$DIST/index.html"
cp "$WEB_APP/web/styles.css" "$DIST/styles.css"

echo "Built $DIST"
