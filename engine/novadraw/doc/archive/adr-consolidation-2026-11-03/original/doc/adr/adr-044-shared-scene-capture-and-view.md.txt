# ADR-044：共享场景捕获与只读视图

类型：`architecture-decision`

能力归属：Core scene；Host 编排 Runtime。

状态：已接受，2026-10-23，修订 3。

## 问题

Thumbnail 等视图需要观察同一可变场景；复制业务场景或跨 Runtime 挂载活 Figure
都会产生第二份状态权威及生命周期冲突。

## 决策

source Runtime 独占可变树；捕获产生 owned、不可变、受检快照与资源闭包。
destination 只安装只读 view，独立管理资源版本与 lease。
结构循环准入统计整棵捕获子树，共享组提交保护与本地版本检查一起完成。
capture 复用稳定化而不推进源帧、不消费其 damage 或确认 cursor。

此决定为 ADR-031 增加受检 SceneSnapshot 出口；ADR-033 组合根仅新增 `scenes()`，
方法预算从 27 调整为 28。不开放内部 recorder 或第二个帧提交入口。

## 替代与代价

不跨 Runtime 共享可变 Figure，不以隐藏/clip 过滤循环依赖。
快照和多版本资源有留存成本；有界合并、关闭交接与 lease 回收必须显式管理。

## 关联设计

[共享场景与宿主背景](../design/architecture/shared-scene-view-background-proposal.md)
维护捕获、资源闭包、循环准入、配置与安装；宿主背景的独立选择见 ADR-045。
