# 官方样例逐项功能复核

类型：`verification`

日期：2026-10-26

范围：完整官方目录的 25 个入口，包含此前登记完成的入口。
官方源码基线：`4463d9d0ce13c19d10fbe769d29f28b7345a8cba`。

## 当前结论

ObstructionExample 的初值就是 5 个绿色障碍矩形和坐标文字，没有端点或连线。
端点配对和路线由用户操作创建。初始画面简单符合官方源码，是否完成仍须检查
创建、拖动、重新寻路和删除。

全目录尚未完整对等。本轮确认 BidiBlockExample 遗漏右侧 Canvas 的镜像语义，
将该入口从 `complete` 改为 `in_progress`。修订后目录为 16 个 `complete`、
8 个 `in_progress`、1 个 `blocked`，24 个入口有运行工厂；默认共享窗口提供
23 个入口，InheritBackground 使用独立双 Canvas 宿主。
来源：[目录和完成计数](../../../examples/native/draw2d-official-examples/src/lib.rs)。

这个计数只表示登记状态。Caret 的选择绘制与边界、HelloWorld 的窗口尺寸适配、
字体替代以及真实窗口视觉证据仍需分别核验，不能将 16 个完成标记解释为
16 个入口均已通过官方逐像素对照。

本记录补充[此前剩余能力核对](draw2d-official-examples-remaining-audit-2026-10-26.md)：
此前 17／7／1 是原快照，保留其原文；本次将已完成入口也纳入复核。
原始需求与范围见[全入口契约](../plans/draw2d-official-examples-remaining-contracts.md)
和[Canvas Core 计划](../plans/draw2d-official-examples-core-plan.md)。

## 核对方法

每个入口先读取官方初值、Figure 构成、直接 helper 和可达回调，再核对 Rust 工厂、
实际共享窗口的 `panel_scene` 分派及现有测试。初始画面、交互功能、字体与绘制、
宿主验收分别判断。没有发现确定性失败的验证空白，只记为待核验。

Java 源码只读，没有编写、修改、编译或运行 Java，没有使用 Zest。
Canvas 外的 SWT 控件外观、打印对话框与作业不纳入本次完成条件。
SWT.RIGHT_TO_LEFT 引起的 Canvas 镜像会改变内部 Draw2D 行为，仍在范围内。

下表按窗口固定序号排列。“已覆盖”指源码调用链和现有断言能够支持的范围；
“待核验”没有被本次检查证明完成。官方入口路径可由
[来源清单](../../../examples/native/draw2d-official-examples/source-contracts.tsv)
逐项定位，完整源码目录见文末。每行链接指向对应 Rust 实现，文件内测试可追溯
该行的已覆盖行为。

## 25 个入口的预期与实现

