# ADR-013: Figure 生命周期与 Runtime 身份域（已替换）

类型：`architecture-decision`

状态：`superseded`，2026-09-10 由
[ADR-014](adr-014-extensibility-and-lifecycle-boundaries.md) 替代。

当前规范：[Figure 生命周期](../design/architecture/figure-lifecycle.md)。

原文仅在[历史快照](../archive/adr-audit-2026-09-10/original/doc/adr/adr-013-figure-lifecycle-and-runtime-identity.md.txt)
保留，不再作为当前编码依据。撤回的是通用 owned detach、任意引用自动重映射及其
生命周期原子性承诺；namespace 隔离、可靠 dispose 和旧区域 damage 的目标仍保留。

替代原因、Draw2D/GEF 证据和实现门禁见
[审计记录](../verification/reviews/adr-audit-2026-09-10.md)。
