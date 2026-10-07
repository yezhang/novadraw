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
| P0 | 动画与表现平面 | 显式时钟、Timeline/Track、属性/布局/路由/生命周期/视口动画、无动画快速路径 | P2-M01，ADR-026 已接受；M01-A/B/C complete，M01-D verification pending |
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
详细合同见
[Animation / Presentation Plane 设计](../design/animation/animation-system.md)、
[Animation 公开 API 合同](../design/animation/public-api-contract.md)、
[Animation 领域能力集成矩阵](../design/animation/capability-integration.md)与
[ADR-026](../adr/adr-026-animation-and-presentation-plane.md)。

```text
AnimationBehavior                     AnimationPlan
├── Trigger                           ├── Target
├── Scope                             ├── Motion
├── PlanFactory              ───────→ ├── Composition
└── ReducedMotionFallback             ├── Interruption
                                      └── Suspension
```

Trigger 不属于 `AnimationPlan`。显式调用直接创建 Plan；只有属性、状态、事务或生命周期
等隐式策略才安装 Behavior。详细合同见
[Animation Behavior / Trigger 合同](../design/animation/behavior-trigger-contract.md)。

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

#### 5.2.1 应用案例：Connection 虚线流向动画

动画基础设施必须能够表达 Connection 虚线沿 source → target 或反方向持续流动的效果。
该案例属于持续程序动画，不属于路由动画：Connection route、Anchor、Locator 和
PointList 保持不变，Timeline 只采样表现态 dash phase。

概念计算为：

```text
period = validated_dash_period
phase(t) = wrap(direction * speed * (t - start_time), period)
effective_dash_offset = wrap(source_dash_offset + phase(t), period)
```

约束：

- 复用受检 `StrokeStyle` 的 dash pattern、source dash offset 与 visual envelope；
- `speed` 使用 Canvas 逻辑长度/秒，不使用物理像素/帧；
- 公开方向使用 `Forward` / `Reverse` 语义；`Forward` 固定表示 source → target，
  dash offset 的正负映射由引擎内部按规范路径方向处理；
- 使用 Host 提供的单调绝对时间采样，不通过逐帧累加推进相位，避免掉帧改变流速；
- 动画只覆盖 presentation dash offset，不修改 source `StrokeStyle`、route points、
  Figure bounds、hit-test、通知或 undo/redo history；
- route 在其他事务中变化时保留当前规范化 phase，并在新路径上继续播放；dash pattern
  改变时按新周期重新规范化，非法 pattern 继续由 Graphics 输入契约拒绝；
- 每帧 damage 保守覆盖 Connection 的完整 visual envelope；后端只有在能证明等价时
  才能进一步缩小局部区域；
- 多条流动 Connection 共享 Runtime Clock 和同一次 frame tick，不为每条连接创建 timer；
- effectively hidden、surface suspended 或无可见 damage 时暂停持续帧请求；
- `Disabled` 保留 source dash offset 的静态虚线；`Reduced motion` 默认使用静态箭头或
  方向标记，不强制持续移动；
- 终点箭头表达稳定业务方向，虚线相位只表达活动或流动感，不能作为方向真值的唯一来源。

该案例的最低验证包括：

1. source → target 与 target → source 两个方向的固定时间采样；
2. 不同刷新间隔得到相同时间点的相同 phase；
3. nested transform、Viewport scroll/zoom 与 DPI 变化不改变逻辑流速；
4. route 更新、dash pattern 更新、中断与恢复保持确定相位；
5. Disabled/Reduced motion 不产生持续 tick，且静态像素与 source style 一致；
6. 多连接共享 Clock，不触发 reroute、layout、FigureMoved 或全树扫描。

#### 5.2.2 应用案例：沿线 pulse、拖尾与端点箭头交接

市场上的流向动画通常不是把移动物体直接做几何 morph，而是组合稳定路径、沿路径
移动的 traveler、可选拖尾和终点反馈：