| 序号与入口 | 官方预期画面及操作 | 当前覆盖 | 差异或剩余证据 |
|---|---|---|---|
| 01 HelloWorld | 一个 Hello World Label；Canvas 随窗口变化显示文字 | [实现](../../../examples/native/draw2d-official-examples/src/examples/hello_world.rs)保留 Label 与 200×100 初值，结构测试通过 | 目录 complete。额外白色包装 root 内的 Label 固定为 200×100，没有布局管理器；现有测试仅检查初值，需补 resize 后的尺寸、对齐和绘制核验。默认系统字体被 Inter 替代 |
| 02 ObstructionExample | 初始 5 个绿色矩形及 x/y/w/h 文字；空白左拖建障碍、两次右键建端点/路线；拖动重新寻路；右键删障碍、中键删端点 | [实现](../../../examples/native/draw2d-official-examples/src/examples/obstruction.rs)有真实输入、组求解、障碍生命周期、待配对端点和成对删除；4 个交互测试覆盖这些路径 | 目录 complete。初始无连接正确；成对删除修正官方 parent mismatch，属于已记录差异。showSegs 在官方和当前均只翻转状态，不能要求它额外显示未被官方绘制的网格 |
| 03 ConnectionAndAnchorExample | 两个 GroupBox、4 个彩色矩形、2 条箭头连接；Midpoint、两个 Endpoint 标签和说明；拖动节点时锚点与标签跟随 | [实现](../../../examples/native/draw2d-official-examples/src/examples/connection_and_anchor.rs)覆盖组成、owner-aware locator、标签测量与偏移面板内拖动重路由 | 目录 complete。已有代表性 Native 图像；字体和系统颜色使用便携替代，不能据此宣称所有系统主题逐像素等价 |
| 04 CompoundGraphDemo | 10 个图输入、SOUTH/EAST 两方向；蓝色子图边框先于节点/边绘制；图模型可嵌套，Figure 视觉平铺 | [实现](../../../examples/native/draw2d-official-examples/src/examples/graph_examples.rs)经 GraphProjection 求解全部输入；真实面板切换、替换与释放、跨组边有测试 | 目录 complete。20 个组合属于既有算法验收；Tahoma 被 Inter 替代。代表性图像不能证明所有组合的窗口呈现均已逐帧验收 |
| 05 DirectedGraphDemo | 16 个图输入、SOUTH/EAST 两方向；灰底节点标签、入度为零节点粗边、灰色箭头边和虚拟节点折点 | [实现](../../../examples/native/draw2d-official-examples/src/examples/graph_examples.rs)覆盖全部输入、方向切换、标签资源和连接发布 | 目录 complete。32 个组合属于既有算法验收；没有额外节点拖拽要求。实际字体与平台呈现仍按证据范围报告 |
| 06 EllipseHitting | 4 个不同宽高的椭圆按 Flow 排列；鼠标进入椭圆变黄，离开变白；外接矩形的角落不算命中 | [实现](../../../examples/native/draw2d-official-examples/src/examples/ellipse_hitting.rs)覆盖官方尺寸公式、精确椭圆命中和 enter/exit 更新 | 目录 complete。结构与交互已有断言；连续窗口 resize 和旧新颜色的实际后端像素不能由命中断言代替 |
| 07 FlowLayoutExample | 11 个官方尺寸和颜色的 Shape；横纵方向、换行、主次轴对齐、两组独立 spacing、stretch 与恢复 | [实现](../../../examples/native/draw2d-official-examples/src/examples/flow_layout_example.rs)和真实面板覆盖矩阵、整数居中、不等 spacing、缩放窗口与自然尺寸恢复 | 目录 in_progress。连续真实窗口 resize、文字 minimum 消费者和 retained-partial 擦除仍未闭合 |
| 08 GridLayoutExample | 初始 2 个 Shape；Add 按 9 项序列循环、Clear；点选粗描边并编辑 columns、margin、spacing、span、grab、hint、indent、alignment | [实现](../../../examples/native/draw2d-official-examples/src/examples/grid_layout_example.rs)覆盖实际面板、选择释放、9 项序列、布局约束、非法输入和零空间恢复 | 目录 in_progress。不能用测试中的旧 mock 控件证明当前面板；已有独立真实面板测试。文字约束、连续窗口 resize 和 retained-partial 像素仍待验 |
| 09 ToolbarLayoutExample | 5 个 Shape 单行/单列排列；方向、minor alignment、spacing、stretch、收缩与恢复；方向切换仅在 stretch 时 reset | [实现](../../../examples/native/draw2d-official-examples/src/examples/toolbar_layout_example.rs)覆盖自然尺寸、方向/stretch/恢复和真实面板驱动 | 目录 in_progress。文字 minimum、连续窗口 resize 和 retained-partial 擦除仍待验 |
| 10 PathExample | 红色闭合四点路径旋转 45°、线宽 3；旋转 zoom 文字、下划线与选字；滑块公式 `(value+50)*0.02f` | [实现](../../../examples/native/draw2d-official-examples/src/examples/path_example.rs)覆盖闭合/旋转、fill/outline 各一次文字绘制、selection clip、字体指标下划线、SchemeBorder 和真实按钮缩放 | 目录 complete。README 的旧 in_progress 描述已纠正。跨字体段通用装饰仍未提供，但不属于这个单字体静态入口的缺失功能 |
| 11 PrintExample | 三个字体标签、矩形/椭圆/圆角矩形、Draw2D Button、2 个连接节点、连接和 brazil.ico；图文可输出 | [实现](../../../examples/native/draw2d-official-examples/src/examples/printing.rs)有 11 个 children、真实 ICO、字体/图像命令与 resize 重路由 | 目录 in_progress。Helvetica、Times New Roman、Perpetua 都被 Inter 替代；字号还需统一 pt 到逻辑 px 的映射。实际打印对话框/作业不属于本范围 |
| 12 ScrollPaneExample | 两个同位置重叠的 InternalFrame；一个 Toolbar/Flow 与 George 图像内容，一个固定 100×100；拖标题、八向 resize、自动滚动条及 cursor | [实现](../../../examples/native/draw2d-official-examples/src/examples/scroll_pane_example.rs)覆盖两个 frame、标题/边框、真实 ScrollPane、图像、拖动和 resize | 目录 in_progress。主题与完整视觉验收待完成。初始前面的 frame 遮住后面的 frame 符合上游相同初始 bounds，移动前面的 frame 后再检查后者 |
| 13 InheritBackgroundExample | 两个 Canvas 同样文字；左侧橙色自绘背景，右侧透明并继承宿主白/绿/白渐变；滚动时背景相位正确 | [专用宿主](../../../examples/native/draw2d-official-examples/src/inherit_background.rs)与两个 Runtime 实现背景输入、滚动、移动、DPI、移除和重建 | 目录 complete。必须从专用二进制验收；默认共享场景的单 Runtime 不能证明双 Canvas 背景继承 |
| 14 ShapeStylesExample | 5 种 Shape；8 个样式轴任取 X/Y 构成矩阵；每单元先施加 X 再 Y；含 AA、零宽、低 miter、dash offset、alpha 和 disabled | 当前无工厂，矩阵消费者未实现 | 目录 blocked。普通 stroke/alpha API 和独立 AA 实验不能替代此入口；AA OFF、hairline、miter 0/0.5 仍缺完整生产契约与后端证据 |
| 15 BidiBlockExample | 两侧各 10 个段落；分别比较默认 Canvas 与 RTL 镜像 Canvas；段落 LTR/RTL 和相对/绝对对齐共存；共享纵向 range | [实现](../../../examples/native/draw2d-official-examples/src/examples/bidi_block.rs)有独立内容、十段方向/对齐与共享 range | 本轮改为 in_progress。右侧与左侧创建过程相同，没有 Canvas 镜像；现有测试未检查两侧镜像差异 |
| 16 CaretExample | 多语种文字、蓝底、10 单位边框和 begin/end FlowBorder；移动鼠标更新 offset/trailing、光标位置及选择 | [实现](../../../examples/native/draw2d-official-examples/src/examples/text_interaction.rs)有 FlowSnapshot 命中、selection/caret 几何、准备结果和实际绘制；[边框](../../../examples/native/draw2d-official-examples/src/examples/text_test_border.rs)有字体测量 | 目录 complete 仅有功能范围证据。Tahoma 被 Inter 替代；当前透明蓝选区与上游 XOR 绘制不同；选择边界、行尾、组合文字和 affinity 尚无完整对照断言，见下文 |
| 17 MultiLineLabelExample | 只读可换行文字；需要时显示滚动条；可聚焦、键盘滚动、焦点框及 accessibility | [实现](../../../examples/native/draw2d-official-examples/src/examples/text_examples.rs)组合 ScrollPane 和 FlowPage；[交互](../../../examples/native/draw2d-official-examples/src/examples/text_interaction.rs)支持焦点、方向键和可访问名称 | 目录 complete。下方 SWT Text 比较区已按范围排除；现有测试只明确验证纵向键盘滚动，横向溢出、字体变化和镜像方向键需要补验 |
| 18 TextFlowExample | 6 个段落：标题、普通换行、只按 newline 断行代码、嵌套蓝色粗斜体 InlineFlow、10 字号 baseline、右对齐 | [实现](../../../examples/native/draw2d-official-examples/src/examples/text_examples.rs)保留真实结构和各段样式；flow 快照能够稳定发布 | 目录 complete。Helvetica/Courier 替代为 Inter/JetBrains Mono；综合稳定性测试没有逐一断言六段的字体样式、baseline、换行与每段像素 |
| 19 TextFlowLargeExample | 20 组普通 block 加 20 个居中字号 block；400 个字号片段与嵌套句子；首 target 在首 block，字母/空格追加，数字忽略，纵向滚动条 Always，tooltip | [结构](../../../examples/native/draw2d-official-examples/src/examples/text_examples.rs)和[交互](../../../examples/native/draw2d-official-examples/src/examples/text_interaction.rs)覆盖 40 段、首来源追加、其他段不变与滚动条配置 | 目录 complete。target 最终归属直接构造，未在示例中重演上游先 add 再 reparent 的中间步骤；tooltip 属性存在不等于宿主显示已验。未在本次做输入耗时/增量成本基准 |
| 20 TextSurroundingFigureExample | 文字围绕两个真实 FlowAdapter；第一组 Toolbar 含圆角矩形/三角形/椭圆；第二组 Stack 内 RTL 文字；窄宽重排 | [实现](../../../examples/native/draw2d-official-examples/src/examples/text_surrounding.rs)保留两个 adapter 和真实子树；测试验证窄宽高度增长及稳定测量 | 目录 complete。没有将图形替换成图片；完整 baseline、clip、子图命中与 partial damage 像素仍需独立验收。圆角参数已按 Core 的 corner dimensions 语义核对，未凭参数名误判为双倍半径 |
| 21 ThumbnailExample | 主图绿色/蓝色矩形可拖；overview 实时同步并保持纵横比；关闭 overview 释放订阅 | [实现](../../../examples/native/draw2d-official-examples/src/examples/thumbnail.rs)的实际 panel 持有活源 Runtime；两处同步、resize 不重新捕获和取消订阅有断言/既有 Native 图像 | 目录 complete。官方两个 Shell 被同窗主视图/overview 承载，属于已接受宿主变体；单 Runtime 工厂不能独自证明 live overview |
| 22 SimpleTreeExample | Graph Root、2 个 child 和 1 个孙节点；Normal/Hanging 混合、LEFT 对齐、PageNode 折页/渐变、透明层先于标题 | [实现](../../../examples/native/draw2d-official-examples/src/examples/simple_tree_example.rs)覆盖 4 分支、真实布局、层次顺序、hanging 缩进及横纵转置 | 目录 complete。没有官方面板或动画开关要求；不能因名称为 Tree 就额外要求完整编辑器操作 |
| 23 TreeExample | 19 分支；点选、双击展开/折叠、增减子节点、方向/压缩/间距/样式/对齐；默认关闭的可选动画 | [实现](../../../examples/native/draw2d-official-examples/src/examples/tree_example.rs)与[动画](../../../examples/native/draw2d-official-examples/src/examples/tree_animation.rs)覆盖实际浮动面板、双击、同源连线、反向、输入接管、隐藏/释放 | 目录 complete。已有 animation.tree 的 GPU Partial/Full 对照；这不关闭跨平台和真实 surface present 的发布验收 |
| 24 UMLClassDiagram | 两个三段类、聚合菱形连接、end1 与 connection 标签；90 单位宽便笺自适应高度和 DogEar 边框 | [实现](../../../examples/native/draw2d-official-examples/src/examples/uml_class_diagram.rs)覆盖图标、组成、owner-side locator、标签文字重新测量及便笺窄宽测量 | 目录 in_progress。已知 DogEar 外框宽高减一与 5/6 单位折角不一致；字体、图标与窄宽视觉验收待完成 |
| 25 ZoomExample | Table/Column 两个三段类及 SVG 图标；真实 `< Zoom >` 滑块，初值 40、extent 25，公式 `(value+10)*0.02f` | [实现](../../../examples/native/draw2d-official-examples/src/examples/zoom_example.rs)覆盖 range、arrow/page/drag、最小/最大/恢复、资源密度更新与实际面板输入仲裁 | 目录 in_progress。跨显示器 DPI、Arial 实际字体及完整窗口视觉验收仍未完成。源码的 createThumbnail helper 未被 main 调用，不应计为漏做功能 |

