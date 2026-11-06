# novadraw-ui

能力归属：其他——独立 egui 工作台组件。

本地维护 craft-ui 的停靠树、标签页、分割布局、浮窗、工具栏、按钮、
下拉控件、截图回调和 IME composer。不依赖 craft crate；来源和本地差异见
[SOURCE.md](SOURCE.md)。egui／winit 使用 crates.io 官方发布版。

应用实现 `DockContent<P>` 并持有 `Workspace<P>`。绘制返回布局动作，
通过 `apply_all` 校验并原子提交；持久化布局在绘制前调用 `validate`。
`Permissions::PROTECTED` 可保护文档面板。

`serde` 启用持久化；`native-windows` 启用原生浮窗；`testing` 提供
真实 egui 输入驱动的无窗口测试工具。应用负责主题、字体、文件和文档命令。
本包可以公开 egui 类型，默认不依赖 Core、Editor 或 Vello。

```bash
cargo test -p novadraw-ui --features serde,native-windows,testing
cargo clippy -p novadraw-ui --lib --features serde,native-windows,testing -- -D warnings
```
