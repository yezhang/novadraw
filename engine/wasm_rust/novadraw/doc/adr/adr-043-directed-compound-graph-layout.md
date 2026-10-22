# ADR-043：有向图布局的独立模型、开放求解器与 Figure 投影

类型：`architecture-decision`

## 状态

已接受，修订 1，实施中（2026-10-22，用户批准 ADR-043 并授权实施）。
实施与验收按下述切片推进，不代表 P2-L01 已完成。
承接官方示例 K06；`api_semantics` 为 `layout.graph`、`layout.manager`、
`connection.routing`、`figure.measurement` 和 `scene.preparation`。

## 目标与来源

DirectedGraphDemo 与 CompoundGraphDemo 需要根据拓扑求解节点位置、rank、
跨层边折点和子图矩形。它们消费算法输出，再创建 Label、子图边框和 Connection。
图的包含关系是算法输入；官方 Compound 场景中的 Figure 仍在同一 Panel 下。
因此不能把 FigureTree 的父子关系直接当成图的包含关系。

只读参考 `org.eclipse.draw2d.graph` 与官方 `examples.graph`。不扫描现状实现
来定义理想模型，不使用 Zest，不编写、编译或运行 Java。

参考仓库为 `/Users/bytedance/Documents/code/GitHub/gef-classic`，固定参考提交
`4463d9d0ce13c19d10fbe769d29f28b7345a8cba`。新增核对
ConvertCompoundGraph、CompoundRankSorter、RouteEdges 的端点与排序规则。
已核对的主要来源：

- DirectedGraph、Node、Edge、Subgraph；
- DirectedGraphLayout、CompoundDirectedGraphLayout；
- TransposeMetrics、VerticalPlacement、RouteEdges；
- AbstractGraphDemo、DirectedGraphDemo、CompoundGraphDemo；
- GraphTests、CompoundGraphTests 的公开 fixture 清单。

原版算法不保证每次结果相同。本项目增加确定性要求，但不将某组 Java 像素坐标
作为唯一答案。核心契约是 rank 约束、包含关系、padding、attachment 与路径顺序，
以及权重驱动的布局目标；Rust fixture 按源码输入重建，验证结果和不变量。

## 决策

### 独立布局域

公开调用面放在 `novadraw::layout::graph`，避免与内部 Figure 图存储混淆。
纯模型、验证和默认算法先保留在 Core 模块内，不创建空扩展 crate。未来第三方
算法通过同一公开协议接入；不向 Core 公开签名泄漏第三方图模型。
这延续 [ADR-023](adr-023-crate-consolidation-and-extension-boundaries.md)。

图请求只含受检身份、几何、拓扑、排序和算法参数，不持有 Runtime、Figure、
资源对象或 UI 控制器。请求不可变；算法的 cycle inversion、rank、virtual nodes、
spanning tree 与 compound boundary 都在实例局部工作区中处理。

拟定字段如下，命名可在实施中调整：

| 对象 | 输入 |
|---|---|
| GraphNode | 请求域内 NodeId、受检尺寸要求、可选 padding、incoming/outgoing attachment、可选所属 GroupId、typed 算法约束 |
| GraphGroup | 请求域内 GroupId、可选父 group、minimum size、外部 padding、border insets、inner padding、attachment、typed 算法约束 |
| GraphEdge | 请求域内 EdgeId、source/target endpoint、width、padding、可选 attachment override、typed 算法约束 |
| GraphLayoutRequest | nodes、groups、edges、方向、默认 padding、margin、算法身份与配置、所需结果能力、工作预算 |

