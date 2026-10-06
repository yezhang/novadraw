# Animation / Presentation Plane 设计提案

类型：`normative-design`

状态：`accepted`，按 M01-A 至 M01-D 分阶段实施

目标 delta：P2-M01

`api_semantics`：`animation.timeline`、`animation.presentation`、
`frame.preparation`、`damage.repaint`

## 1. 目标

Novadraw 将动画作为可选的一等公民：

- 属性、布局、Connection 路由、生命周期、视口和持续效果共享同一时钟与编排模型；
- source/committed state 立即到达最终业务状态；
- animation 只生成可丢弃的 presentation state；
- 未配置或禁用动画时，现有 mutation、validation、damage 和 submission 行为不变；
- 无活动动画时不持续 tick、request redraw、扫描 FigureTree 或提交空帧；
- 外部 Figure 可以扩展动画表现，不要求修改 Core 枚举。

本设计建立动画基础合同，不要求所有 Figure、Layout 或应用启用动画。
属性、布局、路由、视口、生命周期与后续组件如何消费该合同，见
[Animation 领域能力集成矩阵](capability-integration.md)。

## 2. 非目标

- 不复制 Draw2D 的静态 `Animation`、Singleton Animator 或同步阻塞 run loop；
- 不让动画进度成为模型、Figure bounds、route points、selection 或 undo/redo 真值；
- 不让 Figure、LayoutManager、Router 或 backend 创建私有 timer；
- 不使用字符串 property path、反射或无类型 payload；
- 不在首批实现通用 path morph、shared element、force solver 逐轮动画、粒子系统；
- 不以 backend compositor 快路径定义 Core 语义；
- 不保证任意两个不兼容几何都能插值。

## 3. 依据

Draw2D 事实见
[Animation / Animator 源码事实](../../reference/draw2d/figure/animation.md)。
Draw2D 证明了布局和路由可以共享一次 before/after animation session，但其全局状态、
阻塞播放和每帧 source mutation 不适合 Rust Runtime。

市场框架共同将动画拆为正交维度：

- Qt Quick：Property/Path/Parent/Anchor、Behavior、Transition、Spring、
  Sequential/Parallel；
- WPF/JavaFX：typed property animation、keyframes、Timeline/Storyboard；
- Flutter：implicit/explicit、Tween/Physics、Stagger、shared element；
- GoJS/G6/Cytoscape：layout、lifecycle、viewport、property 与持续 edge effect；
- Konva/PixiJS/D3：Tween、逐帧 driver 与数据更新 transition。

Novadraw 采用这些共同边界，不复制任一框架的对象模型。

## 4. 正交模型

一个动画由四个独立维度组合：

```text
AnimationPlan
├── Target       property / layout / route / lifecycle / viewport
├── Motion       tween / keyframes / spring / decay / procedural
├── Trigger      explicit / property change / state / transaction / lifecycle
└── Composition  parallel / sequence / stagger / repeat
```

### 4.1 Target

Target 说明“什么表现值发生变化”，不决定时间曲线：

- 通用 presentation property：opacity、local transform、clip/paint 等；
- layout snapshot：child presentation bounds；
- route snapshot：Connection presentation path；
- lifecycle：temporary visual enter/exit；
- viewport：presentation pan/zoom；
- custom Figure：完整、受检的 FigurePresentation。

### 4.2 Motion

Motion 把标准化时间映射为采样值：

- Tween：start/end + easing；
- Keyframes：有序 key time/value；
- Spring/Decay：可终止 simulation；
- Procedural：显式时间输入的纯采样器。

Motion 不读取系统墙钟，不请求重绘，不修改 Runtime。

### 4.3 Trigger

Trigger 决定 plan 何时创建：

- Explicit：应用显式启动；
- PropertyChange/State：已安装 Behavior 对 committed fact 响应；
- Transaction：source transaction 稳定后比较 before/after；
- Lifecycle：attach、dispose、show/hide；
- Host gesture：viewport/scroll 等平台无关输入结果。

Core 默认不为普通 mutation 安装隐式 Trigger。

### 4.4 Composition

Timeline 支持：

- Parallel；
- Sequence；
- Stagger；
- Delay；
- Repeat/Reverse；
- 显式 group cancel。

Composition 只组织 track 时间，不取得 FigureTree 可变引用。
Pause 属于 active timeline 的运行状态，不与 Delay 建立重复的 composition node。

## 5. 状态平面

### 5.1 Source / committed state

source state 是唯一业务真值：

- Figure topology、bounds、style、content；
- layout/route 最终结果；
- Viewport logical origin/scale；
- model、selection、command history；
- accessibility role/state 与 action。

动画开始前 source transaction 必须先完成既有 validation/derived-state convergence。

### 5.2 Presentation state

presentation state 是 Runtime-owned、可丢弃的派生层：

- 当前插值 transform/opacity/paint；
- layout/route 的当前表现 geometry；
- temporary visual；
- endpoint decoration override；
- 当前 track visual envelope。

presentation state 不产生 FigureMoved、PropertyChanged、route notification 或
undo/redo entry。动画结束或取消后清除 override，最终 source state 已经存在。

