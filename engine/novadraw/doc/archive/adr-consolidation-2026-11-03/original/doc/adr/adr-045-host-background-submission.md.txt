# ADR-045：宿主背景统一帧提交

类型：`architecture-decision`

能力归属：Host 加载/候选，Core 冻结/提交，backend 合成。

状态：已接受，2026-10-23，修订 3。

## 问题

透明 Canvas 需要透出宿主背景；若宿主在帧准备后追加背景，资源、DPI 和位置版本
会脱离 Runtime 的统一确认。

## 决策

Host 提供不可变候选、请求身份、坐标版本与资源闭包，保留平台 I/O 与调度。
destination Runtime 是唯一提交权威，一次冻结背景、Canvas、lease 与 surface 输入；
backend 消费同一个受检 submission。背景不叠加内容 viewport origin。
背景更新/移除独立唤醒，重试与 reset 保留受检结果和必要 lease。

## 替代与代价

不建立宿主私有合成出口，不把 ResourceProvider 放进 Core。
首批采用 Full damage，有额外录制成本；局部修复需另有契约和证据。

## 关联设计

[共享场景与宿主背景](../design/architecture/shared-scene-view-background-proposal.md)
维护请求、版本、背景帧与后端准入。资源导入沿用 ADR-044。
