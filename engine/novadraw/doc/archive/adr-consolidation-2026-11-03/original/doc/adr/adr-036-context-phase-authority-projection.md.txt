# ADR-036：Context 阶段权限投影与 Query 命名

类型：`architecture-decision`

能力归属：Core。

状态：已接受，2026-10-13。

## 问题

Context 若暴露 Runtime/tree 或全阶段 SceneQuery，会让回调逃逸当前阶段的权限，
把只读计算变成第二套提交入口。

## 决策

Runtime 是 owner，Context 只投影阶段权限，不能逃逸 owner。
Query、Snapshot 与 owned result 分开；Event 分离身份、Query 和 Effects，
Layout/Route 保持 Query → Output，Paint 不获得 SceneQuery。
扩展 capability 使用 typed identity，并按阶段授权。

## 替代与代价

不采用 UniversalContext、全阶段 SceneQuery 或按业务需求不断堆 getter。
阶段接口数量增加，但权限、数据寿命和提交因果可从类型及调用点判断。

## 关联设计

[阶段权限协议](../design/architecture/context-phase-authority.md)
维护命名、能力表、阶段签名与失败；
[查询协议](../design/architecture/query-identity-and-failure.md)维护查询结果语义。
