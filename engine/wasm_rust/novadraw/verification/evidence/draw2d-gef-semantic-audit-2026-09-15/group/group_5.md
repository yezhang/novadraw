# Group 5: GEF 模型、命令历史、EditPart 与投影生命周期语义审计

## 1. 高价值发现

| 编号 | 严重度 / 类型 | 置信度 | 结论 |
|---|---|---|---|
| G5-F1 | P1 / 条件性功能缺陷 | 10/10 | Viewer 已 fault 时，Domain 仍提交模型和 history，之后才返回投影错误 |
| G5-F2 | P1 / 条件性功能缺陷 | 10/10 | 初始 containment 构建失败时，已激活节点因 contents 尚未登记而漏掉 deactivate |
| G5-F3 | P2 / 防御性不足 | 9/10 | Domain history 未绑定文档身份，切换到另一份同类型模型可以执行错误的 undo/redo |

前三条写入同目录 `group_5.jsonl`。没有 P0。本审计是当前实现的 `full_file` 评估，不要求问题由本次 diff 引入。扩展能力差异另列，不因缺少某个 Java 同名 API 自动判 bug。

### G5-F1: Viewer fault 没有阻止后续模型/history 提交

- 定位：[domain.rs:184-220](file:///Users/bytedance/Documents/code/GitHub/drawjs/engine/wasm_rust/novadraw/novadraw-editor/src/domain.rs#L184-L220)，JSONL 主定位 `193-198`。
- 相关链路：Viewer 的 `refresh` 会设置/检查 fault；`validate_selectable` 只检查 Part 存在和 active；`command_for_request` 没有 fault 检查，当前行号见下方映射。无活动手势时，`SelectionTool::cancel` 和 bendpoint cancel 直接成功。
- 触发条件：Viewer 发生 revision gap、dangling endpoint 或扩展返回错误，但仍有 live Part；Host 收到错误后再次调用 Domain 的编辑入口。
- 最小场景：用 G4 的 `DiagramModel`/`NodePolicy` 建 Viewer，发布跳号 revision 并调用 `refresh()` 得到 `RevisionGap`；再向原有节点提交 ChangeBounds。`command_for_request` 仍产生命令，`command_stack.execute(viewer.model_mut(), command)` 更新 bounds、revision 和 undo entry，随后 `viewer.refresh()` 才返回 `Faulted`。最终是“API 返回错误，但发生了一次新的模型/history 修改”。对已有 history 调用 `undo/redo` 同样先移动 history。
- 契约：MVC 要求模型修改和投影职责分离；项目 `architecture.md` 第 12 节额外要求结构一致性破坏后 Editor 停止编辑。这里不是要求投影失败回滚已经成功的原命令，而是禁止 fault 后再接受下一条命令。
- 建议：在 Domain 所有命令入口、Viewer 策略入口建立统一 readiness 检查，覆盖 Viewer、Runtime 和 CommandStack。第一次命令成功、投影随后失败时保留真实 history，并将整个编辑会话锁为 faulted；后续 execute/undo/redo 必须在调用用户命令前拒绝。
- 证据：S，静态调用链确认；G2 的 `revision_gap_faults_viewer_before_projection_changes` 和 G4 的 `domain_executes_compound_move_and_refreshes_then_undoes_and_redoes` 分别覆盖局部行为，尚无二者组合的断言。未运行复现。

### G5-F2: 初始子树失败漏停用已激活的前缀

- 定位：[viewer/mod.rs:2319-2324](file:///Users/bytedance/Documents/code/GitHub/drawjs/engine/wasm_rust/novadraw/novadraw-editor/src/viewer/mod.rs#L2319-L2324)。
- `new` 调用 `create_subtree` 后，只有整棵子树成功才调用 `designate_contents`；`create_subtree` 已先激活当前 behavior/policy，再递归创建子节点。新增 anchor hook 没有改变该路径，已定向复核。
- 触发条件：根/前序子节点已成功激活，后续某个 containment child 的 factory、create_figure、configure_visual、refresh 或 activate 返回 `Err`。
- 最小场景：模型 root 有两个 child；root 和第一个 child 的 `activate` 登记一个外部模型监听；factory 创建第二个 child 时返回 `EditPartError::operation`。`GraphicalViewer::new` 返回错误并析构局部 viewer；`parts.contents()` 仍为 `None`，Drop 提前返回，root 和第一个 child 均收不到 `deactivate`。自动 Drop behavior 对象并不等价于调用契约指定的监听注销函数。
- GEF 依据：`EditPart.java:101-120` 保证不再使用的 EditPart 被 deactivate，注销 activate 中建立的监听；`AbstractEditPart.java:209-225,256-261` 把结构初始化与激活分开。
- 建议：初始化阶段维护独立于 `contents` 的已创建/已激活记录，失败时按依赖顺序清理；或先完成结构构建和登记，再进行受控激活。不能只在成功返回后才建立清理入口。policy 也应跟踪实际激活状态，避免失败后重复或对未激活项盲目停用。
- 证据：S。已有 G2 Drop 测试只覆盖成功构造；G5 的 failed connection activation 测试发生在 contents 已存在之后，不覆盖本路径。未运行故障注入。

### G5-F3: 多 Viewer 的 history 缺少文档归属检查

- 定位：[domain.rs:202-220](file:///Users/bytedance/Documents/code/GitHub/drawjs/engine/wasm_rust/novadraw/novadraw-editor/src/domain.rs#L202-L220)，JSONL 主定位 `203-209`。
- `EditorDomain<A>` 只保存 `CommandStack<A>`，没有文档/session identity 或 Viewer 登记。`GraphicalViewer` 分别拥有 `A`；`undo` 将历史里的命令应用到调用当次传入的 `viewer.model_mut()`。ModelId 的 `Copy + Eq + Hash` 只保证身份可用，不保证不同文档的 ID 不碰撞。
- 最小场景：同类型 Viewer A/B 都有模型节点 `NodeId(2)`，A 初始 x=50，B 初始 x=500；通过 Domain 在 A 上移动节点并入 history；无活动手势时调用 `domain.undo(&mut B)`。G4 已有的 `SetBoundsCommand::undo` 会把 A 的 before bounds 写进 B 的节点，A 仍保持移动后状态，Domain 却把 A 的命令移到 redo。这里无需传入 foreign EditPartId，现有 namespace 检查完全不参与。
- GEF 依据：`EditDomain.java:32-46,62-73` 管理一个或多个登记的 Viewer；Java `Command.undo()` 不接收“当前 Viewer 的模型”，其操作对象由命令本身保留。Rust 改为 `Command<A>::undo(&mut A)` 后，必须另外维护目标文档绑定。
- 定级：当前读取的应用和契约测试均按单 Viewer 配对使用，未观察到实际跨文档调用；依照“当前调用方满足前置条件”的审计规则，降为 P2，而非已发生的数据损坏或 P0/P1。
- 建议：Domain 绑定稳定 DocumentSessionId/模型会话身份。初版若只支持单 Viewer，就显式拒绝其他绑定；真正多 Viewer 必须共享同一文档会话、各有事件游标。身份应在模型/Domain 层，不应把 FigureId/EditPartId 存入 Command；也不能仅以 ModelAdapter 类型、root ModelId 或 revision 判定同一文档。
- 证据：S/U。沿 G4 既有命令实现可静态推导；尚无跨 Viewer history 测试。

## 2. 范围与证据规则

- 日期：2026-09-15。
- Novadraw HEAD：`6b83ac0fa980dc60fd1284607a51460858fa883b` 加持续更新的工作区。**不再与 `evidence/baseline.json` 全部一致**：`part/mod.rs`、`viewer/mod.rs`、`g5_connection_projection_contract.rs` 已在初审后变化。原“一致”声明仅是初审时点的校验，不适用于本修订版。
- 已重新读取这两个源码文件的新增 anchor context/descriptor、endpoint behavior hook、构建/复用/替换/清理调用链和新增 key 测试。用户提示时的 diff 为 part `+92/-1`、viewer `+294/-60`；后续又出现 router/constraint 声明的并行变更，因此该统计不作为最终工作区快照。另已读 `domain.rs` diff，未将删除代码作为问题。
- GEF 基线：`4463d9d0ce13c19d10fbe769d29f28b7345a8cba`，现场 `git rev-parse` 确认。官方依据是该版本的 Javadoc、package.html、`doc-files/mvc.html` 和 `terminology.html`，不是第三方教程。
- 只参考 `org.eclipse.gef` / `org.eclipse.draw2d` 允许范围；未使用 Zest，未读取 `doc/archive`。
- SSOT：ADR-014、ADR-015、`doc/design/editor/architecture.md`、`g5-connection-projection.md`、`doc/parity/gef/api-coverage.md`。账本的 `verified` 是待评估声明，不是本轮证据。
- Group 6 负责 selection/input/Tool/Request 及 Policy target 身份问题，本组不重复报告；最新分工将 Viewer/model fault 和 Domain 多 Viewer history 风险交由本组。
- 技能：已完整读取并采用 `bits-code-guard` 的分组 Step 3、七维度和定级规则，以及仓库 `analyzing-gef-code`。目录：SKILL_ROOT=`/Users/bytedance/.trae-cn/skills/bits-code-guard`；REPO_ROOT=`/Users/bytedance/Documents/code/GitHub/drawjs`；WORK_DIR/ARTIFACTS_DIR=`doc/verification/reviews/draw2d-gef-semantic-audit-2026-09-15/evidence`。本组只交付 `group/group_5.md` 与 `group/group_5.jsonl`，不重复主 Agent 的聚合/HTML 工作。

证据级别：

- **S**：本轮已读完整相关函数、公开入口和直接调用链，静态成立。
- **T**：本轮已读现有测试的具体断言，表示有该断言，不表示本组运行通过。
- **U**：提出的验收/故障注入场景，本组未运行。
- **M**：主 Agent 在协作消息中告知 workspace 测试 **644 通过、2 忽略**；本组未独立核验其运行日志，也不把该结果自动外推到随后新增的 anchor/router 代码。fmt/clippy 失败详情由主 Agent 汇总，不归因到本组具体问题。

本组没有运行 cargo、增加/修改测试或修改源码。下表不存在“本组运行通过”的证据等级。

### 账本状态与本轮结论

以下仅供主审计校准声明边界，不修改 `api-coverage.md`，不把已排期非目标强行改成 bug：

| Family | 账本当前声明 | 本轮可支持的结论与限制 |
|---|---|---|
| `model.identity` / `part.identity` | verified | 稳定模型 ID、namespaced generational Part ID 的单 Viewer 语义有证据；跨文档 history 归属未由这些 ID 自动保证 |
| `model.notification` | verified | snapshot/revision 检查有静态和局部测试证据；不能扩大为整个 Editor fault 闭环已完成 |
| `command.protocol` / `command.stack` / `command.dirty_state` | verified | Stack 内部成功、显式失败、补偿、保存点行为有证据；默认 canRedo、PRE/POST、跨 Viewer 绑定及 Domain fault 联动必须单列差异 |
| `part.factory` | verified | 构建期工厂注入成立；不表示 live factory/root/behavior 替换已经提供 |
| `part.lifecycle` | verified | 不支持无条件维持完整声明：初始失败前缀漏停用已确认；正常 preorder 回调测试不能证明 GEF 生命周期与异常清理完备 |
| `part.visual` / `part.refresh` | verified | primary/content pane 绑定、bounds/style 刷新成立；一般模型属性、内部 visual 和自定义组件刷新仍属部分能力 |
| `part.tree` / `connection.part` | verified | 标准关系和默认投影有证据；新增 endpoint anchor hook/key 已接入且有新增断言，撤回“无 anchor hook”判断。连接子 Part/c-to-c 仍后置；router/constraint 正在并行改动，不能沿用旧的封闭能力结论 |
| `viewer.registry` | verified | 内建一模型键与多 visual/ancestor 映射成立；不等于任意 multi-key alias 或外部自由登记 |
| `viewer.contents_root` | verified | 初始 synthetic root/contents 成立；live contents/root replacement 不在已实现语义内 |

## 3. 详细语义映射

路径缩写：

- Java 相对 `/Users/bytedance/Documents/code/GitHub/gef-classic/org.eclipse.gef/src/org/eclipse/gef/`。
- `C`=`novadraw-editor/src/command/mod.rs`，`M`=`src/model/mod.rs`，`P`=`src/part/mod.rs`，`V`=`src/viewer/mod.rs`，`D`=`src/domain.rs`；省略 crate 前缀的 `src` 均属于 `novadraw-editor`。
- 测试文件 `T1`=`g1_command_stack_contract.rs`，`TM`=`g1_model_contract.rs`，`T2`=`g2_viewer_projection_contract.rs`，`T5`=`g5_connection_projection_contract.rs`，`T4`=`g4_editing_loop_contract.rs`，均在 `novadraw-editor/tests/`。
- “等价”指该行限定语义，不宣称整个 Java 类兼容；“合理变体”需保留语义或明确限定代价；“部分/缺口”不能凭 Rust 所有权自动豁免。

| Family ID | Java 类/方法及行号 | 不可丢失语义 | Rust 公共 API / 实现行号 | 判定 | 测试名称与证据 |
|---|---|---|---|---|---|
| `model.identity` | `doc-files/mvc.html:5-10`；`EditPart.java:179-188,338-348` | 模型独立于 view/controller；绑定稳定事实身份 | `ModelAdapter` M:138-166；`Command<A>` C:65-100；`PartNode::model_id` P:122-125 | 合理变体：业务 ID 与两类运行时 ID 分离；Viewer 拥有 adapter 不等于强迫业务模型继承框架类 | TM `model_adapter_exposes_stable_identity_children_and_ordered_events`:58-78，S/T |
| `model.notification` | `mvc.html:6-10,23-25`；`EditPartViewer.java:278-291` | 模型通知驱动投影，不由命令伪造 Figure 结果 | `ModelEvent` M:92-132；`GraphicalViewer::refresh` V:1473-1519；Domain D:184-220 | 合理变体：typed journal + 全量 reconciliation；事件 payload/subject 不被用于局部刷新 | TM 同上；T4 `domain_executes_compound_move_and_refreshes_then_undoes_and_redoes`:382-415，S/T |
| `model.notification` | `mvc.html:6-10`（GEF 不规定 revision 算法） | Rust 新增的稳定 revision 必须无零值、无回绕 | `ModelRevision::new/next` M:14-40 | 合理变体：应用负责每次提交完整发布 revision；不是 Java 强制契约 | TM `model_revision_rejects_zero_and_advances_without_wrapping`:45-55，S/T |
| `model.notification` | `EditPartViewer.java:278-291`；`mvc.html:23-25` | 不得静默丢通知并宣称视图最新 | `GraphicalViewer::refresh/validate_revisions` V:1473-1557 | 等价目标 + 更严格变体：拒绝 stale/gap；同 revision 批内多事件可接受 | T2 `revision_gap_faults_viewer_before_projection_changes`:258-277；`one_revision_may_carry_multiple_ordered_events`:280-299；`stale_notification_faults_viewer_without_changing_projection`:322-340，S/T |
| `model.notification` | `mvc.html:6-10` | 初始快照与增量流衔接，不能重复应用已包含的通知 | `GraphicalViewer::new` V:656-667 | 合理变体：吸收小于等于初始 snapshot revision 的事件，拒绝未来事件 | T2 `initial_projection_absorbs_events_already_represented_by_the_snapshot`:302-319，S/T |
| `command.protocol` | `commands/Command.java:51-67,105-110,152-160`；`EditPart.java:31-41` | canExecute/canUndo 与模型-only execute/undo | `Command<M>` C:61-100；`CommandStack::execute/undo` C:399-517 | 合理变体：显式 `&mut M` + Result；禁止持有 live Part/Figure 是文档契约，Rust 泛型本身不证明它 | T1 `command_capabilities_reject_undo_and_redo_without_moving_history`:286-323，S/T |
| `command.protocol` | `Command.java:42-49,126-132` | redo 与执行能力、undo 后状态一致 | `Command::redo/can_redo` C:90-100；stack C:520-574 | 部分：默认 redo 调 execute，但默认 can_redo=true，不像 Java 委托 canExecute；模型敏感的 redo 前置条件必须另行实现，当前 can_redo 不接收模型 | T1 `failed_redo_preserves_the_redo_entry_and_model_state`:265-283；动态 canExecute 变假后的默认 redo 未测，S/T/U |
| `command.protocol` | `CompoundCommand.java:18-29,88-120,137-139,185-204` | 非空才可执行；顺序 execute/redo、逆序 undo | `CompoundCommand` C:103-214 | 大体等价；空 compound 的 can_execute=false，但 can_undo/can_redo 因 vacuous all 为 true，直接查询并非 Java 等价，Stack 正常入口不会入栈空 compound | T1 `compound_command_rolls_back_executed_prefix_and_undoes_in_reverse_order`:326-347，S/T；测试是可交换加法，未真正区分 undo 顺序 |
| `command.protocol` | `CompoundCommand.java:137-139,200-204`（无任意副作用事务承诺） | 失败必须区分已知原状态与未知状态，不能伪造原子性 | `CommandError` C:12-59；compound C:135-157,178-204 | 合理增强：补偿成功前缀，补偿失败/unknown-state 令 Stack fault；仅在扩展履行失败无副作用约定下成立 | T1 `compound_undo_failure_restores_the_executed_prefix`:350-365；`failed_compound_compensation_faults_the_stack`:368-381，S/T |
| `command.stack` | `CommandStack.java:21-31,226-251,375-389,429-443` | execute 后可撤销，undo/redo 移动正确条目；新分支清空 redo | `CommandStack::execute/undo/redo` C:399-574 | 等价成功路径；失败路径是有意增强：成功后才清空 redo，Java execute 前清空 | T1 `command_stack_executes_undoes_redoes_and_reports_stable_events`:155-187；`rejected_or_failed_execute_does_not_change_history_or_clear_redo`:190-217，S/T |
| `command.stack` | `CommandStack.java:226-251,375-389,429-443` | 失败不能把未成功操作记为成功历史 | C:435-447,492-510,549-568 | 合理增强：recoverable undo/redo 错误把条目放回原栈，unknown/panic fault | T1 `failed_undo_preserves_the_undo_entry_and_model_state`:245-262；`failed_redo_preserves_the_redo_entry_and_model_state`:265-283；`explicit_unknown_state_failure_faults_the_stack`:454-468，S/T |
| `command.dirty_state` | `CommandStack.java:322-343,234-246` | dirty 指示保存点身份，不只是当前栈长度 | `HistoryState` C:216-227；`mark_save_location/is_dirty` C:620-633 | 合理变体：单调 history identity 区分分支；fault 恒 dirty | T1 `dirty_state_tracks_history_identity_across_branching`:220-242；同长度不同分支和 trim 后保存点建议补强，S/T/U |
| `command.stack` | `Command.java:96-103`；`CommandStack.java:203-210,234-240,253-276,413-422` | trim/flush/dispose 释放不用命令；清理不等于 undo | `with_undo_limit/flush/enforce_undo_limit` C:390-396,640-650,685-693 | 合理变体：Box Drop 代 dispose；None 为无限、Some(0) 不留 undo；flush 与 Java 一样重置 clean，非“保存文件” | T1 `undo_limit_discards_oldest_history_without_changing_the_model`:384-403；`flush_drops_history_and_marks_the_current_document_clean`:406-423，S/T |
| `command.stack` | `CommandStack.java:81-127,213-250,365-368` | observer 能解释命令转变，PRE/POST 的时间语义不能混淆 | `CommandStackEvent/take_events` C:229-272,449-459,652-655 | 部分：只有成功提交后的 journal，无 PRE、失败 POST、历史 dirty/revision 快照；不能替代 Java PRE 监听来开启模型事务 | T1 `command_stack_executes_undoes_redoes_and_reports_stable_events`:174-186，S/T |
| `command.stack` | `Command.java:105-110,152-160`（Java 无 Rust unwind/fault 契约） | 扩展异常后不能继续以未知模型状态编辑 | C:405-447,470-510,577-608；V:1473-1562；D:193-220 | 部分/缺口：Stack 自身保护明确，Viewer/Domain 未形成同一 fault 门禁，见 G5-F1 及第 4 节 | T1 `command_panic_faults_the_stack_and_blocks_later_operations`:426-451；`command_query_panic_faults_the_stack_before_model_mutation`:471-487；跨层未测，S/T/U |
| `command.stack` / `model.identity` | `EditDomain.java:32-46,62-73,110-117`；`Command.java:159` | 多 Viewer 不改变历史命令的文档目标 | `EditorDomain::undo/redo` D:203-220；`GraphicalViewer::model_mut` V:726-728 | 缺口/防御性不足：ModelAdapter 类型相同不足以识别同一文档，G5-F3 | T4 仅单 Viewer 的 `domain_executes_compound_move_and_refreshes_then_undoes_and_redoes`；跨文档测试缺失，S/U |
| `part.identity` | `EditPart.java:179-196` | 控制器身份不等于模型身份；退休对象不能再被误认成新对象 | `EditPartId/EditorNamespace` P:16-47；`PartTree::get/resolve` P:205-208,487-507 | 合理变体：UUID namespace + generational key；查询 Option 与 Result 按入口区分 | T2 `part_ids_are_namespaced_and_rejected_by_another_viewer`:205-213；`removing_and_readding_a_model_rebuilds_runtime_identity`:343-367，S/T |
| `part.tree` | `EditPart.java:142-150,190-196`；`AbstractEditPart.java:769-809` | 有序 containment；重排保留可复用 Part | `PartTree::parent/children` P:210-222；`GraphicalViewer::refresh` V:2144-2191 | 等价在同父重排范围；子树重复 ID/环通过 seen 拒绝 | T2 `viewer_projects_model_tree_and_registers_model_and_visual_identity`:176-202；`refresh_reuses_reorders_adds_and_removes_parts_with_lifecycle_cleanup`:216-255，S/T |
| `part.tree` / `part.lifecycle` | `AbstractEditPart.java:769-809,882-899` | 模型 reparent 后结构正确，旧控制器不再存活 | V:1985-2009,2193-2213；P:461-485 | 合理但有代价的变体：先退役再重建移动子树，Part/Figure 身份、订阅和交互状态不保留，不是 Runtime 同域 reparent 等价 | T2 `reparenting_is_order_independent_and_rebuilds_the_moved_part`:370-392，S/T |
| `part.factory` | `EditPartFactory.java:15-34`；`AbstractEditPart.java:264-278` | 应用可按模型与上下文替换控制器 | `EditPartFactory::create/create_connection` P:847-867；context P:614-706 | 合理变体：静态泛型 factory + Box behavior；连接上下文显式提供 source/target；默认适配降级为 root 普通 context | T2 `RectangleFactory::create`:143-155；T5 `DiagramFactory::create_connection`:221-238，S/T |
| `part.visual` | `AbstractGraphicalEditPart.java:204-211,440-475` | primary Figure 与 content pane 可不同，compound visual 归属清楚 | `VisualBuildContext` P:708-761；V:1582-1611 | 合理变体：构建可新增任意 Figure 子树，content pane 仅允许本 Part 拥有的 visual | T2 `viewer_projects_model_tree_and_registers_model_and_visual_identity`:189-197，S/T |
| `part.refresh` / `part.visual` | `EditPart.java:291-295`；`AbstractEditPart.java:739-818` | 所有显示中的模型属性都能经 controller 刷新 | `EditPartBehavior::refresh_visuals` P:828-836；`VisualUpdateContext` P:763-798；V:2215-2228 | 部分/缺口：只支持 primary bounds/style；Label 文本、内部 Figure、组件内容不能经此上下文更新，不可称通用 refresh 等价 | T2 `one_revision_may_carry_multiple_ordered_events` 仅断言 bounds；无文本/自定义组件测试，S/T/U |
| `part.lifecycle` | `EditPart.java:66-120`；`AbstractEditPart.java:209-225,256-261` | attach/register/refresh 与 activate 有明确顺序 | V:1564-1641；P:838-844 | 部分：本节点登记后 refresh/activate，但递归 children 和 connections 尚未完成；与“complete initial projection exists”的 Rust 注释不符，失败清理见 G5-F2 | T2 `viewer_projects_model_tree_and_registers_model_and_visual_identity`:198-201 锁定 preorder activate；失败前缀未测，S/T/U |
| `part.lifecycle` | `AbstractEditPart.java:297-302,882-899,956-966`；`EditPart.java:315-323` | deactivate/unregister/visual retire 与监听清理；Drop 不等同 removeNotify | V:2230-2288,2291-2352 | 部分：正常删除清 registry 后 dispose；正常 Drop 做 deactivate 后依赖字段 Drop。Rust containment 停用 preorder，不是 GEF 子节点先停用；初始化失败路径缺口 | T2 `dropping_viewer_deactivates_every_live_part_in_preorder`:463-483；G5-F2，S/T/U |
| `viewer.contents_root` | `EditPartViewer.java:360-376,499-516` | synthetic root 无模型；contents 是单一 model-backed child | `PartTree::root/contents` P:160-203；`GraphicalViewer::new/contents` V:656-713 | 等价静态结构；live root 变更被 RootChanged 拒绝，无 setContents replacement，重建 Viewer 是当前途径 | T2 `root_part_is_not_a_model_part`:455-460；root replacement 无测试，S/T/U |
| `viewer.registry` | `EditPartViewer.java:278-304,412-423`；`AbstractEditPart.java:829-858` | model -> Part、visual -> Part 查询和对称清理 | V:801-810,927-943,1136-1155,1604-1609,2129-2141 | 合理变体：私有映射阻止外部污染；支持多个 visual/ancestor 查询；不提供 Java 任意 multi-key model alias | T2 `unknown_and_foreign_visuals_do_not_resolve_to_parts`:427-438；`unregistered_internal_visual_resolves_through_its_registered_ancestor`:441-452，S/T |
| `connection.part` | `ConnectionEditPart.java:15-42`；`AbstractGraphicalEditPart.java:328-333` | source/target 与 containment 分离；两端只能汇合成一个 connection Part | `ModelAdapter::connections` M:155-158；`ConnectionPartId/PartTree` P:60-96,224-273,307-339；V:1644-1793 | 合理变体：一个全局有序快照代替两端分别枚举；关系显式建索引，不用 synthetic parent 暗藏连接 | T5 `ordered_snapshot_projects_one_connection_part_outside_containment`:267-316；`connection_part_id_is_a_checked_view_of_edit_part_identity`:319-333，S/T |
| `connection.part` / `part.tree` | `ConnectionEditPart.java:61-83`；`terminology.html:19-29` | source/target 可为同一节点；连接不能重复登记 | P:307-339,524-540；V:385-403 | 等价 self-loop 关系，两个端点字段仍独立；几何与 handle 由其他组负责 | T5 `self_loop_is_indexed_once_at_both_ends`:336-349，S/T |
| `part.refresh` / `connection.part` | `AbstractGraphicalEditPart.java:640-730` | 连接顺序可更新，保留项不因重排重复创建 | V:1804-1837；P:371-391,524-540 | 合理变体：同一快照顺序同时决定 layer/outgoing/incoming；不保留 Java 两端独立排序自由度 | T5 `model_order_drives_layer_and_relation_order_without_recreating_parts`:352-390，S/T |
| `connection.part` | `ConnectionEditPart.java:31-42`；`AbstractConnectionEditPart.java:286-319` | 端点变化保留连接身份，解除旧关系再安装新关系 | `GraphicalViewer::connection_endpoints` V:818-825；内部 V:1839-1935,2011-2082 | 等价已实现范围：预先 detach 受影响端点，保留未变化 AnchorId，rebind 原 Part/Figure | T5 `reconnect_preserves_part_figure_order_and_unchanged_anchor`:393-429，S/T |
| `model.notification` / `connection.part` | `ConnectionEditPart.java:17-36`（完整快照原子校验是 Rust 增强） | duplicate/dangling/collision 不应造成半投影 | `GraphicalViewer::new/refresh`；`ModelSnapshot::capture` V:369-429 | 合理增强：先校验 containment、全部连接及有限 bendpoints、末尾 revision；计划失败不改投影 | T5 `duplicate_and_missing_endpoint_snapshots_are_rejected`:432-465；`connection_and_containment_model_identity_collision_is_rejected`:468-484；`revision_drift_during_snapshot_is_rejected`:582-600，S/T |
| `part.lifecycle` / `connection.part` | `AbstractGraphicalEditPart.java:343-345,766-769,780-803`；`AbstractConnectionEditPart.java:119-123,257-260` | 连接关系与 anchor 清理不能滞后到 endpoint 退休后 | V:2011-2142；P:461-470 | 等价正常删除：connection-first 协调；删除节点但模型还引用它则 fault，不推断业务级联 | T5 `node_and_connection_can_be_removed_in_one_model_revision`:514-543；`dangling_endpoint_faults_before_existing_projection_changes`:603-624；`viewer_drop_deactivates_connections_before_endpoint_parts`:647-659，S/T |
| `part.factory` / `connection.part` | `AbstractConnectionEditPart.java:111-113,150-152` | connection factory 必须产生满足连接协议的 visual | V:1676-1690,1767-1792 | 等价能力校验；连接初始化错误清退已登记对象；不证明所有分配点异常都有完整回滚 | T5 `connection_factory_must_create_a_connection_figure`:627-644；`rejected_incremental_connection_creation_leaves_no_registration`:692-727；`failed_connection_activation_rolls_back_all_registrations`:730-766，S/T |
| `connection.part` | `NodeEditPart.java:17-35,40-64`；`AbstractConnectionEditPart.java:189-220,228-248` | 根据节点/连接模型选择 anchor，可扩展不同端口 | `ConnectionAnchorContext/Descriptor`、`EditPartBehavior::source_connection_anchor/target_connection_anchor` P:anchor_hooks；`build_connection_anchor/bind_connection_part` V:anchor_binding | 合理变体：endpoint behavior 获取模型与连接上下文，返回稳定 key + 任意 ConnectionAnchor；None 才 fallback Chopbox。相同 endpoint model 与 key 保留 AnchorId，key 改变仅替换该端；不再是“无 hook” | T5 `endpoint_anchor_descriptor_reuses_stable_keys_and_replaces_only_changed_end`:456-497 已读同 key 双端复用、key 改变只换 source、Part 不变断言，S/T；非 Chopbox 几何、target hook、错误 key 仍 U |
| `connection.part` / `part.refresh` | `AbstractConnectionEditPart.java:145-152,228-248`；`NodeEditPart.java:40-64` | 路由策略/constraint 是可替换组合，不能被编辑器刷新无条件覆写 | 新增 `ConnectionRouterKey/Registration/Selection`、`ConnectionRoutingDescriptor` 与 behavior/factory hook P:routing_hooks；Viewer 调用链仍在并行修改 | 本轮保留为待最终快照交叉审查：已见注册 key 和 `Box<dyn RoutingConstraint>` 声明，不再断言接口完全封闭；声明存在不等于最终创建/刷新链路已验证 | 本组未运行新增 router 门禁；不以先前 644 通过结果覆盖后续变更，S/U |
| `connection.part` | `ConnectionEditPart.java:38-42` | 连接可拥有子 EditPart，也可作为另一连接的端点 | P:210-222,509-514；V:394-403 | 后置且明确不等价：connection children 返回 None，endpoint 只接受 containment；G5.1 非目标已声明，不列现存 bug。decorative Figure child 不是 model-backed child EditPart | T5 `ordered_snapshot_projects_one_connection_part_outside_containment`:277-278 明确断言 None，S/T |
| `connection.part` | `AbstractConnectionEditPart.java:223-248` | 路由暂不可解不应冒充业务连接已删除 | V:1795-1802；公开 `connection_part_for_model` V:806-810 | 合理增强：Unresolved 保留 Part 和依赖，不升级 Viewer fault；其它 Runtime error 仍传播 | T5 `unresolved_route_keeps_a_live_recoverable_connection_part`:546-579；`endpoint_geometry_change_is_rerouted_by_runtime`:662-689，S/T |

## 4. Fault、扩展性与 Rust 迁移判断

### 4.1 必须把 fault 的“触发”和“拒绝后续操作”连起来

CommandStack 在 label/canExecute/canUndo/canRedo/execute/undo/redo 调用周围 catch unwind，并设置自身 fault。相比之下：

- Viewer 的 `refresh` 只处理 `Result::Err`，直接调用 model drain/query、factory 和 behavior；`refresh_part_visuals` (`V:2215-2228`) 的用户 panic 会跳过 `fail`。
- `command_for_request` (`V:1178-1186`) 直接执行 policy understands/target/command，没有 unwind 保护。policy 可以通过自身状态或 `&A` 的 interior mutability 留下变化后 panic；外层 Host catch 后，Viewer fault 仍为 false。此处评估异常边界，不重复 Group 6 的 target 身份问题。
- Runtime 自身的 `guarded` 会在其回调 panic 时设置 Runtime fault 后 resume unwind，但 Viewer 不自动继承该状态；普通 behavior/policy 的 panic 更不在 Runtime guard 内。
- 因而只在 `refresh()` 开头补 `if faulted` 不足够。需要在进入扩展调用前建立 fault-on-unwind 边界，并在调用任何命令/策略前检查一致的编辑会话状态。panic 是否转结构化错误或继续 unwind 可保留 Host 选择，但不能宣称继续可用。

建议故障注入：behavior 先成功更新 bounds 再 panic；policy 先改变内部计数再 panic；model `drain_events` 或 `children` panic。Host 使用 catch_unwind 后应观察 fault，并验证后续 execute/undo/redo 无模型/历史副作用。现有 G1 panic 测试不能证明这些投影/策略边界。本组未执行注入；此项作为 G5-F1 对应的完整 fault 迁移范围记录。

### 4.2 生命周期顺序不是 Rust 自动析构能替代的

当前 containment 顺序是 factory -> create_policies -> create/attach Figure -> configure -> Part/registry -> install policies -> refresh -> behavior activate -> policy activate -> 递归 children。所有 containment 完成后才构建 connections。这并不满足 `EditPartBehavior::activate` 注释声称的“完整初始投影已存在”。

正常删除采用父节点先 deactivate，再后代，随后反序 Drop behavior。GEF `AbstractEditPart.deactivate` 则先子节点再父 policy；连接在 endpoint 前停用的主顺序得以保留。父节点资源是否仍需供子节点 deactivate 使用必须成为公开生命周期契约，不能因为测试锁定 preorder 就认定与 GEF 等价。

Drop 的连接-first 测试只验证回调事件序列，没有验证 Runtime anchor/router/visual 的逐项析构顺序。connection 初始化错误有清退分支，containment 初始化的失败前缀清理则不完整。应使用真实订阅 guard/资源计数验证，而非仅打印名称。

### 4.3 模型独立与多视图

`ModelAdapter` 的关联 ID/Event/Error 和 `Command<M>` 不要求业务模型继承框架类，体现模型独立。`GraphicalViewer<A,F>` 是 adapter、PartTree、Runtime 的组合根，behavior/policy 通过借用 `&A` 读取，而命令通过 `&mut A` 写入；没有单例，也没有第二份业务对象仓库。

不过类型系统只证明借用期间独占，不证明文档归属、不禁止模型/behavior 内部共享可变状态，也不禁止命令偷偷保存 live FigureId。多 Viewer 若共享同一底层模型，不能共享一个 destructive drain 队列；应为每个 adapter 建独立通知游标。两个独立 `A` 即使 root ID/revision 相同也不等于同一文档。G5-F3 是这个所有权迁移尚未闭合的边界，而非要求现在实现全部多视图产品能力。

### 4.4 可创建不等于可刷新、可替换

`Box<dyn EditPartBehavior<A>>` 和 `Box<dyn Command<M>>` 在固定 A/M 下对象安全；ModelAdapter 的关联类型也可在固定关联类型后表达对象接口。`GraphicalViewer<A,F>` 当前选择静态泛型组合，并未提供开箱即用的动态 factory 替换入口；这不是 Rust 不可能支持替换。确需运行时切换时可先用应用分发型 factory，框架不必立即引入万能 registry。

实质限制在于 `VisualUpdateContext`：其 Runtime 私有，只公开 primary bounds/style。创建一个 LabelFigure 后，模型 rename 通知无法经 `refresh_visuals` 调用 `Runtime::set_label_text`；已创建内部 Figure 的内容、布局或自定义组件更新同样没有入口。Core 已有 `FigureComponentUpdate`/prepared update 协议，Editor 尚未转交该能力。让 app 在命令后手动拿 `runtime_mut()` 补绘会破坏统一通知刷新链路，不能作为等价证明。

建议给 context 增加本 Part 所有 visual 的受检更新能力，复用 Runtime 的 typed component update 与必要的内置能力 facade，保持 registry/topology mutation 仍由 Viewer 协调。不应直接把整个 `&mut Runtime` 暴露给 behavior。验收至少覆盖 rename/undo/redo、内部 content pane 更新、外部自定义 Figure 内容变更，并断言身份不变、damage/measurement 正确。

### 4.5 Anchor、Router 与 typed constraint

有序 `ModelConnection<I>` 和受检 `ConnectionPartId` 保留单连接与双向关系语义，ID/role 拆分合理。代价是模型必须能给出完整有序连接集合；不具备该集合的领域模型需要 adapter 汇总。

修订后，Editor 已有 `ConnectionAnchorContext`（连接/两端模型及 Figure 上下文）和 `ConnectionAnchorDescriptor`（key + `Box<dyn ConnectionAnchor>`）。`build_connection_anchor` 调用对应 endpoint behavior 的 source/target hook，只有返回 None 才创建 Chopbox fallback。`bind_connection_part` 每次重新求 descriptor，以 endpoint model、anchor 是否存在及 key 相等性决定复用；即使 endpoint ModelId 不变，key 改变也会重建该端 anchor。投影保存 `source_anchor_key/target_anchor_key`，detach 时清理对应 key。**撤回初审的“无 anchor hook/始终 Chopbox”及“同 ID 无 descriptor 驱动更新”判断。**

新增测试 `endpoint_anchor_descriptor_reuses_stable_keys_and_replaces_only_changed_end` 已读取：重复发布相同 key 保留两端 AnchorId；source version/key 改变仅替换 source，target 和 ConnectionPart 不变。测试策略仍使用 Chopbox，不能据此声称任意自定义 port 几何、target hook、错误 key 和异常清理已验证。descriptor 的 key 由应用给出，相等必须意味着足以复用 anchor 的语义等价；与 anchor 自身 `semantic_group_key` 的一致性仍需明确契约，而非只比较 Rust 值就保证正确。

本次定向复核期间，part/lib 又出现 `ConnectionRouterKey/Registration/Selection`、`ConnectionRoutingDescriptor`、`connection_routing`、`connection_routers` 声明，Viewer 正在接入。因此初审的“非空 bendpoints 一律强制内置 Router”不再作为最终工作区结论；最终注册、选择、constraint、refresh/重连不覆写和失败路径应由主审计在稳定快照交叉校验。该声明与本轮已经核对的 anchor 调用链分开记录，不把接口出现直接升级为 verified。

Connection child EditPart、connection-to-connection 已在 G5.1 明确后置，不作为 bug；未来扩展时必须放开角色组合，不能把现在 `PartKind` 的互斥设计当作永远成立的 GEF 契约。

## 5. 七维度复核与测试不足

| 维度 | 本组结论 |
|---|---|
| 逻辑 | 成功路径 history 顺序、snapshot 去重和 relation 更新可追溯；跨层 fault 与初始化失败路径存在 G5-F1/F2 |
| 领域语义 | 模型、Part、Figure 三域分离成立；endpoint anchor hook/key 已新增并接入，撤回旧缺口。通用 visual refresh、多 Viewer history 仍未闭合；router 接入待最终快照 |
| 安全 | 所读范围没有网络、权限、文件路径或脚本执行入口；未发现可证实的注入/越权安全缺陷；跨文档 history 按归属/健壮性问题处理 |
| 并发 | 主 mutation 使用 `&mut` 串行化，没有共享全局状态；`&A` 不排除 interior mutability，snapshot revision 仅验证捕获阶段，不能证明任意回调纯度 |
| 健壮性 | revision/collision/错误 kind 的显式检查充分覆盖正常入口；fault-on-unwind、初始化清理、Domain 文档归属仍需闭合 |
| 性能 | `PartTree::insert_connection/unbind/retire` 每次重建全部索引，批量建立 E 条连接静态操作数可达 O(E²)；`synchronize_subtree` 使用 Vec contains/逐子递归，也不能笼统称所有路径 O(V+E)。没有基准，不据此报告已发生的性能故障 |
| 质量 | 未把宽函数、命名、默认空实现或格式差异列为缺陷；检查重点是承诺与可观察行为 |

测试不足：

1. Compound 逆序 undo 测试使用加法，可交换操作无法区分顺序错误；应使用有依赖的操作或调用记录，另覆盖 redo 补偿和空 compound 能力。
2. 保存点测试未完整覆盖“保存于旧 redo 分支，再建立同深度新分支”、保存点被 trim、limit=0 和 Drop 前缀的全部组合。
3. G2 正常 Drop 断言不能证明构造失败后监听全部注销；需 root/第一子节点激活后下一节点失败的场景。
4. G5 failed activation 只核对模型注册与 layer children，没有直接断言 AnchorId 不可查询、policy 停用次数或扩展析构状态。
5. 连接 endpoint reparent 后保留 connection identity、退休旧 anchor 后重新绑定，以及同 revision 多节点/多连接联合变化仍需专门场景。
6. 通用 visual 更新、应用 router 不被 refresh 覆写、多 Viewer history、模型共享下独立 event cursor 仍需验证。同 ID anchor descriptor key 变化已有新增断言，不再列为无测试；自定义非 Chopbox 几何和 target hook 仍需补充。
7. 主 Agent 报告 workspace 测试通过，只能证明已有场景，没有覆盖上述尚未加入的场景。

## 6. 迁移建议与依赖顺序

1. **先统一 fault 协议**：定义 Domain/Viewer/Runtime/Stack readiness；在策略、命令和模型投影的扩展调用周围确保 fault-on-unwind。验收 fault 后任何新编辑/undo/redo 在模型写入前拒绝；原来已提交的命令 history 如实保留。
2. **补生命周期事务记录**：区分 allocated、registered、behavior-active、policy-active，不依赖成功 contents 作为唯一清理入口。验收各初始化阶段失败，已有订阅/资源恰好清理；继续保持 connection-before-endpoint。
3. **固定文档归属**：先明确单 Domain 的模型会话身份和支持的 Viewer 数量，拒绝错误绑定；之后才扩展共享文档多 Viewer 与独立通知游标。验收同类型、同 ModelId 的两个文档不能互相消费 history。
4. **补足受限 visual 更新能力**：复用 Core typed update，不扩张到任意 Runtime mutation；验收 Label 与自定义 Figure 的非 bounds 属性及 undo/redo，保留 Part/Figure identity。
5. **验收新增连接投影扩展**：endpoint descriptor/key 已实现，不再建议重复引入。补齐 source/target 自定义几何、key 语义和失败生命周期验证；router/constraint 以并行接入的最终版本验收，检查应用 router 不被刷新覆写、未知 key/constraint 在可见提交前拒绝。
6. **最后优化及推进后置能力**：先测 V/E 批量投影操作数和耗时，再批量重建关系索引；connection child/c-to-c、live contents/root replacement 按明确 delta 推进，不回填为现有等价。

这些是后续可执行建议，不是本轮已经实施或验证通过的修改。

## 7. 实际阅读清单

### Rust

完整读取本组七文件：`src/command/mod.rs`、`src/model/mod.rs`、`src/part/mod.rs`、`tests/g1_command_stack_contract.rs`、`tests/g1_model_contract.rs`、`tests/g2_viewer_projection_contract.rs`、`tests/g5_connection_projection_contract.rs`。

初审完整读取 `src/lib.rs` 与 `src/domain.rs`；Viewer 读取构造/snapshot、registry 查询、命令入口及完整投影与析构函数。修订时定向重读 part 的 anchor context/descriptor/hook 与后续 router 声明，Viewer 的 refresh、create_subtree、build_connection_anchor、create_connection_part、bind/detach/remove_connection_part 和 Drop；新增 anchor 测试完整读取。所有 P/V 数字引用重新按本轮工作区符号定位，不再沿用初审行号。selection/input 的部分阅读只用于边界定位，不宣称完成 Group 6 审计。

按最新分工完整读取 `tests/g4_editing_loop_contract.rs`，仅将 model、command、domain 断言用于本组。定向读取 `src/policy/mod.rs:160-318` 的生命周期/PolicyStore 和 `src/tool/mod.rs` 的四个 cancel 函数；没有以此宣称审计全部策略和工具。

直接调用方与共享能力：`apps/native/node-editor-demo/src/main.rs` 的模型 Command、DemoPart、DemoConnectionPart；`novadraw-scene/src/runtime/runtime.rs` 的 `update_component`、`dispose_subtree`、`guarded`、bounds/style setter、`set_label_text` 实现；`novadraw-scene/src/figure/mod.rs` 的 `AsAny`/`Figure` 定义；`novadraw-scene/src/runtime/mutation/mod.rs` 的 `FigureComponentUpdate`/prepared update 定义；修订新增读取 `connection/anchor.rs` 的 `AnchorSemanticKey`、`ConnectionAnchor` 和 Chopbox semantic key 实现。共享文件在并行工作中行号变化，以上以实际读取的符号标识，不把旧行号作缺陷定位。`graph/mod.rs` 仅定向搜索 setter/panic 入口，不计全文阅读。

### GEF 官方源码与文档

完整读取：

- `commands/Command.java`、`commands/CompoundCommand.java`、`commands/CommandStack.java`。
- `EditPart.java`、`NodeEditPart.java`、`ConnectionEditPart.java`、`EditPartFactory.java`、`EditDomain.java`。
- `editparts/AbstractConnectionEditPart.java`。
- `doc-files/mvc.html`、`doc-files/terminology.html`、`package.html`、`commands/package.html`。

定向读取完整相关方法及 Javadoc：

- `editparts/AbstractEditPart.java:150-325,735-905,940-969,1027-1042,1170-1210`。
- `editparts/AbstractGraphicalEditPart.java:190-350,438-544,610-810,865-883`。
- `EditPartViewer.java:264-305,358-376,402-424,495-518,550-560,579-598`。
- `ui/parts/AbstractEditPartViewer.java:350-375,460-485,670-690,750-761,795-820,836-861`。

未将 `rg` 命中、文件存在、旧审计结论或测试函数名当成本轮语义正确性/测试通过的证明。
