# E04 开放吸附：依赖核验与修订 1 评审说明

类型：`architecture-review`

能力归属：Editor（`novadraw-editor`）。Core 作为坐标、几何、场景查询与绘制依赖；
平台负责实际输入，应用负责业务 Command 和 Guide 持久化。
日期：2026-11-02。状态：只读依赖核验与设计修订完成，待用户评审，未实施。

本轮按用户“按计划推进，如果需要，就给出方案等待评审”的要求，
核对既有 E03、S09-D 和 E04 草案。当前可推进 E04 的设计与依赖核验；
不据本报告关闭前两批验收，也不启动 E04 Execute。
决策见 [ADR-058 修订 1](../../adr/adr-058-open-editor-snapping.md)，
完整行为见[专题](../../design/editor/open-snapping.md)，
退出条件见[矩阵](../plans/editor-open-snapping.md)。

## 当前结论

本轮检查的 GEF 吸附依赖在 Core 有对应契约，没有确认必须先新增的 Core 能力。
需要增加的是 Editor 的受检特征查询、策略注册、会话有效性与 Tool 编排。
这只是源码与契约对应结论，不是 E04 的运行、性能或平台验收结果。
实现中若发现必要依赖不满足，仍须停止 Editor 新增并提出独立 Core 建议。

参考源码为本地 `gef-classic` commit
`4463d9d0ce13c19d10fbe769d29f28b7345a8cba`，
仅查看 `org.eclipse.gef` 与 `org.eclipse.draw2d`，未修改、编译或运行 Java。
Novadraw 依赖入口已定向审阅；本报告不按历史行号推定当前代码仍然等价。

## GEF → Draw2D → Core → Editor

下表的 GEF 路径均相对 `org.eclipse.gef/src/org/eclipse/gef/`。
Draw2D `IFigure` 声明 `getBounds`、`isVisible` 与
`translateToAbsolute`／`translateToRelative`；具体采用边界如下。

| GEF 源码与行为 | Draw2D 基础依赖 | Core 当前入口 | Editor 采用与缺口 |
|---|---|---|---|
| `SnapToHelper.java:37–56`、`SnapToGrid.java:127–169`：content 域与 absolute 域转换 | Figure 父链、content transform、正反向坐标转换 | `TreeQuery::{local_to_surface_transform, child_content_to_surface_transform, surface_to_local_transform}`；完整父链校验，奇异逆为合法缺失，非法输入／溢出为错误 | `ToolContext::{child_content_to_surface, surface_to_child_content}` 已有；吸附复用，不能另建坐标树 |
| `SnapToGeometry.java:205–211,305–353`：排除操作对象、读取 sibling 边／中心，过滤隐藏对象 | bounds、可见性、父子关系 | `TreeQuery::{figure_bounds,is_effectively_visible}`、Figure 身份与树查询 | `PartTree` 与模型映射已有；新增受检特征查询，保留原始角点，不以 surface AABB 假造旋转后的边 |
| `SnapToGuides.java:253–327`：Guide 对齐、坐标转换及 attachment 事实 | content 坐标与几何 | 同上，不需要 Core Guide 服务 | Guide 身份、资源版本、策略与业务 attachment 分属 Editor／应用 |
| `CompoundSnapToHelper.java:57–62`：配置顺序、剩余方向 | 纯几何运算，无新增 Draw2D 调度服务 | 既有 `Affine2D`、Point、Rectangle | 仲裁归 Editor；外部耦合候选与内置候选共用检查路径 |
| `DragEditPartsTracker.java:570–629`：auto-expose 修复起点、目标切换更换 helper | viewport／坐标查询 | 既有 viewport、scale 与父链变换 | `EditorDomain` 的 auto-expose、zoom／origin／resize 刷新与 E03 的源／目标映射检查已有；新增 SnapSession 失效编排 |
| Layout feedback 和释放前清理 | Figure 挂载／卸载、绘制与生命周期 | Core Figure／Runtime 协议 | 复用 `FeedbackVisual` 与 `remove_overlay_visuals` 的批次预检；不新增旁路 renderer |

