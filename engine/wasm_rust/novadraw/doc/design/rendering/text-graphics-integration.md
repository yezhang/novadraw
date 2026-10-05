# 文字测量、图形绘制与可替换字体轮廓引擎

类型：`normative-design`

规范效力：已接受，依据
[ADR-025](../../adr/adr-025-unified-graphics-and-glyph-preparation.md)。

日期：2026-10-05

范围：P2-G02 统一 Graphics API 与可替换 glyph 准备链路。
实现状态见 [P2 backlog](../../roadmap/p2-delta-backlog.md)，规范接受不代表已经实现。

本专题定义统一公共 API 设计中的文字/图形详细合同；
[公共 API 总设计](../architecture/public-api-experience-proposal.md) 负责跨领域角色、
命名、模块与迁移。两者共用 ADR-025 的边界，本页独占文字方法、资源和 glyph 链路语义，
不把普通 API 调用者带入后端预处理细节。
以下代码用于说明已落地接口的合同形态，不保证片段可脱离上下文直接编译，
也不定义后端 GPU buffer ABI。

## 1. 核心合同

**统一文字与图形的调用上下文、绘制状态和有序命令流；内部把排版、字形轮廓和
栅格化分别组织，并通过同一字体身份与不可变排版结果连接。**

仅隐藏 `glyph_runs` 不能解决测量与绘图分属不同调用体系的问题。
仅将所有字体功能放进 Vello backend，也会让布局依赖 GPU 并封死独立替换。

保留三个真实扩展边界：

1. `TextLayoutEngine`：文本、字体、约束 → 排版结果与交互几何。
2. `GlyphOutlineProvider`：精确字体实例、glyph ID → 字形贝塞尔轮廓。
3. `RenderBackend`：有序绘图命令 → 像素；内部可以选择 Vello 或自研曲线绘制路径。

第二项针对已提出的自研字体需求，是本次新增扩展合同；
不要求现在就实现自研引擎，也不为它建立空 package。

## 2. SWT 与 Vello 实际分工

### SWT：统一 facade，内部委托

本地 SWT Cocoa `GC.java`：

- `textExtent` 4197–4211：检查当前字体，建立 attributed string，调用
  NSLayoutManager 获取 glyph range 和 used rectangle。
- `drawText` 1839–1864：使用 FONT、FOREGROUND、CLIPPING、TRANSFORM 等 GC 状态。
- `doDrawText` 1890–1920：通过同一布局管理器绘制 glyph。
- `stringExtent` 与 `textExtent` 区分 tab/newline 处理，不是用字符个数估算宽度。

SWT 还提供独立 `TextLayout`：`getBounds`、`getLocation`、`getOffset` 与
`draw(GC, ...)`。因此统一 GC 与可复用排版对象可以共存。

Draw2D 又通过 TextUtilities 承载布局阶段字体测量，通过 Graphics 承载绘制；
这说明保留式框架需要区分阶段，但不应让用户为不同阶段配置不同字体系统。
Novadraw 不复制 SWT/Draw2D 的全局服务和平台生命周期假设。

### 锁定依赖，而非独立 checkout 的最新代码

依据本仓库 Cargo.lock 与本机 Cargo registry 源码：

| 依赖 | 版本 | 在这条链路中的职责 |
|---|---|---|
| Parley | 0.7.0 | 文本分段、双向文本、样式、换行、对齐、布局结果 |
| Fontique | 0.7.0 | 字体集合、匹配与 fallback；Novadraw 仍须限定显式注册集合 |
| HarfRust | Parley 使用 0.3.2 | shaping：字符序列到 glyph ID、advance、offset、cluster |
| Swash | 0.2.10 | Parley 在此版本中使用其文本分析/cluster 等功能；不能据依赖名称认定它是该版本主 shaper |
| Skrifa | Parley 使用 0.37.0；Vello 使用 0.44.0 | 字体指标、实例、轮廓、hinting 等；两版本类型不能直接互传 |
| read-fonts / font-types | 对应 Skrifa 分别为 0.35.0/0.10.1、0.41.0/0.12.4 | 字体表与基础值解析 |
| Vello / vello_encoding | 0.10.0 | glyph/path 编码、轮廓缓存、GPU 矢量绘制 |
| Peniko / Kurbo | 0.6.1 / 0.13.1 | 后端 paint/style 数据与路径几何操作；不进入 Novadraw 中立签名 |

