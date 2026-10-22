# ADR-043 草案审核意见

类型：`architecture-review`

## 结论与基线

**建议修改后复审，暂不通过实施门禁。**

独立图模型、实例级服务、开放算法入口和 flat Panel 投影的方向可以保留。
需要补齐的是公共结果、端点和失败发布契约，无需推翻整体方案。

本文件保留 2026-10-22 会话中的审核意见，供另一个 agent 修改
[ADR-043](../../adr/adr-043-directed-compound-graph-layout.md)。
审核对象为“提议，待评审”的 189 行草案，尚未实施。
下文行号均指此基线，修改后应按章节和引用原句定位。

- 仓库提交：`141462da89ffc74b4dd6db951e610bca090cd923`。
- ADR-043 文件 SHA-256：`7c7285790944c4bb61133a6dfb6fc849b95958972a3b829e21c95b63eb866b3e`。
- Draw2D 参考仓库：`/Users/bytedance/Documents/code/GitHub/gef-classic`。
- 参考提交：`4463d9d0ce13c19d10fbe769d29f28b7345a8cba`。

本次审核核对了相关 ADR、语义账本和 Draw2D 源码，没有扫描 Novadraw Rust
实现，没有编写、编译或运行 Java，也没有执行实现测试。
以下发现是方案的契约缺口，不是已经复现的 Rust 实现缺陷。
本报告记录历史判断，不覆盖 ADR 或其他已接受规范。

## 交接范围

接手 agent 先按 `AGENTS.md`、`CLAUDE.md` 的启动规则读取上下文，
再修改 ADR-043 及必要的文档引用。理想架构修订继续遵守禁止先扫描本项目
Rust 实现的规则，参考范围限定 Draw2D/GEF，不使用 Zest。

本次用户要求保存审核意见。后续是否批准方案或授权实施，应由用户另行决定；
保存本报告不表示 ADR-043 已接受，也不表示 P2-L01 已完成。

修改时逐项给出处理结果、对应的新章节和理由。若不采纳某项建议，提供源码或
已接受契约的依据。保留本报告的原始判断，复审结论另行记录。

## R01：明确保留旧结果的范围

严重级：`P1`。位置：ADR-043“Figure 投影与稳定发布”，135–140 行。

草案规定“任一测量、求解、校验或 Locator 失败保留上一稳定结果”；
[ADR-040](../../adr/adr-040-group-route-solving.md)的组输出验证则规定失败后
清空路线、进入 unresolved。两种失败行为的适用边界尚未明确。

建议明确图布局是否整体进入
[ADR-042](../../adr/adr-042-structured-flow-layout.md)的派生检查点，
并区分候选失败、源状态已改变、端点删除三种情况。
旧派生快照不能因为“保留旧结果”继续被当作当前有效结果；
恢复也不能撤销已提交的源属性、拓扑或 binding。

复审检查：写清检查点 owner、恢复范围及恢复后的有效性；
说明与组路由 unresolved 的关系，并覆盖上述三类失败及后续恢复。

## R02：公共结果不应强制要求分层信息

严重级：`P1`。位置：ADR-043“开放算法与结果验证”，86–94 行。

草案要求所有算法返回 rank、feedback 和跨层辅助几何。
即使 registry 开放，非分层算法仍需生成没有实际含义的数据。

建议基础结果保留身份、placement、extent 和明确的路由输出；
rank、feedback、rank span 验证放入算法声明的受检能力或 typed 扩展结果。
消费者要求某项能力而算法不支持时显式拒绝。

复审检查：外部消费者证据至少包含一个不产生 rank 的算法；
明确能力声明、请求要求和结果校验如何共用开放协议，
不要求外部实现生成假 rank 或修改 Core 枚举。

## R03：明确 attachment 与 Anchor 的端点责任

严重级：`P1`。位置：ADR-043“Figure 投影与稳定发布”，126–138 行。

草案要求同一结果供节点布局和路由使用，但 ADR-040 的端点由 Runtime
调用 Anchor 准备。若 Anchor 读取旧 bounds，新结果路径可能与受检端点不一致。

Compound 的端点规则还依赖包含关系：
`org.eclipse.draw2d.graph.ConvertCompoundGraph.convertSubgraphEndpoints`
按对端是否属于 group 选择 head/tail；
`RouteEdges.revisit` 对 subgraph head 还有内边界端点规则。
仅规定 group 可作为 endpoint，尚不足以确定这些行为。

建议规定候选 bounds、attachment、Anchor metadata 和 points 的准备顺序
及一致性检查；明确采用完整算法路径还是 routing hints。
不能同时让算法和 Anchor 独立决定最终端点。
同时补充 edge→ConnectionId、routing scope 及 binding 覆盖范围的约束。

