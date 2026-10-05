# Review Index

类型：`documentation-index`

本目录保存已完成审计和阶段验证的结论。报告是对应快照的历史证据，不覆盖当前
`design/`、ADR、parity ledger 或 roadmap。

## Core 架构与迁移

| 日期 | 报告 |
|---|---|
| 2026-08-23 | [核心渲染管线](core-pipeline-2026-08-23.md) |
| 2026-08-29 | [文档与源码一致性](design-code-audit-2026-08-29.md) |
| 2026-09-08 | [Draw2D 核心能力差异](draw2d-core-capability-audit-2026-09-08.md) |
| 2026-09-08 | [长期架构与可扩展性](architecture-sustainability-review-2026-09-08.md) |
| 2026-09-08 | [D3 Runtime mutation](d3-runtime-mutation-2026-09-08.md) |
| 2026-09-08 | [D3 Runtime listener](d3-runtime-listener-2026-09-08.md) |
| 2026-09-09 | [D4 审计校准](d4-audit-calibration-2026-09-09.md) |
| 2026-09-09 | [D4 派生状态收敛](d4-derived-state-convergence-2026-09-09.md) |
| 2026-09-09 | [D4 资源因果与 Backend Session](d4-resource-causality-and-backend-session-2026-09-09.md) |
| 2026-09-10 | [ADR-001 至 ADR-013 审计](adr-audit-2026-09-10.md) |
| 2026-09-10 | [ADR-014 首批实现门禁](adr014-implementation-gate-2026-09-10.md) |
| 2026-09-10 | [ADR-014 D4.4](adr014-d4.4-increment-2026-09-10.md) |
| 2026-09-10 | [ADR-014 D4.5 性能](adr014-d4.5-performance-2026-09-10.md) |
| 2026-09-10 | [ADR-014 D4.6 收口](adr014-d4.6-completion-2026-09-10.md) |
| 2026-09-13 | [Draw2D Core 1.0 最终审计](draw2d-core-1.0-final-audit-2026-09-13.md) |
| 2026-09-16 | [Draw2D / GEF 全量语义映射](draw2d-gef-semantic-mapping-2026-09-16.md) |
| 2026-09-16 | [Draw2D / GEF 语义差异报告](draw2d-gef-semantic-differences-2026-09-16.md) |
| 2026-09-16 | [Draw2D / GEF 迁移实施方案](draw2d-gef-migration-plan-2026-09-16.md) |
| 2026-09-20 | [Draw2D / GEF 核心语义后续审计](draw2d-gef-core-semantic-follow-up-audit-2026-09-20.md) |
| 2026-09-22 | [Core 公开 API 语义与命名审计](core-public-api-audit-2026-09-22.md) |
| 2026-09-28 | [引擎能力与 API 稳定化评估](engine-capability-assessment-2026-09-28.md) |
| 2026-09-29 | [Crate 边界与发布结构审计](crate-boundary-audit-2026-09-29.md) |
| 2026-09-29 | [P2-T02 Text Interaction Geometry](p2-text-interaction-2026-09-29.md) |
| 2026-09-30 | [项目目标、设计与实现一致性审计](goal-design-code-audit-2026-09-30.md) |
| 2026-10-01 | [GA-4 模块与扩展表面](ga4-module-extension-completion-2026-10-01.md) |
| 2026-10-01 | [GA-5 文档与门禁](ga5-documentation-gate-completion-2026-10-01.md) |
| 2026-10-01 | [GA-6 目标矩阵审计](ga6-goal-matrix-audit-2026-10-01.md) |
| 2026-10-05 | [P2-G02 统一文字与 Graphics](p2-g02-text-graphics-evidence.md) |

## 平台与产品验收

| 日期 | 报告 |
|---|---|
| 2026-09-10 | [M10.5 Tooltip 与 Accessibility](m10.5-tooltip-accessibility-2026-09-10.md) |
| 2026-09-12 | [macOS Live Resize](macos-live-resize-transaction-2026-09-12.md) |
| 2026-09-12 | [M1-M8 人工验收](m1-m8-manual-acceptance-2026-09-12.md) |
| 2026-09-13 | [M10 Web 等价](m10-web-equivalence-2026-09-13.md) |
| 2026-09-30 | [Web EditContext 集成验证](web-edit-context-capability-2026-09-30.md) |

## Editor

| 日期 | 报告 |
|---|---|
| 2026-09-13 | [GEF / Editor 启动就绪](gef-readiness-analysis-2026-09-13.md) |
| 2026-09-13 | [G1 Model 与 Command](g1-model-command-completion-2026-09-13.md) |
| 2026-09-13 | [G2 Viewer 投影](g2-viewer-projection-completion-2026-09-13.md) |
| 2026-09-13 | [G3 Selection 与 Targeting](g3-selection-targeting-behavior-2026-09-13.md) |
| 2026-09-14 | [G4 编辑闭环](g4-editing-loop-behavior-2026-09-14.md) |
| 2026-09-14 | [G5.1 Connection Projection](g5-connection-projection-design-2026-09-14.md) |
| 2026-09-14 | [G5.2 Connection Creation](g5-connection-creation-behavior-2026-09-14.md) |
| 2026-09-15 | [G5.3 Connection Reconnect](g5-connection-reconnect-behavior-2026-09-15.md) |
| 2026-09-15 | [G5.4 Connection Bendpoint](g5-connection-bendpoint-behavior-2026-09-15.md) |
| 2026-09-15 | [G5.5 Viewport Auto-expose](g5-viewport-autoexpose-behavior-2026-09-15.md) |
| 2026-09-15 | [Connection 扩展边界整改](connection-extensibility-correction-2026-09-15.md) |
| 2026-09-16 | [Draw2D / GEF 语义审计整改状态](draw2d-gef-semantic-remediation-2026-09-16.md) |

完整语义审计的固定基线见
[`../reference/draw2d-gef-semantic-baseline-2026-09-16.md`](../reference/draw2d-gef-semantic-baseline-2026-09-16.md)，
原始分组报告、日志和结构化结果见
[`../../../verification/evidence/`](../../../verification/evidence/README.md)。
P1 关闭进度见
[`draw2d-gef-semantic-remediation-2026-09-16.md`](draw2d-gef-semantic-remediation-2026-09-16.md)。
