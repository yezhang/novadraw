# Animation 领域能力集成矩阵

类型：`normative-design`

状态：`accepted`，按领域消费者分阶段实施

目标 delta：P2-M01

`api_semantics`：`animation.timeline`、`animation.presentation`、
`frame.preparation`、`damage.repaint`

## 1. 目的

本文验证动画正交模型能够支撑真实框架能力，而不是只提供抽象 Timeline：

- 属性与交互状态；
- Layout result transition；
- Connection route transition；
- Viewport pan/zoom/fit；
- enter/exit lifecycle；
- dash flow 与 moving pulse；
- 后续图自动布局、控件、feedback 和 Thumbnail。

每项能力必须明确 committed truth、presentation channel、交互几何、fallback 和
毕业切片。没有这些信息的“可动画”声明不进入实现。

## 2. 正交维度与状态平面

四个正交维度回答不同问题：

| 维度 | 回答 | 不负责 |
|---|---|---|
| Target | 哪个 subject 的哪个表现 channel 变化 | 时间曲线、触发时机 |
| Motion | 给定时间如何采样值 | 查找 Figure、提交帧 |
| Trigger | 何时创建或 retarget plan | 插值、组合 |
| Composition | Track 的并行、顺序、错峰和重复关系 | source mutation |

所有领域能力共享三层状态：

```text
committed truth
→ Runtime-owned presentation override / temporary visual
→ backend lowering and pixels
```

动画只拥有中间层。关闭或移除该层后，场景必须直接呈现 committed truth。

## 3. 首批 Presentation Channel

| Channel | Value | Subject | Committed truth | Presentation 输出 |
|---|---|---|---|---|
| Opacity | `f64` | Figure/temporary visual | style opacity 或默认 1 | paint alpha override |
| Local transform | `Affine2D` | Figure | Figure placement/transform | paint 与可选 hit-test transform |
| Bounds | `Rectangle` | Figure child | layout/explicit final bounds | presentation bounds |
| Color/Paint | `Color`/受检 Paint | Figure channel | final style | paint override |
| Route geometry | PointList/path snapshot | Connection | router final route | presentation path/crossfade |
| Viewport transform | origin + scale | Viewport | logical origin/scale | presentation pan/zoom |
| Dash phase | logical length | Connection stroke | source dash offset | wrapped dash offset |
| Visual progress | normalized scalar | temporary visual | 无 source 对象 | provider-sampled drawing |
| Decoration blend | normalized scalar | endpoint decoration | final committed decoration | pulse/arrow crossfade 与 scale |

Channel 不是一个要求 Core 穷举所有外部 Figure 状态的巨型枚举：

- Core 只提供通用 channel constructor；
- Connection、Viewport、Widget 等模块拥有自己的 typed descriptor；
- 外部 Figure 通过 typed provider 增加 channel；
- Runtime 内部用 subject/channel key 仲裁 owner；
- 未声明 additive algebra 的 channel 只能有一个 owner。

## 4. 能力总矩阵

| 能力 | Target | Motion | Trigger | Composition | Interruption | 交互策略 | ReducedMotion | 切片 |
|---|---|---|---|---|---|---|---|---|
| opacity/color/transform | Figure property channel | Tween/Keyframes/Spring | Explicit/State | Parallel/Sequence | Replace | Committed 或 Presentation | 短 fade/直接终值 | M01-A/B |
| Layout result | child bounds snapshot | Tween/Spring | Transaction capture | Stagger/Parallel | Replace | 默认 Committed | 短 fade/直接终值 | M01-C |
| Connection route | route/opacity channels | Tween | Transaction capture | Parallel crossfade / with layout | Replace | 默认 Committed | 直接 final route | M01-C |
| Viewport pan/zoom/fit | viewport transform | Tween/Spring/Decay | Explicit/Host gesture | Sequence | Replace by direct input | Presentation | 短 pan/直接终值 | M01-C |
| enter/update/exit | Figure/visual snapshot | Tween/Keyframes | Lifecycle | Stagger/Sequence | Replace/Queue | exit NonInteractive | fade/直接出现消失 | M01-B/C |
| dash flow | dash phase | Procedural Continuous | Explicit/State | Repeat | Replace | Committed | 静态箭头/虚线 | M01-C |
| moving pulse | route visual progress | Procedural Finite | Explicit | Stagger + Sequence | Replace/Queue | NonInteractive | 静态箭头/短 fade | M01-C |
| hover/pressed/focus | widget property channels | Tween/Spring | State | Parallel | Replace | Committed | 离散状态 | P2-W01 consumer |
| graph layout transition | node bounds + routes | Tween | Layout result capture | Stagger + Parallel groups | Replace | Committed | 直接 final layout | P2-L01 consumer |
| drag/create feedback | temporary visual | Tween/Spring | Tool lifecycle | Parallel | Replace | NonInteractive/Presentation | 静态 feedback | 后续 feedback delta |
| Thumbnail/minimap | viewport marker/optional scene presentation | Tween | Viewport/scene commit | Parallel | Replace | NonInteractive | 静态 viewport marker | 后续 overview delta |

