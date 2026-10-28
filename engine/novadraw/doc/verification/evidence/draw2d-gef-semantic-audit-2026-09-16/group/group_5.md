# Group 5: GEF Model / Command / EditPart / Projection 语义审计

日期：2026-09-16。范围：`full_file`，以当前工作区内容为准，不限 diff。
结论：确认 3 条 P1 问题；20 条语义映射。CommandStack 的主要失败、历史和保存点路径
静态核对成立；Viewer 初始化清理、panic 隔离和投影复杂度尚未闭合。
本次只做静态审计，未修改产品或测试，未运行 cargo、测试或性能基准，也未提交 Git。

## 1. 确认问题

### G5-F01 [P1] 初始 containment 投影失败会遗漏已激活 Part 的停用

- 类型：业务语义问题；置信度：10/10。
- 主位置：`novadraw-editor/src/viewer/mod.rs:2691-2695`。
- 调用证据：`new:707-708`、`create_subtree:1881-1908`。
- 具体触发：合法模型为 root -> child；root behavior 的 `activate` 成功注册外部模型
  listener，child factory 的 `create` 返回 `EditPartError::operation`。也可以是已成功
  激活若干子节点后，后续兄弟的 visual 创建失败。
- 执行结果：`create_subtree` 在递归前已经激活 root/已有子节点，但 `new` 只有整个
  subtree 成功返回后才调用 `designate_contents`。失败返回触发 Viewer Drop 时，
  `parts.contents()` 仍是 `None`，直接 return，所有已激活 containment behavior 都没有
  收到 `deactivate`。对象内存被 Drop 不等于通过 deactivate 注册/注销的外部订阅被清理；
  应用按协议在 deactivate 中移除的 listener 会残留。
- 官方契约：GEF guide:367-370、400-409、419-431；`AbstractEditPart.activate/
  deactivate` 明确成对维护模型监听。项目 `architecture.md` 第 5、12 节要求生命周期与
  失败清理闭合。
- 建议：以 synthetic root 下实际已创建节点或独立激活记录为清理入口，不依赖已提交
  contents；记录成功激活状态并保证每项只停用一次。也可把初始投影拆成构建/提交/激活，
  但失败清理仍需覆盖部分激活。不要只在单个失败 child 上补一次 deactivate。
- 验证缺口：`g2_viewer_projection_contract.rs:463-483` 仅检查成功构造后的 Drop；
  `g5_connection_projection_contract.rs:797-833` 检查的是增量 connection 激活失败，
  不覆盖初始 containment 创建失败。建议增加 root 已订阅、child 创建返回 Err 的用例，
  用外部 listener 计数确认全部释放。

### G5-F02 [P1] 未变化的连接顺序仍触发平方级全局投影工作

- 类型：性能问题；置信度：10/10。
- 主位置：`novadraw-editor/src/viewer/mod.rs:2174-2181`。
- 具体触发：单 connection layer 中有 E 条有效连接，拓扑及顺序均未变，仅发布一次合法
  属性变化 revision。即使每条边使用常数成本的独立 router，仍可触发下述开销。
- 执行结果：每次 `refresh` 都进入 `synchronize_connections`，逐边调用
  `Runtime::move_child_to_index`。Runtime 在判定顺序相同之前调用
  `validate_child_order_mutation`，其中 `child_order(parent)` 克隆 E 个 FigureId，
  `contains` 扫描该列表；随后 `child_z_index` 又线性查找。E 次调用产生
  Θ(E²) 列表复制/查找，原顺序已经正确也不例外。
- 直接证据：`runtime/runtime.rs:1825-1853,1918-1943`；
  `graph/mod.rs:2447-2451,2476-2482`。E=10,000 时，仅重复克隆 child order 就累计复制
  100,000,000 个 FigureId；这是代码路径计数推导，不是实测耗时。
- 同一投影批次还有独立的全局扫描成本：`bind_connection_part:2238-2247` 在端点/key
  可复用时仍 resolve；`resolve_connection:2144-2151` 的直接 Runtime 实现
  `runtime/runtime.rs:785-798` 每次重新构造整个 layer 的 routing order。
  因而只跳过排序调用仍不足以满足线性 reconciliation。
