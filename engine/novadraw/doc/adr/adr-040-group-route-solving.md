# ADR-040：连接组的统一路由求解

类型：`architecture-decision`

## 状态

已接受（2026-10-21）；组接口、visibility 算法及示例已实现并验证。
属于官方示例 K03 的 Obstruction 切片，语义身份为 `connection.router`。
承接[官方示例 Core 计划](../verification/plans/draw2d-official-examples-core-plan.md)。
既有 P2-C02 正交路由仍按[连接契约](../design/architecture/connection-routing.md)运行。

## 源码证据

只读仓库 `/Users/bytedance/Documents/code/GitHub/gef-classic`，
提交 `4463d9d0ce13c19d10fbe769d29f28b7345a8cba`。
范围仅为 org.eclipse.draw2d 与官方 Draw2D examples，不使用 Zest。
本批不编写、修改、编译或运行 Java 验证代码。

| 源码入口 | 证据 |
|---|---|
| examples/cg/ObstructionExample.TestFigure | 五障碍、背景手势、成对端点、Path 注册与删除 |
| graph/Path.createVisibilityGraph / addSegment | visibility graph 按矩形边界和端点连接，允许斜线 |
| graph/Path.labelGraph / determineShortestPath | 按欧氏距离标号并回溯路径 |
| graph/Path.refreshExcludedObstacles | 严格包含 source/target 的障碍按路径排除，边界不是内部 |
| graph/ShortestPathRouter.solve | 全组依次 solveDirtyPaths → countVertices → checkVertexIntersections → growObstacles → labelPaths → orderPaths → bendPaths → recombine → cleanup |
| graph/Vertex.grow / bend / updateOffset | spacing 默认 4；共享 corner 数量决定 grow，邻近障碍限制 offset |
| graph/Obstacle.init / containsProper | corner 使用 right/bottom 的减一边界，严格内部判断 |
| graph/Segment.cosine / crossProduct | 共享 corner 排序和路径 inversion 使用转向几何 |

源码两次 grow pass 不是任意次数重试；不能只增加 clearance 或 bend penalty，
就把现有正交算法声明成官方 visibility 路由。
Path 的 oval threshold 是搜索裁剪优化，失败或超阈值后取消阈值重新搜索；
输出最短路径与共享 corner 排列仍须独立验证。

## 现有边界与增量

现有 ConnectionRouter.route 接收一条 Connection 的 Anchor、constraint 和只读 group。
Runtime 顺序计算 group，group.route 只提供已存在或已计算的路线。
shared Manhattan 可以按稳定顺序保留已用 lane；Obstruction 的 corner count 与路径排序
需要读取本次完整组，且后面的路径可能改变前面的输出。

拟增加可选的组求解扩展，默认不提供组候选，Runtime 沿用现有逐条 route。
支持组求解的内置或外部 Router 都使用同一 trait 方法和同一批次验证。
不增加内置算法枚举，不 downcast 识别算法，不让 solver 持有 Runtime 或可变 Figure。

## 拟采用的输入与输出

`RouteGroupInput` 是一次候选求解的不可变快照，包含：

- 同一个 routing scope 的稳定 ConnectionId 顺序；
- 每条连接的受检端点 metadata，以及借用的 typed RoutingConstraint；
- 同一 routing domain 中的 immutable RoutingObstacle 集合；
- 只读、可记录依赖的 query 权限。

端点由 Runtime 调用各自 Anchor 的 reference/location 协议准备，相关观察加入批次。
自定义 Router 的组求解与内置能力获得同样的输入，不从前一帧 route 反推本次端点。
只有实际需要这类输入的组求解器才走该入口，旧 route 的 Anchor 回调顺序保持原契约。

新增 `ConnectionRouter.solve_group`，默认返回成功的 None，表示策略未提供组求解，
不是错误，也不表示组内没有路径。组候选成功时返回带 ConnectionId 的完整 outputs；
输出必须与输入组一一对应，不能遗漏、重复、越组或重新排序改变身份。
采用命名输入/输出类型和构造方法，准确签名在接受后先通过外部消费者编译测试固化。

