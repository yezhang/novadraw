# P2 Delta Backlog

类型：`roadmap`

本页记录 Draw2D Core 1.0 与当前 Editor roadmap 之外已确认的能力 delta 及其状态。
每项进入实施前必须先补充对应的 design/parity delta、自动验证和完成证据；本页不定义
运行时契约。

## 已完成批次

| 顺序 | Delta | 范围 | 状态 | 依赖 |
|---|---|---|---|---|
| 1 | P2-C01 | Connection decoration、endpoint locator 与 PointList visual bounds | `complete` | M9、M10.1 |
| 2 | P2-C02 | 障碍感知 shortest-path routing | `complete` | P2-C01 |
| 3 | P2-F01 | ScalablePolygonFigure | `complete` | P2-C01 的 PointList envelope |
| 4 | P2-T01 | TextFlow 第一阶段 | `complete` | M10.2、D4.4 |

状态只表示本页 delta 的执行进度：

- `not_started`：契约已进入 backlog，尚未实现；
- `in_progress`：已有实现或验证增量，完成判据尚未全部满足；
- `complete`：契约、实现、验证和 parity 证据全部闭合；
- `deferred`：已确认边界，但不进入当前 Core 增强批次。

本批次已于 2026-09-29 完成自动门禁与 macOS Native/Vello 人工验收。后续工作必须
建立新的 P2 delta 并完成设计评审；FigureInspector、Studio、offscreen thumbnail
与高级 self-loop 继续保持 deferred，不因本批次完成而自动进入实施。

## 下一批规划

| 顺序 | Delta | 范围 | 状态 | 依赖 |
|---|---|---|---|---|
| 1 | P2-T02 | TextFlow immutable interaction geometry | `not_started` | P2-T01 |
| 2 | P2-E02 | Direct text edit、draft、selection、IME 与 Command | `not_started` | P2-T02、G1-G5 |

本批次分成 Core 与 Editor 两个可独立验收的 delta。P2-T02 不引入 mutable editor
state；P2-E02 不复制 shaping、caret 或 selection geometry。实施前先执行 P2-T02，
再执行 P2-E02a headless session 和 P2-E02b Native/Web input bridge。

## Connection

### P2-C01: Decoration、Endpoint Locator 与 PointList visual bounds

状态：`complete`

`api_semantics`：`builtin.figures`、`connection.figure`、`connection.locator`

原 `P2-R01` PointList miter visual bounds 合并到本项，不再独立实施。原因是普通
Polyline/Polygon 与 Connection decoration 必须共享同一 stroke envelope 契约，不能
让箭头 Figure 和普通 PointList Figure 使用两套 bounds 规则。

交付范围：

1. PointList visual envelope 同时消费 line join、stroke width 和显式 miter limit；
   Round/Bevel 不采用 miter 扩张，style mutation 必须重新规范化 bounds；
2. 提供基于模板点的 polygon/polyline decoration Figure；方向来自 terminal
   non-zero segment，找不到时才使用 AnchorSite normal，否则返回结构化错误；
3. source/target terminal locator 输出位置与切线方向；side-aware endpoint label
   locator 使用沿切线的 `u_distance` 与沿法线的 `v_distance`；
4. decoration 仍是 Connection 的普通 child；route endpoint 保持 Anchor 真值，
   line inset 只影响 Connection paint/hit-test，不改 committed route points；
5. geometry、locator child、subtree envelope、freeform extent 与 old/new damage 在
   同一 Runtime route commit 中预检并原子提交。

完成判据：

1. 锐角 miter、Round/Bevel、style mutation 和 clip 不截断测试；
2. source/target、直线/折线/self-loop、重复 terminal point 与全退化 route 测试；
3. transform/viewport 下 decoration orientation 和 endpoint offset 测试；
4. decoration old/new damage、freeform extent 与 path bounds 分离测试；
5. `core.p2-c01-connection-decoration` suite 通过。

规范入口：

- `doc/design/architecture/reusable-shape-border.md`
- `doc/design/architecture/connection-routing.md`
- `doc/parity/draw2d/api-coverage.md`

完成证据（2026-09-29）：

- `PolylineFigure`、`PolygonFigure` 与 `ConnectionFigure` 共享
  `DEFAULT_STROKE_MITER_LIMIT` visual outset；
- `FigureEditor` 可原子修改 PointList stroke width / line join；
- `PolygonDecorationFigure`、`PolylineDecorationFigure`、`EndpointLocator` 已进入
  Core 公开 API；
