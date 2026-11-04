# ADR-057：服务失败与 Inspector 订阅生命周期

类型：`architecture-decision`

能力归属：Core 服务/observations，Inspector 诊断/恢复，Editor 扩展错误。

状态：已接受，修订 3，2026-10-30。

## 问题

服务失败被默认值吞掉、Inspector 订阅与 Runtime 故障混用、恢复游标缺乏因果，
会使诊断产生额外故障并丢失扩展错误的原始来源。

## 决策

服务使用领域结果，保留原始 source；Inspector 本地故障与 Runtime fault 隔离。
订阅由 consumer 显式拥有，锁竞争不阻塞 Runtime 发布；恢复使用已发布 checkpoint
与游标，失败保留复合诊断。Editor 扩展错误与 panic 按其 owner 记录。

本决策局部修订 ADR-010 的自注销与订阅生命周期约束，具体范围由设计正文定义，
不改写其他观察者合同。

## 替代与代价

不以静默成功、阻塞式诊断回调或全局共享订阅隐藏失败。
非阻塞观测需要丢失/恢复协议与 consumer 本地状态，但不会让诊断阻断引擎。

## 关联设计

[服务、观测与 Inspector 协议](../design/architecture/service-failure-and-inspector-lifecycle.md)
按 owner 定义服务结果、锁竞争、订阅、游标、恢复与扩展 source。