Runtime 验证所有 RouteOutput 的端点、有限几何、坐标域与身份，再按 ADR-039 准备
Locator 和 Decoration。全部成功后才提交；任何输出失败，整组沿现有 unresolved
批次通道清除路线并保留恢复依赖。合法 None 沿现有 route 循环；错误不能回退成近似路线。

不在 paint 中 solve，不发布一半路径后再更新其余路径，不通过 Mutex 或全局状态
让逐条 route 暗中收集和改写本组结果。

## 官方 visibility 算法的归属

新增 VisibilityGraphConnectionRouter，保留 ShortestPathConnectionRouter 的既有正交行为。
算法 workspace 只存放本次输入生成的 obstacle/corner/segment/path 索引；
这些索引不成为 FigureId、ConnectionId 的替代身份，不长期持有场景引用。
跨帧增量缓存在获得必要证据后单独设计，不作为初版对等的前提。

实施依发生顺序分为：

1. 根据受检 rectangle 和 endpoint 构建 visibility graph；核对 perimeter、
   斜线、重叠障碍与端点在内部的排除规则。
2. 求每条路径的欧氏最短路线，确定 corner 的身份与访问序列。
3. 全组统计 corner count，按邻近障碍压缩 spacing，执行源码规定的 grow passes。
4. 计算 INNIE/OUTIE、inversion 和必要的子路径拆分，排序共享路径并生成折点。
5. 合并子路径，验证 finite geometry、端点一致性、障碍关系和本次组输出。

默认 spacing=4 以命名常量定义。Corner 的 right/bottom 减一语义属于该算法的
局部兼容规则，不修改 Core Rectangle 的连续几何语义。
本项目使用确定的插入顺序与身份作为等距候选 tie-break，
不复制 Java HashSet 的未规定遍历顺序，也不宣称等距路径逐点唯一对等。
跨组和嵌套坐标转换沿既有 routing domain，不引入算法自己的 surface 偏移修补。

## Obstruction 示例的状态与生命周期

示例只持有障碍 FigureId、有序 endpoint pair、pending source 与手势会话状态。
Core 继续拥有 Figure 树、输入捕获、坐标转换、Connection binding 与路由提交。
每次成功挂载后才发布对应身份；拒绝挂载不增加 obstacle 或 pair 状态。

| 输入 | 对应行为 |
|---|---|
| 初始构造 | 五个固定 seed=0 障碍，位置和高度公式来自原例；不把随机数发生器作为 Core 能力 |
| 背景左拖，abs(dx)>10 | 按 press/release 点创建 rectangle，纳入共享 obstacle 集合 |
| 背景右键 | 先创建 source，再创建 target；配对完成后注册 Connection |
| 障碍拖动 | 在父坐标域提交 bounds，依赖失效触发整组求解 |
| 障碍右键 release | 删除图元与障碍身份，重新求解受影响组 |
| 端点拖动 | 更新 bounds，既有配对身份保持稳定，通过 Anchor 重新准备端点 |
| 端点中键 release | 清理该端点、配对端点和 Connection；若是未配对 source，同步清除 pending 状态 |
| 背景其他 release | 保留 showSegs 切换；原例没有根据该值绘制诊断段，不新增可视化行为 |

原例 handleRemoveEllipseDragFigure 调用 TestFigure.getParent().remove(otherFigure)，
但 otherFigure 是 TestFigure 的 child。这是静态可证的 parent mismatch。
Novadraw 的受检 remove 不能复制该错误调用。本方案明确采用“删除整对”的兼容策略，
通过真实 owner 删除 pair 两端与 Connection，一次事务清理状态。
该差异须写入示例与验证文档，不暗改源码，也不把修正后的行为称为原例已运行的结果。

原例使用静态 router/maps/drag offset，paintBorder 中 solve；这些状态归示例实例、
输入会话和 Runtime，不引入 Singleton 或绘制热路径求解。

## 外部消费者与验收矩阵

下列证据由 Rust 测试与 Native 场景提供：

