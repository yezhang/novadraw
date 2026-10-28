# 官方样例能力补充方案

类型：`proposal`

日期：2026-10-26

状态：2026-10-27 用户批准方向上下文、低 miter、Cocoa 零宽映射和通用下划线实施。
通用设备 hairline 仍须独立评审，不属于本次批准范围。
用户已要求先分析方案，再落实能力。本方案给出实施边界，重大协议变更仍按
[CLAUDE.md](../../../CLAUDE.md)的“重大架构变更需用户确认后再实现”执行。
实现状态统一登记在 [P2 backlog](../../roadmap/p2-delta-backlog.md)。

`api_semantics`：`graphics.context`、`paint.protocol`、`text.flow`、
`text.interaction`、`figure.properties`、`coordinate.conversion`、
`damage.repaint`、`viewport.scroll_zoom`。
这些名称对应的采用关系以 [API 账本](../../parity/draw2d/api-coverage.md)为准，
本页不新增里程碑编号。

## 优先补消费者，再决定 Core 增量

已有 Figure、Layout、Border、Graphics 和资源服务足够表达的能力，直接完善样例。
测量、选择与局部重绘缺少验证时，先补真实消费者和失败反例。
只有现行契约不能表达目标，或反例证明通用机制错误，才修改 Core。

本次分析基线为 Novadraw `bf6409d0cb9ef65aa47091d8d4b3d07b3a1cd973`；
工作区已有其他任务的改动，本批只修改明确列出的样例与方案文件。
官方源码只读，范围限定 Draw2D 与所需 SWT GC；不使用 Zest，不运行 Java。
Canvas 外的原生控件、打印作业与完整 Editor 继续服从
[Canvas Core 计划](../../verification/plans/draw2d-official-examples-core-plan.md)。

| 能力与消费者 | 当前证据 | 实施归属 |
|---|---|---|
| HelloWorld resize | 包装 root 没有布局，Label 固定 200×100 | 用已有 StackLayout 填满 client area |
| UML DogEar | 填充与描边共用 6 单位多边形，遗漏描边宽高减一 | 外部 Border，6 单位填充与 5 单位折角描边分开 |
| ShapeStyles 几何 | 原实现使用浮点对称内缩、二次曲线圆角和不同三角形 | 已有 Path、圆弧、整数几何；保留原始样式参数 |
| 三种布局的文字与局部擦除 | Shape geometry/full-frame 证据不能证明文字 minimum 或 retained-partial | 真实 Label/TextFlow 测量与连续帧验证；有反例再修 Core |
| Bidi mirrored Canvas | 两侧相同构造，缺少右侧 mirror 上下文 | Core 方向上下文、text preparation 与 ScrollPane 组合契约 |
| ShapeStyles 线宽 0 | Core 的逻辑宽度 0 不绘制；SWT 各平台行为并非统一设备 hairline | 先裁定样例零宽映射，再独立决定通用设备 hairline |
| ShapeStyles miter 0/0.5 | StrokeStyle 拒绝小于 1；SWT 传递输入至原生后端 | 独立受检值、轮廓、bounds 与后端增量 |
| 混合字体下划线 | 有单字体指标，缺少跨 run/行/bidi 装饰准备 | Text/Graphics 的统一准备结果 |
| Caret 选择范围 | 已有命中与几何；上游注释和绘制分支不能直接等同 | 边界映射与最终选择几何验证；失败后确定修复位置 |
| Print/UML/Zoom 字体与 DPI | 已有资源注册，请求字族被便携字体替代 | Host 提供实际 face/bytes，明确 point/逻辑 px 与 DPI |

## 现有能力内的样例修正

HelloWorld 保持原文字、颜色与初值，包装 root 使用 StackLayout；
宿主修改 root bounds 后由同一布局流程更新 Label。
验证收缩、放大、恢复，不在宿主或样例中追加独立 resize 布局算法。

StickyNote 的填充由 Figure 绘制，DogEar 使用公开 Border 协议。
Border 保持 3 单位 Insets；外框宽高先减 1，折角使用 5 单位。
折角三角形、上/下/左/右线分别绘制；保留官方 6 单位填充轮廓。
上游 top-left 到自身的零长线不承载可见边界，本批不增加重复绘制。
现有窄宽测量与连接标签测试继续保护组合行为。

ShapeStyles 保留 X 后 Y 的样式应用、五种图形和八个参数轴。
Rectangle 填满原 bounds，描边使用 floor/ceil 的整数内缩；
Ellipse 的填充和描边都使用官方 optimized bounds。
RoundedRectangle 填充使用 8×8 corner dimensions，描边另减整数 line inset；
Path 的圆弧半径为 corner dimensions 的一半。
Triangle 使用 NORTH 的整数布局，几何与描边宽度无关；Polyline 保留官方三点。
Enabled 轴同时展示输入禁止与 disabled 绘制：先在 `(1,1)` 偏移处绘制浅色
fill/outline，再在原位绘制深色 fill/outline。两次绘制保持原 AA、alpha 与 stroke；
临时变换使用 Graphics 状态栈恢复。浅/深色使用明确的便携白/灰配色，
不宣称与各平台系统按钮主题颜色完全相同。
这些修正不等于所有 native raster 像素已与上游一致。

