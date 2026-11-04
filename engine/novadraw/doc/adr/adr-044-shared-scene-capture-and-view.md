# ADR-044：可变场景所有权与不可变跨 Runtime 捕获

类型：`architecture-decision`

能力归属：Core source Runtime 与不可变捕获；destination 管理只读安装；Host 编排。

状态：已接受，修订 3，2026-10-23。
本次压缩归并仅调整知识表达，不新增架构授权；实现状态以 roadmap／verification 为准。

## 问题

Thumbnail 等视图需要复用一个可变场景；复制业务状态或跨 Runtime
挂载活 Figure 会产生第二份状态权威和冲突生命周期。

## 决策

source Runtime 独占可变树；捕获产生 owned、不可变、受检快照与资源闭包。
destination 只安装只读 view，独立管理资源版本与 lease。
捕获复用稳定化，不推进源帧、不消费 source damage 或确认 cursor。
结构依赖、循环准入与发布前版本检查共同保证安装有效。
新增捕获出口仍服从统一录制和出版边界，不开放内部 recorder 或第二帧入口。

## 替代与代价

跨 Runtime 共享可变 Figure 难以划清提交和销毁权威；
通过 hidden／clip 忽略依赖不能证明结构无环。
不可变快照承担多版本资源留存、保守循环检查、合并和 lease 回收成本；
捕获不保证自动减少所有遍历，性能需由实际负载证明。

## 关联设计

[共享场景与宿主背景](../design/architecture/shared-scene-view-background-proposal.md)、
[资源生命周期](../design/architecture/resource-lifecycle.md)；
统一出版见 [ADR-025](adr-025-unified-graphics-and-glyph-preparation.md)。

原批准范围和修订依据见[原稿 044](../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-044-shared-scene-capture-and-view.md.txt)；
其他原编号与当前入口见[归并记录](../archive/adr-consolidation-2026-11-03.md)。
