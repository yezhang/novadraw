# ADR-038：查询身份、合法缺失与失败传播

类型：`architecture-decision`

## 状态

已接受（2026-10-16），采用修订 1；P2-S07 已实现并完成行为验收。
完整证据见[完成核验](../verification/reviews/p2-s07-query-identity-completion-2026-10-16.md)。

[修订 1 复核](../verification/reviews/adr-038-revision-1-review-2026-10-16.md)
通过后，按用户“确认无误后推进到实现阶段”的授权接受本决策。

本次按[首轮评审](../verification/reviews/adr-038-draft-review-2026-10-15.md)
S07-DR01–DR04 修订，处理记录见本文“修订记录”。

本草案先从需求、ADR-018/036 和 Draw2D 基础协议确定目标契约，再对当前实现做
定向反例核对。反例盘点不能反过来决定目标语义。

## 背景与范围

FigureId 是带所有者与代次的引用。对象不存在、对象合法但没有 parent、对象的布尔
状态为 false，是三个不同事实。把它们合并为 None、false 或零几何，会让外部布局器
保留的旧 ID 被解释为正常输入，并生成看似有效的布局候选。

本批次覆盖 FigureTree 的任意 ID 结构、几何、状态和测量查询，以及
LayoutSceneQuery 的查询和失败传播。Event、Route、Stable 与 typed capability 查询
核对相同身份规则，保留各阶段权限和领域结果。

ADR-036 的 Event Query/Effects、LayoutSceneQuery/RouteSceneQuery 命名迁移已完成；
本批次不重复迁移。服务注册、Animation fault、Inspector poison 和通用扩展错误
source 整改属于 S09；detached 输入不变量属于 S08。

## 决策

### 1. 先验证身份，再解释值

接收任意 FigureId 的公开查询必须先校验 namespace 和 arena membership：

- foreign：身份属于其他 owner；
- unknown/stale：同 namespace 下没有对应 live slot，包括从未有效的 slot 和删除后
  已失效的代次；
- valid：身份在当前查询的 owner/revision 下有效。

foreign 必须与本 owner 的缺失区分。没有历史保留证据时，不要求把 unknown 和 stale
拆成两个可判定变体；可以用同一结构化 missing-identity 错误，但须保留输入 ID，并
明确覆盖 stale。不得为细分错误新增无限增长的墓碑表。

成员判断 `contains(id)` 是例外：它回答“是否属于当前 live 集合”，false 就是答案；
不得用它替代需要读取对象事实的受检查询。

### 2. 合法缺失与领域失败

| 情形 | 返回语义 |
|---|---|
| 有效 root 没有 parent | 成功，合法 None |
| 有效叶子没有 children | 成功，空集合 |
| 有效 Figure 未声明某 capability/constraint | 成功，合法 None；错型按既有领域错误 |
| 有效 Figure 不可见/未启用 | 成功，false |
| 有效 Figure 测量为零 | 成功，零尺寸 |
| 有效 Figure 没有 maximum 上限 | 成功，既有无上限表达 |
| foreign 或 unknown/stale | 结构化身份错误 |
| 有效 Figure 的逆变换不存在 | 成功的 None，明确表示不可逆 |
| 有效身份之间没有领域允许的转换路径 | 按该坐标入口的无路径契约表达，不解释为身份缺失 |
| 测量或派生几何计算失败 | 对应领域错误，不生成默认成功值 |

不强制所有返回值都是 `Result<Option<T>, E>`。必有事实使用 Result<T, E>，
有合法缺失的事实使用 Result<Option<T>, E>；已验证的不可变值使用直接 getter。

### 3. 错误归属与查询权威

优先复用 tree 领域的身份错误，领域错误通过结构化转换或 source 保留身份信息。
不建立全阶段巨型 QueryError；相同变体名不是统一类型的充分理由。

FigureTree 不拥有 Runtime fault 状态，不增加虚构的 Faulted 分支。Runtime 或阶段
owner 才判断是否允许进入相应查询阶段：

- committed tree 的纯事实可供 fault 诊断读取，不触发稳定化；
- 需要健康 owner 的计算阶段在构造/调用边界拒绝 fault；
- `Observations::stable_query` 保留 Faulted/NotStable 准入拒绝；此前保存的 owned
  stable 值仍属于原 epoch，不代表 fault 后新 publication；
- callback panic 遵循现有 fault boundary，不把它降级为 None 或零尺寸。