Interruption 是 channel ownership policy，不属于 Motion 或 Composition。Crossfade 也不是
独立 Motion；它由两个 opacity Tween 通过 Parallel 组合得到。

## 5. 属性与交互状态

属性动画是 M01-A 的最小外部消费者：

```text
committed property change
→ optional Behavior observes stable fact
→ TrackTarget<Color/opacity/transform>
→ Motion samples presentation value
→ old/new visual envelope enters damage
```

约束：

- 显式 plan 可以给出 start/end value，不强制使用 scene capture；
- State Trigger 只观察 committed hover/pressed/selected/focus，不改变输入状态机；
- pressed/action 发生事实立即提交，视觉回弹不能延迟 action；
- 同一 channel 的新状态默认 Replace，从当前 presentation value retarget；
- 没有安装 Behavior 时，控件继续使用最终静态样式。

首批只需一个外部 Figure provider 证明新增 channel 不修改 Core match。

## 6. Layout Result Transition

LayoutManager 仍只计算一次 final result：

```text
capture stable child bounds
→ source mutation / layout invalidation
→ validation computes final committed bounds
→ transition compares before/after
→ presentation bounds interpolate
```

约束：

- 不在每帧调用 LayoutManager；
- layout constraint、preferred/min/max size 和 notification 全部基于 committed bounds；
- before capture 只冻结目标 child，不复制整棵 FigureTree；
- 新增/删除 child 分别使用 enter/exit visual，不伪造不存在的两端 bounds；
- parent clip、nested transform、scroll/zoom 作用于采样后的 presentation bounds；
- Stagger 使用稳定 child order 或调用方显式 key，不能依赖 arena slot 顺序；
- 中途发生第二次 layout 时，原子 retarget 到新的 committed result。

默认 hit-test 使用 Committed。需要拖拽跟随表现几何的场景必须显式选择 Presentation，
并验证输入与绘制使用同一采样 epoch。

## 7. Connection Route Transition

Connection 的 source/target、Anchor、Router constraint 和 final route 始终是 committed
truth。动画只覆盖可见 path。

### 7.1 Compatible topology

满足以下条件时可逐点插值：

- before/after 使用相同 route topology descriptor；
- 对应 segment 语义一致；
- 所有点有限且 visual envelope 可计算。

### 7.2 Incompatible topology

首批使用 crossfade：

```text
Parallel
├── old route temporary visual: opacity Tween 1 → 0
└── new committed route override: opacity Tween 0 → 1
```

不通过插入重复点伪造“通用 morph”，也不把 Draw2D 未声明的索引插值当作语义。

### 7.3 与 Layout 的组合

节点 layout transition 与 route transition 使用同一次 committed transaction 的 capture，
但分别拥有 bounds channel 和 route channel：

```text
Parallel
├── Stagger(node bounds tracks)
└── Parallel(connection route tracks)
```

Router 只在 final committed geometry 上运行。动画采样不触发 reroute。

## 8. Viewport Transition

覆盖 pan、zoom、fit-selection、fit-all 和 scroll-to：

- committed viewport origin/scale 在操作事务中先到 final value；
- presentation transform 从旧 snapshot 过渡到 final；
- wheel、pinch、drag 等直接用户输入默认 Replace 自动 transition；
- inertia 使用有终止条件的 Decay，不读取墙钟；
- auto-expose 是按 drag 状态实时驱动的 controller，不包装成固定 Tween；
- surface suspend 使用 plan 的 Advance/Pause/Finish policy。

Viewport 使用 Presentation hit-test 时，事件点降域必须消费与当前帧相同的 presentation
transform。首批若未提供该能力，则 admission 只允许 Committed，并在设计中明确视觉与
交互差异。

## 9. Lifecycle 与 Temporary Visual

### 9.1 Enter

Figure 已 attach 且 committed state 可查询。enter track 可以覆盖 opacity/transform，
但生命周期通知不等待动画结束。

### 9.2 Exit

dispose 前显式 capture immutable visual：

```text
capture drawing + transform + envelope
→ detach/dispose Figure
→ add NonInteractive temporary visual
→ sample exit motion
→ remove temporary visual and wakeup
```

temporary visual 不持有 FigureId 活引用，不参加 layout、route、selection、
accessibility tree 或 undo/redo。

### 9.3 Update

内容替换默认直接显示 committed final content。需要 crossfade 时捕获 old drawing，
并与新 committed drawing 并行淡入淡出。

## 10. Connection 持续效果

### 10.1 Dash flow

```text
phase(t) = wrap(direction * speed * elapsed, dash_period)
offset(t) = wrap(source_offset + phase(t), dash_period)
```

