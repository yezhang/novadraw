# P2-M01A Animation Clock / Timeline 实现证据

类型：`implementation-verification`

日期：2026-10-06

状态：`complete`

目标：关闭 P2-M01 的 M01-A Clock / Timeline / optionality 切片。P2-M01 整体仍为
`in_progress`，Presentation Plane 与领域消费者属于 M01-B/C/D。

规范：

- [ADR-026](../../adr/adr-026-animation-and-presentation-plane.md)
- [Animation / Presentation Plane](../../design/animation/animation-system.md)
- [Animation 公开 API 合同](../../design/animation/public-api-contract.md)
- [Animation 领域能力集成矩阵](../../design/animation/capability-integration.md)

## 1. 实现边界

新增 `novadraw::animation`：

- Runtime-owned `AnimationService`；
- scoped mutable `AnimationMut<'_>`；
- Runtime-scoped generational `AnimationId` 与 typed `AnimationChannel<V>`；
- `AnimationValue` 外部扩展协议；
- Tween、Keyframes、Spring、Decay；
- Parallel、Sequence、Stagger、Delay、finite Repeat/Reverse；
- Enabled、Disabled、ReducedMotion static fallback；
- Replace、Ignore、cancel 与 atomic retarget；
- active/channel/track/terminal-history 预算；
- 与 Tooltip 共用 `Runtime::advance_time` / `next_wake_deadline`。

typed channel 保存：

```text
committed final value + optional presentation override + channel owner
```

Timeline 只写 override。完成、取消、模式切换或 channel 移除会清除 override，重新显示
committed final value。

## 2. 正交性

- Target：`AnimationChannel<V>`；
- Motion：Tween/Keyframes/Spring/Decay；
- Trigger：M01-A 只实现 Explicit `start`；
- Composition：Parallel/Sequence/Stagger/Delay/Repeat；
- Interruption：与 Composition 分离的 Replace/Ignore。

同一 channel 的 Sequence/Repeat 每个时间点只采样当前生效 Track。Parallel 中同一
channel 的重叠 Track 在构造阶段拒绝。

## 3. 时间与可选性

- 首次 `advance_time` 为 Scheduled plan 建立锚点；
- 相同时间重复 advance 幂等；
- 非单调时间在 Tooltip/Animation 共享入口拒绝，动画状态和值不变；
- 掉帧按绝对时间采样，不逐帧累加；
- active set 为空不遍历 Track；
- Disabled/ReducedMotion 不创建 AnimationId、owner、override 或 wakeup；
- active timeline 的 wake deadline 与 Tooltip deadline 取最早值。

## 4. Motion 与 Composition

### Tween / Keyframes

- 显式 start/end 或从当前 presentation 值开始；
- Linear/EaseIn/EaseOut/EaseInOut；
- keyframe offset 有限、位于 `[0,1]`、严格递增且包含首尾；
- terminal value 必须等于 channel committed final value。

### Spring / Decay

- Spring 显式 mass、stiffness、damping、tolerance 与 max duration；
- Decay 显式 rate 与 max duration；
- 非有限、非正或派生溢出参数在构造时拒绝；
- 两者到 max duration 必须显示 committed terminal value。

### Repeat / Reverse

- 只支持有限 count；
- Reverse 反转 Track 时间位置与 Motion 方向；
- current-value Tween 不能静态反转，结构化返回 InvalidComposition；
- repeat 展开前检查 active-track 预算，避免超量分配。

## 5. 身份、冲突与失败原子性

- foreign/stale channel 与 AnimationId 分层拒绝；
- Replace 在新 plan 完整预检后取消旧 owner；
- Ignore 返回既有 owner，不创建半成品 timeline；
- retarget 从当前 presentation 值开始；
- retarget 预检失败时旧 plan、owner 和 override 原样保留；
- channel removal 取消完整 owner 并使旧 typed handle 失效；
- terminal state 保存在有界 journal，避免无限增长。

## 6. 外部扩展证据

`p2_animation_contract.rs` 在 Core 外定义 `Percent`：

```rust,ignore
impl AnimationValue for Percent {
    fn is_valid(&self) -> bool { /* domain invariant */ }
    fn interpolate(&self, target: &Self, progress: f64) -> Self { /* typed interpolation */ }
}
```

该类型不修改 Core 枚举或内部 match，即可创建 typed channel、Tween 并由 Runtime
确定性采样。

## 7. 自动验证

正式 suite：`core.p2-m01-animation`。

结果：

- `cargo xtask verify core.p2-m01-animation`：14 项通过；
- `cargo clippy -p novadraw --all-targets -- -D warnings`：通过；
- `cargo test -p novadraw`：完整 crate 单元、集成与 doctest 通过；
- `cargo fmt --all -- --check`：通过。

覆盖：

1. Disabled 零工作与 ReducedMotion static fallback；
2. 首次时钟锚点、绝对时间、同时间幂等、非单调拒绝；
3. Tween/Keyframes/Spring/Decay；
4. Parallel/Sequence/Stagger/Repeat/Reverse；
5. 同 channel 序列边界与重叠拒绝；
6. Replace/Ignore/cancel/retarget；
7. final-value mismatch 原子拒绝；
8. foreign/stale handle；
9. channel removal 与 terminal state；
10. 外部 typed value consumer。

## 8. 未宣称完成

M01-A 不提供：

- Figure opacity/transform/bounds 等内置 presentation channel；
- old/new visual envelope 与 damage；
- temporary visual；
- layout/route/viewport capture；
- lifecycle transition；
- dash flow 或 moving pulse；
- Native/Web 动画像素与工作量基线。

上述能力按 M01-B、M01-C、M01-D 推进，不得用 typed channel 单测替代。
