# 通用设备 hairline 候选

类型：`proposal`

能力归属：候选 Core Stroke/帧准备、backend 设备覆盖；Host 提供 surface/DPI。

状态：待独立评审；2026-10-27 的样例批准不包含本增量。

## 问题与候选

SWT Cocoa/GTK 将原始宽度 0 映射为 1，不能据此推出所有 DPI、旋转和非均匀缩放下
恒为一个设备像素。ShapeStyles 的逻辑宽度映射由
[Cocoa profile](low-miter-and-cocoa-sample-profile.md)负责，Core 宽度 0 仍不绘制。

若接受通用 hairline，可用显式 StrokeMetric 区分 logical width 与 device hairline，
保留 `with_width(0)` 的不绘制含义。设备域构造一个像素宽的轮廓，
dash 按声明长度域分段，paint/brush、clip 与透明度保留正确变换；
不得用平均 scale 近似非均匀变换。

## 需先证明的边界

device bounds 依赖受检 surface/DPI，须进入帧准备、旧新 damage 与命中协议；
不能以源码常量扩大所有 Figure bounds。不可逆变换、设备数值不可表示、
预算超限及 backend 不支持均在修改资源或输出目标前结构化拒绝。
这是独立的 surface-aware envelope 取舍，须完成架构评审及各平台证据后才能实施。

只读 SWT 来源及限制见[样例适配](../architecture/official-example-capability-increment-2026-10-26.md#只读依据)。
Win32 GDI/GDI+ 与 DPI 行为尚待核验。性能须测轮廓、run、预算与额外分配，
平台像素未验收时不声明对等，体积仅在用户明确要求时比较。
