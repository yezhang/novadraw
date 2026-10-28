# P2-S04 Editor 开放协议完成记录

类型：`verification`

日期：2026-10-12

状态：`complete`

## 1. 范围

本批按
[ADR-034](../../adr/adr-034-editor-tool-request-handle-protocol.md)
关闭 API 与架构 Review R05 / TC-06：

- Request 使用 typed key、payload、metadata、contribution 和 reducer；
- EditPolicy 在安装时注册 typed handler；
- Policy slot 与 Handle action 使用开放 typed identity；
- EditorDomain 统一持有 `Box<dyn Tool<A>>`；
- 内置与外部 Tool、Request、Policy、Handle 共用同一注册、分派和错误通道。

## 2. 实施结果

- 新增 `RequestProtocol<A>`、`RequestKey<R>`、`RequestMetadata`、
  `RequestResolution<C>` 与私有 erased dispatch registry；公开调用保持 payload、
  contribution 和 reducer 的类型关系；
- `EditPolicy` 改为安装期注册 typed handler，`PolicyKey<P>` / `PolicySlot<P>`
  取代 `PolicyRole`，duplicate slot 与 handler descriptor 不一致均结构化拒绝；
- 内置 request 全部迁入 contribution/reducer 链，删除 `EditorRequest`、
  `EditPolicy::understands` 和三个内置 `start_*` 特权 hook；
- 新增 object-safe `Tool<A>`、`ToolContext` 与 `ToolKey<T>`；Selection、
  Connection Creation、Connection Endpoint、Connection Bendpoint 四个内置 Tool
  与外部 Tool 共用 `EditorDomain` 的 `Box<dyn Tool<A>>` 生命周期、输入和 fault 边界；
- 新增 `HandleActionProtocol` 与 typed factory registry，删除 `HandleRole`；
- overlay 标识符只从 `novadraw_editor::overlay::*` 公开，crate root 不再重导出；
  `VisualOwner` 字段已私有化。该项关闭 R08 的公开暴露问题，不代替 P2-S06 的
  OverlayBatch / ProjectionPlan prepared publication；
- Native node editor、Web direct edit 与 Editor G1-G5/P2-E02 调用点已迁移；
- 独立外部 consumer 在不修改 Editor Core 的条件下实现 Marquee Tool、
  AlignToGuide Request/Policy slot 与 GuideHandle action，并验证 duplicate slot、
  duplicate factory 和 Tool callback panic fault。

## 3. 验证

- `cargo clippy -p novadraw-editor --lib -- -D warnings`：PASS；
- `cargo xtask verify editor.p2-s04-open-protocol`：PASS；
- suite 包含 Editor G1-G5、P2-E02、node-editor 编译、开放协议 source gate 与独立
  external consumer；
- Editor 私有单元测试覆盖 duplicate request handler 与 descriptor mismatch；
- external consumer 覆盖 duplicate policy slot、duplicate handle factory 与 Tool
  callback panic fault；
- `cargo xtask check --quick`：PASS；
- `cargo xtask docs`：PASS；
- `git diff --check`：PASS；
- P2-S02/S03/S04 组合交付 `cargo xtask check --full`：PASS，包括 workspace
  Clippy、全部单元/集成/doctest、Native/Web backend 与公开 API 边界。

## 4. 关闭结论

ADR-034 已实现，R05 / TC-06 已关闭；R08 的 overlay 暴露面已关闭。TC-08/R06 的
结构化 prepared publication 仍归 P2-S06，不在本批提前关闭。P2-S05 尚未完成设计
冻结，因此审计整改队列当前没有 `in_progress` 批次。
