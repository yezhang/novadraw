# ADR-043：有向图布局的独立模型、开放求解器与 Figure 投影

类型：`architecture-decision`

## 状态

提议，待评审（2026-10-22）。尚未实施，不代表 P2-L01 已完成。
承接官方示例 K06；`api_semantics` 为 `layout.graph`、`layout.manager`、
`connection.routing`、`figure.measurement` 和 `scene.preparation`。

## 目标与来源

DirectedGraphDemo 与 CompoundGraphDemo 需要根据拓扑求解节点位置、rank、
跨层边折点和子图矩形。它们消费算法输出，再创建 Label、子图边框和 Connection。
图的包含关系是算法输入；官方 Compound 场景中的 Figure 仍在同一 Panel 下。
因此不能把 FigureTree 的父子关系直接当成图的包含关系。

只读参考 `org.eclipse.draw2d.graph` 与官方 `examples.graph`。不扫描现状实现
来定义理想模型，不使用 Zest，不编写、编译或运行 Java。以下源码已核对：

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
| GraphNode | 请求域内 NodeId、尺寸、可选 padding、可选 row constraint、incoming/outgoing attachment、可选所属 GroupId |
| GraphGroup | 请求域内 GroupId、可选父 group、minimum size、外部 padding、border insets、inner padding、attachment |
| GraphEdge | 请求域内 EdgeId、source/target endpoint、minimum rank span、weight、width、padding、可选 attachment override |
| GraphLayoutRequest | nodes、groups、edges、方向、默认 padding、margin、算法身份与配置、工作预算 |

NodeId、GroupId、EdgeId 独立于 FigureId，并由请求 builder 验证归属。
节点与 group 都允许作为边端点；endpoint 的封闭值类型表达实体种类，
不承担算法或扩展行为的分派。算法行为走开放 registry。
节点没有多个 parent；group 包含关系必须无环，深度受检且不超过 10,000。
尺寸与 Insets 有限、非负；rank span 与权重的合法范围由默认算法配置明确约束。
旧 Java 的 `-1` sentinel 分别转为 `None`，不得进入公共 Rust 几何协议。

空图返回空结果。默认 Draw2D 算法要求弱连通：普通图检查边的无向连通性，
compound 检查边与包含关系的并集；非法输入结构化拒绝，不偷偷补边或分块。
扩展算法可以有不同准入能力，但必须显式声明并接受同一请求验证。

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

候选结果必须经 `GraphLayoutResult::validate`，才能用于 Figure 投影。结果包含：

- 每个输入 node/group 的 bounds 与 rank 信息；
- 每条输入 edge 的 source→target 有序 points、feedback 标记和跨层辅助几何；
- 图 extent、方向、请求 revision 与可选布局诊断。

禁止遗漏、重复或补造输入实体身份；virtual nodes 仅是 edge 的派生辅助数据，
不进入消费者节点身份空间。校验有限几何、extent、完整身份、路径方向和 group
包含关系。算法声明的 rank 约束按其消环方向检查，feedback 不能改写输入端点。
权重优化与交叉减少通过默认算法独立测试验证，不假装几何校验器可以证明全局最优。

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

同一不可变结果同时供 LayoutManager 节点 bounds 和 RouteGroupSolver 连接 points
使用。服务缓存属于实例，缓存键至少包含请求 revision、binding、算法配置、
受检 measurement/style/resource generation。只能引用当前稳定场景前缀；
旧图的 FigureId 不能用于新 contents。

bounds 与 route 通过已有 LayoutOutput、连接组和派生状态收敛事务发布，延续
[ADR-040](adr-040-group-route-solving.md) 与
[ADR-042](adr-042-structured-flow-layout.md)。若现有协议无法保证同一结果的节点和
路径同时稳定，必须先报告契约缺口、补充设计，不能在 adapter 中自行绕过事务。
任一测量、求解、校验或 Locator 失败保留上一稳定结果；新内容替换的准入失败
保留旧树。不得在 paint 期间求解或逐节点发布部分结果。

节点测量与图布局造成的 bounds generation 分开归因，避免布局自己触发无限求解。
释放服务/binding 时不保留 source Figure 引用。预算耗尽返回带阶段信息的错误，
后续合法输入可以恢复；不吞错误、生成 fallback 网格或发布假 rank。

## 外部消费者证据

实施必须包含一个独立外部算法，用 typed key 注册自己的配置、生成结果并接入
同一 Figure adapter 与 route group。Core 不依赖该类型，也不提供内置快路径。
另外包含无 Runtime 的纯图消费者，证实图身份与 Figure 生命周期已经分离。

验证外部算法的失败、非有限结果、缺失/重复节点、错误端点、非法 group bounds，
确认同一结果门禁拒绝发布。外部普通 Figure 提供测量，验证其字体或尺寸变化
使图请求失效，且重复稳定帧不会重新求解。

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

## 实施顺序与门禁

1. 评审本 ADR，再实施纯请求/结果、身份验证与开放算法 registry。
2. Directed 默认阶段与 16 fixture 的两方向契约。
3. Compound 阶段与 10 fixture 的两方向契约。
4. 统一 Figure/Connection adapter，外部算法与失败恢复契约。
5. 官方两个场景和宿主切换，逐项维护语义账本与 Canvas 清单。

每次只推进一个切片；按风险运行精确测试与 crate lib check/Clippy。
最终运行 workspace fmt/check/Clippy/test、文档门禁和代表性 Native 验证。
在上述证据齐全前，P2-L01 与两个 graph 入口均不能登记完成。

## 取舍

不复制 Java 的可变输入/输出混合模型和 visitor 工作区，也不引入产品编辑器模型。
独立图请求使默认算法、第三方算法和 Headless 测试共用协议；Figure adapter 的
成本是维护明确 binding 和失效前缀，但避免把图拓扑藏进 FigureTree。
第三方算法选择留在开放入口之后，不在没有准入与失败证据时替换默认契约。