NodeId、GroupId、EdgeId 独立于 FigureId，并由请求 builder 验证归属。
节点与 group 都允许作为边端点；endpoint 的封闭值类型表达实体种类，
不承担算法或扩展行为的分派。算法行为走开放 registry。
节点没有多个 parent；group 包含关系必须无环，深度受检且不超过 10,000。
尺寸与 Insets 有限、非负。row constraint、minimum rank span 和 weight 属于
分层算法的 typed 约束，不强加给所有算法；要求它们的请求必须选择支持对应能力
的算法。node 和 group 都可携带 row constraint。它在同一 rank 的同级包含域内
排序实体，group 作为连续成员区块参与排序，成员在区块内按自己的约束排序；
不能把 group 约束展开为与外部节点交错的成员顺序。
旧 Java 的 `-1` sentinel 分别转为 `None`，不得进入公共 Rust 几何协议。
attachment offset 非负且有限，保留 Draw2D 允许超出侧边长度的语义：
TransposeMetrics 不转置 offset，官方 offsetTest 的 EAST 输入依赖这一点。
端点按候选 bounds 与 offset 计算，并受完整路径、clearance 与 extent 验证。

空图返回空结果。默认 Draw2D 算法要求弱连通：普通图检查边的无向连通性，
compound 检查边与包含关系的并集；非法输入结构化拒绝，不偷偷补边或分块。
扩展算法可以有不同准入能力，但必须显式声明并接受同一请求验证。

### 边界输入与预算

默认分层算法接受零 rank span；零值表示允许同 rank，不保证一定同 rank。
正 span 的自环不可满足，结构化拒绝；零 span 自环也明确返回
UnsupportedSelfLoop，当前默认算法不承诺自环路由。声明自环能力的外部算法可接受，
但仍须通过 attachment、路径及 clearance 校验，不得返回伪造直线。
weight 为非负整数，零表示不参与 weighted length 目标，仍保留 rank 硬约束。
这些策略是本项目的显式准入边界，不宣称上游未定义输入都已对等。

空 group 允许，结果至少满足 minimum size、border insets 与 inner padding；
默认 compound 仍为其建立私有 head/tail 边界。没有节点、group 和 edge 的请求
才是空图。group containment cycle、缺失端点、重复身份始终拒绝。

同一工作预算覆盖请求验证、包含/连通遍历、辅助实体数与分配额度、算法阶段、
测量反馈轮次和结果验证。分配前检查额度，整数计算检查溢出，遍历检查深度；
验证及外部 handler 消耗同一计数器。预算不能只包围 solve 而留下无界准入开销。
外部同步算法须遵守计数器协议；该协议不是对不合作代码的抢占式超时保证。
耗尽错误携带阶段、用量与限制，失败产物不进入成功缓存。

### 开放算法与结果验证

算法采用 typed identity + payload trait + registry。标准 Draw2D directed、
compound 配置也是注册项；Runtime 不根据算法名字或内置类型分支。
registry 属于消费者的布局服务实例，不使用全局 singleton。

拟定协议：

```rust
trait GraphLayoutAlgorithm {
    fn solve(
        &self,
        request: &GraphLayoutRequest,
        context: &mut GraphSolveContext,
    ) -> Result<GraphLayoutCandidate, GraphLayoutError>;
}
```

GraphSolveContext 只提供预算和诊断，不提供场景 mutation、paint 或宿主服务。
GraphAlgorithmKey<P> 和受检 payload 标识算法配置；注册的 erased handler
将 typed 配置送给相同 solve 协议。外部算法不需要改 Core 枚举或添加 typed-only hook。

### 基础结果与 typed 能力结果

基础候选仅包含每个 node/group 的 placement、每条 edge 的完整 source→target
路径及 attachment 记录、extent、方向和请求身份/revision。路径明确为完整路径，
不同时混入 routing hints。rank、feedback、virtual nodes 不是基础结果必填字段。

算法注册声明可提供的 typed 结果能力及支持的 typed 输入约束；请求列出所需能力。
服务在求解前检查能力匹配，缺失则返回 UnsupportedCapability。候选通过实例 registry
附带相同身份的 typed 结果 payload，并调用注册的能力验证器；要求的能力缺失、
类型不符或验证失败均拒绝，不按内置算法名称分支。标准分层结果也走这一协议。
消费者通过受检能力查询取值，不能把合法“无 rank”视为错误或默认零。

