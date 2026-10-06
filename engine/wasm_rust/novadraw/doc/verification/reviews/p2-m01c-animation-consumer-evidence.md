# P2-M01C 领域消费者实现记录

类型：`implementation-verification`

日期：2026-10-07

状态：`in_progress`

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
- LayoutManager 不在动画帧重新执行，hit-test 继续使用 committed geometry。

## 2. 合同覆盖

`novadraw/tests/p2_animation_transition_contract.rs` 当前 5 项：

1. capture → source mutation → transition 保持 source final；
2. unchanged 与 zero-duration suppression；
3. empty、duplicate、foreign、disposed capture；
4. 多 Figure stable-order stagger；
5. 非零尺寸到零尺寸的不可表示变换原子拒绝。

## 3. 待完成

- LayoutManager 稳定结果的一站式 capture/stabilize facade；
- Connection compatible route transition 与 incompatible crossfade；
- Viewport pan/zoom/fit transition；
- continuous procedural motion、dash flow；
- arc-length pulse 与 endpoint decoration handoff。
