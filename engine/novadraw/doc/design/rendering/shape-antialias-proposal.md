# 逐图形抗锯齿方案

类型：`proposal`

状态：修订 2 已于 2026-10-26 获用户批准，按
[ADR-047](../../adr/adr-047-per-operation-antialias.md)进入实施。

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

具体接口建议为 `PaintContext::set_antialias(AntialiasMode)` 和
`set_text_antialias(AntialiasMode)`，配套只读查询；CommandRecorder 返回的
PaintContext 使用同一入口。提示初值均为 Default，setter 只更新受限绘制状态，
不查询某个后端或修改 Figure 源状态。有限枚举值本身不存在输入校验失败。
IR 在上述 shape draw 与 DrawGlyphRun 的 payload 中增加 `antialias` 字段；
clip、image、状态栈命令不增加语义无关字段，也不依靠回放时的隐式 AA 状态继承。
SceneView 克隆命令时保留该字段，外部消费者可以直接读取已冻结的值。

Default 由 backend session 的稳定配置解释，默认 Vello 保持目前 Msaa16 结果。
Default 不从某条先前命令继承，也不随宿主临时全局状态改变。
ShapeStyles 每个单元独立设置提示，标题文字继续采用独立文字 AA。

实际命令要求逐操作 AA 能力。Runtime 在发布前检查，backend 在资源与像素修改前
检查；外部 recorder、SceneView 回放与普通 Figure 使用相同能力和错误通道。
不支持 OFF 的后端返回 `UnsupportedRenderCapability`，不接受该帧，也不降级为 On。

## 设备像素游程复用 Vello 合成

修订 2 建议先采用设备像素中心采样与整数游程路径。backend 将 OFF 图形的最终
轮廓变换到 device space，按每个像素中心计算 winding；连续内部像素合并成水平
矩形游程，全部游程合成一个 NonZero 路径。该路径使用整数坐标和单位变换，
交给生产 Vello Msaa16 绘制。整数边界对应完整设备像素，几何覆盖率因此为 0 或 1。
Core 仍只描述 AA 提示，不持有设备像素、Kurbo 路径或缓存。

每条 OFF draw 在原来的命令位置绘制一个游程路径，使用原 paint、全局 alpha、
clip 栈和混合顺序。渐变的 brush transform 保留原 local-to-device 映射，
几何改成单位变换时不能同时重置 brush transform。文字读取自己的 AA 提示，
图形 OFF 不修改字形或父 clip 的覆盖率。

这条路径使用项目已有的 Vello/Kurbo，不增加依赖 fork、shader、分段合成或专用
mask 纹理。On/Default 保持当前 lowering；OFF 的差异集中在几何准备。
stroke 先在原逻辑坐标域按现有宽度、cap、join、miter、dash 展开轮廓，
再变换到设备域。不能先变换中心线再按同一个标量宽度描边，否则非均匀缩放
会改变现有逻辑描边语义。

独立实验使用当前依赖 `vello 0.10.0`，在 Apple M1 Pro / Metal 上通过：
变换圆形的 4,096 个像素符合中心采样，ON 有 156 个部分覆盖像素；
半透明 OFF 保留 paint alpha；曲线 clip、layer alpha 与前后 ON 图形交错绘制
时，游程路径和逐整数像素参考的最大通道差为 0。描边和渐变的补充结果见
[后端实验记录](../../verification/reviews/shape-aa-feasibility-2026-10-26.md)。

实验只证明现有 Vello 可以合成这种几何。实验中的逐像素 `winding` 是固定
64×64 范围的参考算法，不能进入生产路径。它尚未证明全部 ShapeStyles、
EvenOdd、自交边界、文字 OFF、WebGPU、retained-partial 或最终性能。
Vello 自定义逐 draw shader 和独立 raster backend 只作为复核失败后的备选；
切换路线必须重新评审，不能在实施时隐式引入。

## 准备预算与失败必须先于资源提交

生产准备使用扫描线交点，先按 y 单调段拆分曲线，再维护活动边；
在 `y + 0.5` 求交并按填充规则生成整数区间。顶点采用半开边规则，水平边、
相切点、多重根与正好落在边界上的像素必须有明确归属及独立用例。
曲线容差由设备空间精度约束，不能因 mask 分辨率调整而静默改变几何。
将轮廓限制到 surface 的可见扫描行时，仍须保留原轮廓的 winding；
不能简单删除 surface 外的边。

最坏游程数可达到可见像素数，复杂度不能只按路径长度估算。backend session
需提供显式配置的扫描行、曲线段、交点、游程、输出 PathEl 与缓存字节预算，
同时约束单命令和整帧工作。预算由消费场景和实测决定，本方案不填写未经测量的
通用阈值。算法必须在增加元素前检查剩余预算，使用受检加法和可失败容量预留；
不能先生成任意大路径再检查总数。

