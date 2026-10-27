# Editor / GEF 路线图

类型：`roadmap`

本页是 G0–G5 编号、状态和人工检查点的唯一入口。Core 使用 M1–M10；
后续 Editor 能力使用 [P2 backlog](../p2-delta-backlog.md)，不重开既有里程碑。
原 G6 产品 schema、serializer、保存加载及产品级 Native/Web 场景由独立产品包负责。

## 里程碑

| 顺序 | 阶段 | 状态 | 毕业范围 | 证据 |
|---:|---|---|---|---|
| 0 | G0 架构与工程启动 | `complete` | 独立 Editor、身份/所有权、语义账本、crate 与文档门禁 | [ADR-015](../../adr/adr-015-editor-framework-boundary.md)、[启动核验](../../verification/reviews/gef-readiness-analysis-2026-09-13.md) |
| 1 | G1 Model Adapter / CommandStack | `complete` | model identity/revision、execute/undo/redo、compound、dirty/save 与 fault | [G1 完成](../../verification/reviews/g1-model-command-completion-2026-09-13.md) |
| 2 | G2 EditPart / Viewer 投影 | `complete` | factory、生命周期、registry、contents/root、增量 containment | [G2 完成](../../verification/reviews/g2-viewer-projection-completion-2026-09-13.md) |
| 3 | G3 Selection / Targeting / 输入仲裁 | `complete` | 多选/primary/focus、visual targeting、Figure/Editor 消费与 capture；检查点 A PASS | [G3 核验](../../verification/reviews/g3-selection-targeting-behavior-2026-09-13.md)、[人工手册](../../verification/manual/g3-selection-targeting.md) |
| 4 | G4 Tool / Request / Policy | `complete` | create/move/resize/delete、feedback、undo/redo；检查点 B PASS | [G4 核验](../../verification/reviews/g4-editing-loop-behavior-2026-09-14.md)、[人工手册](../../verification/manual/g4-editing-loop.md) |
| 5 | G5 Connection / Viewport | `complete` | create/reconnect/bendpoint、auto-expose、scroll/zoom 反馈；检查点 C PASS | [连接与视口设计](../../design/editor/00-index.md)、下表阶段证据 |
| 6 | G6 下游产品毕业 | `external` | 独立产品包的持久化、业务 schema 和跨平台应用 | 不属于引擎门禁 |

`complete` 表示对应基线的引擎契约、自动验证、要求的人工检查点和文档已闭合，
不表示后续全部语义或平台能力已交付。P2 状态只在 backlog 维护。

## G5 阶段证据

| 顺序 | 阶段 | 固定边界与核验 |
|---:|---|---|
| 1 | G5.1 Connection Projection | source/target 双向发现、单 Part 去重、关系和 binding 生命周期；[核验](../../verification/reviews/g5-connection-projection-design-2026-09-14.md) |
| 2 | G5.2 Creation | 两阶段请求、反馈、Command 与取消；[核验](../../verification/reviews/g5-connection-creation-behavior-2026-09-14.md) |
| 3 | G5.3 Reconnect | 两个独立 endpoint handle、身份保持、重连事务；[核验](../../verification/reviews/g5-connection-reconnect-behavior-2026-09-15.md) |
| 4 | G5.4 Bendpoint | 模型约束、create/move/delete、自环；[核验](../../verification/reviews/g5-connection-bendpoint-behavior-2026-09-15.md) |
| 5 | G5.5 Viewport / Auto-expose | 同一坐标真源、双轴滚动、固定手势目标与反馈；[核验](../../verification/reviews/g5-viewport-autoexpose-behavior-2026-09-15.md) |

## 当前开发入口

规范从 [Editor 设计](../../design/editor/00-index.md)进入，外部事实见
[GEF 分析](../../reference/gef/00-index.md)，采用关系见
[GEF 语义账本](../../parity/gef/api-coverage.md)。
模型写入与 Viewer 驱动、开放 Tool/Request/Handle、增量失败隔离分别由
[ADR-032](../../adr/adr-032-viewer-edit-session-and-drive-boundary.md)、
[ADR-034](../../adr/adr-034-editor-tool-request-handle-protocol.md)和
[ADR-037](../../adr/adr-037-editor-incremental-failure-isolation.md)细化。

Direct text edit/IME 的回归范围见
[设计](../../design/editor/p2-direct-text-edit.md)与
[矩阵](../../verification/plans/p2-text-direct-edit.md)。
后续失败、输入不变量和服务整改顺序只在
[P2 backlog](../p2-delta-backlog.md#当前推进顺序2026-10-14-校准)维护。

## 下游产品与变更规则

`node-editor-demo` 是引擎验证入口。产品可消费模型加载、选择、节点编辑、连接编辑、
undo/redo 和视口能力；业务持久化不得反向定义 Runtime 或 Command 所有权。
产品验证发现的通用缺口以独立 delta 回流。

新增能力先更新 GEF family；涉及身份、Command 原子性、输入仲裁或模型所有权时
先评审契约。Draw2D Core 缺口进入 P2，不重开 M1–M10。
可执行验证见 [suite 入口](../../../tests/README.md)，不在本页复制命令定义。
crate 拆分判据见 [目录与发布边界](../../design/architecture/directory-structure.md#editor-拆分判据)。
