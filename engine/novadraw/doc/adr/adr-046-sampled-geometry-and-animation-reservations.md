# ADR-046：表现几何与动画容量预留

类型：`architecture-decision`

能力归属：Core Animation/Presentation；Tree 是消费者。

状态：已接受，2026-10-24，修订 3。

## 问题

布局、绘制、命中与 viewport 若各自读取不同动画几何，表现会互相矛盾；
活动计划若不预留容量，又会因执行顺序被驱逐。

## 决策

源状态保持 committed 权威，Runtime 维护统一只读表现几何版本。
绘制、clip、可见性、damage 与 viewport 消费同一版本，采样与帧确认分开。
typed channel/provider 同一开放模型；计划准入预留上限，retarget 按净容量替换，
自身违约与共享容量不足分别归因。

## 替代与代价

不写回 source，不用宽高比缩放冒充真实 bounds，不新增 Tree 特权或遍历主线。
统一几何与容量预留增加活动期快照成本；无动画时不构造快照或增加派生扫描。
保留旧 BoundsTransition 与 viewport affine transition 含义。

## 关联设计

[Tree 表现几何](../design/animation/tree-layout-presentation-proposal.md)
维护版本、capture、几何 provider、容量、输入接管及验证。
