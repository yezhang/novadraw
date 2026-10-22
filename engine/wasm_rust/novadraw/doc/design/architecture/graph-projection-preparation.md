# 图投影的候选快照查询

类型：`normative-design`

状态：`proposal`，待评审（2026-10-22）。
属于 P2-L01 的 Figure/Connection adapter 切片，补充
[ADR-043](../../adr/adr-043-directed-compound-graph-layout.md)。
纯请求、Directed 和 Compound 切片已实现；本提案尚未实施。
`api_semantics`：`scene.preparation`、`figure.measurement`、`connection.routing`。

## 实施前发现的契约缺口

候选 bounds 已可通过现有 Anchor 查询读取。Runtime 先应用候选 LayoutOutput，
随后用同一 FigureTree 构造 FigureTreeRouteSceneRead；border_box 和 map_rect
读取当前树中的几何，并未固定到上一稳定帧。因此无需增加另一套候选坐标转换。

缺少的是同一候选的受检图结果、身份和测量查询。以下结论来自静态源码核对，
没有以未执行的运行测试作为证据：

| 入口 | 当前行为 | 对 adapter 的影响 |
|---|---|---|
| `novadraw/src/connection/query.rs`：RouteSceneRead、RouteSceneQuery | 只提供几何、可见性、拓扑与坐标映射；不提供 prepared presentation 或领域数据 | Anchor/Router 无法从 Runtime 取得本次 GraphLayoutResult 和 attachment boundary 证据 |
| 同文件：DependencySubject、dependency_generation | 路由观察只包含几何、可见性、命名几何、变换、拓扑 | bounds 不变但请求、算法配置或完整路径改变时，已有 resolved route 没有结果身份依赖 |
| `novadraw/src/graph/query.rs`：prepared_presentation | 普通 FigureTree 查询可以借用已有 presentation | Router 只获得 RouteSceneQuery，不能借助此方法绕过受检查询权限 |
| `novadraw/src/graph/layout_measurement.rs`：preferred_measurement、minimum_size | 无 layout manager 且已有 prepared 值时，直接返回该值；新 constraints 不参与重新 preparation | EAST 候选宽度不能靠这两个调用完成任意外部 prepared Figure 的重新测量 |
| `novadraw/src/figure/preparation/scene.rs`：revision | 包含 bounds 与 layout generation | 派生布局写入会改变该 revision，不能直接作为 graph 的 source revision |
| `novadraw/src/runtime/runtime/frame_submission.rs`：resolve_dirty_connection_routes、stabilize | 路由错误记入 connection_error；组 unresolved 本身不阻止 stable epoch 晋升 | placement 成功、路径失败的图候选需要额外的整体发布检查 |
| `novadraw/src/graph/derived.rs` 与 `novadraw/src/connection/runtime.rs`：检查点恢复 | 恢复树几何与 preparation；连接恢复依赖后统一标为 Dirty | 尚未区分同源有效旧结果与源已改变后的 invalid/unresolved |

在 Anchor/Router 与布局器之间放置可变结果槽，会让检查点外的缓存成为候选权威，
且现有 Runtime 无法恢复该槽。用 RoutingConstraint 更新结果又会把派生写入变成
source mutation。两种方式均不满足 ADR-043 的整体检查点契约。

本提案保留原 ADR 的独立图模型、开放 registry 和完整路径决策，
增加通用 preparation 查询与发布约束。引擎不识别 Directed 或 Compound 类型。

## Runtime 持有候选快照

沿用 `FigurePresentation::with_data` 的不可变领域数据承载方式。
graph adapter 生成 `GraphProjectionSnapshot`，包含受检 GraphLayoutResult、
不可变请求、完整 binding、routing scope、source 前缀和最终测量记录。
算法结果能力仍通过 GraphLayoutResult 的 typed registry 查询；
GraphProjectionSnapshot 只组合数据，不分派算法行为。

