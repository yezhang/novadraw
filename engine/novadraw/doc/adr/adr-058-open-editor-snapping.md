# ADR-058：Editor 开放吸附协议

类型：`architecture-decision`

能力归属：Editor（`novadraw-editor`），对应 GEF 开放吸附协议。

状态：修订 2 已接受；2026-11-02。用户授权在复核通过后推进，P2-E04-A 进入实施。
依赖 [ADR-055](adr-055-container-editing-protocol.md)的父域映射和容器事务。
专题见 [开放吸附设计](../design/editor/open-snapping.md)。
本次修改意见见[修订 1 评审记录](../verification/reviews/adr-058-revision-1-review-2026-11-02.md)；
逐项修改见[修订回应](../verification/reviews/adr-058-revision-2-response-2026-11-02.md)，
原评审记录保持不变。[修订 2 复核](../verification/reviews/adr-058-revision-2-review-2026-11-02.md)
复核通过，用户的条件授权已满足。

## 分层归属

| 层／owner | 本方案职责 | 改动与验收边界 |
|---|---|---|
| Editor | 策略注册、会话、候选、仲裁、版本失效、Tool 与 Overlay 编排 | 新增吸附协议与编辑行为；独立消费者证明 GEF 框架能力 |
| Core | Figure、命中、坐标、Runtime 与绘制基础协议 | 消费既有 API；不新增 Core 吸附服务，所需 Core delta 另列 |
| 平台／宿主 | 原生输入、DPI、capture 与取消事件 | 验证真实输入进入 Editor 的链路 |
| 应用 | Grid／Guide 资源、业务资格、Command 与 attachment 持久化 | 消费者责任；应用功能不计为框架新增能力 |

本轮已完成本页引用方法的只读依赖核验，证据链见
[依赖核验与修订说明](../verification/reviews/adr-058-dependency-and-design-review-2026-11-02.md)。
所查坐标、bounds、有效可见性与反馈基础契约在 Core 有对应入口；
新增受检特征查询和吸附会话编排归 Editor。该结论不表示实现或平台验收通过。
若确认必要 Core 契约缺失，停止增加 Editor 吸附能力，先交付 Core 增补建议；
Core 依赖验收通过后才能恢复。本门禁见
[项目规则](../../CLAUDE.md#editor-缺失-core-依赖时停止新增)。

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
释放必须使用本次 release 的最新 logical surface 位置重新建立输入，
执行 E03 的目标、资格和完整计划校验。仅当完整输入相同且依赖仍有效时，
可复用吸附计算；不能因版本相同复用上次 move 的位置。

有效性分开处理：E03 冻结的源模型 revision、源身份与权限代际变化
取消原手势，不能重新取一份模型基线继续提交。坐标、目标候选或独立吸附配置变化
仅在 E03 来源仍有效时重算；回调前后再次核对输入。
每次 release 进行一次完整求解流程，包含必要的首次释放计算，不提供额外失效重试。
流程中再次变化则结构化拒绝；后续新输入不能复原已取消的手势。
具体状态规则在专题维护。

阈值使用宿主逻辑像素，不随 DPI 增大；模型网格先从目标 content 域映射到 surface，
在 surface 比较修正距离。阈值捕获与精确投影使用公共候选准入协议，
准入类型由受控策略绑定授权；第三方和内置策略均可采用同一协议。
阈值捕获检查有限捕获半径，始终量化使用验证原始输入到目标的精确投影关系，
不受捕获半径限制；数值、自由度、布局、版本与预算检查仍相同。
仲裁器不为 Grid 增加免检分支。不可逆、无路径或溢出按现有坐标错误拒绝，
不降级成未吸附成功。resize 只改变允许边，固定边与最小尺寸先于候选发布检查。

“每轴”指操作声明的自由度，不将目标 content 轴与 surface 轴混同。
原始角点和完整仿射映射进入检查；surface AABB 不代表旋转后的真实边。
候选显式记录修正写入的自由度、对齐约束依赖的自由度及可求值的约束。
需要同时改变多个自由度的修正以耦合候选表达，使用与外部策略相同的仲裁路径。

按显式策略顺序仲裁每轴；同策略候选先比较距离，再按稳定特征身份排序。
有耦合约束的候选整体接受或拒绝，不能拼接其一半修正。
不重复累加已吸附结果，不在反馈层另算一套位置。
每次加入候选都检查暂定几何和此前接受的约束；合法冲突跳过较晚候选，
扩展错误或非法输出拒绝整次求解。最终布局结果再次验证全部采用约束，
反馈只展示仍成立的命中事实。
布局允许的位置集合进入输入；policy 返回实际计划几何后只做一次最终校验。
布局改写使命中失效则拒绝并清理反馈，不反复从同一原始输入重新吸附，
也不自动退成未吸附提交。完整顺序在专题维护。

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

GEF `SnapToGuides.snapRectangle` 对多选直接跳过。Novadraw 的多选 Guide
明确作为扩展：唯一锚点决定整个批次平移，Guide 命中事实不自动持久化到所有成员。
该扩展单独验证，不宣称是 GEF 已有的多选行为。

Java 只读，不使用 Zest。`api_semantics`：GEF `snap.guides`、
`tool.tracker`、`feedback.protocol`、`interaction.change_bounds`，
以及 Draw2D `coordinate.conversion`。family 使用现行语义账本的规范名称。

## 外部消费者证据

本节是完整消费者要求。E04-A 首段已通过开放策略、provider、求值器的公开
消费者与 Native／Wasm 编译验证，见[实施记录](../verification/reviews/p2-e04-a-implementation-2026-11-02.md)；
内置策略、Tool、实际 plan 和平台行为仍待后续验证。
上述 GEF 源码依赖核验与下述消费者验收分别记录，不能互相替代。

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
公开 API 使用 `novadraw_editor::snap` 领域路径，不增加全局 prelude 或根目录平铺导出。
Native／Wasm API 与依赖边界作为默认门禁；是否独立 feature 按依赖和消费边界评审。
产物体积仅在用户明确要求时测量，不作为本片默认门禁。

设计评审后才分片实施：纯协议／仲裁 → 内置策略与外部消费者 →
Tool／Overlay 集成 → Native／Web 输入与成本。
用例见 [验证矩阵](../verification/plans/editor-open-snapping.md)。
本决策按用户授权进入 E04-A；不关闭其他批次。
当前进度、剩余验收及环境条件统一见
[执行队列](../roadmap/p2-delta-backlog.md#editorgef-框架能力补充2026-10-29-用户授权)。
