# Editor Architecture

类型：`documentation-index`

本目录定义独立 Editor 框架的规范契约。总体所有权、身份和编辑事务以
[architecture.md](architecture.md) 为准；G5 专题按执行顺序细化连接编辑与
Viewport 协作。

| 顺序 | 文档 | 范围 |
|---:|---|---|
| 1 | [architecture.md](architecture.md) | Model、Command、EditPart、Viewer、Tool、Request 与 Policy |
| 2 | [g5-connection-projection.md](g5-connection-projection.md) | 连接关系发现、身份和投影 |
| 3 | [g5-connection-creation.md](g5-connection-creation.md) | 两阶段连接创建 |
| 4 | [g5-connection-reconnect.md](g5-connection-reconnect.md) | 端点重连与事务边界 |
| 5 | [g5-connection-bendpoint.md](g5-connection-bendpoint.md) | 折点约束与 self-loop |
| 6 | [g5-viewport-autoexpose.md](g5-viewport-autoexpose.md) | 缩放反馈与双轴 auto-expose |

相关入口：

- GEF 外部事实：[`../../reference/gef/`](../../reference/gef/00-index.md)
- 语义采用账本：[`../../parity/gef/api-coverage.md`](../../parity/gef/api-coverage.md)
- 架构决策：[`../../adr/adr-015-editor-framework-boundary.md`](../../adr/adr-015-editor-framework-boundary.md)
- 实施状态：[`../../roadmap/editor/00-index.md`](../../roadmap/editor/00-index.md)
- 验证记录：[`../../verification/reviews/00-index.md`](../../verification/reviews/00-index.md)
