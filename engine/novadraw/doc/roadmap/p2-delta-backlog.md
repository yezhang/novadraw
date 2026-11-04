# P2 Delta Backlog

类型：`roadmap`

更新：2026-11-03。能力归属：Core、Editor、Inspector、backend、platform 和应用分别列项。
本页是 P2 的唯一执行队列，记录已确认增量、当前状态、阻塞和恢复条件；
运行时契约由 design／ADR 定义，验证依据由 verification 保存。

`complete` 表示该切片的契约、实现和验收闭合；`in_progress` 表示交付仍有缺项，
不表示正在并行执行。`design-draft` 尚未通过设计评审，`not_started` 尚未实现，
`deferred` 不进入当前实施主线。未完成平台证据和长期能力分母不能因文档归并而删除。
执行遵守[单链工作流](../verification/audit-remediation-workflow.md)。

Editor 的必要 Core 依赖须有 GEF → Draw2D → Core 证据。确认缺失后停止新增 Editor
能力，先提出独立 Core 增补建议；Core 验收通过后恢复。
详见[依赖门禁](../../CLAUDE.md#editor-缺失-core-依赖时停止新增)。

## 当前推进顺序

| owner／工作 | 当前状态 | 下一步与恢复条件 |
|---|---|---|
| Core 长文本 | FR-01 已实现并完成定向与匹配采样；[FR-02 内部归因](../verification/reviews/flow-assembly-attribution-2026-11-03.md)完成，[合同修订 1](../verification/reviews/adr-060-revision-1-response-2026-11-03.md)已回应六项评审，待复核。细诊断扰动及关闭路径尾延迟变化保留，整体延迟未达标，ADR-059 B1 扩张暂停 | 复核 [复用合同](../design/architecture/flow-assembly-reuse.md)与[验收矩阵](../verification/plans/flow-assembly-reuse.md)，ADR 未接受，生产复用 Execute 尚未启动；诊断关闭路径无回退仍需受控证据，恢复 B1 前明确端到端收益及推进顺序 |
| Editor E04 | 移动、创建／drop、单源 resize 已通过定向验证；E04-C／D 未关闭 | 补组合矩阵、完整事件成本和平台输入；校验成本整改提案待评审 |
| Editor E03 | 开放容器协议、自动行为及部分 Native／Web 输入已验证 | 补 Native 系统 DnD、跨显示器 DPI、完整 Native 组合和内部工作量 |
| 跨包 S09 | A／B／C 已完成，D 已实现并通过定向验证，整批未关闭 | 补 S09-D 最新 Wasm、成本及最终门禁；保留磁盘不足导致的失败证据 |
| 平台与发布准备 | P2-G01 surface、M01-D、GA-2／GA-3 仍未完成 | 按[目标调整计划](goal-alignment-adjustment-plan-2026-09-30.md)后置采证；发现 Core 正确性问题仍须重新 Triage |

不同 owner 的状态不是并行 Execute 指令。用户指定 Editor 范围时按 Editor 队列推进，
不转入 Core 或应用开发。后续 Editor 候选为键盘导航、多视图协调、框选策略和可选编辑支持，
接口尚未冻结。P2-L01、P2-T03 已完成，不再列为下一批。

## 已完成批次

| Delta | owner | 完成范围／依据 |
|---|---|---|
| P2-C01／C02 | Core | Connection decoration／locator／visual bounds、障碍感知路由 |
| P2-F01 | Core | ScalablePolygonFigure |
| P2-T01／T02 | Core | TextFlow 第一阶段、不可变交互几何 |
| P2-E02 | Editor／platform | Direct edit、draft、selection、IME、Native／Web bridge 与人工复验 |

以上批次于 2026-09-29 完成自动和 macOS 验收，E02 的视觉修正版于 2026-09-30
通过 Native／Web 人工复验。后续能力须登记独立 delta。

## 长期能力分母批次

对应[Draw2D 长期能力分母](../parity/draw2d/api-coverage.md#长期能力分母与处置)。
每项实施前核对 `api_semantics`，以可观察行为、失败语义和外部消费者证明等价。
公开 Render IR 必须被启用后端完整消费或结构化拒绝；只参考 Draw2D／GEF，不采用 Zest。

已登记切片的语义身份保留如下；具体行为由契约和账本负责，不随状态精简而删除：

| Delta | `api_semantics` |
|---|---|
| P2-F02 | `paint.protocol`、`figure.lifecycle`、`figure.box.client_area`、`figure.properties`、`event.input_listeners`、`accessibility.bridge`、`viewport.scroll_zoom`、`connection.figure`、`text.flow`、`widgets.basic` |
| P2-S01 | `figure.tree`、`layout.manager`、`notification.ancestor`、`notification.layout_update`、`validation.protocol`、`damage.repaint` |
| P2-G02 | `graphics.context`、`text.flow`、`text.interaction`、`paint.protocol`、`damage.repaint`、`render.backend_session` |
| P2-G01 | `graphics.context`、`geometry.primitives`、`paint.protocol`、`builtin.figures`、`connection.figure`、`damage.repaint`、`frame.preparation`、`render.backend_session` |
| P2-M01 | `animation.timeline`、`animation.presentation`、`frame.preparation`、`damage.repaint` |
| P2-C01 | `builtin.figures`、`connection.figure`、`connection.locator`；原 P2-R01 PointList visual bounds 合并于此 |
| P2-C02 | `connection.router` |
| P2-F01 | `builtin.figures` |
| P2-T01／T02 | `text.flow`／`text.interaction` |
| P2-R02 | `graphics.context` |
| P2-E02 | `direct_edit` |

| Delta | owner | 状态 | 剩余范围或完成依据 |
|---|---|---|---|
| P2-S01 | Core | `complete` | 原子 indexed + constraint child add，Builder／Runtime 失败、顺序和通知 |
| P2-G01 | Core／backend／platform | `in_progress` | Graphics 已贯通；Native surface 局部修复验收未闭合 |
| P2-G02 | Core／backend | `complete` | 统一测量／绘制、默认与外部 glyph 轮廓链路；不含未来自研 shader |
| P2-F02 | Core | `complete` | 开放 capability 与统一 mutation |
| P2-M01 | Core／platform | `in_progress` | A／B／C 已完成；D 缺 Native／Web 关键帧、组合及实际 runner 预算 |
| P2-L01 | Core | `complete` | 独立图模型、开放 solver、统一投影、52 个官方组合、Native 与最终门禁 |
| P2-C03 | Core | `not_started` | 跨 viewport Connection 的 nearest-common viewport、damage 和 hit-test |
| P2-T03 | Core | `complete` | ADR-042 structured flow、fragment style、inline／block 与六个 text 入口 |
| P2-W01 | Core | `deferred` | repeat scheduler、ButtonGroup／radio／checkbox／slider |
| P2-O01 | Core／backend | `deferred` | printing／export／ScaledGraphics 等价输出目标 |
| P2-A01 | Core／platform | `not_started` | Web action／focus 与 Native AT 双向闭环 |
| P2-LC01 | Core | `deferred` | 同 Runtime live unmount／mount 评估；不恢复跨 Runtime 活对象迁移 |

## API / 架构 Review 2026-10-09 批次

| Delta | owner | 状态 | 契约与完成证据 |
|---|---|---|---|
| P2-S02 | Core／Editor | `complete` | ADR-035 显式领域导入和 crate root；[完成记录](../verification/reviews/p2-s02-explicit-domain-imports-completion-2026-10-09.md) |
| P2-S03 | Core | `complete` | ADR-033 子 facade 与统一 Figure mutation；[完成记录](../verification/reviews/p2-s03-runtime-facade-completion-2026-10-10.md) |
| P2-S04 | Editor | `complete` | ADR-034 开放 Tool／Request／Policy／Handle；[完成记录](../verification/reviews/p2-s04-editor-open-protocol-completion-2026-10-10.md) |
| P2-S05 | Core | `complete` | 深层布局和文字 presentation；[完成记录](../verification/reviews/p2-s05-deep-layout-presentation-completion-2026-10-12.md) |
| P2-S06 | Editor | `complete` | ADR-037 与矩阵修订 2；[完成记录](../verification/reviews/p2-s06-failure-isolation-completion-2026-10-15.md) |
| P2-S07 | Core | `complete` | ADR-038 查询身份与失败；[完成核验](../verification/reviews/p2-s07-query-identity-completion-2026-10-16.md) |
| P2-S08 | Core | `complete` | ADR-056 的 A／B／C／D；[完成记录](../verification/reviews/p2-s08-d-completion-2026-10-29.md) |
| P2-S09 | Core／Editor／Inspector | `in_progress` | ADR-057 修订 3 已接受；A／B／C 已完成，D 定向通过、成本和最终门禁待补验 |

S06 永久限制：不承诺完整 synchronize 前缀、扩展副作用或成功模型命令回滚；
Web 异步 fault 退出保留 `S06-PL02`，Native surface 归 G01。
S08 关闭 R03／TC-03／API-F04／API-10；颜色解析修复不单独证明整批完成。
S09、R07 与 TC-10 保持未关闭，Inspector 容量 64／4096 的成本证据不代表引擎最大容量验收。

S09 当前依据：
[Animation](../verification/reviews/p2-s09-a-implementation-2026-10-30.md)、
[Observations／Inspector](../verification/reviews/p2-s09-b-implementation-2026-10-30.md)、
[Connections](../verification/reviews/p2-s09-c-implementation-2026-11-02.md)、
[Editor 扩展诊断](../verification/reviews/p2-s09-d-implementation-2026-11-02.md)。
设计轮次归入[评审结论](../verification/reviews/service-failure-review-status.md)。

### GEF 对等样例接入（2026-10-28 用户授权）

| 切片 | owner | 状态 | 最新范围与限制 |
|---|---|---|---|
| Scene backend 输出 | backend | `complete` | ADR-053；[契约、GPU 和成本](../verification/reviews/gef-scene-target-implementation-2026-10-28.md) |
| Masonry Widget／Xilem View | adapter | `deferred` | 独立接入保留；不承担 Shapes 外壳，不阻塞 Shapes；IME、AT、窗口外 capture 按消费者需求续验 |
| egui Shapes 宿主及 SH-01～10 | 应用 | `in_progress` | 创建／框选／drop、连接、多选、Outline、文件和保存点已实现；[自动证据](../verification/reviews/shapes-editor-implementation-2026-10-28.md)；完整 Native、跨显示器、capture、IME、AT 待验 |
| Logic 首个切片 | 应用 | `in_progress` | 半／全加器、四位加法器、Live／Ground、三种路由、相对折点及历史；[最新证据](../verification/reviews/gef-logic-routing-2026-10-30.md)；Circuit 层级、边界端口和完整平台输入未验收 |

### Editor／GEF 框架能力补充（2026-10-29 用户授权）

| Delta | 状态 | 最新验证与剩余条件 |
|---|---|---|
| P2-E03 | `in_progress` | ADR-055 修订 2，容器移动／创建／drop、跨布局和外部消费者已接入；Web DPR 1／2 的 46 个组合检查点和四组阶段成本完成；[组合续验](../verification/reviews/editor-container-combinations-2026-11-02.md)。Native 系统 DnD、跨显示器 DPI、完整 Native 组合和内部工作量未完成 |
| P2-E04 | `in_progress` | ADR-058 修订 2，开放注册、内置／第三方策略、实际移动／创建／resize 计划及失败诊断定向通过；[当前实施与成本](../verification/reviews/p2-e04-a-implementation-2026-11-02.md)。其余矩阵、完整事件成本、最终门禁和平台输入未完成 |

E04 已覆盖单源八方向及比例角 resize，居中和多源 resize 不在该切片。
六组无窗口采样证明 prepared 命中，但完整 owner 扫描、版本回调和分配仍昂贵；
10000 sibling 的完整 release 被默认 validation 上限拒绝，没有正式成功样本。
[校验成本整改提案](../design/editor/snapping-validation-cost-proposal.md)待评审，
不授权 Core 改动。workspace fmt／check 通过，最终 Clippy 被既有 Core 三项告警阻断。
依赖和设计结论见[开放吸附评审状态](../verification/reviews/open-snapping-review-status.md)。

## 官方 Draw2D 示例

范围限于 Canvas／LightweightSystem 的 Draw2D 能力，Java 仅供只读参考，
不建设 SWT 原生控件替代体系。目录与逐入口完成度以
[当前矩阵](../verification/plans/draw2d-official-examples-remaining-contracts.md)为准。
当前为 17 complete／8 in_progress，不从某次窗口通过推断全部完成。

owner-side Locator、组路由、ScrollBar、structured flow、图布局、共享场景、
宿主背景和 Tree 表现几何的通用切片已按 ADR-039～046 实施。
Tree T-1～T-4 已关闭，[实施复核](../verification/reviews/tree-layout-presentation-implementation-2026-10-26.md)
保留 surface 返回 Skipped 的限制。方向、低 miter、Cocoa 零宽和实际字体下划线
按 ADR-048～050 完成，[Core 增量验证](../verification/reviews/official-example-core-increment-2026-10-27.md)
不等于通用设备 hairline 已交付。

Flow／Grid／Toolbar、ScrollPane 和 Zoom 的 macOS 部分窗口交互已验收，
完整连续输入、跨平台、DPI 和呈现成本仍未闭合。
[布局窗口](../verification/reviews/official-layout-native-mouse-2026-10-29.md)、
[Scroll／Zoom 窗口](../verification/reviews/scroll-zoom-native-window-2026-10-29.md)和
[Zoom 最新文字指标对齐](../verification/reviews/zoom-typographic-alignment-2026-10-30.md)
保存具体范围；不再保留旧目录计数与逐次实施流水。

## Figure capability

### P2-F02: 开放的 Figure capability registry

状态：`complete`。契约为[ADR-027](../adr/adr-027-open-figure-capability-registry.md)和
[能力模型](../design/architecture/figure-capability-model.md)；
attach-time descriptor、typed discovery／mutation、外部 capability 和
`PresentationBinding<V>` 已验证。后续 ADR-033 取消 concrete facade 例外，
内置与扩展共用更新、失效、通知和错误通道。
[完成依据](../verification/reviews/p2-f02-figure-capability-completion-2026-10-08.md)。

## Figure 树 API

### P2-S01: 原子 indexed + constraint child add

状态：`complete`。Builder／Runtime 同一事务检查身份、位置和布局约束，
失败不发布部分树、revision 或通知。当前 API 以
[公共契约](../design/architecture/public-api-contract.md)为准。

## Graphics

### P2-G02: 统一测量/绘制 API 与 glyph 准备链路

状态：`complete`。[ADR-025](../adr/adr-025-unified-graphics-and-glyph-preparation.md)
定义字体指标、测量、图文绘制和布局后轮廓边界；
[完成证据](../verification/reviews/p2-g02-text-graphics-evidence.md)覆盖默认和可替换链路。
后端 GPU 格式和未来自研字体算法不进入 Core 布局结果或当前交付。

### P2-G01: Graphics 扩展

状态：`in_progress`。custom stroke、path clip、gradient、glyph 和 feedback 已贯通，
Native 离屏与 WebGPU 像素通过；Native surface 局部 retained repair 尚未闭合。
完整重绘不能代替 surface 局部修复。
[契约](../design/rendering/p2-g01-graphics-extension.md)、
[验证与限制](../verification/reviews/p2-g01-graphics-evidence.md)。

## Animation

### P2-M01: Animation Timeline 与 Presentation Plane

状态：`in_progress`。A Clock／Timeline、B Presentation／Damage、C 领域消费者已完成；
D 的 Native／Web 关键帧、组合场景和实际 runner 预算待验。
不重复推进 A／B／C，Tree T-1～T-4 完成不关闭 M01-D。
[动画契约](../design/animation/00-index.md)与[ADR-026](../adr/adr-026-animation-and-presentation-plane.md)。

## Connection

### P2-C01: Decoration、Endpoint Locator 与 PointList visual bounds

状态：`complete`。语义范围由[路由专题](../design/architecture/connection-routing.md)
和[连接路由](../design/architecture/connection-routing.md)定义。

### P2-C02: 障碍感知 Shortest-path Router

状态：`complete`。采用 Draw2D 障碍感知路由语义；后续组求解按
[连接组求解](../design/architecture/group-route-solving.md)细化。高级 self-loop 产品策略另列。

## Figures

### P2-F01: ScalablePolygonFigure

状态：`complete`。受检模板、bounds 映射、实际描边包络和命中共用 Core Figure 协议；
契约见[Shape／Border](../design/architecture/reusable-shape-border.md)。

## Text

### P2-T01: TextFlow 第一阶段

状态：`complete`。后续 structured flow 由 ADR-042／P2-T03 承接，
不把性能优化或海量渐进模式计入此阶段。[文字规范](../design/architecture/text-layout.md)。

### P2-T02: TextFlow interaction geometry

状态：`complete`。不可变来源、caret、selection 和命中几何由同一布局快照提供，
不向 Core 引入 mutable editor state。
[完成记录](../verification/reviews/p2-text-interaction-2026-09-29.md)。

## Rendering

### P2-R02: Image source rectangle

状态：`complete`。公开 source rectangle、完整 image resource 与受检绘制边界已收口，
以[公共 API 契约](../design/architecture/public-api-contract.md)为准。

## Developer tooling

### P2-D01: FigureInspector 开发期可观测性

状态：`deferred`。后续 Inspector 能力不由早期 FigureInspector 候选自动授权；
当前已有能力见[诊断规范](../design/architecture/figure-inspector.md)，
生命周期整改见 S09。Studio／offscreen thumbnail 的产品方向不自动进入实施。

## Editor

### P2-E01: 高级 Self-loop 路由策略

状态：`deferred`。GEF 基础合同保留单一 ConnectionPart、独立端点、身份和历史原子性；
开口方向、lane、障碍避让及序列化属于后续产品策略。既有显式折点 demo 不定义 parity。

### P2-E02: Direct text edit 与 IME

状态：`complete`。Editor 持有单会话 draft／selection／preedit，accept 生成一次 Command，
cancel 不写历史；Native／Web 输入 bridge 和人工复验已完成。
[设计](../design/editor/p2-direct-text-edit.md)、
[矩阵](../verification/plans/p2-text-direct-edit.md)。

## P2 长文本精确布局优化

能力归属：Core Text／Flow／Runtime；backend 负责字形覆盖和合成，
platform 负责真实输入与呈现，研究应用只提供消费者。
当前设计由[总方案](../design/architecture/long-text-performance-proposal.md)、
[精确 Flow 复用](../design/architecture/exact-flow-layout-reuse.md)和
[可选阶段文字协议](../design/architecture/staged-text-width-reuse.md)分别维护。

| 切片 | 状态 | 已实现与剩余范围 |
|---|---|---|
| LT-00 | `in_progress` | release 驱动、请求／阶段诊断、冻结和匹配采样已提供；first-ready、生产 GPU／present 仍缺证据 |
| LT-01 | `in_progress` | 共享布局／snapshot／glyph range、source 索引和依赖去重已实现；其他复制、缓存留存和完整成本未验完 |
| LT-02 | `in_progress` | 精确请求／段落复用、失效反例和受约束测量已验证；完整贡献组装重复及 16 ms 更新目标未关闭 |
| LT-03 | `in_progress` | 已有 Full／Partial GPU 对照和共享 IR；默认原生输出仍为追加后 51,233 glyph，4 ms 查询／IR 与生产覆盖证书未通过 |
| LT-04 | `in_progress` | ADR-059 A 已完成，B 存储、复制／对齐／断行准入已有实现；B1 扩张暂停，B2～B4、C／D 和默认启用未完成 |
| LT-05～09 | `not_started` | worker、渐进覆盖、紧凑来源和后端／平台后续范围独立评审，未授权整批实施 |

FR-01 在同一不可变借用内复用准备输入，每次仍重读外部 provider revision，
不缓存 contributor／handler／border 回调结果。每版本 500 个匹配 fresh Runtime 样本中，
resize 批次验证 P95 为 2.458 → 1.537 ms，整体 resize P95 为 37.236 → 36.439 ms，
P99 上升；13 次贡献组装及 520 请求／514 命中的分母未变。
支持局部复用有效，不支持稳定尾延迟已改善。

FR-02 已完成组装内部归因、30 项定向回归和 3200 个正式样本；细诊断的阶段占比
不能当作无诊断生产成本，关闭路径尾延迟变化尚未解释。ADR-060 修订 1 已回应
FR02-DR01～DR06，下一步复核声明证明发布链、共享内容、留存容量与阶段门禁；
合同未接受，生产矩阵未执行，不删布局反馈轮次或跳过外部重读。
独立 TextFlowLarge 应用已实现，自动 CPU／Native
两轮回放通过；人工交互、DPI、IME、AT 和屏幕呈现未验收。
最新证据和测量限制统一见[文字优化验证](../verification/performance/text-optimization-validation.md)；
阶段边界见[文字阶段验证状态](../verification/reviews/text-stages-status.md)。
产物体积只按用户明确请求比较，不作为日常功能门禁。

## P2-F03：逐角圆角与矩形分边描边

状态：`complete`。Core Shape／Border；[逐角与分边描边](../design/architecture/independent-corners-and-edge-borders.md)、
[实现验证](../verification/reviews/independent-shape-corners-2026-10-27.md)和
[自由端续验](../verification/reviews/shape-free-end-butt-2026-10-27.md)。
逐角和逐边使用统一绘制、包络与外部消费者合同。
