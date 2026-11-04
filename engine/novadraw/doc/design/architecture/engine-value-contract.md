# 引擎基础值与可执行渲染合同

类型：`normative-design`

能力归属：Core Geometry、Graphics、Render；backend 负责 lowering。
规范效力：沿用原编号 020（2026-09-28 已接受）中仍有效的基础值合同；
公开录制边界和包归属分别由
[录制与出版](../rendering/graphics-recording-and-publication.md)及
[ADR-023](../../adr/adr-023-crate-consolidation-and-extension-boundaries.md)负责。

## 受检 Color

Color 分量私有，范围为 `[0, 1]`。`rgba` 明示饱和构造，确定性处理 NaN 和无穷；
`try_rgba` 在需要拒绝非法输入的边界返回结构化错误。
`from_hex`、FromStr 和反序列化复用相同的不变量检查。
只读通道 accessor 与 `to_rgba_array` 不授予修改权限。
饱和构造与拒绝构造语义不同，不能通过公开字段或 serde 绕过。

## 几何词汇与圆弧

Point 表示位置，Vec2 表示位移；`Point - Point -> Vec2`，
`Point +/- Vec2 -> Point`。公开合同使用 Rectangle、Dimension、Insets、Affine2D；
无独立语义的 Precision／Size／Vector／AffineTransform alias 不成为第二套入口。
第三方数学类型仅在私有算法和显式 interop 中使用。

Draw2D 风格 center arc 的角度单位是 degree，anticlockwise 参与 sweep。
center arc 与 SVG endpoint arc 最终归一为同一 backend-neutral Path IR。
圆弧采用受容差控制的 cubic Bezier，不能用固定段数折线冒充曲线。
bounds 覆盖 curve control geometry，不能只统计 endpoint。
非法或退化输入不得生成非有限 path；各原始值边界仍按所属领域返回错误。

## 可执行命令

公开命令必须有生产者和消费者，或在 submission capability 检查时显式拒绝。
backend 完整处理命令和 path operation，不能用 wildcard 静默跳过。
图像 source rectangle 的现行 producer、输入检查和 lowering 由
[图像源区域](../rendering/image-source-rectangle.md)负责。
Graphics／PaintContext 是公开绘制语言，CommandRecorder 负责独立录制；
内部 NdCanvas 不定义公共命名或权限。

## 取舍与验证

受检值、唯一词汇和明确错误要求消费者处理曾被隐去的失败；0.1 阶段直接迁移，
不长期维护无语义差异的兼容 alias。
验证非法 Color 文本／数值／serde，顺逆时针、小弧／大弧／完整圆，
curve bounds、公开第三方类型和 unsupported 命令。
该合同不恢复旧 package 暂缓选择，也不定义应用 schema、serializer 或业务 UI；
产品分工见 [ADR-015](../../adr/adr-015-editor-framework-boundary.md)。

原批准范围、替代及实施证据见
[原稿 020](../../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-020-engine-value-and-render-contract.md.txt)。