## 镜像须由通用方向上下文表达

建议为普通容器提供可继承、可局部覆盖的布局方向上下文，宿主只在 Canvas
边界提供初值。不能只给 root 或 Bidi 样例设置特权标志。
公开输入可采用领域值 `LayoutDirection`；具体规范路径与 mutation payload
在独立 ADR 中冻结，统一通过 Builder 和 Runtime-scoped mutation 设置。
方向值只描述布局方向，不替代 `ParagraphDirection`，也不替代几何 AffineTransform。

测量和 preparation 读取同一已解析方向。Leading/Trailing 先按段落方向解释，
再根据 mirrored layout 决定容器坐标中的位置；绝对 Left/Right 同样遵守上游
Canvas 坐标到视觉坐标的约定。文字 shaping、run 排序与 fragment placement
共同产出布局结果。禁止对最终 glyph 纹理做水平反射，避免字形本身倒置。

输入继续用引擎的 surface/local 坐标查询，按已准备的 fragment 几何命中。
ScrollPane 的侧别与方向键语义消费同一上下文，共享纵向 RangeModel 保持不变。
方向 mutation 必须使受影响子树的 layout/text preparation 失效，
清除旧像素并更新命中结果；方向不变时不触发全树重排。

验收覆盖 mirrored/unmirrored × LTR/RTL × 五种 alignment、嵌套覆盖、窄宽、
共享滚动、方向 mutation、caret affinity 和输入坐标；外部容器、layout 与
文字引擎消费同一上下文。仅两侧文字看起来不同不足以退出。

## 零宽对等与设备 hairline 分开裁定

方案形成时 [ADR-047](../../adr/adr-047-per-operation-antialias.md)保留逻辑宽度 0
不绘制，并要求 ShapeStyles 的相关输入报告不支持。用户批准后，
[ADR-049](../../adr/adr-049-low-miter-and-cocoa-sample-profile.md)已修订样例 profile，
Core 零逻辑宽度仍不绘制。

源码核对修正了早先“上游零宽就是设备 hairline”的笼统判断：
SWT Cocoa 的 `checkGC`/`setCGClipping` 将宽度 0 传为 1，并选用零宽专用 dash 数组；
GTK `checkGC` 也将 0 传为 1。Win32 的 GDI/GDI+ 分支和 DPI 行为需另行核验。
这些事实不能推出旋转、非均匀缩放和所有 DPI 下恒为一个设备像素。

建议先按明确的 Cocoa 对等 profile 实现样例零宽映射：
原始输入保持 0，绘制使用 1 逻辑单位，标准 dash 使用上游 ZERO 数组；
控制面板公开说明 profile。该方案复用 Core 正逻辑线宽，不改变 Core 零宽语义。
ZERO 数组分别为 DOT `[3,3]`、DASH `[18,6]`、DASHDOT `[9,6,3,6]`、
DASHDOTDOT `[9,3,3,3,3,3]`。跨平台完全对等仍须按各平台证据报告。

通用设备 hairline 是独立可选增量，不能作为该样例的必需条件。
若接受，建议以显式 `StrokeMetric` 区分 logical width 与 device hairline，
保留旧 `with_width(0)` 的不绘制行为；公开范围、属性身份和兼容 setter 由 ADR 冻结。
hairline 的轮廓在设备域构建，宽度为一个设备像素；dash 先按已声明长度域分段，
paint/brush、clip 和透明度继续保存正确变换，不能用平均 scale 近似非均匀变换。

device bounds 依赖已提交 surface/DPI，必须进入帧准备、旧新 damage 与命中协议，
不能通过源码级常量扩大所有 Figure bounds。不可逆变换、设备数值不可表示、
预算超限和后端不支持均通过 Result 拒绝，且拒绝前不修改资源或目标纹理。
该增量涉及 surface-aware envelope，是本方案中需要单独评审的核心取舍。

## 低 miter 保留输入，轮廓和边界共同消费

建议将有限、非负 miter limit 纳入受检值；负值、NaN 和无穷大继续拒绝。
原始 0/0.5 必须保留在 Graphics 状态、IR、快照、缓存 key 与诊断中。
普通 miter join 的尖角扩张比至少为 1；对低于 1 的阈值，候选轮廓采用 bevel
退化语义。该规则是拟采用的便携契约，尚未证明所有原生平台的特殊处理均一致。

Vello 普通与 AA OFF 两条描边路径必须读取同一规则。
visual bounds 和 damage 采用相同有效 join，不用 clamp 到 1 的假状态。
实施时按 ADR-049 保留既有中心线与 tolerance 命中协议，不新增尖角区域的精确命中承诺。
Round/Bevel join 保持原语义；0、0.5、1、2、50、开闭路径、反向路径和锐角
通过独立轮廓参考与 GPU 像素验证，再解除样例 unsupported 标记。

