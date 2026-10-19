# 验证计划

类型：`documentation-index`

本目录保存设计阶段的验证矩阵；草案须标明待评审，不代表已获准实施。实现开始后，
实际可执行 suite 由 xtask 维护；完成后证据转入 `../reviews/`。

| 文档 | 范围 | 状态 |
|---|---|---|
| [p2-text-direct-edit.md](p2-text-direct-edit.md) | P2-T02 TextFlow interaction geometry 与 P2-E02 direct edit/IME | `complete` |
| [p2-s06-incremental-failure-isolation.md](p2-s06-incremental-failure-isolation.md) | 新建 Part/Connection 失败清理、Overlay 预检及模型/history 边界 | `complete`，修订 2；组合证明及平台限制永久保留 |
| [p2-s07-query-identity-and-failure-semantics.md](p2-s07-query-identity-and-failure-semantics.md) | 查询身份、合法缺失、测量失败与候选传播 | `complete`，修订 1；实现与行为验证完成 |
| [xtask-command-simplification-2026-10-16.md](xtask-command-simplification-2026-10-16.md) | xtask 三入口、13 个跨边界 suite 与直接 Cargo 门禁 | `complete` |
