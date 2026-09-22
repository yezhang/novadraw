# Core Architecture

类型：`documentation-index`

本目录定义 Novadraw Core 的规范架构。总体文档负责边界和导航，范围更窄的专题契约
负责具体行为；明确标记为 `proposal` 的文档不具有覆盖效力。

## 基础结构

| 文档 | 职责 |
|---|---|
| [overview.md](overview.md) | 总体职责、设计原则和权威边界 |
| [directory-structure.md](directory-structure.md) | Crate 与模块边界 |
| [static-architecture.md](static-architecture.md) | 类型、所有权与依赖结构 |
| [dynamic-architecture.md](dynamic-architecture.md) | 运行时交互和事务时序 |

## 核心协议

| 文档 | 职责 |
|---|---|
| [figure-style.md](figure-style.md) | Figure 样式继承与覆盖 |
| [figure-lifecycle.md](figure-lifecycle.md) | Figure 生命周期与 Runtime 身份 |
| [tree-search-and-focus.md](tree-search-and-focus.md) | 树查询、命中与焦点遍历 |
| [derived-state-convergence.md](derived-state-convergence.md) | 派生状态收敛事务 |
| [resource-lifecycle.md](resource-lifecycle.md) | 资源因果与 Backend Session |
| [figure-inspector.md](figure-inspector.md) | 稳定场景与提交后事件诊断 |
| [component-update.md](component-update.md) | 第三方组件更新提案 |

## Figure 能力

| 文档 | 职责 |
|---|---|
| [layer-and-freeform.md](layer-and-freeform.md) | Layer、Freeform 与范围传播 |
| [connection-routing.md](connection-routing.md) | Connection、Anchor 与 Router |
| [reusable-shape-border.md](reusable-shape-border.md) | Shape 与 Border 扩展 |
| [text-layout.md](text-layout.md) | Text Layout、Label 与标题边框 |
| [basic-widgets.md](basic-widgets.md) | Clickable、Button 与 Toggle |
| [tooltip-accessibility.md](tooltip-accessibility.md) | Tooltip 与 Accessibility Bridge |

坐标、输入和渲染专题分别见
[`../coordinates/`](../coordinates/coordinate-system.md)、
[`../input/`](../input/scroll-zoom-gesture-contract.md) 和
[`../rendering/`](../rendering/update-manager.md)。
