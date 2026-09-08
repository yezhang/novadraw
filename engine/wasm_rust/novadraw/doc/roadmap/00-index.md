# 06-Roadmap 路线图

类型：`roadmap`

本目录承载 Novadraw 朝 draw2d 核心演进的里程碑定义、**产品视图**与
**demo 视图**；相关 API 语义覆盖由
`doc/parity/draw2d/api-coverage.md` 补充。

## 文档职能边界

| 文件 | 职能 | 性质 | 更新频率 |
|------|------|------|----------|
| `doc/roadmap/00-index.md` | **路线图入口**：M1-M10 编号、依赖关系和当前状态 | 人读，里程碑唯一入口 | 每个 milestone 状态变化时 |
| `doc/parity/draw2d/api-coverage.md` | **语义账本**：draw2d API family、Novadraw 对照方向、覆盖状态与 milestone 映射 | 人读，架构与实现对齐入口 | 按语义收敛持续更新 |
| `doc/roadmap/product-deliverables.md` | **产品视图**：每个 milestone 下要交付的图元数量、布局种类、边框种类等策略层清单 | 人读，启动期定稿 | 启动期一次，后续微调 |
| `doc/roadmap/demo-matrix.md` | **验证视图**：每个 milestone 配套的 demo 名称、覆盖范围、截图/帧率断言策略 | 人读，启动期定稿 | 启动期一次，后续微调 |
| `doc/roadmap/m8-m9-contract-plan.md` | **阶段计划**：Viewport 与 Connection 的交付边界和验证门禁 | 人读，不定义理想架构 | 推进 M8/M9 时 |
| `doc/roadmap/core-completion-and-draw2d-migration-plan.md` | **当前执行计划**：R8/R9 后的核心收口 delta 与 M9/M10 分批顺序 | 人读，不创建新 milestone 编号 | 每个执行批次完成时 |

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
| M1 | 几何与 Graphics 基础 | `behavior_verified` | 几何与 Graphics 状态栈测试通过；高级 Graphics API 延后 |
| M2 | Figure 树与盒模型 | `behavior_verified` | active Figure 的树、盒模型、生命周期与 z-order 已验证 |
| M3 | 绘制遍历与裁剪闭环 | `behavior_verified` | `clip-app` 与 paint/hit-test 一致性测试已完成 |
| M4 | 坐标域与变换闭环 | `behavior_verified` | `m4_coordinate_contract` 与 `transform-app` 已完成 |
| M5 | Layout + Validation + UpdateManager | `behavior_verified` | 六布局、约束/尺寸、两阶段事务、panic 恢复、damage 与 1,024 Figure 契约测试已闭合 |
| M6 | 事件分发与交互状态机 | `behavior_verified` | mouse/cursor/hover/capture/focus 分离；scroll/zoom gesture session 与 pointer capture 分轨，连续手势固定 target |
| M7 | 通知语义分层 | `behavior_verified` | 内部 typed listener 与因果顺序已验证；Runtime 完整注册/注销公共面转入 D3.3 |
| M8 | Viewport / Scroll / Zoom | `behavior_verified` | RangeModel、ViewportLayout、ZoomManager、ScrollPane/ScrollBar、手势缩放与 24 项契约测试已闭合；等待人工窗口签收后再评估 complete |
| M9 | Connection / Anchor / Router | `complete` | D3.1 shared Manhattan、严格 viewport topology、八场景截图与人工窗口验收已闭合 |
| M10 | 常用 Figure 与文本/控件 | `in_progress` | M10.1-M10.4 已完成；D3 审计收口后进入 M10.5，完整 TextFlow 保持 P2 |

## 当前执行顺序

R8/R9、D0-D2 与 M10.1-M10.4 已完成。2026-09-08 审计发现 M9 正式契约和核心
Runtime 公共面仍有缺口；D3.0-D3.1 已完成，M9 已恢复 `complete`，当前进入 D3.2
Runtime 动态 mutation 公共面。后续按
[`core-completion-and-draw2d-migration-plan.md`](core-completion-and-draw2d-migration-plan.md)
执行：

```text
D0 路线图与证据校准
→ D1 核心公共协议收口
→ D2 Layer / Freeform
→ M9 / M10.1-M10.4 产品基线
→ D3 Draw2D Core 审计收口
→ M10.5 Tooltip / Accessibility
→ Draw2D Core 1.0
```

`D0-D3` 是跨 milestone 的 architecture delta，不是新的 milestone 编号。它们负责
消除会被 M9/M10 放大的公共协议缺口；M1-M10 的状态仍只在本文维护。

状态提升规则：

1. `behavior_verified` 至少要求语义账本中的主 API family 有可重复的契约测试。
2. `complete` 必须同时通过语义账本、产品交付清单和 demo 验证矩阵。
3. 已存在的原型、类型或 demo 不能单独作为 milestone 完成依据。

## GEF 边界

当前语义账本中标记为 GEF 层或非核心目标的能力，不纳入 draw2d 核心里程碑：

- EditPart / EditPolicy / Tool / Command / Request / Viewer / Palette / Selection provider / Undo-redo command stack

这些不在 draw2d 核心里程碑内。带"节点编辑器"性质的 demo 视为 GEF 层早期探索，详见 `demo-matrix.md` 附录。

## 文档列表

| 文档 | 主题 |
|------|------|
| `product-deliverables.md` | 每个 milestone 下要交付的产品策略层清单 |
| `demo-matrix.md` | 每个 milestone 对应的 demo + 验证矩阵 |
| `m8-m9-contract-plan.md` | M8/M9 交付范围、前置契约与验收门禁 |
| `core-completion-and-draw2d-migration-plan.md` | 当前核心收口与后续 Draw2D 功能迁移执行顺序 |