## Bidi 的遗漏有源码证据

官方 `BidiBlockExample.run` 以默认样式创建左侧 FigureCanvas，以
`SWT.RIGHT_TO_LEFT` 创建右侧 FigureCanvas，两侧调用同一 `createContents`。
因此，重复创建相同十段文字只是内容要求，右侧的宿主镜像也是场景变量。

Draw2D 的 `LightweightSystem.RootFigure.isMirrored` 从 Canvas 的 `SWT.MIRRORED`
读取镜像状态。`BlockFlowLayout.addBelowPreviousLine` 先按段落方向解释相对对齐，
再按 `isMirrored` 翻转布局坐标中的左右；`LineRoot` 同样接收镜像状态。
`BlockFlow.validate` 对 LTR 段落和 mirrored Canvas 的组合还会添加 bidi 控制字符。
这些调用链表明 Canvas mirror 与单段 RTL shaping 是不同的契约，不能相互代替。

当前 Rust 的 `for pane in panes` 对两侧使用完全相同的 page、block direction 和
alignment 设置，未传递左右 Canvas 的差异。测试只检查每侧 10 段、部分 direction/
alignment 和共同滚动到 y=20，没有镜像、bidi run、视觉左右或滚动条位置断言。
测试通过不能证明右侧 RTL Canvas 已对等。

