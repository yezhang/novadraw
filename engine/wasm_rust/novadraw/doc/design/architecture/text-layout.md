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
→ TextLayoutEngine（默认 Parley）
→ immutable TextLayout
→ measurement / truncation / Novadraw glyph IR
→ RenderBackend（默认 Vello）
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

文本布局和字形绘制是两个独立替换边界：

```rust
pub trait TextLayoutEngine {
    fn layout(
        &mut self,
        request: &TextLayoutRequest,
    ) -> Result<TextLayout, TextError>;
}
```

默认 `ParleyTextEngine` 负责 font fallback、shaping、bidi、line breaking 和 glyph
positioning。它实现上述 Novadraw trait，但 Parley 类型不得出现在 trait、
`TextLayout` 或 RenderCommand 的公开字段中。

文本引擎是显式拥有的可变服务：

```rust
pub struct ParleyTextEngine {
    font_context: parley::FontContext,
    layout_context: parley::LayoutContext<TextBrush>,
    revision: u64,
}
```

- Runtime 独占 `Box<dyn TextLayoutEngine>`；构建器只移交服务/配置，FigureTree 不拥有第二实例；
- 测试可创建隔离实例；
- 不使用 `static`、thread-local singleton 或隐藏全局 cache；
- 注册字体时递增 revision，并使依赖该字体的布局失效。

替换 Parley 只需实现 `TextLayoutEngine` 并产出相同的 Novadraw `TextLayout`。替换
Vello 只需让新 RenderBackend 消费相同的 glyph command；两者互不依赖。

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

Novadraw 分发以下 OFL-1.1 内置字体，但 Runtime 启动时不自动注册：

- Inter：默认 UI 与拉丁文本；
- Noto Sans SC：简体中文及 CJK fallback；
- JetBrains Mono：代码和技术标注。

字体文件、许可证和校验值位于 `assets/fonts/`。应用必须通过
`Runtime::register_builtin_font` 显式选择需要的字体；这会同时更新 ResourceRegistry
和 TextLayoutEngine。未注册字体不得通过系统字体隐式成功。Figure 可以继续从父级
继承字体描述，因此通常只需在应用根节点或主题层指定一次。

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

glyph run 使用 Novadraw 自有类型，保存精确 `FontFaceRef`、font size、normalized
coordinates、可选 synthesis 和 glyph id/position。`FontFaceRef` 包含资源身份、
revision 和字体集合 index；字体字节由 RenderSubmission 资源增量传输，不在每条
command 中重复保存。

`TextLayout` 可在引擎内部保存 source text、cluster mapping 和 accessibility 数据，
但这些内容不进入最终绘制 command。

外部 `TextLayoutEngine` 通过 `TextLayoutParts` 和 `TextLayout::from_parts` 构造该快照。
构造器统一校验 FontDescriptor、TextConstraints、总体与逐行指标、glyph run 数值以及
UTF-8 visible range；外部实现不能直接写入 `TextLayout` 私有字段绕过不变量。
`with_visibility` 使用同一校验路径。独立非 Parley 引擎必须产生非空 GlyphRun 并通过
Runtime/NdCanvas 消费，返回 `TextLayout::default()` 不构成替换能力验证。

## 6. Command IR

规范底层文本指令只有一种：

```rust
pub enum RenderCommandKind {
    DrawGlyphRun {
        run: GlyphRun,
        origin: Vec2,
        paint: GlyphPaint,
    },
}
```

其中：

- `GlyphRun`、`PositionedGlyph`、`FontFaceRef` 和 `GlyphPaint` 都由 Novadraw 定义；
- 坐标和字号使用逻辑单位，backend 负责 DPI 映射；
- 一个 command 只引用一个精确 font face；fallback 文本自然展开为多个 run；
- fill 和 stroke 共享 glyph geometry，通过 `GlyphPaint` 区分；
- command 不携带原始 text、font family 或 max width，这些都已在 layout 阶段解析；
- backend 不执行 shaping、line breaking、ellipsis 或字体 fallback。

现有名称按以下方式收口：

- `Text` 底层 variant 删除；背景文字展开为 `FillRect + DrawGlyphRun`；
- `fill_text_layout` 是 NdCanvas 高层 API，接收 `TextLayout` 并降低为 fill glyph runs；
- `stroke_text_layout` 是 NdCanvas 高层 API，接收 `TextLayout` 并降低为 stroke glyph runs；
- raw-string `Text` / `FillText` / `StrokeText` command 与 NdCanvas API 已删除。

