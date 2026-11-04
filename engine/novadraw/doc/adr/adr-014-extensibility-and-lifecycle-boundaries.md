# ADR-014：分层原子性、生命周期与稳定发布

类型：`architecture-decision`

能力归属：Core 提交、生命周期和稳定发布；Host 负责明确恢复。

状态：已接受，2026-09-10；替代原编号 013，沿用既有局部修订。
本次压缩归并仅调整知识表达，不新增架构授权；实现状态以 roadmap／verification 为准。

## 问题

运行期写入、自定义组件生命周期、派生计算和外部观察不能共享一个含糊的
“事务”承诺；任意 Rust 用户代码也无法自动回滚。

## 决策

Runtime 统一预检并提交源操作；扩展只准备 typed 候选与受限 effects。
源操作、完整派生输出、稳定场景发布、backend 确认分别定义原子性。
失败不发布部分候选，也不回滚已提交 FIFO 前缀或任意外部副作用。

内部失效和必需生命周期行为先完成，派生工作闭合后才发布稳定场景及外部观察日志。
一般扩展 panic 进入 Runtime fault 边界，不以恢复 phase guard 冒充完整恢复。
dispose 使身份失效，同 Runtime reparent 保持身份。
跨 Runtime 默认从模型／描述重建；整体转交则 move 完整 Runtime。

## 替代与代价

任意原地修改闭包、自动提取活子树和全局回滚无法证明引用、资源与析构安全，
因此不提供通用活对象迁移承诺。观察 listener 也不能充当事前拦截。
代价是候选计算、生命周期清理和独立恢复需要显式协议；不承诺 panic=abort、
二次析构 panic 或外部副作用恢复。

## 关联设计

[组件更新](../design/architecture/component-update.md)、
[生命周期](../design/architecture/figure-lifecycle.md)、
[收敛](../design/architecture/derived-state-convergence.md)、
[通知订阅](../design/architecture/notification-subscription-lifecycle.md)、
[资源与 session](../design/architecture/resource-lifecycle.md)。
历史裁决见 [审计](../verification/reviews/adr-audit-2026-09-10.md)。

原批准范围和修订依据见[原稿 014](../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-014-extensibility-and-lifecycle-boundaries.md.txt)；
其他原编号与当前入口见[归并记录](../archive/adr-consolidation-2026-11-03.md)。
