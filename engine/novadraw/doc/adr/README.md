# 长期架构决策

类型：`documentation-index`

本目录保留 10 项对长期边界有实质影响的选择。
ADR 解释为什么采用某种所有权、依赖、身份或发布模型，以及拒绝的替代和代价；
完整协议、算法、参数、组件组合与平台适配由 [design](../design/00-index.md)负责，
执行状态由 [roadmap](../roadmap/00-index.md)负责。

## 现行决策

| 编号 | 长期边界 |
|---|---|
| [001](adr-001-webgpu-rust-stack.md) | Rust 与 WebGPU 技术栈 |
| [003](adr-003-rust-runtime-and-geometry-boundaries.md) | Runtime 所有权与二维坐标边界 |
| [014](adr-014-extensibility-and-lifecycle-boundaries.md) | 分层原子性、生命周期与稳定发布 |
| [015](adr-015-editor-framework-boundary.md) | Core、Editor 与应用模型边界 |
| [023](adr-023-crate-consolidation-and-extension-boundaries.md) | 发布包、公共路径与第三方依赖边界 |
| [025](adr-025-unified-graphics-and-glyph-preparation.md) | 图文协议、独立录制与后端预处理边界 |
| [026](adr-026-animation-and-presentation-plane.md) | 源状态与动画表现分离 |
| [027](adr-027-open-figure-capability-registry.md) | 类型化身份与统一开放扩展模型 |
| [038](adr-038-query-identity-and-failure-semantics.md) | 身份、合法缺失与领域失败分离 |
| [044](adr-044-shared-scene-capture-and-view.md) | 可变场景所有权与不可变跨 Runtime 捕获 |

这些正文归并已有批准，未重新授予实现授权。52 篇原稿、迁出编号和专题去向见
[归并记录](../archive/adr-consolidation-2026-11-03.md)；此前迁出的 8 篇见
[前次分类](../archive/adr-classification-2026-11-03.md)。
编号不重排、不复用，不为迁出方案保留空壳。

## 新决策门槛

普通方案先进入 design，可以独立接受；评审通过不要求补 ADR。
新增或修改 API、能力、性能策略和篇幅本身均不构成 ADR 理由。
只有无法由既有长期边界表达、确实改变权威／依赖／身份／发布模型的选择，
才单列 ADR。具体规则见[知识维护](../knowledge-maintenance.md#adr-与具体设计分开维护)。

## ADR 模板

```markdown
# ADR-XXX：[长期边界选择]

类型：`architecture-decision`

能力归属：[Core／Editor／backend／platform／应用，跨层逐项区分]
状态：[提议／已接受／已替代；日期与修订]

## 问题

[需要作出的长期选择，不罗列实施缺口]

## 决策

[权威、所有权、依赖、身份或发布边界]

## 替代与代价

[替代为何不采用，承担什么成本和限制]

## 关联设计

[完整协议与验证入口；接受不等于实现完成]
```