标准分层结果提供 node rank、group 的 head/tail rank 区间、边消环方向、
feedback 与 virtual node 几何。group 跨多个 rank，不能以单 rank 替代；
rank span 按边实际选取的 head/tail 端点和消环方向检查。feedback 恢复的是路径
顺序，不改变请求中的 source/target。virtual nodes 不进入实体身份空间。
row constraint 验证同级区块顺序、成员连续性和区块内部顺序，EAST 按转置后的
规范 rank/cross axis 检查。

### 候选硬约束与不可变验证结果

`GraphLayoutResult::validate` 必须同时接收具体不可变请求、候选和能力 registry。
验证成功后生成字段私有、只读的结果，绑定请求域身份、revision、输入/测量约束
前缀、算法配置和验证能力版本。候选修改后必须重新验证；复制结果不解除绑定，
相同拓扑但不同请求 revision 不能沿用旧结论。

| 验证 owner | 硬约束 |
|---|---|
| Core 通用验证器 | 身份完整且唯一、无额外实体；请求身份/revision 匹配；几何有限、非负且 extent 覆盖所有 placement 与路径 |
| Core 尺寸与包含验证器 | fixed 尺寸不变、每轴 minimum 与允许拉伸约束；group minimum；成员位于扣除 border insets/inner padding 的内区；无包含关系的 node/group 内部不侵入 group 保留区或与其内部重叠 |
| Core spacing/端点验证器 | 请求定义的实体外部 padding 与边 clearance；每条路径首尾与其受检 attachment 相同，原始 source→target 顺序保持；attachment 与候选 bounds、方向和 metadata 一致 |
| typed 能力验证器 | 标准分层的 node/group rank、span、feedback 消环方向、row constraint 与连续区块；外部能力自身的附加硬约束 |
| Figure adapter 准入 | 当前 source/binding/measurement 前缀与结果一致；最终 Anchor 端点、ConnectionId、routing scope 与完整路径一致 |

group 与祖先之间按包含规则允许嵌套，非成员排除检查针对无包含关系实体，
不把祖先当成侵入者。clearance 按算法声明的端点出口豁免和走廊规则验证，
不能因边首尾接触 owner 就误判；扩展规则由同一 typed 验证入口受检。
各失败保留实体/edge 身份和违反的约束分类，进入候选拒绝通道。
权重优化与交叉减少仍由独立算法测试证明，不声称上述验证器可证明全局最优。

### 默认算法的语义切片

默认实现从 Draw2D 约束出发分阶段落地，不能用简单拓扑排序代替完整交付：

1. SOUTH 作为内部规范方向；EAST 转置输入几何、padding、结果与路径，
   不修改消费者输入。
2. 确定性消环，保留原边方向和 feedback provenance；compound 消环需要
   同时考虑包含图及其逆向可达关系。
3. 初始 rank、tight spanning tree 与 weighted rank assignment。最小 rank span
   是硬约束，weighted edge length 是优化目标；固定输入顺序用于所有平局。
4. 跨层 virtual nodes、rank padding 与 row height；同层节点沿主轴拉伸，
   EAST 转置后对应宽度变化。row constraint 与 compound 的连续排序一起处理。
5. 交叉减少、横向 placement 和邻域优化；compound 区域不得被外部节点穿入，
   border insets 与 inner padding 是包含空间约束，不能只在结束时包围子节点。
6. 路由按 attachment 和跨层走廊生成 points。源码 RouteEdges 使用 virtual node
   邻域限制走廊，不能直接连首尾，也不把 `±100000` 障碍延长值照搬为 magic number。
   走廊采用显式有限 extent 或受检半平面表示，具体策略在该切片核对源码后确定。
7. 恢复 feedback 边的 source→target 路径顺序；输出消费方向中的所有几何。

默认实现的阶段状态私有。不要把 GraphVisitor、workingInts、rank forest 等 Java
工作区结构变成公共 API。精确算法细节在各切片实施前继续只读核对对应源码；
本 ADR 接受领域边界，不把尚未核对的实现细节写成已验证事实。

### Figure 投影与稳定发布

