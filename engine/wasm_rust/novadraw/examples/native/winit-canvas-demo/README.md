# 双 Canvas Novadraw 样例

类型：`implementation-guide`

一个 winit 窗口内放置两个 Canvas。每个 Canvas 拥有独立的
WinitCanvasHost、FigureTree、Runtime、VelloTextureRenderer 和 RGBA 纹理。
两侧都由 Novadraw 绘制矩形和 Label；应用只负责窗口事件接入和最终纹理合成。

```sh
cargo run -p winit-canvas-demo
cargo run -p winit-canvas-demo -- --verify
cargo run -p winit-canvas-demo -- --verify-resize
```

按住矩形拖拽，Tab 切换 Canvas 焦点，方向键移动获得焦点的 Canvas，
R 恢复 Canvas 布局。窗口 resize 时更新两个 Runtime 的根 Figure 尺寸和纹理。
矩形位置以 Runtime 的 FigureTree 为准，宿主只保存拖拽时的指针偏移。
拖拽为本样例的消费者行为，不代表完整的 Figure 键盘/IME 编辑组件。

绘制通过 Host 的 SurfaceInfo 准备提交，backend 写入纹理后，
Runtime 完成提交，再确认对应 Canvas 的重绘版本。
未变化的 Canvas 复用已有纹理。窗口 surface 与 GPU device 由两个后端共享，
场景状态、资源缓存和 backend session 各自独立。
suspend/resume 后重建两套后端并重置两个 Runtime 的 backend session。

`--verify` 检查两个 Runtime 的 session 不同、局部坐标拖拽隔离、
未变化纹理复用、Canvas 移动、旧位置像素清除，以及两侧矩形颜色和文字 glyph 提交。
Native 截图和结果位于 `target/verification/winit-canvas/`：
`native-before.png`、`native-after.png`、`result.txt`。

窗口呈现复用 `winit-canvas::gpu::presentation`：macOS 上固定旧 drawable
到左上角、配置 opaque 背景，窗口尺寸或 DPI 变化时在 acquire 前开启事务呈现，
present 后恢复普通模式。失败时保留上一帧尺寸，以便重试新尺寸帧。
绘制前重新核对 Window 的实际尺寸，避免 redraw 先于 Resized 到达时使用旧几何。
新建、恢复窗口 surface 时重新配置呈现策略；同尺寸输出纹理直接复用。

`--verify-resize` 执行九次真实窗口尺寸变化，覆盖只变宽、只变高、
双轴缩放和恢复，检查 Window、Canvas、Runtime 与纹理尺寸一致、
Figure 位置保持不变，以及事务呈现模式的开启/恢复。
结果与截图为 `novadraw-resize-result.txt` 和 `novadraw-resize.png`，
位于同一验证目录。

纯 WGPU 的独立组件样例仍位于 `winit-canvas/examples/standalone/`，
可通过 `cargo run -p winit-canvas --example standalone --features gpu` 运行。
