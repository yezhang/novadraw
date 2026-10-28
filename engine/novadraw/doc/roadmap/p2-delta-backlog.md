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

- `design-draft`：已形成 ADR 或 normative design 草案，尚未批准进入实施；
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
| 4 | P2-M01 | Animation Timeline 与 Presentation Plane | `in_progress` | P2-G02、P2-F02 已完成；Native surface 视觉毕业依赖 P2-G01 present 证据 | A/B/C 已完成；D 待 Native/Web 关键帧、组合场景与实际 runner 预算 |
| 5 | P2-L01 | Directed/Compound graph layout adapter | `complete` | GA-4 扩展边界、ADR-043 已实施 | 独立模型、默认/外部算法、统一投影、52 个官方组合与 Native 验收、最终门禁 |
| 6 | P2-C03 | 跨 viewport Connection 可见性与 clipping provider | `not_started` | M8、M9、P2-C02 | nearest-common viewport、damage、hit-test |
| 7 | P2-T03 | fragment style 与剩余 inline/block flow | `complete` | P2-T01/T02、ADR-042 | 测量、paint、interaction 使用同一快照；六个官方 text 入口验收 |
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

## API / 架构 Review 2026-10-09 批次

2026-10-26 按用户要求先分析、再补官方样例能力，形成
[能力补充方案](../design/architecture/official-example-capability-increment-2026-10-26.md)。
现有 K02/K08 契约内先实施 HelloWorld resize、UML DogEar 与 ShapeStyles 几何修正；
验证结果见 [实施记录](../verification/reviews/official-example-capability-increment-2026-10-26.md)。
2026-10-27 用户批准方向上下文、低 miter、Cocoa 零宽映射和通用下划线。
方向切片按 [ADR-048](../adr/adr-048-inherited-layout-direction.md)已通过定向
测试与 Core 严格 Clippy，含方向/resize 的 GPU Partial/Full 比较；
K08 低 miter 与 Cocoa 零宽映射已按 [ADR-049](../adr/adr-049-low-miter-and-cocoa-sample-profile.md)
接入两条后端并通过 GPU Bevel 对比；通用下划线按
[ADR-050](../adr/adr-050-prepared-text-underlines.md)完成实际字体/run 准备、公开绘制
和局部擦除回归。本批最终门禁与产物成本已完成：
workspace 1,516 项测试通过，严格 Clippy、Native/Wasm API、三本书和 GPU 复核通过。
完整记录见 [本批 Core 增量](../verification/reviews/official-example-core-increment-2026-10-27.md)；
下划线准备 P95 为 0.207–0.349 ms；同配置 Core 探针启用后的 Wasm gzip 增加 18,690 字节。
通用设备 hairline 保持独立 Design/Gate。
SWT Cocoa/GTK 的零宽处理不等于统一设备 hairline。
本批不变更入口完成计数，不覆盖此前快照，也不启动第二个 Core 实施切片。

2026-10-26 [25 入口功能复核](../verification/reviews/draw2d-official-examples-functional-audit-2026-10-26.md)
发现 BidiBlockExample 的右侧 Canvas 镜像遗漏，入口撤回 complete，当前目录为
16 complete、8 in_progress、1 blocked。此发现回到 K05/K07 的契约核验，
不撤销已通过的 structured flow 和 shared range 能力证据，不新增并行执行批次。
Caret 选择绘制与边界、HelloWorld resize 和跨入口字体替代列为待核验；
原始完成记录保留其当时快照。

