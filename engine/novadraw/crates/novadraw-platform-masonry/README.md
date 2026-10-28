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
同一文档的 rebuild 不重新创建 Viewer；明确打开另一文档时由应用替换 session。
CanvasAction 回传 undo/redo 资格、保存点、model revision、工具与失败诊断。
`.project(...)` 在稳定 action 时投影模型、选择及画布窗口逻辑 bounds，
`.on_key(...)` 提供焦点内应用快捷键；返回 false 时可表示键继续交给 Tool。
paint 只合成缓存 Scene；
输入、layout/rebuild、DPI 变化和 Retry 请求一次动画回调进行完整帧提交。
Native 应用组合 `host::run`，由公开的上游宿主委托入口处理 DPI / exposure。
绘制目标支持默认 AA、Core 文字轮廓、路径、裁剪、描边、渐变和图像；
逐操作 AA 与原生 glyph run 明确拒绝。完整边界见
[接入契约](../../doc/design/editor/masonry-xilem-canvas-integration.md)。

普通画布接受键盘焦点，Escape、失焦、禁用、stash 与 teardown 取消工具反馈；
不接受平台 IME。`template::template_source` 是携带应用 typed payload 的模板源，
支持点击、窗口内 capture 拖放和取消，应用根据窗口逻辑点判断 drop 目标。
滚轮与中键平移走 EditorDomain 视口协议，动画驱动现有 auto-expose。
pointer feedback 在 widget 内更新；稳定快照、选择、会话和画布尺寸变化才回传 action。
完整文字输入、系统 AT、窗口外 capture 和原生 auto-expose 人工验收仍待完成。
内置 Canvas description 只说明画布，不代表已提供节点级 AT。