| 参考 | 常见效果 | 对本案例的启示 |
|---|---|---|
| [React Flow Animating Edges](https://reactflow.dev/examples/edges/animating-edges) | 圆点、自定义 SVG 或节点沿 edge path 移动；路径重算与动画播放分离 | traveler 应是独立 temporary visual，不能成为 Connection route 的一部分 |
| [G6 Animation](https://g6.antv.antgroup.com/en/manual/animation/animation) | dash offset 形成 ant-line，halo 呼吸用于局部强调 | 流向、活动和到达强调应拆成可组合 channel |
| [GoJS Process Flow](https://gojs.net/latest/samples/processFlow.html) | 管线通过 stroke dash offset 表达持续流动 | 密集或低缩放场景可降级为低成本 dash flow |
| [deck.gl TripsLayer](https://deck.gl/docs/api-reference/geo-layers/trips-layer) | 以 `currentTime` 驱动路径头部和渐隐 trail | 拖尾应由当前时间和 route 区间重建，不应依赖逐帧历史点累积 |

Novadraw 采用“稳定方向层 + traveler 层 + 到达反馈层”，不把 literal path morph
作为基础合同：

1. **稳定方向层**：committed Connection path 与 endpoint arrow 始终是业务方向真值；
2. **Traveler 层**：紧凑 pulse 沿 source → target 匀速移动，可带短拖尾并按路径切线定向；
3. **到达反馈层**：pulse 进入 target handoff zone 后压缩/淡出，箭头同步增强；主题可以
   追加一次性 endpoint halo，但不得启动第二条私有时间线；
4. **稳定收尾**：清除 temporary visual 和 presentation override，只留下 committed
   Connection 与 arrow。

同一能力提供两种语义明确的表现策略：

- **Reveal**：用于新 Connection 首次出现。最终箭头已提交，但在首帧录制前原子安装
  presentation override，使箭头从低强调度随 pulse 到达而显现，不能先闪现一帧再隐藏；
- **Activity**：用于已有 Connection 的一次或重复活动。箭头全程保持可见，pulse 到达
  只产生短时强调，不重新揭示或替换箭头。动画结束只表示视觉序列完成，除非应用另有
  已提交事件，否则不得解释为消息已送达或任务已成功。

概念采样为：

```text
route_length = arc_length(route)
head_distance(t) = clamp(speed * (t - start_time), 0, route_length)
head = sample_by_arc_length(route, head_distance)
trail = route_interval(max(0, head_distance - trail_length), head_distance)
handoff = ease(saturate(
    (head_distance - (route_length - handoff_length)) / handoff_length
))
```

`handoff_length` 必须按当前 route length 和 target decoration envelope 受检收敛。
拖尾是 route 上的瞬时区间及透明度分布，不保存逐帧采样点，因此掉帧、暂停和恢复不会
改变形状或逻辑速度。

阶段语义：

1. **Admission**：校验 route、方向、速度、视觉预算与 fallback；提交最终 arrow，并按
   Reveal/Activity 策略在下一次 frame recording 前安装 presentation override；
2. **Travel**：按 route arc length 采样 pulse 的 position、tangent 和 normal，拖尾末端
   连续衰减，折线、曲线及不等长 segment 均保持逻辑匀速；
3. **Approach/Handoff**：进入 handoff zone 后，pulse 与 trail 渐隐或压缩，arrow
   emphasis 渐入；不得要求任意 pulse 轮廓与任意 decoration 具有可 morph 拓扑；
4. **Finish**：清除 pulse、trail、halo 与 decoration override，不保留 frame callback
   或重复 wakeup。

约束：

- committed state 在动画开始前已包含最终 route 与 endpoint arrow；动画只控制
  presentation，不延迟业务方向、通知或 history 提交；
- `speed`、`trail_length` 与 handoff 距离使用 Canvas 逻辑长度，时间来自 Host 单调绝对
  Clock；刷新率、route 分段数量、Viewport zoom 和 DPI 不改变逻辑进度；
- pulse 方向由 Connection source/target 语义确定，不能依赖 path command 顺序猜测；
- pulse 的视觉 token 可以是 dot、capsule 或领域图标，但位置采样、朝向、拖尾和交接
  使用同一通用 motion-path 合同，不为每种外观建立专用动画类型；
- route 更新时优先把当前 traveler head 投影到新 route 的方向一致最近点，并按剩余弧长
  继续；若投影超过调用方声明的视觉连续性预算，则按显式 interruption policy 执行
  crossfade、restart 或 cancel，不允许回退到旧 committed route；
- 缺失 endpoint decoration 时，按 target port 一次性强调、pulse 在端点淡出、立即显示
  静态 Connection 的顺序降级；zero-length route、disposed endpoint、预算拒绝或中途取消
  必须清理全部 temporary visual；
- pulse、trail 与 halo 默认 `NonInteractive`，不参与 hit-test、cursor、selection 或
  accessibility；箭头和 Connection 的交互语义来自 committed Figure；
- damage 覆盖 pulse/trail 的 old/new envelope，以及 handoff 期间 arrow/halo 的 old/new
  envelope；不得为单个 traveler 扫描全树；
- 多 pulse 使用 Stagger 与同一次 Clock advance。高密度、低缩放或预算受限时，策略可以
  依次关闭 halo、缩短/关闭 trail、降级为 dash flow 或静态箭头，但不能改变方向语义；
- `Disabled` 立即显示 committed arrow；`Reduced motion` 默认使用静态箭头加一次短时
  opacity/emphasis，或完全静态，不执行沿线位移；
- Reveal 与 Activity 共享 Runtime Clock、temporary visual、damage 和清理协议，不创建
  Connection 私有 timer、backend 动画循环或第二套 route cache。

最低验证包括：

1. 固定时间下 pulse 的 position/tangent/normal、trail 区间与 handoff progress；
2. 直线、折线与曲线路由按弧长匀速，刷新间隔变化不改变同一时刻的结果；
3. Reveal 不出现箭头首帧闪烁，Activity 全程保留稳定箭头；
4. handoff 前后 pulse、trail 与箭头的合成像素连续，无双箭头或越过 target 的拖尾；
5. route 变更时的重投影、crossfade/restart/cancel 及剩余距离语义；
6. decoration 缺失、zero-length、预算拒绝、cancel 和 dispose 的降级与清理；
7. nested transform、Viewport scroll/zoom、clip 与 DPI 变化下的几何和 damage；
8. Disabled、Enabled、Reduced motion 三种模式及低缩放/高密度降级；
9. 多 pulse 的 Stagger、共享 Clock、active-track/temporary-visual 预算；
10. 不产生 route/layout、Figure 通知、accessibility、undo/redo history 或业务送达状态
    副作用，结束后不保留持续 tick。

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
2. P2-G02 已按 ADR-025 完成统一测量/绘制 API 与 glyph 准备链路；
3. 保持 Graphics、TextLayout、Figure paint 和 backend capability 单一合同。

### 阶段 1：动画与表现平面

1. 已完成 Draw2D Animation/Animator 对标，ADR-026 已接受；
2. 已登记 P2-M01、稳定 API family 与 `core.p2-m01-animation` 可执行 suite；
3. 已在提案中冻结 Clock、Timeline、Track、Motion、Composition 以及独立
   Behavior/Trigger 边界；
4. 已冻结 source/committed state、presentation state 与 backend state 三平面；
5. M01-A 已实现 typed 属性 channel、并行/顺序/错峰编排和确定性 headless clock；
6. M01-B 已实现 Figure presentation override、old/new damage、temporary visual 与
   suspend/dispose/fault 清理；M01-C 已实现 transaction/layout、Connection route、
   Viewport、continuous procedural、dash flow 与 pulse/handoff；
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
4. 每个切片证明组合价值，避免扩大基础 `Figure` trait 或通用 `FigureMut`。

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
- Draw2D Animation 源码事实：
  [`../reference/draw2d/figure/animation.md`](../reference/draw2d/figure/animation.md)
- Animation / Presentation Plane：
  [`../design/animation/animation-system.md`](../design/animation/animation-system.md)
- Animation 公开 API 合同：
  [`../design/animation/public-api-contract.md`](../design/animation/public-api-contract.md)
- Animation 领域能力集成矩阵：
  [`../design/animation/capability-integration.md`](../design/animation/capability-integration.md)
- ADR-026：
  [`../adr/adr-026-animation-and-presentation-plane.md`](../adr/adr-026-animation-and-presentation-plane.md)
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
- React Flow Animating Edges：
  <https://reactflow.dev/examples/edges/animating-edges>
- GoJS Process Flow：
  <https://gojs.net/latest/samples/processFlow.html>
- deck.gl TripsLayer：
  <https://deck.gl/docs/api-reference/geo-layers/trips-layer>
- Cytoscape.js：
  <https://js.cytoscape.org/>
- D3 Transition：
  <https://d3js.org/d3-transition>
- Konva Animation 与 PixiJS Ticker：
  <https://konvajs.org/docs/vue/Simple_Animations.html>、
  <https://pixijs.com/8.x/guides/components/ticker>