Cargo.lock 中另有 HarfRust 0.12.0，不能将它误报为 Parley 0.7 的依赖版本。
不同 Skrifa 版本通过字体字节、face index、规范化坐标和引擎自有值桥接；
本设计不因版本重复而强制升级依赖。

### Vello 字形入口已经属于同一 Scene

`Scene::fill`、`stroke`、`draw_image`、`draw_glyphs` 都向同一 Scene 编码。
`draw_glyphs(&FontData)` 接收已经定位的 glyph，配置字号、变体、transform、
brush、fill/stroke 和 hinting。它不接受任意字符串并返回布局宽高。

`vello_encoding::GlyphCache` 使用 Skrifa 的 `outline_glyphs()` 和
`outline.draw(DrawSettings, OutlinePen)`，将字形轮廓编码为路径。
部分路径直接写入 encoding，并不先建立完整 BezPath。
因此 Vello 的文字和图形在绘制阶段已经汇入相同矢量管线；
Novadraw 的高层 API 没有必要照搬它的低层调用形式。

重要限制：Vello 0.10 的公开 `draw_glyphs` 路径绑定 FontData/Skrifa，
没有从该入口注入任意 Novadraw 轮廓 provider 的参数。
自研轮廓必须走自有 provider → path → Vello fill/stroke，
或进入自研 backend，不能仅安装 provider 后继续调用 `draw_glyphs` 并假定它会生效。

## 3. 统一 API：准备、测量、绘制在同一入口可发现

独立绘图/录制采用一个 `Graphics` facade，组合借用文字服务和 recorder：

```rust,ignore
let mut gc = Graphics::new(&mut text_system, &mut recorder);
gc.set_font(font);

let text = gc.layout_text("Hello 世界", constraints)?;
let size = text.size();

gc.set_fill_paint(background);
gc.fill_rect(Rectangle::new(origin.x(), origin.y(), size.width, size.height))?;
gc.set_fill_paint(foreground);
gc.fill_text(&text, origin)?;
```

这份示意强调：

- 字体配置、文字准备、图形绘制通过同一上下文到达；
- `TextLayout` 是可复用的普通结果，调用者不处理 glyph run；
- 只要能持有排版结果，优先 `layout_text` 后读取指标并绘制，避免先测量再重复排版；
- 提供 `measure_text(text, constraints) -> Result<TextMetrics, TextError>`，
  委托同一排版服务/缓存；它不是另一个平均字符宽度算法，也不产生绘图命令；
- 同一 API 集合提供 `font_metrics() -> Result<FontMetrics, TextError>`，返回当前解析
  字体实例的 ascent/descent/leading 等指标；它不替代含 fallback、多行文本的实际
  `measure_text`。两种查询都可在无 GPU 的准备阶段执行；
- 改变当前 font 只影响之后的 layout/measure，不重新解释已有 TextLayout；
- 完整 Graphics 的当前 font、Paint、StrokeStyle、opacity、transform、clip
  具有一致的作用域规则。字体字节、字体集合与服务缓存不属于 push/pop 的复制对象。

不必把所有操作塞进一个巨大 trait。Graphics 是组合 facade；
排版与后端保持独立替换，默认使用具体内部委托。
`graphics` 模块组织绘图上下文，`text` 模块组织文字描述、结果与专业扩展类型，
模块分工不要求应用手动协调两套服务。

### 3.1 方法集合与可复用上下文

