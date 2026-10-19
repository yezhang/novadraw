# Draw2D 能力补齐规划

类型：`roadmap`

本页承接 2026-10-05 能力规划，维护价值分组和拆分原则，不定义运行时契约或 delta
状态。执行顺序见 [P2 backlog](p2-delta-backlog.md#当前推进顺序2026-10-14-校准)，
平台声明见 [支持矩阵](platform-support-matrix.md)，对等分母见
[语义账本](../parity/draw2d/api-coverage.md#长期能力分母与处置)。

## 能力分组

| 价值分组 | 能力 | 去向与契约入口 |
|---|---|---|
| P0 | 可选动画与表现平面 | P2-M01；[动画专题](../design/animation/00-index.md) |
| P1 | repeat、ButtonGroup、checkbox/radio、Slider | P2-W01；[基础控件](../design/architecture/basic-widgets.md) |
| P1 | LabeledContainer/GroupBox、Separator、Focus/Frame Border、MultiLineLabel | 消费者成立后登记；优先组合已有容器、Label、Border、TextFlow |
| P1 | Directed/Compound graph layout adapter | P2-L01；独立图快照及确定性输出，限制参考于 `org.eclipse.draw2d.graph` |
| P2 | Ghost/snapshot feedback、Thumbnail/minimap | 消费者成立后登记；稳定或表现快照只读消费 |
| P2 | Graphics surface、跨 Viewport Connection、多区域 clip | P2-G01/C03；[Graphics](../design/rendering/p2-g01-graphics-extension.md)、[连接](../design/architecture/connection-routing.md) |
| P2 | fragment style、剩余 inline/block flow | P2-T03；[文字布局](../design/architecture/text-layout.md) |
| P2 | printing/export、ScaledGraphics 等价输出 | P2-O01；输出目标与 scale/clip adapter |
| P2 | Web action/focus、Native AT provider | P2-A01；[Tooltip/AT](../design/architecture/tooltip-accessibility.md) |
| P3 | getClip/getAbsoluteScale/removeAll/isMirrored 等便利入口 | 真实跨消费者需求成立后登记；不重建内置专用 mutation |

P0–P3 只表示能力价值分组，不是审计严重级或另一份执行顺序。
M1–M10/G0–G5 的完成含义不变；候选不能预占编号或据本表直接进入实施。
对标限定 Draw2D/GEF，禁止使用 Zest 定义目标。

## Figure 与组件的拆分

Checkbox/radio 复用 Clickable 与输入/AT；ButtonGroup 使用独立 selection model，
不复用 Editor selection；repeat 使用 Runtime/Host 时钟。
GroupBox、Separator、Focus 等优先用 Border/style preset，只有稳定测量、Insets
或 AT 关系无法组合时才新增具体 Figure。

MultiLineLabel 复用 TextFlow、Viewport 和 AT，不新建 shaping、换行或 caret 算法。
Panel、PuristicScrollPane 和 SchemeBorder 族优先通过容器/样式组合；规则几何优先
使用 Path/Polygon。仅提供同名 convenience、没有新行为的 Java 类不单独迁移。

Ghost/Thumbnail 不复制业务 Figure 活状态，不侵入主循环。
offscreen 资源、DPI、分块更新与失效由 backend/host 承担；Thumbnail 明确选择
CommittedOnly 或 PresentationFrame，不复制 timeline 或时钟。

## 动画需求由专题承载

动画的价值是让属性、布局、route、viewport、生命周期和持续效果共享时钟、
表现平面及清理语义。无动画应用保持直接显示 committed final state。
Timeline、Trigger、cancel/retarget、Disabled/ReducedMotion 和能力毕业条件统一见
[动画设计入口](../design/animation/00-index.md)。

Connection dash flow、沿弧长 pulse/trail、Reveal/Activity 与端点 handoff 的需求详见
[持续效果契约](../design/animation/capability-integration.md#10-connection-持续效果)。
原规划中的公式和验证条件已归入该专题，不再在规划中维护第二份运行时合同。
市场参考保留 React Flow、GoJS、G6 和 deck.gl 等来源，属于产品输入，不能替代
Draw2D/GEF 语义证据。

图布局结果过渡与 solver 过程动画分别设计。算法一次发布最终结果，
可选动画消费稳定快照；force solver 的预算、checkpoint 和取消由算法 driver 负责。
无动画布局不以动画平台毕业为启动条件。

## 图布局与输出的后续设计

P2-L01 先固定普通/compound 图的不可变 node/edge snapshot、rank、反馈边、
long-edge bendpoints、输入顺序及数值策略；外部算法通过 adapter 接入。
布局应用先校验身份、版本、数值与覆盖范围，再由已有 owner 发布，
不预设覆盖模型/history 的通用回滚事务。

P2-O01 使用输出目标及 scale/clip adapter。文字、图像、路径与屏幕语义同源，
支持 tile、fit-page/width/height；能力不足提交前拒绝。具体文件格式与打印桥由
backend/host 承担，不把 SWT Printer 带入 Core。

## 毕业条件

每项登记 family 和源码证据；冻结 owner、输入/输出、错误与替代边界；
由独立消费者证明不修改 Core 枚举或中心分支即可接入。
公开签名不暴露平台/后端类型，public IR 被所有启用后端消费或预先拒绝。
正例、反例、恢复、生命周期及真实视觉证据按范围闭合，再更新 backlog/parity。
详细验证命令见 [验证入口](../../verification/README.md)。

不借能力补齐重建产品 schema、serializer、Palette 或业务工作台，不新增空 Scene3D，
不绕过 Runtime，不为内置能力建立特权路径。可见能力先证明静态行为，
再按动画专题声明可选 channel、interaction、fallback 与预算。

## 产品研究来源

- [Qt 状态与动画](https://doc.qt.io/Qt-6/qtquick-statesanimations-animations.html)
- [Flutter 动画](https://docs.flutter.dev/ui/animations)
- [WPF 动画](https://learn.microsoft.com/en-us/dotnet/desktop/wpf/graphics-multimedia/animation-overview)
- [React Flow Animating Edges](https://reactflow.dev/examples/edges/animating-edges)
- [GoJS Animation](https://gojs.net/latest/learn/animation)、[Process Flow](https://gojs.net/latest/samples/processFlow.html)
- [G6 Animation](https://g6.antv.antgroup.com/en/manual/animation/animation)
- [deck.gl TripsLayer](https://deck.gl/docs/api-reference/geo-layers/trips-layer)
- [Cytoscape.js](https://js.cytoscape.org/)、[D3 Transition](https://d3js.org/d3-transition)
- [Konva](https://konvajs.org/docs/vue/Simple_Animations.html)、[PixiJS Ticker](https://pixijs.com/8.x/guides/components/ticker)
