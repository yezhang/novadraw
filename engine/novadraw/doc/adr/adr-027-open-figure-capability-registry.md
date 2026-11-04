# ADR-027：类型化身份与统一开放扩展模型

类型：`architecture-decision`

能力归属：Core／Editor 的开放行为身份与注册；各领域拥有自己的 payload 和错误。

状态：已接受，2026-10-08；沿用原编号 028／033／034 的统一扩展修订。
本次压缩归并仅调整知识表达，不新增架构授权；实现状态以 roadmap／verification 为准。

## 问题

中心枚举、专用 accessor 和内置特权路径会让外部扩展持续修改框架；
公开 Any／downcast 又把类型安全和错误边界交给每个消费者。

## 决策

内置与外部共用 typed identity、payload／descriptor、registry、
发现与提交协议；不得为内置保留闭集、typed-only hook 或快捷提交通道。
registry 归具体 owner，不使用全局类型目录；owned descriptor 不保存活借用，
安全适配在内部完成，消费者不依赖公开 downcast。

横切 capability 要有独立跨类型消费者；具体私有内容使用 typed component update。
两者共享预检、NoChange、commit、revision 与通知。属性行为用 typed key，
字符串只作诊断。开放扩展不授予改写固定 scheduler 或任意插入 pipeline 的权限。

## 替代与代价

不断扩大 Figure accessor／中心枚举不能支持独立外部扩展；
统一万能方法集合会擦除领域合同。代价是 descriptor 留存、key lookup、
adapter 与领域错误的维护；新行为仍需真实消费者和成本证据，
同名诊断文本不构成行为等价，TypeId 不作持久化身份。

## 关联设计

[Capability 模型](../design/architecture/figure-capability-model.md)、
[属性身份](../design/architecture/typed-property-identity.md)、
[组件更新](../design/architecture/component-update.md)、
[Editor 交互协议](../design/editor/tool-request-handle-protocol.md)。

原批准范围和修订依据见[原稿 027](../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-027-open-figure-capability-registry.md.txt)；
其他原编号与当前入口见[归并记录](../archive/adr-consolidation-2026-11-03.md)。
