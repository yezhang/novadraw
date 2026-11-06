# Native 工作台样例

能力归属：其他——UI 工作台及 Canvas 宿主；编辑使用 Editor，绘制使用 Core 和 Vello。

```bash
cargo run -p novadraw-workbench-app
```

原生截图检查可运行
`cargo run -p novadraw-workbench-app -- --smoke-screenshot target/verification/workbench.png`。
它等待五帧后截图并关闭窗口；输出目录须已存在。截图记录真实宿主和 Canvas，
不代替跨窗口拖动及真实 IME 的人工验收。

左侧工具区、中间受保护的文档 Canvas、右侧 Layers／Inspector／Notes
均通过同一个 `DockContent` 接口绘制。拖动标题可拆分、重排或生成原生浮窗；
顶部开关隐藏和恢复面板，Remember／Restore 在本次进程中保存和恢复布局。
Notes 使用 egui 文本编辑，可检查浮窗中的输入法与焦点行为。

Canvas 显示真实 Novadraw Figure，Vello 与 eframe 共享 WGPU 30 设备，
窗口和分割栏调整后由 `novadraw-ui::editor_canvas::EditorCanvas` 管理纹理和帧提交。
选择、拖动、框选、矩形／椭圆创建以及顶部 Undo／Redo 使用同一份 Editor Domain。
Layers 接入真实模型选择；中键平移，滚轮滚动，Ctrl／Cmd+滚轮缩放。
本样例复用 Shapes 的应用模型和 Factory，UI 库不依赖该样例。
当前六图形连接场景受并行 Core 坐标投影预算错误影响，Native 完整验收尚未通过；
输入自动测试与 Native 截图分别记录在
[Canvas 验证](../../../../../doc/verification/reviews/egui-editor-canvas-2026-11-06.md)。
本样例不自动写文件；后续添加数据文件时放在本样例目录。

组件与权限协议见 [库入口](../../../README.md)及
[设计契约](../../../../../doc/design/ui-workbench.md)。
