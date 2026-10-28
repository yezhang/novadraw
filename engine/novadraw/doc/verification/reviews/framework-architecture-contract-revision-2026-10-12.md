# 图形框架五项架构契约修订记录

类型：`verification`

日期：2026-10-12

状态：规范修订完成，文档门禁通过。新增行为要求尚未执行实现核验。

## 1. 范围

用户已确认落实
[文档与架构审查](framework-documentation-and-architecture-review-2026-10-12.md)
后提出的五项调整。本次修改 design、受影响 ADR 和阅读入口，不修改 Rust 实现，
不物理归档全部候选文档，也不改写历史审查或完成报告。

读取时工作区已包含 P2-S05 的实现、测试、manifest、backlog 和索引修改。
这些既有修改保留；本记录不把它们计为本次文档调整的实现产物。

## 2. 五项落点

| 调整 | 规范落点 | 同步结果 |
|---|---|---|
| Figure mutation 统一 | [公共 API](../../design/architecture/public-api-contract.md)、[Capability](../../design/architecture/figure-capability-model.md)、[所有权](../../design/architecture/runtime-ownership-and-module-boundaries.md) | FigureMut 同时承载节点/容器/typed update；删除 ADR-019/027 正文中的内置 concrete facade 例外；ADR-033 同步 S03 完成态与 12 个服务视图 |
| 当前值、NoChange 与 effect 权限 | [组件更新](../../design/architecture/component-update.md) | prepare 读当前提交前缀；NoChange 不提交、不提升 revision、不产生 work/journal/damage；资源/结构 effect 在写入前完整预检 namespace、scope 和原子性 |
| 统一版本与阶段发布 | [派生状态收敛](../../design/architecture/derived-state-convergence.md)、[UpdateManager](../../design/rendering/update-manager.md) | measurement/layout/route/post-route/presentation 统一依赖版本、过期候选和发布合同；final presentation 完成后才晋升 stable epoch |
| 固定机制与开放策略分离 | [总览](../../design/architecture/overview.md)、[Capability](../../design/architecture/figure-capability-model.md)、[树查询](../../design/architecture/tree-search-and-focus.md) | 固定调度/校验/提交/隔离/出版，开放 layout/projection/clip/hit/presentation 策略；Draw2D 模板作为默认兼容模型，其他合成次序仍需独立合同与评审 |
| 当前公共 API 专题 | [公共 API 契约](../../design/architecture/public-api-contract.md) | 补齐模块职责、owner、路径、权限、单位、失败与组合；总索引及设计索引同步；旧综合 proposal 降为历史入口 |

静态结构中的旧 accessor、无失败 paint 示意和可写 submission 字段改为角色/权限图。
动态时序、UpdateManager、ADR-018/031 的公开帧路径同步为 RuntimeFrames；
Runtime 仍是唯一提交者，CommandRecorder 不成为第二个帧出版者。

## 3. 实现证据与待验证要求

已有完成记录证明的范围：

- [P2-S02](p2-s02-explicit-domain-imports-completion-2026-10-09.md)：显式领域路径、
  root 只含模块、无库级 prelude。
- [P2-S03](p2-s03-runtime-facade-completion-2026-10-10.md)：统一 Figure mutation、
  12 个无状态 facade、共用 prepare/effect validation/commit、显式 no-op、
  属性/资源/结构 effect。
- [P2-S05](p2-s05-deep-layout-presentation-completion-2026-10-12.md)：content/outer/insets/
  projection 顺序、嵌套布局首帧 glyph/clip、变化后的 old/new partial damage，
  外部容器与布局同路径。

这些记录不自动证明本次新增的全部规范要求。后续进入相关实现批次时，先冻结以下
定向证据：

1. 连续提交及 deferred FIFO 的 prepare 当前值；NoChange 对 revision/work/journal/
   damage/资源/结构关系的完整零变化，包括 revision 容量边界。
2. 非法资源版本、foreign identity、越权结构 effect 和 effect batch 末项失败时，
   组件与所有引擎状态在 commit 前保持不变。
3. 约束/资源/geometry 版本变化时的过期候选拒绝，final presentation 失败时的
   stable epoch/通知/帧一致性；外部测量与发布权限对等。
4. disabled 可见 Figure 的纯几何查询，输入资格仍拒绝；外部 clip/projection/
   branch containment 的 paint/hit/damage 一致性。

以上是规范验收要求，未经代码审查和契约运行不能登记为已确认实现缺陷。
不创建新的 P2 编号，不重新打开已完成 S05，不把 Core 要求混入下一批 Editor S06。
后续定级、批次归属及状态继续由 [P2 backlog](../../roadmap/p2-delta-backlog.md)
和审计闭环维护。本次保留现行递归模板，没有新增任意 renderer hook、通用 DAG、
全局 registry、空 crate 或全系统 Result 包装。

## 4. 验证

- `cargo xtask docs`：PASS；manifest 96 commands / 54 suites，文档元数据、链接、
  xtask 引用、索引分类与 public API probes 有效；32 项 doctest 通过。
- `git diff --check`：PASS。
- 本次未运行 Rust 行为、像素、性能或 workspace full gate；上述 S02/S03/S05
  的 PASS 属于其各自完成记录，不能作为本次新增行为验证结果。
