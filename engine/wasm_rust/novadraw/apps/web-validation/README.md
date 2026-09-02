# Web Validation

This application exercises the Novadraw `Runtime`, `WebInputAdapter`, and
`WebPlatformHost` in a browser with a Canvas2D `RenderBackend`.

## Prerequisites

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.127 --locked --root target/wasm-tools
```

## Run

```bash
./scripts/build_web_validation.sh
./scripts/serve_web_validation.sh
```

Open <http://127.0.0.1:4173/>.

The page exposes browser-verification state on the `body` element:

- `data-ready`
- `data-frame-count`
- `data-pointer-events`
- `data-wheel-events`
- `data-key-events`

Pointer, wheel, and keyboard input are dispatched through the same Runtime
event path used by native applications. The DPR button exercises surface
resize and full redraw at 1x and 2x scale factors.
