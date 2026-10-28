# Draw2D Reference

类型：`documentation-index`

本目录记录 Eclipse Draw2D 源码事实。对标范围仅限 `org.eclipse.draw2d`；这些分析
不直接定义 Novadraw 行为，采用关系以
[`../../parity/draw2d/api-coverage.md`](../../parity/draw2d/api-coverage.md) 为准。

## Architecture

- [设计公理](architecture/design-axioms.md)
- [演进历史](architecture/history.md)

## Figure

- [核心概念](figure/core-concepts.md)
- [IFigure 接口](figure/ifigure-interface.md)
- [Figure 实现](figure/figure-implementation.md)
- [树操作](figure/tree-operations.md)
- [树与坐标](figure/tree-coordinates.md)
- [Bounds](figure/bounds.md)
- [盒模型](figure/box-model.md)
- [布局约束](figure/layout-constraints.md)
- [Scalable Figure 与 Zoom](figure/scalable-zoom.md)
- [Connection、Anchor 与 Router](figure/connection-routing.md)
- [Reusable Shape 与 Border](figure/reusable-shape-border.md)
- [Text、Label 与 TitleBarBorder](figure/text-label.md)
- [Animation、LayoutAnimator 与 RoutingAnimator](figure/animation.md)

## Rendering

- [Graphics API](rendering/graphics-api.md)
- [Clip](rendering/clip.md)
- [UpdateManager 与渲染管线](rendering/update-manager.md)

源码基线与纠错记录见
[`../../verification/reference/draw2d-source-audit.md`](../../verification/reference/draw2d-source-audit.md)。