| 方法族 | Graphics | MeasureContext | PaintContext |
|---|---|---|---|
| `font` / `set_font` | 当前请求字体 | 当前测量的局部请求字体 | 不提供改变排版字体的入口 |
| `font_metrics` / `measure_text` / `layout_text` | 同一文字服务 | 同一文字服务 | 不提供 raw-text 排版 |
| `fill_text` / `stroke_text` | 消费完成的 TextLayout | 不提供 | 消费完成的 TextLayout |
| `fill_rect/path` / `stroke_rect/path` / `draw_image` | 有序录制 | 不提供 | 同一有序录制 |
| Paint/Stroke/clip/transform 状态 | 可设置 | 不提供绘制状态修改 | 与 Graphics 相同的状态语义 |
| `paint()` | 短借用为 PaintContext，复用绘制函数 | 不提供 | 已经是该视图 |
| finish、清命令、写 damage、提交/确认帧 | 不提供 | 不提供 | 不提供 |

目标签名骨架：

```rust,ignore
impl Graphics<'_> {
    fn font(&self) -> &FontDescriptor;
    fn set_font(&mut self, font: FontDescriptor);
    fn font_metrics(&mut self) -> Result<FontMetrics, TextError>;
    fn measure_text(
        &mut self, text: &str, constraints: TextConstraints,
    ) -> Result<TextMetrics, TextError>;
    fn layout_text(
        &mut self, text: &str, constraints: TextConstraints,
    ) -> Result<TextLayout, TextError>;
    fn paint(&mut self) -> PaintContext<'_>;

    // 以下绘制方法与 PaintContext 共享内部实现，不复制状态机。
    fn set_fill_paint(&mut self, paint: impl Into<Paint>);
    fn set_stroke_paint(&mut self, paint: impl Into<Paint>);
    fn set_stroke_style(&mut self, stroke: StrokeStyle);
    fn fill_text(&mut self, text: &TextLayout, origin: Point) -> Result<(), GraphicsError>;
    fn stroke_text(&mut self, text: &TextLayout, origin: Point) -> Result<(), GraphicsError>;
    fn fill_rect(&mut self, rect: Rectangle) -> Result<(), GraphicsError>;
    fn stroke_rect(&mut self, rect: Rectangle) -> Result<(), GraphicsError>;
    fn fill_path(&mut self, path: &Path) -> Result<(), GraphicsError>;
    fn stroke_path(&mut self, path: &Path) -> Result<(), GraphicsError>;
}
```

`set_font` 接收受检 FontDescriptor 值，实际字体匹配/可用性在字体查询与 layout 时
返回 TextError。第三方具体字体类型不进入签名。`layout_text` 返回 owned 不可变
TextLayout，不借住 Graphics 的可变引用，因而准备后可继续使用同一 gc 绘制。

`fill_path(&Path)` / `stroke_path(&Path)` 直接消费独立路径值，是可组合路径的首选入口；
不要求辅助绘制函数先清空 Canvas current path。若保留 `begin_path/fill/stroke`
便利族，必须与独立路径同一几何/paint 实现，后者不消费 current path。
一个路径可以参与 fill、stroke 和构造 ClipPath，不能因一次绘制丢失原路径。

### 3.2 测量结果与绘制状态

- `FontMetrics` 描述解析后的一个字体实例，并携带可追溯 face/revision；
  fallback 混排的文本高度以 TextMetrics 为准。
- `TextMetrics` 描述这次约束下的 layout size、baseline、行指标和截断状态；
  它不持有 GPU 资源。`TextLayout::metrics()` 返回同一测量结果，
  `size()` 是其中 Dimension 的便利查询。
- `TextLayout::glyph_runs()` 保留专业只读出口；专业词不改成含糊的 TextBlock。
  字体实例、glyph 和映射必须来自同一 snapshot，外部创建仍走受检构造器。
- fill_text 的 origin 是 layout 原点，baseline 由布局结果提供。当前 transform 和
  clip 只影响绘制，不改 layout size；文本与矩形使用同一逻辑坐标域。
