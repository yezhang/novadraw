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
| 7 | [p2-direct-text-edit.md](p2-direct-text-edit.md) | TextFlow 交互几何、直接编辑与 IME |

机制分析（解释现行契约，不替代规范）：

- [revision 的作用与边界](revision-semantics.md)：模型发布、Viewer 同步进度、交互版本与撤销历史。

已批准分批实施的方案（不覆盖现行规范，完成证据见 P2 backlog）：

- [GEF 官方样例对等实现](gef-official-examples-parity-proposal.md)：样例能力分层、
  Masonry 自定义 Canvas、可选 Xilem 外层与实施切片。

已接受的失败隔离契约：

- [ADR-037：增量视图失败隔离与 Overlay 批量校验](../../adr/adr-037-editor-incremental-failure-isolation.md)；
- [P2-S06 验证矩阵](../../verification/plans/p2-s06-incremental-failure-isolation.md)。

相关入口：

- GEF 外部事实：[`../../reference/gef/`](../../reference/gef/00-index.md)
- 语义采用账本：[`../../parity/gef/api-coverage.md`](../../parity/gef/api-coverage.md)
- 架构决策：[`../../adr/adr-015-editor-framework-boundary.md`](../../adr/adr-015-editor-framework-boundary.md)
- 实施状态：[`../../roadmap/editor/00-index.md`](../../roadmap/editor/00-index.md)
- 验证记录：[`../../verification/reviews/00-index.md`](../../verification/reviews/00-index.md)
