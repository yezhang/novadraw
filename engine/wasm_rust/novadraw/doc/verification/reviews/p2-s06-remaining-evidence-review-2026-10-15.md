# P2-S06 剩余验收复核

类型：`verification`

日期：2026-10-15

状态：`accepted`，2026-10-15 用户接受本页验收调整。
R06/TC-08 按修订 2 矩阵和最终关闭门禁判定。

本页补充[实施证据](p2-s06-implementation-evidence-2026-10-15.md)，
其验收调整已纳入[验证矩阵修订 2](../plans/p2-s06-incremental-failure-isolation.md)。
目标契约仍是 [ADR-037](../../adr/adr-037-editor-incremental-failure-isolation.md)。

## prepare 拒绝的可达性

`Runtime::dispose_subtree` 在写入前检查身份、删除关系和 Connection dirty revision。
Part/Connection 创建清理和 Overlay 整批预检已经确定有效的子 Figure；
在没有外部并发写入的这段调用内，剩余的可恢复拒绝来自 dirty revision 耗尽。
`ConnectionRuntime::validate_disposal` 检查全部已登记连接，不只检查待删除子树。
任一连接达到 `u64::MAX` 时，独立 Figure 的 disposal 也会拒绝。
因此，“Runtime 健康时继续独立清理”表示继续尝试有权限的步骤，
不能解释成计数耗尽后一定能成功删除其他 Figure。

计数器没有公共设置入口；现有内部 fixture 仅在 Core 的 `cfg(test)` 下编译。
Editor 使用正常的 Core 依赖，不能调用这个 fixture。
通过公共增量操作到达这个状态需要耗尽 64 位计数器，
不适合作为可执行集成门禁。本轮没有新增公共注入 API、缩小计数器位宽，
也没有新增仅供测试替换 Runtime disposal 的机制。

已补充两项证据：

| 证据 | 覆盖范围 | 不能据此声称的保证 |
|---|---|---|
| Core `s06_late_disposal_prepare_rejection_keeps_retired_prefix_and_live_suffix` | 从 `MAX - 1` 开始，首次 disposal 成功；后项真实 prepare 拒绝；前项 ID 失效，后项和连接仍存活；拒绝不新增 topology/damage 写入，Core 保持健康 | 未通过 Editor 的完整 Part/Connection/Overlay 调用注入计数耗尽 |
| Editor `s06_cleanup_driver_keeps_real_refusal_and_continues_independent_roots` | 使用真实 synthetic-root 拒绝验证既有清理分派；保留原始失败和领域错误；前后两个独立 Figure 各退休一次；关闭不重复 callback | synthetic root 不属于创建调用取得的资源；此测试不等同 P04a/C05c/O07a 的 prepare 拒绝集成 |

Part 与 Connection 的创建清理共用 `finish_creation`、
`cleanup_created_figures` 和 `record_figure_progress`。
后者按实际 attached 状态报告存活与退休，拒绝不会被误报为已退休。
Overlay 删除在首次 disposal 失败后记录整批实际身份、标记 Viewer fault 并停止该批；
后续公开修改受 `ensure_ready` 拒绝，关闭释放 owner，不再次执行 Figure lifecycle。
这些调用关系可静态核验，但静态核验不能被记作完整集成测试。

## Native 宿主队列退出

新增 suite：`editor.p2-s06-native-exit`。
fixture 使用真实 Winit window、VelloRenderer 和 Apple M1 Pro / Metal 队列；
故障通过公开 Figure lifecycle 扩展点注入。

已核验顺序：

1. 执行模型命令，保存提交后的 revision 和 CommandStack history。
2. 将旧 submission 渲染到真实 GPU 的 retained texture，暂不完成 Runtime acknowledgement。
3. Figure attach panic 后 Core/Viewer fault；新编辑和 prepare 拒绝；
   旧 completion 返回 fault，不解锁旧 Runtime。
4. 调用现有 `wait_for_gpu_idle`，在有界等待内排空旧 GPU 队列。
5. 释放旧 Viewer/Runtime，保持原模型与 CommandStack；不重试旧 Figure callback。
6. 新 Runtime 分配新 Part/Figure 身份，以 Snapshot + Full 重建 retained texture。
7. 新 Runtime 拒绝迟到旧 completion，接受新基线 completion 后恢复编辑。

backend gate 对同一 Runtime 的旧 generation 判 stale；
跨 Runtime 的 Snapshot 可以接管 backend。旧 namespace 不能继续生产提交，
由宿主的唯一生产者所有权及退出顺序保证，不能假定 backend 会拒绝所有旧 namespace。

该 suite 证明真实 Native GPU 队列的生产者退出与资源接管。
它不证明 WindowServer drawable 呈现、compositor 或 scanout，也不代表 Web
异步队列退出验收。正常 surface 提交在本环境返回 `Skipped`，失败日志保留为
`doc/verification/evidence/p2-s06-native-exit-r2.log` 至 `-r6.log`。
不得把 retained texture 的 `Presented` 结果记成 surface 验收通过。

最终队列退出日志：
`doc/verification/evidence/p2-s06-native-exit-final.log`。
中间 retained texture 用例曾错误要求 backend 拒绝旧 Runtime namespace 的 Snapshot，
失败日志保留为 `p2-s06-native-exit-retained.log`；已按既有协议修正验收断言。

## 已接受的验收调整

P04a/C05c/O07a 接受“Core 真实计数耗尽用例 + Editor 真实拒绝分派用例 +
三条调用路径的静态复核”作为组合证据，明确保留完整集成注入未执行这一限制。
这会改变已接受矩阵要求的证据方式，不改变 ADR-037 的行为契约，
也不能把组合证据改名成集成测试。

F03b/F03c 以 Native GPU 队列退出作为本批宿主关闭条件。
Web 异步队列退出和本环境 surface `Skipped` 保持独立未验收项，
登记在[执行总清单的平台待验收项](../checklists/audit-remediation-execution.md#web-editor-fault-退出)
及 P2-G01；S06 关闭不代表这两项通过。

按修订 2 完成最终门禁和关闭记录后进入 S07 Design。
完整集成注入未执行的限制永久保留在矩阵和 completion record 中。
