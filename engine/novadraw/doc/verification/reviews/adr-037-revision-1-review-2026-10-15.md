# ADR-037 修订 1 复核

类型：`architecture-review`

日期：2026-10-15

结论：设计复核通过。用户已授权“确认无误后推进到实现阶段”，
据此接受 [ADR-037 修订 1](../../adr/adr-037-editor-incremental-failure-isolation.md)，
P2-S06 进入 Execute。实现与运行验证尚未完成，R06/TC-08 保持开放。

本轮先对照首轮评审与现行 Core/Editor 契约复核，未以本项目 Rust 实现定义目标。
后续实现阶段可按已接受契约审阅相关源码。

| 评审项 | 复核结论 |
|---|---|
| S06-DR01 | §3.1 区分 dispose prepare、engine commit、completion；§3.2 记录 callback 已尝试与完成，旧 ID 失效后不再 dispose |
| S06-DR02 | §2/§6.1 区分健康 Runtime 清理与 Core fault 隔离退出；不解锁 fault、不借普通提交清理全部残留 |
| S06-DR03 | §4 按 binding admission、已提交 resolved/unresolved 和提交失败分类；非法 Router 输出可转稳定 unresolved 并恢复 |
| S06-DR04 | §3.1 分别约束 Part/policy 与 Figure lifecycle；Connection 先于 endpoint/layer 退休，不改正常 removal 的既有顺序 |
| S06-DR05 | §3.2 保留无 Result deactivate；§6.2 区分新帧拒绝、旧 completion 和宿主退出 |

[验证矩阵](../plans/p2-s06-incremental-failure-isolation.md)已分别覆盖上述阶段，
并包含失败后关闭、清理期间 Core fault 和非法 route 恢复用例。
该矩阵说明实施要求，不代表测试已执行。

接受边界：保留 Viewer/Runtime 所有权、三个实施切片和内置/扩展同路径；
不建设具名 Plan/Batch、不承诺完整 synchronize 回滚。
现有 Core fault 退出及宿主接管规则继续有效。
若运行证据证明现有退出路径无法满足资源隔离、释放或禁止重复 callback，
回到 Design 评估该具体缺口，不临时解除 fault 或扩大公共权限。

后续按 Part、Connection、Overlay 顺序补反例测试并实施修复，执行受影响定向 suite，
在实际证据满足矩阵后才能关闭本批。
