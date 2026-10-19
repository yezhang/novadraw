# Draw2D 官方示例对等实现审计

类型：`verification`

更新日期：2026-10-19

参考基线：`eclipse-gef/gef-classic@4463d9d0ce13c19d10fbe769d29f28b7345a8cba`

## 当前结论

官方插件的 25 个 `main` 入口是本次核对的完整分母。文章用于定位示例，业务契约以
`org.eclipse.draw2d.examples/src/org/eclipse/draw2d/examples/` 的 Java 源码及其调用的
Draw2D 基础类为准。[官方源码](https://github.com/eclipse-gef/gef-classic/tree/4463d9d0ce13c19d10fbe769d29f28b7345a8cba/org.eclipse.draw2d.examples/src/org/eclipse/draw2d/examples)

原先复用 `novadraw-example-scenes` 的近似实现已移除。独立 crate 只依赖 `novadraw`
和示例宿主 support，不依赖已有业务场景。当前保留 11 个独立移植入口，其中 9 个仍有
明确差额，不能宣称全部完成。

| 状态 | 数量 | 判定 |
|---|---:|---|
| `complete` | 2 | 已核对基础结构和行为 |
| `in_progress` | 9 | 有独立可运行实现，尚未闭合全部官方语义 |
| `not_implemented` | 1 | 尚未完成移植，不等同于 Core 缺能力 |
| `blocked` | 13 | 已确认具体算法、API 或平台契约缺口 |

代码状态入口：[catalog](../../../examples/native/draw2d-official-examples/src/lib.rs)。
所有 `in_progress` 条目会列出差额；无实现的条目不绑定替代 factory。

## 逐项对照

下表路径均相对于官方 `org/eclipse/draw2d/examples/`。Rust 路径相对于
`examples/native/draw2d-official-examples/src/examples/`。

| 官方名称与路径 | 状态 | 已实现或待移植的官方契约 | 具体差额或能力缺口 |
|---|---|---|---|
| HelloWorld · `HelloWorld.java` | complete | `hello_world.rs`；单个 Hello World Label | 无已知业务差额；统一浏览宿主尺寸与原 Shell 不同 |
| EllipseHitting · `hittest/EllipseHitting.java` | complete | `ellipse_hitting.rs`；四个椭圆、FlowLayout、精确命中、进入变黄和离开变白 | 结构及指针测试通过；跨平台字体和栅格差异未逐像素断言 |
| ConnectionAndAnchorExample · `connections/ConnectionAndAnchorExample.java` | in_progress | `connection_and_anchor.rs`；两组 GroupBox、四个可拖动节点、ChopboxAnchor、Midpoint/Endpoint Locator、目标 Decoration | 已测试拖动和重路由；官方双侧截图对照、完整 Locator 几何仍待验证 |
| ObstructionExample · `cg/ObstructionExample.java` | not_implemented | Java 固定随机初始障碍、左键拖出新障碍、右键成对创建端点、拖动重建 Path、删除清理 | 已核对：官方 visibility graph 允许斜线，按共享拐角计数分配路径间距；Core 的 Hanan grid 正交路由不是相同算法。官方算法、生命周期和交互仍待移植 |
| FlowLayoutExample · `layouts/FlowLayoutExample.java` | in_progress | `flow_layout_example.rs`；11 个官方 Shape、方向、主次轴对齐、拉伸、两组间距控件 | 已补黑色描边和官方 RGB；Scale 键盘语义、原生控件外观、窗口 resize 后布局仍待验证 |
| GridLayoutExample · `layouts/GridLayoutExample.java` | in_progress | `grid_layout_example.rs`；初始两个 Shape、九项形状/颜色循环、Add/Clear、选中线宽、列数/等宽/边距/间距、九项 GridData | 下拉选择已补；SWT 可编辑 Combo、Text 光标/选择/删除、FocusOut 与 Shell.pack 的完整行为尚未对等；Grid 算法边界仍须对照 |
| ToolbarLayoutExample · `layouts/ToolbarLayoutExample.java` | in_progress | `toolbar_layout_example.rs`；五个官方 Shape、方向、拉伸、对齐、spacing | 描边已补；stretch/resetShapes、Scale 键盘和窗口布局待验证 |
| ShapeStylesExample · `swt/ShapeStylesExample.java` | blocked | 官方两轴矩阵含八轴、五种 Shape、默认线宽；先应用 X 再应用 Y | Render IR/Vello 没有逐 Shape 的 ON/OFF/DEFAULT 抗锯齿模式；不能把固定样式展示当完整矩阵 |
| InheritBackgroundExample · `swt/InheritBackgroundExample.java` | blocked | 两个 FigureCanvas 对比自绘背景与继承 SWT 父渐变 | Winit 宿主无原生 Composite 背景继承契约 |
| DirectedGraphDemo · `graph/DirectedGraphDemo.java` | blocked | GraphTests fixture 选择、两方向、rank、虚拟节点和边路径 | P2-L01 DirectedGraph 布局未实现；固定坐标不等价 |
| CompoundGraphDemo · `graph/CompoundGraphDemo.java` | blocked | CompoundGraphTests、嵌套组、padding/insets、跨组边 | P2-L01 CompoundDirectedGraph 模型和布局未实现 |
| PathExample · `path/PathExample.java` | blocked | 45° 旋转闭合 Path、旋转 bounds、ScrollBar 缩放、自定义 ZoomFigure | PathFigure 每次 fill/outline 还绘制部分下划线、选择背景的 SWT TextLayout；fragment 装饰和原 ScrollBar 完整协议缺失 |
| ZoomExample · `zoom/ZoomExample.java` | in_progress | `zoom_example.rs`；Table/Column 三段式类、字段/方法图标、XYLayout、StackLayout、带文字 thumb、步进/翻页/拖动 | ScrollBar 的五子节点构成、SchemeBorder、REPEAT_FIRING 未移植；SVG 当前使用上游 PNG 替代资源，不能证明缩放对等 |
| ScrollPaneExample · `scrollpane/ScrollPaneExample.java` | in_progress | `scroll_pane_example.rs`；两个重叠 InternalFrame、组合标题边框、Toolbar/两个 FlowPanel、十八个 George Label、固定 100×100 内容、tracksWidth、自动滚动条、标题移动和八方向 resize | 动态方向光标缺少事件期更新接口；Core ScrollBar 子节点构成和主题仍与官方不同；GroupBoxBorder 与字体仍须双侧视觉核对 |
| ThumbnailExample · `thumbnail/ThumbnailExample.java` | blocked | 主窗口与 overview 共享同一可变 Figure 树，拖动实时反映 | 第二实时投影及双窗口生命周期契约缺失；复制场景不等价 |
| BidiBlockExample · `text/BidiBlockExample.java` | blocked | 两侧 LTR/RTL Canvas、每侧十段、相对/绝对对齐、共享纵向 range | per-block base direction、ALWAYS_LEFT/RIGHT、双 Canvas 共享 RangeModel 的公开组合接口缺失 |
| CaretExample · `text/CaretExample.java` | blocked | 混合脚本、offset/trailing 查询、单字符 selection、原生 Caret、Hit info | Core 已有文本交互几何；原生 caret overlay 和事件期查询/高亮链路尚无完整宿主实现 |
| MultiLineLabelExample · `text/MultiLineLabelExample.java` | blocked | MultiLineLabel 与只读 SWT Text 对照、换行及必要滚动条 | MultiLineLabel widget 和原生多行 Text 比较宿主缺失 |
| TextFlowExample · `text/TextFlowExample.java` | blocked | 六段结构、源码只按换行断行、嵌套 InlineFlow、继承颜色、粗斜体和不同字号 baseline | P2-T03 fragment 字体/颜色及嵌套 InlineFlow 缺失 |
| TextFlowLargeExample · `text/TextFlowLargeExample.java` | blocked | 20 组 nested inline/block、混合字体、仅字母/空格 fake typing | fragment 字体和官方嵌套流结构缺失；整体文本可变不等价于 fragment 级 mutation |
| TextSurroundingFigureExample · `text/TextSurroundingFigureExample.java` | blocked | FlowAdapter 内嵌三 Shape、文本、第二个 RTL FlowPage | 任意 Figure 子树作为 inline object 的 FlowAdapter 协议缺失 |
| SimpleTreeExample · `tree/SimpleTreeExample.java` | in_progress | `simple_tree_example.rs`、`tree_layout.rs`；Graph Root、Child 1、hanging Child 2 及其 Child 1；AnimatingLayer、PageNode/Label、折页组合边框、normal/hanging 行高和轮廓算法 | 已通过结构、行高、方向、轮廓接触及压缩测试；宿主背景尚为灰色，官方白色 Canvas、字体和双侧视觉仍待核对 |
| TreeExample · `tree/TreeExample.java` | in_progress | `tree_example.rs`；19 个节点、五根分支、选择目标、方向/对齐/样式/压缩/间距、整数 Scale 和 increment=5、动态间距标签、删除最后一个子分支、非动画双击与按钮折叠 | More Children、Use Animation、选中渐变/白色文字、SWT Group 和原生面板尚未对等。Core 已有 bounds transaction，但事件回调没有构造完整新子树及调用该 transaction 的入口 |
| UMLClassDiagram · `uml/UMLClassDiagram.java` | in_progress | `uml_class_diagram.rs`；双类、association diamond、end1/middle Label、StickyNote；已补 Header MarginBorder、7×3 diamond 和宽度驱动便笺高度 | ConnectionEndpointLocator 缺 owner-side/child-size 定位契约；字体指标、DogEar 边框及双侧视觉仍待核对 |
| PrintExample · `printing/PrintExample.java` | blocked | 多字体/图元/Button/Image/Connection 预览、PrintDialog/Printer/PrintFigureOperation | P2-O01 输出目标和打印资源生命周期 deferred；截图不等价于打印 |

## 本批实现证据

TreeExample 保留官方 19 个节点，根子序列为 Normal Style、Child、Normal Style、
Child、Normal Style；两个 hanging 分支及三个/两个 child 子组逐一对应 Java fixture。
布局模型增加 expanded 和 active 状态，折叠同时隐藏 AnimatingLayer、缩减行高/轮廓，
删除只移除所选分支最后一个子树。新增结构与真实鼠标操作测试通过，覆盖选中、
normal→hanging、折叠/展开和删除后布局，不用直接调用按钮闭包替代事件链路。

Native 截图为 `draw2d-official-examples_TreeExample_1792384257.png`，白色 Canvas、
十九个折页节点及两种连线已检查。直接编译上游 tree 包和 AbstractExample，通过
反射调用未经修改的 createContents，生成
`target/visual-verification/official/TreeExample-official.png`、
`SimpleTreeExample-official.png` 及两份逐节点 geometry 记录。截图使用上游 SWT
默认字体、相同 850×780 树区域；Rust 使用内置字体，仍不能声称逐像素等价。
官方面板没有包含在这次离屏截图中。

ObstructionExample 核对已追溯到 graph.ShortestPathRouter/Path/Obstacle/Vertex。
官方采用 visibility graph、障碍顶点增长和共享拐角间距；现有 Core 为正交搜索，
算法差额已补入 api-coverage。上游 handleRemoveEllipseDragFigure 调用
getParent().remove(otherFigure)，而 otherFigure 是 TestFigure 的孩子，存在父级不匹配；
移植时应记录该行为并验证，不能擅自改成理想化的成对删除。
官方双路径探针得到 `(0,50)→(36,83)→(63,83)→(100,50)` 和
`(0,55)→(32,87)→(67,87)→(100,55)`，原始输出为
`target/official-shortest-path-probe.log`。初始五个障碍的 Java Random(0) 高度为
87、82、86、85、85。

SimpleTree 保留官方四个分支，每个分支先添加 AnimatingLayer，再添加 PageNode，
PageNode 使用 StackLayout 和单个 Label。折页边框保留 `(10,2,4,4)` insets，
并组合 `(4,4,8,3)` MarginBorder；渐变、折角和阴影 RGB 均取自 PageNode.java。
独立布局扩展实现 normal/hanging 行高传播、轮廓合并、深层接触的五像素间隔、
负向重叠修正和远端兄弟空隙分配，不依赖已有业务树样例。

树相关新增六项测试通过。Native 已生成
`draw2d-official-examples_SimpleTreeExample_1792381958.png`，已检查折页节点和
两种连线。灰色宿主背景与官方白色 Canvas 不同，仍保留 in_progress。

ScrollPane 的两个 InternalFrame 均保留初始 `(20,20,200,140)`，按下时不置顶，
因为官方 `count` 没有递增，置顶语句也已注释。标题栏按官方不对称 padding 绘制，
CompoundBorder 保留 raised、title、三像素 ring 和 lowered 的组合。每个 FlowPanel
保留 `Label 0..4,6..9`，George 是上游单帧 32×32 GIF 的无损 PNG 转换。
移动和八方向缩放遵守官方最小宽 100、高 22，事件通过 Runtime 延迟修改 bounds。

Core 的 ScrollPaneFigure 新增通用 `BORDER` capability，外部示例通过现有
`ReplaceFigureBorder` 挂载 GroupBoxBorder。没有插入替代容器节点，布局继续消费
标准 client area。新增四项测试覆盖 Figure 构成、八方向公式、标题拖动不改 Z-order，
以及缩小/增高时滚动条自动显隐；均已通过。

Native 验证曾拒绝标题 TextLayout，原因是临时 Runtime 与最终 Runtime 的资源 namespace
不同。已将标题测量和 Border replacement 移到最终 Runtime，并让测试明确拒绝
`FramePreparation::Error`。修复后生成 `ScrollPaneExample_1792381469.png`，已检查
固定内容裁剪、标题和嵌套边框；另一个窗口按官方初始位置被完全遮挡。

`GridLayoutExample` 的事件回调直接修改 parent-owned `GridConstraint`；全部九类属性有
对应控件。选中时线宽变为 4，换选后旧 Shape 恢复 1，取消选择和 Clear 会禁用属性面板。
形状序列和 RGB 来自官方 `createShape()` 与 `ColorConstants`，不来自已有业务场景。

Native 截图曾发现 Grid Shape 填充色为空。原因是自定义 Figure 委托内置 Shape 绘制时，
未向 NodeState 声明初始背景，渲染继承了宿主白色。补充初始前景/背景后，截图显示
蓝色椭圆和红色矩形。宿主改用已有 AppBuilder 尺寸接口，保证全部控件可见。

Zoom 的 RangeModel 保留 `minimum=0`、`maximum=200`、`extent=25`、`value=40`。
公式按 Java float 运算保留 `(value + 10) * 0.02f` 的舍入，最大可用 value 为 175。
同步和延迟缩放共用 `RuntimeViewports::set_scale`，没有另建修改 ScaleModel 的路径。

新增 [延迟缩放契约测试](../../../novadraw/tests/p2_deferred_scale.rs) 覆盖外部 Figure
回调和非法 scale 拒绝；非法值不会改变原 scale。该增量涉及 `viewport.scroll_zoom`
语义族，已有账本明确把 ScrollBar REPEAT_FIRING 留给后续 repeat scheduler。

UML 的便笺宽度从 90 增到 160 后，布局测量高度随换行减少，定向测试通过。
新版截图为 `UMLClassDiagram_1792380464.png`。同次检查发现 `end1` 与类内容重叠：
当前 EndpointLocator 使用切线/法线偏移，上游 ConnectionEndpointLocator 使用 owner
quadrant、child preferred size 与默认 vDistance=4。该 API 差异已补到语义账本，
不能以调整示例固定坐标替代。Middle Label 的 opaque 设置也已补回。

## 验证记录与限制

已观察到的结果：

- `cargo test -p draw2d-official-examples --lib`：33 项通过。
- `cargo test -p novadraw --test m8_viewport_contract`：26 项通过。
- `cargo test -p novadraw --test p2_deferred_scale`：2 项通过。
- `cargo clippy -p draw2d-official-examples --lib -- -D warnings`：通过。
- `cargo run -p draw2d-official-examples -- --screenshot-all`：8 个场景成功生成截图。
- `cargo run -p draw2d-official-examples -- --screenshot=ScrollPaneExample`：新移植入口截图成功。
- `cargo run -p draw2d-official-examples -- --screenshot=SimpleTreeExample`：树入口截图成功。
- `cargo test -p draw2d-official-examples --lib sticky_note_height`：新增便笺测量测试通过。
- `cargo xtask docs`：328 个 Markdown 文件通过；`git diff --check` 通过。
- `cargo clippy --workspace -- -D warnings`：通过。
- `cargo test --workspace`：1,074 项通过、1 项 ignored。补回 SetScale 的
  topology 测试辅助 match 分支；xtask 首次失败来自嵌入旧临时检出路径的测试缓存，
  包级 clean 后重建通过，没有修改 xtask 源码。日志为
  `target/official-examples-workspace-test.log`。
- `cargo check --workspace`：通过。

截图输出位于 `target/visual-verification/screenshots/`，本批文件时间戳为
`1792380145` 至 `1792380151`。已检查汇总图中的填充、图标、控件可见性和嵌套文字。
这不是官方与 Novadraw 的同尺寸、同字体逐像素比较，也不是全部交互的 Native 验收。
workspace fmt、Clippy 和 test 已通过；官方功能差额仍未闭合，不能据此升级全部示例状态。

## 继续推进的顺序

继续关闭九个独立移植条目的实际差额，并移植尚未实现的 ObstructionExample。对十三个 blocked
条目，应分别确定公开 API 或平台增量；不得为了增加完成数绑定近似业务场景。
每个条目升级 complete 前须补源码映射、行为测试及上游双侧视觉证据。
需要评审的后续 Core 边界已整理为
[剩余契约方案](../plans/draw2d-official-examples-remaining-contracts.md)。

## 参考资料

- [The Draw2d Examples - A Hidden Treasure](https://nyssen.blogspot.com/2010/12/draw2d-examples-hidden-treasure.html)
- [官方示例源码基线](https://github.com/eclipse-gef/gef-classic/tree/4463d9d0ce13c19d10fbe769d29f28b7345a8cba/org.eclipse.draw2d.examples/src/org/eclipse/draw2d/examples)
- [Draw2D 基础类源码](https://github.com/eclipse-gef/gef-classic/tree/4463d9d0ce13c19d10fbe769d29f28b7345a8cba/org.eclipse.draw2d/src/org/eclipse/draw2d)
- [Draw2D API 语义覆盖账本](../../parity/draw2d/api-coverage.md)
