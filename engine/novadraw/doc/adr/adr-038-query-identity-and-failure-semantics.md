# ADR-038：身份、合法缺失与领域失败分离

类型：`architecture-decision`

能力归属：Core 身份与领域查询；Editor／Inspector 消费失败。

状态：已接受，修订 1，2026-10-16。
本次压缩归并仅调整知识表达，不新增架构授权；实现状态以 roadmap／verification 为准。

## 问题

Option、默认值和通用错误不能同时表达错误身份、合法缺失与计算失败，
否则失效引用和非法结果可能进入成功缓存。

## 决策

先检查 owner 与 live identity，再解释查询值。合法缺失与领域失败分离；
已验证短借用视图与任意 ID 查询分开。坐标、测量、资源和扩展保留领域错误及 source。
候选发布前完整检查输入／输出；不能把奇异变换、非有限几何、扩展失败或预算错误
变成零值、缺失或普通缓存 miss。受检构造不能由序列化和 attached 入口绕过。

## 替代与代价

保持便利 getter 或用全局错误擦除原因可减少调用处代码，
却无法让消费者区分恢复、合法空值与真实失败。该选择增加 Result 处理、
复合错误和负面用例，换取可验证的失败传播和发布一致性。

## 关联设计

[查询与测量](../design/architecture/query-identity-and-failure.md)、
[基础值](../design/architecture/engine-value-contract.md)、
[输入不变量](../design/architecture/detached-input-invariants.md)、
[服务与订阅失败](../design/architecture/service-failure-and-inspector-lifecycle.md)。

原批准范围和修订依据见[原稿 038](../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-038-query-identity-and-failure-semantics.md.txt)；
其他原编号与当前入口见[归并记录](../archive/adr-consolidation-2026-11-03.md)。