2026-10-19 按用户批准的
[官方示例 Core 计划](../verification/plans/draw2d-official-examples-core-plan.md)
按用户最新要求只推进 25 个入口中的 Canvas / LightweightSystem 能力。
Java 仅作只读语义参考，撤销 Java 编写/运行探针及其生成基线、验证门禁；
撤销 SWT 原生控件库、完整面板对等和真实打印作业作为本计划目标。
临时示例控制工具可以保留，但不作为 Core 演进方向或完成条件。
K01 的 subtree/回执、资源准入、contents 替换、Connection binding 和消费者保留；
K02 的 Grid span/grab 与 Flow/Toolbar 测量修复保留，改用独立 Rust 契约验证。
撤销及 Rust 回归已完成。当前 K03 首批修复普通 Locator child 的旧 bounds 尺寸，
改为受检 preferred measurement，非法测量拒绝路由批次并可恢复；
2026-10-21 补齐普通 Locator child 的 layout generation 失效跟踪，尺寸覆盖/重置与
UML 连接文字修改无需端点移动即可重新定位；测量失败可自动恢复并保持稳定。
owner-side 定位已按用户批准的 [ADR-039](../adr/adr-039-owner-aware-connection-locator.md)
接入统一 LocatorInput/ChildLocatorPlacement、tracked owner 边界与官方 UML，最终门禁通过。
保留 EndpointLocator 既有切线偏移；ConnectionEndpointLocator 实现 owner 四侧与 child-size 定位。
Obstruction 已按用户批准的 [ADR-040](../adr/adr-040-group-route-solving.md)
实现完整组求解、visibility/shared corner 与 Canvas 生命周期。
外部 solver、失败恢复、独立障碍不变量、真实输入与四阶段 Native 截图已验证；
最终 workspace 门禁通过（1,155 项测试通过、1 项忽略）。K03 的其他示例验收继续保留。
2026-10-21 按用户要求实施 [ADR-041](../adr/adr-041-composable-scrollbar.md)：
独立 ScrollBar 的五个普通 Figure 部件、可替换 RangeModel/thumb、横纵布局、
250ms/40ms 连发及取消共用公开 FigureActivity；Zoom 删除私有组件，
ScrollPane 复用同一实现。九项外部契约、26 项 viewport 回归、Zoom 两项交互
与 Native 截图通过；最终 workspace fmt、Clippy 和 test 通过
（1,164 项通过、1 项忽略）。
K04 通用切片完成后回到 K05 的只读源码契约与设计，Path/MultiLineLabel
仍依赖文字流和 Graphics 增量，不关闭 K04 整批。K05 先形成可评审契约，
重大公共 API 变更依仓库规则评审后实施。
K05 已按用户批准进入 [ADR-042](../adr/adr-042-structured-flow-layout.md) 实施，
明确开放贡献协议、styled request、同 revision fragment 快照、
FlowBorder 和真实 Figure placement；现已完成。
styled 引擎、通用原子 preparation、开放 flow registry、结构化节点与来源快照
已有独立消费者证据。相邻/嵌套 margin、跨字号 baseline、RTL 对齐、fragment
命中、真实 Figure 二次测量、外部 fragment border 与字体/来源失效已覆盖；
普通 page/block Border、外部 root 的公开布局服务、跨段落首尾边框与独立替代
文字引擎已补齐。内嵌反馈失败闭环、四侧边框度量与六个 text 入口均已实现，
Native 六个截图与 56 项示例测试通过；最终 workspace fmt、check、Clippy 和 test
通过（1,216 项通过、1 项忽略）。后续回到 K06、K07/K08 的依赖顺序。
2026-10-22 Path 首个消费者切片已接入，状态改为 in_progress。通用 BorderLayout
修复显式 Center 预留不存在的边带；Path 的真实 ScrollBar 输入、旋转路径、
双绘制选字与 Native 最小/最大截图已覆盖。SchemeBorder 与字体指标下划线
仍留在 K08；不关闭 K04/K05/K08 整批。接下来进入 P2-L01 Design，
只读 Draw2D 图算法与官方 fixture，形成独立模型、求解器与 Figure 投影契约。
[ADR-043](../adr/adr-043-directed-compound-graph-layout.md) 修订 1 已获用户批准，进入实施；
默认/外部算法共用 typed registry，16+10 个 fixture 的双方向组合共 52 项。
按纯请求/结果与 registry → Directed → Compound → Figure/Connection adapter →
两个官方场景推进；纯请求/受检结果与开放 registry 第一切片已实现，
18 项外部非分层协议测试通过。Directed 阶段已实现确定性消环、加权 rank、
virtual nodes、排序/放置与有限走廊路由；16 fixture 的双方向 32 组合、
穷举加权目标及独立端点/排序/证据验证通过。Compound 的包含域消环、head/tail、
bridge、连续区块与共享 separator 已实现，10 fixture 的双方向 20 组合、
小数几何与嵌套/反馈端点反例通过。统一 Figure/Connection adapter 已完成 Core 接入，
官方场景与整体交付随后完成，最终证据见本段末尾。
adapter 实施前的静态核对确认候选 bounds 可读，但候选领域数据/身份、prepared Figure 的
约束重测量和整组必需路由发布缺少完整契约。随后形成
[候选快照查询提案](../design/architecture/graph-projection-preparation.md)，
已按[增量评审意见](../verification/reviews/graph-projection-preparation-review-2026-10-22.md)
完成提案修订 2，全部采纳 GP01–GP04，补齐 identity 保留条件、结构环检测、
测量优先级与结构化错误。用户随后授权按 ADR-043 和增量方案推进 Core 编码，
修订 2 进入实施，先推进候选快照查询与必需组发布，再推进测量/source 前缀和 adapter。
Core 约束测量协议已落地：`measure_scene`/`measure_figure`、独立 preferred/minimum
来源选择、单次 Border 组合、结构化错误和测量重入检查。独立外部消费者覆盖
新约束、override/LayoutManager 优先级、不支持/非法尺寸/baseline、非对称边距、
缩放、有限输入溢出及错误 source 链。后续 Core 批次已接入 Runtime 快照身份、
稳定结果版本、源/派生 bounds 分离、路由快照依赖及必需组发布检查。
快照计算观察已参与路由环检测，必需组准入拒绝读取路由及 Locator 子树输出，
受检查询与缓存复用校验观察。必需组连接的成功记录支持同源 resolved 恢复，
改源/绑定失败不能恢复旧 resolution，首帧失败保留重试入口。
LayoutManager 子查询已进入同一受约束测量会话；preferred/minimum 单独分派，
缓存命中保留传递观察，嵌套错误保留叶节点及原始原因。
受约束测量源依赖与已提交 metrics 依赖已分开；同源失败后重试保留
快照 identity、resolved metadata 和 route calculation count。
快照源输入已改为逐项比对，批次写入前复核源与计算观察。
字体替换或移除立即撤销旧快照的路由查询权限，重新准备后更新 identity；
`p2_prepared_scene_routing_contract` 覆盖资源变更后的显式路由拒绝与恢复。
挂载时的 capability/显式依赖和 component effect 绑定的资源状态参与源前缀与测量源依赖；
图片完成或移除后，失败恢复保留资源变化并拒绝旧 resolution。
无关图片更新不替换图快照；测量、路由及 Label 定向回归共 42 项通过。
同批候选依赖校验、共享预算与 FlowPage/TextFlow 的约束测量已完成 Core 接入。
统一 `GraphProjection` producer、完整 flat binding、候选宽度重测量、
attachment Anchor 与整组完整路径输出已实现；外部非分层算法走同一 adapter。
纯结果缓存核对精确源输入与最终测量，Runtime 保持唯一发布权威。
宿主可配置每次完整稳定化的有限预算，图求解与测量继续共享计数器。
官方 Directed/Compound 场景已接入，52 个组合通过真实 Label、完整路径和 binding
检查；宿主菜单真实点击覆盖 fixture/方向替换。Core graph 容器的受约束测量复用纯
producer，tracked viewport 测量/滚动保持图 identity 和路由计算次数。
Native 代表性截图已生成并检查。普通布局约束测量、共享不可变源前缀及
Editor 文字 offset 更新后的稳定查询已补齐，默认预算保持不变。
最终 workspace fmt、check、严格 Clippy、test 通过（1,321 项通过、1 项忽略、
0 项失败）；五个应用探针、Web backend 编译和公开 API 边界门禁均通过。
P2-L01 为 `complete`，证据见[实施记录](../verification/evidence/adr043/implementation.md)。
原 ADR 修订 1 与增量修订 2 保持有效；K06 的 tree contour 与动画不随图布局关闭。
Canvas 内的 size/reset、文字换行和布局边界继续保留。Draw2D Figure ScrollBar、TextFlow、
MultiLineLabel 等仍在范围内；平台只承担 Canvas 输入、DPI/font、surface、背景源
和生命周期适配。K00/H04 的清单与场景验收按 Canvas 范围维护，S08/S09 未启动。

2026-10-23 官方示例继续消费现有 Core：ConnectionAndAnchor 改为 owner-aware
端点标签并使用真实标题测量；PageNode 根据选中状态绘制蓝色渐变；PrintExample
增加完整 Canvas 组合和图形输出消费者。K08 普通扩展增加 SchemeBorder 与受检字体
下划线指标，Path/Zoom 滑块标签接入同一 Border 契约；K09 增加原始 ICO 解码；
Zoom 使用原始 SVG，经已有解码器在固有尺寸栅格化。
契约见[本批增量](../design/architecture/official-example-border-output-increment.md)。
K07 的共享树投影和透明 Canvas 背景输入为下一批 Design；K06 动画、K08 AA 模式、
SVG 随缩放/DPI 重栅格化与混合字体装饰仍保留未完成，不扩张本批完成范围。
ConnectionAndAnchor、Path、SimpleTree 已收口，入口状态为 14 complete、8 in_progress、
3 blocked；本批[交付证据](../verification/evidence/official-examples-2026-10-23/implementation.md)
保留真实 Native 截图与验证边界。

