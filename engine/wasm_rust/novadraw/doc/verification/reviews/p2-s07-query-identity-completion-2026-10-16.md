# P2-S07 查询身份与失败语义完成核验

类型：`implementation-verification`

日期：2026-10-16

状态：`complete`；核心实现、消费者迁移、行为验证与规范同步完成。

规范：[ADR-038](../../adr/adr-038-query-identity-and-failure-semantics.md)；
逐项验收：[验证矩阵](../plans/p2-s07-query-identity-and-failure-semantics.md)。

## 实施结果

任意 FigureId 事实查询先校验 owner namespace 和 live arena membership。foreign 与
本 owner 的 unknown/retired 分开，错误携带输入 ID。有效 root、叶子、false、零测量、
缺 capability/constraint 与不可逆变换继续使用成功值；contains 保留成员判断的 bool。
短借用 getter 读取已验证对象，owned snapshot 读取原 epoch 值，旧 ID 再查询仍受检。
Node 非权威 preferred/minimum/maximum getter 已退出公开 API。

三个正向坐标入口返回 Result，两个逆向入口返回 Result<Option>。内部求逆采用精确
dyadic 乘积与缩放，区分数学奇异、非有限输入和逆结果不可表示；线性、平移结果均受检。
极端可表示逆和溢出乘积的合法抵消通过。NumericalFailure 是防御分支，没有有限输入
下的独立可触达 fixture，不宣称做过故障注入。

布局器的 preferred/minimum 使用 Result。Tree owner 在选中分支、Border/insets、
container projection 与成功缓存写入前校验指标。maximum 保留外框上限和正无穷合同。
非法 callback Ok 值不写缓存，正常 revalidate 后可重试。callback 查询失败时丢弃
本轮未发布 LayoutOutput，合法 bounds/repaint 前缀及派生通知、damage 均未发布，
latest stable epoch 不晋升；Started 诊断与此前 source mutation 不承诺回滚。

Route 的存在性指纹与对象事实分开：删除依赖触发失效，事实查询拒绝旧 ID，失败观测
保留，Runtime 收敛为 recoverable unresolved。有效新 ID 与新 binding 恢复路由。
内置 adapter/外部 Anchor 的稳定化和外部 RouteSceneRead/DirectRouter 的重新计算
分别有执行证据。connection 自身的成功拓扑准入不成为几何自依赖；失效检查更新保留
主体的当前指纹，避免提前失败导致旧指纹每轮重复失效。
显式 obstacle 列表保留已删除 ID 时，查询错误经 RouteError::Scene 保留
UnknownFigure/ForeignFigure；不把身份错误折叠成空障碍列表或 InvalidObstacle。
合法 detached/hidden obstacle 仍可跳过，几何无效仍保留 InvalidObstacle；新 Router
绑定移除旧 ID 后恢复，普通身份错误不使 Runtime fault。

Core、Editor、Inspector、平台、Native/Web 示例、benchmark、integration tests
及独立 Editor consumer 调用点已迁移。受保护的递归渲染主循环未修改，S08/S09 未启动。

## 证据

