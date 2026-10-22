# 图投影的候选快照查询

类型：`normative-design`

状态：`accepted / implementing`，修订 2（2026-10-22，用户授权按 ADR-043 和增量方案推进 Core 编码）。
已按[增量评审意见](../../verification/reviews/graph-projection-preparation-review-2026-10-22.md)
处理 GP01–GP04。原评审报告保留其基线判断；当前执行授权来自修订后的用户指令。
属于 P2-L01 的 Figure/Connection adapter 切片，补充
[ADR-043](../../adr/adr-043-directed-compound-graph-layout.md)。
纯请求、Directed 和 Compound 切片已实现；本增量进入实施，完成状态仍以验证证据为准。
`api_semantics`：`scene.preparation`、`figure.measurement`、`connection.routing`。

Core 实施进度：约束测量与结构化错误已完成首批；候选快照查询、稳定领域结果版本、
源 bounds 与派生 bounds 分离、路由快照观察和必需组发布屏障已接入。
外部验证入口为 `p2_preparation_measurement_contract` 和
`p2_prepared_scene_routing_contract`。快照携带计算观察，路由环检测展开这些观察；
preparation 查询自身快照返回有序结构环，必需组准入拒绝直接或经快照读取
route/Locator 子树的几何与测量。原始 Figure/capability 借用进入路由子树时
也记录几何读取；children/parent 源查询不增加此类边。缓存复用核对观察代数，
身份比较不依赖观察顺序，受检查询拒绝计算观察已漂移的快照。
必需组连接另存完整稳定化成功记录，失败后核对绑定版本、快照有效性及依赖版本；
同源恢复旧 resolved generation，改源或改绑定保留错误与当前观察并拒绝旧 resolution。
恢复后必需组未 resolved 的 owner 及后代 presentation 标为 invalid；
受检快照查询拒绝读取，不能通过显式 resolve 绕过重新 preparation。
首帧尚无成功记录时保留重试状态；普通路由失败继续走既有通道。
完整 LayoutManager 传递观察、批次候选依赖图、
约束测量的传递观察与共享预算仍待后续批次；这不是整个增量的完成声明。
下表保留实施前基线，不能用来判断上述入口的当前代码行为。

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

Runtime 持有 `PreparedSceneIdentity`，身份字段私有，包含 owner 与不复用的
快照身份，不能由消费者构造。preparation 尝试与实际快照替换分别记录；
身份更新条件见下节。尝试次数不参与路由依赖 generation。

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

## 快照未改变时保留 identity

Runtime 为内部计算尝试记录 attempt 标识，仅用于预算和诊断；
`PreparedSceneIdentity` 标识受检的不可变结果及其输入前缀。
相同 owner、source、binding、算法配置/验证器版本、测量约束与需求、计算依赖
版本和领域结果重复准备时，复用当前受检快照及 identity，返回 NoChange。
不得因 Locator child 更新、layout generation 改变或 prepare 回调再次执行，
就分配新 identity、失效路由或发送结果改变通知。

领域 payload 可能是不透明的外部数据，Runtime 不按具体算法 downcast 比较。
producer 必须随 PreparedScene 提供稳定的领域结果版本，并在读取前缀或结果改变时
更新它；相同版本下的 payload、placement、attachment、完整 points 与能力证据
必须等价。Runtime 同时核对冻结的完整输入与计算依赖，版本不能替代输入验证。
通用 layout/presentation 输出按各自的受检变化规则处理，纯绘制 damage 不改变
路由消费的领域结果版本。版本协议对内置和外部 producer 相同。

graph adapter 复用同一合法前缀的不可变请求和受检 GraphLayoutResult；
不能每次回调都重建一个随机 request identity，再把它解释为 source 已改变。
如果显式 source/request revision、binding、约束、算法配置或领域结果版本改变，
即使 bounds 相同，也要替换快照并分配新 identity。路径、attachment 或能力证据
改变同样属于结果替换。新 identity 只在候选完整校验成功后进入检查点，
被拒绝的尝试不能覆盖旧 identity。

`DependencySubject::PreparedScene(owner)` 观察当前快照替换的 generation。
相同 identity 保持 generation；替换或有效性失效才改变 generation。
不能将 attempt 标识、回调计数或普通子树 layout generation 作为该 generation。
回退到旧有效分支时恢复它的 identity 和依赖观察，不复用已拒绝候选的身份。

## 候选测量使用显式约束

图适配层初始测量使用配置的 outer constraints。得到候选宽度后，用该宽度、
未限定的高度重新测量；不把上一轮 bounds 当作 source 尺寸。

preferred 与 minimum 分别遵循现有权威顺序，按实际查询选择测量来源：