- 这不是历史缺陷照搬：`PartTree::insert_connection:337-343` 已改为增量追加；
  `bind_connection:357-375` 也只更新相关端点，顺序相同不会重建所有 adjacency。
  但这些修复没有消除 Viewer 到 Runtime 的逐边全局扫描。
- 契约依据：`g5-connection-projection.md` 第 9 节明确承诺每批 O(V+E) 的全局
  reconciliation。GEF 的 `refreshSourceConnections/refreshTargetConnections` 有当前位置
  快速检查；不据此宣称 Java 任意重排场景严格线性。
- 建议：每批只读取/比较一次 layer 顺序；保持未变项不进入单项全局校验，改变顺序时使用
  受检批量 reorder。routing order 每批复用，路由按 dirty/group 批量协调；性能优化不得
  绕过 Runtime 的验证与原子提交边界。补充规模递增的操作计数或基准后再关闭此项。
- 现有验证：G5 测试只有少量边的顺序/身份断言，没有工作量上界。containment 中同类
  `desired.contains`/逐项 reorder，以及批量删除时重复压缩 connection_positions，
  见第 5 节复杂度分析，归并在此项，不重复生成缺陷。

### G5-F03 [P1] Projection 扩展 panic 绕过 Viewer fault 标记

- 类型：健壮性问题；置信度：9/10。
- 主位置：`novadraw-editor/src/viewer/mod.rs:1768-1779`。
- 具体触发：有效 revision 批次使 `refresh` 调用某个 `EditPartBehavior::refresh_visuals`；
  behavior 先通过 context 修改 primary bounds，再 panic。Host 在 Viewer 外围捕获
  unwind 后检查 `viewer.is_faulted()`。这是显式扩展 panic 契约的失败用例，不是声称当前
  demo 正常交互必然 panic。
- 执行结果：通知已在 1746 行 drain，前面节点/当前 visual 可能已更新，但 panic 不会
  经过 `match result` 的 Err 分支，因此 Viewer 仍报告 `faulted == false`，且
  `applied_revision` 仍是旧值。若是 behavior 自身 panic，Runtime 也未必 fault。
  后续 refresh 可能因已耗尽通知而报 gap，但不能替代在 unwind 当时立即隔离半提交状态。
- 调用证据：`synchronize_subtree:2522` -> `refresh_part_visuals:2587-2599` 是直接调用，
  Viewer 模块没有 unwind guard。`EditorDomain` 的直接命令调用方在成功执行命令后调用
  `viewer.refresh()`（`domain.rs:206-229`），此处仅确认调用边界，不审查 Tool/Policy。
  Runtime 自己的 `guarded:1534-1541` 会标记 Runtime 后 resume_unwind，也不会替 Viewer
  设置 fault；CommandStack 则已经单独实现 catch_unwind。
- 契约依据：项目 `architecture.md` 第 12 节及 `g5-connection-projection.md`
  第 10 节显式要求 extension panic 使 Viewer faulted。Java GEF 不提供这个增强保证，
  缺陷是 Rust 声明的 fault 契约未实现，不是 Java 同名 API 差异。
- 建议：从 model drain/query 到投影提交建立统一 Viewer unwind/poison 边界，
  unwind 时先设置 fault，再选择 resume_unwind 或结构化上报；不能无标记地吞掉 panic。
  正常 Err 与 panic 共享一致性隔离原则，不承诺任意扩展副作用自动回滚。
- 验证缺口：4 个本组测试文件没有 Viewer callback panic 用例。建议用“修改 visual 后
  panic”的 behavior，在外层捕获后断言 Viewer faulted、revision 未提交、后续 refresh
  拒绝；另覆盖 Runtime callback 的传播。未在本次执行此验证。

## 2. 阅读范围与官方证据

### 2.1 启动与方法

