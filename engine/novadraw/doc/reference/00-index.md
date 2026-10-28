# 外部参考分析

类型：`documentation-index`

本目录只记录外部项目源码事实和有明确证据的架构归纳，不定义 Novadraw 行为。

- [`draw2d/`](draw2d/00-index.md)：Figure、布局、渲染、更新与架构公理
- [`gef/`](gef/00-index.md)：GEF 控制器、策略与命令体系
- [`graphite/`](graphite/00-index.md)：基于 Vello GPU 路径的开源 2D 图形工具参考
- [`xi-editor/`](xi-editor/00-index.md)：Rope Science 与文本编辑器内部机制参考
- [Lapce 与 Xi 文本机制](lapce-text-engine.md)：固定源码版本的 delta、视觉行缓存、
  可见绘制与安全增量边界
- [SWT GC macOS](swt/gc-macos.md)：平台绘图实现
- [Zed 响应式机制](zed/reactive-design.md)：通知与订阅生命周期
- [文本渲染管线与自研替换范围](fonts/text-rendering-pipeline.md)：当前项目从字体、
  shaping、排版到 GPU 输出的完整链路、直接／间接依赖职责与替换分析
- [字形轮廓存储与填充规范](fonts/glyph-outline-storage-conventions.md)：TrueType 与
  CFF/CFF2 的方向、闭合、控制点、复合字形和填充规则
- [字形渲染与彩色 Emoji](fonts/glyph-rendering-and-color-emoji.md)：三种轮廓算法、
  局限性、主流彩色字体格式及后端选型依据
- [字体渲染比较报告](fonts/reports/00-index.md)：保留原始 GPU demos 的独立报告
- [技术中文写作参考](writing/00-index.md)：受控技术中文规范讨论稿

Draw2D/GEF 当前审计基线及纠错记录见
[`../verification/reference/draw2d-source-audit.md`](../verification/reference/draw2d-source-audit.md)。
Novadraw 对这些语义的采用关系见 [`../parity/`](../parity/00-index.md)。