### 5.3 Backend state

RenderBackend 只消费 Runtime 生成的 RenderSubmission。backend 可以优化 transform/
opacity 等动画，但必须保持 Core 定义的时间采样、damage、取消和最终像素语义。
不支持某 presentation capability 时必须在提交前结构化拒绝或使用已声明 fallback。

## 6. 所有权与时间

Runtime 独占 AnimationService：

```text
Runtime
├── FigureTree / UpdateManager / existing services
└── AnimationService
    ├── mode
    ├── active timelines
    ├── presentation overrides
    ├── temporary visuals
    └── next wakeup
```

Host 继续使用现有 `Runtime::advance_time(MonotonicTime)` 时间入口：

- 同一 Runtime 的时间单调不减；
- Tooltip、caret blink、auto-expose 与 animation 共享 host 时间来源；
- Runtime 不持有 PlatformHost，不创建线程或系统 timer；
- active timeline 使 Runtime 返回 pixels changed / next wakeup；
- active set 为空时不请求持续 wakeup。

帧因果顺序：

```text
advance monotonic time
→ apply source mutations
→ stabilize committed scene
→ create/retarget triggered plans
→ sample active tracks
→ build immutable presentation snapshot
→ accumulate old/new presentation damage
→ record and submit frame
→ flush committed source notifications
```

动画采样不得反向产生 layout、route 或 source mutation。

## 7. 公开调用面方向

详细调用合同、时间锚点、before/after capture、错误枚举与 M01-A 门禁见
[Animation 公开 API 合同提案](public-api-contract.md)。

目标调用形状为：

```rust,ignore
let capture = runtime.animations().capture(targets)?;
runtime.figure(figure)?.set_bounds(final_bounds)?;
let outcome = runtime.animations().transition(capture, spec)?;

let outcome = runtime.animations().start(plan)?;
runtime.animations().retarget(id, replacement)?;
runtime.animations().cancel(id)?;
```

约束：

- `AnimationMut<'_>` 是 Runtime scoped mutable facade，不是全局 service handle；
- Disabled start 返回 Suppressed，不创建 timeline 或 ID；
- plan、track、target 和 value 在 admission 时完成 namespace、数值和 capability 校验；
- invalid/foreign/disposed target 不创建半成品 timeline；
- ID 携带 Runtime namespace 与 generation；
- layout/route/lifecycle transition 使用显式 one-shot capture，不从 mutation 后状态猜测
  before snapshot；
- query 不推进时间；
- no-animation 调用方不需要构造上述对象。

## 8. 扩展边界

Core 提供常用 typed presentation channel，不建立覆盖全部 Figure 私有状态的枚举。

第三方扩展分两类：

1. 自定义值插值器：实现受检、纯函数式 interpolation，不访问 Runtime。
2. 自定义 Figure track provider：在 prepare 阶段冻结输入，sample 阶段只产出
   `FigurePresentation`、visual envelope 和可选 temporary visual。

provider 不能：

- 修改 FigureTree、Runtime、UpdateManager 或模型；
- 保存可变 Runtime/Figure 引用；
- 产生没有 visual envelope 的可见输出；
- 返回 Vello、Kurbo、Winit 或 GPU 类型；
- 用 untyped blob 绕过公开 presentation 合同。

Runtime 内部可以类型擦除 prepared track，但 public registration 保持 typed。
扩展 panic 沿 ADR-014 的 Runtime fault 边界处理。

## 9. 冲突、中断与 retarget

同一 subject/channel 同时只有一个有效 owner：

- `Replace`：默认策略，从当前 presentation value retarget 到新目标；
- `Queue`：显式排队，不能作为交互默认；
- `Ignore`：已有 owner 时拒绝新 plan；
- `Additive`：只有 channel 明确声明可组合时允许。

独立 cancel 必须清除 override/temporary visual 并显示 committed final state。
不提供会在 timeline 已终止后永久保留 override 的 `KeepCurrent` 或
`RevertPresentation`。

从当前表现值继续只能使用原子 retarget：

- 先完整预检 replacement；
- 再在同一 Runtime 事务中采样当前 presentation、转移 channel owner；
- replacement 失败时旧 plan 原样继续；
- “回到旧起点”是一个新 plan，不是 cancel mode 或 source rollback。

source target dispose 时，相关 track 和 temporary visual 必须在同一 Runtime 清理事务中
终止。取消或失败不能留下持续 wakeup。

## 10. Interaction 与 Accessibility

每个可见 track 声明交互几何策略：

- `Committed`：hit-test 使用 committed geometry；
- `Presentation`：hit-test/cursor 使用当前 presentation geometry；
- `NonInteractive`：temporary visual 不参与命中。

第一批 temporary visual 默认 NonInteractive。语义 role/state/action 始终来自 committed
scene；accessibility bounds 在首批保持 committed，除非平台契约明确支持低频
presentation bounds 更新。

不允许画面移动而命中策略未声明。

## 11. Damage 与性能

每个 sampled effect 提供 old/new visual envelope：

```text
damage = union(old_presentation_envelope, new_presentation_envelope)
```