按顺序读取 `AGENTS.md`、`CLAUDE.md`、本次 `reviewer-brief.md`，再读取
`review_groups.md` Group 5；补读项目记忆与最近日摘要。采用本地
`analyzing-gef-code` 和 `bits-code-guard` 的通用工作流、七维检查、严重度/置信度规则。
本任务是既有六组审计的 Group 5，不重新生成总报告或其他组产物。

- `SKILL_ROOT`：`/Users/bytedance/.trae-cn/skills/bits-code-guard`。
- `REPO_ROOT`：`/Users/bytedance/Documents/code/GitHub/drawjs`。
- 项目根：`engine/wasm_rust/novadraw`。
- `WORK_DIR` / 本组产物边界：本报告所在 `evidence/group` 目录。
- 项目 HEAD：`e8d54ac063d05c63abb9f2850365a837ec4e8c9d`，有既有未提交修改；
  结论针对读取时的工作区，不声称仅属于该 commit。
- GEF HEAD：`4463d9d0ce13c19d10fbe769d29f28b7345a8cba`，与 GEF parity 账本基线一致。
- 未使用 Zest、旧报告结论或 `doc/archive`；未分析 Tool/Policy 实现。

### 2.2 先读官方基线

以下 Java 路径相对于
`/Users/bytedance/Documents/code/GitHub/gef-classic/org.eclipse.gef/src/org/eclipse/gef/`：

- 官方 guide：`org.eclipse.gef.doc.isv/guide-src/guide.adoc:25-264`
  （MVC、模型通知、三套结构、content pane、connection layer、factory/refresh）、
  `357-456`（commands 与 lifecycle）、`827-837`（redo 必须恢复原模型身份）。
- `commands/Command.java`、`commands/CompoundCommand.java`、`commands/CommandStack.java`
  全文，包含 Javadoc、异常 finally、save location、limit、dispose 与 listener 行为。
- `editparts/AbstractEditPart.java:154-308,737-905,940-979,1028-1050,1175-1215`：
  add/remove、activate/deactivate、refreshChildren、model/visual registry。
- `editparts/AbstractGraphicalEditPart.java:192-347,437-520,612-774,862-879`：
  content pane、connection 两端发现/去重/重排、注册与删除。
- `editparts/AbstractConnectionEditPart.java:35-146,247-323`：
  connection layer、setSource/setTarget/setParent 与 removeNotify。
- `EditPartViewer.java:266-307,402-427,490-518`、
  `ui/parts/AbstractEditPartViewer.java:353-375,457-473,670-688`：
  双 registry、domain notification lookup、factory 和 contents。

先确立的契约：模型持久身份/状态与 controller/view 分离；命令只改模型且通过 stack；
模型通知驱动 refresh；activate/deactivate 配对；删除后不复活旧 EditPart；
child Figure 进入 content pane；connection 是独立关系、共用 registry 并进入独立 layer。
Java compound/stack 不保证失败回滚，不能把 Rust 补偿/fault 误写成 Java 既有语义。

### 2.3 当前实现与验证源码

全文读取 `novadraw-editor/src/{model,command,part}/mod.rs`。Viewer 读取 model snapshot、
root layer 创建、构造、模型/Runtime 访问、registry/ancestor lookup、全部 refresh、
containment/connection 创建、rebind、清理和 Drop 完整函数；Viewer 输入/Tool/Policy
决策不在本组，连接预览 helper 仅在稳定 anchor descriptor 的共享调用边界定位。

共享引擎只读取投影所调用的职责：`novadraw-scene/src/graph/mod.rs` 的 child order、
attachment、z-index、reorder 和 style；`runtime/runtime.rs` 的 connection 注册/删除、
配置/resolve 完整函数、Figure add/dispose/order、bounds/style mutation 及 guard；
`novadraw-scene/src/lib.rs` 全文核对公开出口。未扫描渲染主循环。

设计交叉检查：`doc/design/editor/architecture.md`、
`doc/design/editor/g5-connection-projection.md`、`doc/parity/gef/api-coverage.md`。
静态全文读取以下 4 个测试文件，共 49 个 `#[test]`，只用作覆盖证据：