Core 证据入口：
[图查询](../../../crates/novadraw/src/graph/query.rs)、
[仿射几何](../../../crates/novadraw/src/geometry/transform.rs)。
Editor 证据入口：
[ToolContext](../../../crates/novadraw-editor/src/tool/mod.rs)、
[容器候选校验](../../../crates/novadraw-editor/src/container/service.rs)、
[Viewer](../../../crates/novadraw-editor/src/viewer/mod.rs)、
[Domain 驱动](../../../crates/novadraw-editor/src/domain.rs)。
既有失败语义证据见
[S07 坐标查询核验](p2-s07-coordinate-route-query-evidence-2026-10-16.md)
和 [E03 组合续验](editor-container-combinations-2026-11-02.md)。

## 本轮修订

| 问题 | 修订 1 的处理 | 所属验证 |
|---|---|---|
| 草案把模型变化和坐标变化都写成重算，可能绕过 E03 冻结来源 | 源模型 revision、源身份和权限代际变化取消原手势；viewport／独立吸附配置变化只在 E03 仍有效时重算 | SN-06／07／16 |
| `part_bounds_in_surface` 将查询错误变成 `None`，且返回 AABB | 新 Editor 特征入口使用 Core 的 fallible 查询，分别表示合法不参与与失败；输出带坐标来源的真实角点／特征 | SN-05／12／17 |
| “每轴”在旋转、非均匀缩放和比例 resize 下含义不清 | 仲裁使用操作声明的自由度；surface 距离验证所有被改变的控制点；耦合修正不可拆分 | SN-02／05／08／18 |
| GEF Guide 对多选直接跳过，草案却没有说明扩展语义 | 明确 Novadraw 的多选 Guide 是唯一锚点驱动的扩展；输出 attachment 事实，由应用决定持久化范围 | SN-04／13／19 |
| release 重算可能不受限，回调也可能使输入再次失效 | 每次求解前后检查同一输入；release 最多重新求解一次，仍失效则拒绝，无自动无限重试 | SN-07／10／16 |
| 复杂度只计算候选查询，遗漏来源验证和整体投影 | 分开记录冷建索引、依赖验证、查询、资格、反馈、提交与稳定化；一般外部候选不套用轴索引复杂度 | SN-14／20 |
| 外部候选与索引缺少资源准入边界 | 显式有限预算和有界 sink；超限整体拒绝，不承诺强制中止任意外部计算 | SN-21 |
| 草案仍自动要求产物体积对照 | 移除默认体积门禁，保留 Native／Wasm API 和 feature 边界；只在用户明确要求时测体积 | SN-15 |

错误／panic 使用 S09-D 的 owned source、操作位置和 payload 诊断，
不新增一套文本错误包装。普通拒绝和回调 panic 仍遵循所属 owner 的既有规则。
未通过几何、资格或反馈清理检查时不构造业务 Command。

## 实施切片与评审重点

按协议与受检输入 → 内置／外部策略 → Tool 与反馈 → 平台及成本四片串行推进，
各片具体退出条件在验证矩阵维护，不预造 xtask suite 或实施结果。
每片包含失败路径和独立消费者；内置与扩展统一从第一片开始验证。

评审重点是：来源变化必须取消的边界、带坐标来源的自由度与耦合规则、
多选 Guide 的扩展语义，以及有界重算与错误保留。
修订 1 获批前只做设计修订；获批后按执行队列处理 E03 的依赖与剩余验收，
不将批准方案等同于允许跳过既有退出条件。

## 保留的验收与环境限制

S09-D 最终 workspace／Wasm／成本仍待补验。本轮开始磁盘只余约 165 MiB，
没有重新发起构建；此前空间不足失败记录保持。
E03 的跨显示器 DPI 需要多显示器环境，完整 Native 组合、系统文件 DnD
和内部工作量计数仍待补验。本轮没有新增平台观测，不把这些条件解释为 Core 缺失。

本轮只交付方案、依赖核验和验证矩阵，没有修改 Core／Editor Rust 源码，
没有运行新功能测试，没有提交 Git。
成稿按 technical-writing-clarity 和好好说话自审；没有独立 reviewer，
源码依赖核验不替代运行时与外部消费者证明。
