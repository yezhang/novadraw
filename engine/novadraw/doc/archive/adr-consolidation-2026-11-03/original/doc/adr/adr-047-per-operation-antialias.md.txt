# ADR-047：逐操作抗锯齿与受预算约束的覆盖准备

类型：`architecture-decision`

能力归属：Core Graphics/IR，Vello 覆盖准备。

状态：已接受，2026-10-26，修订 2。

## 问题

整帧阈值化无法保持交错图形、文字与透明混合的逐操作语义；
OFF 覆盖若无预算，可产生无限准备成本。

## 决策

图形与文字独立冻结逐操作 AA 提示；Default 使用稳定 session 配置。
OFF 几何在资源同步前完成有限覆盖准备，仍使用原命令位置的 paint、clip 与混合。
预算超限或分配失败结构化拒绝，不隐式降级、无限 Retry 或 fork 依赖。

## 替代与代价

不整帧阈值化，不以关闭全部后端 AA 代替命令语义。
设备覆盖准备有扫描、缓存及输出成本，需按完整几何、变换、DPI 与 session 失效。

## 关联设计

[逐操作 AA](../design/rendering/shape-antialias-proposal.md)维护覆盖算法与预算。
[低 miter/profile](../design/rendering/low-miter-and-cocoa-sample-profile.md)
修订原样例限制；[字体下划线](../design/rendering/prepared-text-underlines.md)
另行维护装饰协议。通用设备 hairline 尚未接受。
