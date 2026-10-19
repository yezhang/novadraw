# P2-S06 失败隔离完成记录

类型：`verification`

日期：2026-10-15

状态：`complete`，按用户接受的验证矩阵修订 2 关闭 R06/TC-08。

目标契约：[ADR-037](../../adr/adr-037-editor-incremental-failure-isolation.md)。
关闭依据：[验证矩阵修订 2](../plans/p2-s06-incremental-failure-isolation.md)，
用户已接受[组合证据与 Native 宿主验收范围](p2-s06-remaining-evidence-review-2026-10-15.md)。
过程证据及失败日志索引见[实施证据](p2-s06-implementation-evidence-2026-10-15.md)。

## 根因与实现

增量同步绕过初始创建的失败清理，导致后代 factory 拒绝后新父 Part、Figure 和
registry 残留。Overlay 删除只逐项检查，重复 ID 会在第一次删除后才拒绝。
Connection 晚期失败的清理还需要覆盖独立 Connection Part 作用域及先取得的 anchor。

已完成：

- 初始与增量创建共用一次新建调用的资源基线和显式清理；
  清理本次新增资源，保留完整 synchronize 的成功前缀。
- policy/behavior 记录成功 activation 前缀和已尝试 deactivation，
  显式清理与关闭不重复 callback。
- `ProjectionCleanup` 保留原始失败、嵌套清理诊断、清理错误及存活/退休身份。
  Core fault 后停止普通 mutation，不通过解锁 Runtime 继续清理。
- Connection cleanup 按独立 Connection Part 处理；先取得的 source anchor
  在 target provider/binding/activation 晚期失败后清理。
- 连接登记和 Figure disposal 在结构写入前校验 dirty revision；
  不在 overflow 拒绝后留下未完成的 binding 登记。
- Overlay 删除完整校验重复、祖孙重叠、身份、owner 和跨 owner 后代；
  新增经 `Runtime::add_figures` 共用 Figure/style 整批准入。
- 非法 route 成功提交为稳定 unresolved 时保留 Part、anchor 和依赖，
  后续合法输入恢复 resolved。
- Viewer fault 停止新编辑/prepare；Runtime 健康时仅允许结清已有 completion；
  Core fault completion 保持结构化 `Faulted`，新 Runtime 拒绝旧 session completion。
- 模型命令已成功时保留模型 revision、history 和 dirty，不自动 undo。

内置与外部 Figure/Tool/Handle 使用相同准入、owner 校验、错误和清理入口。
没有新增公共 Plan/lease、公共 registry getter 或测试注入 API；
`render_recursive.rs` 未修改。

## 逐项证据

下表使用实际测试名或明确的组合证明，不能将静态复核写成集成通过。
G2/G4/G5 integration fixture 通过公开 factory/behavior/policy/Router API 使用引擎；
独立 `editor-open-protocol` consumer 验证外部 typed Tool/Handle 同路径。

| 矩阵 ID | 证据 |
|---|---|
| P01、P06 | G2 `failed_incremental_subtree_clears_new_parent_and_preserves_successful_sibling`：完整新增子树清除，成功兄弟身份保留，目标 revision 不接受 |
| P02 | Editor `s06_creation_failures_clear_all_acquired_registries`：configure/refresh/activation Err 与 configure panic；复合 Figure、Part、registry 退休 |
| P03 | G2 `late_initial_projection_failure_deactivates_the_complete_live_prefix_once`、`policy_activation_failure_rolls_back_only_the_active_prefix_once`；Editor policy cleanup panic 用例记录成功/尝试前缀 |
| P04a、C05c | Core 两项真实 revision overflow/disposal 用例 + Editor `s06_cleanup_driver_keeps_real_refusal_and_continues_independent_roots` + 下述 Part/Connection 静态复核；按修订 2 接受组合证明 |
| P04b、P04c | Editor `s06_cleanup_panic_reports_retired_structure_and_never_repeats_callback`、`s06_fault_close_preserves_original_panic_when_text_feedback_cleanup_panics`：旧 ID 失效、原始原因保留、关闭不重复 callback |
| P05a | Editor `s06_creation_failures_clear_all_acquired_registries`、policy cleanup panic 用例：Viewer-only panic 与健康 Runtime 分离，继续独立有权限清理 |
| P05b | Editor `s06_core_attach_panic_preserves_live_identity_for_isolated_exit`：Core/Viewer fault、存活身份隔离；Native fixture 断言新 prepare 为 Faulted |
| P07 | Editor `s06_faulted_viewer_only_completes_an_existing_healthy_runtime_submission`、Native fixture：新 Part/Figure 身份，旧 ID 在新 Runtime 失效 |
| C01、C02a、C04、C05a | G5 `late_connection_refusals_and_panic_preserve_existing_projection`：TargetProvider、BindingConstraint、ActivationPanic × 普通连接/self-loop；既有连接和端点保留 |
| C02b、C06 | G5 `invalid_route_output_clears_geometry_and_recovers_without_recreating_resources`、`unresolved_route_keeps_a_live_recoverable_connection_part`：清空无效几何，保留身份/anchor/dependency，后续合法输入恢复 |
| C03 | G5 `failed_connection_activation_rolls_back_all_registrations`：成功前缀清理及 binding/anchor 退休 |
| C05b | G5 `connection_cleanup_lifecycle_panic_preserves_original_error_and_retired_ids`：原始 activation 拒绝保留，Core fault，已退休 Figure/Part/anchor 分别记录，关闭仅一次 callback |
| C07 | G5 `reconnect_preserves_part_figure_order_and_unchanged_anchor`、`dangling_endpoint_faults_before_existing_projection_changes`：既有端点更新保持单项关系/快照预检边界，不进入新建资源清理 |
| O01 | G2 `duplicate_overlay_batch_rejects_before_disposing_the_first_figure`：重复 ID 在首次删除前拒绝 |
| O03、O04 | G2 `overlay_foreign_and_stale_batches_leave_valid_prefix_alive`：foreign/stale 拒绝，合法前缀保留；合法互不重叠批次及空批次；其他 owner 由下行 topology fixture 覆盖 |
| O02、O03 | Editor `s06_overlay_topology_is_checked_before_any_disposal`：祖孙两种排列及含其他 owner 后代的根，合法前缀未删除 |
| O05 | Core `s06_figure_batch_preflight_preserves_topology_damage_and_capability_registration`、Editor `s06_feedback_batch_preflights_both_layers_and_styles`：整批 Figure/style 准入零写入，descriptor 每项只登记一次 |
| O06 | Editor `s06_overlay_installation_panic_reports_the_attached_prefix_without_publishing_owners`：无 Result 的安装生命周期以 attach panic 注入，报告真实存活前缀，不声称健康回滚 |
| O07a | Core `s06_late_disposal_prepare_rejection_keeps_retired_prefix_and_live_suffix` + Editor 真实拒绝分派 + 下述 Overlay 静态复核；按修订 2 接受组合证明 |
| O07b | Editor `s06_overlay_late_lifecycle_panic_records_retired_prefix_and_item`：前项和后项均退休，Core fault、旧 ID 失效、owner 清除、关闭不重复 |
| O08 | 独立 consumer `external_typed_handle_and_tool_use_overlay_batch_admission`：自定义 typed Tool/Handle 获得同一 `InvalidOverlayBatch`，随后合法删除仍成功 |
| F01 | G4 `successful_command_history_survives_projection_failure`：模型命令成功、history/dirty 保留、Viewer fault 且不接受目标 revision |
| F02 | Core `s06_explicit_cleanup_keeps_real_notifications_damage_and_retires_subscriptions`：真实 attach/detach、scoped subscription 退休及旧/新区域 damage |
| F03a | Editor `s06_faulted_viewer_only_completes_an_existing_healthy_runtime_submission`：新驱动拒绝，健康 Runtime 既有 completion 可结清，Viewer revision 不推进 |
| F03b、F03c | `editor.p2-s06-native-exit`：真实 Metal GPU 工作、旧 completion Faulted、有界排空、旧 owner 释放、Snapshot + Full retained texture 基线、迟到 completion MismatchedSession、模型/history 保留及编辑恢复 |