结构查询不 flush effects、不驱动更新、不重入 Runtime。测量可能使用既有受控缓存与
测量服务，但不得提交 scene mutation。

### 4. 任意 ID 入口与已验证视图

本批次优先直接把现有任意 ID 入口改为受检返回，不为了消除 Result 新建整套 typed
view。已有短借用或 owned immutable snapshot 的直接 getter 可以保留，但必须证明：

1. 构造阶段验证身份；
2. 短借用有效期间不能改变被引用拓扑；
3. owned snapshot getter 读取保存的值，不把其中保存的 ID 当作当前 live 引用；
4. 把保存的 ID 重新交给任意 live query 时仍需重新验证。

布局器不能因 container 已验证，就把它保存的任意 sibling/旧 child ID 视为有效。
内置与外部布局器使用相同受检入口；不保留内置专用 unchecked 快路径。

多主体查询先验证每个被读取的身份，再检查关系。`child_z_index(parent, child)` 对两个
有效但无直接父子关系的身份返回 Ok(None)；`constraint_as(container, child)` 需要
直接父子关系，关系不符返回既有 InvalidChild，不把它当作“没有 constraint”。
is_attached、有效可见性和 focus predicates 必须先验证目标，不能靠短路返回 false
隐藏无效身份。集合排除策略中只用于相等比较的 ID 不属于对象事实查询。

### 5. 坐标查询

ADR-018 的五个变换入口保留“返回变换，由调用方应用”的方向：

- 必有正向变换使用 `Result<Affine2D, E>`；
- 可能不可逆的入口使用 `Result<Option<Affine2D>, E>`；
- 身份校验必须在求逆之前完成，foreign/stale 不能成为 None；
- 有效但不可逆返回 Ok(None)，不自动判为 Runtime fault；
- 有效身份的计算溢出等几何失败进入领域错误，不返回恒等矩阵或 ZERO。

`Ok(None)` 仅表示有限输入在数学上奇异。判定基于输入 f64 系数所表示的精确值，
不使用固定 epsilon 将合法小尺度归为奇异。必须先检查完整仿射输入有限性；
非有限输入、数学逆存在但结果超出 f64 表示范围，以及数值计算无法可靠完成或
判定的情况，均返回几何错误。逆矩阵的线性与平移系数都须通过有限性校验。

直接计算行列式得到 0 或无穷，可能只是中间乘法 underflow/overflow，不能据此
判为数学奇异或断言逆结果不可表示。实现须使用缩放、可靠的退化判定或等价方法，
尽量避免中间范围限制；无法可靠判定时返回数值计算失败，不能返回 None。
有限可逆且逆结果可表示的范围用例必须成功，不能统一以数值失败拒绝。
例如 diag(1e-320, 1) 的逆存在但超出 f64 范围，属于结果不可表示；
该例是数值分类要求，不代表 Runtime 已接受该输入。

求逆所需的最小内部调整属于本批范围。任意 ID 坐标查询不能简单包装现有
`Affine2D::inverse` 的 Option 并推断原因；须使用能区分上述结果的内部计算通道。
该调整不预定公开 geometry API 重构，也不修改受保护的递归渲染主循环。

具体错误组合复用 tree 身份与几何领域错误，不要求所有入口共享一个万能枚举。
本节获批后替换 ADR-018 第 2 节与“错误处理”中的坐标失败折叠规则，并按实施清单
同步现行规范；Runtime 驱动、MeasureConstraints 和 preferred/minimum 回退契约继续有效。

### 6. 布局测量与候选传播

LayoutSceneQuery 的 bounds、children、preferred/minimum/maximum 等任意 ID 查询
不得对无效身份返回 ZERO、INFINITY、空集合或 default。

权威测量保留 override、LayoutManager、Figure intrinsic、Border/insets 和 container
projection 的既有组合顺序。minimum 的 preferred 回退必须保留，合法 unbounded、
零尺寸与 baseline 不得被误判为错误。

凡测量 callback 可能因查询或领域计算失败，都必须有显式 Result 通道；错误转换保留
失败 FigureId 和领域分类。不得在 FigureTree 或 LayoutSceneQuery 用 `.ok()`、
`unwrap_or_default()` 把失败降为正常测量。

Result 的 Ok 值仍须由权威测量 owner 校验。FigureTree 的测量组合入口负责统一
准入，LayoutSceneQuery 使用同一通道；内置与外部 callback 没有不同校验路径。
复用 FigurePresentation 的指标合同：

