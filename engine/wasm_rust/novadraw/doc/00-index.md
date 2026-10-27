# Novadraw 文档入口

类型：`documentation-index`

先按要解决的问题选择入口。本文只负责导航；契约、决策、执行状态和历史证据各有
独立真源，文档规则见 [知识维护](knowledge-maintenance.md)。

## 从任务进入

| 任务 | 阅读顺序 |
|---|---|
| 理解 Core | [架构概览](design/architecture/overview.md) → [静态结构](design/architecture/static-architecture.md) → [动态协议](design/architecture/dynamic-architecture.md) |
| 编写应用或扩展 | [公共 API 契约](design/architecture/public-api-contract.md) → [Figure 能力](design/architecture/figure-capability-model.md) → [组件更新](design/architecture/component-update.md) |
| 修改几何、布局或渲染 | [坐标](design/coordinates/coordinate-system.md) → [收敛事务](design/architecture/derived-state-convergence.md) → [渲染专题](design/rendering/00-index.md) |
| 开发输入与动画 | [Scroll/Zoom 输入](design/input/scroll-zoom-gesture-contract.md) → [动画专题](design/animation/00-index.md) |
| 开发 Editor | [Editor 规范](design/editor/00-index.md) → [GEF 采用账本](parity/gef/api-coverage.md) → [Editor 路线图](roadmap/editor/00-index.md) |
| 核对 Draw2D/GEF 行为 | [参考分析](reference/00-index.md) → [源码审计基线](verification/reference/00-index.md) → [语义账本](parity/00-index.md) |
| 确定下一批工作 | [P2 backlog](roadmap/p2-delta-backlog.md#当前推进顺序2026-10-14-校准) → [整改执行清单](verification/checklists/audit-remediation-execution.md) |
| 验证实现 | [可执行 suite](../tests/README.md) → [人工手册](verification/manual/00-index.md) → [历史核验](verification/reviews/00-index.md) |
| 评估产品或迁移 | [产品策略](strategy/00-index.md)、[Java/Rust 迁移](migration/00-index.md) |

## 知识归属

| 入口 | 维护内容 |
|---|---|
| [design](design/00-index.md) | 当前行为、所有权、权限、失败和扩展契约 |
| [ADR](adr/README.md) | 决策原因、取舍、后果和替代关系 |
| [reference](reference/00-index.md) | 外部源码事实及其基线 |
| [parity](parity/00-index.md) | 外部行为的采用、调整、收窄和验证映射 |
| [roadmap](roadmap/00-index.md) | M1–M10 编号与状态、P2/GA 队列；Editor 状态由其子入口维护 |
| [verification](verification/00-index.md) | 回归矩阵、验收方法和基线报告 |
| [strategy](strategy/00-index.md) | 产品假设、市场调查和决策输入 |
| [migration](migration/00-index.md) | 可复用迁移方法 |
| [archive](archive/00-index.md) | 退出当前使用的方案、原稿与校验值；历史追溯时显式进入 |

## 状态与证据入口

M1–M10、G0–G5、P2、GA 和平台资格分别在以下文件维护，不从 accepted ADR、
已存在 package、测试数量或某个 demo 推断完成：

- [Core 里程碑](roadmap/00-index.md)；
- [Editor 里程碑](roadmap/editor/00-index.md)；
- [P2 增量及唯一执行队列](roadmap/p2-delta-backlog.md)；
- [目标调整与 GA 门禁](roadmap/goal-alignment-adjustment-plan-2026-09-30.md)；
- [平台支持等级](roadmap/platform-support-matrix.md)。

原始日志、JSON 和探针见 [tests/evidence](verification/evidence/README.md)。
本次全量修正的处置、保留范围和验证结果见
[知识修正记录](verification/reviews/documentation-knowledge-revision-2026-10-19.md)。