### 三条清理路径的静态复核

Part 创建：`finish_creation` 从基线差集取得资源，保留原始错误及 nested report；
Part/policy/behavior 清理后仅调用一次 `cleanup_created_figures`，
由 `cleanup_attempt` 收集实际领域拒绝，随后按 attached 状态记录身份。
拒绝项不会再次加入一个 retry 队列。

Connection 创建：使用相同 `finish_creation`；独立 Connection Part 不走组合子树查询，
binding removal、anchor removal 和 Figure disposal 分别尝试与记录。
binding/anchor 拒绝保留存活登记，不将依赖失败写成已释放；
后续 Core fault 停止普通 mutation，旧 owner 隔离释放。

Overlay 删除：整批身份与拓扑预检后逐项 disposal；首次错误按整个批次的
attached 状态记录退休前缀和存活后缀，Viewer fault 后普通入口由 `ensure_ready`
拒绝。不恢复 ID、不重新执行已退休项的 lifecycle。

这三项静态复核与 Core/Editor 的动态用例共同构成修订 2 的组合证明。
完整 Editor dirty revision 耗尽集成没有执行，synthetic-root 用例不属于创建资源。

## 验证结果与日志

| 门禁 | 结果 | 日志 |
|---|---|---|
| `editor.p2-s06-failure-isolation` | passed，92 tests | `verification/evidence/p2-s06-failure-isolation-followup.log` |
| `editor.p2-s06-native-exit` | passed，Apple M1 Pro / Metal | `verification/evidence/p2-s06-native-exit-final.log` |
| Native bin Clippy | passed | `verification/evidence/p2-s06-native-clippy.log` |
| `quick` | passed | `verification/evidence/p2-s06-followup-quick.log` |
| 文档门禁、diff | passed | `verification/evidence/p2-s06-closure-docs.log` |
| 最终 `full` | passed | `verification/evidence/p2-s06-closure-full.log` |

之前的 failed/rerun 日志保留于实施证据，不作为 passed 证据。
最终门禁不以通过数量替代矩阵覆盖范围。

## 关闭范围与保留项

最终 full 已通过，本批按用户接受的修订 2 关闭 R06/TC-08。
原审计“prepared publication”候选已在机制必要性复核时收缩为失败作用域清理及
整批输入准入；没有建设全局 detached Plan 或完整 synchronize 回滚。

以下限制永久保留：

- P04a/C05c/O07a 为组合证明，完整 Editor 计数耗尽集成未执行。
- Native 使用真实 GPU retained texture，不证明 WindowServer surface 呈现、
  compositor 或 scanout；surface `Skipped` 仍由 P2-G01 承接。
- Web 异步队列 fault 退出为独立待验收项 `S06-PL02`，见
  [执行总清单](../checklists/audit-remediation-execution.md#web-editor-fault-退出)。
- 外部 callback panic 只覆盖 unwind，不覆盖进程 abort 或任意外部副作用恢复。
- GA-2/GA-3、G01/M01-D 状态不随本批变化。

S06 关闭后进入 P2-S07 Design；本记录不批准 S07 的重大 API 改造进入实施。