| 简写 | 文件（相对于 `novadraw-editor/tests/`） | 用例数 |
|---|---|---:|
| T1 | `g1_model_contract.rs` | 2 |
| T2 | `g1_command_stack_contract.rs` | 14 |
| T3 | `g2_viewer_projection_contract.rs` | 15 |
| T4 | `g5_connection_projection_contract.rs` | 18 |

## 3. 详细语义映射

本节 Rust 路径中 `model`、`command`、`part`、`viewer` 分别表示
`novadraw-editor/src/<名称>/mod.rs`。状态“静态吻合”不等于本次运行验证通过；
“合理迁移”“明确收窄”“待闭合”也不修改既有 parity 账本枚举。

| ID | Java / 官方契约 | Rust API 与具体证据 | 状态、验证与局限 |
|---|---|---|---|
| G5-M01 | guide:50-58、`AbstractEditPart.setModel`：业务模型任意，模型是可持久化事实源，命令修改模型 | `ModelAdapter`（model:132-159）以关联 `ModelId/Event/Error` 和有序 children 暴露模型；`Command<M>`（command:61-101）只接收 M，不接收 Viewer | 合理迁移。T1:58；稳定 ID 是 adapter 责任，`Copy+Eq+Hash` 本身不能保证持久性。命令不保存运行时句柄仍是契约约束，不是类型系统完全禁止 |
| G5-M02 | guide:138-142、`EditPartViewer.getEditPartRegistry:279-291`：模型监听可以集中分发到 Part，不强制每对象独立 listener | `ModelEvent`/`ModelRevision`（model:8-58,90-130），`GraphicalViewer::refresh/validate_revisions`（viewer:1742-1810）消费有序批次 | 合理迁移。T1:45、T3:258,280,302,322。接受新 revision 同批多事件，拒绝 stale/gap；事件 subject/payload 不参与局部筛选，按最终快照重投影 |
| G5-M03 | `Command.canExecute/execute/undo/redo`；`CommandStack.execute:226-250`：拒绝不可执行命令，成功执行进入历史，新执行终止 redo 分支 | `CommandStack::execute/undo/redo`（command:398-574）区分 Rejected、NothingToUndo/Redo、CommandError，只有成功后移动历史 | 静态吻合并增强。T2:155,190,245,265,286。Rust recoverable execute 失败保留 redo；Java 在调用 execute 前已 flushRedo，这项差异是有意的失败原子性增强 |
| G5-M04 | `CompoundCommand.canExecute/canUndo/execute/undo:88-139,200-204`：非空、所有成员可执行、顺序 execute、逆序 undo | `CompoundCommand`（command:103-214）记录 executed，执行失败逆序补偿，undo 失败正序恢复已撤销后缀 | 合理迁移。T2:326,350,368；补偿失败/unknown/panic fault stack，不宣称任意副作用回滚。空 compound 被 can_execute 拒绝；Java unwrap/chain/getChildren 未逐 API 复制 |
| G5-M05 | `CommandStack.execute/undo/redo` 的 try/finally 只保证 POST 事件，不提供异常后的模型恢复 | `CommandError::operation/state_unknown` 与 stack catch_unwind（command:19-44,404-447,462-608,666-683）明确普通失败/未知状态/扩展 panic | Rust 增强。T2:426,454,471。fault 后 dirty 恒真，execute/undo/redo/flush/save 拒绝。Drop panic 明确不受支持；缺少 undo/redo/query 多种 panic 与嵌套 compound 的实测证据 |
| G5-M06 | `CommandStack.markSaveLocation/isDirty:323-342` 与 execute 的 saveLocation 失效：dirty 必须区分保存分支，而非只看历史长度 | `HistoryState`/entry.before/after 与 `save_state`（command:216-227,449-458,512-515,569-572,620-649）采用唯一历史身份 | 静态吻合。T2:220,406。undo 回保存身份变 clean，新分支即使同深度也不同身份；flush 与 Java 一样清历史并标记 clean，不等价于持久化保存 |
| G5-M07 | `Command.dispose`、`CommandStack` 的 limit/flush/dispose（Java:203-210,234-240,253-275,413-421）释放历史持有资源 | Rust owned `Box<dyn Command>`、`with_undo_limit`、`enforce_undo_limit`（command:390-395,640-649,685-693）通过 Drop 释放 | 合理迁移。T2:384,406。None 为无限；Some(0) 为零保留，与 Java 0 不限不同；无动态 setter/getCommands 的完整对等 API，trim Vec 为 O(limit)，不是恒定时间 |
| G5-M08 | `CommandStack` PRE/POST event listener、mark-save/flush 通知（Java:37-127,338-367） | `CommandStackEventKind`/`take_events`（command:229-272,652-655）只记录已提交 transition，拥有 label String | 明确收窄。T2:155 断言成功事件顺序；不是 Java PRE/POST batching callback 等价物，失败/fault 无 journal event，消费者必须检查返回值和 is_faulted |
| G5-M09 | guide:445-453：controller 删除后失效，undo 创建新 controller；模型原身份在 redo 时恢复（guide:832-835） | `EditorNamespace+KeyData`、`PartTree::get/resolve/retire_subtree`（part:17-47,208-237,484-544）；Viewer 删除 registry 后销毁 Figure（viewer:2602-2660） | 合理迁移。T3:205,343,370；跨 Viewer/retired ID 不再解析。ConnectionPartId 是同 EditPartId 的受检角色包装，不建立第二身份域 |
| G5-M10 | guide:158-180、`AbstractEditPartViewer.setContents`：synthetic root 无模型，factory 创建 contents 和 descendants | `PartTree::new/designate_contents`（part:163-191,431-436），`GraphicalViewer::new/create_subtree`（viewer:664-710,1833-1910），两类 FactoryContext | 静态吻合。T3:176,455；Rust 只在 new 绑定 root，后续 RootChanged fault，不提供 live setContents/root replacement。RootLayerFactory 在目标设计中有列出，当前 root 组合仍为固定 helper |
| G5-M11 | `AbstractEditPart.addChild/addNotify/activate/removeChild/removeNotify`：挂载、注册、刷新、订阅，移除逆向清理，生命周期不能漏配对 | `create_subtree`、`remove_subtree`、Viewer Drop（viewer:1833-1910,2602-2724）集中维护行为、Part、Figure、registry | 部分吻合，G5-F01。T3:176,216,463 固定 preorder activation/deactivation；Rust 在创建 descendants 前激活当前节点，与 guide 的 subtree refresh 后激活不同，不能声称完全相同。part:1087 的“complete initial projection exists”注释也不能作为实现事实 |
| G5-M12 | guide:107-112、`AbstractGraphicalEditPart.getContentPane/addChildVisual/removeChildVisual`：compound Figure 的指定 pane 承载子 Part 视图 | `VisualBuildContext::add_child/set_content_pane`（part:919-971）限制本 Part 所有 visual；`create_subtree` 用父 content_pane，reorder 同样用 pane（viewer:1842-1854,2552-2558） | 静态吻合于创建/挂载。T3:176 检查 primary 与 pane 不同；动态 pane resize、内部 label 更新能力不完整，见第 4 节，不能把此行扩张为完整 compound visual 编辑能力 |
| G5-M13 | `AbstractEditPart.registerModel/unregisterModel`、`AbstractGraphicalEditPart.registerVisuals/unregisterVisuals`、Viewer 双 Map：通过模型或命中 visual 找 Part | `model_registry/visual_registry`（viewer:647-651,1873-1876）、lookup（962-976,1099-1114,1308-1327）；remove paths 对应注销 | 合理迁移。T3:427,441；Rust 注册所有 context-owned internal visuals，也可 ancestor fallback。公开 map 不可变，不提供 Java 任意多 key 注册；`runtime_mut` 可绕过所有权，Host 不应自行销毁受管 primary/pane |
| G5-M14 | `AbstractEditPart.refresh/refreshChildren:747-809`：依据模型 child 列表复用、重排、创建和移除 controller | `ModelSnapshot::capture_subtree`（viewer:432-458），`parts_to_retire/remove_reparented_subtrees/synchronize_subtree`（2355-2378,2516-2585）先确定退休项，再按 desired 顺序同步 | 身份与最终拓扑静态吻合；复杂度待闭合。T3:216,370,409。事件批次合并成最终快照；reparent 重建 moved subtree，未承诺保留 moved Part 身份。逐兄弟 contains/reorder 是平方级风险，见 G5-F02 |
| G5-M15 | guide 模型通知契约没有统一 revision 数值协议；Java refresh 直接查询模型 | `ModelSnapshot::capture`（viewer:384-429）前后校验 revision；检查 duplicate/cycle、模型与连接 ID 冲突、缺失端点；refresh 在修改投影前完整验证 | Rust 增强。T3:395,409、T4:499,535,649,670。属性仍由 behavior 从 `&A` 读取，结构快照不是模型全部数据的深拷贝；内部可变/跨线程 adapter 必须保证稳定查询，不把 revision 检查当作锁 |
| G5-M16 | guide:224-232、`AbstractGraphicalEditPart.createOrFindConnection/refreshSourceConnections/refreshTargetConnections`：两端可发现同一连接并用 registry 去重 | `ModelAdapter::connections` 的单有序全局描述（model:60-88,153-156）；snapshot.connection_indexes 与 `create_connection_part`（viewer:397-416,1976-2141）只建一个 Part | 合理迁移。T4:291,360,499；canonical order 消除双端顺序冲突，不复制 Java 两次发现。source/target 必须是本快照 containment，拒绝半连接 |
| G5-M17 | `AbstractConnectionEditPart.activateFigure/setSource/setTarget`：连接 visual 属于 connection layer，source/target 不是普通 child 关系 | `PartKind::Connection`、独立 endpoints/outgoing/incoming（part:150-159,310-428）；Viewer 在 connection layer 创建 visual（viewer:2014-2024），checked role 查询 | 静态吻合于约定子集。T4:291,343,360,376。parent/children 返回 None 而不是 Java 的 root parent；connection-to-connection 与连接 Part children 明确后置；增量索引不能证明整条 projection 是 O(E) |
| G5-M18 | `AbstractConnectionEditPart.setSource/setTarget`、`AbstractGraphicalEditPart` source/target 管理：重连保留连接对象并更新端点 | `bind_connection_part`（viewer:2187-2299）按 endpoint model/part 和 AnchorSemanticKey 决定复用；`PartTree::bind_connection`（part:347-377）只移除/插入相关 adjacency | 合理迁移。T4:417,456；保留 Part/Figure 和未变化端 anchor，shared Runtime 使用稳定 ConnectionId。未知 key/注册失败会使 refresh fault，不声称任意扩展失败均回滚全部 Viewer 状态 |
| G5-M19 | `AbstractGraphicalEditPart.removeNotify`、`AbstractConnectionEditPart.deactivateFigure/removeNotify`：连接从 registry/layer/端点解绑；view 销毁不是业务删除 | `prepare_connections_for_containment_change/detach_connection_binding/remove_connection_part`（viewer:2380-2514）先解连接，再移除端点；Drop 先 connection 后 containment | 正常路径静态吻合。T4:554,581,714,759,797；dangling model endpoint 在快照期拒绝。Drop 保证停用回调，不等同于逐个执行完整 remove_connection_part。复杂度和初始化失败不由这些成功测试覆盖 |
| G5-M20 | Java refresh/createFigure/生命周期回调是扩展点；异常无统一 poison 保证 | `GraphicalViewer::fail`（viewer:1828-1831）只覆盖 Result Err；`resolve_connection` 对 Unresolved 保留可恢复连接（2144-2151）；CommandStack 已独立隔离 panic | 待闭合，G5-F03。T4:613 验证 unresolved 不 fault，T4:759,797 覆盖显式错误；没有 callback panic 后的 Viewer 状态验证。不得把底层 Runtime fault 与 Viewer fault 合并推断 |

