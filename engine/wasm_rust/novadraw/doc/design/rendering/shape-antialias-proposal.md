# 逐图形抗锯齿方案

类型：`proposal`

状态：待评审，未实施。

目标：K08 ShapeStylesExample 的 AA ON/OFF/DEFAULT 完整绘制链路。

`api_semantics`：`graphics.context`、`paint.protocol`、`render.backend_session`、
`damage.repaint`。

## OFF 必须改变覆盖率

关闭抗锯齿时，每个设备像素只按中心采样判断是否落在图形内部，几何覆盖率为 0 或 1。
paint alpha、全局 alpha 和 clip 的覆盖率继续独立参与合成。半透明图形的最终 alpha
可以介于 0 和 1，不能用最终 alpha 的阈值处理代替几何采样。

Draw2D 的 `SWTGraphics` 分别保存图形 AA 与文字 AA，`reconcileHints` 独立调用
`GC.setAntialias` 与 `GC.setTextAntialias`。其状态栈保存并恢复这些提示。
本地 Vello 参考的 `AaConfig` 只有 Area、Msaa8、Msaa16，三者都抗锯齿；
现有选择既没有 OFF，也不能给同一帧中不同图形分别选择。

本增量需要 Graphics 状态、Render IR、能力准入与具体后端一起变更。仅增加 setter
或结构化拒绝不足以完成 ShapeStyles。不得将 Area 重新命名为 OFF，也不得修改整帧
像素来模拟逐图形提示。

## Graphics 保存提示，命令冻结提示

建议增加 backend-neutral `AntialiasMode::{Default, On, Off}` 值，分别作为图形
和文字状态。它们属于有限的绘制语义值，不是 Figure 类型或扩展角色分类。
规范导入路径为 `graphics::AntialiasMode`，不增加根目录平铺导出。

PaintContext 与 CommandRecorder 共用同一 API；push、restore、pop 保存两个提示。
每条实际 draw command 冻结对应提示：

| 绘制内容 | 使用的提示 |
|---|---|
| fill/stroke Rect、Ellipse、Line、Polyline、Path | 图形 AA |
| DrawGlyphRun 的 fill/stroke | 文字 AA |
| Image 与 SVG 栅格 | 沿用已有图片采样契约，不把图形 AA 当成 SVG 重栅格化配置 |
| clip | 保持既有裁剪语义；图形 OFF 不关闭父 clip 的 AA |

Default 由 backend session 的稳定配置解释，默认 Vello 保持目前 Area 结果。
Default 不从某条先前命令继承，也不随宿主临时全局状态改变。
ShapeStyles 每个单元独立设置提示，标题文字继续采用独立文字 AA。

实际命令要求逐操作 AA 能力。Runtime 在发布前检查，backend 在资源与像素修改前
检查；外部 recorder、SceneView 回放与普通 Figure 使用相同能力和错误通道。
不支持 OFF 的后端返回 `UnsupportedRenderCapability`，不接受该帧，也不降级为 On。

## 后端需要一次独立可行性切片

建议在 `novadraw-backend-vello` 中增加逐操作几何覆盖路径。先验证最小 OFF 图形，
再扩展到全部 shape。Core 只描述提示，不持有纹理、设备尺寸或 GPU mask 类型。

完整方案按命令顺序生成 device-space 二值几何 mask，将相同 paint、alpha、transform
和既有 clip 合成到目标。On/Default 继续使用正常 Vello 覆盖结果。
两个路径必须在同一帧保持原有绘制顺序，不能把所有 OFF 图形收集到帧末统一覆盖。
分段提交、离屏目标与 mask 均由 backend session 管理，不新增全局状态。

可行性切片必须证明以下机制，然后才能固定 GPU 实现：