顺序固定为：校验普通输入及 AA 能力 → 准备整帧 OFF 几何并完成预算检查 →
同步资源 → 编码并提交目标。失败前可以丢弃临时几何，不能部分接受该帧的
资源或写入 retained target。现有后端是在普通输入校验后同步资源，
因此 AA 准备必须插入资源同步之前。

建议将确定性预算超限作为带 `command_index` 的 `InvalidGraphicsInput`：
增加 `GraphicsInputError::AntialiasPreparationLimit`，报告预算种类、请求量和上限。
它通过现有 `RenderFailure::InvalidInput` 通道拒绝该帧，不伪装成 GPU 失效或
无限 `Retry`。实际容量预留失败使用现有 `OutOfMemory`。新增诊断值属于有限的
错误分类，不承担 Figure/Capability 身份。此错误契约已随 ADR-047 获批；
不以 AA 名义实现尚未批准的 P2-S08 descriptor-specific admission。

缓存由 backend session 持有。key 包含完整几何、填充规则或 StrokeStyle、
有效变换、surface DPI 与采样域；hash 命中后核对完整身份，不能仅凭 hash 复用。
paint、clip 和 damage 不改变几何采样，不能让 partial clip 的起点改变像素相位。
原轮廓的保守 envelope 要向设备像素边界外扩后参与 damage，旧新区域均须覆盖。
resize/DPI/session reset 清理不兼容缓存，Retry 不发布半成品。超出缓存容量
可以淘汰已完成项，但不能绕过单帧准备预算。

## 性能与包体积退出条件

游程路径通常每条扫描行只有少量矩形，但极端细碎路径可能生成大量 PathEl，
增加 CPU 准备、GPU flatten/binning、上传及 retained 内存。生产扫描线的成本
由输入段数、求交数、排序与输出游程数共同决定；输出至少是 O(游程数)。
缓存只减少重复几何准备，不能消除首次提交或连续变换的成本。
默认 On/Default 不进入扫描线，也不分配 OFF 路径。

实验的 CPU 时间只记录 dev 构建的单次参考采样，不能用来宣称性能合格。
实施后必须比较相同 release 配置的 Native/Wasm 产物与传输压缩体积，
以及默认 AA、25 个矩阵单元、复杂 dashed path、连续变换与高 DPI 场景。
记录冷/热缓存准备时间、P95 帧耗时、输出元素数、上传字节与峰值内存；
不以“没有新增依赖”代替最终包体积测量。

若预算内的代表性场景仍无法满足既定消费预算，则 AA-2 不通过，
返回 Design 比较 GPU 逐 draw 覆盖路线。Core setter 通过不代表后端或入口完成。

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
| AA-0 | 独立设备游程实验 | `complete`，只证明 Native Vello 合成；不关闭生产能力 |
| AA-1 | 固化提示、状态栈、IR 与能力失败契约 | 外部 Figure/recorder、状态恢复、整帧拒绝、Native/Wasm API |
| AA-2 | 有预算的扫描线、轮廓准备、缓存与合成 | 斜线/曲线 mask、EvenOdd/边界、On/Off 顺序、透明 paint 与 clip、Native/WebGPU |
| AA-3 | 五种 Shape、路径、dash/gradient 与文字独立性 | 同帧混合像素、变换/DPI、resize/Retry/reset、旧 AA 默认图像不变 |
| AA-4 | ShapeStyles 原始矩阵和清理恢复 | 五种 Shape 各单元、真实参数面板、移除旧像素与完整 Canvas 验收 |

Hairline 与低 miter 进入各自评审后切片，不借 AA-4 隐式扩大范围。
最终修改须同步 ADR、Graphics 设计、书籍、API 快照、parity 和验证入口。

实施结果见 [AA-1–AA-4 实施记录](../../verification/reviews/per-operation-antialias-implementation-2026-10-26.md)：
Native 离屏像素、40 个受支持矩阵场景和 Web 编译已验证。成本已按 release
测量，持续变换 DPI 2 的帧等待 P95 为 19.7–28.9 ms，未保证 60 fps。
编码流大小已记录，驱动实际上传、窗口 present、浏览器像素及输入延迟仍未测量。
AA-4 关闭既定自动验证切片，ShapeStyles 完整对等和平台呈现保留未完成状态。

参考：本地 `org.eclipse.draw2d/SWTGraphics.java` 的 `reconcileHints`、
`setAntialias` 与状态栈；本地 `vello/vello/src/lib.rs::AaConfig`；
[ADR-024](../../adr/adr-024-graphics-paint-stroke-and-clipping.md)、
[ADR-025](../../adr/adr-025-unified-graphics-and-glyph-preparation.md)。
