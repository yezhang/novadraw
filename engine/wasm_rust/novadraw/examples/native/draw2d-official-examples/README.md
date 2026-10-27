# Draw2D Official Examples

本目录独立移植全部 25 个官方入口中的 Canvas / LightweightSystem 内容。
Figure 构成、布局、连接、文字、输入和图形资源按官方 Draw2D 语义实现，
不引用 `novadraw-example-scenes` 的近似场景。

SWT 原生 Text/Combo/Scale/Group、控制面板、Shell pack 和打印对话框不属于
引擎建设或对等验收范围。现有控制工具只是驱动场景参数的临时示例界面，
不代表 Novadraw 原生控件替代方案。Canvas 内真正的 Draw2D Figure 控件仍需实现。
不编写、修改、编译或运行 Java 验证代码，上游源码仅作只读参考。

```sh
cargo run -p draw2d-official-examples
cargo run -p draw2d-official-examples -- --list
cargo run -p draw2d-official-examples -- --screenshot=HelloWorld
cargo run -p draw2d-official-examples -- --screenshot-all
```

`src/lib.rs` 保存当前实现状态，未移植条目不绑定替代场景。
历史 complete 只保留当时证据范围，其他状态在 Canvas 功能验收后更新；
移除原生控件需求不会自动关闭尚未核对的 Canvas 差额。

窗口标题按完整官方目录显示固定序号，例如 `[01/25] HelloWorld`、
`[23/25] TreeExample`、`[25/25] ZoomExample`。独立 InheritBackground 窗口使用
`[13/25]`，ShapeStyles 使用 `[14/25]`；场景列表跳过独立宿主条目时不重编号。
`--list` 的首列是同一序号，后续列给出状态、ID、原始类名和上游源码路径。
截图继续使用原始场景名；数字截图参数仍是运行列表中从 0 开始的索引。

25 个入口的预期画面、操作和剩余证据见
[逐项功能复核](../../../doc/verification/reviews/draw2d-official-examples-functional-audit-2026-10-26.md)。
本次发现 BidiBlock 的右侧 Canvas 镜像遗漏，撤回该入口的完成标记；
该复核时目录登记 16 个 complete、8 个 in_progress、1 个 blocked。
complete 只表示已登记的 Canvas 功能范围，字体替代和未经执行的视觉验收需单独说明。

2026-10-27 BidiBlock 已通过通用 `FigureStyle.layout_direction` 给右侧 Canvas
提供镜像上下文。排版片段、caret 与选择几何同步换位，字形保持正常朝向；
纵向滚动条换侧，两侧继续共享纵向 RangeModel。
`cargo xtask verify text.layout-direction` 覆盖文字契约和方向/resize 的 GPU
Partial/Full 比较。上游与跨平台呈现仍须分别验收，入口计数暂不调整。

ShapeStyles 已接入 8 个原始参数轴、5 种图形及真实控制面板，图形
ON/OFF/DEFAULT 使用公开 Graphics API。低 miter 保留输入并按 Bevel 退化；
零宽明确采用 Cocoa profile，映射为 1 逻辑单位与上游 ZERO dash 数组，
控制面板说明该映射，不代表通用设备 hairline。
Rectangle/Ellipse 已按官方整数边界绘制，圆角使用圆弧，Triangle 使用官方 NORTH
布局；disabled 图形绘制偏移浅色层和原位深色层，采用便携白/灰配色。
完整上游栅格、系统主题与特殊描边仍待验收，入口保留 `in_progress`。验证入口：
`cargo xtask verify render.antialias`，其中包含 40 个离屏 GPU 矩阵场景及旧区域清理。
`cargo xtask verify render.low-miter` 覆盖受检状态与 ON/OFF 开闭反向描边像素。

Path 的范围下划线通过 `MeasureContext::prepare_underlines` 准备，
按实际 fallback 字体、字号、行与 bidi run 拆分，再由公开 Graphics 绘制。
`cargo xtask verify text.underlines` 覆盖混合字体、组合字符、资源替换及
收缩、恢复和移除后的 GPU Partial/Full 一致性。

HelloWorld 使用已有 StackLayout，文字区域随 Canvas 收缩、放大和恢复。
UML 便笺使用独立 DogEar Border，填充折角为 6 单位，描边折角为 5 单位，
描边外框宽高减 1。现有能力内的修正与后续 Core 门禁见
[能力补充方案](../../../doc/design/architecture/official-example-capability-increment-2026-10-26.md)
和[实施记录](../../../doc/verification/reviews/official-example-capability-increment-2026-10-26.md)。

TreeExample 的 Use Animation 默认关闭。单击标题会选中节点，并启用 Selected Node
控件；双击有子节点的标题可立即展开/折叠；
开启动画后，双击进入同一 queued toggle 管线，也可使用面板的 Expanded 控件。
共享 winit 宿主通过平台 `WinitPointerAdapter` 识别真实按下/释放序列中的双击，
无需应用手动调用 Core 的双击入口。`animation.tree` 同时覆盖平台识别、宿主投递
以及实际浮动面板输入归属、动画开/关两种 Tree 消费者回归。

`source-contracts.tsv` 保留官方入口/helper/resource 的只读来源清单；
`completion-contracts.tsv` 只登记 Canvas 完成范围。普通本地验证：

```sh
cargo test -p draw2d-official-examples --test source_contracts
```

当前计划见 [Canvas Core 计划](../../../doc/verification/plans/draw2d-official-examples-core-plan.md)。
本轮新增的 Java 探针、运行脚本和生成基线已撤销。

