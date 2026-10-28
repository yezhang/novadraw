# ADR-047：逐操作抗锯齿与受预算约束的覆盖准备

类型：`architecture-decision`

状态：Accepted，2026-10-26 用户批准按方案修订 2 实施。实施状态只在 P2 backlog 维护。

`api_semantics`：`graphics.context`、`paint.protocol`、`damage.repaint`、
`render.backend_session`。

## 决策

采用 [逐图形 AA 方案修订 2](../design/rendering/shape-antialias-proposal.md)：
`graphics::AntialiasMode::{Default,On,Off}` 分别控制图形与文字覆盖；
Graphics 状态栈保存提示，绘制命令冻结提示。Default 使用后端 session 的稳定配置。
clip、image 和 clear 保留原语义，OFF 只改变该操作的几何覆盖率。

Vello 后端在资源同步前准备整帧 OFF 几何，按设备像素中心计算覆盖，
将连续像素合成整数游程路径；在原命令位置复用 Vello paint、brush transform、
透明度、clip 和混合机制。扫描行、段数、交点、游程、输出元素和缓存均有显式预算。
预算超限通过带命令索引的 `InvalidGraphicsInput` 拒绝，实际预留失败使用
`OutOfMemory`。禁止整帧阈值化、隐式降级、无限 Retry 和依赖 fork。

缓存归 backend session，核对完整几何与变换身份；DPI、采样域和后端重建改变时
失效。默认 AA 不进入 OFF 准备。性能与包体积按相同 release 配置测量，
实验通过不代替生产性能及跨平台门禁。

## 外部消费者证据

独立 integration consumer 使用普通 PaintContext 绘制交错图形、渐变和文字；
CommandRecorder 与 SceneView 保留相同提示和准入要求。第三方 RenderBackend
通过同一 IR 检查逐操作 AA 能力。内置 Shape 与外部 Figure 没有分支特权。
Core 外部消费者及 40 个 ShapeStyles CPU/GPU 场景已通过，证据见
[实施记录](../verification/reviews/per-operation-antialias-implementation-2026-10-26.md)。
平台呈现、完整上游几何对等与高 DPI 连续更新的延迟保证仍未关闭。

## 范围

本决策批准 AA-1 到 AA-4；hairline、低 miter、混合字体装饰仍按独立契约推进。
Core 逻辑宽度 0 仍不绘制。[ADR-049](adr-049-low-miter-and-cocoa-sample-profile.md)
已批准有限非负 miter 和 ShapeStyles 的明确 Cocoa 零宽映射；
该映射不代表通用设备 hairline。混合字体下划线按
[ADR-050](adr-050-prepared-text-underlines.md)使用实际 run 指标与不可变片段。
平台窗口呈现与非本机 runner
继续按发布准备门禁记录，不将未执行验证标为完成。