Runtime 为每次受检 preparation 分配 `PreparedSceneIdentity`。身份字段私有，
包含 owner 与不复用的快照身份，不能由消费者构造。相同几何下换了领域结果，
也必须产生不同身份。只改变绘制 damage、不改变快照时保留身份。

公开类型放在 `novadraw::figure`；拟采用以下查询形状：

```rust
pub struct PreparedSceneSnapshot { /* private fields */ }

impl PreparedSceneSnapshot {
    pub fn identity(&self) -> PreparedSceneIdentity;
    pub fn presentation(&self) -> &FigurePresentation;
}

// RouteSceneRead 与 RouteSceneQuery 使用相同的只读权限。
fn prepared_scene(
    &mut self,
    owner: FigureId,
) -> Result<PreparedSceneSnapshot, RouteSceneQueryError>;
```

RouteSceneRead 版本使用 `&self`。为已有外部 query 实现提供默认的结构化
UnsupportedQuery 错误；不把查询失败或过期数据表示为合法 None。
RouteSceneQuery 新增 `DependencySubject::PreparedScene(owner)` 观察，
generation 来自 Runtime 持有的快照身份。TrackedRouteSceneQuery 负责观察，
外部 Anchor/Router 无需手工登记依赖。

稳定期只返回当前 source 匹配的已提交快照。派生期只在 Runtime 发给
Anchor/Router 的查询中返回本检查点已校验的候选；普通宿主查询不能选择候选分支。
请求未准备、source 已漂移、身份退休或结果 invalid 时返回受检错误。
消费者持有旧快照只具有诊断权限，不能凭其 identity 准入当前路由。

attachment-aware Anchor 每次从容器快照取得 edge 的端点记录，并读取 owner 的
候选 border box，验证 binding、bounds、方向及 boundary 归属后返回位置和法线。
RouteGroupSolver 从同一快照取得完整 points；组内必须共享快照 identity。
Runtime 继续验证 ConnectionId 覆盖与 Anchor metadata，不能让 solver 自行省略。

## 候选测量使用显式约束

图适配层初始测量使用配置的 outer constraints。得到候选宽度后，用该宽度、
未限定的高度重新测量；不把上一轮 bounds 当作 source 尺寸。

对 intrinsic Figure 与 LayoutManager，继续使用 FigureTree 的现有盒模型组合。
对支持 preparation 的 Figure，补充可失败的约束测量方法：

```rust
fn measure_scene(
    &self,
    context: &mut MeasureContext<'_>,
    constraints: MeasureConstraints,
    scene: &PreparationScene<'_>,
) -> Result<PreparationMeasurement, TextError>;
```

`PreparationMeasurement` 只含受检 preferred measurement 与 minimum size，
不包含 attached bounds，也不承诺 drawing 已准备完成。返回值处于该 preparation
现有 content measurement 域，Runtime 统一处理 Border、insets 和容器正投影。
constraints 按现有 outer→扣 insets→反投影顺序传给 owner。
非法尺寸、baseline、派生溢出与不支持约束测量分别进入结构化失败通道。

默认实现返回明确的不支持错误。已有只按当前 client bounds 准备的 Figure
继续可绘制；动态 graph binding 只有在其能证明约束测量时才能准入。
fixed fixture binding 使用请求中的源码尺寸，完全跳过动态测量。
不能向 prepare 传无限 Rectangle，也不能用旧 prepared metrics 代替新约束结果。

PreparationScene 增加统一的 `measure_figure` 入口，由 Runtime 测量服务委托上述
方法或普通 FigureTree 测量。adapter 不自行调用 intrinsic 方法、解析字体或加 insets。
后代 Layout/Presentation 仍在检查点内准备；其最终需求变化时重建请求再求解。
同一稳定化过程共享预算，不能每轮重置 GraphSolveContext。

## source 前缀与派生前缀分开记录

