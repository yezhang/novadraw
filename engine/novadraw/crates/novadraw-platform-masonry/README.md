# Masonry / Xilem 编辑画布

类型：`implementation-guide`

`session::EditorSession` 持有 Viewer、EditorDomain 和逻辑 imaging recording。
`widget::EditorCanvas` 处理 Masonry 输入、capture、取消及单次 Scene 更新。
`view::editor_canvas` 用 build/sync/action 回调接入 Xilem，重建保留编辑会话。

该包属于根 workspace，但不在默认构建成员中。Native UI 使用未修改的上游 Git
提交 `66c8893f12232b9d5cdf4296f773aa3dc07ee776`，不需要本地 vendor。
Core/Editor 不依赖它。UI 要求 Rust 1.96；安装该工具链不必改变全局默认版本。

```sh
cargo +1.96.0 check -p novadraw-platform-masonry --lib
cargo +1.96.0 run -p gef-editor-demo
cargo +1.96.0 test -p gef-editor-demo --lib -- --test-threads=1
```

build 回调只创建一次 session，sync 回调消费应用动作，返回是否需准备新 Scene。
不要在 sync 中重新创建 Viewer。CanvasAction 回传 undo/redo 资格及失败诊断，
应用可在 action 回调读取自己需要的 UI 状态。paint 只合成缓存 Scene；
输入、layout/rebuild、DPI 变化和 Retry 请求一次动画回调进行完整帧提交。
Native 应用组合 `host::run`，由公开的上游宿主委托入口处理 DPI / exposure。
绘制目标支持默认 AA、Core 文字轮廓、路径、裁剪、描边、渐变和图像；
逐操作 AA 与原生 glyph run 明确拒绝。完整边界见
[接入契约](../../doc/design/editor/masonry-xilem-canvas-integration.md)。

普通画布接受键盘焦点，Escape、失焦、禁用、stash 与 teardown 取消工具反馈；
不接受平台 IME。完整文字输入、系统 AT、模板拖放、窗口外 capture 与自动 expose
仍需后续桥接验收。内置 Canvas description 只说明画布，不代表已提供节点级 AT。
