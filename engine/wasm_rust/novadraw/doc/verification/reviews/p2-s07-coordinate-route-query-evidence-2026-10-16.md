# P2-S07 坐标与 Route 查询实施证据

类型：`implementation-verification`

状态：坐标与 Route 身份切片已实施。本记录保留切片执行时的进度；后续验收
已完成，P2-S07 当前状态见[完成核验](p2-s07-query-identity-completion-2026-10-16.md)。
规范：[ADR-038](../../adr/adr-038-query-identity-and-failure-semantics.md)；
验收范围：[验证矩阵](../plans/p2-s07-query-identity-and-failure-semantics.md)。

## 已实施合同

FigureTree 三个正向坐标入口返回 Result，两个逆向入口返回 Result<Option>。
CoordinateQueryError 保留 Tree 身份错误；几何失败区分非有限输入、逆结果不可表示、
数值计算失败与正向组合溢出，携带查询 FigureId。

内部求逆用 f64 的精确 dyadic 系数计算乘积，接近抵消的差值使用 u128 对齐，
较大指数差使用缩放表示。精确奇异不依赖 epsilon，也不依赖直接行列式乘法是否
溢出或下溢。逆平移先计算分子再除以行列式，避免单项溢出掩盖合法抵消。
公开 Affine2D::inverse 不在本批重构范围。

RouteSceneRead 的 attached、parent、visibility 和 viewport 事实使用 Result；
内置 adapter 与外部消费者采用相同协议。tracked query 可以先登记失败主体的
存在性观测，随后事实查询返回错误。adapter 为每个依赖 Figure 显式 hash live
membership；即使已隐藏、零 bounds 或 root-like 的主体被删除，指纹也反映存在性变化。
非 border-box anchor 也先校验身份，拓扑预检保留错误来源及失败观测。

Route 坐标转换验证双方身份后计算，受检求逆保留几何分类；
normal 映射取消固定 epsilon，矩形投影补充最终 extent 溢出检查。
Event 内部输入分发和命中查询仍按其既有“无法命中/未处理”结果拒绝不可映射分支，
不生成伪造坐标。权威坐标查询本身保留错误。

## 执行结果

`core.p2-s07-coordinate-route-query` 通过，共 31 项：

| 范围 | 实际证据 |
|---|---|
| 外部坐标消费者，5 项 | foreign/retired 五入口；合法奇异；极端可表示逆；逆线性/平移不可表示；非有限输入；正向父链溢出 |
| 内部求逆，3 项 | diag(1e±200, 1e±200) 与混合尺度；diag(1e-320, 1)；精确奇异；乘积舍入相等但精确行列式非零；溢出平移分子的合法抵消 |
| 内部 Route adapter，2 项 | foreign/retired 在 detached 与 anchor 缺失前拒绝；隐藏 Figure 删除导致 geometry/visibility/topology/transform 指纹变化；失败观测保留 |
| M4 回归，5 项 | 多层 Insets、Point/Rectangle 往返、输入点降域、原子 bounds 与 damage |
| M9 回归，16 项 | Anchor/Router/Locator 合法行为与外部 RouteSceneRead 缺失事实、foreign、指纹变化及合法 false/None |

完整日志：[坐标与 Route suite](../../../verification/evidence/p2-s07-coordinate-route-suite.log)。
数值计算失败变体是内部防御分支；当前有限输入实现没有独立可触达 fixture，
不宣称执行了该分支的故障注入。范围用例与精确分类由其余分支测试覆盖。

## 规范同步与剩余工作

ADR-018、ADR-036、坐标规范、API symbol snapshot、workspace 调用点、parity
coordinate/anchor 条目和验证 manifest 已同步。
workspace quick 通过，包含 Native/Web 编译和公开 API 边界检查：
[执行日志](../../../verification/evidence/p2-s07-coordinate-quick.log)。

Tree 结构/状态、Stable/Event 任意 ID 查询与非权威 Node getter 仍需迁移。
Route 删除旧绑定后换成新有效绑定的完整 unresolved 恢复证据、prepared/Border
测量分支及失败候选 notification/damage/stable publication 断言仍待补齐。
本记录不关闭审计条目，S08/S09 未启动，递归渲染主循环未修改。

## 新绑定恢复与回归

当前入口为 `core.query-contracts`，
[执行记录](../../../verification/evidence/p2-s07-core-query-contracts.log)通过。
新增外部 `DependentAnchor` 经公开 tracked query 读取次级 Figure 依赖：
成功路由 → 删除原 Figure → 稳定 unresolved 并保留依赖 → 无变化时不重复产帧
→ 新 ID 与新 anchor binding 恢复 resolved；旧 ID 始终 UnknownFigure。

该测试修复了两处迁移问题。connection 自身的成功拓扑准入不加入几何依赖集合，
避免错误自依赖；失败准入仍保留身份观测。失效检查标记 dirty 时记录当前存在性指纹，
保留旧依赖主体，避免 callback 提前失败后未访问的旧指纹每轮重新失效。
这些观测不代表旧 Figure 事实查询成功。

连接 Runtime 的 29 项回归通过：
[执行记录](../../../verification/evidence/p2-s07-route-runtime-regression.log)。
本轮补齐 P05 的内置 adapter 与外部 anchor 路径，外部 RouteSceneRead 的存在性协议
继续由 `m9_connection_contract` 验证。