| 验收 | 要证明的边界 |
|---|---|
| 外部 group solver | 不改 Core，得到完整端点组、障碍 snapshot 和 query，返回可批量验证的 outputs |
| 既有 Router | 默认 None 保持 direct、Fan、Manhattan、正交 shortest path 和 typed constraint 行为 |
| 组错误 | 缺失/重复/外部身份、非有限 route、Locator 失败整组拒绝，修正后恢复 |
| 单路径 visibility | 空障碍、斜线、单/多障碍、重叠、边界接触、端点内部排除、同输入确定性 |
| 共享 corner | 反向路径、多次共享、inversion、grow 后新交叉、邻近障碍 spacing squeeze |
| 动态状态 | Add/Move/Remove obstacle，pair 完成与删除，未配对 source 删除，拒绝挂载与清理 |
| 平台输入 | 父坐标拖动，capture/release/exit，不把 surface 偏移重复计入 |
| Native 场景 | 初始五障碍及多对端点，移动与删除后的路线、child/map 数量和截图 |

只使用 Rust fixture、属性不变量和真实 Novadraw 示例验证。来源中的异常与兼容选择保留，
不生成 Java 探针、Java 基线或上游运行门禁。

## 执行顺序

先评审组输入/输出与默认适配，再建立外部消费者与失败反例，
随后实现 visibility 与共享 corner 流程，最后接入 Obstruction 的交互和生命周期。
每个切片通过后维护 K03 与 catalog；算法、交互和真实示例验收全部完成前，
Obstruction 保留 in_progress 状态。

## 实施证据

`ConnectionRouter.prepares_group_endpoints` 显式选择端点准备；
`solve_group` 默认返回 None。完整组输出按输入顺序验证身份与 metadata，
RouteOutput 构造拒绝非有限点，Locator/Decoration 沿原有批次预检与提交。
失败保留全组观察，修正输入后可以恢复。既有逐条 Router 不改变 Anchor 回调顺序。

外部消费者测试为 `crates/novadraw/tests/p2_group_route_contract.rs`：
验证完整组、缺失、重复、重排、外部身份、metadata 变化、非有限几何、
solver 错误及第二条连接的 Locator 失败，确认整组清空与恢复。
`crates/novadraw/src/connection/visibility.rs` 的测试覆盖斜线、重叠、严格内部排除、
反向共享、spacing squeeze 和多个端点方向；独立开矩形裁剪检查线段不穿障碍内部。

连接的规范 routing domain 仍是直接父节点的 child content。
`FigureSubtree.add_sibling` 让同一描述中的连接和端点作为 Canvas 的直接子节点，
沿既有完整准入流程发布，不增加算法专属路由域。
`FigureMut.replace_child_subtree` 与 `EventEffects.replace_child_subtree_later`
先准入完整替代描述，再移除 pending source；拒绝时保留旧节点。
contents/viewport contents 仍只接受单根描述。
成对删除在同一次 dispatch 的 deferred effects 中完成，下一帧前清理整对与连接状态；
不宣称各个 lifecycle 回调能观察到整对同时退休。

`examples/obstruction.rs` 的真实输入测试覆盖三对共同路由、障碍新增/移动/删除、
端点拖动与删除、pending 位置和准入拒绝、pointer exit/release。
障碍文字用统一 FigurePreparation 测量，移动通过 typed component update 同步文字和 bounds。
原例 showSegs 没有绘制行为，保留切换状态。

Native 复现命令：

```bash
cargo run -p draw2d-official-examples --example obstruction_verification
```

该入口用公开 input API 构造初始、多对端点、障碍移动、障碍/配对删除四个场景，
截图保存于 `target/visual-verification/screenshots/`，名称为
ObstructionInitial、ObstructionShared、ObstructionMoved、ObstructionRemoved。
顶部路线可越出 Canvas，障碍文字沿 Figure 边界裁剪，均不擅自增加源码未规定的避让或文字布局。

最终验证：workspace check 与 Clippy（`-D warnings`）通过；
workspace test 共 1,155 项通过、1 项忽略、0 项失败。
文档门禁与 diff whitespace 检查通过；中文半角标点及破折号机械扫描无命中。