2026-10-23 后续复核形成
[11 个未收口入口的工作清单](../verification/plans/draw2d-official-examples-next-slices-2026-10-23.md)。
Grid 的 Native Canvas 用普通 MarginBorder/FillLayout 跟随逻辑视口 resize；
跨列 grab/fill Shape 的缩小、放大、恢复及 surface 原点通过真实入口回归。
Toolbar 的方向切换、stretch、收缩和尺寸恢复通过真实 Shape geometry 回归。
ScrollPane 的五部件 ScrollBar 已实现，清除将其列为缺失 Core 能力的过时描述。
本批不关闭入口或 K02；当前仍为 14 complete、8 in_progress、3 blocked。
K07 进入 `design-draft`：
[共享视图与宿主背景草案](../design/architecture/shared-scene-view-background-proposal.md)
提出不可变源快照、destination 资源导入与独立宿主背景帧输入，待用户评审，
没有开始共享视图实现；K06 动画和 K08 AA/SVG 保留原状态。
本批执行证据见
[后续实施记录](../verification/evidence/official-examples-followup-2026-10-23/implementation.md)。

K07 草案已按
[2026-10-23 评审记录](../verification/reviews/k07-shared-scene-view-background-review-2026-10-23.md)
形成修订 2，逐项采纳 K07-R01 至 K07-R08，处理说明见
[修订响应](../verification/reviews/k07-shared-scene-view-background-revision-2026-10-23.md)。
共享场景捕获与宿主背景统一合成作为两个独立决策，分别固化 ADR 和验收；
当前仍为 `design-draft`、待复核，未获实施批准，不改变其他批次状态。

K07 [修订 2 复核](../verification/reviews/k07-shared-scene-view-background-revision-2-review-2026-10-23.md)
暂不通过：循环依赖按实际捕获内容统计，尚未覆盖普通几何与 derived commit 的写入前保证。
原方案已形成修订 3，改为捕获根整棵子树的保守结构依赖，由 Runtime 统一提交器强制
准入并在提交点重验 graph revision；同时补齐捕获根的最近值 alpha 继承。
处理见 [修订 3 响应](../verification/reviews/k07-shared-scene-view-background-revision-3-response-2026-10-23.md)。
当前仍为 `design-draft`、待复核，未获实施批准，保留原复核记录与其他批次状态。

2026-10-23 [修订 3 设计复核通过](../verification/reviews/k07-shared-scene-view-background-revision-3-review-2026-10-23.md)，
用户明确批准开始实施。已固化 [ADR-044](../adr/adr-044-shared-scene-capture-and-view.md)
和 [ADR-045](../adr/adr-045-host-background-submission.md)；K07 从 `design-draft` 转为
`in_progress`。下表是本批唯一执行切片，已批准契约不因切片安排缩减：

| 顺序 | 切片 | 当前状态 | 最低退出证据 |
|---|---|---|---|
| K07-1 | source-local capture、完整资源闭包、身份/配置、订阅有界更新与终止交接 | `complete` | [捕获证据](../verification/reviews/k07-1-scene-capture-evidence-2026-10-23.md)：13 个外部契约测试、397 个库级测试；alpha/clip/精确资源、暂停/在途/错误隔离、独立终止和 API 28/28 已验证 |
| K07-2 | binding group、保守结构依赖与 Runtime 强制提交准入 | `complete` | [共享组证据](../verification/reviews/k07-2-scene-group-evidence-2026-10-23.md)：9 个外部反例、5 个内部准入反例与 28 个子树回归；交错候选、hidden/rect 外、callback/typed reparent、完整替换和耗尽批次已验证，API 28/28 |
| K07-3 | destination 多版本导入、SceneView Figure 与测量/完整 damage | `complete` | [目标视图证据](../verification/reviews/k07-3-scene-view-evidence-2026-10-23.md)：11 个外部契约、13 个定向库级检查；完整库级 407 项通过；font/image 共存、Border/提示、stale/conflict、reset/Retry、释放及 API 28/28 |
| K07-4 | Thumbnail 官方 Canvas 与真实双视图消费者 | `complete` | [消费者证据](../verification/reviews/k07-4-thumbnail-evidence-2026-10-23.md)：独立活源 Runtime、真实目标输入、两个只读视图、resize 不重新捕获、终止清理及三张 Native 截图 |
| K07-5 | 宿主背景候选、统一输入冻结与恢复 | `complete` | [背景证据](../verification/reviews/k07-5-host-background-evidence-2026-10-23.md)：7 项外部契约、身份耗尽与 Native GPU 像素通过；版本拒绝、移除/迟到、lease、Retry/reset、明确清屏和后端拒绝，API 28/28 |
| K07-6 | InheritBackground 官方 Canvas 与平台合成验收 | `complete` | [双 Canvas 证据](../verification/reviews/k07-6-inherit-background-evidence-2026-10-23.md)：真实双 Runtime、宿主 typed provider、opaque/transparent、内容滚动与 Canvas 移动、背景移除、DPI、暂停恢复及 session repair 的 Native 像素通过 |

K07 六个批准切片已完成，Thumbnail 与 InheritBackground 的 macOS Native 消费已验收。
Web 背景像素、Windows/Linux runner 和跨显示器系统 DPI 仍未验收；
其他 K06/K08 与 S08/S09 的状态保持不变。

2026-10-23 返回 K02/H04 已批准的布局消费者契约，Flow/Toolbar/Grid
[后续核验切片](../verification/reviews/k02-layout-consumers-evidence-2026-10-23.md)已完成：
Flow 的官方 majorSpacing 映射到行间距、minorSpacing 映射到元素间距；
Flow/Toolbar 通过普通 MarginBorder/FillLayout 随 Host 视口布局；验证实际 egui 输入、
方向/双轴 alignment/stretch/尺寸恢复及连续 Native GPU 提交后的旧像素清理。
真实面板、零可用空间恢复、Native GPU 擦除、两张窗口截图和最终 workspace 门禁通过。
本批不新增 Core API；连续 Native 窗口 resize、文字 minimum 与 retained-partial
像素仍待验收，不关闭三项完整入口或 K02 批次。

