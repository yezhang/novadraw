# ADR-039：连接标签的 owner 方位定位

类型：`architecture-decision`

## 状态

已接受（2026-10-21），用户批准按草案实施；Core 与官方 UML 消费者已实现并通过最终门禁。
属于官方示例 K03，关联语义身份 `connection.locator`。
本决策补充[连接契约](../design/architecture/connection-routing.md)的 child placement；
EndpointLocator 的既有切线/法线行为保持原契约。

## 问题与源码证据

官方 UMLClassDiagram 的 `end1` 使用 ConnectionEndpointLocator，设置 uDistance=8，
保留默认 vDistance=4。当前示例使用 EndpointLocator 的切线偏移，且 vDistance=0，
与官方标签 placement 不等价。

只读参考仓库：`/Users/bytedance/Documents/code/GitHub/gef-classic`，
提交 `4463d9d0ce13c19d10fbe769d29f28b7345a8cba`。
证据为 `org.eclipse.draw2d/.../ConnectionEndpointLocator.java` 的
`relocate`、`calculateConnectionLocation`、`calculateTan`、`calculateYShift`，
以及 `org.eclipse.draw2d.examples/.../uml/UMLClassDiagram.java` 的 `end1` 构造。
不编写、编译或运行 Java 验证代码。

官方算法使用四类输入：

- source 或 target 的路由端点及其相邻点；
- 对应 Anchor owner 的边界；无 owner 时按首段方向推导方位；
- child 的 preferred 尺寸；
- uDistance 与 vDistance，其中 vDistance 的符号决定标签在横向归一化坐标中的偏移。

侧边定位与曲线方向不同。owner 方位决定标签向哪一侧伸出；斜率影响沿连接方向的
纵向偏移。只向现有 point 加切线和法线距离无法表达标签的最终边界。

## 拟采用的开放输入

在 `connection` 领域新增只读 `LocatorInput`，包含：

| 输入 | 含义 |
|---|---|
| points | 准备中的 Connection 局部路线 |
| metadata | 同域端点位置、reference 与 normal |
| child_size | 通过统一 preferred measurement 准入的 Dimension |
| source_owner_bounds / target_owner_bounds | Anchor owner 的边界，转换到 Connection 局部域；无 owner 为合法 None |

该值是一次定位计算的借用输入，不含 Runtime、可变 Figure、backend 类型或隐式全局状态。
构造与字段演进使用明确构造方法和读取方法；不要求扩展者伪造 owner 身份。

在 ConnectionLocatorStrategy 增加 `locate_child(&LocatorInput)`，
返回 `Result<ChildLocatorPlacement, LocatorError>`。默认实现委托现有
`locate_with_metadata`，将结果包装成默认居中 placement，保持既有外部策略兼容。

新增 ChildLocatorPlacement，包含既有 LocatorPlacement 与可选 child bounds override，
通过构造方法和读取方法访问，保留 LocatorPlacement 现有 struct literal 调用面：
普通 point locator 默认返回 None，Runtime 使用测量尺寸居中；
需要 owner 方位的策略返回 Some(Rectangle)，Runtime 验证后直接应用。
point/reference 仍用于现有 decoration 方向计算。既有策略只实现 locate 或
locate_with_metadata 时不需迁移；新外部策略可覆盖 locate_child。

bounds override 必须有限、宽高非负，且宽高等于受检 child_size。
策略只能选择 placement，不能覆盖测量权威。溢出或非有限值通过 LocatorError 返回，
不得截成零尺寸或默认位置。

## 策略命名与兼容

保留 EndpointLocator 的既有切线/法线行为。新增 ConnectionEndpointLocator，
显式承载官方 owner-side 语义，避免改变已接受的 EndpointLocator offset 契约。
两个策略共用 ConnectionLocatorStrategy、同一绑定注册和同一路由批次提交。

新策略构造使用 RouteEnd、有限的 u/v 距离；提供官方默认值 u=14、v=4 的命名构造方法，
具体常量归策略实现。UML 示例显式设置 u=8，保留 v=4。
外部策略也可以返回 bounds override，不为内置策略保留类型匹配或特殊回调。

## owner-side 算法

以端点为起点，target 使用倒数第二个路线点作为邻点。
有 owner 时，比较端点相对 owner 中心的位置与矩形长宽比例，区分左、右、上、下；
角点平局使用左右侧。无 owner 时比较首段的轴向绝对长度，平局使用纵向。
源码 quadrant 的数字注释与屏幕 y 方向不一致，实现使用语义名称和屏幕坐标，
不得直接复制注释中的 Top/Bottom 名称。

