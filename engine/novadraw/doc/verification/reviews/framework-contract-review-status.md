# 共享场景、表现几何与容器编辑评审结论

类型：`architecture-review`

更新：2026-11-03。能力归属：Core 共享场景／表现几何、宿主背景、
Editor 容器协议分别负责。本页汇总已接受设计的评审结果，不定义跨层新能力。

| 专题 | 最新设计结论 | 实施与验收边界 |
|---|---|---|
| K07 共享场景与宿主背景 | 修订 3 于 2026-10-23 获批，结构依赖、强制准入与继承 alpha 已收口；决策见 [ADR-044](../../adr/adr-044-shared-scene-capture-and-view.md)、[ADR-045](../../adr/adr-045-host-background-submission.md) | 六个切片与 macOS Native 消费已验收；捕获和宿主合成分别验证，不推断其他平台通过 |
| Tree 表现几何 | 修订 3 获批，R01～R10、F01／F02 的时间快照、版本和容量合同已解决；决策见 [ADR-046](../../adr/adr-046-sampled-geometry-and-animation-reservations.md) | T-1～T-4 已完成，Native retained texture 像素与窗口 surface present 分开；[实施复核](tree-layout-presentation-implementation-2026-10-26.md)保留 surface 返回 Skipped 的限制，不关闭 M01-D |
| E03 容器编辑 | ADR-055 修订 2 于 2026-10-29 获批，九项首轮意见及三项修订意见已采纳；决策见 [ADR-055](../../adr/adr-055-container-editing-protocol.md) | 公开协议、移动／创建／drop、外部消费者及部分平台组合已验证；最新 [46 个 Web 检查点与四组成本](editor-container-combinations-2026-11-02.md)不关闭 Native 系统 DnD、跨显示器 DPI、完整 Native 组合和内部工作量 |

K07 分片证据：
[捕获](k07-1-scene-capture-evidence-2026-10-23.md)、
[组](k07-2-scene-group-evidence-2026-10-23.md)、
[视图](k07-3-scene-view-evidence-2026-10-23.md)、
[Thumbnail](k07-4-thumbnail-evidence-2026-10-23.md)、
[背景提交](k07-5-host-background-evidence-2026-10-23.md)、
[背景继承](k07-6-inherit-background-evidence-2026-10-23.md)。

运行时契约仍由所属 design／ADR 定义，状态与恢复条件由
[P2 backlog](../../roadmap/p2-delta-backlog.md)维护。
十一份修订评审／回应已退出当前阅读路径；原判断、字节与 SHA-256 见
[维护前快照](../../archive/knowledge-maintenance-2026-11-03.md)。