- 当前 font 改变不影响已有 TextLayout；当前 Paint、StrokeStyle、opacity 在调用
  fill/stroke 时冻结到命令中。颜色变化不需要重新 shaping。
- `measure_text` 是同一布局算法的指标投影，可复用缓存；缓存可驱逐，
  不承诺丢弃 TextLayout 后再次 layout 必定零计算。显式持有布局再绘制才保证不重新排版。

### 3.3 保留式 Figure：按阶段投影能力

统一 facade 不意味着 Figure 可以在 paint 回调中改变自己的布局尺寸：

| 阶段 | 借用视图 | 允许行为 |
|---|---|---|
| 独立准备与录制 | Graphics | 准备/测量文字，绘制文字与路径 |
| Figure measure/arrange | MeasureContext | 同名 `layout_text/measure_text`，只读节点/样式，发布受检候选快照 |
| Figure paint | PaintContext | 与 Graphics 同名的 fill/stroke/clip/transform；绘制已准备 TextLayout |

三个视图按能力借用同一组服务和录制机制，不拥有不同的字体解析器或文字样式模型；
MeasureContext 不借用 recorder，PaintContext 不取得可变排版服务。
推荐以具体类型限制 paint 阶段没有 raw-text 排版入口，不使用一个万能上下文在运行时
返回“当前阶段不能测量”的错误；也不要求用户理解一组复杂 phase 泛型。

因此上一提案中直接把 Figure 的 NdCanvas 改成完整 Graphics 尚不够精确：
完整 Graphics 是独立准备/录制 facade，Figure 得到的是受限 PaintContext。
这些名称表达接口角色，实施时可细化借用签名，但必须保留同一 API 集合中的
字体指标、文字测量、文字绘制和图形绘制能力。

Runtime 继续独占挂载场景的文字服务。MeasureContext 由引擎在布局阶段借出；
不能将 `&mut Runtime` 塞进 Figure，也不能为每个 Figure 新建 TextSystem。
独立场景可显式拥有自己的 TextSystem；跨场景不可直接复用带 foreign 字体身份的
TextLayout，需要显式移交资源或重新准备。

MeasureContext 的局部 font 从 Figure 的 resolved style 初始化；局部覆盖只影响
本次测量请求，不修改 FigureStyle 或父级样式，也不会跨 Figure 泄漏。
Figure 通过既有“计算候选、校验、发布”协议保留 TextLayout 和 FigureMeasurement；
失败不发布候选，arrange/paint 使用同版本结果。此设计不以公开 `&mut dyn Figure`
或任意 closure 绕过 Runtime 的提交权威。

### 3.4 录制错误与资源边界

原始 geometry/path、layout 资源身份或版本可以在录制时校验的，返回 GraphicsError，
保留底层结构化原因。一次文字调用涉及多个 run 时必须先预检整体，不能先录入几个
字形再因后续字体失效退出。失败不改变本次调用之前的命令/状态/current path。
受检 Paint/StrokeStyle 的安装不产生额外失败分支。

与具体 backend/device 相关的 capability、精度和上传失败继续在提交阶段拒绝。
绘制函数返回成功只表示录制成功；整帧发布仍需 Runtime 和 RenderBackend 协议闭合。
PaintContext 将错误交回帧录制调用链，禁止由自定义 Figure 吞错后出版残缺帧。

文本命令所引用字体必须由当前 Registry/资源快照保活。独立录制不能只携带
FontFaceRef 就宣称脱离 TextSystem 可重放；需要命令与匹配版本资源共同组成结果。
公共 API 总设计提出的 RecordedDrawing 是该资源闭合的候选表示，
不改变现有 ResourceSync/RenderSubmission 的确认协议。

## 4. 度量与像素不应混成一种 bounds

| 结果 | 来源 | 用途 |
|---|---|---|
| layout size / line advance / baseline | shaping + line layout | 布局、对齐、光标与选择 |
| ink bounds | 实际字形轮廓及实例变换 | 文字可见轮廓、精确视觉范围 |
| paint bounds | ink + stroke/合成效果 + 栅格化覆盖边界 | damage、culling、截图区域 |