Thumbnail 的 Native entry 持有独立活源 Runtime；主视图转交真实鼠标输入，overview
只消费同一源快照。拖动源区的绿色或蓝色矩形，两处同步变化。宿主面板可调整 overview
宽高或取消订阅。主视图保持与源捕获域相同的逻辑尺寸；overview resize 不重新捕获源。
宿主将上游两个 Shell 的 Canvas 内容放在一个窗口，单 Runtime 工厂只返回活源树。

```sh
cargo test -p draw2d-official-examples --lib thumbnail
NOVADRAW_THUMBNAIL_EVIDENCE=1 cargo run -p draw2d-official-examples -- --screenshot-all
```

三阶段 Native 证据见
[K07-4 记录](../../../doc/verification/reviews/k07-4-thumbnail-evidence-2026-10-23.md)。
Thumbnail 的 Canvas 消费已完成。

InheritBackground 使用独立 Native 宿主，在同一窗口内拥有两个 Canvas Runtime。
左侧 root 绘制橙色，右侧 root 透明；宿主提供白色、浅绿色、白色的纵向渐变，
通过 Core 的受检背景完成入口进入统一帧包。两侧显示同一段官方文字。
滚轮滚动当前 Canvas 内容，Tab 切换焦点，方向键移动 Canvas，B 改背景，
X 移除或恢复背景，R 重建后端 session。

```sh
cargo run -p draw2d-official-examples --bin inherit-background
cargo xtask verify example.inherit-background
```

验证保存初始、滚动、移动、背景变化、重建、暂停恢复、DPI 与移除的 Native 图像。
主程序的 `--inherit-background` 和 `--screenshot=InheritBackgroundExample` 可转到该
二进制，需先运行 `cargo build -p draw2d-official-examples --bins`。
通用单 Runtime 场景列表只展示其透明 Canvas 内容；背景对等验收以独立宿主为准。

Obstruction 的组求解和交互按 [ADR-040](../../../doc/adr/adr-040-group-route-solving.md)
实现。初始画面只有 5 个绿色矩形及坐标文字，没有端点或连接，与官方初值一致。
运行后，背景左拖创建障碍；背景右键两次创建一对端点与连接；
左拖移动障碍或端点；障碍右键删除；端点中键删除整对，未配对端点中键只删除自身。
成对删除修正了官方示例的 parent mismatch，保留差异记录。

```sh
cargo test -p novadraw --test p2_group_route_contract
cargo test -p draw2d-official-examples --lib obstruction
cargo run -p draw2d-official-examples --example obstruction_verification
```

最后一个命令通过真实输入构造四阶段 Native 场景并截图，产物在
`target/visual-verification/screenshots/`。路线允许斜线与 Canvas 外折点，
不增加官方源码未规定的视口避让。

Zoom 的连续缩放验证使用真实 ScrollBar 按钮输入，依次截图初始、最小、最大、
再次最小四个状态。子布局使用反算后的逻辑区域，滚动条按自身最小尺寸保持厚度。
共享宿主按最新鼠标位置判断面板输入归属；鼠标离开面板后无需等待重绘即可按下滑块。
滑块捕获期间移入面板仍继续拖动，松开后停止缩放。无窗口回归覆盖实际 Zoom 面板、
平台按钮适配与 Core 捕获链路，窗口操作仍需人工核验。

```sh
cargo xtask verify example.zoom-input
cargo run -p draw2d-official-examples --example zoom_verification
```

Path 已接入旋转闭合路径、选字 clip 和两次文字绘制；最小/最大缩放使用同一
ScrollBar 的真实按钮输入验证。SchemeBorder 和字体指标下划线已完成，
入口状态为 `complete`；跨字体段下划线已按 ADR-050 接入通用准备与绘制协议。

```sh
cargo test -p draw2d-official-examples --lib path_example
cargo run -p draw2d-official-examples --example path_verification
```

DirectedGraphDemo 和 CompoundGraphDemo 通过 Core `GraphProjection` 显示全部官方
fixture，并支持 SOUTH/EAST 切换。场景只复用 `examples/scenes/` 中按源码声明的
graph fixture 输入，不复用近似画面或预先计算的布局。Label 与 group 边框是
同一容器下的 sibling；图包含关系保留在独立请求中。

图容器的受约束测量驱动 tracked viewport，超出视口的内容通过宿主面板的
Horizontal/Vertical 控制查看。固定节点尺寸保持源码输入；字体使用内置 Inter。
Native 验证展示 offset attachment、EAST 跨层路径、厚子图边框和嵌套子图：

```sh
cargo test -p draw2d-official-examples --lib graph_examples
cargo run -p draw2d-official-examples --example graph_verification
```

两个图入口的 Canvas 契约、真实菜单切换和 Native 代表性截图已验收，
状态为 `complete`。证据见
[ADR-043 实施记录](../../../doc/verification/evidence/adr043/implementation.md)。

TextFlowLargeExample 的精确布局共享与段落复用已接入，性能证据和剩余验收见
[长文本实施记录](../../../doc/verification/reviews/long-text-performance-implementation-2026-10-27.md)。

```sh
cargo xtask verify text.long-layout
target/release/textflow-perf 1 extended
```

性能驱动调用原样例工厂，每轮独立 Runtime，固定向第一文字来源追加 Z。
额外模式记录滚动、resize 和 CPU 帧准备；窗口呈现另行采证。
