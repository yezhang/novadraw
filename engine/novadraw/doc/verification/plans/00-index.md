# 验证计划

类型：`documentation-index`

本目录保存设计与回归矩阵；草案须标明待评审，不代表已获准实施。完成矩阵保留
验收范围和永久限制，执行结果从 reviews 查阅。当前 suite 由
[验证入口](../../../tests/README.md)维护，表内状态只指矩阵对应切片。

| 文档 | 范围 | 状态 |
|---|---|---|
| [specialized-edit-controller.md](specialized-edit-controller.md) | ADR-061 单一会话、专用控制器、外部消费者、richtext 与 Native/成本 | 接口与样例已实施，平台及完整性能验收开放 |
| [textflow-large-research-app.md](textflow-large-research-app.md) | 独立 Large 原场景、固定回放、CPU／Native 测量与有界导出 | 已实现；自动回放通过，人工交互未验收 |
| [long-text-benchmark-protocol.md](long-text-benchmark-protocol.md) | 可恢复基线、计时、采样分母、统计与正确性对照 | 承接既有 LT-00 方法；性能验收未关闭 |
| [flow-assembly-reuse.md](flow-assembly-reuse.md) | FR-02 组装内部归因、开放复用反例与性能／留存矩阵 | 归因完成；修订 1 待复核，生产矩阵未执行 |
| [figure-drawing-recording.md](figure-drawing-recording.md) | FR-02 通用 Figure 录制、完整状态、命令共享、外部消费者与峰值留存 | 具体协议候选，待独立复核；未执行 |
| [editor-container-editing.md](editor-container-editing.md) | 容器 targeting、父域坐标、换父/有序插入、嵌套创建与外部策略 | 修订 2 已批准；无窗口已验证，平台待验收 |
| [editor-open-snapping.md](editor-open-snapping.md) | 开放策略、移动／创建／resize、失败、成本及平台矩阵 | 修订 2 已接受；定向通过，完整矩阵未关闭 |
| [p2-s09-service-failure.md](p2-s09-service-failure.md) | Core／Editor／Inspector 服务失败、订阅与 source | A／B／C 完成，D 与整批待补验 |
| [draw2d-official-examples-remaining-contracts.md](draw2d-official-examples-remaining-contracts.md) | 全部 25 个官方入口、共享缺口与 Core/扩展/平台责任矩阵 | 修订 3，已获准实施 |
| [draw2d-official-examples-core-plan.md](draw2d-official-examples-core-plan.md) | Core K00–K09、算法/控件扩展、宿主 H01–H04 的顺序与退出条件 | 已获准按顺序实施 |
| [p2-text-direct-edit.md](p2-text-direct-edit.md) | P2-T02 TextFlow interaction geometry 与 P2-E02 direct edit/IME | `complete` |
| [p2-s06-incremental-failure-isolation.md](p2-s06-incremental-failure-isolation.md) | 新建 Part/Connection 失败清理、Overlay 预检及模型/history 边界 | `complete`，修订 2；组合证明及平台限制永久保留 |
| [p2-s07-query-identity-and-failure-semantics.md](p2-s07-query-identity-and-failure-semantics.md) | 查询身份、合法缺失、测量失败与候选传播 | `complete`，修订 1；实现与行为验证完成 |
| [p2-s08-detached-input-invariants.md](p2-s08-detached-input-invariants.md) | Border／Style 构造、外部准入、溢出与失败恢复 | `complete`；保留回归矩阵 |
| [xtask-command-simplification-2026-10-16.md](xtask-command-simplification-2026-10-16.md) | xtask 三入口、13 个跨边界 suite 与直接 Cargo 门禁 | `complete` |
