# 官方对等样例剩余能力核对

类型：`verification`

日期：2026-10-26

范围：官方 25 个入口中的 Canvas / LightweightSystem 能力。
本记录核对当前代码与既有证据，不修改入口状态或执行队列。

## 状态口径

当前 catalog 登记 17 个 `complete`、7 个 `in_progress`、1 个 `blocked`，
24 个入口有运行工厂，没有 `not_implemented` 条目。
登记完成指已记录的 Canvas 范围，不代表全部平台、系统字体和窗口呈现均已验收。
来源：[当前目录](../../../examples/native/draw2d-official-examples/src/lib.rs)、
[Canvas 收口清单](../../../examples/native/draw2d-official-examples/completion-contracts.tsv)、
[剩余工作](../plans/draw2d-official-examples-next-slices-2026-10-23.md)。

当前未收口的 8 个入口见下一节。其余 17 个为：
HelloWorld、ObstructionExample、ConnectionAndAnchorExample、CompoundGraphDemo、
DirectedGraphDemo、EllipseHitting、PathExample、InheritBackgroundExample、
BidiBlockExample、CaretExample、MultiLineLabelExample、TextFlowExample、
TextFlowLargeExample、TextSurroundingFigureExample、ThumbnailExample、
SimpleTreeExample、TreeExample。

旧记录里的 14／25、16／25，以及 Tree、Thumbnail、InheritBackground、Path 的早期
阻塞描述均属于当时快照，不能作为当前缺口。K06 Tree 的 T-1 至 T-4 已完成；
K07 六个批准切片与 K08 SVG 密度切片也已完成。
来源：[当前执行记录](../../roadmap/p2-delta-backlog.md)、
[Tree 实施复核](tree-layout-presentation-implementation-2026-10-26.md)、
[SVG 密度证据](k08-svg-density-evidence-2026-10-23.md)。

## 尚未完整对等的入口

| 入口 | 已实现范围 | 剩余工作 | 主要归属 |
|---|---|---|---|
| FlowLayoutExample | 官方 Shape、双轴对齐、不等 spacing、换行、stretch、方向切换、零空间恢复；连续 GPU full-frame 擦除已有证据 | 连续真实窗口 resize、文字 minimum、retained-partial 像素验收 | K02 既有布局契约验证、H01／H04 |
| GridLayoutExample | span／grab／hint／indent、真实 Add／Clear 面板、选择释放、零空间恢复及 GPU 清空 | 文字消费者的约束与恢复、连续真实窗口 resize、retained-partial 像素验收 | K01／K02 既有契约验证、H01／H04 |
| ToolbarLayoutExample | 对齐、方向、stretch、收缩、恢复及 GPU 擦除 | 文字 minimum、连续真实窗口 resize、retained-partial 像素验收 | K02 既有布局契约验证、H01／H04 |
| PrintExample | Canvas 图文、Shape、Draw2D Button、连接、原始 ICO 和通用命令输出 | Helvetica、Times New Roman、Perpetua 均以 Inter 替代；需真实字体来源、指标和画面证据 | H01 字体适配、样例资源与 H04 |
| ScrollPaneExample | 五部件 ScrollBar、range／repeat、InternalFrame 拖动和八向 resize、自动显隐、cursor | 完整主题、边框与 Canvas 视觉验收 | Figure 组件及样例样式、H04 |
| UMLClassDiagram | 两类分段、图标、owner-aware 标签定位及重新测量、便笺文字和折角绘制 | 标题／成员字体、图标与窄宽换行验收；DogEar 描边还有具体源码差异 | 样例 Figure／Border、H01／H04 |
| ZoomExample | 五部件 ScrollBar、原始公式、SVG 源与密度更新、共享资源 revision、GPU 替换恢复；宿主滑块输入已修复 | 跨显示器系统 DPI、字体／完整视觉对等与真实窗口验收 | H01／H02／H04 |
| ShapeStylesExample | 当前没有运行工厂，状态为 blocked；已有通用 stroke／alpha 能力不能代替该入口 | 五种 Shape、八个样式轴的矩阵消费者；逐图形 AA；hairline 与低 miter 语义及像素证据 | K08 Graphics／Render IR／backend，样例编排与 H04 |