- route preflight 同时校验 decoration geometry，terminal duplicate point 与
  AnchorSite normal fallback 已覆盖；
- `advanced-figures-app` 提供 decoration orientation、line inset 与
  EndpointLocator offset 的 Native 可视验收场景；
- `cargo xtask verify core.p2-c01-connection-decoration` 通过。

### P2-C02: 障碍感知 Shortest-path Router

状态：`complete`

`api_semantics`：`connection.router`

在既有 `RouterRegistry`、routing-domain group、tracked dependency 和 atomic batch
commit 上增加障碍感知路由，不复制 Draw2D Router 持有 Figure listener、直接修改
Connection 或使用全局可变状态的实现方式。

交付范围：

1. Router 配置显式给出有序 obstacle FigureId；Runtime 每批将其中 attached、可见且
   非 Connection member 的 Figure 映射为 immutable routing-domain snapshot；
2. 每个 routing batch 只构建一次稳定 obstacle snapshot；obstacle
   geometry/visibility/topology/transform 通过现有 dependency generation 增量失效；
3. 端点从 obstacle 边界安全出入，搜索结果为确定性正交 PointList，不穿越 obstacle
   interior，并移除重复点与共线冗余点；
4. 同批 connection 先完成 route 与 locator geometry preflight，再原子提交；任一
   失败产生结构化 unresolved 原因，不保留陈旧 route；
5. 建立节点数、障碍数、连接数明确的 headless 性能基线，记录 route calculation 与
   obstacle snapshot rebuild 次数。

完成判据：

1. 直达、单障碍、多障碍、不可达、端点贴边、障碍移动/删除/隐藏测试；
2. 同一 snapshot 输出确定、child order 稳定、无关 mutation 不重路由测试；
3. batch failure 无 partial commit，恢复后自动重路由；
4. 64 connections / 64 obstacles 基线与增量失效计数验证；
5. `core.p2-c02-shortest-path-routing` suite 通过。

规范入口：

- `doc/design/architecture/connection-routing.md`
- `doc/parity/draw2d/api-coverage.md`

完成证据（2026-09-29）：

- `ShortestPathConnectionRouter` 使用显式有序 obstacle FigureId 与可配置 clearance、
  bend penalty、minimum stub；
- Runtime 将障碍映射到 routing domain，一次 snapshot 服务同组全部 connection；
- Hanan grid + bend-aware Dijkstra 生成确定性正交路径，直线与 L 路径使用无阻挡快速路径；
- 几何、可见性和拓扑变化通过 tracked dependency 自动重路由；
- 64 connections / 64 obstacles 验证为 1 次 snapshot build、64 次 route calculation；
- `advanced-figures-app` 提供双连接共享 obstacle snapshot 与上下避障路径的 Native
  可视场景；
- `cargo xtask verify core.p2-c02-shortest-path-routing` 通过。

## Figures

### P2-F01: ScalablePolygonFigure

状态：`complete`

`api_semantics`：`builtin.figures`

交付一个 bounds-driven 的可缩放多边形。模板点是不可变设计坐标；实际 local points
由模板 bounds 到 Figure client bounds 的确定性映射派生。它不建立独立渲染路径，
继续复用 PolygonFigure 的 fill/outline、precise hit、stroke envelope 和 Runtime
damage 协议。

交付范围与完成判据：

1. template replacement、bounds resize、stroke/join mutation 都使派生几何失效；
2. 空模板、单轴退化模板和非有限输入有明确 Result 语义，不产生 NaN；
3. preserve-aspect 与 stretch 模式均有稳定 alignment 规则；
4. fill/outline、锐角 miter、precise hit、nested transform 和 old/new damage 测试；
5. 示例展示同一模板在不同 bounds 和缩放模式下的结果；
6. `core.p2-f01-scalable-polygon` suite 通过。

规范入口：

- `doc/design/architecture/reusable-shape-border.md`
- `doc/parity/draw2d/api-coverage.md`

完成证据（2026-09-29）：

- `ScalablePolygonFigure` 将不可变 template 映射到当前 NodeState bounds；
- Stretch 与 PreserveAspect 支持双轴 alignment 和单轴/双轴退化模板；
- stroke/miter outset、fill/outline 与 precise hit 共用同一派生点集；
- FigureEditor 支持 template、scale mode 与 alignment 的 typed mutation；
- `shape-app` 与 `advanced-figures-app` 提供 scalable polygon 场景，后者集中验证
  Stretch、PreserveAspect、alignment 与 bounds mutation；