纯算法可以完全脱离 FigureTree 使用。需要动态 Figure 布局时，使用公开
GraphLayoutService 和 adapter，消费者持有 graph identity→FigureId binding。
binding 负责对应关系；Core 负责冻结测量、坐标域、失效与受检输出。

adapter 按当前 LayoutSceneQuery 测量节点，生成独立请求。所有结果几何处于布局
容器的 child-content 域；viewport、scale、insets 的转换继续使用现有引擎协议，
示例不手算坐标。compound group 不要求 Figure 嵌套，支持官方 flat Panel 投影。

### binding、attachment 与完整路径的准备顺序

binding 覆盖全部请求 node/group→FigureId 和 edge→ConnectionId；默认 adapter
要求实体一一对应、连接不重复使用，同一结果的边完整覆盖一个显式 route group。
组外连接不被暗中接管；共享某节点的其他连接沿自己的正常依赖通道失效。
所有 binding 属于同一 Runtime、同一 graph service 和当前源前缀。
flat 投影中的节点、group Figure 与 Connection 均是布局容器的直接 child，
routing scope 就是该容器的 child-content 域。初版 adapter 不承诺跨容器 binding；
违反范围时受检拒绝，不把 surface 坐标冒充 routing scope。

本 ADR 采用**完整算法路径**，不是让普通 Anchor 另算首尾后再套用 hints。
请求 attachment 负责端点语义，算法根据候选 bounds 生成端点；adapter 的
attachment-aware Anchor 负责向现有 Runtime 准入通道提供同一端点与 metadata，
不能再独立选择 Chopbox 端点。未经兼容验证的普通 Anchor 不能混入本组。

一个候选按以下顺序准备，任何步骤失败都不晋升 stable epoch：

1. 冻结 source、binding、样式/字体和测量约束前缀，生成不可变请求。
2. 求解 candidate placement、attachment、完整 points 和 typed 能力结果；
   进行纯结果验证，再进入同一个 Runtime 派生检查点应用候选 LayoutOutput。
3. 验证后代布局与宽度敏感测量反馈，必要时在同一预算内重建请求/重求解，
   直到候选 outer bounds、presentation 和 measurement 前缀稳定。
4. Runtime 在候选 bounds 视图上调用 attachment-aware Anchor，准备位置与
   Anchor metadata；记录请求 revision、候选结果身份、endpoint 实体和方向。
   不允许从上一 stable bounds 取端点，也不允许绕过 Runtime 端点准备。
5. RouteGroupSolver 读取这一候选的完整 points；按 ADR-040 检查组覆盖、位置
   与 metadata 完整性，首尾必须等于本次 Anchor 准备的端点，再准备 Locator、
   Decoration、hit 和 damage。结果只能复用同一候选，不能另求一套布局。
6. 检查冻结前缀未漂移，整体提交快照与稳定 epoch，才允许 paint/input 消费。

第 2–5 步内部可以暂存候选几何，不能向 stable query、通知或 RenderSubmission
暴露部分节点或路径。现有机制是否支持候选 bounds 的 Anchor 读取，需要在 adapter
切片验证；若不支持，补充公开的受检候选查询契约并复审后实施，不能读取旧 bounds
或在示例手写端点换算作为替代。

默认 compound attachment 在 SOUTH 消环后的规范工作图中选择边界：
group 作为 source、对端属于其成员时选 head，否则选 tail；group 作为 target、
对端属于其成员时选 tail，否则选 head。转换按源码先 source 后 target 的顺序，
以私有 boundary 的归属判断嵌套；保存选择依据与原始 endpoint 身份，恢复 feedback
时连同端点记录反转。不得在恢复后的原始方向上重新猜测另一组边界。
RouteEdges 对 source head 使用 group.y + insets.top 的内边界位置；其他位置按
对应 boundary 的候选 geometry 与 attachment offset 计算。EAST 最后统一转置。
head/tail 不成为用户实体或 FigureId。group→成员、成员→group、跨 group、
嵌套 group 和 feedback 都需独立端点反例，不可只验最终包含矩形。
以上分层规则通过 typed attachment 结果与验证器承载，非分层外部算法可声明
自己的受检边界选择，仍须与同一 Anchor 准入结果一致。

