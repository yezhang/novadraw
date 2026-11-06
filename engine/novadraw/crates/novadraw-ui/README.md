# novadraw-ui

能力归属：其他——独立 egui 工作台组件。

本地维护 craft-ui 的停靠树、标签页、分割布局、浮窗、工具栏、按钮、
下拉控件、截图回调和 IME composer。源码可直接修改，不依赖任何 craft crate。
来源、固定提交和本地差异见 [SOURCE.md](SOURCE.md)。

应用实现 `DockContent<P>`，持有自己的 `Workspace<P>`，绘制后将返回的动作
交给 `apply_all` 校验并提交。`P` 由应用定义，外部面板无需修改 UI 库。
布局动作失败时整批回滚；持久化数据在绘制前调用 `validate`。
`Permissions::PROTECTED` 可保护文档 Canvas。

`serde` 启用布局持久化与数据命令；`native-windows` 启用独立原生浮窗；
`testing` 提供真实 egui 输入驱动的无窗口测试工具。
`canvas` 提供共享 eframe WGPU 设备的 `canvas::CanvasSurface`；
`editor-canvas` 再提供 `editor_canvas::EditorCanvas<A, F>` 和输入适配器
`EditorCanvasInput`，接入同一份 Viewer、Domain、工具和编辑会话。
应用负责主题、字体、国际化、文件、文档与撤销历史。
Core／Editor 不依赖本包。本包可以公开 egui 类型，属于 UI 专用接口。

```bash
cargo run -p novadraw-workbench-app
cargo test -p novadraw-ui --features serde,native-windows,testing,editor-canvas
cargo clippy -p novadraw-ui --lib --features serde,native-windows,testing,editor-canvas -- -D warnings
```

[工作台样例](examples/native/workbench-app/README.md)通过 eframe 和 Vello
共享 WGPU 30 设备绘制真实 Novadraw Figure，并通过可复用 Canvas
接入图形选择、拖动、创建、视口与同一份撤销历史。
依赖版本与契约见 [设计说明](../../doc/design/ui-workbench.md)。

Canvas 创建时将 `CreationContext::wgpu_render_state` 传给 `EditorCanvas::new`，
然后在面板 body 中调用 `canvas.show(ui)`。已有 Editor 状态可通过 `from_parts`
移入；只需输入适配的消费者使用 `EditorCanvasInput`，继续持有自己的会话封装。
隐藏或替换文档前调用 `deactivate`。设备重建后调用 `recreate_gpu`；
应用仍负责获取新的 RenderState 和最终窗口合成。
错误通过 `CanvasOutput.error` 返回，不在绘制路径打印日志。
完整边界及当前 Native 阻塞见
[Canvas 契约](../../doc/design/egui-editor-canvas.md)和
[验证记录](../../doc/verification/reviews/egui-editor-canvas-2026-11-06.md)。

egui、eframe、egui-winit、egui-wgpu 和 winit 使用 crates.io 官方发布版，
不需要下游配置 Git 依赖或 `[patch.crates-io]`。
IME 编辑目标身份由本地 composer／窗口记录持有，通过官方 `IMEOutput`
提交用途、候选窗口位置和中断请求。官方接口没有定制分支的 context generation、
原生重置确认事件和按字节删除事件；这些能力不计入当前验收。
RTL 由当前 `Ui` 的官方 Layout 控制。
