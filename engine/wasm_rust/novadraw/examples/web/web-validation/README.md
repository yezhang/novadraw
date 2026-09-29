# Web Example and Validation

该示例展示如何在浏览器中组合 Novadraw `Runtime`、`WebInputAdapter`、
`WebPlatformHost` 和渲染后端，同时承担 Web 集成验证。Vello WebGPU 是默认后端，
Canvas2D 是诊断基线。

完整环境准备、操作步骤、通过标准和故障定位见
[`../../../doc/verification/manual/web-platform.md`](../../../doc/verification/manual/web-platform.md)。

快速启动：

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli \
  --version 0.2.127 \
  --locked \
  --root target/wasm-tools
cargo xtask verify web.build
cargo xtask manual web.build
```

默认入口为 <http://127.0.0.1:4173/?backend=vello>。页面从
`novadraw-example-scenes::catalog()` 加载 Native/Web 共用的验证场景；每个主题的场景
可通过画布上方的箭头切换。主题与场景可通过稳定 ID 复现，例如
<http://127.0.0.1:4173/?backend=vello&theme=viewport&scene=zoomed-content>。
数字场景索引仍作为兼容入口保留。

Canvas2D 对照入口为 <http://127.0.0.1:4173/?backend=canvas2d>。

`layer-freeform` 场景中：

- 普通滚轮或触控板双指滚动用于平移；
- `Ctrl+wheel` 用于以指针位置为锚点缩放；
- 浏览器上报为 `ctrlKey=true` wheel 的触控板 pinch 同样进入缩放链路。
