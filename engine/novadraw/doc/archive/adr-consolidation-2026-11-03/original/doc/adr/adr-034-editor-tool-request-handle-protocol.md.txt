# ADR-034：Editor Tool / Request / Handle 统一开放协议

类型：`architecture-decision`

能力归属：Editor。

状态：已接受，2026-10-12。

## 问题

闭合 Request/Tool/Handle 枚举与内置专用 hook 会要求每种扩展修改 Editor，
且内置实现和第三方实现拥有不同派发、失败及生命周期。

## 决策

公开协议始终类型化，擦除只在 registry 内部。Request metadata 与 payload 分离；
Policy 在安装时注册 typed handler，slot 使用开放 key。
Tool 使用统一 object-safe 生命周期，Handle action 使用开放 typed protocol。
内置实现不保留 fast path。

## 替代与代价

不保留闭合业务枚举、typed-only 内置回调或永久双轨兼容。
开放 registry 增加绑定和错误检查，但外部消费者能复用相同工具、策略及事务通道。

## 关联设计

[开放协议](../design/editor/tool-request-handle-protocol.md)定义签名、注册和失败；
[模板与框选](../design/editor/template-creation-and-marquee.md)是具体工具设计。