空格可以有 advance 而没有 ink；斜体可以越过 advance；组合附加符号可以不推进位置。
不能通过把各 glyph 路径 bounds 相加得到文本宽度，也不能用文本 layout box 裁剪所有墨迹。

默认采用与设备无关的逻辑排版指标。普通 DPI/缩放不重排文本，只改变绘制映射；
字体大小、约束、features、language、variations 或字体 revision 改变才失效相关布局。
若未来引入 device-fitted metrics，必须显式将设备测量模式加入 layout key，
不能在后端私自用 hinted advance 改变既有 glyph 位置。

Skrifa 的 hinted drawing 可能返回 adjusted advance/side bearing；
这是必须定义的策略差异，不是“用同一字体文件就自动完全一致”。
首个自研轮廓切片建议采用 unhinted 曲线，验证指标不变，再单独定义小字号 hinting。

## 5. 自研字体引擎的三种接法

| 自研范围 | 保留能力 | 替换边界 |
|---|---|---|
| 自己从 TTF/OTF 生成贝塞尔轮廓 | Parley/HarfRust 排版、Vello 曲线绘制 | GlyphOutlineProvider |
| 自己将贝塞尔曲线绘制成像素 | 现有排版，Skrifa 或自研轮廓 | RenderBackend 的曲线/glyph 执行部分 |
| 自定义字体格式、字形身份或排版规则 | 按需保留图形 backend | 字体资源解析与 TextLayoutEngine 适配，并配套 outline provider |

仅实现轮廓/栅格化无需重写 ligature、kerning、bidi、换行、光标导航。
但是任意自定义字体格式不能直接交给现有 Parley OpenType 解析路径；
若 glyph ID、advance 或变体解释发生变化，就必须同时适配排版，不能只替换轮廓。

### 轮廓协议骨架

```rust,ignore
trait GlyphOutlineProvider {
    fn outline(
        &mut self,
        font: FontInstanceRef<'_>,
        glyph: GlyphId,
    ) -> Result<GlyphOutline, FontError>;
}
```

这里的类型全部由 Novadraw 定义：

- FontInstanceRef 固定资源 namespace/id/revision、collection face index、
  变体实例及只读字体数据，不用 family 字符串在 backend 再次选字体。
- GlyphId 只能在其 font face 中解释；provider 不能进行 fallback 或重新 shaping。
- GlyphOutline 是受检、可共享的引擎 Path、font-unit ink bounds 与 units-per-em；
  原点为 baseline，Y 向上，未缩放、未 hint，不夹带当前 Canvas transform。
- 支持 Move/Line/Quad/Cubic/Close；TTF 与 CFF 的二次/三次曲线不能被固定折线近似。
- 合成斜体/加粗在实例处理阶段显式应用一次并更新 ink；不能同时由 provider 和 backend
  各应用一次。quad → cubic 可精确转换，是否做该规范化由实际 consumer 决定。
- 空轮廓是合法结果；缺 glyph、坏字体、unsupported format 和空格不使用同一返回值。

一次 glyph 绘制的概念变换为：

```text
字体单位轮廓
→ 字号 / units-per-em 与 Y 翻转
→ 实例合成变换
→ 排版得到的 glyph origin（已含 advance/offset）
→ 文本放置位置
→ Figure/Graphics transform
→ surface DPI
```

绘制 adapter 不再累加一次 advance。渐变仍处于约定的 Graphics 逻辑坐标域，
不能因逐 glyph 平移让每个字重新从渐变起点着色。

### provider、资源与缓存归属

Runtime 组合根拥有文字服务及其 CPU 字体几何服务；默认轮廓适配使用 Skrifa，
自研 provider 在这里替换。它与排版服务共用 ResourceRegistry 中的字体身份和版本，
不各自注册出不同 FontId。

