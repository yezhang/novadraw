# 公共 API 统一设计与迁移提案

类型：`proposal`

状态：P2-G02 与已批准跨领域迁移已完成；后续定向项保留

日期：2026-10-05

设计盘点基线：`c96b675`。P2-G02 的实现状态以路线图和验证记录为准，
且不改变 P2-G01 验收状态。已接受的部分以所引 ADR 为准，其余目标接口为综合设计建议；
标为 `rust,ignore` 的代码是设计示意，不是已编译示例。

2026-10-05 文字整合修订：统一的测量/绘制 facade 与自研字体轮廓扩展先于名称迁移，
用户已确认采用，具体合同见
[ADR-025](../../adr/adr-025-unified-graphics-and-glyph-preparation.md) 与
[文字与图形整合专题](../rendering/text-graphics-integration.md)。
以下 Graphics/录制分离方案据此细化为按阶段借用的上下文，不再只做 NdCanvas 改名。

2026-10-06 跨领域迁移已按独立切片完成：帧准备状态、角度单位、Dimension/Insets、
ImageRegion、受检图元构造、`*Mut` Runtime 可变借用、Editor/Inspector 领域出口、
Core root 收口以及 Vello 初始化失败传播。提交与验证映射见
[公共 API 统一迁移完成记录](../../verification/reviews/public-api-unification-completion-2026-10-06.md)。
Figure capability 全面改造和 Editor direct-edit session facade 仍为后续定向，不在本轮
完成范围内。

本页是公共 API 的总设计和迁移入口；文字专题是文字/图形合同的唯一详细定义。
阅读顺序：第 2 节定位角色与入口 → 第 4 节理解组合 → 第 5 节检查值与失败 →
第 6 节查看全包处置 → 第 8 节执行迁移。无需先理解 glyph、backend session 或
内部更新队列才能开始使用 API。

## 1. 目标与范围

从调用者的任务出发，使公开 API 可发现、可预测、可组合，并保留真实扩展能力。
`glyph_runs` 是检查入口，但目标不只是把专业词换成短词。

本次综合设计覆盖六个公开包的入口与主要领域契约：Core、Editor、Inspector、
Vello、Winit、Web。第 6 节现状取自此前 `c96b675` 公开面盘点；
本次从已确认的调用需求、ADR 和第三方研究结论推导目标，不新增本项目 Rust 扫描。
它不是所有函数的逐行正确性审计，也不据此宣称 API 已完整验收。

继续遵守：

- [ADR-019](../../adr/adr-019-composable-api-and-scoped-editors.md) 的 detached、
  build、attached、query、drive 边界；
- [ADR-023](../../adr/adr-023-crate-consolidation-and-extension-boundaries.md) 的六包边界；
- [ADR-024](../../adr/adr-024-graphics-paint-stroke-and-clipping.md) 的 Paint、
  StrokeStyle、clip 和能力预检契约；
