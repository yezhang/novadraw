# 交付证据说明

类型：`verification`

人读交付物：

- [语义映射表](../../../doc/verification/reviews/draw2d-gef-semantic-mapping-2026-09-16.md)
- [差异报告](../../../doc/verification/reviews/draw2d-gef-semantic-differences-2026-09-16.md)
- [迁移实施方案](../../../doc/verification/reviews/draw2d-gef-migration-plan-2026-09-16.md)

`group/*.jsonl`、原日志、baseline与probe元数据保留历史证据。`comments.jsonl`为最终
裁定聚合（16 P1、1 P2），`final_comments.json`精选5项用于技能要求的简版报告；
其余条目未删除，详细差异见完整报告。`cross-group-review.md`解释评级调整和去重。
`assemble_delivery.py`可重建聚合与恢复时hash差异，不重新运行产品验证。

可重建HTML/Markdown简版输出在项目的
`target/verification/semantic-audit-2026-09-16/report.html` 与 `report.md`，
按仓库规定不放入文档或提交的证据目录。生成器是
`/Users/bytedance/.trae-cn/skills/bits-code-guard/scripts/generate_report.py`，
输入为本目录`final_comments.json`。简版仅表示原快照精选缺陷，不表示当前整改状态。
HTML统计的变更行数0表示本次为全文件语义审计，不是零行源码被阅读；
158文件/75,788行清单是范围，不是逐行证明。

恢复汇总时HEAD已变化，`delivery-checks.json`列出与初始158文件hash不同的路径。
后续修复验证由整改页记录。本次收尾只执行文档检查，不重新证明动态工作区的全部语义。

历史`run_event_probe.py`、`run_editor_probe.py`、`run_additional_checks.py`和
`prepare_audit.py`是在旧目录结构下执行并归档的脚本，包含当时相对路径与日志输出位置，
不是迁移后可直接重跑的现行门禁。若需复现，先在隔离构建中恢复对应源码和依赖，
按现目录修正路径并将新输出写到新的target证据目录，避免覆盖原始日志。
不能对已修改产品重新运行反向断言探针后，把断言失败视为产品回归。
