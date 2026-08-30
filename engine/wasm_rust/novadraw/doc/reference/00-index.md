# 外部参考分析

类型：`reference-analysis`

本目录只记录外部项目源码事实和有明确证据的架构归纳，不定义 Novadraw 行为。

- `draw2d/`：Figure、布局、渲染、更新与架构公理
- `gef/`：GEF 控制器、策略与命令体系
- `swt/`：SWT GC 平台实现
- `zed/`：Zed 响应式通知机制

Draw2D/GEF 当前审计基线及纠错记录见
[`../verification/reference/draw2d-source-audit.md`](../verification/reference/draw2d-source-audit.md)。
Novadraw 对这些语义的采用关系见 [`../parity/`](../parity/00-index.md)。
