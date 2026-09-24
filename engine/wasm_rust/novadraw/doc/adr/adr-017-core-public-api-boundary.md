# ADR-017: Core 公开 API 边界与失败契约

类型：`architecture-decision`

## 状态

已接受

## 背景

Draw2D Core 1.0 完成后，Novadraw 的行为语义已经闭合，但公开 API 仍保留迁移期痕迹：

- 规范身份已经是 `FigureId`，公开方法、句柄和事件仍使用 `block`；
- `FigureTree` 同时暴露构建期写入、运行期 primitive、查询、validation 和 render；
- LayoutManager 既有隐式作用于 contents/root 的入口，也有显式容器入口；
- 结构 mutation 同时使用 null handle、`bool`、panic 和 `Result` 表达失败；
- `FigureTree::validate` 只修改 valid bit，却使用了 Draw2D 完整验证协议的名称。

这些入口会被后续 G6 重建、持久化和投影代码继续消费。先继续功能演进会扩大 breaking
change 的调用面，并固化错误的阶段边界。

本决策落实
[`Core 公开 API 语义与命名审计`](../verification/reviews/core-public-api-audit-2026-09-22.md)
中的 P0 项，不处理 Graphics、Geometry alias 或聚合 crate 导出等 P1/P2 项。
后续 Runtime 驱动、坐标查询和结构化测量由
[ADR-018](adr-018-runtime-driving-and-measurement-api.md) 补充裁决。

## 决策

### 1. 公共语言统一为 Figure、node 和 container

`BlockId/FigureBlock/FigureGraph` 已不属于当前架构。公开 API 不再使用 `block`：

- 返回 `FigureNode` 的查询命名为 `node`；
- 树深度查询命名为 `depth`；
- Viewport/Scale typed handle 暴露 `figure_id`；
- Figure、Ancestor、Property、Action 和 Notification payload 使用 `figure_id`；
- Figure 引用属性值命名为 `PropertyValue::Figure`。

内部实现可以使用局部存储术语，但不得再次把 `block` 导出为公共概念。

### 2. FigureTreeBuilder 是唯一构建期写入口

pre-Runtime 场景组装通过短生命周期 `FigureTreeBuilder`：

```text
set_contents
add_child
set_layout_manager
set_layout_constraint
set/clear size override
set bounds and node flags
reorder children
validate subtree
```

`FigureTree` 保留稳定只读查询。Runtime、container 和 update 模块使用的底层 mutation
primitive 限制为 crate 内部。Builder 不承担 damage、interaction、resource 或
notification publication 事务。

### 3. LayoutManager 目标始终显式

删除隐式选择 `contents.unwrap_or(synthetic_root)` 的 LayoutManager getter/setter。

- 构建期：`FigureTreeBuilder::set_layout_manager(container, manager)`；
- 查询：`FigureTree::layout_manager(container)`；
- 运行期：`Runtime::{set_layout_manager,clear_layout_manager}(container, ...)`。

构建期和运行期替换 manager 都必须先校验 container 当前保存的全部 child constraints。

### 4. 结构 mutation 只保留 Result 入口

Runtime 的规范入口为：

```text
set_contents(...) -> Result<FigureId, RuntimeMutationError>
add_figure(...) -> Result<FigureId, RuntimeMutationError>
remove_figure(...) -> Result<bool, RuntimeMutationError>
reparent(...) -> Result<bool, RuntimeMutationError>
```

不再提供 `try_*` 与吞错 convenience 双入口。非法、foreign、disposed、detached 或
layered parent 等情况必须返回结构化错误；`Ok(false)` 只表示合法幂等操作没有改变状态。

Builder 的 fallible topology/layout 操作同样返回 `Result`，不返回
`FigureId::null()`。Null handle 可继续作为显式无效身份和测试输入，但不能表示正常 API
失败。

### 5. 删除伪 validation API

删除 `FigureTree::validate()`。它没有运行 LayoutManager、递归 validation 或
UpdateManager 两阶段事务，不能代表 Draw2D `Figure.validate()`。

- 构建期立即验证由 Builder 的 `validate_subtree` 表达；
- crate 内部需要无 damage 验证时调用明确的内部 primitive；
- 运行期只通过 Runtime 稳定化和 UpdateManager 两阶段事务验证。

### 6. 当前版本直接 breaking

Workspace 当前版本为 `0.1.0`，且所有已知调用点都在同一仓库内。本批次直接迁移并删除
旧名称，不保留 deprecated 双入口，避免继续扩大公共表面。

## 错误处理

- Builder topology 失败返回 `GraphMutationError`；
- Builder layout/constraint 失败返回 `LayoutError`；
- Runtime mutation 失败返回 `RuntimeMutationError`；
- panic 只保留给内部不变量破坏和明确的 fault boundary，不作为普通参数错误；
- public setter 不得以 unknown Figure 和幂等 no-op 共用同一个 `false`。

## 扩展点与稳定性

- 第三方 Figure、LayoutManager、Router 和 Backend 的扩展 trait 不因本决策收窄；
- `LayoutSnapshot` 与 `LayoutOutput` 继续作为 LayoutManager 的受控扩展协议；
- Inspector 需要的树状态通过只读 query/snapshot 提供，不以公开 mutation 换取可观测性；
- 后续 P1/P2 API 清理必须另建 delta，不混入本次 P0 迁移。

## 失败模式

- 构建期 layout constraint 与新 manager 不兼容时，manager replacement 原子失败；
- unknown/foreign/disposed Figure 不修改树、update queue 或派生状态；
- callback 延迟 mutation 继续按 FIFO 独立提交，错误进入既有 deferred error queue；
- 旧名称残留由源码检索和编译期调用面测试拦截。

## 验证

- Builder 与 Runtime 构造同一布局时得到等价树和几何；
- 结构 mutation 对 unknown/foreign/disposed Figure 返回结构化错误；
- 不兼容 constraint replacement 在构建期和运行期均不改变旧 manager；
- 删除 `FigureTree::validate` 后，validation 契约仍通过；
- 公开源码和非归档文档不再使用旧 `block` API；
- `cargo xtask docs`、`cargo xtask check --quick`；
- 本 breaking change 交付前执行一次 `cargo xtask check --full`。

## 关系

- 落实 [ADR-003](adr-003-rust-runtime-and-geometry-boundaries.md) 的 Runtime 所有权边界；
- 收口 [ADR-009](adr-009-runtime-dynamic-mutation-contract.md) 的错误模型；
- 保持 [ADR-014](adr-014-extensibility-and-lifecycle-boundaries.md) 的扩展和 fault 边界；
- 不改变 [ADR-016](adr-016-figure-inspector-observability.md) 的只读观察协议；
- 对应 `api_semantics`：
  `figure.tree`、`figure.geometry.bounds`、`layout.manager`、
  `validation.protocol`、`notification.figure`。

## 日期

2026-09-24
