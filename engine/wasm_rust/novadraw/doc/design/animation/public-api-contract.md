# Animation 公开 API 合同提案

类型：`normative-design`

状态：`accepted`，M01-A 公开合同 SSOT

目标 delta：P2-M01

`api_semantics`：`animation.timeline`、`animation.presentation`、
`frame.preparation`、`damage.repaint`

## 1. 目的

本文把
[Animation / Presentation Plane](animation-system.md)
收口为可实施的公开调用合同，重点解决：

- Runtime 如何暴露动画能力而不让无动画应用承担额外对象和调度；
- heterogeneous Track 如何保持 typed target/value；
- layout、route 等结果过渡如何取得稳定 before/after snapshot；
- Disabled、ReducedMotion、取消、retarget 和失败分别返回什么；
- 外部 Figure 如何增加 channel，而不修改 Core 枚举。

本文冻结行为形状，不承诺首批 Rust 类型的最终字段布局。
领域 channel 与消费者如何组合见
[Animation 领域能力集成矩阵](capability-integration.md)。

## 2. Runtime 入口与可选性

动画沿用 scoped mutable facade 模式：

```rust,ignore
let animations = runtime.animations();
let outcome = animations.start(plan)?;
```

`Runtime::animations(&mut self) -> AnimationMut<'_>` 不返回全局对象，不泄露
`AnimationService` 所有权，也不需要调用方先安装 service。

- `AnimationMut` 只是 Runtime 可变借用，不因访问而分配 timeline；
- Runtime 默认模式为 `Enabled`，但 Core 默认不安装任何隐式 Trigger；只有显式
  `start/transition` 或应用安装 policy 才产生动画；
- Runtime 始终可以处于 `Disabled`，该模式不创建 active track；
- 未调用动画 API 的应用不需要构造 plan、target、clock 或 scheduler；
- `advance_time` 在 active set 为空时只做 O(1) 判断，不扫描 FigureTree；
- `next_wake_deadline` 返回所有 Runtime 时间服务的最早 deadline，Host 不区分 tooltip、
  animation 或其他服务。

Core 不提供第二套 `tick_animation` 或 backend timer。

## 2.1 Behavior 与 Trigger

隐式策略使用 Runtime-owned Behavior，不把 Trigger 塞入 `AnimationPlan`：

```rust,ignore
let behavior = AnimationBehavior::new(
    AnimationTrigger::property_changed(novadraw::event::property::SELECTED),
    move |context| Ok(build_selection_plan(context)?),
)
.scoped_to(figure)
.with_reduced_motion(move |context| Ok(build_selection_fade(context)?));

let behavior_id = runtime.animations().install_behavior(behavior)?;
runtime.animations().remove_behavior(behavior_id)?;
```

- Behavior = Scope + Trigger + PlanFactory + optional reduced-motion factory；
- factory 只读取同一 stable epoch 的 committed facts 与 `StableSceneQuery`；
- source transaction 稳定后、presentation snapshot 冻结前执行 factory；
- 显式 `start/transition/retarget` 不创建 `ExplicitTrigger`；
- 未安装 Behavior 时普通 mutation 仍是纯静态行为。

完整匹配、coalescing、cursor、dispose 与错误合同见
[Animation Behavior / Trigger 合同](behavior-trigger-contract.md)。

## 3. 身份、状态与启动结果

候选公开类型：

```rust,ignore
pub struct AnimationId { /* opaque Runtime namespace + generation */ }

pub enum AnimationMode {
    Disabled,
    Enabled,
    ReducedMotion,
}

pub enum AnimationStart {
    Running(AnimationId),
    Suppressed(AnimationSuppression),
}

pub enum AnimationSuppression {
    Disabled,
    ReducedMotionStaticFallback,
    NoVisualDelta,
}

pub enum AnimationState {
    Scheduled,
    Running,
    Completed,
    Cancelled,
    Failed(AnimationFailure),
}
```

约束：

- ID 是不透明、generational、Runtime-scoped 的值；foreign/stale ID 结构化拒绝；
- `Disabled` 下 `start` 完成 plan 基础校验，但不分配 ID 或 presentation override，
  返回 `Suppressed(Disabled)`；
- ReducedMotion 可以启动声明过的短时 fallback，也可以返回
  `Suppressed(ReducedMotionStaticFallback)`；
- 没有像素差异时返回 `Suppressed(NoVisualDelta)`，不请求帧；
- query 不推进时间，不采样新值，不产生 damage；
- terminal state 只保留在有界 journal 中；被回收的 ID 返回 `UnknownAnimation`，
  不能无限积累完成记录。

## 4. Plan、Track 与 typed channel

一个 `AnimationPlan` 持有 composition tree 和 plan-level policy。叶节点是 Track：

