# 语义对标

类型：`documentation-index`

本目录连接外部源码事实与 Novadraw 设计，但不取代两者：

- [`draw2d/api-coverage.md`](draw2d/api-coverage.md)：Draw2D API family、Novadraw 合理变体、覆盖状态与 milestone 映射
- [`draw2d/notification-mapping.md`](draw2d/notification-mapping.md)：Draw2D 通知语义、Zed 借鉴与 Novadraw 映射
- [`gef/api-coverage.md`](gef/api-coverage.md)：GEF 核心编辑语义、Editor 目标契约与 G0-G5 映射

审计按基线查阅：[源码基线](../verification/reference/00-index.md)与
[阶段报告](../verification/reviews/00-index.md)。
当前增量及关闭证据从 [P2 backlog](../roadmap/p2-delta-backlog.md)进入；
早期 Core 差异或 GEF 启动报告不代表当前能力状态。

对标文档必须分别标明“外部事实”“架构推导”“Novadraw 选择”，禁止使用一段混合
描述同时承担三种权威角色。
