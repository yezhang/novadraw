# 语义对标

类型：`parity-contract`

本目录连接外部源码事实与 Novadraw 设计，但不取代两者：

- [`draw2d/api-coverage.md`](draw2d/api-coverage.md)：Draw2D API family、Novadraw 合理变体、覆盖状态与 milestone 映射
- [`draw2d/notification-mapping.md`](draw2d/notification-mapping.md)：Draw2D 通知语义、Zed 借鉴与 Novadraw 映射
- [`gef/api-coverage.md`](gef/api-coverage.md)：GEF 核心编辑语义、Editor 目标契约与 G0-G5 映射

最新实现差异审计见
[`../verification/reviews/draw2d-core-capability-audit-2026-09-08.md`](../verification/reviews/draw2d-core-capability-audit-2026-09-08.md)。
GEF 启动分析见
[`../verification/reviews/gef-readiness-analysis-2026-09-13.md`](../verification/reviews/gef-readiness-analysis-2026-09-13.md)。

对标文档必须分别标明“外部事实”“架构推导”“Novadraw 选择”，禁止使用一段混合
描述同时承担三种权威角色。
