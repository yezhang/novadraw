# ADR-032：Viewer 编辑会话与宿主驱动边界

类型：`architecture-decision`

能力归属：Editor；应用拥有模型，宿主持有驱动能力。

状态：已接受，2026-10-09。

## 问题

Viewer 若同时公开模型可变引用、命令状态和完整 Runtime，消费者可以绕过历史、
同步与宿主权限边界。

## 决策

模型编辑通过 ViewerEditSession 关闭命令模型事务，不公开裸可变引用。
ViewerDrive 只投影宿主驱动所需能力；CommandStack 的纯查询与扩展检查分开。

## 替代与代价

不以完整 Runtime 或模型可变借用作为便利入口，也不以 drop 的隐式副作用表示成功。
显式会话增加调用步骤，但事务失败、模型发布与驱动权限可以独立判断。

## 关联设计

[会话与 drive 协议](../design/editor/viewer-edit-session-and-drive.md)维护具体接口；
[Editor 架构](../design/editor/architecture.md)维护总体身份与所有权。
