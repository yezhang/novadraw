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
| [runtime-ownership-and-module-boundaries.md](runtime-ownership-and-module-boundaries.md) | Runtime 所有权、tree/update 协作与扩展更新入口 |
| [public-api-contract.md](public-api-contract.md) | 当前公共 API 规范：领域路径、owner、权限、单位、失败与组合 |

## 核心协议

| 文档 | 职责 |
|---|---|
| [figure-style.md](figure-style.md) | Figure 样式继承与覆盖 |
| [detached-input-invariants.md](detached-input-invariants.md) | ADR-056 已接受：Border 构造、受检样式、外部准入与派生溢出 |
| [figure-lifecycle.md](figure-lifecycle.md) | Figure 生命周期与 Runtime 身份 |
| [tree-search-and-focus.md](tree-search-and-focus.md) | 树查询、命中与焦点遍历 |
| [derived-state-convergence.md](derived-state-convergence.md) | 派生状态收敛事务 |
| [deep-layout-presentation-convergence.md](deep-layout-presentation-convergence.md) | 深层布局盒模型与文字 Presentation 收敛 |
| [graph-projection-preparation.md](graph-projection-preparation.md) | 已实施修订 2：ADR-043 adapter 的候选快照、约束测量与必需路由发布增量，GP01–GP04 已处理，最终验证通过 |
| [resource-lifecycle.md](resource-lifecycle.md) | 资源因果与 Backend Session |
| [figure-inspector.md](figure-inspector.md) | 稳定场景与提交后事件诊断 |
| [ADR-057 草案](../../adr/adr-057-service-failure-and-inspector-lifecycle.md) | S09 待评审：服务结果、Inspector 本地故障、订阅所有权与扩展 source |
| [component-update.md](component-update.md) | 第三方组件 typed update 规范 |

## Figure 能力

| 文档 | 职责 |
|---|---|
| [figure-capability-model.md](figure-capability-model.md) | Figure typed capability 注册、发现、mutation 与生命周期统一模型 |
| [layer-and-freeform.md](layer-and-freeform.md) | Layer、Freeform 与范围传播 |
| [connection-routing.md](connection-routing.md) | Connection、Anchor 与 Router |
| [reusable-shape-border.md](reusable-shape-border.md) | Shape 与 Border；逐角圆角、逐边描边增量见 ADR-052 |
| [official-example-border-output-increment.md](official-example-border-output-increment.md) | 官方样例的 SchemeBorder、字体下划线指标与 ICO/帧输出消费者增量 |
| [official-example-capability-increment-2026-10-26.md](official-example-capability-increment-2026-10-26.md) | 现有能力内的样例修正；方向上下文、Cocoa 零宽映射、低 miter 和通用下划线已批准实施 |
| [shared-scene-view-background-proposal.md](shared-scene-view-background-proposal.md) | K07 修订 3 已批准，ADR-044/045 接受：保守结构依赖、Runtime 提交点版本校验、独立捕获与宿主背景统一提交；功能验收按切片推进 |
| [text-layout.md](text-layout.md) | Text Layout、Label 与标题边框 |
| [long-text-performance-proposal.md](long-text-performance-proposal.md) | 修订 2 首批设计通过，LT-00～LT-03 实施中；后续准入、预算与渐进协议待独立评审 |
| [long-text-scroll-resize-optimization-proposal.md](long-text-scroll-resize-optimization-proposal.md) | 待用户审核：滚动与 resize 的共享 IR、原生可见输出、宽度复用、调度及分阶段验收 |
| [long-text-incremental-layout-proposal.md](long-text-incremental-layout-proposal.md) | 待评审：基于第四轮测量与 Lapce／Xi 的 source delta、分阶段排版、安全巨段增量及可见输出细化 |
| [long-text-native-coverage-boundary.md](long-text-native-coverage-boundary.md) | 原生字形、数值转换与设备覆盖的定向核验；生产证书合同待复核 |
| [basic-widgets.md](basic-widgets.md) | Clickable、Button 与 Toggle |
| [tooltip-accessibility.md](tooltip-accessibility.md) | Tooltip 与 Accessibility Bridge |

坐标、输入和渲染专题分别见
[`../coordinates/`](../coordinates/coordinate-system.md)、
[`../input/`](../input/scroll-zoom-gesture-contract.md) 和
[`../rendering/`](../rendering/update-manager.md)。