## 4. 合理迁移、收窄与后置能力

### 合理迁移

- 应用关联类型 ModelId 代替 Java Object 身份；generational Part/Figure 分域，
  undo 重建 view 而保留业务 ID。
- 单一 canonical connection snapshot 代替两端重复发现；关系/containment 分离，
  AnchorSemanticKey 复用策略由 endpoint behavior 提供，不使用地址身份猜测。
- Result、recoverable/unknown-state、compound 补偿、history identity 和 stack fault
  是 Rust 明确增强，不以 Java 没有同名方法判为不兼容。
- 全局 notification drain 是 GEF domain listener 的合理变体；不透明 payload 下按批
  reconciliation 合理，但算法复杂度仍须符合明确契约。

### 明确收窄及尚需写清的边界

- Stack journal 只有 committed events；没有 Java PRE/POST listener、失败后 POST 通知、
  动态 undo limit、完整历史对象枚举。`can_redo` 默认 true 且不接收模型，与 Java 默认
  canExecute 不同，应用通过自身状态或 redo 返回错误表达约束。
- Viewer 只支持构造时 root/contents；不开放任意 registry key，也不允许半连接、
  connection 作为 endpoint 或 containment parent。这些不应伪装为缺陷。
- `VisualUpdateContext:974-1009` 只有 primary bounds/style setter；虽然能读到 content pane
  ID，却没有 pane/internal visual bounds、文本、layout 或通用受检组件更新入口。
  `VisualBuildContext` 也只有 add_child/set_content_pane。创建静态 compound visual 可行，
  不能据此宣称拥有 Java `refreshVisuals` 对任意内部 Figure 的完整更新能力。
  这属于当前公开 API 的能力收窄且需要规范明确，不依赖未指定产品行为上升为功能 bug。
