# 通知与订阅生命周期

类型：`normative-design`

能力归属：Core Runtime；Inspector／Editor 只消费观察结果。
规范效力：沿用 2026-09-08 原编号 010、2026-09-10 生命周期修订和
2026-10-30 服务／订阅修订 3；实现状态见 [P2 backlog](../../roadmap/p2-delta-backlog.md)。

## 所有权与身份

Runtime 统一存储 Update、Figure、Coordinate、Ancestor、Property、Action、Layout
七类 listener。注册时显式声明 Runtime 或 Figure(owner) scope；删除 owner 自动解除
所属订阅。不能根据 closure 捕获或 payload 过滤推断归属，也不复制每节点订阅列表。
ListenerId 携带 Runtime namespace 与本地身份，不使用进程全局分配器。

注册失败、foreign ID、重复注销、身份耗尽和 fault 后的无 callback 注销，以
[服务失败与订阅恢复](service-failure-and-inspector-lifecycle.md#adr-010-的局部替代关系)
及该专题的修订 3 合同为准。原稿中的不可恢复 ID 耗尽和旧布尔返回值不再定义当前接口。

## 分发与自注销

提交后冻结 NotificationEffect 快照，按 effect FIFO、同类 listener 注册顺序分发。
callback 返回 Keep 或 Remove。Remove 让当前 callback 完整执行，且不跳过同一 effect
上的其他 listener；当前 listener 在此 effect 后移除，不再收到本次 flush 的后续 effect。
UpdateListener 的主回调及 ValidatingListener adapter 必须完成当前 event 后才移除。
自注销不制造额外通知，也不开放 callback 中按 ID 注销其他 listener 的重入接口。

观察 callback 不接收可变 Runtime、FigureTree 或 UpdateManager。
场景写入使用输入／组件更新的受限 effects，仍由 Runtime 提交。
不用共享原子取消 token 或全局取消表引入额外并发与生命周期语义。

## 历史事实与稳定查询

每条 effect 保存发生时的 revision／epoch、sequence 和必要 old/new 值。
callback 查询的是 flush 时最新 stable scene；它不是历史事件发生时的完整树。
Validating／Painting 是过去阶段的事实，不是事前拦截 hook。
未稳定时的失败通过诊断通道报告，不能依赖成功通知。

callback panic 沿 Runtime fault 边界传播；服务／Inspector 本地故障的隔离和恢复
由服务专题负责，不承诺外部副作用回滚或未送达 journal 自动重放。

## 验证与取舍

验证七类注册、scope 清理、foreign 身份、重复注销、当前 effect 完整、后续 effect
停止、注册顺序以及历史 payload 与最新查询的区分。身份耗尽、结构化 source 和
fault 后注销使用服务专题的验证矩阵，不沿用旧稿的过期预期。
该协议增加显式生命周期与返回值处理，换取确定性分发和无重入观察。

原批准与局部替代前的判断见
[原稿 010](../../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-010-runtime-listener-lifecycle.md.txt)。
长期提交与生命周期边界见 [ADR-014](../../adr/adr-014-extensibility-and-lifecycle-boundaries.md)。