```text
Track<V>
├── target: TrackTarget<V>
├── motion: Motion<V>
├── interaction: InteractionGeometryPolicy
├── suspension: SuspensionPolicy
└── reduced_motion: ReducedMotionFallback<V>
```

`V` 必须是受检的 backend-neutral 值。Core 首批提供 Color、标量、Point、
Dimension、Rectangle、Affine2D、opacity 和受控 paint/route 值。

每个 target 在 admission 后解析为：

```text
subject identity + typed channel identity + prepared sampler + visual envelope provider
```

- subject/channel 共同构成冲突 key；
- builtin channel 由 Core 提供 typed constructor；
- 外部 Figure 通过 typed `PresentationChannel<V>`/provider 构造
  `TrackTarget<V>`，不注册字符串 property path；
- provider prepare 可以读取稳定 snapshot，但 sample 只能读取冻结输入与时间；
- sample 不持有 Runtime/Figure 可变引用，不产生 source work；
- 内部可以类型擦除 heterogeneous Track，公开构造与 admission 仍保持 typed。

`Additive` 只对明确声明组合代数、identity 和 clamp 规则的 channel 开放。首批未声明的
channel 一律按单 owner 处理。

## 5. Timeline 与终止规则

公开 builder 至少表达：

```rust,ignore
Timeline::track(track)
Timeline::parallel(children)
Timeline::sequence(children)
Timeline::stagger(interval, children)
Timeline::delay(duration, child)
Timeline::repeat(count, behavior, child)
```

`Pause` 是 active timeline 的运行状态，不作为与 Delay 重复的 composition node。

终止规则：

- Tween/Keyframes 必须有有限 duration；零 duration 表示同一事务直接显示终值；
- Spring/Decay 必须同时声明 settle tolerance 与 `max_duration`；
- Procedural 必须声明 `Finite(duration)` 或 `Continuous`；
- `Repeat::Forever` 只能位于允许 continuous 的根组合中，不能阻塞 Sequence 后续 sibling；
- finite Repeat 的 count 必须大于零且乘法时长不能溢出；
- Sequence、Stagger 使用稳定 child order；相同时间点的采样和完成顺序确定；
- 非有限 duration/key time/value、逆序 keyframe 和无法终止的有限组合在 admission
  阶段拒绝。

这些规则确保 Headless 固定时间 replay 不依赖刷新率或真实 sleep。

## 6. 时间锚点

Runtime 只接受 `MonotonicTime`：

- Runtime 已有时间时，`start` 以最后一次成功 `advance_time` 为锚点；
- Runtime 尚未收到时间时，plan 进入 `Scheduled`，第一次成功 advance 建立锚点；
- 同一时间重复 advance 幂等，不产生重复 damage；
- 非单调时间返回既有 `TimeError`，active timeline、presentation 和 wakeup 不变；
- Host 掉帧时直接采样新的绝对时间，不逐帧累加；
- backend `Skipped/Retry` 不回退动画时间；下次成功帧使用新的 Runtime 时间，
  中间帧允许自然丢弃。

## 7. Before/After Snapshot 协议

属性 Tween 可以显式携带 start/end value。Layout、route、reparent 和 lifecycle
不能在 source mutation 完成后猜测旧状态，必须使用显式 capture：

```rust,ignore
let capture = runtime
    .animations()
    .capture(TransitionTargets::figures([figure]))?;

runtime.figure(figure)?.set_bounds(final_bounds)?;

let result = runtime
    .animations()
    .transition(capture, transition_spec)?;
```

合同：

- `capture` 先稳定当前 committed scene，只冻结所选 target 的值、geometry、drawing
  revision 和 visual envelope；
- capture 是 Runtime-scoped、one-shot、不可 Clone 的 opaque handle；
- capture 不锁定 FigureTree，不阻止后续 source mutation，也不承诺 source rollback；
- `transition` 再次稳定 committed scene，验证 namespace、target generation 与 channel；
- admission 失败时 source mutation 保持已提交，capture 被消费或显式返回给调用方，
  但不得产生半个 presentation override；
- before/after 无视觉差异时返回 `Suppressed(NoVisualDelta)`；
- 已 dispose target 的普通 transition 返回 `DisposedTarget`；
- exit animation 必须在 dispose 前使用专门的 immutable visual capture，随后只驱动
  NonInteractive temporary visual，不保活 Figure。

Core 默认不自动为普通 mutation 建立 capture。Behavior/Transaction Trigger 只是上述
协议的可选封装，不能形成第二套 snapshot 语义。

## 8. 冲突、取消与原子 Retarget

同一 subject/channel 同时只有一个 owner。新 plan admission 使用：

- `Replace`：原子 retarget；
- `Queue`：等待 owner terminal，仅用于显式非交互流程；
- `Ignore`：返回已有 owner，不创建新 plan；
- `Additive`：仅限 channel 已声明组合代数。