- `activate` 当前是 parent-first、children 创建前触发，测试锁定此顺序；项目接口注释的
  “完整初始投影之后”与实现不一致。需区分“本 Part visual 已就绪”和“整个 Viewer 已就绪”，
  生命周期失败遗漏则已单独报告 G5-F01。
- `model_mut/runtime_mut` 是 Host 逃生口，允许应用绕过 command/history 或损坏受管视图；
  不把所有逃生口用法自动判 bug，但必须明确管理对象的所有权和调用前置条件。

### 已明确后置

- connection-to-connection、connection child label：G5.1 非目标。
- 文档保存/加载与重建的端到端一致性：parity `document.persistence` 仍为 specified，
  save location 只表示历史标记，不是 serializer 已交付。
- clipboard、direct edit/IME、snap/guides、palette 等依账本后置；TreeViewer/Workbench
  集成不采用。Tool/Policy/交互能力的完整审计归 Group 6，不在此提升或降低状态。

## 5. 外部扩展性、复杂度与 Rust API

### 外部扩展性

ModelAdapter 不强制业务 schema；Factory 拿到 parent/model 或 source/target context；
Behavior 承担 Figure 构建、refresh、订阅和 Anchor/Router descriptor。trait 小于 Java
宽接口，但主要扩展出口真实存在。稳定 anchor key 与命名共享 router 避免把连接策略
硬编码在 Viewer。VisualUpdateContext 的能力不足需单列，不能以 Host `runtime_mut`
手改视图代替 notification -> behavior 的框架扩展契约。

