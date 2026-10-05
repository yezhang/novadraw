# Draw2D 内置能力丰富度补齐计划

类型：`roadmap`

日期：2026-10-05

状态：`current`

## 1. 文档边界

本计划回答 Draw2D Core 1.0 之后，Novadraw 应如何继续补齐内置能力丰富度。
它负责能力分组、价值判断和建议顺序，不定义运行时架构，也不维护 delta 执行状态。

- M1-M10 与 G0-G5 的历史完成含义不变。
- 已登记 delta 的唯一状态入口仍是
  [`p2-delta-backlog.md`](p2-delta-backlog.md)。
- 平台支持等级只在
  [`platform-support-matrix.md`](platform-support-matrix.md) 维护。
- 行为对等关系只在
  [`../parity/draw2d/api-coverage.md`](../parity/draw2d/api-coverage.md) 维护。
- 对标范围只包括 `org.eclipse.draw2d`、`org.eclipse.draw2d.graph` 与
  `org.eclipse.gef`；不得使用 `org.eclipse.zest` 定义需求或实现。
- 不以复制 Java 类名或增加 Figure 数量作为完成判据；组合替代必须证明可观察行为和
  失败语义等价。
- 动画作为一等公民表示它进入统一时钟、表现状态、damage 和提交协议，不表示应用或
  Figure 必须启用动画。没有安装动画策略时，所有变更必须立即到达最终状态。

候选能力只有在补充 normative design 或 ADR、登记正式 P2 delta、验证 suite 和
毕业证据后才能进入实施。本计划不为尚未登记的能力预占 delta 编号。

## 2. 当前基线

Novadraw 已完成的主要能力包括：

- Figure 树、盒模型、绘制遍历、坐标转换、命中和生命周期；
- Layout、Validation、Damage Repair 与 UpdateManager 两阶段事务；
- 输入分发、capture、focus、typed listener、Tooltip 与 accessibility snapshot；
- Viewport、ScrollPane、Zoom、Layer、Freeform 与六类常用布局；
- Connection、五类 Anchor、Direct/Bendpoint/Manhattan/Fan/ShortestPath Router、
  Locator 与 decoration；
- Rectangle、Ellipse、RoundedRectangle、Polyline、Polygon、Triangle、
  ScalablePolygon、Label、Image、TextFlow；
- Clickable、Button、Toggle 与 Editor G0-G5 编辑闭环。

因此，下一阶段的主要缺口不是更多普通几何图元，而是从框架机制走向可直接组装应用的
内置策略、组合组件、动态表现和输出能力。动画应先于这些能力建立为可选横切基础设施，
使后续图布局、Connection、控件和概览能力按需接入，而不是各自实现私有动画循环。

## 3. 能力缺口分组

| 优先级 | 能力组 | 主要缺口 | 当前去向 |
|---|---|---|---|
| P0 | 动画与表现平面 | 显式时钟、Timeline/Track、属性/布局/路由/生命周期/视口动画、无动画快速路径 | 尚未登记正式 delta |
| P1 | 内置控件族 | repeat scheduler、ButtonGroup、checkbox、radio 互斥；Slider 作为现代扩展候选 | P2-W01 |
| P1 | 组合 Figure 与边框 | LabeledContainer/GroupBox、Separator、Focus/Frame 类边框、MultiLineLabel 高层入口 | 尚未登记正式 delta |
| P1 | 图自动布局 | DirectedGraphLayout、CompoundDirectedGraphLayout 与外部算法 adapter | P2-L01 |
| P2 | 概览与反馈 | Ghost/snapshot feedback、Thumbnail、ScrollableThumbnail/minimap 协作 | 尚未登记正式 delta |
| P2 | 高级图形与裁剪 | Graphics surface 验收、跨 Viewport Connection 可见性、多区域 clipping provider | P2-G01、P2-C03 |
| P2 | 高级文本 | 统一 Graphics 测量/绘制、fragment style、剩余 inline/block flow | P2-G02、P2-T03 |
| P2 | 输出能力 | printing/export、ScaledGraphics 等价输出目标 | P2-O01 |
| P2 | 平台无障碍 | Web action/focus 与 Native 原生 AT provider | P2-A01 |
| P3 | API convenience | `getClip`、`getAbsoluteScale`、`removeAll`、`isMirrored`、统一 Shape mutation | 真实跨消费者需求成立后登记 |

