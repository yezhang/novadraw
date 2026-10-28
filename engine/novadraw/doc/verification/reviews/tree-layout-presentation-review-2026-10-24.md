# Tree 表现几何方案评审

类型：`architecture-review`

日期：2026-10-24

状态：评审完成，建议修订后复核；尚未批准实施。

对象：[Tree 动画需要表现几何](../../design/animation/tree-layout-presentation-proposal.md)。
本文合并方案首轮评审与 Rust 实现核对，记录该次快照的发现，不替代设计、ADR 或路线图。

## 方向可接受，实施契约仍需补齐

真实 bounds 与 affine transform 分离、源状态先提交、Runtime 统一驱动、所有消费者
共享表现几何，这些方向符合 ADR-026。现有 Timeline、事务稳定化、取消和资源清理
可以复用。进入 T-1 前，建议补齐下表的 P1 契约，并在修订稿中逐项说明处理结果。
P1 表示影响本方案实施的设计缺口或适配风险，不表示当前静态行为全部存在缺陷。

| 编号 | 优先级 | 原方案章节 | 修订主题 | 涉及切片 |
|---|---|---|---|---|
| R01 | P1 | 折叠保留显示，交互立即使用终值 | 显式折叠收拢终点 | T-1、T-3 |
| R02 | P1 | 增加 Runtime 拥有的表现几何快照 | 完整绘制几何与裁剪读取点 | T-1 |
| R03 | P1 | 增加 Runtime 拥有的表现几何快照；线条使用同一个几何视图；取消与失败使用既有组语义 | 采样阶段、provider 依赖与错误传播 | T-1、T-2 |
| R04 | P1 | 增加 Runtime 拥有的表现几何快照；取消与失败使用既有组语义 | 捕获范围、版本与当前表现起点 | T-1、T-3 |
| R05 | P1 | 折叠保留显示，交互立即使用终值；取消与失败使用既有组语义 | 表现有效可见性与 suspension | T-1、T-3 |
| R06 | P1 | Viewport 跟踪与布局共用一个进度 | retarget 锚点、clamp 与输入接管 | T-3 |
| R07 | P1 | 增加 Runtime 拥有的表现几何快照；实施与退出证据 | damage 几何一致性与遍历预算 | T-1、T-4 |
| R08 | P1 | 线条使用同一个几何视图；实施与退出证据 | Tree 线条与 Viewport 消费者迁移 | T-2、T-3、T-4 |
| R09 | P2 | 增加 Runtime 拥有的表现几何快照；线条使用同一个几何视图 | channel 身份、组合与兼容策略 | T-1、T-2 |
| R10 | P2 | api_semantics；实施与退出证据 | 语义覆盖与退出证据补充 | T-1 至 T-4 |

## R01：折叠需要独立的收拢终点

**发现。** 原方案明确展开从收拢几何开始，但未定义折叠后代的表现终点。静态隐藏
布局的 committed bounds 不必等于收拢到操作节点的 bounds。直接向静态终值插值，
可能只得到逐渐裁掉后代的效果。

