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
| 5 | P2-T02 | TextFlow immutable interaction geometry | `complete` | P2-T01 |

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
| 1 | P2-E02 | Direct text edit、draft、selection、IME 与 Command | `complete` | P2-T02、G1-G5 |

P2-T02 已闭合且未向 Core 引入 mutable editor state。P2-E02b 的 Native/Web input
bridge 与可运行场景已完成；2026-09-30 视觉回归修正版已通过 Native/Web 人工复验。
P2-E02 不复制 shaping、caret 或 selection geometry。

## 长期能力分母批次

以下 delta 承接
[`Draw2D 长期能力分母`](../parity/draw2d/api-coverage.md#长期能力分母与处置)。
`deferred` 表示已确认目标和退出条件，但未进入当前实施主线；不能从长期分母中删除。

| 顺序 | Delta | 范围 | 状态 | 依赖 | 最低毕业证据 |
|---|---|---|---|---|---|
| 1 | P2-S01 | 原子 indexed + constraint child add | `complete` | GA-1 mutation/API | Builder/Runtime 原子失败、顺序与通知 |
| 2 | P2-G01 | path clip、gradient、custom dash/offset、miter 与 XOR 替代裁决 | `in_progress` | GA-1 root/Graphics API | Core IR、Vello lowering、失败语义、视觉验证 |
| 3 | P2-F02 | 开放的 Figure capability registry 与 Runtime mutation | `complete` | ADR-014/019、GA-4 | 外部标准/自定义 capability、无 Core accessor 扩张或横切能力 concrete 分支 |
| 4 | P2-M01 | Animation Timeline 与 Presentation Plane | `in_progress` | P2-G02 complete；M01-B 前闭合 P2-G01 visual surface；外部 binding 依赖 P2-F02 | 确定时钟、双平面、跨域消费者、禁用零工作与视觉证据 |
| 5 | P2-L01 | Directed/Compound graph layout adapter | `not_started` | GA-4 扩展边界 | 独立图模型、确定性输出、外部算法集成 |
| 6 | P2-C03 | 跨 viewport Connection 可见性与 clipping provider | `not_started` | M8、M9、P2-C02 | nearest-common viewport、damage、hit-test |
| 7 | P2-T03 | fragment style 与剩余 inline/block flow | `deferred` | P2-T01/T02 | 测量、paint、interaction 使用同一快照 |
| 8 | P2-W01 | repeat scheduler、ButtonGroup/radio/checkbox/slider | `deferred` | M6、M10.4 | 输入状态、时钟、action 与 accessibility |
| 9 | P2-O01 | printing/export/ScaledGraphics 等价输出目标 | `deferred` | P2-G01、RenderBackend | 无 SWT 类型、scale/clip/text/image 等价 |
| 10 | P2-A01 | Web action/focus 与 Native 原生 AT provider | `not_started` | GA-3 平台矩阵 | Core snapshot/action 到平台双向闭环 |
| 11 | P2-LC01 | 同 Runtime live unmount/mount 所有权评估 | `deferred` | ADR-014 生命周期 | ADR、真实消费者、ID/资源/退出路径 |

共同约束：

1. 每项实施前新增或修订 `normative-design` / ADR，并列出 `api_semantics`；
2. 不以复制 Java 类名作为毕业条件，以可观察行为和失败语义为准；
3. 所有 public Render IR 必须被启用 backend 完整消费，或在提交前结构化拒绝；
4. P2-L01 只参考 `org.eclipse.draw2d.graph`，不得使用 Zest 定义目标；
5. P2-LC01 不恢复自动跨 Runtime 活对象迁移，默认仍是模型/描述重建；
6. 平台相关 delta 的支持声明以 `platform-support-matrix.md` 为准。

## Figure capability

### P2-F02: 开放的 Figure capability registry

状态：`complete`（2026-10-08 registry、mutation、分层迁移与开放
`PresentationBinding<V>` 已通过验证）。

目标：`GOAL-EXT`、`GOAL-CAP`。关闭
[临时概念设计审计 TC-02](../verification/reviews/temporary-concept-design-audit-2026-10-08.md#32-tc-02capability-仍由-core-列举)，
并为 P2-M01 外部 presentation binding 提供发现基础。

`api_semantics`：`paint.protocol`、`figure.lifecycle`、`figure.box.client_area`、
`figure.properties`、`event.input_listeners`、`accessibility.bridge`、
`viewport.scroll_zoom`、`connection.figure`、`text.flow`、`widgets.basic`。

规范入口：

- [ADR-027](../adr/adr-027-open-figure-capability-registry.md)；
- [Figure Capability 统一扩展模型](../design/architecture/figure-capability-model.md)。

实施切片：

1. attach-time owned descriptor registry、typed key、query/mutation error 与外部 consumer；
2. Scale、Layer、Freeform、Container、Input、Lifecycle、Accessibility；
3. PointList、ScalablePolygon、TextFlow、Label、Viewport、Image 收口为 concrete
   model/type、typed component update 与专用 facade；
4. Border、Clickable、Connection、Decoration 等真实跨类型 capability；
5. 删除 `Figure` 领域 accessor、伪 behavior trait、横切能力 concrete 分支和旧 root
   导出；类型专用 facade 的受控闭合 adapter 保留；
6. P2-M01 使用开放 `PresentationBinding<V>`。

最低毕业证据：

1. 外部 Figure 接入标准和自定义 capability 均不修改 Core 类型分支；
2. 同步与 deferred capability update 共享 prepared commit、revision、invalidation、
   damage 和 notification；
3. duplicate、foreign、stale、absent、prepare rejection 和 panic/fault 有结构化语义；
4. Core 中不再通过具体 Figure downcast 判断横切能力；PointList/TextFlow/Image/
   Viewport 等具体类型不再伪装成 capability；
5. render、layout、event、connection、Native/Web 编译及外部消费者门禁通过；
6. 工作量基线无未解释回退，最终 quick/full 按项目分层门禁执行。

实现与验证证据（2026-10-08）：

- attach-time owned registry、typed query、同步/deferred capability update 与共享 prepared
  commit 原语已落地；
- Preparation、Input、Lifecycle、Accessibility、Container、Layer、Freeform、Scale、
  Border、Clickable、Connection、Decoration 已使用标准 descriptor；
- `PointListFigureBehavior`、`ScalablePolygonBehavior`、`TextFlowBehavior` 及对应
  `Figure` accessor 已删除；PointList 使用 Polyline/Polygon crate-private 闭合 adapter，
  ScalablePolygon/TextFlow/Image/Viewport 使用 concrete type、typed handle 或专用 facade；
- Label/Button/Toggle 的共享 Label 内容使用闭合 concrete subcomponent adapter，不对外
  声称开放 capability；
- `PresentationBinding<V>` 已通过 attach-time registry 开放；外部 Figure 可提供
  committed value 与 immutable content presentation，不修改 `ChannelBinding` 或 renderer；
- Connection route/dash 已迁移到标准 binding，递归主循环删除领域特判；同 Figure
  同 family channel 可组合，不同 family 的冲突、Replace、dispose 与 cancel 复用统一
  Animation 语义；
- capability/animation 基础设施未进入 prelude，旧伪 behavior 已退出 root；完整
  root/prelude allowlist 归 TC-13；
- `core.p2-f02-figure-capability` 覆盖 6 项专用外部契约及 120 项关联回归；
  Core Clippy、docs、quick 与 diff check 均通过；full 的 fmt、Facade、Native/Web 和
  workspace Clippy 通过，最终 workspace test 被并行中的 Builder `Result` 测试迁移阻断；
- 完整记录见
  [P2-F02 Figure Capability 实现记录](../verification/reviews/p2-f02-figure-capability-completion-2026-10-08.md)。

## Figure 树 API

### P2-S01: 原子 indexed + constraint child add

状态：`complete`

目标：`GOAL-CAP`、`GOAL-EXT`。2026-10-02 核心 API 主线首个切片。

`api_semantics`：`figure.tree`、`layout.manager`、`notification.ancestor`、
`notification.layout_update`、`validation.protocol`、`damage.repaint`。

规范入口：[动态协议 §5.1](../design/architecture/dynamic-architecture.md#51-原子-child-insertionp2-s01)。
对标 `org.eclipse.draw2d.Figure.add(IFigure,Object,int)` 的顺序、constraint 与通知时序；
延续 ADR-019 命名复合操作，使用 Rust owned Figure 与 Result 失败语义。

交付与毕业条件：

1. Builder/ContainerMut 提供指定 index 的添加及携带外部 typed constraint 的添加；
2. 预检 parent、admission、index、深度及 manager compatibility，失败不发布半成品；
3. 无 manager 的 constraint 保留、后续替换 manager 校验、生命周期与通知顺序一致；
4. 外部自定义 Figure/Layout 消费者验证，不修改引擎枚举即可接入新 constraint；
5. `core.p2-s01-child-insertion` suite 覆盖顺序、布局、错误原子性与发布通知；
6. 使用平台无关 headless 契约验证，O(n) 有序插入不新增全树扫描；平台采证后置。

实现与验证证据（2026-10-02）：

- Builder 与 ContainerMut 已共享结构预检/发布原语；既有 add 与 Layer 添加继续
  复用该原语，Runtime 资源登记、失效与生命周期复用既有路径；
- `novadraw/tests/p2_child_insertion.rs` 仅通过公开 API 实现外部 Figure/Layout 与
  自定义 Placement constraint，验证 Builder/Runtime 得到相同顺序与布局；
- `core.p2-s01-child-insertion`：8 项通过，覆盖原子拒绝、失败 ID 作废、通知、
  admission、10,000 层深度及 validator panic 隔离；
- `cargo xtask check --quick`：通过，包含 Native/Web 编译与第三方类型边界检查；
- `cargo xtask check --full`：通过，包含 Clippy 与 workspace 单元、集成、文档测试；
- 本项不改变 callback FIFO、已有 Figure reparent 或 keyed Layer 协议。

## Graphics

### P2-G02: 统一测量/绘制 API 与 glyph 准备链路

状态：`complete`（2026-10-05 API、默认/可替换轮廓链路与验证门禁已完成）。

目标：`GOAL-CAP`、`GOAL-EXT`。在现有布局与 Graphics 基础上提供统一调用集合，
并为未来片段着色器字体后端保留“布局 → 曲线 → 预处理 → 绘制实例”链路。

`api_semantics`：`graphics.context`、`text.flow`、`text.interaction`、
`paint.protocol`、`damage.repaint`、`render.backend_session`。

规范入口：[ADR-025](../adr/adr-025-unified-graphics-and-glyph-preparation.md)、
[文字与图形整合](../design/rendering/text-graphics-integration.md)。
本项与 deferred 的 P2-T03 富文本样式无关，也不关闭 P2-G01 未完成的 surface 验收。

交付切片：

1. Graphics 同一 API 集合提供 font metrics、measure/layout text 与图文绘制，
   MeasureContext/PaintContext 保持阶段边界，复用 Runtime-owned 服务。
2. 默认 Parley/Vello 主链路与独立 GlyphOutlineProvider 消费者；
   保留字体身份、变体、曲线、baseline、布局与 ink 的一致性。
3. 独立字体后端 consumer 验证定位 glyph、曲线预处理和实例组合；
   网格/曲线索引格式保持 backend-local，覆盖缓存复用、资产导入校验和失效。
4. 外部 Figure、失败原子性、图文交错 clip/transform/paint 的 Native/Web 验证。

实现证据：

- `Graphics`、`MeasureContext`、`PaintContext` 和 `RecordedDrawing` 已形成统一、
  分阶段且资源闭合的公开调用面；外部 Figure 通过不可变 `FigurePresentation`
  原子发布 measurement、visual bounds 与 drawing。
- `SkrifaOutlineProvider`、`OutlineCache`、`GlyphInstance` 与显式
  `OutlineTextEngine` 已实现；默认 glyph 路径保持不变，显式 provider 路径降低为
  backend-neutral Path 并由 Vello 编码。
- `core.p2-g02-text-graphics` 覆盖 fallback/RTL/variation、空轮廓、缓存复用、
  stale/foreign revision、状态栈和外部 Figure；详细记录见
  [P2-G02 实现证据](../verification/reviews/p2-g02-text-graphics-evidence.md)。
- 用户未来自研片段着色器、分网格算法和 GPU 数据格式仍属于 backend-local 后续实现，
  不作为本项 Core API 交付。

### P2-G01: Graphics 扩展

状态：`in_progress`（实现已贯通，Native GPU 离屏与 WebGPU 像素已验证；
Native surface 局部修复验收尚未闭合）。

目标：`GOAL-CAP`、`GOAL-EXT`。

`api_semantics`：`graphics.context`、`geometry.primitives`、`paint.protocol`、
`builtin.figures`、`connection.figure`、`damage.repaint`、`frame.preparation`、
`render.backend_session`。

规范入口：[Graphics 扩展契约](../design/rendering/p2-g01-graphics-extension.md)、
[ADR-024](../adr/adr-024-graphics-paint-stroke-and-clipping.md)。
已对标 Draw2D/GEF，并核对锁定版本 Vello 0.10 的 lowering 能力；
规范已获批准，能力毕业仍需实现和视觉证据。

实施顺序：

1. 统一 StrokeStyle，贯通 custom dash/offset/miter、backend 与实际 visual bounds；
2. ClipPath/FillRule 与矩形 clip 共享可恢复状态链；
3. Paint/linear gradient 贯通形状、路径和 glyph，以及多能力与非法输入拒绝；
4. 反馈 layer 替代 XOR 交互用途，完成外部消费者及 Native/Web 视觉验收。

公共 IR、RenderOutcome 错误边界与 XOR 处置已获批准；
`core.p2-g01-graphics` 已登记，随切片加入对应验证。毕业仍要求 Core IR、
Vello lowering、失败原子性和视觉证据全部闭合。
本项不推进后置的 WindowServer/Chrome 性能采证或原生 runner 建设。

实现切片进展（2026-10-03，尚未毕业）：

- 受检 `StrokeStyle`、custom dash/offset/miter 已贯通 Canvas 状态与矢量 IR、
  Vello lowering；显式 recorder 携带完整样式，宽度 setter 返回 Result。
- Polyline/Polygon、Connection、端点 decoration 与 ScalablePolygon 统一持有
  完整受检描边；包围盒与绘制使用相同实际 miter/cap。PointList scoped mutable facade
  支持完整替换及 miter/cap/dash/offset 修改，在同一事务中完成几何重归一、
  新旧 damage、freeform extent 失效与单次 FigureMoved。
- `required_capabilities` 与统一预检已接入 Runtime、Vello 和 Canvas2D fallback；
  backend 在接受资源/session 前拒绝不支持的描边。
- `core.p2-g01-graphics` 已登记 Core 和 Vello 两个验证入口。描边基础轮次 Core
  8 项公开契约、Vello 17 项单测通过，M1/M10 相关回归 16 项通过。Figure 轮次
  Core 公开契约扩展到 13 项通过，M10 图元/边框 14 项及 M9 Connection 29 项
  回归通过，workspace 全目标类型检查通过。新增用例覆盖 miter 增大/缩小、
  dash 重绘、非法修改不排入重绘、实际 decoration envelope、freeform extent
  与一次移动通知；旧无效 Connection 几何 fixture 已迁移为有限描边包络溢出。
- 路径裁剪已贯通受检 `ClipPath`、独立路径所有权、Canvas 状态深度及 Vello
  矩形/路径混合裁剪链；`FillPath` 携带 NonZero/EvenOdd。恢复前缀比较包含完整
  几何、fill rule 和调用时 transform；空裁剪保留抑制绘制的语义。Vello 单测
  扩展到 19 项通过，workspace 全目标类型检查通过。
- Paint/LinearGradient/GradientStop 已贯通矢量与 glyph 消费，原颜色 setter
  替换为 Solid；Canvas 快照将 alpha 乘入所有渐变站点一次，恢复原 Paint。
  Vello 显式使用 Pad、sRGB 与 premultiplied alpha；渐变端点随几何缩放。
- GlyphPaint 使用完整 StrokeStyle。锁定版 Vello glyph cache 忽略 dash，
  因而有虚线的轮廓字形由 backend 私有 Skrifa 轮廓转换接入共享路径描边；
  没有矢量轮廓的彩色/位图字形延续 Vello 行为，不声称其具有轮廓虚线。
- 多能力预检覆盖 ellipse 两种 Paint 和 glyph 的三项组合；Runtime 发布前、
  Vello 接收资源/session 前（含截图入口）拒绝实际 DPI/transform 下的溢出、
  退化渐变与裁剪曲线溢出。结构化 InvalidGraphicsInput 携带命令索引与原因。
- Core 公开契约扩展到 23 项通过，Core 完整 crate 测试通过；新增用例覆盖
  alpha/state、颜色覆盖渐变、逐项能力消融、派生值拒绝、重复拒绝后帧号和资源
  snapshot 保留并重试。Vello 21 项单测、Editor 9 项直接文本编辑回归通过。
  跨 crate quick gate（含 Native/Web 后端）、文档门禁通过。
- 2026-10-05 共享外部消费者与像素程序已落地，使用真实公开 API 组合渐变、
  custom stroke、曲线路径裁剪、glyph/image 及专用 feedback layer。
  Native GPU 离屏 DPI 1/2 通过；WebGPU 使用可选帧内 PNG 快照避免读取已清空的
  交换缓冲区，复用相同像素断言。miter 增大/还原与反馈显示/移动/取消有连续帧证据，
  后端两个提交入口拒绝非法输入后保留原像素且原合法帧可重试。
- 最终 Core/Vello 契约、Native 离屏、WebGPU 像素、文档门禁通过；
  full 质量门禁各入口通过（Clippy 奇偶判断等价整改后续跑 Clippy 与 workspace
  单测、集成和 doctest，未重复已通过入口）。
- Native surface suite 在重绘事件后首帧仍返回 Skipped，局部 retained repair
  不能用离屏完整重绘结果替代，因此本 delta 保留 `in_progress`。
  详细 suite、结果、平台范围与复现方式见
  [P2-G01 验证记录](../verification/reviews/p2-g01-graphics-evidence.md)。

## Animation

### P2-M01: Animation Timeline 与 Presentation Plane

状态：`in_progress`。ADR-026 已接受；M01-A、M01-B 已完成，下一切片为 M01-C
领域消费者。

目标：`GOAL-CAP`、`GOAL-EXT`、`GOAL-PERF`。建立可选的一等动画基础设施，
让属性、布局结果、Connection route、Viewport、生命周期和持续效果共享同一
Runtime 时钟、编排、取消、damage 与提交协议。

`api_semantics`：`animation.timeline`、`animation.presentation`、
`frame.preparation`、`damage.repaint`。

规范入口：[ADR-026](../adr/adr-026-animation-and-presentation-plane.md)、
[Animation / Presentation Plane 设计](../design/animation/animation-system.md)、
[Animation 公开 API 合同](../design/animation/public-api-contract.md)、
[Animation Behavior / Trigger 合同](../design/animation/behavior-trigger-contract.md)、
[Animation 领域能力集成矩阵](../design/animation/capability-integration.md)。
评审检查见
[P2-M01 Animation 架构评审记录](../verification/reviews/p2-m01-animation-architecture-review.md)。
Draw2D 源码事实见
[Animation / Animator 分析](../reference/draw2d/figure/animation.md)。

依赖与启动门禁：

1. P2-G02 的统一 Graphics、FigurePresentation 与 glyph 准备链路已经完成；
2. M01-B 前必须关闭 P2-G01 的 Native surface visual repair 验收，避免在未稳定的
   presentation/damage surface 上叠加逐帧状态；纯 Core/Headless 的 M01-A 不受阻塞；
3. ADR-026、公开 API、错误枚举与外部 typed consumer 已完成人工评审；
4. `core.p2-m01-animation` 是保留的目标 suite；有可执行 contract test 与命令后才写入
   `verification/suites.toml`，不登记空门禁。

分阶段毕业证据：

1. **M01-A Clock / Timeline / optionality**：确定性单调时间、Tween/Keyframes、
   Parallel/Sequence/Stagger、取消/替换/retarget、三种模式及无活动动画零工作；
2. **M01-B Presentation / damage**：committed/presentation 分离、old/new visual
   envelope、temporary visual 生命周期、hidden/suspend/dispose 与 fault 清理；
3. **M01-C 领域消费者**：属性、布局结果、Connection route、Viewport、dash flow，
   以及沿 route arc length 移动的 pulse 到 endpoint decoration 的交接；
4. **M01-D 平台与视觉**：Headless 固定时间 replay、Native/Web 关键帧等价、
   DPI/nested transform/scroll/zoom、disabled/reduced-motion 与
   1/64/1,024 active track 工作量基线。

完成态必须证明关闭动画不改变 committed state、通知、命中、accessibility 或 history；
Core 默认不安装隐式 Trigger，无 active track 时不持续 tick、request redraw、扫描全树
或提交空帧。

M01-A 实现证据（2026-10-06）：

- Runtime-owned typed channel、AnimationId、Clock 与 scoped `AnimationMut` 已实现；
- Tween/Keyframes/Spring/Decay 与 Parallel/Sequence/Stagger/Delay/Repeat/Reverse 已实现；
- Enabled/Disabled/ReducedMotion、Replace/Ignore、cancel、atomic retarget、预算与有界
  terminal journal 已实现；
- 外部自定义 `AnimationValue` 不修改 Core 枚举即可接入；
- `core.p2-m01-animation` 14 项、Core 完整 crate 测试与 Clippy 通过；
- 详细证据见
  [P2-M01A 实现记录](../verification/reviews/p2-m01a-animation-evidence.md)。

M01-B 实现证据（2026-10-06）：

- Figure opacity/transform override、old/new surface envelope 与 immutable frame snapshot
  已接入既有 damage/recording/submission；
- Runtime-owned temporary visual、hidden/suspend/dispose/cancel/completion/fault 清理已实现；
- provider 非法 sample 结构化失败，provider panic 进入 Runtime fault boundary；
- 无 active presentation 时不构造 snapshot，active 路径只遍历 active subject；
- 详细证据见
  [P2-M01B 实现记录](../verification/reviews/p2-m01b-animation-presentation-evidence.md)。

M01-C 已完成：

- Figure bounds one-shot capture、source-final transition、stable-order stagger 已实现；
- empty/duplicate/foreign/disposed capture 与退化尺寸变换已结构化拒绝；
- Behavior/Trigger 已作为独立于 AnimationPlan 的稳定边界实现，覆盖 scope、stable
  fact coalescing、PlanFactory、ReducedMotion fallback、failure journal 与 dispose；
- layout transaction、route interpolation/crossfade、viewport transition、
  continuous dash 与 arc-length pulse/handoff 已实现；
- 进展证据见
  [P2-M01C 实现记录](../verification/reviews/p2-m01c-animation-consumer-evidence.md)。

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
- `FigureMut` 可原子修改 PointList stroke width / line join；
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
- FigureMut 支持 template、scale mode 与 alignment 的 typed mutation；
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
- FigureMut 支持 page 与 wrapping typed mutation；
- `text-app` 增加共享 Native/Web TextFlow 场景；`advanced-figures-app` 集中验证段落、
  fragment、SoftWrap、NoWrap、CJK 与 Truncate 可视结果；
- `cargo xtask verify core.p2-t01-text-flow` 通过。

### P2-T02: TextFlow interaction geometry

状态：`complete`

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

完成证据（2026-09-29）：

- `TextLayout` 通过 backend-neutral `TextInteractionProvider` 携带 interaction map；
- Parley cursor/selection/cluster 数据被收口为 Novadraw position、caret 与 quad；
- TextFlow 提供 paragraph-local UTF-8 position，并区分 fragment 内换行与 paragraph；
- position 绑定 layout revision，旧位置在 relayout 后结构化拒绝；
- Runtime query 完成 local/surface transform，查询不产生 validation 或 damage；
- `cargo xtask verify core.p2-t02-text-interaction` 通过。

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

状态：`complete`

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

P2-E02a 已完成 Viewer-scoped 单会话、typed feature/descriptor/policy plan、draft、
selection、preedit、TextFlow interaction geometry、feedback lifecycle、accept/cancel、
stale revision、source retire 与 CommandStack undo/redo。P2-E02b 已完成 host lease、
Native Winit IME、Web DOM input bridge、可运行 Web composition root 与默认
caret/selection/preedit 反馈。自动 suite 已通过；首帧背景、编辑文本居中与 caret
可见性修复已通过 Web Vello 自动视觉复核，以及
`doc/verification/manual/p2-direct-text-edit.md` 记录的 Native/Web 人工复验。

规范入口：

- `doc/reference/gef/direct-editing.md`
- `doc/design/editor/p2-direct-text-edit.md`
- `doc/verification/plans/p2-text-direct-edit.md`
- `doc/parity/gef/api-coverage.md`
