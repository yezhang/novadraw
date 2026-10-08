# Novadraw 设计

类型：`normative-design`

本目录包含 Novadraw 的规范设计和明确标记的候选提案；其中
`normative-design` 文档构成行为与架构契约的 SSOT：

- [`architecture/`](architecture/00-index.md)：组件职责、静态结构、动态时序和目录边界
- `coordinates/`：坐标域、变换、命中、事件点与 damage 投影
- [`editor/`](editor/00-index.md)：模型、EditPart、Viewer、Tool、Request、Policy 与 Command
- `input/`：平台无关输入与手势分发
- `animation/`：动画时钟、Timeline/Track、Presentation Plane 与领域 transition；
  ADR-026 已接受，按 M01-A 至 M01-D 实施
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
28. [`rendering/text-graphics-integration.md`](rendering/text-graphics-integration.md)：
    统一测量/绘制与 glyph 预处理合同，ADR-025 已接受，实现状态见 P2-G02
29. [`animation/animation-system.md`](animation/animation-system.md)：
    动画正交模型、可选性、时间驱动与 Presentation Plane
30. [`animation/public-api-contract.md`](animation/public-api-contract.md)：
    scoped mutable facade、typed channel、snapshot capture、取消/retarget 与错误
31. [`animation/capability-integration.md`](animation/capability-integration.md)：
    属性、布局、路由、视口、生命周期、持续效果及后续能力接入
32. [`animation/behavior-trigger-contract.md`](animation/behavior-trigger-contract.md)：
    Behavior/Trigger 的 scope、stable fact、coalescing、factory 与模式合同
33. [`architecture/figure-capability-model.md`](architecture/figure-capability-model.md)：
    Figure capability 的 typed descriptor registry、Runtime mutation 与迁移合同；
    ADR-027 已接受，P2-F02 已实现并通过验证

2026-09-10 的跨专题修订以
[ADR-014](../adr/adr-014-extensibility-and-lifecycle-boundaries.md) 为准；
规范效力不等于实现状态。专题文档必须分别声明两者，不能用 accepted 推断 implemented，
也不能沿用旧验证结果推断当前完成状态。

## 非规范提案

- [公共 API 统一设计与迁移](architecture/public-api-experience-proposal.md)：
  六包角色、命名、组合和迁移总入口；文字合同详见已接受的整合专题，
  录制完成接口及其他领域的迁移候选仍待评审
- [`rendering/display-list-protocol.md`](rendering/display-list-protocol.md)
- [`rendering/displaylist-implementation-plan.md`](rendering/displaylist-implementation-plan.md)

提案只有经过 ADR 接受后才能覆盖或扩展规范设计。