- 二值 mask 按像素中心生成，覆盖填充规则、stroke cap/join/dash 与非整数变换。
- Vello 分段输出与 mask 合成保持 clip、alpha 和混合顺序，重叠的 On/Off 图形正确。
- 临时纹理与像素预算有明确上限；分配失败不部分提交资源或目标像素。
- surface DPI、resize、session reset 与 Retry 使用既有帧恢复路径。
- Native 与 WebGPU shader、绑定和纹理格式均可用，不能只做 macOS 专用路径。

若分段合成无法可靠保留 Vello 的图层语义，则停止该切片，回到 Design：
比较向 Vello 增加逐 draw 覆盖标记和独立 raster backend 的成本。不能在执行中
隐式引入依赖 fork。当前方案不承诺未经验证的 shader 或现成 Vello 接口。

## Hairline 与低 miter 单独决策

已接受的 [Graphics 契约](p2-g01-graphics-extension.md)规定逻辑宽度 0 不绘制，
miter limit 至少为 1。本增量保留这两个既有语义。

ShapeStyles 的设备 hairline 如需实现，建议通过显式的 stroke width 语义值表达，
区分 `Logical(width)` 与 `Hairline`，不把旧宽度 0 改成设备像素线。
Hairline 的最终轮廓与 damage envelope 依赖 DPI 和变换，必须先定义非均匀缩放、
旋转、dash 和边界测量，再修改公共 StrokeStyle。它不随 AA setter 自动获得支持。

低 miter 的上游行为需要独立证据与准入范围决定。现有 `< 1` 错误继续有效；
不能把示例中的输入静默夹到 1 后声称对等。

## 外部消费者证据

第一份消费者在独立测试 crate 中实现自定义 Figure，通过普通 PaintContext 在
同一帧绘制交错的 On/Off 填充、描边、渐变和文字。它不修改 Core 注册逻辑，
不检查 Figure 名称，不使用 ShapeStyles 私有接口。

第二份消费者使用 CommandRecorder 保存该绘图，再通过 SceneView 回放。
两者输出相同提示、能力需求和像素结果；第三方 RenderBackend 也能读取同一 IR，
独立拒绝不支持的提示。内置五种 Shape 与外部 Figure 共用全部绘制机制。

## 像素判据与退出顺序

在明确白色背景上绘制斜线、曲线和带亚像素偏移的路径。OFF 的几何 mask 只能有
0/1 覆盖；On 的边缘必须出现部分覆盖。测试直接比较生成的覆盖 mask，避免把
paint alpha、clip alpha 或颜色量化误判为几何覆盖率。
另测同色半透明图形，确认 OFF 仍保留 paint alpha；改变图形 AA 时，文字区域的
像素必须保持不变。

| 顺序 | 切片 | 退出证据 |
|---|---|---|
| AA-1 | 固化提示、状态栈、IR 与能力失败契约 | 外部 Figure/recorder、状态恢复、整帧拒绝、Native/Wasm API |
| AA-2 | backend 可行性与二值覆盖 | 斜线/曲线 mask、On/Off 顺序、透明 paint 与 clip、Native/WebGPU |
| AA-3 | 五种 Shape、路径、dash/gradient 与文字独立性 | 同帧混合像素、变换/DPI、resize/Retry/reset、旧 AA 默认图像不变 |
| AA-4 | ShapeStyles 原始矩阵和清理恢复 | 五种 Shape 各单元、真实参数面板、移除旧像素与完整 Canvas 验收 |

Hairline 与低 miter 进入各自评审后切片，不借 AA-4 隐式扩大范围。
最终修改须同步 ADR、Graphics 设计、书籍、API 快照、parity 和验证入口。

参考：本地 `org.eclipse.draw2d/SWTGraphics.java` 的 `reconcileHints`、
`setAntialias` 与状态栈；本地 `vello/vello/src/lib.rs::AaConfig`；
[ADR-024](../../adr/adr-024-graphics-paint-stroke-and-clipping.md)、
[ADR-025](../../adr/adr-025-unified-graphics-and-glyph-preparation.md)。
