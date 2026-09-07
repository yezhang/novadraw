# ADR-007: 采用 Parley 统一文本布局

类型：`architecture-decision`

## 状态

已通过

## 背景

M10.2 需要真实字体测量、Label 截断、TitleBarBorder 指标和 Vello 文本绘制。当前
`NdCanvas::measure_text` 仅使用字符数与平均宽度系数估算，`FillText` /
`StrokeText` 也未进入 native Vello backend，无法形成产品文本能力。

Vello 提供 glyph rasterization，但不负责字体 fallback、shaping、bidi 和 line
breaking。测量和绘制若在不同层分别实现，会使 preferred size、ellipsis 和实际像素
结果漂移。

## 决策

接受：

1. 使用 Parley 作为 Novadraw 文本 shaping 与 layout 引擎；
2. Vello backend 只栅格化已经定位的 glyph runs；
3. `TextEngine` 由 Runtime/FigureTree 显式拥有，不使用全局或 thread-local
   singleton；
4. shaping 产出不可变 `TextLayout`，测量、截断和绘制共同消费该快照；
5. M10.2 分为 Text Core、Label、TitleBarBorder 三个原子增量；
6. 现有 CSS-like font 字符串仅作为迁移输入，文本核心使用结构化字体描述；
7. 自定义字体继续通过 FontId/ResourceRegistry 管理生命周期；
8. M10.2 不实现富文本编辑、caret、selection 或 IME。

完整契约见 `doc/design/architecture/text-layout.md`。

## 后果

### 正面

- 测量、换行、截断和绘制共享同一 shaping 结果；
- native/web 可以使用一致的布局协议；
- Parley 与 Vello 的 glyph run 接口直接对接；
- 后续 TextFlow、Button 和 Accessibility 可复用文本快照；
- 字体上下文可隔离测试，不引入全局状态。

### 负面

- 新增 Parley/fontique/skrifa 依赖；
- Figure intrinsic measurement 需要接入显式文本上下文；
- 自定义字体注册需同步 TextEngine 与 ResourceRegistry；
- 系统字体 fallback 的最终字形可能随平台变化，像素级测试需使用注册字体。

## 不采用的方案

### 继续使用平均字符宽度

不采用。比例字体、复杂脚本、fallback、kerning 和 ellipsis 都会产生错误结果。

### 在 Vello backend 中临时 shaping

不采用。layout 层无法获得同一结果，preferred size 与绘制会形成两条真值。

### 使用全局 TextUtilities

不采用。违反无全局状态约束，也无法隔离 Runtime 字体集合与资源生命周期。

### 采用 cosmic-text

不采用。仓库尚未实际接入 cosmic-text；当前 Vello/Xilem 生态已使用 Parley 生成
Vello glyph runs。选择 Parley 可减少适配层，并保留完整 shaping/layout 能力。

## 参考

- `doc/reference/draw2d/figure/text-label.md`
- `doc/design/architecture/text-layout.md`
- `/Users/bytedance/Documents/code/GitHub/xilem/masonry_core/src/core/text.rs`
- `/Users/bytedance/Documents/code/GitHub/vello/examples/scenes/src/simple_text.rs`

## 日期

2026-09-07
