#!/usr/bin/env sh
set -eu

platform="${1:-}"
kernel="$(uname -s)"

case "$platform" in
  macos)
    if [ "$kernel" != "Darwin" ]; then
      echo "macOS validation requires a Darwin host; current kernel: $kernel" >&2
      exit 2
    fi
    exec cargo run -p node-editor-demo
    ;;
  windows)
    case "$kernel" in
      MINGW*|MSYS*|CYGWIN*)
        exec cargo run -p node-editor-demo
        ;;
      *)
        echo "Windows validation requires a native Windows host; current kernel: $kernel" >&2
        exit 2
        ;;
    esac
    ;;
  linux-x11)
    if [ "$kernel" != "Linux" ] || [ -z "${DISPLAY:-}" ]; then
      echo "Linux X11 validation requires a Linux host with DISPLAY set" >&2
      exit 2
    fi
    unset WAYLAND_DISPLAY
    exec cargo run -p node-editor-demo
    ;;
  linux-wayland)
    if [ "$kernel" != "Linux" ] || [ -z "${WAYLAND_DISPLAY:-}" ]; then
      echo "Linux Wayland validation requires a Linux host with WAYLAND_DISPLAY set" >&2
      exit 2
    fi
    unset DISPLAY
    exec cargo run -p node-editor-demo
    ;;
  *)
    echo "usage: $0 {macos|windows|linux-x11|linux-wayland}" >&2
    exit 2
    ;;
esac
