#!/bin/sh
set -eu

unset CDPATH
SCRIPT_DIR=$(cd -- "$(dirname -- "$0")" && pwd)
WORKSPACE=$(cd -- "$SCRIPT_DIR/../.." && pwd)
APP_DIR="$WORKSPACE/target/native-vello-perf/NovadrawNativeVelloPerf.app"
CONTENTS_DIR="$APP_DIR/Contents"
MACOS_DIR="$CONTENTS_DIR/MacOS"
REPORT=

if [ "$(uname -s)" != "Darwin" ]; then
  echo "native Vello app-bundle runner requires macOS" >&2
  exit 2
fi

if ioreg -n Root -d1 | grep -q '"CGSSessionScreenIsLocked"=Yes'; then
  echo "native Vello surface-present measurement requires an unlocked console session" >&2
  exit 3
fi

for argument in "$@"; do
  case "$argument" in
    --report=*)
      REPORT=${argument#--report=}
      ;;
  esac
done

cargo build --release -p native-vello-perf
rm -rf "$APP_DIR"
install -d "$MACOS_DIR"
install -m 644 "$SCRIPT_DIR/macos/Info.plist" "$CONTENTS_DIR/Info.plist"
install -m 755 "$WORKSPACE/target/release/native-vello-perf" "$MACOS_DIR/native-vello-perf"
codesign --force --deep --sign - "$APP_DIR" >/dev/null

if [ -n "$REPORT" ]; then
  case "$REPORT" in
    /*)
      REPORT_PATH=$REPORT
      ;;
    *)
      REPORT_PATH="$WORKSPACE/$REPORT"
      ;;
  esac
  rm -f "$REPORT_PATH"
fi

caffeinate -d -i -u -t 600 >/dev/null 2>&1 &
CAFFEINATE_PID=$!
trap 'kill "$CAFFEINATE_PID" 2>/dev/null || :' 0 1 2 15

open -n -W "$APP_DIR" --args \
  "--workspace=$WORKSPACE" \
  --require-surface-present \
  "$@"

kill "$CAFFEINATE_PID" 2>/dev/null || :
wait "$CAFFEINATE_PID" 2>/dev/null || :
trap - 0 1 2 15

if [ -n "$REPORT" ] && [ ! -s "$REPORT_PATH" ]; then
  echo "native Vello app did not create report: $REPORT_PATH" >&2
  exit 1
fi