- preferred/minimum 的 width、height 必须有限且非负；
- baseline 若存在，必须有限，且满足 `0 <= baseline <= height`；
- maximum 每轴允许非负有限值或表示无上限的正无穷，拒绝 NaN、负数和负无穷。
  maximum 的正无穷须按无上限语义参与 projection，不能因通用 finite 检查被拒绝。

owner 校验 override、LayoutManager、intrinsic 和 prepared 等实际选中分支的结果，
再执行 Border/insets 与 container projection，并校验最终尺寸及 baseline。
minimum 回退 preferred 时仍走同一校验通道。每个参与组合的输入和中间结果按其
阶段指标合同受检；合法输入产生的派生溢出也必须返回错误。有限 preferred/minimum
投影产生的无穷属于失败，不能套用 maximum 的无上限规则。

非法成功值和派生溢出归 LayoutError 的几何失败，保留测量主体与原因；不夹取为零、
截断 baseline 或改为 default。校验属于 S07 的派生结果边界，不要求提前建设 S08
detached 构造 wrapper。直接测量查询也必须完成校验，不能依赖后续 LayoutOutput
placement 准入兜底。

候选处理顺序为：

```text
验证查询身份 → 计算测量/候选 → callback 返回
→ 校验完整 LayoutOutput → 原子提交派生结果
```

callback 返回错误时，丢弃本轮未发布 LayoutOutput，不发布该候选的 bounds、
候选派生 notification、damage 或 stable revision。已有 LayoutEvent::Started 等
执行诊断可保留，不把它们当作成功提交证据。此前已成功提交的 source mutation 和其他
派生前缀不承诺回滚；不得把本轮 layout failure 宣称为整个 Runtime 恢复到旧状态。

普通 LayoutError 经现有 Validation/FramePreparation 错误链返回，不晋升新 stable
epoch，也不自动改为 Runtime fault。callback panic 继续走原 fault boundary。

若扩展明确处理某个查询错误并仍返回有效候选，可按既有 output admission 提交；
Core 不暗中为扩展吞掉失败。内置布局器默认向上传播。

测量缓存仅在该次测量及最终 projection 校验通过后填入，随后才返回成功。
callback 的非法 Ok 值、projection 失败及错误本身均不入成功缓存；其他已经成功的
子测量缓存不要求回滚。失败后通过合法更新修正输入，按正常失效机制重试，必须能够
得到新成功值，不能命中先前失败生成的缓存。

本节获批后同步 ADR-018 第 3 节的 LayoutManager preferred/minimum 签名；
其结构化 constraints、baseline、minimum 回退和无 sentinel 规则继续有效。

### 7. 返回签名与错误组合

以下是已接受的 API 选择，不代表迁移已完成：

| 入口 | 建议返回值 |
|---|---|
| Tree node、bounds、depth、component revision、style、布尔事实与 children | Result<T, TreeQueryError> |
| Tree parent、child_z_index、可选内容/capability | Result<Option<T>, 对应身份/领域错误> |
| Tree 三个正向 transform | Result<Affine2D, CoordinateQueryError> |
| Tree 两个逆向 transform | Result<Option<Affine2D>, CoordinateQueryError> |
| Tree preferred/minimum/maximum 与 LayoutSceneQuery 测量 | Result<T, LayoutError> |
| LayoutSceneQuery children/container_bounds | Result<T, LayoutError> |
| LayoutSceneQuery constraint/freeform_extent/content_scale | Result<Option<T>, LayoutError> |
| LayoutManager preferred_measurement/minimum_size | Result<FigureMeasurement/Dimension, LayoutError> |
| LayoutManager layout | 保留 Result<(), LayoutError> |
| StableSceneQuery bounds/is_attached | Result<Rectangle/bool, TreeQueryError> |
| EventSceneQuery | 保留 EventSceneQueryError 及 detached 准入 |
| RouteSceneRead / RouteSceneQuery 任意 Figure 主体的事实读取 | Result<T 或 Option<T>, RouteSceneQueryError>；存在性指纹另按下述协议 |

TreeQueryError 保留 UnknownFigure(FigureId)，补充 ForeignFigure(FigureId)。
UnknownFigure 明确包含 retired generation；不引入 Detached/Faulted 的全阶段集合。
CoordinateQueryError 组合 Identity(TreeQueryError) 与几何失败，几何失败保留 FigureId，
并结构化区分非有限输入、逆结果不可表示和数值计算失败；不得仅以
NonFiniteGeometry 或字符串把这三类原因合并。