需要自研轮廓的场景在派生准备阶段获取受检几何和 ink，按 glyph/实例缓存，
再将不可变路径数据或已保活的几何资源传给 renderer。RenderSubmission 不携带
`&mut provider` 或任意执行回调；backend 不回调 Runtime 查询字体。
这也使 damage 可以在真实绘制前覆盖自研轮廓。

缓存按职责区分：

- layout：内容、字体集合/face revision、字号、变体/features、语言方向、约束；
- outline：精确 face/revision、glyph ID、变体、provider revision；
- render：outline/font identity，加字号、hinting、设备模式和实际几何处理参数。

paint/clip 改变通常不需要重新 shaping；outline 更换导致 ink 改变须使视觉范围失效。
若度量也变了，则必须使 layout 失效。不能靠替换 renderer 后全局猜测 damage。
默认 Vello 路径按需保留其内部 glyph cache，不强制每次准备都把全文展开为路径。

## 6. 保留 glyph 命令，在执行处选择路径

引擎继续保存“字体实例 + 定位字形 + paint”的文字命令，避免在命令中逐次复制轮廓，
也保留后端的字体缓存与 hinting 机会。用户不需要调用该低层接口。

三条执行路径：

1. 默认：文字命令 → Vello `draw_glyphs` → Vello/Skrifa 轮廓缓存 → Vello GPU。
2. 自研轮廓：文字命令关联准备好的轮廓 → 有序 path fill/stroke → Vello GPU。
3. 自研绘制：文字命令/轮廓 → 自研曲线执行器；由同一个 backend 负责整体场景合成。

第二条路径必须显式选中，否则默认 `draw_glyphs` 仍会读取原字体轮廓。
不同时为一个 glyph 执行两条路径；不由每个 Label 决定后端。
轮廓快照的资源表示与 lowering 接线要在首次 provider 切片中验证后固化，
本设计不提前发明新的公共 IR opcode。

图形与文本共享 paint、opacity、clip、transform、damage 和 Z-order。
若自研栅格器与 Vello 混合执行，backend 必须按原命令顺序提交分段工作并合成，
不能先画全部图形再画全部文字。GPU device/texture 同步属于具体 backend 内部合同，
不能用一个接受 Vello Scene 的“通用字体 trait”伪装后端中立扩展。
第一阶段优先自研轮廓 + Vello path 路径，能以较小边界验证字体曲线能力。

### 彩色字体与绘制质量边界

Vello 0.10 支持部分 COLR/bitmap 字体路径，其 API 明确说明这些 glyph 的 style
会被忽略，brush 作为 foreground。这个第三方事实不等于 Novadraw 已实现相同能力。

纯 GlyphOutlineProvider 首批只承诺可用轮廓的单色字体。没有单色轮廓的 bitmap/COLR
glyph 需要明确能力拒绝，或显式选择具备相应能力的现有路径；
损坏字体等错误不能被自动 fallback 掩盖。
未来真正需要彩色字体时，再定义有序 paint graph/image 合同，不能把所有字体强行看作
单条 Path。

同一轮廓经 Vello glyph 与普通 path 绘制可能因 hinting/抗锯齿策略产生不同像素；
不能未经证据宣称性能或像素完全等价。

## 7. 布局后 glyph 预处理与片段着色器输入

自研字体后端采用以下顺序：

```text
TextLayoutEngine：文本布局
→ 定位 glyph（精确字体实例、glyph ID、逻辑 origin）
→ 按实际 glyph 获取曲线与 ink bounds
→ glyph 预处理缓存查询；未命中则分网格/建立曲线索引等
→ 组合绘制实例与 PreparedGlyph 数据，上传或复用 GPU 资源
→ 片段着色器计算曲线覆盖率
→ 同一 backend 按图文顺序裁剪、混合、合成
```

布局负责选择字形与位置；分网格等算法不进入 TextLayoutEngine。
可预热曲线缓存，但场景使用的 glyph 必须与这次已完成的布局一致，不能为预处理重新
执行 char→glyph 映射。已有缓存命中时跳过轮廓提取/预处理，不重复执行整条链路。

