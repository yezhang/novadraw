# 官方样例的 egui 控制面板

类型：`verification`

日期：2026-10-20

状态：已接入；自动测试与原生截图验证

Grid、Flow、Toolbar 和 Tree 的参数面板使用 egui，Canvas 中保留 Draw2D Figure。
面板属于 `examples/support` 的宿主能力，通过可选 feature `egui-panel` 启用。
Core 不依赖 egui，也不承担 SWT 原生控件的替代实现。

每个场景工厂同时创建 Runtime 和面板控制器。控制器持有该场景的参数状态，
在一次 UI pass 中调用 Runtime 公开 API，修改布局、约束或子树。
Grid 和 Tree 的 Canvas 选择事件与面板共享选择模型，新增子树成功挂载后才发布模型身份。

Vello 后端通过可选 feature `native-overlay` 提供 GPU 合成入口。
egui 共用后端设备，在 Canvas 拷贝后、surface present 前绘制。截图路径在离屏纹理中完成合成。
输入先交给 egui，已消费的输入不会进入 Canvas 或场景切换快捷键。
面板重绘遵循 egui 的 repaint deadline；场景切换重建面板上下文和 GPU 资源。

运行：

```sh
cargo run -p draw2d-official-examples
cargo run -p draw2d-official-examples -- --screenshot=GridLayoutExample
```

用左右方向键切换场景。Grid 的数字编辑器可拖动修改，也可点击后输入；
选择 Canvas 图形后可修改该图形的 GridData。Tree 选择节点后可增删子节点、调整样式和展开状态。
面板窗口可移动、缩放和滚动。

旧 Figure 控件暂时仅在 `cfg(test)` 下编译，用于保留既有回归证据；
实际宿主场景不创建这些控件。新增测试检查实际场景中没有 mock 控件，并验证 egui 数字输入改变 Grid 布局。

验证结果：workspace Clippy 通过，workspace 测试 1,132 项通过、1 项忽略，
格式检查、文档检查与 `git diff --check` 通过。关闭默认 feature 的 support 编译通过。
已生成全部 12 个可运行入口的原生截图，Grid、Flow 和 Tree 面板已人工检查。

## CPU 与空闲重绘修复

宿主原先把 egui-winit 对 `RedrawRequested` 的 `repaint=true` 再次转成
`request_redraw`，形成持续重绘循环；每次 UI pass 又强制重绘整个 Canvas。
现在当前重绘事件不再预约下一帧，延迟重绘到期后先清除截止时间，再请求一帧。
无延迟任务时使用 `ControlFlow::Wait`，文本光标等定时任务仍保留截止时间。

面板缓存上一帧的 shapes 和 DPI，只有图形或纹理变化才重新 tessellate 并请求
Canvas 完整合成；场景参数修改仍走 Runtime 自身的失效路径。

macOS、debug 构建、默认 Grid 场景，启动后无输入运行约 9 秒再结束进程，
通过子进程的 user + system CPU 时间测量：修复前 7.377 秒，修复后 1.319 秒，
减少约 82%。该窗口包含 GPU 和字体初始化，不代表稳定空闲期的 CPU 百分比；
不包含 WindowServer 或 GPU 时间。

回归覆盖当前重绘不预约自身、截止时间只消费一次、空闲 UI 停止重绘、
文本光标保留延迟重绘，以及 egui 数字编辑改变 Grid 布局。