### 尺寸语义、测量反馈与缓存

纯请求中的尺寸要求按轴明确 fixed、minimum、preferred 和允许拉伸方向。
默认分层算法将输入 preferred 作为不可收缩的初始需求，可以沿规范主轴扩大
至同 rank 最大值；SOUTH 改高度，EAST 改宽度。fixed 轴不得更改，不能通过覆盖
请求的 fixed 属性偷偷模拟该拉伸；冲突返回 UnsatisfiedSizeConstraint。
官方 fixture 的 width/height 是明确、可按原版规则拉伸的输入需求，直接进入
纯模型，不用 Label 字体测量替换；最小尺寸与 preferred 初值一致。

动态 Figure binding 则使用受检 preferred/minimum measurement。初始由请求配置
给定 outer constraints（通常未限定的轴保持无界），不能把上一轮派生 bounds
隐式当成 source 尺寸。测量及容器投影遵守
[ADR-018](adr-018-runtime-driving-and-measurement-api.md)、
[ADR-038](adr-038-query-identity-and-failure-semantics.md)和
[深层布局与 Presentation 收敛契约](../design/architecture/deep-layout-presentation-convergence.md)：
outer constraint→扣除物理 insets→反投影→content measurement→outer 正投影。
graph layout 的 placement 是布局容器 child-content 域中该实体的 outer box。

得到候选 placement 后，以候选 outer 宽度重新测量宽度敏感 Figure 的 preferred
高度和 minimum 需求，准备后代布局与 presentation。最终宽度改变可能导致换行、
高度改变；需求改变时生成新测量 revision，再求解，不能仅刷新文字绘制。
显式 fixture 输入不因这个动态测量流程改变。首个成功 submission 必须包含
最终尺寸、glyph、clip 与 route；应用不追加第二帧或 full redraw 修正。
振荡、非法测量或预算耗尽按“检查点恢复与结果有效性”处理。

实例缓存键包含 source/request revision、完整 binding 与 routing scope、
算法 key/payload/能力验证版本、每次测量的 FigureId/约束/样式/资源 generation。
记录约束变化引起的测量依赖；派生 bounds 写入本身不是 source revision。
只有它改变后代实际测量需求或最终约束时才驱动反馈重求解。收敛后记录最终
约束与需求，不重复命中初始无界测量而制造循环；同一合法前缀的重复帧不重算。
失败值不入成功缓存，纠正源输入后按失效机制恢复。

### 检查点恢复与结果有效性

Runtime 的派生收敛事务是检查点 owner；GraphLayoutService 只提供可随事务
暂存/恢复的结果缓存，不拥有独立 rollback 机制。graph placement、后代布局/
presentation、Anchor 派生依赖、组路由、Locator、Decoration 与候选通知/damage
整体进入 [ADR-042](adr-042-structured-flow-layout.md) 的受检派生检查点。
恢复只撤销未发布的派生变化，不能撤销已提交的源属性、拓扑、删除或 binding。
这是 graph adapter 的新增事务要求，不把
[ADR-038](adr-038-query-identity-and-failure-semantics.md)的一般 layout 失败
重新解释为整个 Runtime rollback。

| 失败情形 | 检查点恢复与有效性 |
|---|---|
| 候选失败，源与 binding 前缀未改变 | 撤销本次候选，上一稳定结果若仍完整匹配当前输入可保持有效；失败候选的 unresolved 状态仅存在于未发布派生分支 |
| 源属性、拓扑或测量前缀已改变 | 撤销候选，不撤销 source；旧快照仅供上次已呈现帧/诊断保留，不能作为当前有效布局、hit 或 route；标记整个 graph projection invalid，受影响组进入 unresolved，保留恢复依赖 |
| 端点删除或 binding 退休 | 删除仍生效，恢复不得复活 FigureId/ConnectionId；清除退休身份的缓存/binding/路由依赖，其余受影响组 unresolved；消费者更新拓扑并准入新完整 binding 后重试 |

