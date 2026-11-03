# 历史验证基线导航

类型：`documentation-index`

这些报告保留指定版本的判断与完成事实，不能作为当前设计或执行队列。
当前专题从[评审入口](00-index.md)进入；已归并的过程稿从
[维护前快照](../../archive/knowledge-maintenance-2026-11-03.md)追溯。
各专题契约和路线图仍链接其独立实施证据，下表补充跨专题审计与早期基线。

| 范围 | 历史报告 |
|---|---|
| 早期架构 | [渲染管线](core-pipeline-2026-08-23.md)、[设计／实现](design-code-audit-2026-08-29.md)、[架构可持续性](architecture-sustainability-review-2026-09-08.md)、[Core 能力](draw2d-core-capability-audit-2026-09-08.md) |
| Runtime 收口 | [动态 mutation](d3-runtime-mutation-2026-09-08.md)、[监听生命周期](d3-runtime-listener-2026-09-08.md)、[D4 校准](d4-audit-calibration-2026-09-09.md) |
| Core 1.0／文本输入 | [Web 等价](m10-web-equivalence-2026-09-13.md)、[Tooltip／AT](m10.5-tooltip-accessibility-2026-09-10.md)、[direct edit](p2-direct-text-edit-session-2026-09-29.md)、[输入 bridge](p2-text-input-bridge-2026-09-29.md) |
| Draw2D／GEF 语义 | [连接扩展修正](connection-extensibility-correction-2026-09-15.md)、[语义映射](draw2d-gef-semantic-mapping-2026-09-16.md)、[差异](draw2d-gef-semantic-differences-2026-09-16.md)、[整改计划基线](draw2d-gef-migration-plan-2026-09-16.md) |
| 包与代码质量 | [crate 边界](crate-boundary-audit-2026-09-29.md)、[代码质量](code-quality-review-2026-10-26.md)、[Wasm 文件迁移](wasm-rust-file-migration-2026-10-28.md) |
| 验证工具 | [suite／单测边界](suite-configuration-and-unit-test-boundary-2026-10-16.md)、[xtask 评估](xtask-command-system-value-assessment-2026-10-16.md) |
| 文档维护 | [知识修正](documentation-knowledge-revision-2026-10-19.md)、[写作审计](document-writing-audit-2026-10-29.md) |
| 官方样例 | [API 接入](draw2d-example-api-ergonomics-2026-10-19.md)、[功能审计](draw2d-official-examples-functional-audit-2026-10-26.md)、[系统字体](official-system-fonts-2026-10-28.md) |
| K07 Scene | [捕获切片](k07-1-scene-capture-evidence-2026-10-23.md)、[组切片](k07-2-scene-group-evidence-2026-10-23.md) |
| 输入与服务设计基线 | [S08 形成记录](p2-s08-design-record-2026-10-29.md)、[S09／吸附初期评审](s09-and-snapping-design-review-2026-10-29.md) |
| GEF 消费者 | [Logic 首片](gef-logic-first-slice-2026-10-29.md)、[Shapes／Inspector 布局分析](shapes-inspector-layout-analysis-2026-10-29.md) |

同一问题的当前结论优先引用专题状态，历史比较只在配置与统计口径一致时解释收益。
早期报告中的“下一步”属于当时建议，当前推进只以 roadmap 为准。