| 优先级 | 选中来源 | 处理 |
|---|---|---|
| 1 | 对应显式 preferred/minimum override | 作为 unprojected outer 值校验并正投影；跳过 LayoutManager、preparation 和 Border/insets expansion |
| 2 | LayoutManager | 在反投影后的 content constraints 下测量；由统一入口展开 insets 并正投影 |
| 3 | 实际进入 prepared 测量分支 | 用下述约束测量替换旧 prepared metrics 的直接返回 |
| 4 | intrinsic Figure | 保留现有 intrinsic/content、Border 组合与 minimum 的 preferred 回退规则 |

是否注册 preparation capability 不改变 override 或 LayoutManager 的优先级。
普通无 prepared 值的 intrinsic 查询保留；动态 graph binding 需要 preparation
测量时必须显式准入该分支，不能以尚无快照为由借 intrinsic 绕过不支持错误。
preferred 有 override 不代表 minimum 也有 override，两项独立分派。

对实际选中的 preparation 分支，补充可失败的约束测量方法：

```rust
fn measure_scene(
    &self,
    context: &mut MeasureContext<'_>,
    constraints: MeasureConstraints,
    scene: &PreparationScene<'_>,
) -> Result<PreparationMeasurement, PreparationMeasureError>;
```

`PreparationMeasurement` 只含受检 preferred measurement 与 minimum size，
不包含 attached bounds，也不承诺 drawing 已准备完成。返回值处于该 preparation
现有 content measurement 域，Runtime 统一处理 Border、insets 和容器正投影。
constraints 按现有 outer→扣 insets→反投影顺序传给 owner。
Border/insets 按所选来源只组合一次。返回值已有的内容 padding 保留现有定义，
不得由 adapter 或 callback 再补一次物理 Border；override 不执行内容展开。
每个输入、中间结果和最终正投影结果均受检，minimum 的 preferred 回退使用同一
通道。非对称 insets 和容器 scale 不改变这一顺序。

默认实现返回明确的不支持错误。已有只按当前 client bounds 准备的 Figure
继续可绘制；动态 graph binding 实际选中 preparation 分支时，只有在其能证明
约束测量时才能准入。override 或 LayoutManager 成功的查询不触发该默认错误。
fixed fixture binding 使用请求中的源码尺寸，完全跳过动态测量。
不能向 prepare 传无限 Rectangle，也不能用旧 prepared metrics 代替新约束结果。

PreparationScene 增加统一的 `measure_figure` 入口，由 Runtime 测量服务委托上述
方法或普通 FigureTree 测量。adapter 不自行调用 intrinsic 方法、解析字体或加 insets。
后代 Layout/Presentation 仍在检查点内准备；其最终需求变化时重建请求再求解。
同一稳定化过程共享预算，不能每轮重置 GraphSolveContext。

## 测量错误保留主体和原始原因

新增 `PreparationMeasureError`，放在 `novadraw::figure`，替代新测量接口中的
TextError 返回值。Runtime 的 `measure_figure` 统一包装 callback 和权威测量失败：

```rust
pub struct PreparationMeasureError {
    pub figure: FigureId,
    pub outer_constraints: MeasureConstraints,
    pub content_constraints: Option<MeasureConstraints>,
    pub kind: PreparationMeasureErrorKind,
}

pub enum PreparationMeasureErrorKind {
    UnsupportedConstraintMeasurement,
    InvalidSize { metric: PreparationMetric, value: Dimension },
    InvalidBaseline { baseline: f64, height: f64 },
    DerivedOverflow { stage: PreparationMeasureStage },
    Layout(LayoutError),
    Text(TextError),
    Dependency(PreparationDependencyError),
}
```

以上代码描述增量的目标协议。Core 测量切片已实现 `PreparationMeasurement`、
测量主体与阶段分类，以及 Layout/Text 原因包装；实际实现将 Layout、Text 和
Dependency 原因保存在 Box 中，避免扩大每个 Result 的错误值，原始错误及
`Error::source` 契约不变。Dependency 已覆盖测量重入、快照结构环、
必需路由输出反向读取和深度限制；LayoutManager 传递观察与共享预算仍待后续切片。
`PreparationMetric` 区分 preferred 与 minimum；`PreparationMeasureStage` 标识
constraint shrink、constraint inverse projection、Border/insets expansion、
measurement projection。两者表达固定测量步骤，不分派内置或外部行为。
`content_constraints` 在反投影成功后为 Some；此前失败时为 None。
合法 unbounded 约束与零尺寸不属于非法结果。

