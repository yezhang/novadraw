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
| 8 | [viewer-edit-session-and-drive.md](viewer-edit-session-and-drive.md) | 模型编辑会话、CommandStack 与宿主权限 |
| 9 | [tool-request-handle-protocol.md](tool-request-handle-protocol.md) | typed 请求/策略/工具/Handle 的注册与失败 |
| 10 | [template-creation-and-marquee.md](template-creation-and-marquee.md) | 模板、拖放、创建预览与框选 |
| 11 | [container-editing-protocol.md](container-editing-protocol.md) | 容器资格、父域映射、受检计划与事务 |
| 12 | [incremental-failure-isolation.md](incremental-failure-isolation.md) | Part/Connection/Overlay 的失败清理与恢复 |

机制分析（解释现行契约，不替代规范）：

- [容器编辑：容器编辑与父坐标协议](container-editing-protocol.md)：
  公开容器移动、父域创建/drop、统一资格、XY/Ordered 消费和受检事务；
  [实施验证](../../verification/reviews/adr-055-implementation-2026-10-29.md)列出平台剩余范围。
- [类型化模板创建与框选](template-creation-and-marquee.md)：
  创建工具与模板拖放共享的开放请求、框选查询、应用保存点和连接装饰所有权。
- [revision 的作用与边界](revision-semantics.md)：模型发布、Viewer 同步进度、交互版本与撤销历史。

已批准分批实施的方案（不覆盖现行规范，完成证据见 P2 backlog）：

- [GEF 官方样例对等实现](gef-official-examples-parity-proposal.md)：样例能力分层、
  Masonry 自定义 Canvas、可选 Xilem 外层与实施切片。
- [Masonry 编辑画布与 Xilem View 接入](masonry-xilem-canvas-integration.md)：
  固定上游依赖、session 所有权、Widget 输入/绘制与 View rebuild。
- [GEF Shapes 样例工作流层级](gef-editor-demo-workflow-hierarchy.md)：固定三栏内的
  文档栏、工具轨、上下文栏、Layers / Assets、Inspector 和状态栏；P0–P2 行为与所有权。
- [Logic 首个可运行切片](gef-logic-first-slice.md)：无环组合电路、typed 端口、
  信号派生与独立 egui/Vello 消费者；不关闭完整 Logic 对等验收。

延期的应用外壳候选（不进入当前执行队列）：

- [egui Docking 延期方案](egui-docking-deferred-plan.md)：固定三栏继续生效；
  记录重新评审的触发条件、候选版本、状态所有权和验收门禁。

Editor／GEF 框架能力补充（分批评审与实施）：

- [开放吸附校验成本整改提案](snapping-validation-cost-proposal.md)：E04 正式采样后的
  私有流式校验、容量复用与减少完整扫描的证明条件；待评审，不授权 Core 改动。
- [Editor 开放吸附：Editor 开放吸附协议](open-snapping.md)：
  修订 2 已接受，E04-A 实施中；[专题](open-snapping.md)定义 typed 策略、受检特征、
  自由度与反馈／提交，退出条件见[矩阵](../../verification/plans/editor-open-snapping.md)。
  [依赖与评审结论](../../verification/reviews/open-snapping-review-status.md)
  区分 E03 来源取消、有界重算、Core 依赖和 Editor 新增；
  [协议实施记录](../../verification/reviews/p2-e04-a-implementation-2026-11-02.md)列出已通过与待验收范围。
  逐轮回应退出当前阅读路径，原稿从专题状态的归档入口追溯。
- [Editor／GEF 框架能力补充方案](core-capability-increment.md)：容器、吸附、键盘、
  多视图和可选编辑策略的分批范围。
- [容器编辑：容器编辑与父坐标协议](container-editing-protocol.md)：
  首批开放计划、父域映射、换父事务与身份恢复；
  [验证矩阵](../../verification/plans/editor-container-editing.md)。

已接受的失败隔离契约：

- [Editor 增量失败隔离：增量视图失败隔离与 Overlay 批量校验](incremental-failure-isolation.md)；
- [P2-S06 验证矩阵](../../verification/plans/p2-s06-incremental-failure-isolation.md)。

相关入口：

- [ADR-061 统一会话协调与专用编辑控制器](../../adr/adr-061-edit-session-and-specialized-controller-boundary.md)：
  已接受并实施公开接入；[具体协议](specialized-edit-controller.md)与
  [验证矩阵](../../verification/plans/specialized-edit-controller.md)分别维护行为和验收范围。
- [富文本正文反馈与段落布局复用](richtext-incremental-feedback.md)：
  正文等价证明、只读候选几何、稳定 caret 发布、段落资产与高度索引的复用条件。

- GEF 外部事实：[`../../reference/gef/`](../../reference/gef/00-index.md)
- 语义采用账本：[`../../parity/gef/api-coverage.md`](../../parity/gef/api-coverage.md)
- 架构决策：[`../../adr/adr-015-editor-framework-boundary.md`](../../adr/adr-015-editor-framework-boundary.md)
- 实施状态：[`../../roadmap/editor/00-index.md`](../../roadmap/editor/00-index.md)
- 验证记录：[`../../verification/reviews/00-index.md`](../../verification/reviews/00-index.md)
