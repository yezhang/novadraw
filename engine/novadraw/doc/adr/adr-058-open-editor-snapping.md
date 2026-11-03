# ADR-058：Editor 开放吸附协议

类型：`architecture-decision`

能力归属：Editor；Core 提供既有受检基础协议。

状态：已接受，修订 2，2026-11-02。接受效力与用户条件授权保持。

## 问题

应用各自量化网格不能提供 GEF 框架的可组合吸附；闭合 SnapKind 或内置免检分支
又会使第三方策略失去相同仲裁、错误与成本保证。

## 分层归属

Editor 拥有策略注册、手势、受检候选与仲裁；Core 不新增吸附服务。
Host 提供原生输入与取消；应用拥有 Grid/Guide、业务资格及模型 Command。
必要 Core 契约若缺失，先停止 Editor 新增并建立独立 Core delta。
完整职责表与 GEF → Draw2D → Core 依赖证据见设计和验证入口。

## 决策

所有策略共用 typed identity、payload、registry、候选校验与预算。
候选以未吸附输入求解；同一结果服务预览与提交，多选共享唯一批次锚点。
来源失效取消手势，其他依赖仅在来源有效时重算；release 使用最新输入并完整校验。
耦合候选整体仲裁，布局改写失效明确拒绝，不自动退回未吸附提交。

## 替代与代价

不把 Shapes 网格量化计作框架能力，不在 feedback 重算另一套位置。
严格候选和版本检查有成本；索引复用与流式校验仍须证明全部约束及真实场景收益。

## 关联设计

[开放吸附](../design/editor/open-snapping.md)维护完整行为；
[矩阵](../verification/plans/editor-open-snapping.md)与
[验证状态](../verification/reviews/open-snapping-review-status.md)维护证据和剩余范围。