ADR-040 的组失败清空路线/unresolved 规则保持：失败候选组先沿该通道处理，
随后事务根据表中的当前前缀决定能否恢复上一有效派生分支。源已改变时不能以
“保留旧结果”把旧路线重新标为 resolved。Source/binding 清理属于已提交事实，
从检查点恢复派生依赖时必须按当前身份和输入重建/失效。
没有有效当前结果时稳定查询报告 invalid/unresolved，禁止发布新的 partial frame；
屏幕可保留上一已呈现帧，但旧缓存不得用于当前命中或路由成功返回。

合法输入修正后，从当前 source 前缀重新测量/求解/准备，全部成功才恢复 valid。
新 contents 的准入失败保留旧树；已成功替换 contents 后旧 binding 失效，
不得恢复旧树为当前内容。服务释放不保留 source Figure 引用。
预算错误、测量错误、路径错误保留阶段和身份，不生成 fallback 网格或假 rank。

## 外部消费者证据

实施必须包含一个独立外部算法，用 typed key 注册自己的配置、生成结果并接入
同一 Figure adapter 与 route group。Core 不依赖该类型，也不提供内置快路径。
该算法必须是非分层算法：只返回基础 placement、完整路径与自己的 typed 能力，
不产生 rank/feedback/virtual nodes。基础消费者成功；要求分层能力的消费者得到
UnsupportedCapability，不能修改 Core 枚举或生成假 rank 通过门禁。
另外包含无 Runtime 的纯图消费者，证实图身份与 Figure 生命周期已经分离。

验证外部算法的失败、非有限结果、缺失/重复节点、错误端点、非法 group bounds，
以及身份齐全、几何有限但违反 minimum、inner padding、非成员排除、clearance、
attachment 和请求 revision 的结果，确认同一门禁拒绝发布。候选修改后沿用旧
验证结果也必须拒绝。外部宽度敏感 Figure 提供测量，EAST 拉伸引起换行变化时
验证首帧收敛、非法测量后的恢复和重复稳定帧不重算。

## 示例与验收

GraphTests 有 16 个公开 fixture，CompoundGraphTests 有 10 个，分别验证 SOUTH/EAST，
共 52 个源码输入组合。菜单使用显式 fixture 清单，不依赖 Java 反射顺序或静态状态。
官方 node Label、无 incoming 时双宽边框、gray edge/target decoration、blue group
边框和 flat Panel 层次按源码移植。宿主控制器切换 fixture/方向并替换完整 contents。

不以 52 次 solve 成功代替完成。验收还包括：

- rank span、feedback 恢复、平行边、padding、attachment 和有限 extent；
- 小图 weighted rank 的穷举对照、排序约束与跨层路由独立检查；
- compound 嵌套、跨 group 边、包含 cycle、inner padding 与非成员排除；
- 同输入/算法配置重复结果一致，多实例互不影响，预算与溢出拒绝；
- Figure 测量更新、contents 替换、旧 binding 拒绝、整批失败后恢复；
- 默认/外部算法使用同一入口，Native 代表性截图与方向/fixture 真实切换。

修订 1 增加以下必须通过的反例：

- 同源候选失败、源已变更后失败、端点删除后失败分别检查有效性、unresolved、
  检查点恢复和后续修正；源属性、拓扑与 binding 不得被回滚。
- 新候选 bounds 与 Anchor metadata/完整 points 对应同一请求，错误端点不能
  进入准入；group/成员、跨 group、嵌套、feedback 和 SOUTH/EAST 覆盖边界选择。
- group row constraint 参与同级区块排序，跨 rank head/tail 区间与成员连续性
  正确；不把 group 当成普通单 rank 节点。
- 动态 EAST 宽度敏感测量的成功首帧、不收敛反馈、失败后恢复与稳定缓存；
  fixed 输入冲突、零 span、自环、空 group 及各阶段预算耗尽明确拒绝或成功。

## 实施顺序与门禁

1. 评审本 ADR，再实施纯请求/结果、身份验证与开放算法 registry。
2. Directed 默认阶段与 16 fixture 的两方向契约。
3. Compound 阶段与 10 fixture 的两方向契约。
4. 统一 Figure/Connection adapter，外部算法与失败恢复契约。
5. 官方两个场景和宿主切换，逐项维护语义账本与 Canvas 清单。