- `cargo xtask verify core.p2-f01-scalable-polygon` 通过。

## Text

### P2-T01: TextFlow 第一阶段

状态：`complete`

`api_semantics`：`text.flow`

第一阶段建立只读段落流，不包含 caret、selection、IME 或直接编辑。它复用
Runtime-owned `TextLayoutEngine`、`TextLayout`、GlyphRun 与受宽度约束测量，不建立
第二套 shaping/line-breaking 实现。

交付范围：

1. FlowPage 包含有序 Paragraph，Paragraph 包含有序 inline text fragment；
2. fragment 作为稳定语义边界；第一阶段整页继承 Figure resolved style，fragment
   级 font/foreground 留给后续独立 delta；
3. 支持 hard break、soft wrap 与尾部 truncate，bidi 由 TextLayoutEngine 解析；
4. 相同 width constraint 的 measure、arrange、paint 复用同一 immutable layout；
5. fragment 或 width 变化通过 Runtime validation 原子更新 measurement、glyph IR、
   bounds 与 damage；
6. UTF-8/CJK/bidi、fragment 拼接、窄宽换行、hard break、truncate 和 cache
   invalidation 测试；
7. `core.p2-t01-text-flow` suite 通过。

规范入口：

- `doc/design/architecture/text-layout.md`
- `doc/parity/draw2d/api-coverage.md`

完成证据（2026-09-29）：

- `FlowPage`、`FlowParagraph`、`InlineTextFragment` 与 `TextFlowFigure` 已进入 Core；
- NoWrap、SoftWrap 和行数 Truncate 复用 Runtime-owned TextLayoutEngine；
- paragraph hard break、UTF-8 grapheme-safe ellipsis、CJK/bidi 与 width cache 已覆盖；
- FigureEditor 支持 page 与 wrapping typed mutation；
- `text-app` 增加共享 Native/Web TextFlow 场景；`advanced-figures-app` 集中验证段落、
  fragment、SoftWrap、NoWrap、CJK 与 Truncate 可视结果；
- `cargo xtask verify core.p2-t01-text-flow` 通过。

### P2-T02: TextFlow interaction geometry

状态：`not_started`

`api_semantics`：`text.interaction`

Core 在 P2-T01 immutable TextLayout 上增加 document position、affinity、hit-test、
caret、selection quad 与 movement 查询。`TextFlowFigure` 不拥有 draft、selection 或
IME composition；这些 mutable 状态只由 P2-E02 Editor session 持有。

交付范围：

1. paragraph-local UTF-8 position、upstream/downstream affinity 与有向 range；
2. immutable interaction map 保存 paragraph/fragment、cluster、line、bidi 与 visible
   range 映射；
3. point/position 双向查询、caret geometry 和跨 paragraph selection quads；
4. visual cluster、word、line、paragraph 与 document movement；
5. 外部 TextLayoutEngine 可构造同等受检 interaction map，不暴露 Parley 类型；
6. `core.p2-t02-text-interaction` suite。

完成判据：

1. ASCII/CJK/emoji/combining mark 与 grapheme-safe movement；
2. soft/hard wrap affinity、空 paragraph、跨 fragment/paragraph selection；
3. mixed bidi logical/visual 映射；
4. truncate、stale revision、非法 UTF-8 boundary 的结构化拒绝；
5. nested transform、viewport scroll/zoom 后 caret surface geometry；
6. 纯查询不产生 validation 或 damage。

规范入口：

- `doc/design/architecture/text-layout.md`
- `doc/design/editor/p2-direct-text-edit.md`
- `doc/verification/plans/p2-text-direct-edit.md`
- `doc/parity/draw2d/api-coverage.md`

## Rendering

### P2-R02: Image source rectangle

状态：`complete`

`api_semantics`：`graphics.context`

2026-09-29 已完成 Draw2D `Graphics.drawImage(Image, source, destination)` 对等切片：

1. Image Render IR 始终携带图像物理像素域的显式 source rectangle；
2. `NdCanvas::draw_image_region` 使用结构化错误拒绝非有限、负尺寸和越界 source；
3. Vello adapter 使用 source-to-destination affine 与 destination clip，完整消费该命令；
4. 完整图像 convenience API 进入同一 IR，不保留 `Option` 或 backend 特判协议；
5. `core.image-source-rectangle` 覆盖 backend-neutral、Vello 与 Web feature 门禁。