LayoutError 以 Identity(TreeQueryError) 替换其旧 UnknownFigure 分支，保留 constraint、
InvalidChild 和 NonFiniteGeometry 分类。测量几何失败须补充非法尺寸、baseline 和
派生溢出的结构化原因，不能因负尺寸仍有限而漏判。Event、Capability 可继续保留已有结构化
identity 变体；Route 补充 foreign 区分，并先验证身份，再判断 detached/anchor/transform。

Layout 的可选 freeform 查询仅把“有效对象不是 freeform”映射为 Ok(None)；
Unvalidated 仍是领域错误，建议由 LayoutError 保留对应 FreeformError 原因。
content_scale 仅把合法 capability 缺失映射为 None，descriptor 错误保留原分类。
这些原因按真实调用需要组合，不为追求 Copy/Clone 把结构化失败转为字符串。
Tree 的专用 freeform 查询保留 NotFreeform/Unvalidated 领域合同。

Route 的 `dependency_generation` 是依赖存在性/代次指纹，和 contains 一样允许观察
主体缺失，不属于读取对象事实的受检入口。指纹须能反映原依赖的存在性变化，以供
失效检查；取得指纹不证明 bounds、visibility 或 anchor 读取成功。
tracked query 可在事实读取前登记存在性或失败观测，事实查询仍先验证身份。
不能把缺失指纹机械升级为稳定化失败，也不能将失败观测标记成成功事实读取。

外部 RouteSceneRead 与内置 adapter 共用该协议。有效但缺失 anchor、不可逆映射及
unresolved 恢复继续按 ADR-005/037 分类，保留恢复所需的依赖和失败观测。
删除已绑定主体后，旧 stale ID 永远无效；恢复须更新为有效新绑定，不能假设创建
新 Figure 会使旧 ID 复活。

### 8. 非权威 getter 与直接 breaking

只读 NodeState 的局部值可直接读取，但名字必须表达 override/local value。
不允许用 preferred/minimum/maximum measurement 同名 getter 返回弱化的局部值。
已经删除的旧 getter 只保留防回归证据，不重新创造整改任务。

版本仍为 0.1.0。获批后直接迁移 workspace 调用点、外部消费者与 API snapshot；
不保留吞错 alias、deprecated 双入口或 root 重导出。

## 取舍与稳定性

直接受检返回会增加调用点的错误处理，并改变公开 measurement trait 签名；
优点是错误在原调用链显式传播，避免维护额外的 typed view 和隐藏失败状态。
已验证视图的直接 getter 继续保留，不重复校验冻结值。

把所有错误合成 QueryError 虽然减少类型名，却会把 Runtime fault、tree identity、
constraint 和 coordinate map 绑定成一个长期公共枚举。草案只共享真实公共的身份
合同，领域失败留在对应入口；未来新增领域错误无需扩大其他阶段的权限。

本批次不做性能优化，也不宣称 Result 带来性能收益。实现保留递归主线、现有缓存和
测量组合，受保护渲染主循环不在整改范围。

## Draw2D 对应与差异

参考范围仅为 `org.eclipse.draw2d` 与 `org.eclipse.gef`，不使用 Zest。
`Figure.java` 的 getParent（882）、isVisible（1226）和 isEnabled（1139）直接读取
有效 Java 对象的事实；isShowing（1195）递归组合 parent 可见性。
getMinimumSize（865）在无有效 minimum/layout 结果时回退 preferred，
getPreferredSize（898）委托 LayoutManager，getMaximumSize（846）允许无上限。

Java 的对象引用不具有 Rust owner namespace/arena stale lookup 问题；身份错误是
Novadraw 所有权迁移的显式边界，不伪称为 Draw2D 原生异常合同。

`api_semantics`：`figure.tree`、`figure.geometry.bounds`、`figure.box.client_area`、
`figure.visibility.enabled`、`coordinate.conversion`、`layout.manager`、
`validation.protocol`。账本保留合法输入 verified 基线，并已补充 S07 负测与恢复证据。

## 外部消费者证据

以下要求已由定向测试和外部消费者执行验证，完整日志与最终门禁见
[实施核验](../verification/reviews/p2-s07-query-identity-completion-2026-10-16.md)：