### 几何资产与绘制实例分离

| 数据 | 内容 | 所有者及生命周期 |
|---|---|---|
| Positioned glyph | face/revision、glyph ID、变体、字号、布局 origin/offset | Core 不可变布局结果 |
| GlyphOutline | 字体局部坐标的二次/三次曲线、ink bounds、units-per-em | 中立字体几何服务，可跨位置复用 |
| PreparedGlyph | 曲线系数、网格/分桶、索引、覆盖区域等算法专用数据 | 字体后端 CPU/GPU 缓存 |
| GlyphDrawInstance | PreparedGlyph 引用、glyph→surface 变换、paint 和有序 clip/合成状态 | 当前帧或提交批次 |

接口关系示意，Prepared 类型属于具体字体后端：

```rust,ignore
// 自研后端内部的分层示意，不是 Core 公共 trait 或序列化格式。
let geometry = glyph_cache.prepare(&font_instance, glyph_id, &outline, &options)?;
let instance = GlyphDrawInstance {
    geometry,
    glyph_to_surface,
    paint,
    clip,
};
font_renderer.draw(instance)?;
```

相同字形在多处出现时共享 PreparedGlyph，位置不烘焙进曲线资产。
Shader 输入须同时具备实例的目标坐标变换和曲线数据的坐标定义；
通常可将 fragment 坐标映射回 glyph 局部坐标执行覆盖计算。奇异变换、非有限派生值、
GPU 数值溢出必须按后端失败合同处理，不能用错误逆矩阵继续绘制。

移动、滚动、换行后的位置变化不应使字体局部网格失效。
若预处理确实依赖 ppem、hinting、误差容限、非均匀变换或描边宽度，
这些依赖必须进入 cache key；不得笼统承诺任意缩放均无需重建。
Paint/clip 不进入纯曲线网格 key，除非实际预处理算法消费了这些输入。

### 原始与预处理输入

自研后端可以接收：

1. 中立输入：定位 glyph + 对应字体/轮廓资源，由后端预处理；
2. 已预处理输入：同一后端生产或显式导入的 PreparedGlyph 资产 + 绘制实例。

第二种模式不向 Core TextLayout 或通用 RenderCommand 塞任意 `Vec<u8>`/`Any`。
引擎仍保留字体/glyph 身份与语义，PreparedGlyph 由 backend-local 的类型/handle
关联；切换后端时从中立数据重建，不能把上一后端的 handle 交给新后端解释。

允许离线预处理数据，但导入必须核验算法/格式版本、字体内容身份与 face index、
glyph ID、变体、坐标单位、预处理参数，以及数组长度/索引/数值有效性。
GPU handle 还须匹配 device/session generation；device loss 后保留可复用 CPU 资产，
重建 GPU buffer，不能复用失效的 GPU 地址。

PreparedGlyph 的专用布局不作为引擎公共 ABI，也不要求伪装成 Vello FontData 或 Path。
Core 只要求可解释的资源身份、保活、几何包络、能力和失败结果；
后端可将同一贝塞尔数据组织为任意经验证的分网格/分桶形式。
普通填充覆盖不要求先将曲线三角化，具体顶点覆盖区域与 fragment 算法由后端决定。

### 发布与一致性

预处理不能改变 shaping advance、line break、caret 或 selection。
轮廓变化产生的 ink 与 paint envelope 必须在 Core 稳定发布前确定，
backend 的分网格只改变执行表示；若会改变几何或覆盖范围，必须先声明正确包络。

缺资源、导入失效、预处理失败或 GPU 上传失败时，禁止部分确认当前提交；
保留既有 `(session, frame)` 的发布/重试协议。
后端可以保留成功生成的私有缓存，但缓存命中不代表该帧已被接受。
重复 glyph 可合批，合批不能跨越改变结果的图文 Z-order、clip 或混合边界。

