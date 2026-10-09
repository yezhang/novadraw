# TC-07 Viewer 编辑与驱动边界完成记录

类型：`implementation-verification`

日期：2026-10-09

状态：`complete`

## 交付范围

TC-07 按 ADR-032 将 Viewer 模型写入与宿主驱动收口为：

```text
EditorDomain
-> ViewerEditSession
-> CommandStack operation
-> commit / reject / fault
-> revision validation and projection synchronization

external model update
-> synchronize_model callback or shared adapter notification
-> synchronize

host loop
-> ViewerDrive
-> frame / backend session / surface / time / focus / resources
```

完成内容：

- 删除公开 `GraphicalViewer::model_mut`；
- 删除公开 `GraphicalViewer::refresh`，统一为 `synchronize`；
- `synchronize_model` 提供不进入 CommandStack 的显式外部更新边界；
- crate-private `ViewerEditSession` 统一 execute、undo、redo、Tool release 和 direct-edit
  accept 的模型借用与投影关闭；
- 成功命令自动校验 revision、drain 通知并刷新投影；
- 可恢复错误若仍发布模型 revision，返回 `RejectedEditChangedModel` 并 fault Viewer；
- unknown-state error 与扩展 panic 同时关闭 CommandStack 和 Viewer；
- `ViewerDrive` 成为 Native/Web host 的 frame、surface、time、focus 和资源驱动入口；
- Viewer 上的同义宿主 forwarding 方法已删除；
- `CommandStack::has_undo/has_redo` 成为纯查询；
- `check_undo/check_redo` 显式区分扩展拒绝与 panic，完成 TC-10 的 CommandStack 部分；
- compile-fail doctest 防止 `model_mut` 和直接 frame drive 入口回归。

## 所有权与失败

`ViewerEditSession` 只在 crate 内创建，向一次 CommandStack operation 提供短生命周期模型
写借用。session 必须显式：

- `commit`：同步已提交模型 revision；
- `reject`：验证可恢复失败没有可观测模型变化；
- `fault`：保留未知模型状态，不尝试投影。

未关闭的 session 在 Drop 时 fault Viewer，且 Drop 不 panic。公共 `synchronize` 的任何
错误和扩展 panic 都会 fault Viewer；调用方不能在失败后继续编辑。

`synchronize_model` 的 callback 不返回模型借用，只用于加载、远端同步、测试输入或
其他明确不属于本地 undo history 的变化。普通编辑仍必须经 EditorDomain 和 Command。

## 自动验证

统一 suite：`editor.tc07-viewer-boundary`。

覆盖：

- 2 项 compile-fail Editor doctest；
- G1 CommandStack：15 项；
- G2 Viewer projection：21 项；
- G3 Viewer interaction：8 项；
- G4 editing loop：18 项；
- G5 connection projection：19 项；
- G5 create/reconnect/bendpoint：20 项；
- P2 direct text edit：10 项；
- GA-4 external Editor visual：2 项；
- Native node editor host 编译；
- Web direct-edit host wasm32 编译。

额外内环：

- `cargo check -p novadraw-editor --lib`；
- `cargo clippy -p novadraw-editor --lib -- -D warnings`；
- `cargo check -p node-editor-demo --bin node-editor-demo`；
- `cargo check -p web-validation --lib --target wasm32-unknown-unknown`；
- `cargo xtask verify editor.tc07-viewer-boundary`；
- `cargo xtask docs`；
- `cargo xtask check --quick`；
- `git diff --check`。

`cargo xtask check --full` 的 fmt、facade、第三方类型、API snapshot、hot-path、
Core I/O、Kurbo、Native/Web Vello 与 workspace Clippy 阶段全部通过；最终仅在
`workspace.test` 被既有迁移问题阻断：

- `novadraw-inspector/tests/figure_inspector_contract.rs` 有 3 个 TC-13 收窄前的 root
  导入和 2 个 Builder `Result` 未解包错误；
- `benchmarks/native-vello-perf/src/main.rs` 有 1 个 TC-13 收窄前的
  `ComponentInvalidation` root 导入。

这些目标不依赖 Editor Viewer/Domain/CommandStack，本批定向 suite 与 workspace
生产编译不受影响。

## TC-10 剩余边界

本批只完成 CommandStack 的查询/检查拆分。以下 TC-10 内容仍保持打开：

- Animation mutation 的 fault 与幂等返回分类；
- extension error source 保留；
- Inspector lock poison 与 subscription detach 生命周期；
- 与 TC-03/TC-09 一起收口的其余值和查询语义。

## 关系

- [ADR-032](../../adr/adr-032-viewer-edit-session-and-drive-boundary.md)
- [Editor 架构](../../design/editor/architecture.md)
- [临时概念设计审计](temporary-concept-design-audit-2026-10-08.md)
- [公共 API 后续审计](framework-public-api-follow-up-audit-2026-10-07.md)