### 算法复杂度

设 V 为 containment 节点数，E 为连接数，d(p) 为 p 的直接 child 数，deg(v) 为端点度数；
以下是源码推导，哈希操作按平均 O(1)，未测量耗时，也不包含任意应用 callback 成本。

| 路径 | 当前实际成本 / 结论 |
|---|---|
| `ModelSnapshot::capture` | O(V+E) 查询/存储，外加递归栈 O(depth)；重复 ID 同时阻止 cycle。MAX_PART_TREE_DEPTH=10,000 只是逻辑上限，不是该深度已实测可安全占用原生栈 |
| 不透明通知批处理 | 一次批次只捕获一次最终快照；不是 O(变更数) 的细粒度更新，这种收窄有文档依据 |
| retained containment reconciliation | `desired.contains` 对每个 existing child 扫描；`PartTree::reorder_child:445-466` 逐项 position；Runtime 同样复制/扫描列表。未改变拓扑也为 O(Σ d(p)²)，宽树可达 O(V²) |
| connection 初建索引 | `insert_connection` 平均 O(1) 追加 endpoint/position，已经消除每插入一边重建所有 adjacency 的旧问题 |
| endpoint rebind | 未变端点 O(1) 早退；变更端点的 retain/Vec insert 为 O(相关 deg)，不是 O(E) 全索引重建，但集中重连高阶节点仍可能累计平方级 |
| 显式 connection order 变更 | `set_connection_order` 校验 O(E)，顺序相同直接返回；不同则 positions/adjacency 各 O(E) 重建一次，合理 |
| Viewer connection reconciliation | G5-F02：逐边 Runtime 顺序校验及逐边 routing-order 复制导致 Ω(E²)，独立于 adjacency 增量实现；不能宣称整体 O(V+E) |
| 批量删除连接 | `retire_connection:423-427` 每边 retain 全局 connections 并 rebuild positions，删除 E 边累计 O(E²)；`remove_connection_part` 还逐次扫描 visual_registry 查 overlay，成本随场景规模增加 |
| Command history | unlimited push/pop 摊销 O(1)；每次超过限额，Vec 前缀 drain 移动剩余项，O(limit)；compound 正常操作/补偿为 O(成员数)，不计应用逻辑 |

