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

- [ADR-055：容器编辑与父坐标协议](../../adr/adr-055-container-editing-protocol.md)：
  公开容器移动、父域创建/drop、统一资格、XY/Ordered 消费和受检事务；
  [实施验证](../../verification/reviews/adr-055-implementation-2026-10-29.md)列出平台剩余范围。
- [ADR-054：类型化模板创建与框选](../../adr/adr-054-template-creation-and-marquee.md)：
  创建工具与模板拖放共享的开放请求、框选查询、应用保存点和连接装饰所有权。
- [revision 的作用与边界](revision-semantics.md)：模型发布、Viewer 同步进度、交互版本与撤销历史。

已批准分批实施的方案（不覆盖现行规范，完成证据见 P2 backlog）：

- [GEF 官方样例对等实现](gef-official-examples-parity-proposal.md)：样例能力分层、
  Masonry 自定义 Canvas、可选 Xilem 外层与实施切片。
- [Masonry 编辑画布与 Xilem View 接入](masonry-xilem-canvas-integration.md)：
  固定上游依赖、session 所有权、Widget 输入/绘制与 View rebuild。

延期的应用外壳候选（不进入当前执行队列）：

- [egui Docking 延期方案](egui-docking-deferred-plan.md)：固定三栏继续生效；
  记录重新评审的触发条件、候选版本、状态所有权和验收门禁。

核心能力补充（分批评审与实施）：

- [Editor 核心能力补充方案](core-capability-increment.md)：容器、吸附、键盘、
  多视图和可选编辑策略的分批范围。
- [ADR-055：容器编辑与父坐标协议](../../adr/adr-055-container-editing-protocol.md)：
  首批开放计划、父域映射、换父事务与身份恢复；
  [验证矩阵](../../verification/plans/editor-container-editing.md)。

已接受的失败隔离契约：

- [ADR-037：增量视图失败隔离与 Overlay 批量校验](../../adr/adr-037-editor-incremental-failure-isolation.md)；
- [P2-S06 验证矩阵](../../verification/plans/p2-s06-incremental-failure-isolation.md)。

相关入口：

- GEF 外部事实：[`../../reference/gef/`](../../reference/gef/00-index.md)
- 语义采用账本：[`../../parity/gef/api-coverage.md`](../../parity/gef/api-coverage.md)
- 架构决策：[`../../adr/adr-015-editor-framework-boundary.md`](../../adr/adr-015-editor-framework-boundary.md)
- 实施状态：[`../../roadmap/editor/00-index.md`](../../roadmap/editor/00-index.md)
- 验证记录：[`../../verification/reviews/00-index.md`](../../verification/reviews/00-index.md)