1. 第三方 LayoutManager 通过公开领域导入实现相同 measurement/layout 协议；
2. 保留 foreign/stale child ID，查询得到身份错误，经 Result 到达 owner；
3. 先写一个合法 output，再查询失效 ID 并返回错误，证明该候选未部分发布；
4. 合法 root、叶子、缺失 constraint/capability、零尺寸、无上限与奇异逆变换均可处理；
5. 外部与内置布局器共用错误与 admission，无 Core 分支、原始 tree 引用或 unchecked API。
6. 外部 measurement callback 返回非法 Ok 值时，owner 拒绝成功与缓存；合法更新后重试成功；
7. 外部 RouteSceneRead 的存在性指纹检测已删除依赖，事实查询拒绝旧 ID；
   unresolved 后换成有效新绑定可恢复。

## 验证与实施

逐项验收见[验证矩阵](../verification/plans/p2-s07-query-identity-and-failure-semantics.md)，
当前反例和旧发现处置见[设计盘点](../verification/reviews/p2-s07-query-design-inventory-2026-10-15.md)。

按 Layout 测量/失败传播、tree 查询、跨 crate 消费者三个切片推进。当前查询负测
直接运行 Cargo target，内部测试使用 `--lib s07_`；批次与最终门禁使用
`tests/README.md` 的 Cargo fmt/check、Clippy/test，文档另运行 xtask docs。
设计阶段不注册空 suite。

### 获批后的规范与调用点同步清单

草案阶段保留了 accepted SSOT 的原合同。接受 ADR-038 后，按以下清单迁移并核对
引用；文档不得同时把旧 Option 折叠和新失败分类称为当前规范：

| 入口 | 必须同步的内容 |
|---|---|
| [ADR-018](adr-018-runtime-driving-and-measurement-api.md) 第 2/3 节与错误处理 | 五个坐标入口的返回值、求逆分类、LayoutManager 测量签名与成功指标校验 |
| [公开 API 契约](../design/architecture/public-api-contract.md) | 任意 ID 受检返回、合法缺失、测量权威与局部 getter；移除“坐标 Option 保留旧合同”的冲突 |
| [坐标规范](../design/coordinates/coordinate-system.md) | 身份检查顺序、正逆向签名、数学奇异与数值失败边界；保持 M4 坐标域与组合方向 |
| [Tree 查询与 focus 规范](../design/architecture/tree-search-and-focus.md) | 任意 ID 查询、关系与短路校验、已验证视图的有效期；按实际入口更新签名 |
| [ADR-036](adr-036-context-phase-authority-projection.md) 及阶段规范 | 核对 Layout/Event/Route 错误包装和 owner 准入；保留 committed prefix、存在性观测与恢复路径 |
| [Draw2D API 语义账本](../parity/draw2d/api-coverage.md) | 更新本文受影响 family 的签名与证据引用；合法输入 verified 基线保留，新增负测通过前不宣称 S07 已验证 |
| API snapshot、独立 consumer 与 workspace 调用点 | 同批更新规范领域导入、trait 签名和错误匹配；包含 Core、Editor、平台、示例与测试 |
| 本矩阵与实施/完成记录 | 登记求逆、非法测量、缓存重试及 Route 新绑定恢复的执行证据，映射来源审计 ID |

### 修订记录

2026-10-16，修订 1：

| 评审 ID | 草案处理 | 验证要求 |
|---|---|---|
| S07-DR01 | 第 5/7 节限定 None 为数学奇异，补充三类数值失败与最小内部调整 | S07-G04–G07 |
| S07-DR02 | 第 6 节冻结指标、owner 校验顺序、全部选中分支与缓存准入 | S07-L03、L09–L12 |
| S07-DR03 | 第 7 节区分存在性指纹与事实读取，保留依赖及有效新绑定恢复 | S07-P03–P05 |
| S07-DR04 | 冻结获批后的 SSOT、API 与消费者同步清单 | 实施同步检查 |

修订要求已写入决策与矩阵，2026-10-16 复核通过；行为用例和消费者验收已执行。

## 关系与日期

延续 ADR-018 的测量值合同与 Runtime 驱动；延续 ADR-036 的阶段权限；
延续 ADR-011 的派生候选提交；不扩张 ADR-037 的失败回滚保证。

2026-10-15 起草；2026-10-16 修订 1、复核通过并接受。
