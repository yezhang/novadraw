# P2-S07 查询失败语义验证矩阵

类型：`verification`

日期：2026-10-16，修订 1

状态：`complete`；ADR-038 修订 1 已实现，核心切片、workspace 消费者、发布隔离、
缓存与新绑定恢复通过。最终目标与门禁映射见
[完成核验](../reviews/p2-s07-query-identity-completion-2026-10-16.md)。

规范：[ADR-038](../../adr/adr-038-query-identity-and-failure-semantics.md)。
现状依据：[设计盘点](../reviews/p2-s07-query-design-inventory-2026-10-15.md)。

## 验证边界

本矩阵证明身份错误不会被解释为正常缺失或成功值，且 layout 查询失败到达原有
Query → Output 提交边界。它不要求新增 Plan、整场景快照、公共注入接口或万能错误枚举。
ADR-036 的命名、阶段权限和 Event 延迟写入属于已验证基线，只做回归。

## 用例

| ID | 输入或操作 | 预期结果与证据 |
|---|---|---|
| S07-ID01 | 将另一 FigureTree/Runtime 的有效 ID 传入各任意 ID 查询 | 结构化 foreign，保留输入 ID；与本 owner 缺失区分 |
| S07-ID02 | 保存已删除 ID，再创建新节点以覆盖 arena slot 重用情形 | 旧代次得到 UnknownFigure 或领域包装；新 ID 查询成功，不误解析为旧对象 |
| S07-ID03 | Core 私有单测构造同 namespace 下不在 live arena 的身份 | unknown 错误；不新增公开 ID 构造或错误注入入口 |
| S07-ID04 | 有效 root 的 parent、有效叶子的 children | Ok(None) 与成功空集合，均不同于 ID01–03 |
| S07-ID05 | child_z_index 输入有效非直接 child；constraint_as 输入关系不符；任一主体 foreign/stale | 前者 Ok(None)，后者 InvalidChild；身份失败优先，两个主体都受检 |
| S07-ID06 | 有效 Figure 本地/继承可见性、enabled、opaque、focus/attachment predicates；对比无效 ID | 正常 false 可返回；无效 ID 不能因短路而变成 false |
| S07-ID07 | 已验证短借用 getter、保存的 owned snapshot 与其旧 ID 再查询 | getter 读冻结值；重新进入 live query 必须受检，不能以旧 snapshot 证明当前有效 |
| S07-G01 | 五个 Tree transform 查询，foreign/stale 与多层合法 parent chain | 身份失败为错误；正向变换成功并保持 M4 坐标合同 |
| S07-G02 | 有效身份上的奇异 child transform，正向与逆向分别查询 | 正向允许有效奇异矩阵，逆向 Ok(None)；不把 None 当作 invalid ID 或 Runtime fault |
| S07-G03 | 提供可能导致组合变换溢出的有限输入；与合法零坐标/零尺寸对比 | admission 若已拒绝，记录真实拒绝路径；获准查询若计算失败，返回几何错误；合法零值成功，不返回恒等矩阵或 ZERO 替身 |
| S07-G04 | 内部求逆测试分别输入非有限系数、有限精确奇异矩阵和普通有限可逆矩阵 | 非有限输入为结构化几何错误；奇异为 None；可逆返回有限正确结果；不以 epsilon 分类 |
| S07-G05 | 内部求逆测试使用 diag(1e-320, 1)，另构造逆平移系数溢出的有限输入 | 数学逆存在但线性或平移结果不可表示，返回对应原因，不能为 None；不假称该 fixture 已通过 Runtime admission |
| S07-G06 | 有限可逆矩阵的直接行列式乘法 underflow/overflow，但完整逆可表示；另设精确奇异对照 | 可逆用例成功，奇异对照为 None；中间 0/无穷不能充当奇异判据；求逆单测与公开坐标受检入口证据分别登记 |
| S07-G07 | 内部数值计算确实无法可靠完成或判定的错误路径；任意 ID 查询包装该结果 | 保留数值计算失败及 FigureId，区别于非有限输入和结果不可表示，不折叠为 None；不得为该用例新增公开注入 API |
| S07-L01 | LayoutSceneQuery 的每个任意 ID 方法分别输入 foreign/stale/unknown | children、bounds、constraint、measure、freeform/scale 均返回身份错误 |
| S07-L02 | 有效空容器、零测量、无 maximum override、UNBOUNDED constraints | 空集合、零尺寸与无上限按原合同成功，不能为清除 fallback 破坏合法结果 |
| S07-L03 | preferred/minimum override、LayoutManager、intrinsic、prepared、Border/insets、baseline、container projection 各选中分支 | 各分支与最终投影均受检，保持权威组合与 minimum 回退；有限非负尺寸、baseline 区间和合法零值成功；失败不入成功缓存 |
| S07-L04 | 外部 LayoutManager 的 preferred/minimum 查询保留旧 child ID，并向上传播错误 | 结构化失败经测量到 tree/validation owner；不经 default/.ok() 降为成功 |
| S07-L05 | 外部 layout 先追加合法 child bounds，再读无效 ID 并返回错误 | 整个未发布 LayoutOutput 丢弃；child bounds、候选派生通知、damage 与新 stable publication 均未发布 |
| S07-L06 | layout output 自身含有效与无效 child 混合操作，callback 返回 Ok | 原有整批准入拒绝，无部分候选写入；保留既有 LayoutError 分类 |
| S07-L07 | 有效对象缺 constraint/scale/freeform；freeform 未验证；错型 descriptor | 合法缺失为 None；Unvalidated/错型保留领域错误，不能统一 `.ok()` |
| S07-L08 | 内置与外部 LayoutManager 的相同输入和失败路径 | 使用相同 API、错误、admission；无内置专属 unchecked hook 或 fast path |
| S07-L09 | 外部 preferred/minimum callback 分别返回 Ok(NaN)、负尺寸、无穷及非有限/负/大于 height 的 baseline | 直接 tree/query 测量即返回 LayoutError 几何失败并保留主体与原因；不等待 LayoutOutput，不夹取或填入成功缓存 |
| S07-L10 | maximum 各轴分别为有限非负值、正无穷、NaN、负数、负无穷；无上限再经 Border/container projection | 前两类按有限值/无上限成功，后三类拒绝；maximum 合法正无穷不套用 preferred/minimum 有限规则 |
| S07-L11 | 合法有限输入经 Border/insets 或 container projection 产生尺寸溢出或非法 baseline；独立覆盖各阶段 | 组合阶段及最终结果准入返回几何失败，直接 preferred/minimum 不返回成功，未校验结果不入缓存 |
| S07-L12 | 非法 callback Ok 值或 projection 失败后，合法更新修正原因并重试；比较同一 key 的测量和缓存 | 失败未生成成功缓存，正常失效后重试得到正确值；其他已成功子测量可保留；经 validation/frame preparation 的普通错误不晋升 stable epoch |
| S07-A01 | 调用 Node 三个旧非权威尺寸 getter；用 tree/query 测量替代 | 旧 getter 从公开表面退出；权威测量含 layout/intrinsic/Border/projection |
| S07-P01 | Event query foreign/stale/detached/root 与 callback 内新 effects | 保持现有结构化错误和 committed prefix；不重新实施 ADR-036 |
| S07-P02 | Stable query 准入：fault、pending work、合法 stable；随后查询 foreign/stale | owner 准入仍为 Faulted/NotStable；获准 query 中的无效身份为 tree 错误，epoch 不改变 |
| S07-P03 | 内置与外部 RouteSceneRead 的 foreign/stale、有效 detached、缺 anchor、奇异 map；分别观察指纹与对象事实 | 指纹允许观察缺失，事实读取先判身份再判领域状态；存在性/失败观测不代表事实读取成功，不能将缺失指纹机械升级为稳定化失败 |
| S07-P04 | 合法 route 后失效，再恢复有效 geometry | 保留 unresolved 与依赖恢复合同，不把领域失败升级为 Viewer fault；回归 S06 分类 |
| S07-P05 | 内置与外部 adapter 先记录合法绑定依赖，删除主体，运行失效检查并稳定化；随后换成有效新 ID 绑定 | 原指纹变化触发失效，按既有分类成为稳定 unresolved，保留恢复所需观测；新绑定恢复成功，旧 stale ID 始终拒绝，不能以 slot 重用宣称复活 |
| S07-F01 | measurement/layout 扩展 panic 与 fault 后只读诊断 | 使用原 Runtime fault boundary；不生成默认测量，不新增 standalone tree Faulted |