2026-10-23 K08 SVG 密度切片按
[纯解码契约](../design/rendering/svg-raster-density.md)实现：
保留解析源，分配前校验像素预算，更新密度时自然尺寸不变；Zoom 通过普通资源 token
替换三个共享图标，同密度不重复更新。Native GPU 替换与恢复像素已验证，
见 [SVG 证据](../verification/reviews/k08-svg-density-evidence-2026-10-23.md)。
同时修复 K06 Disabled/ReducedMotion 在零尺寸折叠布局中提前构造仿射变换的错误。
Tree branch/lines/viewport 动画、逐 Shape AA 和混合字体装饰仍未完成；
Zoom 的完整入口和跨显示器系统 DPI 验收不随此切片关闭。
下一步进入 [Tree 表现几何](../design/animation/tree-layout-presentation-proposal.md)
的 Design/Gate；现有 affine 过渡不能表达上游真实 bounds 重放。
[逐图形 AA](../design/rendering/shape-antialias-proposal.md)也已形成待评审方案，
先评审后端可行性切片，不将 hairline 或低 miter 隐式并入 AA。
两份方案均未进入 Execute。

2026-10-26 逐图形 AA 的独立后端实验已通过，证据见
[设备像素游程实验](../verification/reviews/shape-aa-feasibility-2026-10-26.md)。
[方案修订 2](../design/rendering/shape-antialias-proposal.md)改为受预算约束的设备
像素中心采样与整数游程路径，复用现有 Vello 合成；不引入依赖 fork。
AA-0 只关闭实验，AA-1–AA-4 仍为 Design/Gate、待批准，Core 与生产 backend
未实施；ShapeStyles 仍为 blocked。Hairline、低 miter 和混合字体装饰保持独立。

2026-10-26 用户批准按方案修订 2 实施，接受
[ADR-047](../adr/adr-047-per-operation-antialias.md)。AA-1 进入 `in_progress`，
AA-2–AA-4 为 `pending`；按顺序推进，每次最多一个实施切片。
ShapeStyles 的完整入口状态待实际契约与像素证据裁定，hairline 与低 miter 保留原边界。

AA-1 Core 契约与外部消费者验证已完成，见
[实施记录](../verification/reviews/per-operation-antialias-implementation-2026-10-26.md)。
Native/Wasm Core check、严格 lib Clippy、workspace check 及 49 项定向测试通过；
公共快照与全量门禁随完整增量退出核验。随后按单链推进 AA-2–AA-4，
截至下述实施复核的状态为：

| 切片 | 当前状态 | 退出内容 |
|---|---|---|
| AA-1 | `complete` | 独立提示、命令冻结、状态恢复、外部消费者与两级拒绝 |
| AA-2 | `complete` | 预算扫描线、完整缓存身份、描边/文字准备与提交前检查 |
| AA-3 | `complete` | 默认回归、混合像素、DPI/恢复、release 性能与体积 |
| AA-4 | `complete` | 原始样式矩阵消费者、受支持输入与清理恢复 |

2026-10-26 AA-1–AA-4 的实现及既定自动验证切片完成，最终 API/workspace 门禁
通过；workspace 测试通过 1,501 项，8 项忽略。40 个 GPU 矩阵通过，
ShapeStyles 改为 `in_progress`；
窗口呈现、浏览器像素及完整上游几何仍待验收。目录当前为
16 complete、9 in_progress、0 blocked。持续变换 DPI 2 的帧等待 P95
实测为 19.7–28.9 ms，不保证 60 fps；成本与产物增量见上述实施记录。
Hairline、低 miter 与混合字体装饰未实施，不将这些输入夹值后登记为对等。

2026-10-24 Tree 方案已按
[首轮评审 R01–R10](../verification/reviews/tree-layout-presentation-review-2026-10-24.md)
完成修订 2，逐项响应在方案正文。当前仍为 Design/Gate、待复核，未批准实施；
T-1–T-4 与其他批次状态不因文档修订改变。

2026-10-24 [修订 2 复核](../verification/reviews/tree-layout-presentation-revision-2-review-2026-10-24.md)
暂不通过，F01/F02 要求明确采样时刻与预算失败归属。当前方案已形成修订 3，
以最后成功 Runtime 采样的完整表现版本作为锚点，按准入预留容量并区分单组违约
和共享不可用；逐项响应与退出证据见方案正文。仍为 Design/Gate、待复核，未批准实施。

2026-10-24 [修订 3 设计复核通过](../verification/reviews/tree-layout-presentation-revision-3-review-2026-10-24.md)，
用户明确批准落地，接受 [ADR-046](../adr/adr-046-sampled-geometry-and-animation-reservations.md)。
K06 Tree 几何按下表单链进入 Execute，不缩减批准契约；逐 Shape AA 与混合字体装饰
不在本次 Tree 实施范围。

| 顺序 | 切片 | 当前状态 | 最低退出证据 |
|---|---|---|---|
| T-1 | typed 真实几何、S/E 物化、capture/retarget、派生依赖与容量预留 | `complete` | 完整读取与外部 typed 扩展、S/E/留存/净替换/失败回归、公开 API 和全量门禁通过 |
| T-2 | Tree 静态/动画同源端点、冻结内容与资源版本 | `complete` | Normal/Hanging 静态端点与动画同源；字形和线条回归通过 |
| T-3 | 显式 Viewport、收拢与兄弟重排、直接输入接管 | `complete` | 当前 S 反向、后代中心收拢、wheel/scrollbar/公开写入、旧 owner 冲突通过 |
| T-4 | 默认关闭的真实面板与完整 Native 验收 | `complete` | Native GPU 中间帧与 partial/full 擦除、模式最终像素、Retry/reset/释放通过；surface present 独立保留 |

T-1 已完成基础 Motion 缓存的同时间幂等、整组样本预检后发布，以及活动轨道预留、
净容量替换、独立 staging 和配置下调拒绝。公开 `sample_version()` 区分 Unanchored
与最后接受的时间，并标识同时间的活动集合变化。定向测试
`p2_animation_sample_reservation_contract` 的 4 项、原有 `p2_animation_contract` 的
29 项与 `p2_animation_transition_contract` 的 15 项通过。
2026-10-25 基础切片通过 workspace fmt/check、严格 Clippy 和完整测试：
1,420 项通过、0 失败、6 项按原有配置忽略；文档门禁通过 373 份 Markdown。
真实 bounds、完整 E、几何 provider、节点/输出工作上限和快照 lease 尚未完成；
这些基础证据不代替 T-1 退出验证，状态继续为 `in_progress`。