三种布局的 full-frame GPU 结果和 CPU damage 范围检查已经通过；
它们尚不能证明 retained-partial 后端实际擦除所有旧像素。文字 minimum 的剩余项
也不能直接解释为“Core 没有最小尺寸 API”。须通过真实 Label／TextFlow 消费者
复现失败后，才确定是否需要布局修复。
来源：[布局消费者核验](k02-layout-consumers-evidence-2026-10-23.md)。

Print 的字体替代可直接在
[printing.rs](../../../examples/native/draw2d-official-examples/src/examples/printing.rs)
看到。Zoom 的 SVG 与输入证据分别见
[密度更新核验](k08-svg-density-evidence-2026-10-23.md)、
[输入归属修复](zoom-input-arbitration-2026-10-26.md)。
这两项局部完成均不关闭 Zoom 的完整入口。

## Core 与后端仍需补充的能力

### 逐操作抗锯齿

这是明确的缺失链路。官方 ShapeStyles 每个单元可设置 ON／OFF／DEFAULT；
Draw2D 的 SWTGraphics 分别保存图形和文字抗锯齿状态。当前 Core Graphics／IR
没有对应提示，Vello 现有 AA 配置也不能直接提供同帧逐图形 OFF。
来源：[官方 ShapeStyles](https://github.com/eclipse-gef/gef-classic/blob/4463d9d0ce13c19d10fbe769d29f28b7345a8cba/org.eclipse.draw2d.examples/src/org/eclipse/draw2d/examples/swt/ShapeStylesExample.java)、
[SWTGraphics](https://github.com/eclipse-gef/gef-classic/blob/4463d9d0ce13c19d10fbe769d29f28b7345a8cba/org.eclipse.draw2d/src/org/eclipse/draw2d/SWTGraphics.java)、
[当前 AA 方案](../../design/rendering/shape-antialias-proposal.md)。

需要覆盖 Graphics 状态栈、命令冻结、能力准入和后端覆盖率计算。
OFF 必须改变几何采样，并保留绘制顺序、paint alpha、clip 和文字 AA 的独立性。
仅添加 setter、返回 Unsupported 或调整整帧 AA 配置均不能完成入口对等。
Core 与生产后端方案仍处于待评审、未实施状态。随后完成的
[独立 Native 实验](shape-aa-feasibility-2026-10-26.md)证明设备整数游程可复用 Vello
合成；扫描线、预算、缓存、文字与跨平台生产路径尚未完成。

后端可行性应先验证交错 ON／OFF 的曲线、斜线与透明图形，再固定公共契约。
需记录每条命令的额外状态、GPU mask／中间纹理与分段合成成本、峰值预算、
Native／WebGPU 支持及最终产物增量；当前没有这些成本的实测结论。

### 设备 hairline

官方样式矩阵包含线宽 0。当前受检 StrokeStyle 可以保存 0，
但 Vello lowering 明确跳过该描边，因此不能将“接受 0”当作设备 hairline 已实现。
来源：[StrokeStyle](../../../novadraw/src/render/stroke.rs)、
[Vello lowering](../../../novadraw-backend-vello/src/lib.rs)、
[AA 方案中的独立边界](../../design/rendering/shape-antialias-proposal.md)。

如要支持设备 hairline，应定义区别于逻辑宽度的显式语义，并明确 DPI、
非均匀缩放、旋转、dash、命中与 damage envelope。已有“逻辑宽度 0 不绘制”
契约不能在样例代码中偷偷替换。此能力应独立评审，不借 AA 实施扩大范围。

### 低 miter 值

官方矩阵可选择 0 和 0.5；当前 `StrokeStyle::try_new` 拒绝小于 1 的 miter limit。
这是可确认的契约差异。上游各平台对这些输入的实际描边行为尚未在本次核对中裁定，
因此不能直接承诺放开输入就能对等，也不能静默夹到 1。
来源：[官方矩阵](https://github.com/eclipse-gef/gef-classic/blob/4463d9d0ce13c19d10fbe769d29f28b7345a8cba/org.eclipse.draw2d.examples/src/org/eclipse/draw2d/examples/swt/ShapeStylesExample.java)、
[当前校验](../../../novadraw/src/render/stroke.rs)。

后续须从只读 Draw2D／SWT GC 语义证据确定准入、描边轮廓与边界计算，
再评审是否扩展 Core 契约及后端 lowering。这个决定与 AA、hairline 分开。

### 混合字体段的通用文字装饰

现有 `FontDecorationMetrics` 和 Path 消费者已实现单字体的下划线指标绘制；
跨字体段、跨行的通用装饰排版器尚未提供。这属于已经记录的 K08 后续增量，
不能把它重新算成 Path 尚未移植，也没有证据表明它阻塞当前全部 8 个入口。
来源：[边框与输出增量](../../design/architecture/official-example-border-output-increment.md)、
[Path 实现](../../../examples/native/draw2d-official-examples/src/examples/path_example.rs)、
[剩余计划](../plans/draw2d-official-examples-next-slices-2026-10-23.md)。

若继续扩展，需由 Text／Graphics 统一处理每段字体指标、baseline、逻辑范围、
bidi／换行分段和装饰 ink bounds；布局、命中与 damage 应读取同一已准备结果。
已有 styled／nested flow 不是缺失项，不应因此再建一套文本树。

## 样例与宿主的具体缺口

UML 便笺的填充已经采用 6 单位折角，但当前用同一个多边形描边，
并继续用 6 单位绘制内折线。上游 DogEar 则先将 paint rectangle 的宽高减 1，
使用 5 单位折角单独绘制边框。该差异可从源码直接确认，尚未进行本轮像素测量。
来源：[当前 StickyNote](../../../examples/native/draw2d-official-examples/src/examples/uml_class_diagram.rs)、
[上游 StickyNote／DogEar](https://github.com/eclipse-gef/gef-classic/blob/4463d9d0ce13c19d10fbe769d29f28b7345a8cba/org.eclipse.draw2d.examples/src/org/eclipse/draw2d/examples/uml/StickyNote.java)。

这项应使用已有普通 Border／Graphics 能力修正，并验证 Insets、窄宽文字和旧新
描边范围，不需要新增“UML 专用 Core”。当前 catalog 的“仍需视觉核验”应理解为
含有尚未修正或裁定的样例差异，不能仅补一幅截图后关闭。

系统字体来源主要属于宿主。默认 Parley 字体集合禁用自动系统字体扫描，
样例注册内置字体；Core 已提供字体资源注册与请求完成链路。
Print 的三个字体、UML 的系统默认字族、Zoom 请求的 Arial 需要宿主明确提供真实
font bytes／face，并验证实际解析字体与指标；只更改 family 字符串不足以证明对等。
来源：[字体引擎](../../../novadraw/src/render/text.rs)、
[字体资源入口](../../../novadraw/src/runtime/runtime/service_facades.rs)、
[样例字体初始化](../../../examples/native/draw2d-official-examples/src/examples/mod.rs)。

主题、跨显示器 DPI 与窗口呈现也应分别由 Figure 组件、platform／backend 和验证
承担。字体按需加载的体积、首次加载与资源寿命需要评估，不能将全部系统字体内置。
SWT 原生控件、打印对话框／作业和完整 GEF Editor 不属于本次 Canvas 对等目标。
来源：[当前范围裁定](../plans/draw2d-official-examples-core-plan.md)。

## 后续核验顺序

交付收口可先修正 UML 的已知描边差异，再补三种布局的文字与 retained-partial
证据，以及 ScrollPane 的主题验收。这些工作优先复用已有契约，出现确定性失败
再做 Core 根因修复。Print／Zoom 的真实字体和系统 DPI 验收走宿主链路。

Core 能力演进重点是 K08：先审查逐操作 AA 的后端可行性，再分别决定 hairline
和低 miter 契约。混合字体装饰单独保留为后续通用增量。ShapeStyles 还要建设
实际矩阵消费者，不能用 Core API 完成状态替代入口完成。
这些是核验建议，实际批次仍由 P2 backlog 单链安排。

## 核对限制

本记录使用本地当前 Rust 源码、官方只读源码和既有验证记录；
官方源码版本为 `4463d9d0ce13c19d10fbe769d29f28b7345a8cba`。
同一项目的 catalog、计划与证据并非相互独立的外部来源，因此只用于交叉核对
登记状态、代码行为和实际验证范围，不宣称三份独立来源证明全部视觉对等。

本轮没有运行 Java、重跑全部像素用例或验收实际窗口与跨显示器 DPI。
没有把验证缺口当作已发现的 Core 缺陷，也没有将提案描述为已实现能力。
