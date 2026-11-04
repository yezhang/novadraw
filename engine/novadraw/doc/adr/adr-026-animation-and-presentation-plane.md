# ADR-026：源状态与动画表现分离

类型：`architecture-decision`

能力归属：Core Runtime 动画与 presentation；Host 驱动时间；Editor／应用拥有业务状态。

状态：已接受，2026-10-06；后续表现几何及受检值修订由关联设计维护。
本次压缩归并仅调整知识表达，不新增架构授权；实现状态以 roadmap／verification 为准。

## 问题

各 Figure 私有 timer、阻塞动画和每帧源状态写入会破坏确定性驱动、
通知因果以及禁用动画时的业务一致性。

## 决策

Runtime 独占统一动画服务；Host 使用同一单调时间驱动，不设全局时钟或第二循环。
committed source 直接达到业务最终值，动画只生成可丢弃 presentation，
不成为布局、路由、通知或 history 真值。
目标、运动、触发与组合按正交维度扩展，使用 typed 通道和同一开放模型。

隐式动画只由应用显式安装；禁用或无 active timeline 时不持续 tick、扫描或提交空帧。
sampled effect 进入既有 damage／recording 合同；交互表现几何须显式声明。
cancel 清除覆盖，失败 retarget 保留旧计划，不回滚 committed source。

## 替代与代价

复制 Draw2D 全局 animator 或 Figure 私有 timer 破坏多 Runtime 隔离；
仅 backend shader 不能统一布局、取消和 Headless 行为。
该选择增加 timeline、表现状态与容量管理，复杂拓扑不自动获得通用插值，
持续动画仍承担可见性、预算与 reduced-motion 成本。

## 关联设计

[动画系统](../design/animation/animation-system.md)、
[公开合同](../design/animation/public-api-contract.md)、
[表现几何](../design/animation/tree-layout-presentation-proposal.md)。

原批准范围和修订依据见[原稿 026](../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-026-animation-and-presentation-plane.md.txt)；
其他原编号与当前入口见[归并记录](../archive/adr-consolidation-2026-11-03.md)。