2026-10-26 T-1 新增独立 `LayoutBounds`、typed 几何 descriptor、缓存表现几何与
Runtime capture/真实 bounds 事务入口，保留旧 affine 过渡。派生协议先发布几何，
再通过受检 builder 生成 owned 线条；循环、未声明查询、重复输出和内容 owner 冲突
在预检拒绝。输出越限保留 provider 身份与声明/实际计数，清除所属整组；共享分配
失败返回准备错误，不驱逐合法组。静态重建不再次运行失败 provider，并复用无关组
已成功的派生结果。route/dash 原有家族组合回归保持通过。

定向证据：`p2_animation_geometry_contract` 17 项、
`p2_animation_sample_reservation_contract` 4 项、原有 animation 29 项、
behavior 7 项与 transition 15 项均通过；几何与预留目标的严格 Clippy 通过。
仍需完成快照/capture/提交与资源的真实留存容量、完整冻结绘制输入以及实际工作量
证明。当前容量实现包含多维准入与线条输出声明，但不能替代这些剩余条件，
T-1 继续为 `in_progress`，T-2 至 T-4 保持 `pending`。

T-1 后续增量为 owned capture 增加独立 `capture_bytes` 配额和随 capture 释放的
留存凭证。共享同一几何对象的多个 capture 也分别承担留存责任；额度不足拒绝新
capture，配置下调低于存活 capture 时原子拒绝。当前字节统计覆盖几何集合、子顺序、
源依赖集合与字符串，不包含 opaque drawing、边框内部数据及字体/图像 payload；
提交包和恢复前缀的独立留存记账仍待完成，不能据此关闭 F02。
静态 capture 也执行节点、边与集合存储预检；祖先 child order 在克隆前检查边上限。
几何数据由不可变 Arc 共享，无关源 epoch 变化只替换版本包装，不再深拷贝几何表；
最大根归并缓存父链判断，避免重叠目标重复扫描相同祖先。样式、opaque、裁剪策略和
可取得的边框对象从冻结输入读取；任意 Figure 自绘及 visual bounds 的完整冻结
仍属 T-1 剩余条件。
本增量定向回归为几何 19 项、采样预留 4 项、原 affine transition 15 项通过；
上述两个新协议目标与 Core lib 的严格 Clippy、373 份 Markdown 文档门禁及
`git diff --check` 通过。这些结果未覆盖剩余的 payload/资源留存和 Native 验收。

内容冻结增量新增 `FigureContentSnapshot` 与 typed `CONTENT_SNAPSHOT` capability。
Label 和外部 Figure 使用同一注册入口，将已准备的自绘输入复制到不可变 drawing；
采样与录制不再读取这些 Figure 的当前内容。内容 revision 变化取消关联组；仅布局
位置或尺寸变化仍可使用原冻结输入。静态 capture 也保留预检时取得的内容快照。
Label 定向测试直接统计 TextLayoutEngine 调用，并比较各采样帧的完整 glyph runs；
外部消费者测试覆盖单次冻结、连续录制和 revision 失效后切回静态绘制。
本增量回归为几何 21 项、采样预留 4 项、原 affine transition 15 项通过。
该能力仍不提供 opaque drawing 或字体/图像 payload 的字节统计与资源 lease；
这些留存条件和通用 bounds-dependent 自绘仍待完成，T-1 状态不变。

资源冻结增量在准入边界将内容 drawing 录制为不可变命令，继承解析后的画笔样式；
录制器在追加命令时检查存储额度，回调忽略错误也不能发布截断内容。不透明回调及其
domain data 不进入动画快照；glyph、路径、渐变、dash 和精确字体/图像 payload 纳入
留存字节统计，同一冻结源内的资源版本去重。捕获不消耗资源同步前缀，资源 revision
变化取消关联组，即使消费者未手工注册 resource dependency。
新增外部消费者回归验证 capture 保留旧图像版本，Runtime 销毁后该资源仍由 capture
持有，最后一个 capture 释放后资源才释放；另一回归验证不透明绘图回调中的共享数据
变化不会污染已冻结命令。边框数据、提交与恢复前缀的独立存储保证、通用自绘冻结
和工作量证明仍待完成，T-1 不关闭。

提交留存增量新增 `AnimationLimits::submission_bytes` 与公开
`RenderSubmission::retention`，提交包、in-flight 和 Retry 恢复前缀共享独立存储凭证。
准备失败不消耗帧编号或资源同步前缀，不清理合法活动组；新提交成功取得额度后才
移交旧恢复责任。damage 基线缩减为版本与 surface envelope，不保留 drawing。
几何 binding 与 provider 统一使用固定 `Copy` 值表示，值的 Arc 分配纳入字节统计。
新 capture 在冻结内容前取得有限存储预留，完成后归还未使用额度；owner 结束时清理
无查询权限的旧缓存。边框、通用自绘与缓存/候选的完整峰值证明、实际工作计数仍待完成。

冻结自绘增量新增 typed `BOUNDS_SNAPSHOT` 与 `BoundsDrawing`：固定 `Copy` 输入、
函数指针和有限命令存储声明共同描述 sampled border box 下的绘制。矩形、
Line/Margin/Rectangle/Bevel/Etched/Compound 边框使用同一扩展接口；复合描述显式
累计子描述存储和 inset，不保留可变 Border 对象或旧私有边框枚举的堆数据。
未提供尺寸描述的普通 Figure 自绘在捕获时录制为固定内容，动画录制不调用原 Figure。
自绘输出越限携带 Figure 与声明/实际字节数，只清理关联组并重建静态内容。
静态 capture 缓存持有共享留存凭证，几何与节点视图不提供公开 Clone，独立留存
必须通过 capture 或提交包取得额度。标题与 Scheme 边框的冻结描述、完整峰值预检
和实际工作计数仍待完成，T-1 保持 `in_progress`。