source 前缀记录组件 revision、拓扑、binding、算法配置、测量约束、样式和资源
generation；不记录图布局自己写入的 bounds。现有 PreparationScene.revision
保留原用途，另提供 source 前缀读取，不改变 Flow 消费者的现有失效语义。

Runtime 单独记录候选 bounds、prepared identity 与后代测量 generation。
source 未漂移且最终约束与测量需求相同即可稳定。source 已漂移时拒绝整批；
后代需求变化时消耗同一预算后重求解。成功缓存记录初始 source 测量和最终
候选约束两份记录，稳定帧不重复从初始无界测量启动反馈。

## preparation 声明必需路由

PreparedScene 增加 `require_route_group`，声明显式 routing scope 与全部
ConnectionId，作为该 preparation 成功发布的必要条件。它只声明依赖，
不注册连接、不更改 source binding、不调用求解器。

Runtime 校验连接属于准备 owner 的 flat child-content 域，组无重复且覆盖完整。
稳定化结束前检查组成员已 resolved，且结果依赖本次 prepared identity。
Anchor、route、Locator、Decoration 失败或前缀漂移均拒绝晋升 stable epoch。
未声明必需路由的普通 Figure 仍沿现有 ADR-040 失败通道处理。

检查点包含必需组声明、prepared identity、布局与后代 presentation、
连接 metadata/依赖/状态、Locator/Decoration、通知和 damage。
图结果缓存以 Runtime 持有的不可变快照为权威，失效候选不进入成功缓存。

恢复时先比较当前 source 与 binding，再决定旧结果权限：

| 当前前缀 | 恢复结果 |
|---|---|
| 与上一成功结果完全一致 | 恢复旧几何、快照和 resolved metadata；保留本次候选错误供诊断 |
| source、binding 或 measurement 前缀改变 | 旧几何只供上次已呈现帧和诊断；当前 projection invalid，必需组 unresolved，拒绝新 submission |
| 身份删除或退休 | 不恢复已删除实体；清理退休 binding、结果缓存和反向依赖，其余受影响组 unresolved |

恢复依赖必须保留当前 source 的重新准备触发条件。合法输入修正后，
重新冻结当前前缀并通过完整批次，再恢复 valid/resolved。
稳定查询和 hit 不得把 invalid 的图投影报告为当前有效结果；
既有 FigureTree 原始几何查询的诊断用途需在文档中明确。

## 外部消费者和验收

外部非分层算法、外部 prepared Figure 与默认算法共用所有新增入口。
消费者只需实现公开 trait、使用 typed registry 和 immutable presentation 数据，
不接触 Core 私有字段，也不增加内置能力分支。

| 反例 | 必须验证的结果 |
|---|---|
| 首帧从旧 bounds 到候选 bounds | Anchor、完整 points、Locator 与 glyph 使用同一候选，无第二帧修正 |
| bounds 相同，请求或路径改变 | PreparedScene 依赖失效，重新准备整组；不继续返回旧 resolved route |
| 候选 payload identity 或 attachment 不匹配 | 整批拒绝，不晋升 epoch，不 flush 候选通知 |
| EAST 改宽，外部 prepared Figure 改高度 | 显式约束重测量并首帧收敛；重复稳定帧不重算 |
| 测量振荡、非法值或共享预算耗尽 | 结构化失败；失败值不入缓存，修正后恢复 |
| 同源路由失败 | 恢复仍匹配 source 的旧有效结果；普通组失败语义不变 |
| 改源后路由失败、删除端点或退休 binding | 源变化保留，旧结果 invalid，受影响组 unresolved，无新 partial submission |
| contents 成功替换后旧服务再查询 | 旧身份拒绝，无旧树或旧依赖复活 |

通过此增量评审后，按候选查询与必需组发布→测量/source 前缀→graph adapter
的顺序实施。随后接入两个官方场景、真实 fixture/方向切换与 Native 截图，
补齐书籍和语义账本，完成最终门禁后关闭 P2-L01。