## 8. 实施顺序与验收

2026-10-05 用户已确认采用本方案；P2-G02 已按以下顺序完成。
自研片段着色器的具体算法和 GPU 格式由未来后端实现决定，不将其列为当前必须实现的
引擎功能。当前先交付统一 API、可替换曲线入口与独立后端消费者合同。

1. 固定 Graphics/MeasureContext/PaintContext 的借用签名、状态共享和服务借用；
   通过一个外部 Figure 证明 measure → arrange → paint 不需要取得可变 Runtime。
2. 以默认 Parley + Vello 实现统一准备/测量/绘制入口；测量与绘制复用相同结果。
3. 实现并验证 Skrifa 默认 outline adapter 和独立自定义 provider；
   两者在 Parley 不变的条件下，经 Vello path 实际绘制。
4. 自研像素执行器有真实实现时再接入 backend；不要提前开放裸 GPU callback。
   当前通过独立 consumer 验证“定位 glyph → 曲线预处理 → 实例”的数据通路。

关键可执行证据：

- 同一输入经两种轮廓 provider，layout size、baseline、换行、caret/selection 不漂移；
  轮廓变化后的 ink/damage 则正确变化。
- 持有 TextLayout 后绘制不重复 shaping；仅 paint/transform 改变不重新排版。
- Graphics::paint 与 Figure PaintContext 可以调用同一绘制函数；独立 Path 可复用，
  current path 不被独立路径调用消费；字体局部状态、错误返回和资源保活有验证。
- 空格、连字、组合符、CJK/Arabic fallback、RTL、变体字体、负 bearing；
  二次/三次曲线和复合字形均有样例。
- 原点、DPI、非均匀缩放、渐变、文字描边、clip、图文交错 Z-order 一致。
- 字体替换/移除、stale/foreign layout、provider 错误无部分 publication；
  unsupported glyph 格式不静默丢字。
- 重复字形只生成一次对应缓存项；移动/重排仅更新实例，变体或预处理选项变更正确失效；
  预处理资产导入拒绝不匹配版本、字体与非法索引，device loss 重建 GPU 资源。
- 比较 glyph 原生路径与 outline 路径的命令量、CPU 准备、缓存和像素；
  只有有数据后才做性能取舍。

现有相关 suite：`core.p2-t01-text-flow`、`core.p2-t02-text-interaction`、
`core.p2-g01-graphics`、`ga4.extension-consumers`、`graphics.p2-g01-visual`、
`graphics.p2-g01-web-pixels`。新增 provider 契约应另登记验证入口。
本轮仅验证文档，不把这些未来验收项记录为已通过。

## 9. 源码核验入口

本机依赖根目录：
`/Users/bytedance/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`。

- `vello-0.10.0/src/scene.rs` 316–377、455–457、487–650：
  同 Scene 的 path/glyph 入口、glyph builder 和彩色 glyph 约束。
- `vello_encoding-0.10.0/src/glyph_cache.rs` 160–239：
  cache key、Skrifa outline 与路径编码。
- `skrifa-0.44.0/src/outline/mod.rs` 1–80、145–175：
  OutlinePen 二次/三次曲线、hinted adjusted metrics。
- `parley-0.7.0/src/shape/mod.rs` 286–395：
  HarfRust shaping 与实例、script/language；
  `src/layout/data.rs` 359–427：字体实例与 Skrifa 行指标。
- SWT checkout：
  `/Users/bytedance/Documents/code/GitHub/eclipse.platform.swt/bundles/org.eclipse.swt/Eclipse SWT/cocoa/org/eclipse/swt/graphics/`，
  `GC.java` 和 `TextLayout.java`，语义范围见第 2 节。
- [Draw2D 文本参考](../../reference/draw2d/figure/text-label.md)、
  [文本现行规范](../architecture/text-layout.md)、
  [ADR-014](../../adr/adr-014-extensibility-and-lifecycle-boundaries.md)。
