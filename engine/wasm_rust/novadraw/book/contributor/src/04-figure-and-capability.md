# 4. Figure 协议与 Capability

> **本章解决的问题**：新增 Figure 行为时，如何区分基础协议、横切能力、共享数据模型
> 和具体类型更新，避免持续扩张中心接口。

## 4.1 固定管线与差异行为

Figure 参与绘制、测量和命中的基础协议，但不拥有遍历、校验、damage、通知或帧发布
顺序。Runtime 保持固定执行管线，Figure 只提供当前阶段所需的差异行为。

这意味着新能力不能通过覆盖主流程改变执行顺序。需要副作用时，扩展产生类型明确的
候选变化，再由 Runtime 统一提交。

## 4.2 四类扩展对象

| 类别 | 判定 | 示例 |
|---|---|---|
| 基础协议 | 每个 Figure 都必须参与 | 绘制、测量、精确命中 |
| 横切 Capability | 独立消费者需要跨类型发现 | Input、Lifecycle、Accessibility、Connection |
| 共享数据模型 | 多个具体类型复用结构，但无统一消费者 | PointList |
| 具体组件 | 只属于某个 Figure 的私有状态 | TextFlow、Label、Image 数据 |

共享结构不因为被多个类型使用就自动成为 Capability。只有独立消费者需要在不知道
concrete type 的情况下发现和调用统一语义，才进入 registry。

## 4.3 Capability Registry

Figure 在 attach 时通过 `FigureCapabilityBuilder` 登记 owned descriptor。节点持有
冻结后的 capability set，消费者使用 `CapabilityKey<C>` 查询类型明确的 descriptor。

关键约束：

- descriptor 不保存 Figure 或 Runtime 的借用；
- 调用方不接触 `Any`、`TypeId` 或未受检 downcast；
- registry 属于单个 FigureNode，不使用进程全局注册；
- duplicate descriptor 在拓扑发布前失败；
- capability absent 是合法结果，identity 或类型不一致是结构化错误；
- 未知 capability 只有持有对应 typed key 的消费者才会执行。

完整合同见：

- [`ADR-027：开放的 Figure Capability Registry`](../../../doc/adr/adr-027-open-figure-capability-registry.md)
- [`Figure Capability 统一扩展模型`](../../../doc/design/architecture/figure-capability-model.md)

实施或迁移前必须同时核对当前 roadmap 和代码，不能把已接受但尚未完成的设计写成
现有实现。

## 4.4 Capability 与组件更新

两条路径解决不同问题：

```text
跨类型发现和调用
-> CapabilityKey<C>
-> typed descriptor

具体 Figure 私有状态修改
-> FigureComponentUpdate<T>
-> prepare
-> Runtime commit
```

例如 Polyline 的点集合属于具体数据，可通过 `SetPolylinePoints` 一类 typed update
修改；它不应为了复用字段访问而产生 `PointListFigureBehavior`。

## 4.5 新增能力的评审清单

1. 是否存在独立于具体 Figure 的消费者？
2. 是否至少存在多个语义一致的实现或明确外部替换需求？
3. descriptor 的身份和错误是否类型明确？
4. prepare 是否只产生候选结果？
5. invalidation、damage、revision 和 notification 是否仍由 Runtime 提交？
6. detached、attached 和 destroyed 阶段分别允许什么？
7. 外部 crate 能否在不修改 Core 中心枚举的情况下实现和消费它？

若答案只涉及代码复用，应优先使用结构体、泛型函数或组件更新，而不是新增 behavior
trait。
