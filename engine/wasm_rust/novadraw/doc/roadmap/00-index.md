# Novadraw 路线图

类型：`roadmap`

本目录承载 Draw2D Core 与 Editor 两条引擎交付路线。Core 使用 `M1-M10`，Editor
framework 使用 `G0-G5`；原 G6 产品毕业范围已移交独立产品包。相关 API 语义覆盖由
`doc/parity/` 下的账本维护。

## 文档职能边界

| 文件 | 职能 | 性质 | 更新频率 |
|------|------|------|----------|
| `doc/roadmap/00-index.md` | **路线图入口**：M1-M10 编号、依赖关系和当前状态 | 人读，里程碑唯一入口 | 每个 milestone 状态变化时 |
| `doc/parity/draw2d/api-coverage.md` | **语义账本**：draw2d API family、Novadraw 对照方向、覆盖状态与 milestone 映射 | 人读，架构与实现对齐入口 | 按语义收敛持续更新 |
| `doc/roadmap/product-deliverables.md` | **产品视图**：每个 milestone 下要交付的图元数量、布局种类、边框种类等策略层清单 | 人读，启动期定稿 | 启动期一次，后续微调 |
| `doc/roadmap/demo-matrix.md` | **验证视图**：每个 milestone 配套的 demo 名称、覆盖范围、截图/帧率断言策略 | 人读，启动期定稿 | 启动期一次，后续微调 |
| `doc/roadmap/editor/00-index.md` | **Editor 路线图**：G0-G5 引擎里程碑与下游产品边界 | 人读，Editor 里程碑唯一入口 | 每个 milestone 状态变化时 |
| `doc/roadmap/p2-delta-backlog.md` | **P2 delta**：Core 1.0 之后已确认能力的统一状态 | 人读，后续能力入口 | 每项状态变化时 |

## 编号唯一来源

**本文是 `M1-M10` 编号和状态的唯一入口。**

任何文档、提交或实现说明中的 `M{n}` 都指本文定义的 milestone，不允许在其他
文档中发明独立编号。语义覆盖、产品交付和 demo 验证分别由三份配套文档维护，
但都不能单独改变 milestone 状态。

## 状态定义

| 状态 | 含义 |
|------|------|
| `not_started` | 尚未进入当前开发主线 |
| `in_progress` | 已有实现或验证增量，但完成判据尚未全部满足 |
| `contract_aligned` | 公开契约与架构边界已稳定，行为验证尚未全部完成 |
| `behavior_verified` | 核心契约已有可重复的测试或 demo 证据 |
| `complete` | 契约、产品面、端到端验证和文档全部收口 |

## 当前状态

| Milestone | 标题 | 状态 | 当前证据或主要缺口 |
|------|------|------|------|
| M1 | 几何与 Graphics 基础 | `complete` | 几何与 Graphics 状态栈测试、核心管线人工验收已完成；高级 Graphics API 明确延后 |
| M2 | Figure 树与盒模型 | `complete` | active Figure 的树、盒模型、生命周期、z-order 与产品入口已验收 |
| M3 | 绘制遍历与裁剪闭环 | `complete` | `clip-app` 固定/响应式祖先裁剪与 paint/hit-test 一致性已验收 |
| M4 | 坐标域与变换闭环 | `complete` | `m4_coordinate_contract` 与 `transform-app` 自动/人工验收已完成 |
| M5 | Layout + Validation + UpdateManager | `complete` | 六布局、两阶段事务、damage、1,024 Figure 与 root viewport resize 已验收 |
| M6 | 事件分发与交互状态机 | `complete` | capture/focus/key/wheel/gesture session 与 target-domain 自动/人工验收已完成 |
| M7 | 通知语义分层 | `complete` | 七类 typed listener、Runtime 注册/统一注销、稳定事务分发与 self-removal 已闭合 |
| M8 | Viewport / Scroll / Zoom | `complete` | 24 项契约、7 项无窗口 verification、Native 交互与 Web Vello 场景复核已完成 |
| M9 | Connection / Anchor / Router | `complete` | D3.1 shared Manhattan、严格 viewport topology、八场景截图与人工窗口验收已闭合 |
| M10 | 常用 Figure 与文本/控件 | `complete` | M10.1-M10.5 与 Text/Image/Widget Native/Web 等价验收已完成；完整 TextFlow 保持 P2 |

## 当前执行方向

R8/R9、D0-D4 与 M1-M10 已完成；2026-09-08/10 长期架构审计的 A01-A08 已关闭。
2026-09-13 macOS/Web/Headless 总审计与 R9.4 capability 消融复查通过，Draw2D
Core 1.0 完成。后续能力必须进入 Editor roadmap 或明确的 P2 delta。
P2-R02 image source rectangle 已于 2026-09-29 完成。
P2-C01 Connection decoration、P2-C02 shortest-path routing、P2-F01
ScalablePolygonFigure 与 P2-T01 TextFlow 第一阶段已于 2026-09-29 完成自动门禁和
macOS Native/Vello 人工验收。P2-T02 TextFlow interaction geometry 已完成；P2-E02
direct-edit session、Winit bridge、Web DOM host、Native/Web 场景及人工验收已于
2026-09-30 完成。FigureInspector、Studio 与高级 self-loop 策略
继续后置。
2026-09-10 ADR-014 已替换旧 ADR-013；D4.3-D4.6 的新接口、所有权、约束测量、
scope、历史事件、串行 session handoff、递归性能和最终回归门禁已完成。
当前引擎开发从 ADR-020 的基础值与公开渲染契约整改继续。原 G6 的产品 schema、
serializer 和产品级 Native/Web 场景由独立产品包负责；已完成的 D0-D4、M8/M9 与
R8/R9 执行计划保留在 [`../archive/`](../archive/00-index.md)，不再作为当前工作入口。

`D0-D4` 是跨 milestone 的 architecture delta，不是新的 milestone 编号。它们负责
消除会被 M9/M10 放大的公共协议缺口；M1-M10 的状态仍只在本文维护。

状态提升规则：

1. `behavior_verified` 至少要求语义账本中的主 API family 有可重复的契约测试。
2. `complete` 必须同时通过语义账本、产品交付清单和 demo 验证矩阵。
3. 已存在的原型、类型或 demo 不能单独作为 milestone 完成依据。

## GEF 边界

当前语义账本中标记为 GEF 层或非核心目标的能力，不纳入 draw2d 核心里程碑：

- EditPart / EditPolicy / Tool / Command / Request / Viewer / Palette / Selection provider / Undo-redo command stack

这些不在 draw2d 核心里程碑内。Draw2D Core 1.0 完成后，Editor framework 已完成独立
[`G0-G5 路线图`](editor/00-index.md)。带"节点编辑器"性质的 demo 只作为引擎验证入口，
不能定义产品 schema 或产品交付边界。

## 文档列表

2026-09-30 的[目标一致性调整计划](goal-alignment-adjustment-plan-2026-09-30.md)
为待评审提案，覆盖 Draw2D 剩余能力、性能、扩展契约与四平台验证；不改变上述历史状态。

| 文档 | 主题 |
|------|------|
| `product-deliverables.md` | 每个 milestone 下要交付的产品策略层清单 |
| `demo-matrix.md` | 每个 milestone 对应的 demo + 验证矩阵 |
| `editor/00-index.md` | Editor G0-G5 编号、状态和验收检查点 |
| `editor/implementation-plan.md` | Editor 各阶段实施边界和毕业条件 |
| `p2-delta-backlog.md` | 已确认但尚未进入实施的 P2 delta |
