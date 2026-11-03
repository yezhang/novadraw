# ADR-037：Editor 增量视图失败隔离

类型：`architecture-decision`

能力归属：Editor 投影与扩展生命周期；Core disposal。

状态：已接受，2026-10-15；矩阵修订 2 的效力保持。

## 问题

增量投影会调用外部构建、激活与清理代码。全局同步回滚无法撤销这些私有副作用，
而失败后继续发布新增资源会留下不完整的 Part、Connection 或 Overlay。

## 决策

以当前增量新增资源为隔离单位，拒绝全局同步回滚。
Part 子树与连接晚期失败分别清理；Overlay 先完整预检、再发布。
Editor 生命周期与 Core disposal 分开协调，每次激活与清理尝试只记录一次。
模型/history 不因视图恢复被改写。

## 替代与代价

不以全 Viewer 快照恢复伪装跨外部代码原子性，也不在清理失败后继续新增工作。
失败路径可能进入 fault 并要求宿主恢复；清理失败须保留错误并停止新增工作。

## 关联设计

[失败隔离协议](../design/editor/incremental-failure-isolation.md)
维护 Part、Connection、Overlay、清理权限与恢复行为。
