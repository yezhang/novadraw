# P2-M01C 领域消费者实现记录

类型：`implementation-verification`

日期：2026-10-07

状态：`complete`

## 1. 已完成

Figure bounds/layout-result transition 基础已实现：

- `AnimationMut::capture_figures` 按调用方顺序冻结 committed bounds；
- `FigureTransitionCapture` 是 Runtime-scoped、不可 Clone、按值消费的 one-shot capture；
- source mutation 先提交最终 bounds，transition 仅生成 old → final 的 presentation
  transform；
- `BoundsTransition` 支持 easing、stagger、interruption 与 suspension；
- unchanged/zero-duration 返回 `Suppressed(NoVisualDelta)`；
- empty、duplicate、foreign、disposed 与不可表示的零尺寸 transition 在 admission
  阶段拒绝；
- LayoutManager 不在动画帧重新执行，hit-test 继续使用 committed geometry；
- `Runtime::transition_bounds_transaction` 在一个不可插入渲染帧的调用中完成
  before stabilize、capture、source mutation、after stabilize 与 transition admission；
- transaction error 保留 before/capture/mutation/after/transition 阶段，source mutation
  成功后不承诺 rollback。

Behavior/Trigger 基座已实现：

- `AnimationBehavior` 独立持有 scope、Trigger、PlanFactory、fact coalescing 与可选
  reduced-motion factory，`AnimationPlan` 不再承担触发职责；
- `AnimationBehaviorId` 是 Runtime-scoped generational handle，Figure-scoped Behavior
  随 dispose 自动退役；
- UpdateManager 对既有 notification effect 队列维护动画消费 cursor，facts 在 stable
  后、presentation snapshot 前消费，普通 listener 仍收到同一批原始通知；
- bounds/property/state/transaction 匹配和单 stable epoch 合并已实现；
- Disabled 消费事实但不调用 factory，ReducedMotion 只调用 capability 专属 fallback；
- recoverable factory/admission error 进入有界 failure journal，factory panic 沿 Runtime
  fault boundary 清理 presentation。

领域消费者已实现：

- `transition_viewport_transaction` 覆盖 pan/zoom/fit 的 committed-first 过渡，复用
  contents Figure transform，Viewport clip 与 hit-test 保持 committed；
- Connection route 同点拓扑逐点插值，不同拓扑使用 immutable old-route temporary
  visual 与 final route crossfade，Router 每次 source transaction 只计算最终 route；
- `Procedural::finite/continuous` 使用 Runtime monotonic clock，continuous plan 保持
  active，支持 Pause、cancel 与 deterministic replay；
- dash flow 只覆盖 presentation dash offset，不修改 committed StrokeStyle 或 route；
- moving pulse 按 polyline arc length 分配段时长，可在同一 Sequence 末段完成
  endpoint decoration scale/fade handoff；
- Attached/Detached/Shown/Hidden lifecycle Trigger、Figure presentation channel 与
  NonInteractive temporary visual 共同覆盖 lifecycle 组合，不保活 disposed Figure。

## 2. 合同覆盖

`novadraw/tests/p2_animation_transition_contract.rs` 当前 12 项：

1. capture → source mutation → transition 保持 source final；
2. unchanged 与 zero-duration suppression；
3. empty、duplicate、foreign、disposed capture；
4. 多 Figure stable-order stagger；
5. 非零尺寸到零尺寸的不可表示变换原子拒绝；
6. Runtime layout transaction 的 before/after stabilize 与无闪帧 admission；
7. transaction capture/mutation 分层错误；
8. Viewport pan 保持 committed origin；
9. compatible Connection route 插值；
10. incompatible Connection route crossfade 与 temporary cleanup；
11. continuous dash phase；
12. arc-length pulse 与 endpoint decoration handoff。

`novadraw/tests/p2_animation_behavior_contract.rs` 当前 7 项：

1. 未安装 Behavior 时 mutation 保持静态；
2. bounds fact 合并与首帧 presentation override；
3. property/state/transaction Trigger 匹配；
4. Retry、Idle 与重复 prepare 不重放 fact；
5. Disabled 与 ReducedMotion factory 分流；
6. scope 隔离、dispose 退役与有界 failure；
7. factory panic 进入 Runtime fault boundary。

`novadraw/tests/p2_animation_contract.rs` 当前 28 项，其中新增 finite/continuous
Procedural lifetime、绝对时间采样、取消与 committed fallback，以及
1/64/1,024 active track 确定性工作量断言。

`cargo xtask verify core.p2-m01-animation`：47 项通过。

## 3. 后续门禁

M01-C 机制与 headless contract 已完成。发布级 M01-D 仍需独立采证：

- Native/Web 相同关键帧像素；
- DPI 1/2、nested transform、scroll/zoom 组合截图；
- Native/Web 实际 runner 的帧时间性能基线。