- [文本规范](text-layout.md) 的测量与绘制同源、Runtime-owned
  （术语解释见[附录 A](#附录-aowned-与-scoped-术语说明)）文本服务和后端不排版；
- [坐标规范](../coordinates/coordinate-system.md) 的显式坐标域与转换方向。

## 2. 调用者与设计准则

| 调用者任务 | 应当接触的概念 | 不应成为前置知识 |
|---|---|---|
| 添加标签、图形、布局 | LabelFigure、FigureId、LayoutManager、scoped mutable facade（见[附录 A](#附录-aowned-与-scoped-术语说明)） | GlyphRun、资源增量、backend session |
| 编写自定义 Figure | MeasureContext/PaintContext、Paint、StrokeStyle、TextLayout、测量与命中 | RenderSubmission 构造、damage 队列修改 |
| 替换排版或布局算法 | TextLayoutEngine、受检排版结果；LayoutSnapshot/Output | Vello、DOM、全局服务 |
| 接入后端和平台 | RenderBackend、RenderSubmission、PlatformHost | 应用模型和 Editor Command |
| 构建图形编辑器 | ModelAdapter、Command、EditPart、Viewer、Tool/Policy | 通过原始 Figure mutation 替代模型命令 |

准则：

1. **名称预测行为**：`set_*` 修改，名词查询，`with_*` 配置 owned value；
   `into_*` 消耗所有权，`take_*` 取走待处理数据。返回 Result 不要求一律加 `try_`。
2. **同类操作同一语言**：填充为 `fill_*`，描边为 `stroke_*`；
   `draw_*` 留给图像等不属于二者的绘制。不把 fill/stroke 与 foreground/background 混用。
3. **参数表达领域**：矩形用 Rectangle，位置用 Point，位移用 Vec2，尺寸用 Dimension，
   inset 用 Insets；度/弧度、源图像像素/目标逻辑坐标必须可辨。
4. **专业词放在专业任务里**：GlyphRun、EditPart、baseline 等有精确含义，
   不为了通俗改成含糊的 TextBlock、Node 或 offset。
5. **值可组合，状态有权威**：共享 Paint/StrokeStyle、独立 Layout/Border/Router；
   attached mutation 经 Runtime。组合不等于多个操作自动具有回滚原子性。
6. **错误不能消失**：非法输入在构造/提交边界拒绝；Option 表示合法缺失；
   `Ok(false)` 表示幂等不变。保留领域错误，避免全系统统一成字符串错误。
7. **控制迁移成本**：只在减少歧义或维护负担时改名；不为每个已有类型创建 facade，
   不新增空 crate，不保留长期同义别名。

### 2.1 统一的对象角色

| 角色 | 代表类型 | 调用规则 |
|---|---|---|
| 可组合值 | Paint、StrokeStyle、Path、FontDescriptor、布局/Border/Router 配置 | owned 构造和配置，进入绘制/运行期时冻结所消费的值 |
| 身份 | FigureId、ImageId、FontId、EditPartId | 不携带可变场景权限，不把不同身份折叠成 NodeId |
| 服务 | Runtime、TextSystem、TextLayoutEngine、RenderBackend | 明确拥有资源或算法；仅在真实替换边界使用 trait |
| 借用上下文 | Graphics、MeasureContext、PaintContext、scoped mutable facade | 不拥有第二份状态；取得上下文不等于已经提交操作 |
| 不可变结果 | TextLayout、TextMetrics、LayoutSnapshot、FigureTreeSnapshot | 可查询、可共享；不通过 getter 隐式推进 Runtime |
| 录制/提交结果 | CommandRecorder、RecordedDrawing（候选）、RenderSubmission | 区分命令记录、资源保活和后端接受，不能互相冒充 |

同名 `with_*` 表示配置 owned value，不承诺所有对象的构造形式相同。
`LabelFigure::new(text)` 与几何图形 `new(bounds)` 都保留领域含义。
`Clone` 表示共享/复制值，不授予 attached mutation 或跨 namespace 迁移权限。

### 2.2 模块与入口

| 路径 | 推荐内容 | 不在入门导出中平铺 |
|---|---|---|
| `novadraw` / `prelude` | Runtime、FigureId、基础几何、常用 Figure/布局；Graphics 作为常用绘图入口 | registry、IR、route 内部查询、provider、prepared backend 数据 |
| `graphics` | Graphics、PaintContext、Paint、StrokeStyle、Path、clip 与绘制错误 | GPU 类型与 session 管理 |
| `text` | FontDescriptor、FontMetrics、TextMetrics、TextLayout、TextConstraints | 算法缓存和 GPU buffer |
| `text::shaping` / `text::outline` | TextLayoutEngine、受检布局产出；GlyphOutlineProvider 与曲线值 | 应用模型、Vello/Skrifa 类型 |
| `figure` / `layout` | Figure、MeasureContext、capability、LayoutManager、Snapshot/Output | 后端驱动 |
| `runtime` / `tree` | 生命周期、scoped mutable facade、查询、Builder、明确的帧准备 | 任意 `&mut FigureTree` 出口 |
| `render` / `host` | RenderBackend、CommandRecorder、RenderSubmission、资源与平台合同 | 应用入门便利函数 |
| 独立 Editor/Inspector 包 | 模型编辑与只读诊断各自的领域入口 | Core 反向依赖和聚合重导出 |
| 独立 Vello/Winit/Web 包 | 具体构造、平台差异、backend-local PreparedGlyph | 把具体实现类型传进 Core |

该表定义目标分层，不表示所有路径已经存在。
canonical 定义路径只有一个；root/prelude 对高频类型的精选重导出不构成第二套行为 API。
首批 prelude 不同时加入测量、轮廓、后端三个专业协议集合。方法文档按
“准备/测量、绘制、状态、专业集成”分组，使专业方法可发现而不占据入门路径。

## 3. glyph_runs 的裁决

现状：`TextLayout::glyph_runs() -> &[GlyphRun]` 位于
[`render/text.rs`](../../../novadraw/src/render/text.rs)。它是完成 shaping 后的定位字形组，
每组对应精确字体实例和字号等绘制数据；它不是字符数组、文本行，也不是业务段落。
字体 fallback、连字和双向文本使这几个概念无法一一对应。

推荐保留 **GlyphRun 和 glyph_runs 的专业名称**，同时改变普通任务的入口：

- 添加文本：直接用 LabelFigure/TextFlowFigure，修改用 `runtime.label(id)?.set_text(...)`。
- 自定义绘制：使用 `TextLayout` 与 `Graphics::fill_text(&layout, origin)` /
  `stroke_text`，不手动遍历 glyph。
- 自定义排版与后端：保留 `glyph_runs`、PositionedGlyph、FontFaceRef 与受检
  TextLayoutParts。rustdoc 将其列在“排版/渲染集成”方法组，说明一组字形不等于一行。
- 增加 `novadraw::text` 领域入口，承载 FontDescriptor、TextConstraints、TextLayout、
  文本位置和交互查询；排版产出类型归入 `text::shaping`。
  Render IR 引用相同类型，不复制数据结构。
- `TextLayout::glyph_runs()` 无需再套一个没有状态和行为意义的
  `render_data().glyph_runs()` wrapper；它不进入入门示例，也不作为应用绘制路径。

统一 Graphics facade 还应提供 `layout_text/measure_text`，委托同一文字服务，
使准备、测量与图形绘制在同一入口可发现。绘制方法消费完成排版的 TextLayout，
Figure 的 paint 阶段不重新 shaping。现有 `Runtime::layout_text` 是当前入口，
不是目标设计必须固守的调用形式；服务所有权、缓存、revision 和字体失效仍由 Runtime
管理。自研字形曲线通过独立 GlyphOutlineProvider 接入，不与 TextLayoutEngine 捆绑。

## 4. Graphics：统一语言并分开录制管理

### 4.1 现状证据与迁移映射

证据入口：[`render/context.rs`](../../../novadraw/src/render/context.rs)。

| 当前公开 API | 目标 | 分类与原因 |
|---|---|---|
| `NdCanvas` | Graphics facade + Figure 的 PaintContext | 统一文字准备/测量/绘制，按阶段限制可借用能力 |
| `line_width` / `set_line_width` | `set_stroke_width` | 同一 setter；与 StrokeStyle/PointListMut 对齐 |
| `line_cap` / `line_join` | `set_line_cap` / `set_line_join` | 明确写操作；保留 LineCap/LineJoin 专业词 |
| `line_style` / `set_line_style` | `set_dash_pattern(DashPattern)` | 消除 LineStyle 与 DashPattern 的并行表达 |
| `set_stroke` | `set_stroke_style(StrokeStyle)` | 明确完整描边几何配置，与 stroke paint 区分 |
| `fill_style` / `set_background_color` | `set_fill_paint` | Color 转 Paint，不为纯色重复一套 setter |
| `stroke_style` / `set_foreground_color` | `set_stroke_paint` | 避免 style 有时是颜色、有时是完整描边 |
| `global_alpha` / `set_alpha` | `set_opacity` | 范围 0..=1，非有限/越界拒绝，失败不改状态 |
| `fill_rectangle` / `draw_rectangle` | `fill_rect(Rectangle)` / `stroke_rect(Rectangle)` | 动作与几何参数明确 |
| `fill_oval` / `draw_oval` | `fill_ellipse(Rectangle)` / `stroke_ellipse(Rectangle)` | 与 EllipseFigure 统一 |
| `draw_polygon` | `stroke_polygon(&[Point])` | 不再由 draw 暗示描边 |
| `fill_text_layout` / `stroke_text_layout` | `fill_text(&TextLayout, Point)` / `stroke_text(...)` | 普通绘图无需字形知识 |
| `draw_text_layout` | 逐调用迁移，删除歧义入口 | 当前用 stroke paint **填充**文字，不等同于任一名称的简单替换 |
| `transform(a,b,c,d,e,f)` | `concat_transform(Affine2D)` | 拼接与覆盖明确区分 |
| `set_transform(a,b,c,d,e,f)` | `set_transform(Affine2D)` | 复用几何值，保留组合顺序 |
| `clip_rect` / `clip_path` | 保留交集语义，矩形参数结构化 | clip 与 replace/reset 不合并 |
| `set_clip` | `replace_clip_rect(Rectangle)` | 明确清除当前 Canvas clip 链后设置 |
| `push_state` / `restore_state` / `pop_state` | 保留并配状态栈示例 | restore 不弹栈，不能改成一个 save/restore 对 |
| `commands/damage_mut/clear_commands/to_submission_*` | 移出 Figure 的 Graphics 接口 | 归于命令录制与后端集成 |
| `*_with_color/paint/style/paints` | 低层显式录制接口收敛 | 一个显式入口接收完整 Paint/StrokeStyle，不读隐含 style |

`set_fill_paint` / `set_stroke_paint` 可以接受 `impl Into<Paint>`，
使纯色和渐变共用一条调用路径；不允许转换过程中隐藏失败或资源加载。

### 4.2 Graphics 与 CommandRecorder 的目标关系

新增 `render::CommandRecorder` 作为现有 owned 录制状态的明确角色。
它拥有命令流；完整 Graphics facade 同时借用文字服务与 recorder，
不复制命令流、不实现第二套 Graphics 状态机。MeasureContext 只借用测量所需服务，
PaintContext 只提供已准备数据的绘制，完整方法集合及失败阶段见
[文字专题 §3](../rendering/text-graphics-integration.md#3-统一-api准备测量绘制在同一入口可发现)。

Figure paint callback 只取得 PaintContext；不能清空前面 Figure 的命令、修改全局 damage、
伪造 session/frame 或构造 Runtime submission。显式 command 录制保留给后端测试和
自定义离线录制。Runtime 帧出版仍由 `prepare_submission` 负责。

组合函数接收自己真正需要的窄上下文，既可从完整 Graphics 调用，也可从 Figure
paint 调用。Graphics 提供 `paint()` 的短借用以复用绘制函数；两者使用相同状态和
recording primitives，不以 `DerefMut<NdCanvas>` 暴露清空命令或修改 damage 的权限。

```rust,ignore
// 目标：一次编写，在独立绘图和 Figure 中复用。
fn paint_caption(
    gc: &mut PaintContext<'_>,
    text: &TextLayout,
    bounds: Rectangle,
    origin: Point,
    background: Paint,
    foreground: Paint,
) -> Result<(), GraphicsError> {
    gc.set_fill_paint(background);
    gc.fill_rect(bounds)?;
    gc.set_fill_paint(foreground);
    gc.fill_text(text, origin)?;
    Ok(())
}

// text_system 是 composition root 显式拥有的同一文字/资源服务。
let mut recorder = CommandRecorder::new();
{
    let mut gc = Graphics::new(&mut text_system, &mut recorder);
    gc.set_font(font);
    let text = gc.layout_text(caption, constraints)?;
    paint_caption(&mut gc.paint(), &text, bounds, origin, background, foreground)?;
}
let drawing = recorder.finish()?; // 候选：保有资源的 RecordedDrawing
```

绘制函数是否保存/恢复样式必须写进函数合同；上述函数明确留下最后设置的 fill paint。
需要隔离样式时，调用者使用已有 push/restore/pop 语义，不伪造可回滚 transaction。

### 4.3 离线录制必须保有资源

此前 `into_commands()` 示例只保存命令，无法独自解释其中的 FontFaceRef/ImageResourceRef。
推荐以 `finish() -> Result<RecordedDrawing, GraphicsError>` 作为独立录制完成边界：

- 结果包含有序命令及所引用资源的不可变 payload/版本保活；不包含可变 TextSystem。
- `commands()` 只供专业只读检查；裸 command slice 不宣称可脱离资源单独重放。
- finish 检查录制状态与依赖闭合，不发 GPU 工作，不分配 Runtime session/frame。
- 当前 Registry 已有更新/删除后，旧录制只可在显式快照重放会话消费；
  不能拿旧 lease 覆盖当前 Runtime 的资源真值。
- 同一录制中不混合无法被当前资源协议表达的同 ID 多 revision；
  录制跨越资源改变时应拒绝完成或重新录制，不静默选一个版本。
- 真正重放仍通过 host/render 的有序资源同步与 backend 接受协议，
  不新增 `drawing.submit()` 绕过 Runtime 确认。

RecordedDrawing 已随独立 Graphics consumer 和资源 revision 契约完成验证。
它只覆盖既有自定义录制用例，不扩张为通用 DisplayList 场景图或序列化协议。
该切片不修改递归遍历、坐标传播和 damage 算法。

### 4.4 文本迁移必须保持像素语义

当前 `draw_text_layout` 消费 `stroke_paint`，生成 `GlyphPaint::Fill`；
`fill_text_layout` 消费 `fill_paint`；`stroke_text_layout` 才生成 glyph stroke。
所以不能全局把 `draw_text_layout` 替换成 `fill_text`。

迁移前：

```rust,ignore
gc.set_foreground_color(text_color);
gc.draw_text_layout(&layout, x, y);
```

迁移后：

```rust,ignore
gc.push_state();
gc.set_fill_paint(text_color);
let result = gc.fill_text(&layout, Point::new(x, y));
gc.pop_state();
result?;
```

示例中保存/恢复用于避免改变后续填充画刷；原调用产生的其他持久状态影响也须逐点保留。
目标 origin 延续既有 layout 原点语义，不能在改名时偷偷变成 baseline。
Label/TitleBarBorder/TextFlow、自定义 Figure 都需覆盖渐变、alpha 和 sibling 隔离。

Draw2D `Graphics.drawString/drawText` 使用 foreground，且二者区分 tab/newline 处理；
Novadraw 已把这些处理归于排版，因此无需复制这组字符串 API。

### 4.5 自研字体后端的公共边界

应用只看到 TextLayout 与文字绘制；专业后端依次消费：

| 阶段 | 输入与结果 | 可替换范围 |
|---|---|---|
| layout | 文本/字体/约束 → 定位 glyph 与 TextMetrics | TextLayoutEngine |
| outline | 精确字体实例/glyph ID → 未缩放的 GlyphOutline | GlyphOutlineProvider |
| prepare | 曲线 → 后端 PreparedGlyph | 后端算法、网格/曲线索引和资源缓存 |
| draw | PreparedGlyph + 实例变换/paint/clip → pixels | 片段着色器及后端有序合成 |

PreparedGlyph 与 GlyphDrawInstance 不进入 Core prelude，不成为布局结果的泛型参数。
应用不提交无类型 blob；后端提供自己的受检导入方法和 device-local handle。
Core 专业消费者仍能取得原始曲线，直接使用片段着色器时不需要再降回 Vello Path。
Vello glyph fast path 与自研曲线路径由组合根选择，不能由 Label 隐式分支。
数据字段、cache key、坐标和导入校验以文字专题为唯一详细来源。

## 5. 几何与组合值

| 领域现状 | 目标决策 |
|---|---|
| Graphics `rotate` 接受度；Affine2D `from_rotation/then_rotate` 接受弧度 | Graphics 改 `rotate_degrees`；Affine2D 改 `from_rotation_radians/then_rotate_radians/then_rotate_about_radians`，数值语义不变 |
| Path arc 接受角度与方向标量 | 在迁移清单逐个标明当前角度单位；保留现有 degree 入口的数值含义，以 `*_degrees` 明示，不能只把参数名改为 radians；SVG endpoint arc 单独保留旗标语义 |
| FigureMut/Builder/EventContext 尺寸 override 仍使用 tuple | 统一 Dimension；Figure intrinsic size 与 Insets 的 tuple 同批形成逐项映射 |
| Canvas transform 接受六个 f64 | 使用 Affine2D；`concat`、`set` 保持不同含义，矩阵乘法顺序不改 |
| 图像 source/dest 都是 Rectangle，但前者物理像素、后者逻辑坐标 | `ImageRegion` 受检值组合 image + source_pixels；绘制接收 destination；可先文档和参数名明确，再迁移类型 |
| Triangle `with_stroke_width` 直接存 f64，paint 时才遇到 fallible setter | detached 构造就校验，复用 StrokeStyle；不把错误延迟到渲染时静默漏画 |
| 多参数 `StrokeStyle::try_new` | 保留完整构造，推荐 `default().with_width(...)?.with_cap(...).with_dash_pattern(...)` 逐属性组合 |

采用已有领域类型优先；本轮不引入全系统 `Point<Space>` 泛型、通用 Angle 框架或
Geometry trait 层。新增受检值只用于确实易混淆且有消费者的边界。

`ImageRegion` 是候选组合值，不是新增资源 registry。必须保留源像素范围校验、零面积
no-op、资源 revision 和 scale 语义；它的批准不代表图像绘制能力变更。

### 5.1 命名词汇表

| 形式 | 含义 | 例子 |
|---|---|---|
| 名词 / `is_*` / `has_*` | 读取已有结果或状态 | `bounds()`、`is_visible()`、`has_pending_update()` |
| `set_*` / `clear_*` | 替换配置 / 移除 override | `set_stroke_style`、`clear_preferred_size` |
| `with_*` | 返回配置后的 owned value，可返回 Result | `stroke.with_width(width)?` |
| `add/insert/remove` | 明确集合与位置的结构操作 | `container.insert(child, index)` |
| `layout/measure/prepare` | 执行服务计算，可能更新私有缓存 | `layout_text`、`prepare_submission` |
| `fill/stroke/draw` | 追加有序绘图命令；不代表 GPU 已完成 | `fill_text`、`stroke_path`、`draw_image` |
| `take_*` / `into_*` | 取走待处理数据 / 消耗 owned value | `take_events` |
| `*_later` | 排队，尚未提交 | `update_component_later` |

`replace_*` 用于需要强调替换一个集合/关系整体的操作，不与相同行为的 `set_*`
双入口长期共存。`try_*` 仅在确有另一条无失败合同且两者都必要时使用；
Result 本身已经表达失败。公开字段只用于简单值或不可变结果，受检不变量使用私有字段。

### 5.2 失败与状态变化

| 边界 | 返回方式 | 失败后状态 |
|---|---|---|
| 受检值构造 | `Result<Value, DomainError>` | 尚未产生有效值 |
| Graphics 的原始几何/文字录制 | `Result<(), GraphicsError>` | 当前调用不追加半个命令序列，不改变 current path/状态 |
| 已验证 Paint/StrokeStyle 安装 | `()` | 无须再包装无意义 Result |
| 文字准备/测量服务 | `Result<LayoutOrMetrics, TextError>` | 可保留合法私有缓存，不追加绘制命令 |
| attached mutation | `Result<bool, MutationError>` | false 仅表示合法幂等；错误不提交源操作 |
| 只读可选查询 | `Option<T>` 或具名查询错误 | 无目标是合法缺失，非法 namespace 不伪装合法结果 |
| 延后操作 | 入队；提交错误从既有 deferred 结果出口观察 | 不承诺调用时已经完成 |
| Runtime/backend 驱动 | 完整 FramePreparation/RenderOutcome | 保留 Idle/Suspended/等待/错误，不能折叠成 None |

GraphicsError 是绘图领域内的结构化错误入口，应保留几何/文字/资源原因，
不吞掉下层错误或变成全系统通用 String。绘制时可检查的有限值、资源身份与版本
在录制边界拒绝；需要具体 backend/device 的 capability、GPU 精度与上传错误在提交边界拒绝。
paint 回调传播错误后整帧不发布；先前成功的局部命令不意味着可以发布残缺帧。

本表中的非文字跨领域改动仍是候选；既有 ADR 的坐标 `Option<Affine2D>` 语义不被
机械替换成 Result，也不重新定义 CommandStack 的故障隔离规则。

## 6. 全公开面处置清单

“保留”也需清楚的 rustdoc 示例；“后续定向”表示本轮不批准重做该协议。

| 包/领域 | 现状入口或证据 | 处置 |
|---|---|---|
| Core root / prelude | `lib.rs` 仍重导出 SceneQuery、TrackedSceneQuery、RouteOutput、Prepared*、ResourceRegistry、RenderSubmission 等 | 专业协议移至领域模块；root/prelude 以常用类型白名单维护，不将“pub”视为入门推荐 |
| Core text | TextLayout/FontDescriptor 等位于 `render::text` | 按已接受整合专题提供 text 领域入口和统一测量/绘制 facade；排版、轮廓、栅格化分别替换 |
| Core figure | Figure 同时有 tuple/measurement、paint/in_bounds、大量 capability accessor | 分清 intrinsic/arranged/paint envelope；本轮先类型与文档一致性，capability 重构后续定向 |
| Core figure 构造 | `new(x,y,w,h)`、`from_bounds`、`with_bounds` 混用 | 几何必需类型统一 `new(Rectangle)`；Label `new(text).with_bounds(...)` 保留，不能强求所有构造同形 |
| Core shape | `prim_translate`、公开 validate/invalidate；raw width 输入 | primitive 内收；用户保留具名 detached 配置，attached 只用 editor；不得简单将 prim 去前缀而暴露另一条写路径 |
| Core tree | FigureTreeBuilder / FigureTree 查询 / scoped mutable facade | 保留阶段与所有权；container 和 parent 必须显式；不恢复隐式 root layout |
| Core query | `FigureNode::get_preferred_size` 等仍是 override/bounds 局部值 | 内收或明确为 configured override；测量入口保持 FigureTree/LayoutSnapshot 权威，不机械删 get |
| Core layout | LayoutManager、LayoutSnapshot、LayoutOutput、constraint_as | 保留小协议与受检输出，外部算法通过 snapshot/constraints/placement 组合；不增加万能 layout plugin |
| Core container | ViewportHandle/ScaleHandle 与 editor；LayeredPaneMut 借用 Runtime | 借用可变 facade 统一 `LayeredPaneMut`，identity/read handle 保留 Handle；不改变 state 归属 |
| Core connection | Anchor、Router、Locator、RouteRequest/Output、RoutingConstraint | 保留角色分离，避免改成泛称 ConnectionStyle；专业路由查询从 root 移至 connection |
| Core runtime | `prepare_submission` 将 Idle/Suspended/AwaitingCompletion/Error 全折叠为 None | 规范 `prepare_submission(...) -> FramePreparation`，吸收 `_state` 版本，完整状态必须到调用者 |
| Core frame capture | `prepare_frame` / `record_full_frame` 返回 owned NdCanvas | 与 recorder 拆分一起明确离线记录结果和错误；不把它们改名后当作真实 submission 成功证据 |
| Core event | `*_later`、NotificationEffect、StableSceneQuery | 保留延后与稳定阶段可见性；去掉 later 会误导已提交状态，事件点继续由引擎适配 |
| Core resource | FontId/ImageId、ResourceId、ResourceRegistry | 应用使用领域 ID；raw registry 同步归 host/render 专业层；不新增全局资源管理器 |
| Editor 入口 | `lib.rs` 集中重导出，内部 module 私有 | 开放经筛选的 model/command/viewer/tool/policy/text_input 领域入口，root 保留高频类型 |
| Editor 模型/命令 | ModelAdapter、CommandStack、CompoundCommand | 保留 model-only command 与组合契约；不把 Runtime 多操作称作可撤销 transaction |
| Editor 命令探测 | `can_undo/can_redo(&mut self)` 捕获扩展 command panic 后可置 fault | 不机械变 `&self`；候选 `check_undo/check_redo -> Result<bool, CommandStackError>`，显式执行外部代码/故障语义 |
| Editor 输入 | EditorDomain、Viewer、Tool 和 `*_without_selection` | 入门路径只经 Domain 仲裁；自定义 Tool 保留必要协议入口，后续按真实消费者收口，避免双 dispatch |
| Editor direct edit | `start/accept/cancel_direct_text_edit` 等 | 保留 session 与模型提交边界；是否专用借用 session facade 后续定向，不制造第二份编辑状态 |
| Inspector | FigureInspector::attach/capture/events、稳定 epoch | 保留只读 snapshot，文档明确返回 ListenerId 的解除责任；`capture` 可精确为 `capture_tree`，events 空结果与锁失败区分后续定向 |
| Vello | Native `new -> Self` 内部 expect；Web `new_web -> Result` | Native `new -> Result<_, VelloInitializationError>`；平台构造差异保留，初始化失败交还宿主 |
| Vello 诊断 | `size`、screenshot、GPU wait/callback | `pixel_size` 明确单位；保留提交/GPU 完成/窗口呈现区别，不包装为一个 misleading `render_done` |
| Winit adapter | WinitPlatformHost、输入和 IME bridge | 保留 window/事件适配；第三方类型只能在平台包，不渗透 Core；与 Web 对齐领域动作而非强行统一宿主类型 |
| Web adapter | WebPlatformHost、DOM/EditContext/TextInput bridge | 保留 capability fallback 与 effects；Web 异步初始化不伪装同步；不让 adapter 拥有第二份文本模型 |

额外稳定性约束：

- `FigureId`、`ConnectionId`、`EditPartId` 和 ModelId 不能统一成裸 NodeId；
  缩短名称不应消除 namespace 和模型/视觉身份差异。
- 保留 `Result<bool, E>` 已有幂等合同；本轮不为每个 setter 新建 Changed 枚举。
- LayoutSnapshot 的内部 `LayoutContext::get_*` 不属于公开 trait；
  不因文本搜索命中而计入公共迁移。
- 公共 `Figure` capability accessor 中涉及具体 LabelFigure 的扩展限制需要专题设计，
  不能用一次批量重命名宣称已经解决第三方 Figure 组合能力。

### 6.1 场景操作的组合规则

```rust,ignore
// owned 配置自由组合；Box 明确策略对象的所有权转交。
let label = LabelFigure::new(title).with_border(border);
let child = runtime.container(parent)?.add(Box::new(label))?;
runtime.label(child)?.set_text(updated_title)?;
runtime.figure(child)?.set_bounds(bounds)?;

// 读取已提交状态与稳定派生结果分开，查询不会偷偷执行 layout。
let bounds = runtime.tree().figure_bounds(child);
let stable = runtime.stable_query()?;
```

这组现有路径应保留；不再增加 `runtime.set_label_text(id, ...)` 的第二入口。
通用几何归 FigureMut，容器拓扑归 ContainerMut，具体组件语义归 typed mutable facade；
新增第三方组件通过已有 FigureComponentUpdate 协议，不要求引擎不断增加类型分支。

| 组合对象 | 允许的组装方式 | 提交/派生权威 |
|---|---|---|
| Figure + Border + 样式 | detached 配置，再由 Builder/Container 挂载 | 挂载后 Runtime |
| Container + LayoutManager + child constraint | 显式指定容器，添加时携带 constraint | manager 校验，Runtime 提交 |
| Connection + Anchor + Router + Locator | 按各自角色绑定，保留独立参数值 | connection service 与运行期路由协议 |
| Model + Command + Viewer | Command 改模型，Viewer 投影视觉 | EditorDomain/CommandStack，不由 Graphics 改模型 |
| Inspector + Runtime | 订阅并读取稳定快照 | Inspector 只读，生命周期通过 ListenerId 管理 |
| PlatformHost + RenderBackend + Runtime | 应用 composition root 显式组装 | Runtime 出版，backend 返回 outcome，Host 调度 |

多个 editor 调用是按顺序执行的多个操作。必须同时成功的操作应使用具名、可预检的
组合操作；禁止把普通 closure 包装成“事务”。布局器、路由器和绘制函数只访问与任务
相符的 snapshot/context，不取得可拆分的 UpdateManager 或 mutable tree。

## 7. 调用体验验收样例

### 普通文本：构建与修改均不使用 glyph API

以下使用现有 API，展示应保留的短路径：

```rust,ignore
let mut tree = FigureTree::new();
let label_id = tree
    .builder()
    .set_contents(Box::new(LabelFigure::new("标题")))?;
let mut runtime = Runtime::new(tree);
runtime.label(label_id)?.set_text("新标题")?;
```

文本注册与运行驱动由 composition root 负责；例子不代表不注册字体也能显示。

### 自定义绘制：排版一次，测量和绘制共享结果

```rust,ignore
// 目标：Figure 布局阶段取得受限测量上下文，不持有 &mut Runtime。
let layout = measure.layout_text(text, text_constraints)?;
let width = layout.width();

// paint 阶段消费已准备的 layout
gc.set_fill_paint(shared_paint.clone());
gc.fill_text(&layout, origin)?;
gc.set_stroke_style(shared_stroke.clone());
gc.stroke_rect(outline)?;
```

测量上下文使用 Figure 的已解析字体；独立录制则由 Graphics 提供同名入口。
这里复用 Paint/StrokeStyle/TextLayout；不引入另一个文本测量器或 backend 字体匹配。

### 宿主：每种帧状态可见

```rust,ignore
// 目标：原 prepare_submission_state 成为唯一规范 prepare_submission
match runtime.prepare_submission(surface, backend.capabilities()) {
    FramePreparation::Ready(submission) => {
        // 沿现有 complete_submission 协议将 backend outcome 交回 Runtime。
    }
    FramePreparation::Idle => {}
    FramePreparation::Suspended => {}
    FramePreparation::AwaitingCompletion => {}
    FramePreparation::Error(error) => handle_frame_error(error),
}
```

后端 submit/ack 的顺序、资源确认和失败重试不因方法改名而改变。

## 8. 分步实施与门禁

文字整合专题中的上下文、字体身份、轮廓与后端预处理合同已按 P2-G02 完成。
以下跨其他领域切片仍需各自确认范围，不能将本次文字整合批准扩张为全仓改名授权。
实施按依赖顺序组织，接口迁移与受影响 consumer 在同一切片闭合；
不先全仓替换名字再补行为合同：

| 顺序 | 切片 | 验证重点与现有 suite |
|---|---|---|
| A | TextMetrics/FontMetrics 与统一文字准备服务、领域路径 | 字体身份、度量与布局同源；`core.p2-t01-text-flow`、`core.p2-t02-text-interaction` |
| B | Graphics/MeasureContext/PaintContext、独立 Path 和资源闭合录制 | 窄上下文复用、失败传播、字体 lease；`core.facade`、`ga4.extension-consumers`、`core.runtime` |
| C | 内置与外部 Figure 消费同一 API，同步迁移受影响 fill/stroke 名称 | 旧画刷/单位语义、状态栈、图文像素；`core.p2-g01-graphics`、对应 Native/Web 像素 suite |
| D | 独立 outline/预处理 backend consumer 与 glyph 实例 | 坐标、缓存、非法资源、ordered composition；P2-G02 新入口需在实施前登记，已有 suite 不冒充覆盖新合同 |
| 其余候选 | root/prelude、非文字 tuple、Handle/Editor、帧结果、Vello 初始化、Editor/Inspector 收口 | 按领域原子迁移，定向 `core.facade`、`m8.viewport-scroll-zoom`、`g1.model-command` 等 |
| 后续定向 | Figure capability 全面改造、Editor session facade | 独立契约与真实消费者，不作为 P2-G02 隐含范围 |

每个切片先补齐旧→新符号清单、返回值、单位、状态副作用、调用点和外部消费者。
名称迁移与语义变化分别列明；避免批量替换产生编译通过但画刷/原点/状态变化的错误。
若需新增 suite，先登记 verification manifest，不在此杜撰已存在门禁。

验收不只看重命名数量：

1. 普通标签、布局、图像场景不需要导入 glyph/command/session 类型。
2. 自定义 Figure 能组合同一 Paint、StrokeStyle、Path、TextLayout；
   同一布局结果用于测量与绘制，不能靠隐藏全局 cache 达成。
3. 外部排版引擎仍能产出受检的非空 glyph 数据，替换后端仍能消费同一 IR。
4. Compile-fail/public surface 检查证明 Figure PaintContext 不具备
   `clear_commands`、`damage_mut` 和 submission 组装权限。
5. 非法 stroke/opacity/geometry 失败前后状态相同；
   文本画刷、clip、transform 和状态恢复保持像素语义。
6. Native/Web/Headless 的错误和等待状态不再折叠成“无帧”；
   command 探测失败保持已有 fault 隔离。
7. 0.1 阶段迁移所有 workspace 消费者与 rustdoc，无长期 deprecated 双入口。
   专业模块仍公开必要扩展能力，不通过“全部 private”减小接口数量。
8. 同一绘制 helper 在 Graphics 与 Figure PaintContext 中消费相同 TextLayout/Path；
   独立录制保有资源，不能在资源释放后只凭旧 ID 产生伪成功。

P2-G02 与本轮已批准的跨领域候选均按受影响 crate/精确测试、功能 suite、
跨 crate quick 和最终 full 完成分层验证；后续定向项仍需独立设计与门禁。

## 9. 设计效力与取舍

推荐：

- 统一文字准备、测量、图形与文本绘制入口，使用 TextLayout 复用排版，保留专业 `glyph_runs`；
- Graphics 统一 fill/stroke 语言，删除 foreground/background 歧义；
- 明确按阶段借用的上下文与 CommandRecorder 权限边界，保留独立轮廓替换能力；
- 按 A-D 的依赖顺序闭合 P2-G02，其他领域分别迁移，不保留同义转发壳。

ADR-019/023 的所有权与 package 边界、ADR-025 的统一文字 API 与 glyph 后端边界
已经接受。P2-G02 已完成 Graphics/MeasureContext/PaintContext、RecordedDrawing、
FigurePreparation 和 outline consumer；获准实施的非文字跨领域迁移也已按第 8 节
切片闭合。

成本：公共签名、外部 Figure/后端实现、导入路径和示例均需迁移。
收益：调用者可从动作、类型和模块判断行为，专业扩展仍保有同一底层能力。
不选择只加一层“简单 API”并长期保留所有旧入口，因为歧义与验证成本会继续累积。

文字整合部分以 ADR-025 和规范专题为 SSOT；本页保留后续定向清单，
不以本轮迁移完成状态推断 Figure capability 或 Editor session facade 已实现。

## 10. 证据导航

- [Core 公开导出](../../../novadraw/src/lib.rs)、
  [Graphics 导出](../../../novadraw/src/graphics.rs)、
  [绘图上下文](../../../novadraw/src/render/context.rs)
- [文本公开类型](../../../novadraw/src/render/text.rs)、
  [Figure 协议](../../../novadraw/src/figure/mod.rs)、
  [Triangle 构造与绘制](../../../novadraw/src/figure/triangle.rs)
- [树与 Builder](../../../novadraw/src/graph/mod.rs)、
  [Runtime/scoped mutable facades](../../../novadraw/src/runtime/runtime.rs)、
  [帧准备](../../../novadraw/src/runtime/runtime/frame_submission.rs)
- [Layout 协议](../../../novadraw/src/layout/mod.rs)、
  [Affine2D](../../../novadraw/src/geometry/transform.rs)
- [Editor 导出](../../../novadraw-editor/src/lib.rs)、
  [CommandStack](../../../novadraw-editor/src/command/mod.rs)、
  [Viewer](../../../novadraw-editor/src/viewer/mod.rs)
- [Inspector](../../../novadraw-inspector/src/lib.rs)、
  [Vello](../../../novadraw-backend-vello/src/lib.rs)、
  [Winit](../../../novadraw-platform-winit/src/lib.rs)、
  [Web](../../../novadraw-platform-web/src/lib.rs)

第三方参考限定于 `/Users/bytedance/Documents/code/GitHub/gef-classic` 的
`org.eclipse.draw2d` / `org.eclipse.gef`：
Graphics.java 314–372（文字与 foreground）、783–793（状态栈）、
IFigure/Figure/LayoutManager（对象组合与布局）、
commands/CommandStack.java（模型命令历史）。
不从 Zest 推导任何本提案能力。

## 附录 A：owned 与 scoped 术语说明

### A.1 owned：独立持有并可转交所有权

本文中的 `owned` 采用 Rust 所有权语义，表示调用者持有一个不依赖外部借用生命周期的
值。最直接的类型区别是 `T` 与 `&T` / `&mut T`：前者可以独立保存并按 API 合同
`move` 给其他所有者，后两者只能在被借用对象仍然有效且借用规则允许的期间使用。

本文按上下文使用 `owned`：

| 表达 | 含义 |
|---|---|
| owned value / owned 配置 | 调用者可在 detached 阶段独立构造、校验和组合的值，如 Paint、StrokeStyle、Border、Router |
| `with_*` 配置 owned value | 通常消费 `self` 并返回配置后的 `Self` 或 `Result<Self, E>`，不表示修改已挂载的 Runtime 状态 |
| move 进入 Builder / Runtime | 值的所有权转交给树或 Runtime；调用者不能保留旧可变引用绕过 Runtime 修改 attached 状态 |
| 进入绘制/运行期时冻结 | 被消费的配置按确定值或资源 revision 记录；之后修改另一个 clone 不得反向改变已录制命令或已提交状态 |
| Runtime-owned 服务 | 服务的生命周期、缓存、revision 和失效处理由 Runtime 统一拥有和协调，不是全局单例，也不由 Figure 或 backend 各持一份 |
| owned 录制状态 / owned 返回值 | Recorder、NdCanvas 等对象自身持有命令或状态，不借用生产它们的局部对象；这不等于已经完成 Runtime submission |

`owned` 不表示必须使用 `Box`、必须堆分配、底层资源绝对独占、不可 `Clone`，也不表示
值永远可变。owned value 内部仍可通过 `Arc` 等方式共享不可变数据；这里强调的是该值
自身具有明确的所有者、生命周期和转交边界。`Clone` 只产生合同允许的共享或复制值，
不授予 attached mutation 或跨 namespace 迁移权限。

典型生命周期如下：

```text
调用者持有 detached owned value
    -- move / Box 所有权转交 -->
Builder 或 Runtime 挂载
    -- attached 后 -->
Runtime 成为状态与提交权威
    -- 短期借用 -->
scoped mutable facade 提供对象式修改入口
```

### A.2 scoped：能力被限制在一次借用生命周期内

`scoped mutable facade` 中的 `scoped` 表示 facade 的有效范围受一次 Runtime 借用约束。
它不是可长期保存、可复制并独立修改场景的 handle，也不拥有第二份 FigureTree、
UpdateManager 或组件状态。其内部概念上只保存目标身份、能力信息和
`&mut Runtime`，所有修改仍委托 Runtime 的统一 mutation primitive。

```rust,ignore
{
    let mut figure = runtime.figure(figure_id)?;
    figure.set_bounds(bounds)?;
} // facade 的 Runtime 可变借用在此结束，之后才能再次使用 runtime
```

这里的 scope 由 Rust 借用生命周期决定，不要求一定与显式花括号完全相同；编译器可以
在最后一次使用后提前结束借用。关键合同是：

- facade 不能比所借用的 Runtime 活得更久；
- facade 存在期间，调用者不能同时取得冲突的 Runtime 可变借用；
- 获取 facade 只验证 namespace、attached 状态和 capability，不等于已经修改状态；
- facade 方法失败时不得绕过 Runtime 的校验、失效、通知和 damage 合同；
- 借用结束后，状态仍归 Runtime，不归已经失效的 facade。

因此，`scoped mutable facade` 可以理解为“受 Runtime 借用期限制的能力视图”：它提供接近对象
方法的调用体验，同时保持 Runtime 是挂载后状态的唯一提交权威。