## 外部消费者与失败前缀

使用现有独立 consumer crate 的 LayoutManager/Router 路径，必要时增加专门测试 target。
consumer 仅从规范领域路径导入。第三方 layout 的最小失败序列为：

```text
保存一个 child ID → owner 删除该 child → 执行下一次 layout
→ 追加另一个有效 child 的候选 bounds
→ 查询旧 child 的测量并用 ? 返回错误
→ owner 丢弃未发布候选
```

对比保存的 bounds、候选派生通知/damage 与 stable epoch，允许既有 Started 或失败诊断。
source mutation 和先前成功的派生前缀不要求回滚。

身份与测量错误测试分开执行。foreign、same-namespace missing 和 retired generation
各有独立 fixture；不能用一个 foreign ID 代替三类负测，也不能用文档或静态盘点替代
外部消费者执行结果。

求逆范围测试须记录精确输入、预期数学分类与实际结果，分别验证线性和逆平移系数。
如果公开 mutation admission 已拒绝某个数值 fixture，记录该拒绝，将算术分类放在
内部求逆测试中验证；不能为了进入查询而绕过 Runtime 的正常准入。
S07-G07 按实际内部实现登记可触达路径；没有独立可触达分支时须说明覆盖归属，
不得声称执行了不存在的故障注入。