上、下侧将点和 child_size 的 x/y 转置，统一按侧边公式计算，再转回。
令右、下侧的方向符号为 +1，左、上侧为 -1，记为 c。
归一化首段斜率 tan 限制在 [-1, 1]；垂直首段采用源码的 tan=1。
设 child 宽高为 w、h：

```text
x = endpoint.x + c*u + (c < 0 ? -w : 0)
yShift = v < 0 ? -h : (v == 0 ? -h/2 : 0)
y = endpoint.y + c*u*tan + v + yShift
```

Novadraw 的几何和测量使用 f64。本契约保留连续坐标，不在中途复制 Java 整数截断；
整数 fixture 的结果可对照公式，奇数尺寸居中允许半像素。此差异需记录在 parity，
不能把 Java 的 mutable singleton、Transposer 状态和整数 cast 搬进 Core。

owner 为零尺寸、端点与邻点重合时使用明确定义的退化处理：
零尺寸 owner 不提供方位约束，回退首段；首段退化时使用有效 outward normal，
无有效 normal 则返回 DegenerateRoute。无 owner 本身不构成失败。

## 状态与依赖归属

Runtime 在路由候选阶段读取 Anchor owner 身份，并通过 tracked RouteSceneQuery
读取 border box、转换到 routing domain，再平移到准备中的 Connection 局部域。
这些观察并入路由批次的依赖集合，owner resize、移动、祖先变换和删除可触发重新定位。
自定义 Router 未查询 owner geometry 时也不能漏掉这些 Locator 依赖。

owner geometry 使用 border box，契约明确为转换后的轴对齐 envelope。
旋转或 shear 后按该 envelope 分类，不宣称与 SWT 整数矩形等价。

普通 child 在定位前使用统一 preferred measurement；失败不进入策略，也不写入成功缓存。
测量 generation 的失效与确认沿用 `4a3def17`，不建立额外 Figure timer 或全树扫描。
Decoration 继续从 route point/reference 准备旋转几何，不强制对其测量 envelope 重新定位；
不能对 decoration 的输出边界反向推导普通标签的 child_size。

整个批次先准备 route、owner 输入、测量和 placement，再提交。
任何策略、owner 查询或测量失败都沿现有 unresolved / batch rejection 通道传播；
不发布部分 child bounds，不承诺回滚外部策略的私有副作用。

## 外部消费者证据与验收矩阵

证据入口：`crates/novadraw/tests/p2_owner_locator_contract.rs`、
`crates/novadraw/tests/m9_connection_runtime.rs` 和官方 UML 的单元测试。

| 验收 | 要证明的契约 |
|---|---|
| 外部自定义 Locator | 不改 Core，通过同一输入读取 owner/child_size 并返回自定义 bounds |
| 既有纯 point Locator | 默认适配保持 source/target/middle/path fraction 行为 |
| 四侧与首尾端点 | source、target，owner 四侧、角点平局，斜段和 v 正/零/负 |
| 无 owner / 退化 | XYAnchor、零尺寸 owner、重复端点、normal fallback、无效 normal |
| 测量变化 | preferred 覆盖/重置、Label 文字、owner 不动时更新，不变值不额外路由 |
| 变换与生命周期 | 嵌套偏移、缩放、旋转 envelope、owner resize、删除/重绑 |
| 错误隔离 | 非有限 bounds、负尺寸、错尺寸、非法外部测量、批次失败与恢复 |
| 真实 UML | end1 的 u=8/v=4，label 新尺寸，Canvas 图形、路线及 native 截图 |

四侧算法采用 Rust 表驱动 fixture，不生成 Java 基线。
功能落地后才更新 catalog 与 `connection.locator` 的完成状态。

## 下一步

已完成统一输入与输出、兼容适配、owner 边界的 tracked 查询和本地方位计算；
6 项独立契约测试覆盖矩阵中的纯算法、外部策略、变换和生命周期，既有连接回归保持通过。
UML 的 5 项测试包含官方 u=8/v=4 placement 与动态文字测量，macOS Native 截图已人工核验。
整数 fixture 保留连续 f64 结果，半像素居中差异按本决策报告。
最终 workspace 测试 1,144 项通过、1 项忽略，workspace Clippy、定向测试 Clippy、
格式检查、文档检查与 `git diff --check` 通过；Obstruction visibility 路由随后独立推进。