- Forward 固定表示 source → target；
- speed 使用逻辑长度/秒；
- route 不变时不触发 layout/routing；
- route 改变时 phase 保持规范化后投影到新 route；
- hidden/suspended 默认 Pause；
- 每帧 damage 保守覆盖 Connection visual envelope。

### 10.2 Moving pulse → endpoint decoration

```text
Sequence
├── Travel(route arc-length progress 0 → 1)
└── Handoff
    ├── pulse opacity/scale → 0
    └── committed arrow decoration opacity/scale → 1
```

- Travel 按 arc length 采样 position/tangent/normal，不按 segment index 匀速；
- committed final arrow 在动画开始前已存在；
- plan admission 在下一次 frame recording 前安装 arrow presentation override，
  不能先显示 committed arrow 一帧再将其隐藏；
- pulse 与 decoration 只通过 presentation override 交接；
- pulse 默认 NonInteractive；
- zero-length route、缺失 decoration、dispose 和 cancel 均清理 temporary visual；
- 多 pulse 用 Stagger，共享 clock，并受 active-track/temporary-visual 预算限制。

## 11. 图自动布局的协作边界

P2-L01 分为算法平面和表现平面：

```text
Graph layout adapter
→ deterministic final node positions / bendpoints
→ Runtime atomic source publication
→ optional P2-M01 result transition
```

- solver 中间迭代不是普通 animation Track；
- force solver driver 自己负责预算、checkpoint、取消和确定性；
- animation 只消费 solver 已发布的 stable result；
- 禁用动画时 layout 输出和 route 完全相同；
- layout completion 不等待像素 transition。

## 12. Widget 与组合组件的协作边界

P2-W01 及后续组合 Figure 先交付静态行为，再选择性安装动画 policy：

| 组件状态 | Committed fact | 可动画 channel | 默认 fallback |
|---|---|---|---|
| hover | pointer target | background/opacity | 离散样式 |
| pressed | input state | transform/color | 离散 pressed |
| selected | selection model | indicator/color | 静态 indicator |
| focus | focus owner | focus ring opacity | 静态 focus border |
| repeat firing | scheduler action | 可选 pulse | 不动画，action 不变 |
| expand/collapse | expanded fact | bounds/clip/opacity | 直接 final state |

ButtonGroup/radio selection model、Slider value 与 accessibility action 不进入
presentation state。

## 13. Feedback、Thumbnail 与概览

### 13.1 Feedback

- Ghost/create/drag feedback 使用 temporary visual；
- 可以使用 Spring/opacity，但 Tool/Request/EditPolicy 状态是 committed truth；
- feedback cancel 删除 visual 并 damage old envelope；
- 若可交互，必须显式 Presentation hit-test；首批默认 NonInteractive。

### 13.2 Thumbnail/minimap

Thumbnail 必须选择：

- `CommittedOnly`：只展示稳定最终场景；
- `PresentationFrame`：消费与主视图同一只读 presentation snapshot。

不得为缩略图复制 active timeline 或创建第二个 clock。Viewport marker 可以有独立
Track，但仍由同一 Runtime time 驱动。

## 14. 状态机

```text
Scheduled
  └── first monotonic time → Running

Running
  ├── suspend(Pause) → Paused
  ├── retarget → Running(new prepared track)
  ├── natural completion → Completed
  ├── cancel / Disabled / dispose → Cancelled
  └── sample/provider failure → Failed

Paused
  ├── resume → Running
  ├── retarget → Running(new prepared track)
  └── cancel / dispose → Cancelled
```

Terminal transition 在同一 Runtime 事务中：

1. 计算 final damage；
2. 清除 owner/override/temporary visual；
3. 更新有界 terminal journal；
4. 重算 next wakeup；
5. 只在像素变化时请求帧。

## 15. 实施与能力毕业

| Slice | 核心机制 | 必须证明的领域消费者 |
|---|---|---|
| M01-A | clock、typed Track、Tween/Keyframes、composition、mode、cancel/retarget | 外部 property channel |
| M01-B | presentation override、envelope、temporary visual、lifecycle cleanup | opacity/transform + exit visual |
| M01-C | capture、bounds/route/viewport channels、continuous procedural | layout、route、viewport、dash、pulse |
| M01-D | Native/Web/Headless replay、DPI/transform/scroll、预算 | 组合场景视觉与工作量 |

P2-M01 完成不表示所有后续产品能力已经实现。它表示这些能力不再需要私有时钟、
私有 presentation store 或第二套取消语义。

后续 delta 的动画接入毕业条件：

1. 先证明 Disabled 下完整静态行为；
2. 声明 channel、Trigger、Motion、Composition；
3. 声明 committed truth 与 interaction policy；
4. 声明 ReducedMotion 与 unsupported fallback；
5. 覆盖 cancel、retarget、dispose、suspend 与 budget；
6. 证明未新增 Figure 私有 timer、全树逐帧扫描或 source-state mutation。