## 4. Figure 丰富度策略

### 4.1 优先补充

1. **Checkbox 与 radio group**
   - 复用 `ClickableModel`、Runtime 输入状态和 accessibility action；
   - ButtonGroup 是独立 selection model，不复用 Editor selection；
   - repeat firing 使用 Runtime/Host 时钟协议，不在 Figure 内启动私有 timer。

2. **LabeledContainer 与 GroupBox**
   - 优先用普通容器、Label 和 Border 组合表达；
   - 只有组合无法稳定表达 owner-scoped 测量、insets 或 accessibility 关系时，
     才新增具体 Figure；
   - GroupBox/Frame/Separator/Focus 优先作为 Border 或 style preset。

3. **MultiLineLabel 高层入口**
   - 作为 `TextFlowFigure`、Viewport 和 accessibility 的易用 facade；
   - 不建立第二套 shaping、换行、caret 或 selection geometry。

4. **Ghost/snapshot feedback**
   - 用于拖拽、复制、创建预览和只读反馈；
   - 优先消费 retained feedback layer 或受检 snapshot，不复制业务 Figure 的活状态；
   - 不允许绕过 Viewer/Runtime 的输入仲裁和 damage 协议。

5. **Thumbnail/minimap**
   - 消费稳定场景快照、UpdateListener 或提交后 notification；
   - 缩略图源更新不得侵入渲染主循环；
   - offscreen 资源、分块更新、DPI 和失效必须由 backend/host 合同管理。

所有内置 Figure 和控件必须先定义无动画行为。动画只能作为可选表现策略附加：

- 未配置动画时直接提交最终状态；
- 关闭动画不能改变输入、命中、accessibility、通知或 undo/redo 语义；
- 内置能力不得为等待动画结束而延迟业务状态提交；
- Figure 内部不得启动私有 timer、run loop 或 backend-specific animation。

### 4.2 低优先级或由组合替代

- `Panel`：由普通容器、opaque style 和 background paint 表达；
- `PuristicScrollPane`：由 ScrollPane 组合与样式预设表达；
- `SchemeBorder` 家族：优先转为 Border/style preset，不复制 SWT 主题对象；
- 额外规则几何图元：优先使用 Path、Polygon 或外部自定义 Figure；
- 仅提供同名 convenience、没有新行为的 Java 类：默认不迁移。

## 5. 其他关键能力

### 5.1 动画作为可选一等公民

Draw2D 的 Animation/Animator、LayoutAnimator 和 RoutingAnimator 提供关键帧捕获与
插值，但其静态全局状态、Singleton animator 和同步阻塞播放不适合 Novadraw。
市场框架的共同做法是分离动画目标、时间模型、触发方式和时间编排；Novadraw 应采用
同样的正交模型，而不是为每种组合增加独立类。

```text
AnimationPlan
├── Target       property / layout / route / lifecycle / viewport
├── Motion       tween / keyframes / spring / decay / procedural
├── Trigger      explicit / property change / state / transaction / lifecycle
└── Composition  parallel / sequence / stagger / repeat
```

一等公民的目标边界：

- scheduler 由 Runtime 或 Host 显式拥有，不使用全局状态；
- Host 提供单调时间，headless/replay 可以注入确定性时间；
- source mutation 立即提交最终业务状态，动画只生成可丢弃的表现状态；
- 动画消费已提交的起止快照，不成为模型、布局、路由或 undo/redo 真值；
- 时间、暂停、取消、替换、retarget、Figure dispose、surface suspend/resume
  有确定语义；