外部 measurement consumer 独立覆盖 callback 返回错误与 callback 返回非法 Ok 值，
两者分别验证。override、intrinsic、prepared 及投影分支可使用对应 Core fixture，
仍须经过统一 owner 校验。缓存证据包括失败后的内容、正常更新的失效过程和重试结果。

## 实施同步检查

获批后逐项核对 ADR-038 的规范同步清单，记录 ADR-018、public API contract、
坐标规范、tree/focus 查询规范、ADR-036 阶段错误与恢复说明的实际更新或无需变更理由。
API snapshot、独立 consumer、workspace 调用点与 parity 受影响 family 同步迁移。
草案修订不提前改写 accepted SSOT，也不因签名迁移降级合法行为 verified 基线。

## Suite 与证据登记

当前直接运行 Cargo 测试，外部 target 包括 `p2_coordinate_query_failure`、
`p2_route_dependency_recovery`、`p2_query_failure_semantics`、`p2_tree_query_identity`、
`m4_coordinate_contract` 和 `m9_connection_contract`；内部四组查询测试使用 `--lib s07_`
过滤。全量 Core 测试覆盖这些入口。布局、坐标与 Route、Tree 与 Stable 的阶段执行
记录分别见：

- [布局切片记录](../reviews/p2-s07-layout-query-evidence-2026-10-16.md)；
- [坐标与 Route 切片记录](../reviews/p2-s07-coordinate-route-query-evidence-2026-10-16.md)；
- [Tree 与 Stable 切片记录](../reviews/p2-s07-tree-query-evidence-2026-10-16.md)。

历史切片编排已执行通过，日志见
[core.query-contracts](../evidence/p2-s07-core-query-contracts-final.log)。
该 suite 已随命令体系简化退出当前入口；表中同名日志只标识执行时的证据。