callback 从 PreparationScene 取得测量主体及当前约束；Runtime 核对并补全本次
调用上下文。嵌套测量失败保留实际失败 FigureId、该主体的 outer/content constraints
及原因，不能覆盖为 graph 容器身份。调用链可以另附 owner/阶段用于定位。
非法成功尺寸与 baseline 由权威入口分别分类；有限输入产生无穷的组合结果归
DerivedOverflow，不能归为“不支持”或把它夹取为零。

LayoutError、TextError 与依赖错误保留原始值；`Error::source` 返回包装原因，
不先格式化为字符串。在通用 prepare_scene/prepare 的既有 TextError 边界，
新增持有 `Box<PreparationMeasureError>` 的结构化包装变体；FramePreparationError
继续沿 Text 链传播。直接测量入口返回 PreparationMeasureError。
新测量路径禁止经过 `scene_error` 转为 `InvalidPreparedScene(String)`。
既有非测量 TextError 的行为不因本提案改变。

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
ConnectionId，作为该 preparation 成功发布的必要条件。
`require_route_group` 是发布完成屏障，单独存储，不向快照计算依赖图增加
“preparation 读取 route 输出”的边。它不注册连接、不更改 source binding，
也不调用求解器。

Runtime 校验连接属于准备 owner 的 flat child-content 域，组无重复且覆盖完整。
稳定化结束前检查组成员已 resolved，且结果依赖本次 prepared identity。
Anchor、route、Locator、Decoration 失败或前缀漂移均拒绝晋升 stable epoch。
未声明必需路由的普通 Figure 仍沿现有 ADR-040 失败通道处理。

## 快照计算依赖参与结构环检测

Runtime 单独记录 preparation 的实际计算读取，区分源属性与派生输出。
下列关系采用“消费者→被读取的供应者”方向：

- preparation 测量或求解读取 geometry、measurement 或另一个 prepared snapshot，
  记录对应计算依赖；
- Anchor/Router 读取 `PreparedScene(owner)`，记录 route→prepared snapshot；
- Locator/Decoration 读取 route，记录其派生输出→route。

包含全部 ConnectionId 的 source binding 或拓扑查询只读取源数据，
不等于读取 Connection points、route bounds 或 Locator placement。
group 发布屏障不会使合法的 prepared snapshot→route 消费链被误报为环。
测量记录必须保留传递依赖，缓存命中也携带这些观察；
不能以读取的是上一帧 resolved route 或已缓存 metrics 为由省略依赖。

每次新增计算读取时，Runtime 对既有连接依赖及 preparation 计算依赖执行可达性
检查。已知结构关系在 binding/必需组准入时预检；动态关系在实际查询和完整候选
准入时检查。`PreparedScene(owner)` 通过该快照的计算依赖向外展开，
跨 preparation、measurement、layout 和 route 的路径参加同一结构环判定。
发现环立即拒绝新增依赖及相关候选，不执行后续发布或等待反馈预算耗尽。
这只是既有 DependencyCycle 检测的扩展，不开放任意 DAG scheduler。

对于消费快照 P 的 route C，P 不能直接或传递读取 C 的 route、Locator/Decoration
geometry，或由这些输出反向派生的 envelope/range。直接
P→C→P、P→Locator(C)→C→P，以及跨 owner 的
P→Q→C→P 均须拒绝。即使尚未形成闭环，也要拒绝 P 对其已声明必需组输出的
反向计算依赖；该限制使计算方向保持 snapshot→route→post-route。

新增 `PreparationDependencyError::DependencyCycle`，携带有序闭环路径及对应
prepared owner、ConnectionId 或 geometry/measurement 主体。
其他已禁止的反向读取使用 `ForbiddenRouteOutputDependency`，携带读取者和供应者。
测量发生的错误包装为 PreparationMeasureErrorKind::Dependency；
prepare_scene 边界用 TextError 的结构化依赖包装，保留 `Error::source`。
从 Anchor/Router 发现的闭环沿现有 RouteError::DependencyCycle 通道拒绝，
同批诊断保留有序路径，禁止转为字符串或泛化 DidNotConverge。

环检测和遍历也消耗同一预算。结构可达性已证明有环时报告 DependencyCycle；
尚未完成检查就耗尽预算时报告预算错误，不能伪称已证明有环。
失败按本提案的 source/检查点恢复规则处理，保留当前源依赖供合法修正触发重试。
修正 binding 或测量读取后重建当前依赖；删除/替换 owner 时退休旧依赖边。

## 检查点恢复

检查点包含必需组声明、prepared identity、布局与后代 presentation、
快照计算依赖、连接 metadata/依赖/状态、Locator/Decoration、通知和 damage。
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
`FigureTree::prepared_presentation` 同样保留最近成功的原始诊断视图；
它不证明当前投影有效。Router 的 authority 由受检 `prepared_scene` 查询判断。

