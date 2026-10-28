# 动画设计入口

类型：`documentation-index`

动画消费已提交事实，只产生可丢弃的 presentation；关闭动画后的结果必须等价于
已提交 source state。所有者、时钟和提交边界由 Runtime 统一维护。

| 阅读顺序 | 文档 | 唯一职责 |
|---:|---|---|
| 1 | [动画与表现平面](animation-system.md) | 正交模型、状态平面、时间、可选性和生命周期 |
| 2 | [公开 API](public-api-contract.md) | identity、plan/track、capture、cancel/retarget 与错误 |
| 3 | [Behavior/Trigger](behavior-trigger-contract.md) | stable fact、scope、coalescing、factory 和触发阶段 |
| 4 | [领域集成矩阵](capability-integration.md) | 属性、布局、route、viewport、持续效果及能力毕业 |

已接受增量：[Tree 表现几何](tree-layout-presentation-proposal.md)，
定义展开/折叠中的真实 bounds、线端点与 viewport 同步；
修订 3 已批准，决策见 [ADR-046](../../adr/adr-046-sampled-geometry-and-animation-reservations.md)，
当前推进 T-1，完整切片状态见 P2 backlog。

决策见 [ADR-026](../../adr/adr-026-animation-and-presentation-plane.md)，
外部事实见 [Draw2D Animation](../../reference/draw2d/figure/animation.md)。
M01-A/B/C/D 的状态与平台证据只在
[P2 backlog](../../roadmap/p2-delta-backlog.md#p2-m01-animation-timeline-与-presentation-plane)维护。
