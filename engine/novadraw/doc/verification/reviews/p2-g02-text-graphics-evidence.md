# P2-G02 统一文字与 Graphics 实现证据

类型：`verification-record`

日期：2026-10-05

规范：[ADR-025](../../adr/adr-025-unified-graphics-and-glyph-preparation.md)、
[文字与图形整合](../../design/rendering/text-graphics-integration.md)。

## 实现边界

- `Graphics` 在同一入口提供当前字体、字体指标、文字测量/布局以及图形、图片和文字绘制；
  `MeasureContext` 与 `PaintContext` 分别限制布局和绘制阶段能力。
- `CommandRecorder::finish` 产出同时持有命令和精确资源 revision 的
  `RecordedDrawing`；录制失败、状态栈失衡、foreign/stale 字体禁止出版。
- `FigurePreparation` 以 Runtime-owned `MeasureContext` 生成不可变
  `FigurePresentation`，布局指标、视觉包络和 `FigureDrawing` 作为一个候选原子发布；
  paint 阶段只得到 `PaintContext`。
- `GlyphOutlineProvider` 输出未 hint 的字体单位曲线。`OutlineCache` 按
  provider/face revision、glyph 和 variation 缓存曲线，`GlyphInstance` 独立保存
  glyph 到 layout 的变换。
- `OutlineTextEngine` 是显式选择的组合适配器；默认 Parley/Vello glyph 路径不变。
  选中适配器后文字降低为普通 fill/stroke path，Vello 继续按原图文命令顺序编码。
- backend-local 网格、曲线索引、GPU handle 与未来片段着色器算法不进入 Core API。

## 兼容与迁移

内置 Label、TextFlow 与 TitleBarBorder 已显式使用 fill paint 绘制文字，修正旧
`draw_text_layout` 以 stroke paint 执行 fill 的命名歧义。旧方法保留为 deprecated
兼容入口；仓库调用方已迁移到 `fill_text_layout` 或新 Graphics API。

`render_recursive.rs` 只将 Figure 回调适配为受限 PaintContext，未改变树遍历、
坐标传播、children/border 顺序或递归深度合同。

## 验证矩阵

| 入口 | 覆盖 | 结果 |
|---|---|---|
| `core.p2-g02-text-graphics` | 统一测量/绘制、资源保活、错误原子性、外部 Figure、轮廓缓存/实例、fallback/RTL/variation、Vello path 编码 | 通过 |
| `core.p2-t01-text-flow` | paragraph、soft wrap、bidi、截断与 Runtime 刷新 | 3 项通过 |
| `core.p2-t02-text-interaction` | caret/selection、viewport、旧 revision 拒绝与外部 interaction provider | 13 项通过 |
| 定向兼容测试 | Label/TitleBar、约束测量、Graphics、外部 text engine | 40 项通过 |
| `graphics.p2-g01-visual` / `graphics.p2-g01-web-pixels` | 共用图文场景的 Native/Web DPI 1/2 像素断言 | 通过 |
| `cargo xtask check --quick` | workspace 编译、公开 API/第三方类型边界、Native/Web backend | 通过 |
| `cargo xtask check --full` | workspace Clippy、全量单元/集成测试与 doctest | 通过 |

本文不将尚未实现的自研 shader 或 backend 私有预处理格式记为已交付。