后续冻结绘制增量补齐椭圆、SchemeBorder 与 TitleBarBorder。Scheme 的各层端点和静态
协议一致；标题通过 `BoundsDrawing::recorded` 保留排好的字形和精确字体资源，只按
采样宽度调整位置。继承颜色经通用边框入口冻结，CompoundBorder 使用同一接口递交
子边框，不保留可变文本布局或边框对象。记录描述和复合描述累计命令、作用域及资源
存储上限；资源版本冲突在冻结阶段拒绝。
capture 在候选集合分配和尺寸相关绘制前检查合并依赖闭包及源/候选共存峰值；
活动预留使用同一集合与命令上限。Disabled/ReducedMotion 通用 start 在内容冻结与
provider 预检前返回静态 fallback，仍校验 channel 身份和最终值。几何定向回归 28 项、
采样预留 4 项、原有 animation 29 项、behavior 7 项与 affine transition 15 项通过；
新增标题字形/颜色/字体留存、峰值不足时不执行绘制回调和静态模式不调用冻结/派生
接口的反例。完整物化工作计数与 provider 配置存储证明
仍待完成，T-1 保持 `in_progress`，T-2 至 T-4 尚未启动。

工作量与配置增量将 provider 回调收紧为固定 `Copy` 捕获，配置、依赖容器容量、
调度临时表和结果表纳入预留。调度使用线性拓扑队列；查询及坐标操作逐次计数，
忽略越限错误仍拒绝发布。几何遍历提供 `materialization_work()` 实际节点、child
order、矩阵、clip 与 envelope 计数，发布前核对上界；capture 核对合并源工作容量。
活动存储增加 provider 阶段重建与当前/候选共存预留。旧 typed Motion 通过
`AnimationValue::storage_bytes()` 计入内联与堆存储，PointList 包含 Vec 容量；
采样超过端点/当前值声明时以 `TrackStorageExceeded` 终止所属组。
新增配置拒绝、查询越限被忽略、嵌套节点去重与旧轨道堆值越限反例，退出验证待运行。

`AnimationPlan::geometry_track()` 显式区分表现终点与已提交最终几何，只接受
GeometryBinding 通道。完成/取消清除覆盖后显示源状态；普通 track 的最终值校验
保持不变。该入口承接批准方案的非零收拢终点，不引入 Tree 特殊通道。

上述增量已通过 Core lib 与五个 animation integration target 的严格 Clippy、
33 项几何、4 项采样预留、29 项基础动画、7 项 behavior 与 15 项旧 transition
回归；373 份文档检查及差异空白检查通过。T-1 关闭前仍需总体验证与 API 证据复核，
本记录不代替后续 Tree 消费者及 Native 像素验收。

Tree 消费者接入准备已提取 Normal/Hanging 共用端点算法。静态布局继续写最终
Lines；动画使用固定 `Copy` 配置和显式依赖链派生端点，由受检 drawing
发布各分支的线条，不逐帧写 Lines。PageNode 冻结选中颜色，折角边框提供受检的
尺寸相关绘制描述。19 节点树的两个方向已通过首帧与静态线段逐项等价测试。
展开/折叠、Viewport 与 Native 中间帧仍待接入，不以这项测试关闭 T-2。

2026-10-26 最终实现已补齐 Tree 展开/折叠、普通 Viewport、默认关闭的真实 checkbox、
queued toggle 与同 S 反向。直接输入经 Runtime 物化/预检后接管整个关联计划。
外部 visibility revision 撤销旧保留，按 Advance/Pause/Finish 处理；topology/dispose、
字体/内容/边框和 viewport extent/scale 变化按冻结依赖取消。

稳定退出入口为 `animation.tree`：41 项几何、4 项采样预留、28 项 Viewport、21 项
Tree 定向测试通过。Native GPU 横纵方向、DPI 1/2 的中间/最终 retained-partial 与
Full 逐像素等价，最终与默认关闭、Disabled、ReducedMotion 像素等价。40 幅截图及
`geometry-work.csv` 位于 `target/verification/tree-animation/`；每个活动 E 最终遍历
79 节点、78 边、1,181 次矩阵/角点操作、213 次 clip、157 次 envelope。
8/32/128 层重叠 target 回归证明节点/边去重；资源/capture/submission/recovery
按各自留存期释放。真实 surface 提交在本机基线返回 Skipped，不声明 present 通过。
完整边界、证据与限制见
[实施复核](../verification/reviews/tree-layout-presentation-implementation-2026-10-26.md)。
最终 workspace fmt/check、严格 Clippy 与 test 通过：1,472 项通过、0 失败、6 项既有
忽略。host/wasm API 边界通过，Runtime 保持 28/28；374 份 Markdown、三本书
HTML/PDF 与差异空白检查通过。T-1 至 T-4 全部关闭；上文各增量的“待完成”均为当时快照。

承接 [API 与架构 Review](../verification/reviews/api-architecture-review-2026-10-09.md)
的 R01、R02、R05 发现。顺序依
[审计—收口持续闭环工作流](../verification/audit-remediation-workflow.md) 的
"当前 Execute 批次 ≤ 1"约束执行；三份 ADR 均已实现。

| 顺序 | Delta | 范围 | 状态 | 依赖 | 最低毕业证据 |
|---|---|---|---|---|---|
| 1 | P2-S02 | 删除库级 Prelude、显式领域导入与 Crate Root 收口 | `complete` | ADR-021 | ADR-035、module-only root、领域导入 consumer 与 API snapshot 已验证 |
| 2 | P2-S03 | Runtime 子系统 Facade 与统一 Figure Mutation | `complete` | ADR-019、ADR-027、P2-S02 | ADR-033 接受、Runtime 本体公开方法按其预算门禁（K01 两个受检构造器后 ≤ 27）、内置/外部 Figure 共用 component/capability mutation、workspace 调用点迁移 |
| 3 | P2-S04 | Editor Tool / Request / Handle 统一开放协议 | `complete` | ADR-015、ADR-032、TC-06 | ADR-034 已实现、四个闭合枚举及内置 typed hook 已退出、外部 Tool/Request/Policy/Handle 全链路与 G1-G5 定向 suite 已验证 |
| 4 | P2-S05 | 深层复合布局与文字 presentation 收敛 | `complete` | D4.4、P2-T01 | FigureNode 统一 content/outer measurement 投影；两层 Toolbar + Label、partial damage、UML Headless 与 Native/Vello 像素已验证 |
| 5 | P2-S06 | Editor 增量投影失败隔离与 Overlay 批量校验 | `complete` | P2-S04、TC-08 | ADR-037 与矩阵修订 2 完成；失败作用域清理、Overlay 整批校验、模型/history 与 fault 分离、Native GPU 队列退出；组合证明和平台限制见完成记录 |
| 6 | P2-S07 | 结构查询身份与失败语义 | `complete` | ADR-018/036、TC-09 | ADR-038 已实现；身份、合法缺失与数值失败分离；测量准入、候选隔离、缓存和新绑定恢复通过；消费者与最终门禁见完成核验 |
| 7 | P2-S08 | Detached Figure / Border / Style 输入不变量 | `not_started` | ADR-020/024、TC-03 | 复用已有受检值；构造/更新拒绝非法输入；保留合法几何适配与范围收敛 |
| 8 | P2-S09 | 服务 fault、注册、订阅与扩展错误 | `not_started` | P2-S03、TC-10 | fault 与幂等分离；Inspector 错误及 detach 所有权；消除残余 panic 注册入口；保留可追溯错误 |

