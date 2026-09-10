# ADR-007: 可替换文本布局与后端无关 Glyph IR

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

同时，Parley 和 Vello 都只是当前选定实现。未来可能替换文本布局引擎、渲染后端，
或使用自研 glyph renderer。因此公开 Command 不能携带 Parley/Vello 类型，也不能
要求每个 RenderBackend 根据原始字符串重新完成 shaping。

## 决策

接受：

1. 文本系统分为两个独立扩展点：
   - `TextLayoutEngine`：font fallback、shaping、bidi、line breaking 和 measurement；
   - `RenderBackend`：消费稳定绘制 IR 并完成 glyph rasterization。
2. Novadraw 提供 `ParleyTextEngine` 作为默认实现，但 `TextLayoutEngine` 公共契约不得
   暴露 Parley 类型。
3. Vello 是默认 RenderBackend，但 Command 和文本布局结果不得暴露 Vello 类型。
4. `TextLayoutEngine` 由 Runtime/FigureTree 显式拥有，不使用全局或 thread-local
   singleton。
5. shaping 产出不可变 `TextLayout`；测量、截断和绘制共同消费该快照。
6. Command 使用 Novadraw 自有的 backend-neutral glyph IR：

   ```text
   DrawGlyphRun
   ├── FontFaceRef(resource id + revision + collection index)
   ├── logical font size
   ├── variation coordinates
   ├── optional synthesis
   ├── logical positioned glyphs
   └── GlyphPaint(Fill | Stroke)
   ```

7. glyph id 只在其精确 `FontFaceRef` 下有意义；字体字节通过
   ResourceRegistry/RenderSubmission 传给 backend，不嵌入每条命令。
8. Command 中的坐标和字号保持逻辑单位，DPI 转换由 backend 执行。
9. `Text` 不作为规范底层 command；背景绘制规范化为 `FillRect + DrawGlyphRun`。
10. `fill_text` / `stroke_text` 可保留为 NdCanvas 高层 API，但必须接收已完成布局的
    `TextLayout`，并分别降低为 `DrawGlyphRun { paint: Fill | Stroke }`。不得把原始
    字符串和 `max_width` 直接提交给 backend。
11. M10.2 分为 Text Core、Label、TitleBarBorder 三个原子增量。
12. 现有 CSS-like font 字符串仅作为迁移输入，文本核心使用结构化字体描述。
13. 自定义字体继续通过 FontId/ResourceRegistry 管理生命周期。
14. M10.2 不实现富文本编辑、caret、selection 或 IME。
15. Runtime 启动时不自动注册字体；内置字体只是可选资源，应用必须显式注册并选择。
    Figure 仍可继承祖先的字体描述，因而不要求每个 Figure 重复指定字体。

完整契约见 `doc/design/architecture/text-layout.md`。

## 迁移要求

ADR 修订时的 M10.2a 原型仍有两处不符合最终边界：

- `RenderCommandKind::GlyphRun` 直接携带 `parley::FontData`；
- raw-string `Text`、`FillText`、`StrokeText` 与 glyph command 并存。

后续实现必须先完成以下收口，再进入 Label：

1. 定义 Novadraw 自有 `FontFaceRef`、`GlyphRun`、`PositionedGlyph` 和 `GlyphPaint`；
2. 将 `GlyphRun` command 重命名并稳定为 `DrawGlyphRun`；
3. 将 `fill_text` / `stroke_text` 改为接收 `TextLayout` 的 lowering API；
4. 删除或 deprecated raw-string command 路径；
5. 增加一个非 Parley mock `TextLayoutEngine` 和一个非 Vello recording backend 契约
   测试，证明两侧可独立替换。

## 后果

### 正面

- 测量、换行、截断和绘制共享同一 shaping 结果；
- native/web 和自定义 backend 消费一致的 glyph IR；
- 可以独立替换 Parley 或 Vello，不要求另一侧同步重写；
- 后续 TextFlow、Button 和 Accessibility 可复用文本快照；
- 字体上下文可隔离测试，不引入全局状态。

### 负面

- 新增 Parley/fontique/skrifa 依赖；
- 需要维护 Novadraw glyph IR 到各 backend 的适配层；
- Figure intrinsic measurement 需要接入显式文本上下文；
- 自定义字体注册需同步 TextEngine 与 ResourceRegistry；
- 系统字体 fallback 的最终字形可能随平台变化，像素级测试需使用注册字体。

## 不采用的方案

### 继续使用平均字符宽度

不采用。比例字体、复杂脚本、fallback、kerning 和 ellipsis 都会产生错误结果。

### 在 Vello backend 中临时 shaping

不采用。layout 层无法获得同一结果，preferred size 与绘制会形成两条真值。

### 在 Command 中暴露 Parley FontData 或 Layout

不采用。这会把文本引擎实现固化进跨 backend 协议，使自定义 shaping 引擎必须伪装成
Parley，也让 Command 的兼容性受第三方类型变化影响。

### 把原始字符串作为规范 RenderCommand

不采用。每个 backend 会重复实现 shaping、fallback、换行和截断，无法保证测量与
绘制一致。原始字符串属于场景语义与 accessibility 数据，不属于最终绘制 IR。

### 把字形转换成 Path

不作为默认路径。它虽然最通用，但会丢失字体资源复用、glyph cache、hinting 和彩色
字体能力；仅允许不支持 glyph primitive 的 backend 将其作为内部 fallback。

### 使用全局 TextUtilities

不采用。违反无全局状态约束，也无法隔离 Runtime 字体集合与资源生命周期。

### 默认采用 cosmic-text

不采用。仓库尚未实际接入 cosmic-text；当前 Vello/Xilem 生态已使用 Parley。
但 Parley 只属于默认 adapter，不进入 Command 或公共布局 IR。

## 参考

- `doc/reference/draw2d/figure/text-label.md`
- `doc/design/architecture/text-layout.md`
- `/Users/bytedance/Documents/code/GitHub/xilem/masonry_core/src/core/text.rs`
- `/Users/bytedance/Documents/code/GitHub/vello/examples/scenes/src/simple_text.rs`

## 日期

2026-09-07
