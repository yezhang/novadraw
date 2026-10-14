# P2-S06 机制必要性复核

类型：`architecture-review`

日期：2026-10-14

基线：`ec415b2`。本次仅复核源码、运行定向探针并调整任务范围，不修改引擎实现。
本报告记录证据和取舍，执行状态由 [P2 backlog](../../roadmap/p2-delta-backlog.md) 维护。

## 1. 结论

撤销必须建设 `PartProjectionPlan`、`ConnectionProjectionPlan`、`OverlayBatch` 的要求。
三个名字描述了一种解法，不能作为缺陷或毕业条件。需要的演进是：
单次新建 Part 子树失败隔离、Connection 晚期失败清理覆盖、Overlay 完整批量预检，
以及 callback/清理失败与模型/history 的明确边界。

现有所有者仍为 Viewer 与 Runtime。内部候选数据或操作日志只有在定向失败用例证明
现有机制无法满足契约时才设计；不预先增加公共 Plan、lease 或第二套 registry。

## 2. 实际路径与运行证据

| 路径 | 已有机制 | 证据与缺口 | 所需动作 |
|---|---|---|---|
| Part | factory、policy 构造；Figure 挂载；配置；Part/registry 登记；refresh/activate；递归创建 | `create_subtree_inner` 后续子节点失败没有统一撤销新建子树；临时外部探针确认 child6 factory 拒绝后 child5 仍 attached，PartTree/model registry 仍含 child5，Viewer faulted | 明确单次新建子树的所有权范围，集中清理本次新增登记与成功激活前缀；验证拒绝和清理失败 |
| Connection | bendpoint/routing 预检；anchor 注册失败补偿；初始化失败调用 `remove_connection_part` | 既有 activation 拒绝测试验证连接层及 Part/model 登记清除；尚不能由该测试推断所有晚期失败、anchor 泄漏或完整同步原子性 | 复用清理路径，补齐 route/binding/activation 晚期拒绝及清理失败证据，再决定是否需要内部候选 |
| Overlay | 现有 add/remove batch、owner registry、失败后移除成功前缀 | 临时外部探针确认 `[figure, figure]` 全部通过预检，第一次 dispose 后第二次报错；Figure 和 owner 已移除，Viewer faulted | 在写入前处理重复 ID、祖孙重叠、foreign/stale/owner；补齐创建/style 拒绝和清理失败报告 |

源码入口：
[Part/Connection 创建](../../../novadraw-editor/src/viewer/projection.rs)、
[Overlay 批量操作](../../../novadraw-editor/src/viewer/mod.rs)。
探针复用现有 integration fixture，复制到系统临时目录中的独立 consumer crate，
追加两个观察当前行为的测试；两个探针均通过，确认上述缺口，不代表整改通过。
探针采用独立解析的依赖锁，不作为性能或完整回归证据。临时文件不进入仓库。

Connection 运行验证采用工作区原有锁文件：
`failed_connection_activation_rolls_back_all_registrations` 定向测试通过（1 passed），
仅证明该测试断言的范围。后续正式回归分别归
`g2.viewer-projection`、`g3.selection-targeting`、`g5.1.connection-projection`；
新增整改用例需同步现有 suite，不把临时探针当作长期门禁。

## 3. 为什么不预设 Plan 或 Drop 回滚

`configure_visual` 的 VisualBuildContext 操作已挂载 Runtime；
refresh 使用 VisualUpdateContext 与实际 FigureId；activation 发生在 Part 登记之后。
将其全部移到 detached Plan 中需要重新定义 callback 权限、身份分配和激活可见性。
仅新增一个包装类型不会消除已发生的写入，也不会让 callback 自动具备回滚能力。

Overlay 的 Vec 是已发布资源身份集合，而非独立所有者缺失的证明。
Runtime dispose 可失败并触发 lifecycle；因此 Drop 无法保证不可失败的 Runtime 撤销。
未发布 owned 候选可正常释放，已发布资源需由 Viewer 显式清理并报告失败。
现有 batch 函数可以先修正输入预检，无需为重复 ID 缺口新建类型。

GEF 的 `AbstractEditPart.addChild` 按 child 登记、parent 设置、visual 添加、
addNotify、activate、child-added 顺序执行，不提供这里预设的三类 Plan 或通用事务承诺。
参考范围为 `org.eclipse.gef`；不使用 Zest。GEF 的增量生命周期语义支持保留既有
factory/activation 扩展点，但不能替代 Rust 实现自身的失败保证。

## 4. 修订后的验收边界

可恢复拒绝时，单次新建子树不遗留其新增 Figure、Part、registry 与 policy/behavior 登记。
成功激活前缀只 deactivate 一次。完整 synchronize 中既有刷新、退役或先前成功操作
不承诺恢复到调用前快照；通知、damage 和身份分配也不能因资源最终清除就宣称无变化。
设计阶段必须分别定义并验证这些状态，不能把“无资源残留”等同于“从未发布”。

外部 callback panic、外部副作用及清理失败进入明确 fault 边界，不承诺完整回滚。
已成功模型命令与 history 保留为事实；视觉投影失败应单独报告并保持明确的重建路径。
本次不关闭 TC-08/R06，不进入实现，也不为新概念预占 ADR。