| 用例 | 实际 target / 断言 | 结果与证据 |
|---|---|---|
| ID01–03、ID06 | `p2_tree_query_identity` 与内部 `never_live_identity_is_rejected_by_every_fact_query`；foreign、retired 和同 owner never-live 独立构造 | 通过，core.query-contracts 日志 |
| ID04–05、L01–02 | `p2_tree_query_identity`、`p2_query_failure_semantics`、内部 layout query tests；关系双主体、合法 None/false/零值和所有任意 ID 方法 | 通过，core.query-contracts 日志 |
| ID07、P02 | `stable_query_revalidates_ids_and_owned_values_keep_their_original_epoch`；Stable 准入、旧 epoch 和旧 ID 再查询 | 通过，core.query-contracts 日志 |
| G01–03 | `p2_coordinate_query_failure` 与 `m4_coordinate_contract`；五入口、多层组合、合法奇异及有限输入组合溢出 | 通过，core.query-contracts 日志 |
| G04–06 | 内部 `geometry::checked_inverse::s07_checked_inverse_tests`，外部 container fixture；精确奇异、线性/平移范围、中间 underflow/overflow 与合法抵消 | 通过，core.query-contracts 日志 |
| G07 | `NumericalFailure` 为防御分支，有限输入算法没有独立可触达 fixture；错误包装静态核对 | 不宣称注入执行；分类覆盖归属见坐标切片记录 |
| L03、L09–12 | `p2_query_failure_semantics` 与内部 layout measurement tests；override/intrinsic/prepared/Border/container projection、非法成功值、失效与缓存重试 | 切片通过；扩充 minimum 各轴非法值、非有限 baseline 的最终结果登记在 workspace 测试日志 |
| L04–05、L08、L12 | 外部 `QueryFailure` 与内部 `failed_candidate_publishes_no_geometry_notifications_or_damage`；候选前缀被丢弃、通知/damage 不出版、epoch 不晋升 | 通过，core.query-contracts 日志 |
| L06 | `m5_layout_contract` 混合合法/非法 output 整批准入测试 | 既有原子性合同，最终 workspace 回归登记 |
| L07 | 内部 `layout_constraint_type_and_unvalidated_freeform_are_not_absence`；constraint 错型、foreign container、Freeform Unvalidated | 通过，core.query-contracts 日志 |
| L07 descriptor | `figure::capability::tests::malformed_descriptor_remains_a_type_error`；内部构造 erased 错型，公开 typed register 无法产生该状态 | [独立日志](../evidence/p2-s07-capability-descriptor.log)通过，LayoutError::Capability 保留原错误 |
| P01、F01 | 既有 Event committed-prefix 与 Runtime measurement/layout panic/fault 测试 | 保留 ADR-036 合同，最终 workspace 回归登记 |
| P03–05 | 内部 Route adapter、`m9_connection_contract`、`p2_route_dependency_recovery`；失效指纹、失败观测、新绑定和旧 ID 拒绝 | 通过；[外部 adapter 恢复](../evidence/p2-s07-external-route-recovery.log) 与 core.query-contracts 日志 |

maximum override 是既有外框上限合同，不新增 maximum 的 Border/container 投影
callback；带 container projection 的对象查询无 maximum override 时，仍返回成功无上限。
minimum callback 返回 `Dimension`，baseline 校验由保留 baseline 的
intrinsic/prepared/preferred 分支证明，不能伪造不存在的 minimum callback baseline。
不存在的数值注入和公开 typed descriptor 错型路径均不新增公共注入 API。
表中“最终 workspace 回归登记”均已通过，Core 50 个 integration target 无遗漏。
ShortestPath 的显式 stale/foreign obstacle 由 RouteError::Scene 保留身份错误，
新 Router 绑定移除旧 ID 后恢复；执行日志见完成核验的最终 Route 回归。

下列直接测试或跨工具 suite 用于受影响范围回归，不能替代新增负测：

| 入口 | 用途 |
|---|---|
| `cargo test -p novadraw` | Core 合法行为、Layout、Route、Figure capability 与 viewport |
| `cargo xtask verify extensions` | 独立 Figure/Layout/Router 消费者 |
| `cargo xtask verify runtime.apps` | Runtime validation、fault 与应用查询调用点 |
| `cargo xtask verify editor.contracts` | Viewer、Selection 与 Connection projection |
| `web.build` | 公共跨 crate 返回值变更的 Web 迁移 |

每个用例的实现记录必须填写 target、结果与证据路径。定向失败先修复，再执行更宽
门禁；suite 已包含的命令不重复运行。公开 API 批次完成后运行 workspace fmt/check，
最终交付运行 workspace fmt、Clippy 与 test；更新 API snapshot、文档与 parity 受影响
family。

## 关闭条件

- ADR-038 通过评审，实施范围与返回值选择明确；
- 本矩阵的新增正负用例和独立消费者实际执行通过；
- 无效身份不再得到 default/false/None 成功值，合法零值和缺失不退化；
- Query → Output 保证精确到本轮未发布候选，不夸大为整轮回滚；
- S07-DR01–DR04 复核通过，求逆分类、测量准入与缓存恢复、Route 存在性观测均有执行证据；
- 规范同步检查完成，旧返回合同不再与 accepted ADR-038 冲突；
- completion record 映射 R04、TC-09、API-F06、API-07 后再关闭审计 ID。
