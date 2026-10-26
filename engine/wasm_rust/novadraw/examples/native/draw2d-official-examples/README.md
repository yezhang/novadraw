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

TreeExample 的 Use Animation 默认关闭。双击有子节点的标题可立即展开/折叠；
开启动画后，双击进入同一 queued toggle 管线，也可使用面板的 Expanded 控件。
共享 winit 宿主通过平台 `WinitPointerAdapter` 识别真实按下/释放序列中的双击，
无需应用手动调用 Core 的双击入口。`animation.tree` 同时覆盖平台识别、宿主投递
以及动画开/关两种 Tree 消费者回归。

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
实现。运行后，背景左拖创建障碍；背景右键两次创建一对端点与连接；
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

```sh
cargo test -p draw2d-official-examples --lib zoom_example
cargo run -p draw2d-official-examples --example zoom_verification
```

Path 已接入旋转闭合路径、选字 clip 和两次文字绘制；最小/最大缩放使用同一
ScrollBar 的真实按钮输入验证。SchemeBorder 和字体指标下划线仍待 K08，
入口状态保留 `in_progress`。

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
[ADR-043 实施记录](../../../verification/evidence/adr043/implementation.md)。