- 每帧表现变化继续进入正常 damage、paint recording 和 backend submission 合同；
- 可交互表现动画必须声明 hit-test、cursor 和 accessibility bounds 是否跟随表现几何；
- reduced-motion 是独立策略，不能与完全禁用动画混为一谈。

### 5.2 动画类型与适用边界

首个设计应覆盖以下目标域，但它们共享同一 Timeline/Track/Clock：

| 动画类型 | 典型属性或事件 | 目标边界 |
|---|---|---|
| 属性动画 | color、opacity、stroke、corner、数值 | typed value interpolation，不使用字符串 property path |
| 变换与几何动画 | translate、scale、rotate、bounds、motion path | 表现几何与业务 geometry 分离 |
| 布局动画 | 旧稳定布局到新稳定布局 | 优先只计算一次最终布局，再插值表现快照 |
| 路由动画 | Connection endpoint、point list、path | 同拓扑插值；不兼容拓扑使用受控 morph 或 crossfade |
| 生命周期动画 | enter/update/exit、show/hide、创建/删除 | exit 使用 snapshot/temporary visual，不保活已删除 Figure |
| 结构动画 | collapse/expand、reparent、shared element | 依赖稳定身份和 overlay/snapshot，后于基础生命周期动画 |
| 视口动画 | pan、zoom、fit、scroll-to、auto-expose | 与用户手势和手动滚动有明确抢占规则 |
| 交互状态动画 | hover、pressed、selected、focus、feedback | 不改变输入状态机和 action 发生事实 |
| 物理动画 | spring、decay、fling、inertia、snap | 由可终止 simulation 驱动，不直接读取系统墙钟 |
| 持续程序动画 | pulse、breathing、dash flow、逐帧效果 | 必须显式启用、可暂停且声明工作量预算 |

布局动画必须进一步区分：

1. **结果过渡**：布局算法一次计算最终结果，表现层在旧/新快照间插值；作为首批能力。
2. **求解过程动画**：force-directed 等算法持续产出中间状态；属于算法 driver，
   必须有取消、预算、checkpoint 和交互语义，不能冒充普通 Tween。

### 5.3 动画可选性与禁用协议

图形编辑器可以完全不使用动画，也可以只在局部场景显式添加动画。规划要求支持三种
独立策略：

| 模式 | 行为 |
|---|---|
| Disabled | 所有状态立即到达最终值，不创建 Timeline/Track，不请求持续帧 |
| Enabled | 显式计划和已安装的隐式策略正常执行 |
| Reduced motion | 使用缩短、淡化或离散 fallback；具体策略由 capability/host 声明 |

策略优先级必须满足：

1. Runtime/Host 的全局 Disabled 高于局部动画请求；
2. 局部 Figure、capability 或操作可以进一步禁用动画；
3. Core 默认不为普通 mutation 自动安装动画；
4. 只有应用安装 Behavior/Transition policy 或显式提交 AnimationPlan 才产生动画；
5. 没有活动动画时不保留 frame callback、不持续 request_redraw、不执行空 tick；
6. 禁用动画后的最终 committed state、通知顺序和渲染结果必须与动画完成后等价。

“一等公民”因此表示动画有稳定公共协议和完整生命周期，不表示动画默认开启，更不表示
不使用动画的应用承担额外行为复杂度。

### 5.4 图自动布局

P2-L01 是当前最具领域价值的算法补齐项。目标不是把 Draw2D graph 对象模型搬入 Core，
而是建立独立、不可变、可验证的图布局输入输出：

- 支持普通有向图与 compound/subgraph；
- 输出节点位置、rank、反馈边和长边 bendpoints；
- 固定输入顺序、数值策略和确定性要求；
- 外部算法通过 adapter 接入，不持有 FigureTree 或 Runtime 可变引用；
- 布局结果经预检后由 Runtime/Editor 原子发布；
- 默认直接发布最终布局；只有调用方启用布局 transition 时才生成动画。