| 范围 | 结果 / 日志 |
|---|---|
| Tree、坐标、测量、候选隔离、Runtime Route 恢复 | [core.query-contracts](../evidence/p2-s07-core-query-contracts-final.log)通过 |
| 外部 RouteSceneRead 新绑定恢复 | [独立测试](../evidence/p2-s07-external-route-recovery.log)通过 |
| erased descriptor 防御分类 | [内部测试](../evidence/p2-s07-capability-descriptor.log)通过 |
| Editor 完整 unit/integration/doctest | [测试记录](../evidence/p2-s07-editor-contracts-retry.log)通过；同日志 Native host 初次失败 |
| Native host 修复后编译 | [定向复跑](../evidence/p2-s07-native-host-check.log)通过 |
| Core/Editor 外部扩展 | [extensions](../evidence/p2-s07-extensions.log)的 Core 与 Editor 测试通过；独立协议初次编译失败 |
| 独立 Editor 协议修复后 | [定向复跑](../evidence/p2-s07-external-editor-protocol-retry.log)通过，4 项 |
| Update/Event/Viewport/UML/Animation 应用 | [runtime.apps](../evidence/p2-s07-runtime-apps-retry.log)通过 |
| API snapshot、依赖与第三方类型边界 | [架构检查](../evidence/p2-s07-api-snapshot.log)通过，Native/Web rustdoc 已生成 |
| workspace Clippy | [最终日志](../evidence/p2-s07-workspace-clippy-final.log)通过 |
| Core 全部目标 | [workspace 执行前缀](../evidence/p2-s07-workspace-tests-closure.log)、[Animation 定向修复](../evidence/p2-s07-core-remaining-tests.log)、[剩余目标复跑](../evidence/p2-s07-core-remaining-tests-retry.log)、[doctest](../evidence/p2-s07-core-doctests.log)合并覆盖 788 项通过，包括 374 项 lib、50 个 integration target 和 28 项 doctest |
| 其余 workspace consumer | [完整测试](../evidence/p2-s07-workspace-consumer-tests.log)通过，245 项；连同 Core 共 1,033 项，无遗漏 integration target |
| 最后 Route 整改 | [定向回归](../evidence/p2-s07-route-final-regression.log)通过，M9 Runtime 29 项、ShortestPath 3 项、新绑定恢复 1 项 |
| Web consumer | [wasm 编译复跑](../evidence/p2-s07-web-consumer-retry.log)与 [wasm Clippy 复跑](../evidence/p2-s07-web-clippy-retry.log)通过 |
| 格式、文档与 diff | [格式](../evidence/p2-s07-format.log)、[文档](../evidence/p2-s07-closure-docs.log)、[diff](../evidence/p2-s07-diff-check.log)通过 |

失败日志保留真实结果，后续定向复跑证明修复，不把旧失败记录改写为通过。
workspace 原命令先后发现编译漏迁移和旧错误断言，修复后按未执行目标继续验证，
不宣称单次全量命令从头到尾通过。最后的 obstacle 错误来源整改再精确回归，
已经通过且未改变的 10,000 层销毁测试不重复启动。
历史切片使用当时的旧 suite ID，当前查询测试直接运行 Cargo target。

按主题提交前，另导出只含 S07 代码与调用点迁移的暂存区快照，并加入待提交的四个
查询回归 target。S06 实现和其他任务的改动不在该快照内；复用当前 lockfile 与截图
资源。该范围的 [workspace check](../evidence/p2-s07-commit-check.log)、
[格式](../evidence/p2-s07-commit-fmt.log)、
[Clippy](../evidence/p2-s07-commit-clippy.log)及
[单次 workspace test](../evidence/p2-s07-commit-tests.log)通过，
合计 1,015 项测试。新增 suite 的
[xtask 测试](../evidence/p2-s07-commit-xtask-tests.log)另有 5 项通过，
[文档快照检查](../evidence/p2-s07-commit-docs.log)通过。
该计数对应独立提交范围，不替换上文包含 S06 实现的 1,033 项历史验收结果。

## 规范同步

| 入口 | 同步结果 |
|---|---|
| ADR-018 | 坐标正/逆向返回值、求逆分类、LayoutManager Result 与成功值准入已同步；Runtime 驱动与 constraints 保持 |
| public-api-contract | 任意 ID 身份、合法缺失、测量权威及 Node getter 边界已同步 |
| coordinate-system | 身份检查先于求逆、精确奇异与几何失败、M4 组合方向已同步 |
| tree-search-and-focus | bool/关系短路前校验、冻结视图和 live query 生命周期已同步 |
| ADR-036 | Layout/Route 错误包装与存在性观测已同步；Event 准入和 committed prefix 保留，未重复实施 |
| parity ledger | 受影响 family 保留合法输入 verified 基线，补充本批负测及恢复证据 |
| API snapshot | 通过脚本生成；公开领域路径、错误类型、trait 与调用点同步 |

## 来源审计映射

| ID | 关闭依据 |
|---|---|
| R04 | LayoutSceneQuery 身份与测量失败沿 Result 到 owner；不生成默认成功值 |
| TC-09 | Tree/布局/坐标/Route 区分身份、合法缺失和计算失败，外部消费者证明传播 |
| API-F06 | parent/bool/关系查询拒绝无效身份，root/false/非直接 child 保持领域结果 |
| API-07 | ADR-036 已完成命名迁移；本批清理 Node 非权威尺寸 getter，测量统一使用 Tree/query owner |

上述四项由本记录关闭。P2-S07 的关闭不代表 detached
输入、服务注册、Animation fault 或其他 S08/S09 问题已经整改。