后续先冻结 Canvas 镜像、段落方向、相对/绝对对齐、输入坐标和滚动条侧别的契约，
再决定 Core/宿主的最小修改范围。不能通过将全部文字倒序或简单交换 alignment
模拟镜像。此次只修正完成登记，不实施未经评审的坐标或文字管线变更。

来源：[官方 BidiBlockExample](https://github.com/eclipse-gef/gef-classic/blob/4463d9d0ce13c19d10fbe769d29f28b7345a8cba/org.eclipse.draw2d.examples/src/org/eclipse/draw2d/examples/text/BidiBlockExample.java)、
[LightweightSystem](https://github.com/eclipse-gef/gef-classic/blob/4463d9d0ce13c19d10fbe769d29f28b7345a8cba/org.eclipse.draw2d/src/org/eclipse/draw2d/LightweightSystem.java)、
[BlockFlow](https://github.com/eclipse-gef/gef-classic/blob/4463d9d0ce13c19d10fbe769d29f28b7345a8cba/org.eclipse.draw2d/src/org/eclipse/draw2d/text/BlockFlow.java)、
[BlockFlowLayout](https://github.com/eclipse-gef/gef-classic/blob/4463d9d0ce13c19d10fbe769d29f28b7345a8cba/org.eclipse.draw2d/src/org/eclipse/draw2d/text/BlockFlowLayout.java)。

## Caret 不能只核验命中点

当前 `InteractiveDrawing.paint` 根据命中位置查找下一 Unicode 字符边界，绘制透明
蓝色选区，再绘制文字和黑色光标。上游 `TextFlow.paintSelection` 使用 white/XOR。
两者绘制策略不同，需记录为视觉差异并裁定可接受变体，不能说已逐像素对等。

上游 `setSelection` 注释称范围 inclusive，但同一源码的 `paintSelection` 部分分支
使用 `selectionEnd - 1` 和严格大于条件。此前计划将 inclusive 注释直接写成已确定的
高亮结果，证据不足。此次不宣称 `(offset, offset+1)` 必然选中两个字符，也不宣称
当前按一个 Unicode scalar 扩展必然等价；应按 ASCII、行尾、RTL、组合字符和
UTF-16 surrogate 边界分别核验选择几何与 trailing/affinity。

现有 `mouse_move_uses_admitted_fragment_and_preserves_source_geometry` 只验证命中可查、
caret 高度大于零、输入后源几何不变及无延迟错误，没有断言最终选择范围或实际绘制
命令。因此保留其功能完成证据，同时将选择对等列为未裁定项。Hit info 原生 Label
不属于 Core 功能，但可作为后续核验时的宿主观测工具。

来源：[官方 CaretExample](https://github.com/eclipse-gef/gef-classic/blob/4463d9d0ce13c19d10fbe769d29f28b7345a8cba/org.eclipse.draw2d.examples/src/org/eclipse/draw2d/examples/text/CaretExample.java)、
[TextFlow 的实际选择绘制](https://github.com/eclipse-gef/gef-classic/blob/4463d9d0ce13c19d10fbe769d29f28b7345a8cba/org.eclipse.draw2d/src/org/eclipse/draw2d/text/TextFlow.java)、
[当前交互与绘制](../../../examples/native/draw2d-official-examples/src/examples/text_interaction.rs)。

## 字体和视觉差异要跨入口核验

`text_examples::font` 将 Arial、Helvetica、Tahoma、Courier 分别替换成 Inter 或
JetBrains Mono，影响 Caret、TextFlow、TextFlowLarge、TextSurrounding 和
MultiLineLabel 等消费者；CompoundGraph 也未使用官方 Tahoma。
因此，真实字体差异不止存在于 Print、UML 和 Zoom。

Print 还将官方 16/24/48 的 SWT Font 高度直接写成 16/24/48px。其他文字消费者
已对部分显式 point size 使用 4/3 换算。应先按目标宿主固定字体单位、DPI 和
逻辑坐标映射，再核验字号、字重、fallback face、baseline、换行和 ink bounds，
不能只保持字体名称 Label 就说指标一致。

字体替代可能是便携样例的合理变体，但它需要明确声明；若目标是真实字体对等，
宿主必须提供实际 face/font bytes，并核验解析结果。字体资源体积、按需加载和
首次加载成本仍按项目要求评估，不将所有系统字体直接打包进 Core。

结构指标、glyph count 和快照稳定只能证明对应步骤。窗口连续 resize、Partial
擦除、阴影/边框、实际 tooltip、系统颜色和 surface present 需要各自的后端或宿主
证据。三个布局已有 full-frame GPU 和 CPU damage 记录，不能将它们换算成
retained-partial 全部通过。

来源：[当前字体映射](../../../examples/native/draw2d-official-examples/src/examples/text_examples.rs)、
[Print 字体初始化](../../../examples/native/draw2d-official-examples/src/examples/printing.rs)、
[官方 AbstractExample 字体](https://github.com/eclipse-gef/gef-classic/blob/4463d9d0ce13c19d10fbe769d29f28b7345a8cba/org.eclipse.draw2d.examples/src/org/eclipse/draw2d/examples/AbstractExample.java)、
[布局消费者证据](k02-layout-consumers-evidence-2026-10-23.md)。

## 验证范围与后续顺序

本轮运行官方样例 crate 的库级测试和来源清单测试，并执行文档门禁、格式及
diff 检查。结果在本节末尾登记。来源测试只验证 25 入口、路径和依赖清单，
没有逐功能行为断言；库级测试也不能填补前述断言空白。

本轮没有新跑 25 个入口的窗口截图，没有运行官方 Java 或进行跨显示器 DPI 验收。
既有代表性 Native 图像与 Tree 的 GPU 像素记录只按其实际范围使用：
[图布局](../../../verification/evidence/adr043/implementation.md)、
[连接/Path/SimpleTree 等](../../../verification/evidence/official-examples-2026-10-23/implementation.md)、
[Thumbnail](k07-4-thumbnail-evidence-2026-10-23.md)、
[双 Canvas 背景](k07-6-inherit-background-evidence-2026-10-23.md)、
[Tree](tree-layout-presentation-implementation-2026-10-26.md)。

后续优先裁定 Bidi 镜像和 Caret 选择契约，补 HelloWorld resize 与文字入口的逐功能
断言；再补三种布局、ScrollPane、UML 的实际视觉证据。字体/DPI 走宿主链路，
ShapeStyles 的 AA、hairline 和低 miter 分别按既有提案评审。
执行顺序仍由[P2 backlog](../../roadmap/p2-delta-backlog.md)统一维护，本记录不新增
并行执行批次，不撤销 ADR-042/K07 等已经通过的通用能力证据。

官方 25 入口与 helper 的同源事实见
[固定源码目录](https://github.com/eclipse-gef/gef-classic/tree/4463d9d0ce13c19d10fbe769d29f28b7345a8cba/org.eclipse.draw2d.examples/src/org/eclipse/draw2d/examples)；
同源文件不被计作相互独立的外部证据。

本轮已取得的执行结果：

| 验证 | 结果 | 证明范围 |
|---|---|---|
| `cargo test -p draw2d-official-examples --lib -- --test-threads=4` | 85 passed，0 failed，0 ignored | 当前样例库已有断言全部通过，含 Obstruction 的 4 项交互和修订后的目录计数 |
| `cargo test -p draw2d-official-examples --test source_contracts` | 2 passed，0 failed | 25 入口、源码路径、Canvas 完成清单与依赖身份一致 |
| `cargo fmt --all -- --check` | 通过 | 本次状态登记改动符合 Rust 格式 |
| `git diff --check` | 通过 | 当前差异无空白格式错误 |
| 中文成稿机械扫描及回读 | 0 处半角中文标点、破折号或空泛元话语命中 | 保留源码标识符、命令和数字；再次核对“发现差异”与“缺少验证”的措辞 |
