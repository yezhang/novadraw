# 官方样例的几何与文字适配

类型：`normative-design`

能力归属：官方 Canvas 样例；Core 方向、轮廓和下划线由各自专题负责。

2026-10-27 已批准范围保持。实现状态及平台限制见
[实施记录](../../verification/reviews/official-example-capability-increment-2026-10-26.md)与
[P2 backlog](../../roadmap/p2-delta-backlog.md)。

## 消费者范围

已有 Figure、Layout、Border、Graphics 和资源服务能够表达的行为由样例组合，
通用测量、坐标、输入和失效由 Core 负责。发现基础契约缺口时，单独建立 delta。
Canvas 外原生控件、打印作业及完整 Editor 服从
[Canvas Core 计划](../../verification/plans/draw2d-official-examples-core-plan.md)。

| 消费者 | 当前契约入口 |
|---|---|
| Bidi 镜像、ScrollPane 侧别 | [继承布局方向](inherited-layout-direction.md) |
| ShapeStyles 低 miter、零宽 | [低 miter 与 Cocoa profile](../rendering/low-miter-and-cocoa-sample-profile.md) |
| Path 混合字体下划线 | [字体下划线](../rendering/prepared-text-underlines.md) |
| Caret 边界、选择与命中 | [结构化 Flow](structured-flow-layout.md) |
| 字体与 DPI | Host 提供实际 face/bytes，记录 point、逻辑 px 与 DPI |

通用设备 hairline 是[独立候选](../rendering/device-hairline-proposal.md)，
不属于已批准范围。

## 样例几何

HelloWorld 保持原文字、颜色与初值，包装 root 使用 StackLayout。
宿主更新 root bounds 后由统一布局更新 Label，不增加示例私有 resize 算法。

StickyNote 填充由 Figure 绘制，DogEar 走公开 Border；Insets 保持 3 单位。
外框宽高先减 1，描边折角为 5 单位，填充折角为 6 单位。
折角三角形与四边分别绘制；上游零长 top-left 线不增加重复绘制。

ShapeStyles 保留 X 后 Y 的样式应用、五种图形与八个参数轴。
Rectangle 填满原 bounds，描边采用 floor/ceil 整数内缩；
Ellipse 使用 optimized bounds；RoundedRectangle 填充使用 8×8 corner dimensions，
描边另减整数 line inset，圆弧半径为 corner dimensions 的一半。
Triangle 保留 NORTH 整数布局，几何与线宽无关；Polyline 保留原三点。

disabled 绘制先在 `(1,1)` 偏移处绘制浅色 fill/outline，再在原位绘制深色。
两次保留 AA、alpha 与 stroke，临时变换走 Graphics 状态栈恢复。
白/灰是便携配色，不宣称等同所有平台系统主题。

## 消费者验证

三种布局使用真实 Label/TextFlow 测量；最小尺寸、换行、恢复及 retained-partial
必须通过连续帧证明，不能用 Shape geometry 或 full-frame 证据替代。
Caret 依据已发布交互几何核验范围，不从上游注释推断最终选择边界。
验证静态复用、连续变换、DPI 1/1.5/2、窄宽重排与局部追加，
记录 preparation/encoding P95、GPU 等待、输出规模及峰值缓存。
原生呈现与上游像素对等按平台分别报告，未运行的平台保留未验收。
产物体积只在用户明确要求时测量。

## 只读依据

Draw2D 提交 `4463d9d0ce13c19d10fbe769d29f28b7345a8cba`：
`HelloWorld.main`、`StickyNote.paintFigure`/`DogEar.paint`、
`ShapeStylesExample`、`RectangleFigure.outlineShape`、`Ellipse.getOptimizedBounds`、
`RoundedRectangle.fillShape/outlineShape`、`Triangle.validate`、
`LightweightSystem.RootFigure.isMirrored`、`BlockFlow.validate`、
`BlockFlowLayout.addBelowPreviousLine`。完整路径见
[官方来源清单](../../../crates/novadraw/examples/native/draw2d-official-examples/source-contracts.tsv)。

SWT 提交 `cc2762f9f02888b3b9122e534ddeabacd34b4604`：
`bundles/org.eclipse.swt/Eclipse SWT/{cocoa,gtk,win32}/org/eclipse/swt/graphics/GC.java`。
仅只读分析，不使用 Zest、不运行 Java；源码事实不替代原生像素证据。
