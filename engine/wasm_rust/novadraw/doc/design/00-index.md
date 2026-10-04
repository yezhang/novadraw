# Novadraw 设计

类型：`normative-design`

本目录包含 Novadraw 的规范设计和明确标记的候选提案；其中
`normative-design` 文档构成行为与架构契约的 SSOT：

- [`architecture/`](architecture/00-index.md)：组件职责、静态结构、动态时序和目录边界
- `coordinates/`：坐标域、变换、命中、事件点与 damage 投影
- [`editor/`](editor/00-index.md)：模型、EditPart、Viewer、Tool、Request、Policy 与 Command
- `input/`：平台无关输入与手势分发
- `rendering/`：UpdateManager 与渲染提交协议；DisplayList 文件仅为 proposal

`architecture/overview.md` 是导航和总体约束；出现细节冲突时，范围更窄的专题设计
优先。设计可以引用 `reference/` 作为依据，但不得把外部源码描述直接当作本项目契约。

## 核心设计

1. [`architecture/overview.md`](architecture/overview.md)
2. [`architecture/static-architecture.md`](architecture/static-architecture.md)
3. [`architecture/dynamic-architecture.md`](architecture/dynamic-architecture.md)
4. [`architecture/figure-style.md`](architecture/figure-style.md)
5. [`architecture/resource-lifecycle.md`](architecture/resource-lifecycle.md)
6. [`architecture/tree-search-and-focus.md`](architecture/tree-search-and-focus.md)
7. [`architecture/layer-and-freeform.md`](architecture/layer-and-freeform.md)
8. [`architecture/connection-routing.md`](architecture/connection-routing.md)
9. [`architecture/reusable-shape-border.md`](architecture/reusable-shape-border.md)
10. [`architecture/text-layout.md`](architecture/text-layout.md)
11. [`coordinates/coordinate-system.md`](coordinates/coordinate-system.md)
12. [`input/scroll-zoom-gesture-contract.md`](input/scroll-zoom-gesture-contract.md)
13. [`rendering/update-manager.md`](rendering/update-manager.md)
14. [`architecture/derived-state-convergence.md`](architecture/derived-state-convergence.md)
15. [`architecture/figure-lifecycle.md`](architecture/figure-lifecycle.md)
16. [`architecture/tooltip-accessibility.md`](architecture/tooltip-accessibility.md)
17. [`architecture/figure-inspector.md`](architecture/figure-inspector.md)
18. [`editor/architecture.md`](editor/architecture.md)
19. [`editor/g5-connection-projection.md`](editor/g5-connection-projection.md)
20. [`editor/g5-connection-creation.md`](editor/g5-connection-creation.md)
21. [`editor/g5-connection-reconnect.md`](editor/g5-connection-reconnect.md)
22. [`editor/g5-connection-bendpoint.md`](editor/g5-connection-bendpoint.md)
23. [`editor/g5-viewport-autoexpose.md`](editor/g5-viewport-autoexpose.md)
24. [`rendering/image-source-rectangle.md`](rendering/image-source-rectangle.md)
25. [`architecture/component-update.md`](architecture/component-update.md)
26. [`architecture/runtime-ownership-and-module-boundaries.md`](architecture/runtime-ownership-and-module-boundaries.md)
27. [`rendering/p2-g01-graphics-extension.md`](rendering/p2-g01-graphics-extension.md)

2026-09-10 的跨专题修订以
[ADR-014](../adr/adr-014-extensibility-and-lifecycle-boundaries.md) 为准；
规范效力不等于实现状态。专题文档必须分别声明两者，不能用 accepted 推断 implemented，
也不能沿用旧验证结果推断当前完成状态。

## 非规范提案

- [`rendering/display-list-protocol.md`](rendering/display-list-protocol.md)
- [`rendering/displaylist-implementation-plan.md`](rendering/displaylist-implementation-plan.md)

提案只有经过 ADR 接受后才能覆盖或扩展规范设计。
