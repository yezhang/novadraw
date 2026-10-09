# TC-12 Render Backend Failure 完成记录

类型：`implementation-verification`

日期：2026-10-08

状态：`complete`

## 交付范围

TC-12 将 backend 提交合同从扁平 `RenderOutcome` 迁移为：

```text
RenderResult = Result<RenderDisposition, RenderFailure>
```

完成内容：

- `Presented / Skipped / Retry` 只表达成功处置；
- `Unsupported / InvalidInput / SurfaceLost / DeviceLost / OutOfMemory / Fatal`
  形成 backend-neutral 失败分类；
- `RenderFailureDiagnostic` 保存稳定摘要与 source chain，不暴露 Vello/WGPU 类型；
- `Runtime::complete_submission` 返回 `SubmissionAcknowledgement`，并对无 in-flight、
  session/frame 不匹配和 session id 耗尽返回结构化错误；
- surface lost 保留 backend session 并恢复 Full；
- device lost 由 Runtime 递增 session，下一帧强制 `Snapshot + Full`；
- 确定性拒绝与 OOM/Fatal 不进入自动重试；
- Vello 移除 `render_to_texture` panic，安装 device-lost 与 uncaptured-error callback；
- Native demo、Editor host 和 Web validation host 按 acknowledgement 分支；
- 旧 `RenderOutcome` 已从公开 API 快照和源码调用面退出。

## 故障注入

`tc12_render_failure_contract` 覆盖：

- 三种 disposition 的不同 acknowledgement；
- surface lost 不失效 backend session；
- device lost 失效旧 session 并重发 Snapshot；
- InvalidInput、OOM 与 Fatal 不折叠为 Retry；
- stale、duplicate 和 mismatched completion 被结构化拒绝。

Vello 单元测试覆盖 surface status 分类、device lost、OOM、fatal 注入及故障优先级。
真实 GPU device 破坏不作为自动门禁；Native/Web feature graph 分别编译。

## 验证

统一 suite：`core.tc12-render-failure`。

已通过：

- `cargo test -p novadraw --test tc12_render_failure_contract`；
- `cargo check -p novadraw-backend-vello --features native`；
- `cargo check -p novadraw-backend-vello --target wasm32-unknown-unknown --features web`；
- `cargo check --workspace`；
- Native `novadraw-example-support`、`node-editor-demo`、`ndcanvas-app` 与
  `native-vello-perf` 定向编译；
- Web `web-validation` wasm32 composition root 定向编译。

- `cargo xtask verify core.p2-m01-animation`；
- `cargo xtask verify core.tc04-typed-property-identity`；
- `cargo xtask verify core.p2-f02-figure-capability`；
- `cargo xtask docs`；
- `cargo xtask check --quick`；
- full gate 中 fmt、Facade、第三方类型、API snapshot、hot-path、Kurbo、
  Native/Web Vello 与 workspace Clippy 全部通过。

`cargo xtask check --full` 最终仅在 `workspace.test` 阶段被下述既有 Builder
`Result` 迁移问题阻断；TC-12 定向与相邻合同均已通过。

## 已知独立阻断

workspace `cfg(test)` 仍受此前 Builder `Result` 迁移未完成影响，表现为旧测试把
`Result<FigureId, GraphMutationError>` 当作 `FigureId` 使用，本次 full 报告 405 个
编译错误。该问题不属于 TC-12；
TC-12 定向测试、source workspace 编译及 backend feature 编译不依赖该旧调用面。

## 关系

- [ADR-029](../../adr/adr-029-structured-render-backend-failure.md)
- [临时概念设计审计](temporary-concept-design-audit-2026-10-08.md)
- [UpdateManager](../../design/rendering/update-manager.md)
- [动态架构](../../design/architecture/dynamic-architecture.md)
