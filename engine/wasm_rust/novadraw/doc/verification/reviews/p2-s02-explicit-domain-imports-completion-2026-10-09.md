# P2-S02 显式领域导入与 Crate Root 收口完成记录

类型：`verification`

日期：2026-10-09

状态：`complete`

## 1. 范围

本批按
[ADR-035](../../adr/adr-035-explicit-domain-imports-and-root-boundary.md)
关闭 API 与架构 Review R02：

- 删除库级 `novadraw::prelude`；
- crate root 只公开领域模块；
- workspace 调用点迁移到唯一领域路径；
- public API snapshot 删除 prelude scope，并拒绝 root 非 module 项。

## 2. 实施结果

- `novadraw/src/prelude.rs` 与 `pub mod prelude` 已删除；
- `novadraw/src/lib.rs` 的类型重导出已降为 crate-private 内部别名；
- `Color` 的规范路径为 `novadraw::graphics::Color`；
- Figure/Style、Geometry、Layout、Tree、Runtime 类型分别从对应领域模块导入；
- Editor、Inspector、backend、platform、examples、benchmarks 和 integration tests
  已迁移；
- `examples/support` 保留应用本地 prelude，但其内容显式来自各领域模块；
- README 与应用开发 book 已改为显式领域导入；
- API snapshot root 只包含 module，prelude scope 已删除；
- source gate 禁止 Rust 代码重新使用 `novadraw::prelude`，并禁止 Core 重新发布
  `pub mod prelude`。

历史 `verification/evidence/` 中冻结的源码快照未改写。

## 3. 验证

- `cargo check --workspace`：PASS；
- `cargo test -p novadraw --no-run`：PASS；
- `cargo test -p novadraw-editor --no-run`：PASS；
- `cargo test -p novadraw-inspector --no-run`：PASS；
- `cargo test -p novadraw-platform-winit --no-run`：PASS；
- `cargo test -p novadraw-platform-web --no-run`：PASS；
- `cargo xtask verify core.facade`：PASS；
- `cargo xtask verify core.api-surface-hot-path`：PASS；
- `cargo xtask check --quick`：PASS；
- P2-S02/S03/S04 组合交付 `cargo xtask check --full`：PASS；
- `git diff --check`：PASS。

## 4. 关闭结论

R02、API-F07 和 ADR-021 的旧 root/prelude 分层已由 ADR-035 替代。TC-13 的
API snapshot 机制继续保留，但现在以 module-only root 和领域模块作为冻结对象。

下一批为 P2-S03 Runtime 子系统 Facade 与统一 Figure Mutation。