这项会修订 ADR-024/047 的受检范围，但无需新建专用图形类型或改变递归渲染流程。

## 文字装饰消费已有排版快照

首个切片只处理通用下划线，不同时引入新的文本树、文本编辑状态或完整主题系统。
装饰输入绑定逻辑文本范围与样式；preparation 按行、bidi run、实际 fallback face
拆分范围，并使用对应字体实例的 underline offset/thickness 和 baseline。
实际 glyph 的 face/variation/size 是指标来源，不能仅查询请求字族。

输出为 owned、不可变的装饰片段，和 text layout revision 一起发布；
绘制、ink bounds、damage 读取同一输出。测量时区分逻辑高度与装饰 ink 扩张。
字体缺失遵循资源等待，已解析 face 缺少指标则使用明确错误或获批的显式 fallback，
禁止任意固定 1px 替代。取消、替换字体、宽度变化和 detach 使旧结果按版本失效。

外部 TextLayoutEngine 使用相同指标和片段协议；外部 Figure 通过公开 Graphics
消费这些片段。不为内置 TextFlow 增加专属绘制或更新路径。
验证两种实际字体、不同字号、换行、RTL、组合字符、资源替换和局部擦除。
Caret 仍使用已有 TextInteractionMap；只有边界验证发现失败时才追加独立修复。

## 性能、包体积与退出证据

现有样例修正不增加依赖、字体或后端能力；圆角仍走已有矢量 Path。
新增方向上下文只使受影响子树失效，不允许逐帧全树扫描。
低 miter 不增加常驻资源；设备 hairline 和文字装饰分别评估轮廓、run 数量、
缓存身份、预算和额外分配。较大实现以相同 release 消费者测量 Native/Wasm
产物、压缩传输体积与首次加载资源，不能用代码行数代替产物差额。

性能场景包含静态缓存、连续变换、DPI 1/1.5/2、窄宽反复重排和长文局部追加；
记录 preparation/encoding P95、GPU 等待、输出规模和峰值缓存。
现有 AA 高 DPI 连续变换已测得超出 16.7ms，不能据此承诺新增能力达到 60fps。
优化不得引入内置 fast path、隐式降级或 source-state mutation。

实施按既有 K/H 身份单链推进：

| 顺序 | 契约与消费者 | 退出证据 |
|---|---|---|
| 先完成 | K02/K08：HelloWorld、DogEar、ShapeStyles 现有几何 | 样例库、公开绘图命令、40 个 GPU 矩阵；保留 surface/上游像素限制 |
| 批次反馈 | K02/H04：真实文字布局与 retained-partial | 最小尺寸/换行/恢复，旧新 damage，Partial 对 Full；失败定位后修 Core |
| 已批准，先实施 | K05/K07：方向上下文与 Bidi | 独立 ADR、外部消费者、方向/对齐/输入/滚动矩阵 |
| 已批准，随后实施 | K08：低 miter 与明确 profile 的零宽映射 | 修订 ADR、受检错误、两条后端路径、样例矩阵和成本 |
| 已批准，最后实施 | K08：通用下划线 | 独立 ADR、owned 准备结果、外部消费者、bounds/damage 与成本 |
| 后续独立评审 | 可选 device hairline | surface-aware envelope 与各平台证据 |
| 宿主验收 | H01/H02/H04：实际字体、DPI、窗口呈现 | 各入口记录实际 face 和平台；未运行的平台保持未验收 |

批准范围按 2026-10-27 的用户确认执行，具体契约与实施状态以各 ADR 和 backlog 为准。
通用设备 hairline 仍保留独立评审。
已有样例修正的执行结果见
[本批实施记录](../../verification/reviews/official-example-capability-increment-2026-10-26.md)。

## 源码依据

Draw2D 固定提交 `4463d9d0ce13c19d10fbe769d29f28b7345a8cba`：
`HelloWorld.main`、`StickyNote.paintFigure`/`DogEar.paint`、
`ShapeStylesExample`、`RectangleFigure.outlineShape`、
`Ellipse.getOptimizedBounds`、`RoundedRectangle.fillShape`/`outlineShape`、
`Triangle.validate`、`LightweightSystem.RootFigure.isMirrored`、
`BlockFlow.validate`、`BlockFlowLayout.addBelowPreviousLine`。
完整路径见 [官方来源清单](../../../examples/native/draw2d-official-examples/source-contracts.tsv)。

SWT 固定提交 `cc2762f9f02888b3b9122e534ddeabacd34b4604`：
`bundles/org.eclipse.swt/Eclipse SWT/{cocoa,gtk,win32}/org/eclipse/swt/graphics/GC.java`。
核验方法为只读源码分析，没有运行原生 GC；原生像素行为仍需各平台证据。
同一仓库的计划、目录和测试不作为独立外部来源重复计数。
