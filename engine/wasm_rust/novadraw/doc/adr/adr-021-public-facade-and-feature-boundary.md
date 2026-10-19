# ADR-021：公开 Facade 与 Feature 边界

类型：`architecture-decision`

状态：`superseded`。原决策于 2026-09-28 接受，其公开表面与依赖方案已由后续决策取代。
本页保留决策身份和替代关系，旧正文只供历史追溯。

| 原范围 | 当前决策 |
|---|---|
| 聚合 Core、Editor 与具体后端，转发 backend feature | [ADR-023：六包边界](adr-023-crate-consolidation-and-extension-boundaries.md) |
| 绘图 facade、独立录制与 Runtime 帧出版 | [ADR-031：录制与出版](adr-031-graphics-recording-and-publication-boundary.md) |
| root 高频类型与库级 prelude | [ADR-035：唯一领域路径](adr-035-explicit-domain-imports-and-root-boundary.md) |

按任务分组、保留专业扩展入口、平台依赖显式选择等原则由
[公共 API 契约](../design/architecture/public-api-contract.md)和上述 ADR 承接。
不得继续使用旧 `novadraw::editor`、`prelude` 或 Core backend feature 方案。

原稿及校验值见
[修订快照](../archive/knowledge-revision-2026-10-19/README.md)。
实现与验收状态由 [路线图](../roadmap/00-index.md)和
[P2 backlog](../roadmap/p2-delta-backlog.md)维护。