- stroke、shadow、decoration 和 transform outset 必须进入 envelope；
- temporary visual 的创建/移动/删除均产生 damage；
- 后端只有证明 retained 像素等价时才能缩小区域；
- active track 遍历复杂度按 active animation 数量增长，不扫描全树；
- effectively hidden 或 surface suspended 的 track 使用显式 Advance/Pause/Finish
  policy；持续 Connection 效果默认 Pause，短时 transition 默认 Advance；
- 多 track 共享一次 Runtime clock advance 和 frame preparation；
- 持续动画必须声明最大 active 数量、工作量和降级策略。

## 12. 模式与可选性

| 模式 | 行为 |
|---|---|
| Disabled | 直接显示 committed final state，不创建或保留 timeline |
| Enabled | 执行显式 plan 与已安装 Trigger |
| ReducedMotion | 使用 capability 声明的短时 fade、离散或静态 fallback |

全局 Disabled 高于局部请求。局部 capability 可以进一步禁用。模式变化必须原子清理
不再允许运行的 presentation override，并只在像素实际变化时请求帧。

## 13. 首批领域消费者

### 13.1 属性动画

覆盖 opacity、transform、Color、Rectangle 等 typed value，验证 Tween/Keyframes、
并行/顺序/错峰、中断和 current-value retarget。

### 13.2 Layout result transition

布局只计算一次 committed final result。animation 在 old/new stable snapshot 间插值
presentation bounds；不在每帧重新执行 LayoutManager。

### 13.3 Connection route transition

同拓扑 PointList 可以按规范化对应关系插值。不兼容 topology 首批使用 crossfade，
不采用 Draw2D 未声明弧长语义的索引插点作为通用合同。

### 13.4 Viewport transition

覆盖 pan、zoom、fit 和 scroll-to。用户直接输入默认抢占自动 viewport animation；
auto-expose 的实时驱动保持独立，不伪装为普通 Tween。

### 13.5 持续 Connection 效果

至少验证：

- dash phase 沿 source → target / reverse 持续推进；
- moving pulse 按 route arc length 采样位置、tangent 和 normal；
- pulse 到达终点时通过 crossfade/scale 与 committed arrow decoration 交接；
- temporary pulse 清理，不修改 route、hit-test、通知或 history；
- 多 pulse 使用 Stagger，共享 clock，并受并发预算限制。

详细验收例见
[能力丰富度计划](../../roadmap/draw2d-capability-enrichment-plan-2026-10-05.md)与
[领域能力集成矩阵](capability-integration.md)。

## 14. 失败语义

- 非单调时间：拒绝 advance，不改变 timeline；
- 非有限 duration、key time、value、speed 或 spring 参数：admission 失败；
- 无法有限终止的 Spring/Decay、溢出的 Repeat 与阻塞 Sequence 的无限 child：
  admission 失败；
- foreign/disposed target：admission 或下一次 reconcile 结构化失败；
- foreign/consumed transition capture：admission 失败，不产生 override；
- channel 冲突：按 interruption policy 处理，不隐式叠加；
- provider sample error：终止该 plan，清除其 override，显示 committed final state；
- provider panic：沿 Runtime fault 边界处理；
- backend capability 不足：提交前拒绝或使用 plan 已声明 fallback；
- timeline/temporary visual 预算耗尽：拒绝新增 plan，不驱逐无关 active plan。

## 15. 验证与实施切片

目标 suite：`core.p2-m01-animation`，在实现开始前登记到
`verification/suites.toml`。
公开合同门禁详见
[Animation 公开 API 合同提案 §12](public-api-contract.md#12-m01-a-公开合同门禁)。
领域消费者与后续能力接入门禁见
[Animation 领域能力集成矩阵 §15](capability-integration.md#15-实施与能力毕业)。

### M01-A：Clock / Timeline / optionality

状态：`complete`。证据见
[P2-M01A 实现记录](../../verification/reviews/p2-m01a-animation-evidence.md)。

- deterministic time、Tween/Keyframes、Parallel/Sequence/Stagger；
- Disabled/Enabled/ReducedMotion；
- no-active zero work；
- cancel/replace/retarget；
- external typed value consumer。

### M01-B：Presentation / damage

状态：`complete`。证据见
[P2-M01B 实现记录](../../verification/reviews/p2-m01b-animation-presentation-evidence.md)。

- source 与 presentation 分离；
- old/new envelope；
- temporary visual lifecycle；
- hidden/suspend/dispose；
- Runtime fault 与 retry 边界。

### M01-C：领域消费者

状态：`in_progress`。Figure bounds capture/transition 已完成，证据见
[P2-M01C 实现记录](../../verification/reviews/p2-m01c-animation-consumer-evidence.md)。

- property；
- layout result；
- Connection route；
- Viewport；
- dash flow；
- pulse → endpoint decoration。

### M01-D：平台与视觉

- Headless 固定时间 replay；
- Native/Web 相同关键帧像素；
- DPI 1/2、nested transform、scroll/zoom；
- reduced-motion 与 disabled；
- 1/64/1,024 active track 工作量基线。

ADR-026 已接受。各切片先补 public contract test，再实现，不修改
`render_recursive.rs` 主循环协议。
