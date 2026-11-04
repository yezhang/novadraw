# ADR-029：结构化 RenderBackend 失败与恢复确认

类型：`architecture-decision`

能力归属：Core、backend、platform。

状态：已接受，2026-10-08。

## 问题

成功处置、后端失败及宿主恢复混在一个返回值中，会使跳帧被误当失败、失败帧被误确认，
并把具体 GPU 类型带进 Core。

## 决策

成功 disposition 与失败分类正交；诊断使用 backend-neutral 值。
Runtime 的完成确认是帧恢复权威，backend 给出失败与恢复信息，host 决定调度。
Vello 的具体错误在 backend 内转换，保留诊断而不暴露第三方类型。

## 替代与代价

不以 bool、字符串或统一 panic 表示全部结果；不让 host 绕过 Runtime 清除恢复状态。
结构化结果增加类型与调用处理，但允许跳帧、重试、会话重建被独立判断，
避免确认尚未发布的像素。

## 关联设计

[失败与恢复协议](../design/rendering/backend-failure-and-recovery.md)
维护 disposition、失败、确认及 host 调度表。
