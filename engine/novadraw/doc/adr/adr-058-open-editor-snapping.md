# ADR-058：Editor 开放吸附协议

类型：`architecture-decision`

能力归属：Editor（`novadraw-editor`），对应 GEF 开放吸附协议。

状态：草案，待评审；2026-10-29。对应 P2-E04，尚未批准实施。
依赖 [ADR-055](adr-055-container-editing-protocol.md)的父域映射和容器事务。
专题见 [开放吸附设计](../design/editor/open-snapping.md)。

## 分层归属

| 层／owner | 本方案职责 | 改动与验收边界 |
|---|---|---|
| Editor | 策略注册、会话、候选、仲裁、版本失效、Tool 与 Overlay 编排 | 新增吸附协议与编辑行为；独立消费者证明 GEF 框架能力 |
| Core | Figure、命中、坐标、Runtime 与绘制基础协议 | 消费既有 API；不新增 Core 吸附服务，所需 Core delta 另列 |
| 平台／宿主 | 原生输入、DPI、capture 与取消事件 | 验证真实输入进入 Editor 的链路 |
| 应用 | Grid／Guide 资源、业务资格、Command 与 attachment 持久化 | 消费者责任；应用功能不计为框架新增能力 |

## 目标与决策

为移动、换父、嵌套创建／drop 和 resize 提供可组合 Grid、Geometry、Guide 策略。
Shapes 的网格量化不作为框架能力；网格显示与吸附启用保持独立。

采用 typed identity、payload trait 与 registry。内置和第三方策略注册、
候选计算、错误传播、反馈与成本预算完全相同，不使用闭合 SnapKind 分派。
Viewer 拥有注册表与手势会话；应用拥有网格／guide 配置及业务模型命令。
Core 继续只提供受检坐标、Figure 与 Overlay 基础协议。

手势先解析目标容器，再基于未吸附几何计算候选。移动沿用唯一锚点，
全批次共享同一个 surface 平移，不能给每个成员独立量化。
候选记录策略来源、对齐特征、修正、版本和反馈事实；受检结果同时服务预览和提交。
释放时核对模型、视口、目标和策略版本，失效则重新求解和校验资格。

阈值使用宿主逻辑像素，不随 DPI 增大；模型网格先从目标 content 域映射到 surface，
在 surface 比较修正距离。不可逆、无路径或溢出按现有坐标错误拒绝，
不降级成未吸附成功。resize 只改变允许边，固定边与最小尺寸先于候选发布检查。

按显式策略顺序仲裁每轴；同策略候选先比较距离，再按稳定特征身份排序。
有耦合约束的候选整体接受或拒绝，不能拼接其一半修正。
不重复累加已吸附结果，不在反馈层另算一套位置。
policy 只校验和解释受检几何；如果布局强制改写位置，该候选需失效并重算反馈。

## 对标与合理变体

参考 GEF Classic `4463d9d0ce13c19d10fbe769d29f28b7345a8cba`：

| 源码／方法 | 采用的语义 | Rust 变体 |
|---|---|---|
| `SnapToHelper.snapRectangle` | absolute 几何、分轴修正、剩余方向 | 受检 surface 候选与显式轴约束 |
| `SnapToGrid.snapRectangle` | container content 域网格、显示／启用分离 | 浮点连续几何，不复制整数 right−1 规则 |
| `SnapToGeometry.populateRowsAndCols` | sibling 边／中心、排除操作集合、复用候选 | 排除源子树；按模型／视口版本维护快照 |
| `SnapToGuides.snapRectangle` | guide 对齐与 attachment 事实 | typed guide 身份；业务 attachment 持久化仍由 Command |
| `DragEditPartsTracker.setTargetEditPart` | 目标改变重建 helper | 目标／策略／模型／视口任一版本改变失效 |
| `DragEditPartsTracker.repairStartLocation` | auto-expose 修复起点 | 复用 ADR-055 的当前坐标映射 |

Java 只读，不使用 Zest。`api_semantics`：GEF `snap.guides`、
`tool.tracker`、`feedback.protocol`、`interaction.change_bounds`，
以及 Draw2D `coordinate.conversion`。family 使用现行语义账本的规范名称。

## 外部消费者证据

独立 crate 自定义带业务 payload 的策略，与 Grid、Geometry、Guide 共用注册表。
验证替换顺序、失败 source、混合父多选、不同 content pane、zoom／scroll、
无 pointer move 的版本变化和一次 undo／redo。
另一消费者提供耦合候选，证明仲裁不会拆解第三方约束。
不修改 Core／Editor 的 match，也不增加内置专用 hook。

## 成本与退出

Geometry 在候选集合变化时构建索引，pointer 更新只查询阈值邻域。
显式记录全量重建、查询扫描量、每次事件分配、反馈批次与 P95；
第三方策略按相同记录规则消费预算，不用特权快路径。
新增依赖、默认 feature、字体与图片预期均为零；
是否需要单独可选 feature 由同配置可达产物测量决定。

设计评审后才分片实施：纯协议／仲裁 → 内置策略与外部消费者 →
Tool／Overlay 集成 → Native／Web 输入与成本。
用例见 [验证矩阵](../verification/plans/editor-open-snapping.md)。
本决策不关闭 E03 的平台和 revision 耗尽验收，也不启动 E04 Execute。