每次只推进一个切片；按风险运行精确测试与 crate lib check/Clippy。
最终运行 workspace fmt/check/Clippy/test、文档门禁和代表性 Native 验证。
在上述证据齐全前，P2-L01 与两个 graph 入口均不能登记完成。

### 第一切片实施证据

2026-10-22 已实现 `novadraw::layout::graph` 的纯请求与结果协议：
请求域身份、逐轴尺寸、包含结构、完整路径、通用候选验证、共享工作预算，
以及实例级算法、输入约束验证器和结果能力验证器 registry。
`GraphRequestBuilder::from_request` 保留图身份并生成新的不可变前缀；
既有受检结果只能继续证明原请求，不能替代新前缀的验证。

外部消费者测试为 `novadraw/tests/graph_layout_protocol.rs`，通过 18 项测试。
该消费者注册自己的非分层算法，无需 Core 私有 API 或 rank 信息；
覆盖能力准入、身份与实例隔离、尺寸、包含与保留空间、端点、clearance、
预算耗尽及同域测量前缀编辑。此切片尚未实现默认分层算法、
Figure/Connection adapter、动态测量反馈或两个官方图场景。

## 取舍

不复制 Java 的可变输入/输出混合模型和 visitor 工作区，也不引入产品编辑器模型。
独立图请求使默认算法、第三方算法和 Headless 测试共用协议；Figure adapter 的
成本是维护明确 binding 和失效前缀，但避免把图拓扑藏进 FigureTree。
第三方算法选择留在开放入口之后，不在没有准入与失败证据时替换默认契约。

## 草案审核意见处理记录

依据 [2026-10-22 草案审核意见](../verification/reviews/adr-043-draft-review-2026-10-22.md)
完成修订 1。原报告及其基线判断保留；下列“已采纳”只表示文档修订，
不表示复审已通过或实现已验证。六项意见及补充建议全部采纳，无未采纳项。

| 意见 | 处理结果与新章节 | 理由 |
|---|---|---|
| R01 | 已采纳：[检查点恢复与结果有效性](#检查点恢复与结果有效性) | Runtime 拥有整体派生检查点，分开同源候选失败、源已变更、身份退休；明确旧快照有效性及 ADR-040 unresolved，不回滚 source |
| R02 | 已采纳：[基础结果与 typed 能力结果](#基础结果与-typed-能力结果)、[外部消费者证据](#外部消费者证据) | rank/feedback 移入 typed 分层能力；非分层外部算法不造 rank，能力不足求解前拒绝 |
| R03 | 已采纳：[binding、attachment 与完整路径的准备顺序](#bindingattachment-与完整路径的准备顺序) | 选择完整路径，候选 bounds 稳定后由 Runtime Anchor 准备同一端点；补齐 group head/tail、edge binding 与 scope |
| R04 | 已采纳：[尺寸语义、测量反馈与缓存](#尺寸语义测量反馈与缓存) | 明确 fixture 与动态测量、逐轴尺寸要求、最终宽度重新测量和同一帧收敛；约束进入缓存键 |
| R05 | 已采纳：[候选硬约束与不可变验证结果](#候选硬约束与不可变验证结果) | 分开通用/能力/adapter 验证 owner；minimum、padding、非成员、端点、revision 有硬约束与失败通道 |
| R06 | 已采纳：[独立布局域](#独立布局域)、[基础结果与 typed 能力结果](#基础结果与-typed-能力结果) | group 也携带 typed row constraint，按连续区块排序，分层 group 用 head/tail rank 区间表达 |
| 补充建议 | 已采纳：[目标与来源](#目标与来源)、[边界输入与预算](#边界输入与预算) | 固定源码提交；明确自环、零 span、空 group 和准入/分配/反馈/验证预算 |

修订 1 已获用户批准，实施门禁通过；P2-L01 进入实施，完成状态仍以验证证据为准。
