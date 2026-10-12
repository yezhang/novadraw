#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EDITOR_SRC="$ROOT/novadraw-editor/src"

for removed in EditorRequest PolicyRole HandleRole ActiveTool; do
  if rg -n "\\b${removed}\\b" "$EDITOR_SRC"; then
    echo "removed closed Editor protocol remains: ${removed}" >&2
    exit 1
  fi
done

if rg -n 'fn (understands|start_connection|start_reconnection|start_direct_text_edit)\b' \
  "$EDITOR_SRC/policy/mod.rs"; then
  echo "built-in EditPolicy hook remains" >&2
  exit 1
fi

if rg -n "\\bGraphicalViewer\\b|\\bEditPartFactory\\b" "$EDITOR_SRC/tool/mod.rs"; then
  echo "built-in Tool bypasses ToolContext" >&2
  exit 1
fi

if rg -n '^pub (struct|trait|enum|type|fn)[^\\n]*\\b(Any|TypeId)\\b' \
  "$EDITOR_SRC/request/mod.rs" \
  "$EDITOR_SRC/policy/mod.rs" \
  "$EDITOR_SRC/tool/mod.rs" \
  "$EDITOR_SRC/feedback/mod.rs"; then
  echo "type erasure leaked into the public Editor protocol" >&2
  exit 1
fi

for required in \
  "pub trait RequestProtocol" \
  "pub struct RequestKey" \
  "pub struct RequestMetadata" \
  "pub struct PolicyHandlerBuilder" \
  "pub struct PolicyKey" \
  "pub trait PolicySlot" \
  "pub trait Tool" \
  "pub struct ToolContext" \
  "pub struct ToolKey" \
  "pub trait HandleActionProtocol" \
  "pub struct HandleActionKey"; do
  if ! rg -Fq "$required" "$EDITOR_SRC"; then
    echo "required open Editor protocol is missing: ${required}" >&2
    exit 1
  fi
done

CARGO_TARGET_DIR="$ROOT/target/verification/editor-open-protocol-consumer" \
  cargo test \
  --manifest-path "$ROOT/verification/consumers/editor-open-protocol/Cargo.toml" \
  --locked \
  --offline
