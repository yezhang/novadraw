# 12. 类型化 Capability 与组件更新

可扩展框架容易走向两个极端：基础 trait 持续膨胀，或者所有行为都退化成不受检的动态
类型查询。

本章解释：

- 基础 Figure 协议的最小职责；
- 横切 Capability 的独立消费者判据；
- 共享数据模型为什么不等于行为能力；
- 具体 Figure 私有状态与 typed component update；
- CapabilityKey、owned descriptor 与 attach-time registration；
- 类型擦除边界、借用限制和 Runtime mutation。

核心结论：

> Capability 用于跨类型发现统一语义；结构体和组件更新用于表达具体类型的数据与变化。

本章应给出一棵决策树，分别判断 Input、Lifecycle、Connection、PointList、TextFlow
和 Viewport 应属于哪种扩展对象，并说明错误分类如何导致中心接口重新闭合。