不支持 glyph primitive 的 backend 可以在自身内部把 glyph outline 转成 path，但不能
要求上层把所有文本永久降级为 path。

## 7. 测量与绘制同源

相同内容、字体 revision 和约束下的 `TextLayout` 共同用于：

- intrinsic/preferred/minimum size；
- ellipsis 决策；
- Label 中的文本位置；
- TitleBarBorder 的 insets/preferred size；
- `NdCanvas::draw_text_layout`；
- backend-neutral `DrawGlyphRun`。

自然尺寸与受宽度约束的测量可以产生不同快照。换行高度/baseline 在 Layout 阶段
通过 `measure(constraints)` 返回给 parent，arrange 后复用同约束结果绘制；
单行 ellipsis/alignment 才是 paint-only presentation。详见 ADR-014，
外部 TextLayout 构造与非 Parley 非空 IR 已完成 D4.4 验证；受宽度约束测量进入
parent layout、arrange 与同约束 Glyph IR 的端到端契约也已通过外部 Figure/Layout
集成测试。

`NdCanvas::measure_text` 的字符平均宽度实现以及旧 `draw_text`/`fill_text` API
已删除；测量必须通过 Runtime-owned `TextLayoutEngine`。

## 8. LabelFigure

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
- placement 表示文字相对图标的方向：North 文字在上，South 文字在下；
- preferred size 使用完整文本；
- minimum size 使用 `min(full text, ellipsis)`；
- border insets 和 border preferred size 按现有盒模型合并。

截断以 grapheme-safe 的 UTF-8 边界产生最大可放置前缀，再追加 ellipsis；不得切断
UTF-8 code point。具体断点由 Parley layout 结果决定，不使用平均字符宽度。

## 9. TitleBarBorder

TitleBarBorder 仍是不可变、可复用 Border。因为指标依赖 owner 的 resolved font，
Runtime 使用 owner-scoped `BorderSnapshot` 保存测量结果，而不是让 Border 持有
owner、Runtime 或共享派生状态：

```text
FigureNode::BorderSnapshot
├── resolved insets / preferred size
└── 与 Border 组合结构同构的动态子快照
    └── resolved font 对应的 TextLayout
```

`CompoundBorder` 必须递归解析 outer/inner 的 owner-scoped 快照，并用解析后的子指标执行
与 Draw2D 相同的 inset、preferred size 和 paint 组合公式。静态子 Border 继续直接使用
其配置指标；动态子 Border 使用当前 owner 的快照。快照只保存在 `FigureNode`，共享
Border 实例不得缓存任何 owner 派生状态。

其规则为：

- top inset 等于 text height 加 vertical padding；
- preferred width 等于 text width 加 horizontal padding；
- 标题区域 opaque 仅在背景 alpha 为 1 时成立；
- alignment 只影响 paint；
- label/font/padding 变化通过 replacement transaction 触发 revalidate + repaint。

## 10. 更新与缓存

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
| 测量 width constraint（换行影响高度） | 是 | old/new visual |
| 单行 Label 最终 ellipsis width | 否（geometry 已由 layout 提交） | old/new visual |
| alignment | 否 | old/new visual |
| foreground/alpha | 否 | old/new visual |
| font resource transition | 是 | old/new visual |
| TitleBarBorder replacement | 是 | old/new visual |

相同值写入不得产生 validation、damage 或通知。

## 11. 分批顺序

### M10.2a Text Core

- 引入 Parley；
- `TextLayoutEngine`、默认 `ParleyTextEngine`、`FontDescriptor`、`TextLayout`；
- Novadraw 自有 glyph IR 与 `DrawGlyphRun` command；
- `fill_text_layout` / `stroke_text_layout` 高层 API lowering；
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

## 12. 错误模型

- 非法字号或约束：`TextError::InvalidMetric`；
- 字体描述无法解析：`TextError::InvalidFontDescriptor`；
- 显式 FontId 未就绪：确定性 fallback，同时保留依赖；
- shaping 无可用字体：`TextError::NoUsableFont`；
- backend 缺少 FontFaceRef 对应 revision：结构化 missing-resource 错误；
- backend 不支持 stroke glyph：通过 capability 拒绝或内部 outline fallback，禁止
  静默忽略；
- backend 收到空 glyph run：安全 no-op。

不得 panic 或以平均字符宽度伪造成功结果。
