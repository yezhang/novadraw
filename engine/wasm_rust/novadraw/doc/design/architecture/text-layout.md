# Text Layout、Label 与 TitleBarBorder 契约

类型：`normative-design`

状态：`accepted`

范围：M10.2

Draw2D 源码事实见
[`../../reference/draw2d/figure/text-label.md`](../../reference/draw2d/figure/text-label.md)。

## 1. 目标

M10.2 建立一条跨 native/web 一致的文本主链路：

```text
text + resolved font + constraints
→ Parley shaping/layout
→ immutable TextLayout
→ measurement / truncation / glyph command
→ Vello glyph rasterization
```

并在该主链路上交付：

- 单行和基础多行文本测量；
- `LabelFigure` 的文本、图标、alignment、gap 和 ellipsis；
- `TitleBarBorder` 的指标与绘制；
- 文本变化、字体变化和资源完成后的 validation/damage；
- 后续 `TextFlow` 可复用的 line、baseline 和 glyph run 数据。

## 2. 非目标

M10.2 不包含：

- 富文本编辑器；
- caret、selection、IME composition；
- HTML/CSS 文本完整兼容；
- backend 内部重新 shaping；
- 平台原生 text widget；
- 全局字体或文本 singleton。

## 3. 文本引擎

采用 Parley 负责 font fallback、shaping、bidi、line breaking 和 glyph positioning。
Vello 只消费已定位的 glyph runs。

`TextEngine` 是显式拥有的可变服务：

```rust
pub struct TextEngine {
    font_context: parley::FontContext,
    layout_context: parley::LayoutContext<TextBrush>,
    revision: u64,
}
```

- Runtime/FigureTree 持有实例；
- 测试可创建隔离实例；
- 不使用 `static`、thread-local singleton 或隐藏全局 cache；
- 注册字体时递增 revision，并使依赖该字体的布局失效。

## 4. 字体描述

公开契约使用结构化 `FontDescriptor`，不继续把 CSS-like 字符串作为文本系统真值：

```rust
pub struct FontDescriptor {
    pub family: String,
    pub size: f32,
    pub weight: FontWeight,
    pub style: FontStyle,
}
```

现有 `FigureStyle.font: Option<String>` 在迁移期作为兼容输入解析为
`FontDescriptor`。默认 `12px sans-serif` 保持不变。解析失败返回结构化错误，不静默
退回不同字体。

`FontId` 继续表示应用注册的字体资源。字体描述和资源身份职责不同：

- descriptor 表达 family/size/weight/style；
- FontId 表达字体字节的生命周期；
- fallback 由 TextEngine 的字体集合决定。

## 5. TextLayout

`TextLayout` 是 shaping 完成后的不可变快照，至少包含：

- logical width/height；
- full width 与 visible width；
- ascent、descent、baseline；
- line metrics；
- glyph runs；
- UTF-8 visible range；
- truncated 标志；
- text/font/constraint 对应的 revision key。

glyph run 保存 font bytes、font collection index、font size、normalized coordinates 和
glyph id/position。渲染命令引用该快照或复制其 run，不再把原始字符串交给 Vello
backend 重新排版。

## 6. 测量与绘制同源

同一个 `TextLayout` 同时用于：

- intrinsic/preferred/minimum size；
- ellipsis 决策；
- Label 中的文本位置；
- TitleBarBorder 的 insets/preferred size；
- `NdCanvas::draw_text_layout`；
- Vello glyph command。

`NdCanvas::measure_text` 的字符平均宽度实现退出产品路径。旧
`draw_text`/`fill_text` API 可在迁移期间保留，但 M10.2 Figure 不得依赖它们。

## 7. LabelFigure

LabelFigure 保存产品输入，不保存第二份通用样式：

```text
text
optional ImageId
text placement
label alignment
text alignment
icon alignment
icon-text gap
text layout cache
```

foreground、background、alpha 和 font 继续来自 `ResolvedStyle`。

尺寸规则对齐 Draw2D：

- 文本与图标都存在时才计 gap；
- east/west：宽度相加，高度取最大；
- north/south：高度相加，宽度取最大；
- preferred size 使用完整文本；
- minimum size 使用 `min(full text, ellipsis)`；
- border insets 和 border preferred size 按现有盒模型合并。

截断以 grapheme-safe 的 UTF-8 边界产生最大可放置前缀，再追加 ellipsis；不得切断
UTF-8 code point。具体断点由 Parley layout 结果决定，不使用平均字符宽度。

## 8. TitleBarBorder

TitleBarBorder 仍是不可变、可复用 Border。因为指标依赖 owner 的 resolved font，
Border 协议增加显式 measurement context，而不是让 Border 持有 owner 或 Runtime：

```text
BorderMetricsContext
├── resolved font
└── TextEngine access
```

其规则为：

- top inset 等于 text height 加 vertical padding；
- preferred width 等于 text width 加 horizontal padding；
- 标题区域 opaque 仅在背景 alpha 为 1 时成立；
- alignment 只影响 paint；
- label/font/padding 变化通过 replacement transaction 触发 revalidate + repaint。

## 9. 更新与缓存

缓存 key 至少包含：

- text revision；
- resolved FontDescriptor；
- TextEngine font revision；
- width/height constraint；
- wrapping/truncation mode；
- scale-independent logical metrics。

变化分类：

| 变化 | validation | damage |
|---|---|---|
| text/font/icon/gap/placement | 是 | old/new visual |
| width constraint | 是 | old/new visual |
| alignment | 否 | old/new visual |
| foreground/alpha | 否 | old/new visual |
| font resource transition | 是 | old/new visual |
| TitleBarBorder replacement | 是 | old/new visual |

相同值写入不得产生 validation、damage 或通知。

## 10. 分批顺序

### M10.2a Text Core

- 引入 Parley；
- `TextEngine`、`FontDescriptor`、`TextLayout`；
- glyph run render command；
- Vello backend glyph rasterization；
- measurement/render snapshot 测试。

### M10.2b Label

- `LabelFigure`；
- Runtime typed mutation；
- preferred/minimum size、alignment、placement、ellipsis；
- `text-app` 自动场景。

### M10.2c TitleBarBorder

- context-aware Border metrics；
- `TitleBarBorder`；
- replacement、style inheritance、layout cache 测试；
- `border-app` 标题场景。

每批独立提交。M10.2a 不以存在文本命令作为完成依据，必须证明实际 glyph command
进入 Vello scene。

## 11. 错误模型

- 非法字号或约束：`TextError::InvalidMetric`；
- 字体描述无法解析：`TextError::InvalidFontDescriptor`；
- 显式 FontId 未就绪：确定性 fallback，同时保留依赖；
- shaping 无可用字体：`TextError::NoUsableFont`；
- backend 收到空 glyph run：安全 no-op。

不得 panic 或以平均字符宽度伪造成功结果。