规范与证据：

- `doc/design/rendering/image-source-rectangle.md`
- `doc/parity/draw2d/api-coverage.md`
- `novadraw/tests/image_source_rectangle_contract.rs`
- `verification/suites.toml`

## Developer tooling

### P2-D01: FigureInspector 开发期可观测性

状态：`deferred`

FigureInspector 以独立 crate 消费 Runtime 的稳定场景查询与提交后 notification journal，
为 Native、Web 和 headless 工具提供统一诊断事实。它不属于 Draw2D Core 的运行时语义，
也不改变 GEF Editor 里程碑。按当前优先级，本项与 Studio 一并后置，不进入
P2-C01/C02/F01/T01 执行批次；已有实现和文档保留，不继续扩展。

首期范围：

1. `FigureTreeSnapshot` 保留 containment、child order、节点状态和 Figure diagnostic name；
2. 有界 FIFO event timeline 保留 `source_epoch`、`sequence` 和 typed effect；
3. outline tree、属性面板、时间线作为宿主 UI 标准布局；
4. 不进入 render/validation 热路径，不保存逐 effect 的完整历史场景；
5. Editor Figure -> VisualOwner -> EditPart -> Model 关联由可选 adapter 后置。

规范与决策：

- `doc/design/architecture/figure-inspector.md`
- `doc/adr/adr-016-figure-inspector-observability.md`

## Editor

### P2-E01: 高级 Self-loop 路由策略

状态：`deferred`

Draw2D/GEF 核心只提供同源同目标 Connection、Anchor、Router 与 routing constraint
扩展点，不规定 self-loop 的开口方向、折点数量或障碍避让。Native node editor 当前用
两个显式 absolute bendpoints 实现节点右侧的三段正交 self-loop；该策略只承担 demo
验收，不进入 Draw2D/GEF parity。

Core 与当前 G5 必须继续保证：

1. self-loop 只投影一个 ConnectionPart，并同时进入 source/target relation；
2. 两个 endpoint handle 保持独立，重连保留 Connection 身份与 history 原子性；
3. 应用可通过 Anchor Descriptor、Router 和 typed constraint 生成非退化回环；
4. demo self-loop 跨 owner 重连时在新 owner 坐标域重建折点，节点移动时整体平移。

后续产品 delta 可按真实需求补充：

1. 根据可用空间选择上、下、左、右开口；
2. 多条 self-loop 的 lane 分配与稳定排序；
3. 节点、端口、标签和其他连接的障碍避让；
4. resize、端口迁移与自动布局后的形状保持；
5. 对应基准、视觉回归和可序列化 routing descriptor。

### P2-E02: Direct text edit 与 IME

状态：`not_started`

`api_semantics`：`direct_edit`

Editor 在既有 Request/Policy/CommandStack 和 P2-T02 text geometry 上建立一次只允许
一个 owner 的直接编辑会话。它采用 GEF manager/policy/command 的职责分离，但不采用
SWT/JFace CellEditor 作为公共契约或可见文本真值。

交付范围：

1. typed feature descriptor、request/policy role、source revision 与 session identity；
2. transient draft、text selection、preedit、feedback、caret blink 与 cleanup；
3. accept/cancel、stale conflict、CommandStack、undo/redo 和 part retire；
4. platform-neutral text-input event/effect 与 lease identity；
5. Winit IME bridge、Web DOM input/composition bridge 和 candidate area；
6. headless replay、Native/Web 示例与人工验收。

完成判据：

1. draft 不修改业务模型，accept 只生成一个 Command，cancel 不进入 history；
2. Viewer selection 与 text selection 隔离，active session 阻止 Tool fallback；
3. preedit/commit、dead key、focus loss 和迟到 lease event 有确定语义；
4. stale source revision 不覆盖外部修改，cleanup failure 进入 faulted；
5. scroll/zoom/resize 后 feedback、caret 与候选窗对齐；
6. `editor.p2-e02-direct-text-edit` 与 `platform.p2-e02-text-input` suites 通过。

规范入口：

- `doc/reference/gef/direct-editing.md`
- `doc/design/editor/p2-direct-text-edit.md`
- `doc/verification/plans/p2-text-direct-edit.md`
- `doc/parity/gef/api-coverage.md`