### Rust API 评价

- namespaced generational ID、受检 ConnectionPartId、不可直接修改 PartTree 的公开面
  有利于隔离 foreign/stale/kind 错误。
- ModelAdapter 保留 typed Error；Viewer 将模型错误转为 String，诊断仍可读，但丢失 typed
  source；作为 API 取舍记录，不当作纯质量缺陷上报。
- CommandStack 的 `can_undo/can_redo` 需要 `&mut self` 是因为 query panic 可能 fault，
  不是不必要的借用风格问题。
- 行为由 `&mut self` 串行调用，未发现本组框架内部并发数据竞争；应用使用内部可变共享
  模型时，稳定 revision 和同步仍由 adapter 保证。
- Viewer 当前是持有模型的泛型组合根，重建/外部多视图可以由共享模型 adapter 承担；
  不能从 `GraphicalViewer<A,F>` 的存在推导出多 Viewer 共用命令栈已完成验证。

## 6. 七维检查与验证边界

| 维度 | 本组结论 |
|---|---|
| 逻辑 | history 成功/失败移动、保存身份、registry 正常增删、snapshot 校验静态吻合；panic 半提交见 G5-F03 |
| 业务语义 | 初始化失败遗漏 deactivate 为 G5-F01；模型/视图身份分离及正常 connection-first 清理吻合 |
| 并发 | 框架调用以可变借用串行，无本组可证实竞争；不为应用内部同步背书 |
| 健壮性 | stack fault 增强真实实现；Viewer Err 与 panic 隔离不对称，见 G5-F03 |
| 性能 | 保留增量索引修复事实；全局 projection 仍有 G5-F02 的确定性平方级路径 |
| 安全 | 本组无网络、认证、脚本执行等攻击入口；未发现可证实安全缺陷 |
| 质量 | 不按函数长度、命名或风格报缺陷；仅记录会影响契约理解的 activation/visual API 边界 |

现有 49 个用例覆盖主要成功路径及若干 Err 路径，不等于本次通过，也不证明完整 GEF
等价。建议后续门禁重点覆盖：初始 containment 部分创建失败、Viewer 扩展 panic、
大规模未变顺序/批量删除的工作量上界、动态 content pane 内部更新、非交换 compound
顺序、嵌套补偿失败、undo/redo/query panic、保存点位于被放弃 redo 分支及 limit=0。
这些建议仅为缺口清单，本次未新增或执行测试。

本报告和同名 JSONL 只保存最明确的 3 条缺陷。现有 GEF parity 的 `verified` 字样属于
项目原账本状态，本次不修改它，也不据此跳过实际实现核对。底层 route 原子性和 Locator
完整审计由 Group 4 负责；Domain/Tool/Policy fault 传播及交互决策由 Group 6 负责。