### 5.5 打印与导出

P2-O01 应抽象为输出目标和 scale/clip adapter，而不是引入 SWT Printer：

- 文本、图像、路径、clip 和变换与屏幕渲染保持同源；
- 支持 tile、fit-page、fit-width、fit-height 等可观察行为；
- 输出目标能力不足时在提交前结构化拒绝；
- 具体文件格式或平台打印桥由 backend/host 承担。

## 6. 建议执行顺序

### 阶段 0：关闭当前 Graphics 主线

1. 完成 P2-G01 剩余的 Native surface 局部修复验收；
2. 按 ADR-025 完成 P2-G02 统一测量/绘制 API 与 glyph 准备链路；
3. 保持 Graphics、TextLayout、Figure paint 和 backend capability 单一合同。

### 阶段 1：动画与表现平面

1. 对标 Draw2D Animation/Animator 及市场框架，形成 normative design 或 ADR；
2. 登记正式动画 delta、稳定 API family、suite 和外部消费者；
3. 建立显式 Clock、Timeline、Track、Motion、Trigger 与 Composition 边界；
4. 建立 source/committed state 与 presentation state 双平面；
5. 先实现显式属性动画、并行/顺序/错峰编排和确定性 headless clock；
6. 用布局结果过渡、Connection 路由过渡和 Viewport transition 验证跨域能力；
7. 验证 Disabled、Enabled、Reduced motion 及零活动动画快速路径；
8. 暂不实现通用 path morph、shared element、force solver 逐轮动画或 backend
   专有 compositor 快路径。

### 阶段 2：图自动布局

1. 完成 P2-L01 Draw2D graph 行为清单；
2. 冻结独立图模型与确定性输出；
3. 以外部算法 consumer 验证 adapter；
4. 接入 Editor/Runtime 原子发布；
5. 默认无动画；可选消费阶段 1 的布局 transition。

### 阶段 3：内置组件层

1. 评审并推进 P2-W01；
2. 将 LabeledContainer/GroupBox、Separator/Focus Border 与 MultiLineLabel facade
   拆成最小独立切片；
3. 每个切片先定义无动画行为，再声明可选 hover/pressed/focus/lifecycle transition；
4. 每个切片证明组合价值，避免扩大基础 `Figure` trait 或通用 `FigureEditor`。

### 阶段 4：生命周期、结构动画与概览

1. 增加 enter/exit snapshot 和 temporary visual；
2. 评审 collapse/expand、reparent 与 shared-element identity；
3. 为 snapshot feedback 与 Thumbnail/minimap 建立独立 delta；
4. Thumbnail 可以选择消费动画表现帧或稳定最终态，必须显式声明；
5. 动画和缩略图不得共享隐式全局时钟或可变场景副本。

### 阶段 5：需求驱动的高级能力

按真实消费者顺序推进 P2-C03、P2-T03、P2-O01、P2-A01 和生命周期能力。
Windows/Linux 原生运行、Safari/Firefox 和完整 AT provider 保留到对应平台发布准备，
不得由其他平台或 headless 测试替代。

## 7. 每项能力的毕业条件

每个正式 delta 至少满足：

1. 在 parity 账本登记受影响的 API family 和 Draw2D 源码证据；
2. 在 normative design 或 ADR 中定义所有权、扩展点、失败模式和替代边界；
3. 公开 API 不暴露 Kurbo、Vello、Winit 或 SWT 类型；
4. 至少一个外部消费者不修改 Core 枚举或内部 match 即可接入；
5. 构造、替换、失败、取消和 dispose 无半提交状态；
6. 精确定向 contract test 覆盖正例、反例和恢复路径；
7. 可见能力具备 Native/Web 或对应输出目标的视觉证据；
8. 任何 public Render IR 均被启用 backend 完整消费，或在提交前结构化拒绝；
9. 更新 P2 backlog、parity、suite manifest、demo/人工验收和完成证据；
10. 按风险通过 crate 定向验证、对应 suite 与最终质量门禁。

