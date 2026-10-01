#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CHROME_BIN="${CHROME_BIN:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
NODE_BIN="${NODE_BIN:-node}"
EXPECTED_CHROME_MAJOR="${NOVADRAW_CHROME_MAJOR:-154}"
REPORT="${NOVADRAW_WEBGPU_REPORT:-$ROOT/target/verification/reports/ga2-webgpu-browser.json}"
PROFILE=
CHROME_PID=
CAFFEINATE_PID=

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "strict WebGPU browser performance evidence currently requires macOS" >&2
  exit 2
fi

console_is_locked() {
  ioreg -n Root -d1 | grep -q '"CGSSessionScreenIsLocked"=Yes'
}

if console_is_locked; then
  echo "WebGPU browser performance evidence requires an unlocked console session" >&2
  exit 3
fi

if [[ ! -x "$CHROME_BIN" ]]; then
  echo "Google Chrome executable not found at $CHROME_BIN" >&2
  exit 4
fi
if ! command -v "$NODE_BIN" >/dev/null 2>&1; then
  echo "Node.js executable not found: $NODE_BIN" >&2
  exit 4
fi

NODE_MAJOR="$("$NODE_BIN" -p 'Number(process.versions.node.split(".")[0])')"
if [[ "$NODE_MAJOR" != "24" ]]; then
  echo "strict WebGPU runner requires Node.js 24, got $("$NODE_BIN" --version)" >&2
  exit 4
fi

CHROME_VERSION="$("$CHROME_BIN" --version)"
if [[ ! "$CHROME_VERSION" =~ ^Google\ Chrome\ ${EXPECTED_CHROME_MAJOR}\. ]]; then
  echo "strict WebGPU runner requires Google Chrome ${EXPECTED_CHROME_MAJOR}.x, got $CHROME_VERSION" >&2
  exit 4
fi

"$ROOT/scripts/build_web_validation.sh"

EXPECTED_REVISION="$(git -C "$ROOT" rev-parse HEAD)"
rm -f "$REPORT"
PROFILE="$(mktemp -d "$ROOT/target/webgpu-chrome-profile.XXXXXX")"
CDP_PORT="$("$NODE_BIN" -e '
  const net = require("node:net");
  const server = net.createServer();
  server.listen(0, "127.0.0.1", () => {
    console.log(server.address().port);
    server.close();
  });
')"

cleanup() {
  if [[ -n "$CHROME_PID" ]]; then
    kill "$CHROME_PID" 2>/dev/null || true
    wait "$CHROME_PID" 2>/dev/null || true
  fi
  if [[ -n "$CAFFEINATE_PID" ]]; then
    kill "$CAFFEINATE_PID" 2>/dev/null || true
    wait "$CAFFEINATE_PID" 2>/dev/null || true
  fi
  if [[ -n "$PROFILE" ]]; then
    rm -rf "$PROFILE"
  fi
}
trap cleanup EXIT INT TERM

caffeinate -d -i -u -t 600 >/dev/null 2>&1 &
CAFFEINATE_PID=$!

"$CHROME_BIN" \
  --user-data-dir="$PROFILE" \
  --remote-debugging-address=127.0.0.1 \
  --remote-debugging-port="$CDP_PORT" \
  --no-first-run \
  --no-default-browser-check \
  --disable-background-networking \
  --disable-component-update \
  --disable-features=Translate \
  --force-device-scale-factor=1 \
  --window-position=0,0 \
  --window-size=1280,900 \
  --new-window \
  about:blank \
  >/dev/null 2>&1 &
CHROME_PID=$!

osascript -e 'tell application "Google Chrome" to activate' >/dev/null

DRIVER_ARGS=(
  "--cdp-url=http://127.0.0.1:$CDP_PORT"
  "--dist=$ROOT/examples/web/web-validation/dist"
  "--report=$REPORT"
  "--expected-revision=$EXPECTED_REVISION"
  "--expected-chrome-major=$EXPECTED_CHROME_MAJOR"
)
if [[ "${NOVADRAW_ALLOW_DIRTY_EVIDENCE:-0}" == "1" ]]; then
  DRIVER_ARGS+=(--allow-dirty)
fi
"$NODE_BIN" "$ROOT/scripts/run_webgpu_browser_performance.mjs" "${DRIVER_ARGS[@]}"

if console_is_locked; then
  echo "console session locked before WebGPU browser evidence completed" >&2
  exit 3
fi
if [[ ! -s "$REPORT" ]]; then
  echo "WebGPU browser runner did not create report: $REPORT" >&2
  exit 1
fi