独立 `cancel(id)` 的唯一默认结果是：

1. 移除 track、temporary visual 与 wakeup；
2. 清除 presentation override；
3. 显示已经存在的 committed final state；
4. damage 覆盖当前 presentation 与 committed visual envelope。

不提供会无限残留表现状态的独立 `KeepCurrent` 或 `RevertPresentation`：

- “从当前值继续”必须调用原子 `retarget(id, new_plan)`；
- retarget 先完整预检新 plan，再在同一 Runtime 事务中采样当前 presentation 并转移
  channel ownership；
- retarget 失败时旧 plan 原样继续，不出现 committed final state 闪烁；
- “回到旧起点”是一个以历史值为目标的新 plan，不是 source rollback 或 cancel mode。

这避免 cancelled timeline 已无 wakeup、但 presentation override 永久残留的状态。

## 9. Suspension、Visibility 与模式切换

每个 plan 显式声明：

```rust,ignore
pub enum SuspensionPolicy {
    Advance,
    Pause,
    Finish,
}
```

- `Advance`：暂停提交但逻辑时间继续，恢复时采样当前绝对时间；
- `Pause`：恢复后平移 local time，不产生隐藏期间跳变；
- `Finish`：进入 suspend 时清除 override，直接显示 committed final state。

continuous Connection effect 默认 `Pause`；短时业务 transition 默认 `Advance`。
effectively hidden target 使用同一 policy，不发明 Figure 私有 timer。

模式切换原子执行：

- 切到 Disabled：终止 active plan、清除 override/temporary visual、重算 damage；
- 切到 ReducedMotion：仅 retarget 到已声明 fallback；无 fallback 时直接终止；
- 切回 Enabled：不自动复活已终止 plan，调用方必须重新启动；
- 任一切换只在像素发生变化时请求帧。

## 10. 错误模型与失败原子性

候选错误边界：

| Error | 边界 |
|---|---|
| `RuntimeFaulted` | Runtime 已进入 fault boundary |
| `InvalidPlan(AnimationPlanError)` | duration、keyframe、spring、repeat、value 或 envelope 非法 |
| `ForeignTarget` | Figure、capture、provider 或 AnimationId 来自其他 Runtime |
| `DisposedTarget` | admission/reconcile 时 target generation 已失效 |
| `UnknownAnimation` | stale、terminal journal 已回收或不存在的 ID |
| `ChannelConflict` | policy 不允许接管当前 subject/channel |
| `UnsupportedPresentationCapability` | Core/backend 无法实现且未声明 fallback |
| `BudgetExceeded(AnimationBudgetKind)` | active track、temporary visual 或 frame work 超限 |
| `CaptureConsumed` | one-shot capture 重复使用 |
| `ProviderFailed(ProviderError)` | extension prepare/sample 返回结构化失败 |

失败原子性：

- start/transition admission 失败不创建 ID、owner、override、wakeup 或 damage；
- cancel 失败不改变目标 plan；
- retarget 失败保留旧 owner 与采样状态；
- sample error 终止该 plan、清除其 presentation，显示 committed final state；
- provider panic 沿 ADR-014 使 Runtime faulted，不伪装为普通 `ProviderFailed`；
- budget 耗尽只拒绝新工作，不驱逐无关 active plan。

## 11. Interaction 与观测

`InteractionGeometryPolicy` 是 Track admission 的必填项：

- `Committed`：hit-test/cursor/accessibility bounds 使用 committed geometry；
- `Presentation`：hit-test/cursor 使用同一 sampled presentation snapshot；
- `NonInteractive`：temporary visual 不参与输入或 accessibility。

首批 temporary visual 只支持 `NonInteractive`。Accessibility role/state/action 始终来自
committed scene。

观测使用 pull query 或有界 terminal journal；首批不在 sample 热路径执行任意用户
completion callback。需要业务回调时，由 Host 在提交后稳定边界消费 terminal journal。

## 12. M01-A 公开合同门禁

首批 contract test 必须仅通过公开 API 证明：

1. Disabled start 返回 Suppressed，零 allocation/wakeup/damage；
2. 首次时间锚点、同时间幂等与非单调拒绝；
3. typed Tween/Keyframes 与外部 channel consumer；
4. Parallel/Sequence/Stagger 的稳定采样顺序；
5. Spring/Decay 有界终止，非法组合 admission 原子失败；
6. generational/foreign ID 与 target 拒绝；
7. cancel 清除 override，retarget 无终值闪烁且失败保留旧 plan；
8. capture → source mutation → transition 的 before/after 快照；
9. active set 为空时 Runtime 不扫描树、不请求 redraw、不提交空帧；
10. Disabled/ReducedMotion/Enabled 模式切换的原子清理。

实现测试存在后，才把 `core.p2-m01-animation` 写入
`verification/suites.toml`。