动画基础设施及其消费者还必须满足：

11. Disabled 模式和未配置动画的 API 路径不要求构造动画对象；
12. 无活动动画时不产生持续 tick、redraw、全树扫描或 backend submission；
13. 禁用动画与动画完成后的 committed state、通知和像素结果等价；
14. 中断从当前 presentation state retarget，不跳回旧起点或产生半提交；
15. 每个 Track 提供保守 visual envelope，逐帧 damage 覆盖前后表现区域；
16. 自定义动画只能修改受控 presentation channel，不直接修改 FigureTree、模型或路由真值；
17. 时钟可注入，固定时间序列产生确定输出，headless 测试不依赖真实 sleep；
18. reduced-motion、失焦、suspend/resume、dispose 和资源失败有自动契约；
19. 新增可见能力必须记录无动画行为、可动画 channel、fallback 和交互几何策略；
20. 动画启用前后的非动画工作量无未解释回退。

## 8. 非目标

- 不机械复制 Draw2D 全部类、继承层次或 SWT 类型；
- 不以 `org.eclipse.zest` 的扩展行为定义 Core 或图布局目标；
- 不为展示数量增加低价值规则图形；
- 不把产品 schema、serializer、Palette UI 或业务工作台重新纳入引擎 Core；
- 不在真实 3D 用例出现前建设空 Scene3D 实现；
- 不用 Animation、Thumbnail 或 export 绕过 Runtime 单一提交权威；
- 不要求应用、图形编辑器、Figure 或 LayoutManager 启用动画；
- 不默认把所有属性变化、布局或路由变化转换为动画；
- 不使用字符串 property path、隐式反射或核心枚举穷举第三方动画属性；
- 不为属性、布局、路由、Spring、关键帧的每种组合建立独立类层次；
- 不允许 Figure 私有 timer、全局 animation singleton 或每帧 source-state mutation；
- 不用 backend compositor 快路径定义 Core 动画语义。

## 9. 参考入口

- Draw2D 长期能力分母：
  [`../parity/draw2d/api-coverage.md`](../parity/draw2d/api-coverage.md)
- P2 delta 状态：
  [`p2-delta-backlog.md`](p2-delta-backlog.md)
- 产品交付清单：
  [`product-deliverables.md`](product-deliverables.md)
- 平台支持矩阵：
  [`platform-support-matrix.md`](platform-support-matrix.md)
- 当前目标调整：
  [`goal-alignment-adjustment-plan-2026-09-30.md`](goal-alignment-adjustment-plan-2026-09-30.md)
- Basic Widgets：
  [`../design/architecture/basic-widgets.md`](../design/architecture/basic-widgets.md)
- Text Layout：
  [`../design/architecture/text-layout.md`](../design/architecture/text-layout.md)
- Connection Routing：
  [`../design/architecture/connection-routing.md`](../design/architecture/connection-routing.md)
- Qt Quick Animation and Transitions：
  <https://doc.qt.io/Qt-6/qtquick-statesanimations-animations.html>
- Flutter Animations：
  <https://docs.flutter.dev/ui/animations>
- WPF Animation Overview：
  <https://learn.microsoft.com/en-us/dotnet/desktop/wpf/graphics-multimedia/animation-overview>
- GoJS Animation：
  <https://gojs.net/latest/learn/animation>
- G6 Animation Overview：
  <https://g6.antv.antgroup.com/en/manual/animation/animation>
- Cytoscape.js：
  <https://js.cytoscape.org/>
- D3 Transition：
  <https://d3js.org/d3-transition>
- Konva Animation 与 PixiJS Ticker：
  <https://konvajs.org/docs/vue/Simple_Animations.html>、
  <https://pixijs.com/8.x/guides/components/ticker>
