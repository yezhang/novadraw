# ADR-030：ResourceProvider 与资源请求 Token 边界

类型：`architecture-decision`

能力归属：Platform 加载，Core 解码与发布。

状态：已接受，2026-10-09。

## 问题

把 I/O provider 放进 Runtime 会使 Core 依赖平台；异步加载若没有请求身份，
旧完成结果可能覆盖新请求或已删除的 Figure。

## 决策

Runtime 不拥有 ResourceProvider；平台负责 I/O 与调度，Core 保留纯解码与受检注册。
每次异步请求携带 Token，完成时由 Core 仲裁身份与有效性。
资源注册只保留 fallible 权威入口。

## 替代与代价

不把 URL 加载、线程或浏览器对象嵌进 Core，也不以资源名称替代请求身份。
平台需维护请求到完成的生命周期，但 Core 可以保持确定性并拒绝过期结果。

## 关联设计

[加载与请求协议](../design/architecture/resource-loading-and-request-tokens.md)
定义 provider、Token、完成拒绝与平台适配；
[资源生命周期](../design/architecture/resource-lifecycle.md)负责已发布资源因果。