本节 delta 优先级独立于上两节的 Draw2D 能力分母，但同受单链 Execute
约束；P2-S02 → P2-S03 → P2-S04 → P2-S05 已完成。P2-S05 的
[normative design](../design/architecture/deep-layout-presentation-convergence.md)、
实现与验证证据见
[completion record](../verification/reviews/p2-s05-deep-layout-presentation-completion-2026-10-12.md)。
P2-S06 已于 2026-10-15 按用户接受的矩阵修订 2 完成，
证据见[完成记录](../verification/reviews/p2-s06-failure-isolation-completion-2026-10-15.md)。

P2-S07 的契约见 [ADR-038](../adr/adr-038-query-identity-and-failure-semantics.md)，
回归范围见 [验证矩阵](../verification/plans/p2-s07-query-identity-and-failure-semantics.md)。
2026-10-16 实施与验收闭合，R04/TC-09/API-F06/API-07 的关闭依据及各阶段结果见
[完成核验](../verification/reviews/p2-s07-query-identity-completion-2026-10-16.md)。

### GEF 对等样例接入（2026-10-28 用户授权）

用户批准[对等方案](../design/editor/gef-official-examples-parity-proposal.md)并要求开始实施。
本次只执行 Scene 输出前置切片；CreationTool、框选、模板拖放与 Widget/View
依次推进，不另建并行执行队列，不更改既有 LT 或其他未完成批次的状态。

| 切片 | 状态 | 契约与退出证据 |
|---|---|---|
| Scene backend 输出 | `complete` | [ADR-053](../adr/adr-053-embedded-vello-scene-target.md)；[首批证据](../verification/reviews/gef-scene-target-implementation-2026-10-28.md)，4 项契约、12 帧 GPU 对照、39 项 backend 回归及独立消费成本 |
| Masonry Widget / Xilem View | `pending` | 固定兼容版本、稳定 session、输入/IME/capture/lifecycle 与真实宿主 |
| Shapes 工具与完整样例 | `pending` | CreationTool、marquee、模板创建；模型/history/投影与 UI 同步 |

### 当前推进顺序（2026-10-14 校准）

1. **审计整改主链**：P2-S06/S07 已完成，下一批 P2-S08 Design → P2-S09；两批尚未启动。每批先核对当前
   反例和既有机制，再冻结最小契约；顺序不等于相邻批次间均有技术依赖。
2. **后续能力切片**：上述 API/失败边界收口后进入 P2-L01 Design。动画为可选
   消费者，M01-D 的平台证据不阻塞独立图模型、算法 adapter 或无动画布局。
3. **待采证**：P2-G01 Native surface 验收、P2-M01 M01-D、GA-2/GA-3 保留
   未完成状态。G01 的 Skipped 先区分 runner 驱动与引擎缺陷；若证明 Core 正确性
   问题，重新 Triage，不能仅因归入发布准备而忽略。
4. **长期候选**：P2-C03/T03/W01/O01/A01/LC01 及尚未登记的组件、概览能力按
   消费者需求评审；`not_started` 和 `deferred` 均不表示已批准立即实施。

`in_progress` 表示交付尚未闭合，不表示同时执行；G01/M01 的待采证状态不应改成
`complete` 或重新做 A/B/C。单链约束针对当前进入 Execute 的整改批次。
候选类型名、统一错误枚举、所有字段私有化或新 guard 均不能单独成为毕业条件。
校准依据见[路线图复核](../verification/reviews/roadmap-calibration-2026-10-14.md)。

P2-S06 已按 [ADR-037](../adr/adr-037-editor-incremental-failure-isolation.md)与
[矩阵修订 2](../verification/plans/p2-s06-incremental-failure-isolation.md)关闭。
永久限制：采用已接受的组合证明，不承诺完整 synchronize 前缀、扩展副作用或成功
模型命令回滚；Web 异步 fault 退出保留 `S06-PL02`，Native surface 仍归 P2-G01。
机制必要性、草案、实现和剩余证据的时间线由
[完成记录](../verification/reviews/p2-s06-failure-isolation-completion-2026-10-15.md)导航。

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

有效交付范围：attach-time owned descriptor、typed discovery/mutation、标准与外部
capability，以及开放 `PresentationBinding<V>`。后续 ADR-033 已取消 concrete facade
例外；内置与外部 Figure 统一从 FigureMut 提交 component/capability update。

最低毕业证据：

1. 外部 Figure 接入标准和自定义 capability 均不修改 Core 类型分支；
2. 同步与 deferred capability update 共享 prepared commit、revision、invalidation、
   damage 和 notification；
3. duplicate、foreign、stale、absent、prepare rejection 和 panic/fault 有结构化语义；
4. Core 中不再通过具体 Figure downcast 判断横切能力；PointList/TextFlow/Image/
   Viewport 等具体类型不再伪装成 capability；
5. render、layout、event、connection、Native/Web 编译及外部消费者门禁通过；
6. 工作量基线无未解释回退，最终 quick/full 按项目分层门禁执行。

实施快照与当时门禁结果见
[P2-F02 实现记录](../verification/reviews/p2-f02-figure-capability-completion-2026-10-08.md)；
公开路径及统一更新的后续修订见
[P2-S02](../verification/reviews/p2-s02-explicit-domain-imports-completion-2026-10-09.md)与
[P2-S03](../verification/reviews/p2-s03-runtime-facade-completion-2026-10-10.md)。

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
- `crates/novadraw/tests/p2_child_insertion.rs` 仅通过公开 API 实现外部 Figure/Layout 与
  自定义 Placement constraint，验证 Builder/Runtime 得到相同顺序与布局；
- `core.p2-s01-child-insertion`：8 项通过，覆盖原子拒绝、失败 ID 作废、通知、
  admission、10,000 层深度及 validator panic 隔离；
- 当时 quick/full 质量门禁通过；当前直接 Cargo 与跨边界 suite 的分工见
  [验证入口](../../tests/README.md)；
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
本项与 P2-T03 富文本样式增量无关，也不关闭 P2-G01 未完成的 surface 验收。

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

