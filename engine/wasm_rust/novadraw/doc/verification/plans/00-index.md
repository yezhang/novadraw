# 验证计划

类型：`documentation-index`

本目录保存设计与回归矩阵；草案须标明待评审，不代表已获准实施。完成矩阵保留
验收范围和永久限制，执行结果从 reviews 查阅。当前 suite 由
[验证入口](../../../tests/README.md)维护，表内状态只指矩阵对应切片。

| 文档 | 范围 | 状态 |
|---|---|---|
| [draw2d-official-examples-remaining-contracts.md](draw2d-official-examples-remaining-contracts.md) | 全部 25 个官方入口、共享缺口与 Core/扩展/平台责任矩阵 | 修订 3，已获准实施 |
| [draw2d-official-examples-core-plan.md](draw2d-official-examples-core-plan.md) | Core K00–K09、算法/控件扩展、宿主 H01–H04 的顺序与退出条件 | 已获准按顺序实施 |
| [p2-text-direct-edit.md](p2-text-direct-edit.md) | P2-T02 TextFlow interaction geometry 与 P2-E02 direct edit/IME | `complete` |
| [p2-s06-incremental-failure-isolation.md](p2-s06-incremental-failure-isolation.md) | 新建 Part/Connection 失败清理、Overlay 预检及模型/history 边界 | `complete`，修订 2；组合证明及平台限制永久保留 |
| [p2-s07-query-identity-and-failure-semantics.md](p2-s07-query-identity-and-failure-semantics.md) | 查询身份、合法缺失、测量失败与候选传播 | `complete`，修订 1；实现与行为验证完成 |
| [xtask-command-simplification-2026-10-16.md](xtask-command-simplification-2026-10-16.md) | xtask 三入口、13 个跨边界 suite 与直接 Cargo 门禁 | `complete` |