## 外部消费者和验收

外部非分层算法、外部 prepared Figure 与默认算法共用所有新增入口。
消费者只需实现公开 trait、使用 typed registry 和 immutable presentation 数据，
不接触 Core 私有字段，也不增加内置能力分支。

| 反例 | 必须验证的结果 |
|---|---|
| 首帧从旧 bounds 到候选 bounds | Anchor、完整 points、Locator 与 glyph 使用同一候选，无第二帧修正 |
| 路由提交引起 Locator/后代更新，图结果未变 | 重复 preparation 复用 identity；本次合法路由后不因回调重跑而再次 reroute，无虚假 RouteChanged |
| 同 source、约束与领域结果重复准备 | attempt 可增加，identity 与路由 generation 不变；成功稳定帧不重算 |
| bounds 相同，请求或路径改变 | PreparedScene 依赖失效，重新准备整组；不继续返回旧 resolved route |
| 候选 payload identity 或 attachment 不匹配 | 整批拒绝，不晋升 epoch，不 flush 候选通知 |
| EAST 改宽，外部 prepared Figure 改高度 | 显式约束重测量并首帧收敛；重复稳定帧不重算 |
| 同一 Figure 同时具有 override、LayoutManager、preparation | preferred/minimum 各按优先级选中来源；高优先级成功时不调用低优先级不支持分支 |
| 非对称 Border/insets 加容器投影 | 各选中分支约束反投影、单次展开与正投影正确；override 不重复加 insets，minimum 回退受检 |
| 不支持测量、非法尺寸、非法 baseline、有限输入派生溢出 | 分别断言错误 kind、失败 FigureId、outer/content constraints；包装链可取原始原因，无字符串降级 |
| 直接 P→route→P 或 P→Locator→route→P | 在准入/实际查询阶段结构化拒绝；无候选发布，未靠 feedback 上限终止 |
| 跨 preparation 的 P→Q→route→P | 同一环检测发现有序路径；缓存命中和上一帧数据不掩盖传递依赖 |
| 仅声明必需组，计算不读 route 输出 | 发布屏障不产生计算边，无伪环；整组可正常完成 |
| 环失败后修正读取/binding，或删除/替换 preparation owner | 源变化保留，旧依赖退休，重建合法依赖后恢复 valid/resolved |
| 测量振荡、非法值或共享预算耗尽 | 结构化失败；失败值不入缓存，修正后恢复 |
| 同源路由失败 | 恢复仍匹配 source 的旧有效结果；普通组失败语义不变 |
| 改源后路由失败、删除端点或退休 binding | 源变化保留，旧结果 invalid，受影响组 unresolved，无新 partial submission |
| contents 成功替换后旧服务再查询 | 旧身份拒绝，无旧树或旧依赖复活 |

通过此增量评审后，按候选查询与必需组发布→测量/source 前缀→graph adapter
的顺序实施。随后接入两个官方场景、真实 fixture/方向切换与 Native 截图，
补齐书籍和语义账本，完成最终门禁后关闭 P2-L01。

## 增量评审意见处理记录

依据[2026-10-22 增量评审](../../verification/reviews/graph-projection-preparation-review-2026-10-22.md)
完成修订 2。全部采纳，无未采纳项。原报告及其基线清单保留；
本表只记录契约修订，不表示复审通过、实现完成或运行测试通过。

| 意见 | 处理结果与新章节 | 理由 |
|---|---|---|
| GP01，P1 | 已采纳：[快照未改变时保留 identity](#快照未改变时保留-identity) | attempt 与快照替换分离；相同完整前缀及领域结果复用 identity，新请求/路径更新身份；新增后代更新不重复路由验收 |
| GP02，P1 | 已采纳：[preparation 声明必需路由](#preparation-声明必需路由)、[快照计算依赖参与结构环检测](#快照计算依赖参与结构环检测) | 发布屏障与计算读取分开；PreparedScene subject 展开传递依赖，直接环/跨 preparation 环提前结构化拒绝并覆盖恢复 |
| GP03，P2 | 已采纳：[候选测量使用显式约束](#候选测量使用显式约束) | preferred/minimum 独立保留 override→LayoutManager→prepared→intrinsic 优先级，仅替换选中 prepared 分支；明确 Border/insets 单次组合 |
| GP04，P2 | 已采纳：[测量错误保留主体和原始原因](#测量错误保留主体和原始原因) | 新测量接口使用结构化包装，保留主体、约束、分类和 source；TextError 边界增加 boxed 变体，禁止 InvalidPreparedScene 字符串降级 |