复审检查：新节点 bounds 与路线端点来自同一候选；
覆盖 group 与成员之间的边、跨 group 边及方向变化，
并说明完整路径或 hints 如何进入既有路由准入通道。

## R04：补齐测量与布局反馈契约

严重级：`P1`。位置：ADR-043“默认算法的语义切片”，107–108 行；
“Figure 投影与稳定发布”，126 行、142 行。

草案允许拉伸节点，尤其 EAST 会改变宽度，但只说明 generation 分开归因。
宽度变化可能引起文字换行和高度变化，仅分开 generation 无法保证收敛。

建议定义输入尺寸属于固定尺寸、minimum 还是 preferred；
明确测量约束、最终 placement 后是否重新测量，以及缓存如何记录约束。
普通图 fixture 的显式尺寸和动态 Figure 测量也应分开规定。
测量继续遵循 [ADR-018](../../adr/adr-018-runtime-driving-and-measurement-api.md)、
[ADR-038](../../adr/adr-038-query-identity-and-failure-semantics.md)和
[深层布局与 Presentation 收敛契约](../../design/architecture/deep-layout-presentation-convergence.md)。

复审检查：增加宽度敏感 Figure 的 EAST 回归要求，
检查首帧稳定、失败恢复和重复帧不重算；
明确测量约束改变与算法派生 bounds 改变各自如何触发失效。

## R05：补全候选的硬约束验证

严重级：`P1`。位置：ADR-043“开放算法与结果验证”，92–95 行。

草案明确了身份、有限几何和包含关系，但尚未明确验证 minimum size、
inner padding、非成员排除、attachment 一致性及请求 revision。
有限矩形且包含成员，仍可能侵占保留空间或包住外部节点。

建议列出通用校验与算法能力校验各自负责的不变量；
候选必须针对具体不可变请求验证，成功后生成不可随意修改的受检结果。
补充“身份齐全、几何有限，但违反硬约束”的外部算法反例。
全局最优继续由算法测试证明，这项取舍可以保留。

复审检查：各项硬约束都有明确校验责任和结构化失败通道；
受检结果绑定对应请求，修改候选后不能沿用先前验证结论。

## R06：补充 group 的 row constraint

严重级：`P2`。位置：ADR-043“独立布局域”字段表，48–49 行。

字段表只给 GraphNode 提供 row constraint；
Draw2D 的 `CompoundRankSorter.copyConstraints` 同时读取 subgraph
和普通节点的约束。

建议补充 group 排序约束，说明其作用范围及与成员排序、连续包含区域的关系。
默认分层结果中的 group rank 还应明确是区间还是边界信息，
避免把跨多个 rank 的 group 当成单 rank 节点。

复审检查：group 排序约束有输入表示和验证要求；
group rank 的含义足以支持其端点及 rank span 检查。

## 补充验收建议

以下两项承接原会话的补充建议，不另行升级为 P1：

- 在 ADR 中固定参考源码提交号，本次核对为
  `4463d9d0ce13c19d10fbe769d29f28b7345a8cba`。
- 明确自环、零 rank span、空 group 的准入策略，
  以及预算是否覆盖请求验证、辅助数据分配和结果验证。

## 修改清单与复审条件

以下清单用于接手时逐项核对。当前均为待处理，不表示建议已经获批或完成。

- [ ] R01：失败保留、检查点恢复和 unresolved 的适用边界一致。
- [ ] R02：非分层外部算法无需生成 rank，能力不足可受检拒绝。
- [ ] R03：bounds、attachment、Anchor 和 points 的准备顺序与责任明确。
- [ ] R04：尺寸语义、测量约束、反馈收敛和缓存失效明确。
- [ ] R05：通用及能力校验覆盖硬约束，并绑定具体不可变请求。
- [ ] R06：补齐 group 排序约束及其 rank 含义。
- [ ] 在 ADR 固定参考提交，明确边界输入和预算覆盖范围。
- [ ] 给出各项处理结果、新章节引用及不采纳建议的依据。

通过条件：先补齐 R01–R05 的 P1 契约，再固化公共协议。
默认算法的阶段划分和 52 个官方输入组合验收方案可以继续保留。
复审通过不等于实现完成；P2-L01 状态继续由
[P2 backlog](../../roadmap/p2-delta-backlog.md)维护。

文档修订后按仓库规则运行文档门禁和空白检查，
并复核本次涉及的本地链接、章节引用及与已接受 ADR 的关系。
本审核没有执行实现测试，后续实现证据不能以本报告替代。
