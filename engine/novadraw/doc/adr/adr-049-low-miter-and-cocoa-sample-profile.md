# ADR-049：低 miter 与样例零宽映射

类型：`architecture-decision`

状态：Accepted，2026-10-27 用户批准能力补充方案。

`api_semantics`：`graphics.context`、`paint.protocol`、`damage.repaint`。

## 决策

StrokeStyle 接受有限非负 miter limit，保留原始值。负数、NaN 和无穷大拒绝，
失败不改变 Graphics 或 Figure 状态。`effective_join()` 在 Miter 且 limit 小于 1
时返回 Bevel；其他 join 保持原值。普通和 AA OFF 后端使用此规则，
visual bounds 与 damage 使用相同扩张。缓存、通知和诊断保存原始参数。
不修改递归渲染主循环。

Core 逻辑宽度 0 仍不绘制。ShapeStyles 明确采用 Cocoa profile：
原始宽度 0 映射为 1 逻辑单位；零宽标准 dash 分别使用
DOT `[3,3]`、DASH `[18,6]`、DASHDOT `[9,6,3,6]`、
DASHDOTDOT `[9,3,3,3,3,3]`。控制面板说明映射，
不能据此宣称通用设备 hairline 或跨平台像素对等。

本决策细化 ADR-024，替代 ADR-047 中 miter 至少 1 和样例零宽不支持的限制。
来源为方案记录的 Draw2D 与 SWT Cocoa 固定提交，只读分析。

## 外部消费者证据与成本

公开 StrokeStyle、外部 Figure 和 scoped mutation 验证原值、错误原子性、
轮廓边界与旧新 damage。后端验证低 miter 与显式 bevel 在相同几何下像素一致，
覆盖 AA ON/OFF、开闭路径、反向和锐角。ShapeStyles 使用同一公开 Graphics。

不增加依赖或资源。有效 join 是常数时间判断，缓存身份保留原参数。
最终验证与成本记录归本批实施记录；平台原生栅格仍需单独验收。
