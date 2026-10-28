# Animation Behavior / Trigger 合同

类型：`normative-design`

状态：`accepted`，按 P2-M01C 分阶段实施

目标 delta：P2-M01

`api_semantics`：`animation.timeline`、`animation.presentation`、
`frame.preparation`、`damage.repaint`

## 1. 目的

本文冻结动画触发层的职责与 Runtime 事务顺序。Trigger 不属于
`AnimationPlan`：Plan 描述一个已经决定启动的动画，Behavior 才决定何时根据
committed fact 创建或 retarget Plan。

```text
AnimationBehavior
├── Scope                 Runtime / Figure
├── Trigger               bounds / property / state / transaction / lifecycle
├── Coalescing            stable transaction 内合并规则
├── PlanFactory           committed facts → AnimationPlan?
└── ReducedMotionFactory  可选；缺省为静态终值

AnimationPlan
├── Target / Track
├── Motion
├── Composition
├── Interruption
└── Suspension
```

显式 `start/transition/retarget` 不需要安装 Behavior，也不伪装成
`ExplicitTrigger` 对象。

## 2. 公开对象与所有权

目标公开形状：

```rust,ignore
let behavior = AnimationBehavior::new(
    AnimationTrigger::FigureBoundsChanged,
    move |context| {
        // context 只读 committed facts 与 stable scene。
        Ok(Some(build_plan(context)?))
    },
)
.scoped_to(figure)
.with_reduced_motion(move |context| Ok(build_short_fade(context)?));

let id = runtime.animations().install_behavior(behavior)?;
runtime.animations().remove_behavior(id)?;
```

- `AnimationBehaviorId` 是 Runtime-scoped、generational、opaque handle；
- Behavior 由 `AnimationService` 独占，factory 不持有 Runtime/Figure 可变引用；
- Figure scope 在目标 dispose 后自动退役；
- Runtime scope 只在显式移除或 Runtime drop 时退役；
- 安装不追溯安装前已提交的事实；
- Core 默认不安装任何 Behavior。

## 3. Committed fact

首批事实为 backend-neutral、可克隆的不可变值：

```text
FigureBoundsChanged { figure, old_bounds, new_bounds }
PropertyChanged     (PropertyChangeEvent)
Lifecycle           { figure, parent, kind }
```

`State` Trigger 复用 typed `PropertyChanged` fact，但只能由
`PropertyKey<V>` 且 `V: DiscretePropertyValue` 构造；
它不是第二套事件系统。`Transaction` Trigger 接收同一 stable epoch 内所有匹配
scope 的 committed facts。

Update/Paint/Prepared/Submitted 等观察事件不是动画事实，不能触发 source 动画。
fact 不包含 presentation sample，也不因动画采样产生。

## 4. 稳定边界与顺序

唯一允许的隐式触发点：

```text
apply pending source mutations
→ converge validation / routing / prepared presentation
→ freeze one stable epoch
→ expose unconsumed committed facts to installed Behaviors
→ create/admit/retarget plans in registration order
→ sample initial presentation
→ build immutable presentation snapshot
→ record frame
→ flush the same committed facts to ordinary listeners
```

这保证：

- source 已经是最终业务状态；
- 第一张可提交帧已包含初始 presentation override，不闪现终值；
- listener 不能在 recording 中重入 Runtime 启动动画；
- backend `Retry/Skipped` 不重复触发 Behavior；
- validation 失败时不消费 fact，也不创建 plan；
- 动画 facts 的消费不影响普通 listener 随后的同批通知。

UpdateManager 为动画消费维护独立 cursor。flush/clear 后 cursor 与 effect 队列一起
复位；不得通过复制第二个无限增长的事件日志实现。

## 5. 匹配、作用域与合并

Trigger：

| Trigger | 匹配 |
|---|---|
| `FigureBoundsChanged` | `FigureMoved` committed fact |
| `property_changed(key)` | 相同 `ErasedPropertyKey` 的 typed property fact |
| `state_changed(key)` | 相同 identity 的 typed discrete-state property fact |
| `Transaction` | stable epoch 内 scope 匹配的全部 committed facts |
| `Lifecycle(kind)` | attach、detach、show、hide |

