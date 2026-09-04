# Web Validation

该应用通过浏览器验证 Novadraw `Runtime`、`WebInputAdapter`、
`WebPlatformHost` 和渲染后端。Vello WebGPU 是默认后端，Canvas2D 是诊断基线。

完整环境准备、操作步骤、通过标准和故障定位见
[`../../../doc/verification/manual/web-platform.md`](../../../doc/verification/manual/web-platform.md)。

快速启动：

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli \
  --version 0.2.127 \
  --locked \
  --root target/wasm-tools
./scripts/build_web_validation.sh
./scripts/serve_web_validation.sh
```

默认入口为 <http://127.0.0.1:4173/?backend=vello>。页面从
`novadraw-demo-scenes::catalog()` 加载 Native/Web 共用的验证场景；每个主题的场景
可通过画布上方的箭头切换。主题与场景可通过稳定 ID 复现，例如
<http://127.0.0.1:4173/?backend=vello&theme=viewport&scene=zoomed-content>。
数字场景索引仍作为兼容入口保留。

Canvas2D 对照入口为 <http://127.0.0.1:4173/?backend=canvas2d>。
