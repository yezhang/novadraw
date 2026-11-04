# ADR-040：连接组的统一路由求解

类型：`architecture-decision`

能力归属：Core Connection。

状态：已接受，2026-10-21。

## 问题

逐条 Router 无法表达依赖完整连接集合与障碍集合的求解；在示例中维护私有组状态，
会绕过依赖跟踪、失败隔离和原子路线发布。

## 决策

在 Connection 域提供开放组求解协议，输入为完整受检组，输出为完整候选结果。
Runtime 跟踪组依赖并原子发布；组内任一失败不发布部分路线。
内置 visibility 算法与外部 solver 共用注册、输入及结果验证。

## 替代与代价

不以逐条调用共享可变 singleton 模拟组求解，也不让 Obstruction 私自协调路线。
完整组准备有额外内存与求解成本，因此预算与结果完整性必须在发布前检查。

## 关联设计

[组求解与 visibility](../design/architecture/group-route-solving.md)
维护具体输入、算法、生命周期和失败；
[连接总览](../design/architecture/connection-routing.md)维护基础路由。