Scope：

- `Runtime`：匹配任意 Figure；
- `Figure(id)`：只匹配该 Figure；不隐式匹配 descendants；
- 需要 subtree 语义时由调用方安装多个 scope 或显式创建 transaction plan。

默认 coalescing 在单个 stable epoch 内按事实 key 合并：

- bounds：第一条 `old_bounds` + 最后一条 `new_bounds`；
- property/state：按 `(FigureId, ErasedPropertyKey)` 保留第一条 `old_value` +
  最后一条 `new_value`；
- attach/detach 保持拓扑顺序，不互相抵消；
- transaction 保持第一条事实出现顺序；
- 合并后 old == new 的 bounds/property fact 被删除。

不同 stable epoch 不合并。Behavior 按 registration order 运行；同 channel 冲突仍由
Plan 的 `InterruptionPolicy` 决定。

## 6. Factory 约束

PlanFactory 只接收：

- 当前 stable epoch；
- 已匹配并合并的 facts；
- 只读 `StableSceneQuery`。

Factory 可以返回：

- `Some(plan)`：进入现有统一 admission；
- `None`：本次事实不需要视觉过渡；
- 结构化错误：不创建半成品 plan，并记录带 Behavior ID 的失败。

Factory 不得：

- 修改 source、Runtime、FigureTree 或 UpdateManager；
- flush listener；
-读取墙钟、请求重绘或直接提交 backend；
- 保存 `StableSceneQuery` 或 fact borrow；
- 绕过 typed channel 与现有 plan admission。

factory panic 沿 ADR-014 的 Runtime fault boundary 传播，并清理所有 presentation；
普通 factory/admission 错误只终止本次触发，不使 Runtime faulted。

## 7. 模式与 fallback

| Runtime mode | Behavior 行为 |
|---|---|
| `Disabled` | 消费事实，但不调用 factory，不创建 ID/override/wakeup |
| `Enabled` | 调用主 PlanFactory |
| `ReducedMotion` | 调用 capability 专属 reduced factory；未提供则静态终值 |

ReducedMotion factory 是 Behavior 的独立策略，不允许 AnimationService 将所有能力统一
缩短为某个 magic duration。比如 route 可直接终值，enter 可短 fade，viewport 可短 pan，
dash/pulse 可退化为静态 decoration。

模式切换不回放已消费事实，也不复活已结束 plan。

## 8. 生命周期与错误

- 安装先校验 scope namespace 与存活性，失败不占用 ID；
- foreign/stale Behavior ID 结构化拒绝；
- remove 是幂等查询边界：有效 active ID 移除一次，之后 stale；
- scoped Figure dispose 时，先允许 `Detached` lifecycle factory 消费 immutable exit
  capture，再退役 Behavior；
- exit factory 只能创建 `NonInteractive` temporary visual，不能保活 disposed Figure；
- factory 返回的 plan 仍受 channel、target、budget、interaction 与 final-value 校验；
- 同一 facts 不因 Idle、Retry、Suspended 或重复 prepare 被执行两次。

首批实现允许将 lifecycle exit 与 route/viewport factory 延后到对应 M01-C consumer，
但 ID、scope、cursor、coalescing、mode 与 failure 语义不得另起一套。

## 9. 合同门禁

Trigger 基座至少通过以下公开合同：

1. 未安装 Behavior 时 mutation 不创建动画；
2. bounds/property/state 在一个 stable epoch 内正确匹配和合并；
3. factory 在 stable 后、首帧 presentation snapshot 前执行；
4. 同一 facts 在 Retry、Idle 和重复 prepare 下只执行一次；
5. Disabled 不调用 factory；ReducedMotion 只调用专属 fallback；
6. Figure scope 隔离、remove 与 dispose 自动退役；
7. factory error 不创建半成品 plan；factory panic 进入 Runtime fault boundary；
8. listener 仍收到原 committed facts，顺序与 animation 是否启用无关；
9. 无 Behavior 且无 active track 时不增加持续 frame work；
10. registration order 与 channel interruption 结果确定。

实现证据进入 `core.p2-m01-animation`，不建立第二个 Trigger 专用 suite。
