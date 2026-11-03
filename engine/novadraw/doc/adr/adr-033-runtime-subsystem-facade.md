# ADR-033：Runtime 子系统 Facade 与统一 Figure Mutation

类型：`architecture-decision`

能力归属：Core Runtime；Editor 仅投影宿主权限。

状态：已接受，2026-10-12。

## 问题

Runtime 平铺全部 API 会持续膨胀；若拆成分别持有状态的 manager，又会产生第二套
提交权威。内置 typed setter 与外部 component update 并存也会形成特权路径。

## 决策

Runtime 保留组合根与状态所有权；子 facade 是无状态、短借用的调用窗口。
内置与外部 Figure 共用 typed identity、payload、registry 和受检 mutation。
内部 primitive 不构成第二套公开 API；ViewerDrive 只暴露宿主受限视图。

## 替代与代价

不建立独立状态 manager、不保留内置 fast path，也不无限增加 Runtime 顶层方法。
收口需要迁移旧调用，但提交权威、失效与错误语义只有一份。

## 关联设计

[Facade 协议](../design/architecture/runtime-subsystem-facade.md)维护领域表与 mutation；
[公共 API](../design/architecture/public-api-contract.md)维护领域导航。