**证据。** Draw2D `TreeBranch.collapse()` 先计算折叠布局，再调用
`animationReset(getNodeBounds())`，捕获后交换两端；`NormalLayout.layout()` 的静态
折叠分支只将 contents 设为零尺寸。Novadraw 的
[BranchLayout](../../../examples/native/draw2d-official-examples/src/examples/tree_layout.rs#L423-L425)
提交 title、contents bounds 和 contents visibility，也没有定义后代收拢终点。

**建议。** 区分 committed final geometry 与 presentation terminal geometry。显式
定义折叠终点如何使后代标题中心收拢到操作节点，并经统一坐标协议转换到各自父域。
表现终点不写回 source。自然完成时同帧移除几何和可见性覆盖。

**复核证据。** Normal/Hanging、多层后代、零尺寸样本和中途反向；确认后代向操作节点
收拢、字号与描边不随 bounds 缩放，完成或取消后等价于静态折叠。

## R02：几何视图必须覆盖完整绘制协议

**发现。** 当前表现快照只包含 opacity、transform 和 self-content，尚无真实 bounds
与表现可见性。仅增加 bounds 字段不足以实现方案。

**证据。** [FigurePresentationEffect](../../../crates/novadraw/src/animation/service.rs#L230-L249)
定义现有快照内容。[递归绘制](../../../crates/novadraw/src/graph/render_recursive.rs#L119-L196)
先检查 committed visible，再读取 committed bounds；opaque 背景、自绘尺寸和 border
使用这些尺寸。`paint_client_area` 与 `paint_children` 继续读取 committed client area、
child transform 和 child bounds。非 identity 表现 transform 还会绕过 child bounds clip。

**建议。** 在 T-1 中列明统一几何视图的读取点：placement、自绘尺寸、opaque 背景、
border、client area、child transform、child clip、可见性与绘制剔除。真实 bounds
channel 保持裁剪策略，不沿用 affine 过渡的绕过规则。渲染继续复用当前递归协议，
不增加第二套树或 Tree 特殊入口。

**复核证据。** 嵌套 translation/scale/rotation、四侧 insets、普通裁剪与
OverflowVisible、零尺寸和 border。绘制、线条、viewport 与 damage 消费同一采样几何；
普通 FigureTree 查询仍返回 committed 状态。

## R03：provider 阶段与失败传播需要公开契约

**发现。** 方案提出先生成几何快照，再派生线条与 viewport，但未明确阶段顺序和依赖
约束。现有 self-content provider 也不具备跨 Figure 几何读取或结构化失败出口。

**证据。** [PresentationFamily](../../../crates/novadraw/src/animation/mod.rs#L71-L89)接收当前
Figure、同一 Figure 的 channel 值和一个 Rectangle，直接返回 FigurePresentation。
[snapshot](../../../crates/novadraw/src/animation/service.rs#L462-L507)在构造快照时传入
committed bounds，内部使用 Option 合成内容。[公开 API 合同](../../design/animation/public-api-contract.md)
规定 provider sample 只读取冻结输入与时间。现有 Motion sampler 的错误处理不能直接
证明新增几何派生 provider 的失败清理。

**建议。** 明确因果顺序：采样基础几何 → 派生线条与 viewport origin → 冻结最终帧
表现 → 计算 damage → 录制。区分 Motion 采样与几何派生 provider，声明只读依赖、
稳定执行顺序和循环拒绝规则。跨 Figure 查询返回 Result；provider 的普通失败使关联
计划整体清除并显示静态终值，panic 仍沿 Runtime fault 边界。外部 crate 使用同一入口。

**复核证据。** 多 provider 顺序确定、循环拒绝、跨 Figure 查询失败、prepare/逐帧派生
失败、panic、预算失败。失败不留下部分线条、几何或 viewport owner，不回滚已提交源状态。

## R04：capture 需要覆盖布局影响与依赖版本

**发现。** “捕获目标及必要祖先”未说明兄弟分支和 provider 依赖如何纳入。
Tree contour 变化可以重新安排兄弟分支。现有 capture 只冻结 committed bounds，
也不足以保证快速反向从当前表现起点继续。

**证据。** [FigureTransitionCapture](../../../crates/novadraw/src/animation/mod.rs#L584-L588)
仅保存 namespace 与 `(FigureId, Rectangle)`；`capture_figures` 读取所选 Figure 的
committed bounds，没有捕获 epoch、拓扑、绘制版本或当前表现几何。
Draw2D `TreeLayout.layout()` 根据 contour 安排兄弟位置，必要时移动此前兄弟节点。

**建议。** 区分动画目标集合与只读依赖集合，由 Tree 消费者在 capture 阶段显式声明
受影响容器、子节点与线条依赖，冻结身份和顺序。允许一次性收集，不允许逐帧 discovery。
定义捕获版本及 reconcile 规则，分别说明 reparent、再次布局、字体、insets 和 provider
变化后如何 retarget、取消或继续。中途反向同时捕获当前表现几何，不以旧 committed
终值替代新起点。

**复核证据。** 折叠中间分支后两侧兄弟重新排布、重复反向、拓扑变化、字体/insets
变化、依赖删除及 foreign/stale 身份。retarget 失败保留旧计划，不闪现 committed 终值。

## R05：可见性覆盖必须参与 suspension 判断

**发现。** 只允许隐藏后代继续 paint，还不能保证折叠计划持续运行，也不能保证隐藏
祖先和零尺寸 clip 不提前截断表现。

**证据。** [reconcile_visibility](../../../crates/novadraw/src/animation/service.rs#L673-L685)
按 committed effective visibility 判断：任一 Figure target 隐藏，就设置整个计划的
target_hidden。Pause 暂停整组，Finish 结束整组；Advance 不会消除 renderer 的
committed visible 短路。现有测试已覆盖隐藏 Figure 的 Pause/Finish 行为。

**建议。** 定义表现有效可见性的祖先传播规则，区分本计划保留的折叠表现、外部隐藏
祖先与 surface suspend。绘制和 suspension 从同一规则取值；clip 与绘制剔除读取
采样几何，并保留祖先裁剪约束。交互资格仍来自 committed 状态，表现覆盖不能恢复命中、
焦点或 accessibility 资格。

**复核证据。** committed hidden 且 presentation visible 的整组运行与终止；
外部隐藏祖先、Pause/Advance/Finish、suspend/resume、Disabled/ReducedMotion 和 dispose。

## R06：viewport 需要重新锚定与直接输入接管

**发现。** 原公式适用于一次过渡，中途反向需要新的表现锚点；extent/clamp 的输入
来源和直接输入终止范围尚未定义。“使用已有 viewport 输入仲裁”也不能视为已实现
动画 owner 接管。

**证据。** [viewport transition](../../../crates/novadraw/src/runtime/runtime/frame_submission.rs#L148-L215)
实际作用于 contents Figure 的 transform，没有独立 sampled origin channel。
[scroll controller](../../../crates/novadraw/src/runtime/context.rs#L754-L769)固定输入会话目标；
[滚动与 origin 写入入口](../../../crates/novadraw/src/runtime/runtime.rs#L2983-L3003)没有取消或
接管动画 owner 的逻辑。现有 pan 测试不证明 Tree 跟踪和直接输入中断。

**建议。** 每次 retarget 同时冻结当前表现节点位置和当前表现 origin，重新建立锚点。
位置读取截止于被跟踪 viewport 的内容域，不包含该 viewport 自身的滚动映射。定义
extent/clamp 使用 committed 或其他明确来源，以及第一帧、末帧的收敛规则。明确直接
滚动是否取消整组、增量基于哪个 origin，并在引擎层增加动画接管。说明跟踪 channel
与 contents 自身 transform 的组合/冲突规则，避免重复施加位移。

**复核证据。** 双轴跟踪与 clamp、extent 缩小、中途反向、wheel/drag、其他 transform
owner 冲突、retarget 失败、取消和完成后的最终 origin；不写回旧 origin。

## R07：damage 必须共用采样几何并限制重复遍历

**发现。** 当前 envelope 计算不能直接支撑嵌套真实 bounds 重放；“不逐帧 discovery”
也不足以约束已有后代遍历成本。

**证据。** [presentation_envelope](../../../crates/novadraw/src/graph/query.rs#L595-L618)对每个
动画 Figure 遍历后代，读取 committed 可见性、坐标与 visual bounds，再施加该 Figure
的单个 transform。它没有组合其他节点的 sampled geometry。祖先和后代都成为 target
时，同一子树被重复扫描，链状目标的总遍历量可能达到平方量级。
[reconcile_damage](../../../crates/novadraw/src/animation/service.rs#L2377-L2414)对 self-content
变化请求完整重绘，但完整重绘不能消除上述重复计算。

**建议。** 通过统一几何视图组合变换、有效 clip、线条 envelope 和 viewport 影响，
保留旧帧的 envelope。旧 envelope 不用新祖先几何重新解释。定义捕获预算、逐帧派生
预算与遍历统计；避免每个祖先 target 分别扫描同一后代。明确预算失败对关联计划及
无关 active plan 的影响，不隐式扩大既有 fallback 契约。

**复核证据。** 后代越出旧 bounds、普通裁剪/OverflowVisible、viewport 移动的旧新
可见区域、取消/完成清理、retained partial 像素擦除，以及嵌套目标数量增长的工作量。

## R08：Tree 线条和 Viewport 入口需要显式迁移

**发现。** T-2/T-3 尚未把现有消费者的适配范围写完整。

**证据。** [BranchLayout](../../../examples/native/draw2d-official-examples/src/examples/tree_layout.rs#L423-L471)
在 layout 中直接写共享 Lines，折叠时写入空线条。
[TreeBranchFigure](../../../examples/native/draw2d-official-examples/src/examples/simple_tree_example.rs#L244-L269)
只注册 Container capability，paint 读取 Lines。
[19 节点 fixture](../../../examples/native/draw2d-official-examples/src/examples/tree_example.rs#L176-L237)
直接挂到白色 host Figure，没有建立 Tree 专用 Viewport。现有面板尚无 Use Animation
入口，源码注释也将其保留为独立表现/viewport 契约。

**建议。** 静态与动画线条共用端点算法；动画发布独立不可变 drawing，不逐帧写 Lines
或调用 LayoutManager。T-3 显式增加 Viewport 组合、操作节点跟踪和输入接管入口；
T-4 增加默认关闭的 Use Animation 面板。样式切换仍按静态最终结果处理。

**复核证据。** Normal/Hanging 任意采样端点对齐、文字与描边尺寸不变、19 节点真实
面板、动态增删与释放、Native GPU 中间帧和恢复、关闭动画后的静态等价。

## R09：channel 身份与 bounds 组合不能隐含

**发现。** 当前 channel 身份不能仅靠不同诊断名称区分同一值类型的多个语义。
真实 bounds 与 local transform 同时存在时，组合顺序和现有 BoundsTransition 的
兼容策略也未明确。

**证据。** [bind_presentation](../../../crates/novadraw/src/animation/service.rs#L1703-L1714)按
FigureId 与值 TypeId 查找 channel；[Capability 注册](../../../crates/novadraw/src/figure/capability.rs#L497-L509)
按 descriptor TypeId 去重。两个不同名称的 PresentationBinding<Rectangle> 不能成为
同一 Figure 的两个独立 capability/channel。
`bounds_transition_transform` 仍通过宽高比将旧 bounds 映射到新 bounds。

**建议。** 明确语义类型或 typed identity，保持内置与外部消费者对等，不以字符串
名称作为行为身份。写清 sampled bounds、local transform、insets 和 child transform
的组合顺序，以及现有 affine BoundsTransition 的保留、迁移或替代策略。文字沿用
committed 排版还是重新准备，也应写明，避免逐帧 shaping。

**复核证据。** 同一 Figure 多个同底层值类型的语义、外部 channel/provider、bounds
与 transform 同时运行、旧 API 回归、文字 glyph 字号与排版缓存。

## R10：语义范围与切片退出证据应补充

建议在现有 api_semantics 外补充：
`figure.geometry.bounds`、`figure.visibility.enabled`、`figure.box.client_area`、
`clipping.strategy`、`paint.protocol` 与 `frame.preparation`。
归属以 [Draw2D API 语义账本](../../parity/draw2d/api-coverage.md)为准。

| 切片 | 在原退出证据上补充 |
|---|---|
| T-1 | 首帧无终值闪烁；retarget/派生 provider/预算失败原子性；无动画零额外工作；嵌套变换与有效 clip；捕获版本与依赖 reconcile；重复遍历工作量 |
| T-2 | 静态/动画端点同源；同一采样 epoch；glyph 字号、描边宽度与排版缓存；不逐帧写 Lines 或布局 |
| T-3 | 收拢终点；兄弟重新排布；反向后的表现锚点；双轴 clamp 与 extent 收敛；直接输入接管；Viewport 组合入口 |
| T-4 | 默认关闭的真实面板；Native GPU 中间帧与旧像素擦除；Retry/恢复；资源释放与最终静态等价 |

修订稿可新增“评审响应”表，按 R01–R10 引用本记录，填写采用方式、修订章节或不采用
理由。该表放在修订稿中，本次评审原判断保留；修订后另做复核，不把本文改成实施完成记录。

## 核对范围与验证结果

Novadraw 基线提交：`3b6e43d30040f72c0a165c006e3690b8a1ce89c8`。
上述方案与源码定位来自 2026-10-24 工作树；评审时涉及文件没有未提交改动。

参考合同：
[ADR-026](../../adr/adr-026-animation-and-presentation-plane.md)、
[公开 API](../../design/animation/public-api-contract.md)、
[领域能力矩阵](../../design/animation/capability-integration.md)、
[坐标规范](../../design/coordinates/coordinate-system.md)、
[ADR-025](../../adr/adr-025-unified-graphics-and-glyph-preparation.md)。

Draw2D 只读参考：
`/Users/bytedance/Documents/code/GitHub/gef-classic/org.eclipse.draw2d.examples/src/org/eclipse/draw2d/examples/tree/`
中的 Animation、TreeBranch、TreeLayout、NormalLayout 与 HangingLayout。未使用 Zest，
未修改、编译或运行 Java。

本次 Rust 核对已执行以下精确 integration target：

| 命令 | 结果 |
|---|---|
| `cargo test -p novadraw --test p2_animation_transition_contract` | 15 项通过，0 失败，0 忽略 |
| `cargo test -p novadraw --test p2_animation_contract` | 29 项通过，0 失败，0 忽略 |

既有 suite 身份为 `core.p2-m01-animation`，见
[M01-C 实现证据](p2-m01c-animation-consumer-evidence.md)。本轮只运行上表两项定向测试，
未运行完整 suite。结果证明既有基础回归通过，不证明本方案已实现，也不证明 Tree
中间帧或 Native 像素正确。未新增 Rust 测试或修改实现。

## 记录整理与检查

两轮评审已合并，重复发现归入 R01–R10；方案正文只增加本记录链接，未替作者修订
设计。事实核对保留了源码范围、建议模态、测试数量和验证限制，主线仍为实施前的契约补齐。

本记录中文机械扫描：半角标点、破折号、元话语、机械章节编号及对比句式均为 0 处，
无豁免项。引用文件路径与源码起始行检查为 0 个错误。
`cargo xtask docs` 通过，检查 370 个 Markdown 文件及当前 xtask 引用；
`git diff --check` 通过。