公共 IR、现由 ADR-029 统一的 `RenderResult` 错误边界与 XOR 处置已获批准；
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

状态：`in_progress`。ADR-026 已接受；M01-A/B/C 已完成，剩余切片为 M01-D
平台与视觉采证，不重新建设已完成的时钟、表现平面或领域消费者。

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
2. P2-G01 Native surface visual repair 仍未通过；它约束 M01-D 的 Native surface
   视觉毕业，不能倒写为已完成 M01-B/C 的启动阻塞。Core/Headless 已有证据保留，
   Native surface 等价仍须独立验证；
3. ADR-026、公开 API、错误枚举与外部 typed consumer 已完成人工评审；
4. `core.p2-m01-animation` 已登记且覆盖 A/B/C；M01-D 新采证入口先注册可执行
   命令及原始证据路径，不登记空门禁。

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

M01-D 剩余工作：Native/Web 同时间关键帧像素、DPI/nested transform/scroll/zoom
组合、disabled/reduced-motion 视觉等价及实际 runner 帧时间预算。
Headless replay 和 1/64/1,024 active track 工作量已有 C 阶段证据；仅在行为变更后
运行回归，不将它们重复列为未实现能力。不得从 headless 计数推断实际帧时间。

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
2. `PaintContext::draw_image_region` 使用结构化错误拒绝非有限、负尺寸和越界 source；
3. Vello adapter 使用 source-to-destination affine 与 destination clip，完整消费该命令；
4. 完整图像 convenience API 进入同一 IR，不保留 `Option` 或 backend 特判协议；
5. `core.image-source-rectangle` 覆盖 backend-neutral、Vello 与 Web feature 门禁。

规范与证据：

- `doc/design/rendering/image-source-rectangle.md`
- `doc/parity/draw2d/api-coverage.md`
- `crates/novadraw/tests/image_source_rectangle_contract.rs`
- `tests/README.md`

## Developer tooling

### P2-D01: FigureInspector 开发期可观测性

状态：`deferred`

FigureInspector 以独立 crate 消费 Runtime 的稳定场景查询与提交后 notification journal，
为 Native、Web 和 headless 工具提供统一诊断事实。它不属于 Draw2D Core 的运行时语义，
也不改变 GEF Editor 里程碑。按当前优先级，本项与 Studio 一并后置，不进入
P2-C01/C02/F01/T01 执行批次；已有实现和文档保留，工具 UI 和新增诊断功能后置。
现有 Inspector 的 fault、订阅释放与错误传播归 P2-S09，不能因 D01 deferred 而跳过。

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

## P2 长文本精确布局优化

2026-10-27 用户批准[修订 2 首批复核](../verification/reviews/long-text-performance-revision-2-review-2026-10-27.md)
后启动 LT-00～LT-03，契约见 [ADR-051](../adr/adr-051-exact-flow-layout-reuse.md)。
本批是当前新增 Execute 切片；其他未完成项保持待验收状态。

| 切片 | 状态 | 当前证据 |
|---|---|---|
| LT-00 | `in_progress` | 第四轮完成四组匹配构建对照，每组每版 1000 次热样本、200 个新进程及 5 次 RSS，标准场景失败为 0；新增工厂的源提交 receipt 与逐样本阶段核算通过；另完成优化版混排 resize 的 1000 次热样本和 200 个新进程，无失败；原 Large 独立源提交、first-ready、生产 GPU/present 与 Web 体积仍缺证据 |
| LT-01 | `in_progress` | 共享布局/snapshot、source 索引、有序依赖去重、局部边框扫描及同 scene 的静态签名与字体解析复用已接入；第四轮测量仅生成尺寸，完整来源与绘制几何归准备阶段；贡献、边框与内嵌测量仍重读，尺寸一致性及预算失败测试通过；复制量与留存分配归属待采证 |
| LT-02 | `in_progress` | 精确复用与失效反例通过；第四轮原 Large 追加 p50 17.157→15.302 ms、p95 20.112→19.214 ms，Flow 构建自身平均 9.200→7.695 ms，部分尾延迟回退，未达到 16 ms；长段/长行追加 p95 为 38.526/52.733 ms；混排标准宽度为 6.864 ms，默认预算下缩窄成功，优化版 resize p95 14.469 ms |
| LT-03 | `in_progress` | 18 组 Full/Partial GPU 对照通过；第四轮原 Large 滚动新增 shaping 为 0，稳定化 p95 7.867 ms，resize 后 IR p95 5.748 ms；默认 native glyph 仍完整输出 51233 个；生产证书边界已形成待复核提案，4 ms 查询/IR 验收未完成 |

详情见[实施记录](../verification/reviews/long-text-performance-implementation-2026-10-27.md)。
原样例首次布局的五个会话均达到 p95 ≤ 100 ms，追加的五个会话均未达到 16 ms。
第四轮只比较两份独立匹配构建，不与旧轮二进制计算严格提速比。
混排 resize 保留基线预算拒绝与优化版重复成功两份证据，不计算前后耗时比；
有限输入的成功不证明任意混排均可准入。
LT-04～LT-09 未纳入本次授权，保持独立评审；本批整体仍待验收。

## P2-F03：逐角圆角与矩形分边描边

2026-10-27 用户批准实施 [ADR-052](../adr/adr-052-independent-corners-and-edge-borders.md)。
状态：`complete`（Core 实现、自动验证、macOS GPU 与可达产物测量完成）。

Core 提供公开 `ShapeContour`、真实逐角半径、矩形逐边颜色和开关，以及原子的
typed component updates。标题区结构留在 `composite-shapes-app` 独立样例，
不进入 Core。旧圆角宽高和 `RectangleBorder` struct literal 保持兼容。

十一组 macOS GPU 场景检查圆形/椭圆圆角、不等半径、半透明接缝及关闭底边，
同时验证纯几何、外部 Figure、Border snapshot、同值与失败原子性。
suite：`shape.independent-corners`；[实施证据](../verification/reviews/independent-shape-corners-2026-10-27.md)。
Web、Windows、Linux 的实际像素验收仍属于平台发布准备，不由 macOS 结果外推。

2026-10-27 自由端修订已获用户批准：相邻边关闭时采用 Butt 平齐截面，接角保持
原有颜色分区。实现、16 种开关组合、native GPU 端点与最终门禁均已通过；
见[自由端实施记录](../verification/reviews/shape-free-end-butt-2026-10-27.md)。
