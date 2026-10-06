# P2-M01 Animation 架构评审记录

类型：`verification-record`

日期：2026-10-06

状态：`accepted`

目标：评审 ADR-026 与 P2-M01 动画正交模型；ADR 已接受，本文不证明实现完成。

规范候选：

- [ADR-026](../../adr/adr-026-animation-and-presentation-plane.md)
- [Animation / Presentation Plane](../../design/animation/animation-system.md)
- [Animation 公开 API 合同](../../design/animation/public-api-contract.md)
- [Animation 领域能力集成矩阵](../../design/animation/capability-integration.md)
- [Draw2D Animation 源码事实](../../reference/draw2d/figure/animation.md)

## 1. 评审结论

设计已达到人工架构评审条件：

- Target、Motion、Trigger、Composition 职责独立；
- committed truth 与 presentation state 分离；
- Runtime 独占 clock、timeline、override 和 temporary visual；
- 无动画、Disabled 与 active-set 为空均有明确零工作合同；
- layout/route 使用显式 before capture，不从 mutation 后状态猜测旧值；
- cancel、retarget、dispose、suspend 与 provider failure 有终止语义；
- external Figure 通过 typed channel/provider 扩展，不修改 Core 枚举；
- 属性、布局、路由、视口、生命周期、dash flow 与 pulse→arrow 均能映射到同一模型；
- M01-A 至 M01-D 有分阶段消费者与验证门禁。

2026-10-06 用户已接受继续按 ADR-026 推进。实现必须继续遵循 M01-A 至 M01-D
切片与先测试后实现门禁。

## 2. 第一性原理检查

### 2.1 业务真值

通过：

- model、Figure topology、bounds、style、route、Viewport logical state、selection、
  command history 和 accessibility 只存在于 committed plane；
- presentation override 可随时丢弃；
- Disabled 与动画完成后显示同一 committed final state；
- 动画失败不回滚 source transaction。

### 2.2 时间权威

通过：

- Host 只提供 `MonotonicTime`；
- Runtime 使用已有 `advance_time` 与 `next_wake_deadline`；
- Motion 不读取墙钟；
- 同时间幂等、非单调拒绝、掉帧按绝对时间采样；
- Tooltip、animation 与后续时间服务共享一个 Host wakeup。

### 2.3 所有权

通过：

- Runtime 独占 active timeline、owner map、presentation override、temporary visual；
- Figure、LayoutManager、Router、backend 不创建 timer；
- provider sample 不持有 Runtime/Figure 可变引用；
- opaque ID 与 capture 都携带 Runtime namespace/generation。

### 2.4 可选性

通过：

- Core 默认不安装隐式 Trigger；
- 显式 `start/transition` 才产生动画；
- Disabled start 返回 Suppressed，不创建 ID、override 或 wakeup；
- active set 为空时 O(1) 判断，不扫描 FigureTree；
- 无动画应用不构造 AnimationPlan。

## 3. 正交性检查

| 检查 | 结果 | 证据 |
|---|---|---|
| Target 不决定时间 | 通过 | Bounds、route、viewport、dash phase 可分别配 Tween/Spring/Procedural |
| Motion 不持有场景 | 通过 | sample 只消费冻结输入与绝对时间 |
| Trigger 不实现插值 | 通过 | Explicit/State/Transaction/Lifecycle 只创建或 retarget plan |
| Composition 不修改 source | 通过 | Parallel/Sequence/Stagger/Repeat 只组织 Track 时间 |
| Interruption 不混入 Composition | 通过 | Replace/Queue/Ignore/Additive 独立为 owner policy |
| Crossfade 不伪装 Motion | 通过 | 两个 opacity Tween 的 Parallel composition |
| Layout 与 route 不建私有类型层次 | 通过 | 分别使用 bounds/route channel，共享 Timeline |

## 4. Draw2D/GEF 对标

保留的事实：

- 动画可选；
- validation 前后稳定状态可以驱动 layout/route transition；
- layout 与 routing 动画可以共享一次 session；
- 最终状态在动画结束后保持稳定。

有意调整：

| Draw2D | Novadraw |
|---|---|
| 静态全局 Animation session | Runtime-owned AnimationService |
| Singleton Animator | typed channel/provider |
| 同步阻塞 `run(duration)` | Host 驱动单调时间 |
| 每帧写 Figure bounds/points | presentation override |
| 全局 initial/final map | Runtime-scoped one-shot capture |
| revalidate/update 播放循环 | existing frame preparation/damage/submission |

未使用 `org.eclipse.zest` 定义任何目标或算法。

## 5. 状态机与终止检查

通过：

- Scheduled 只在首次 Runtime 时间到达时建立锚点；
- Running 可以 Pause、retarget、complete、cancel、dispose 或 fail；
- terminal transition 清除 owner、override、temporary visual 和 wakeup；
- 独立 cancel 总是回到 committed final state；
- 不允许 cancelled plan 永久冻结旧 presentation；
- retarget 先预检，再原子转移 owner；失败保留旧 plan；
- terminal journal 有界，旧 ID 最终变为 UnknownAnimation。

## 6. Snapshot 检查

通过：

- layout/route transition 在 source mutation 前 capture；
- capture 只冻结目标 channel、geometry、drawing revision 与 envelope；
- capture 不锁树、不承诺 source rollback；
- transition 在 source mutation 后重新稳定 committed scene；
- capture one-shot、Runtime-scoped、不可 Clone；
- exit capture 在 dispose 前创建 immutable visual，随后不保活 Figure。

风险控制：

- capture 数量和 drawing bytes 进入预算；
- stale/disposed/foreign target 结构化拒绝；
- capture admission 失败不产生半个 override。

## 7. Damage 与 Interaction 检查

通过：

- 每个 sampled effect 提供 old/new visual envelope；
- transform、stroke、decoration、temporary visual 进入 envelope；
- hidden/suspended 使用 Advance/Pause/Finish policy；
- temporary visual 默认 NonInteractive；
- Committed/Presentation hit-test 必须显式选择；
- Presentation hit-test 使用与 frame 相同的 sampled snapshot。

M01-C 前不应声称通用 Presentation hit-test 已完成。若该能力延后，Viewport/layout
消费者必须 admission 到 Committed 或 NonInteractive，而不是视觉移动但命中未定义。

## 8. 领域覆盖检查

| 领域 | Committed truth | Presentation | 结果 |
|---|---|---|---|
| property | final style/value | typed value override | 覆盖 |
| layout | final child bounds | bounds override | 覆盖 |
| route | final router PointList/path | path interpolation/crossfade | 覆盖 |
| viewport | final origin/scale | pan/zoom transform | 覆盖 |
| lifecycle | attach/dispose fact | enter override/exit temporary visual | 覆盖 |
| dash flow | StrokeStyle + route | dash phase | 覆盖 |
| moving pulse | route + final arrow | temporary pulse + decoration blend | 覆盖 |
| widgets | input/selection/focus fact | color/opacity/transform | 后续 consumer |
| graph layout | deterministic final result | bounds/route groups | 后续 consumer |
| feedback | Tool/Request state | temporary visual | 后续 consumer |
| Thumbnail | stable scene/viewport | shared snapshot/marker | 后续 consumer |

## 9. 用户指定动画案例

### 9.1 虚线流动

已明确：

- logical length/second；
- source→target 与 reverse；
- absolute time sampling；
- route/dash pattern 更新；
- Disabled/ReducedMotion；
- no reroute/layout/notification/history；
- shared clock 与 visibility budget。

### 9.2 Pulse 到箭头

已明确：

- route arc-length position/tangent/normal；
- Travel 与 Handoff 的 Sequence；
- pulse/arrow crossfade+scale；
- committed arrow 在开始前存在，但 override 必须在下一次 record 前安装，避免闪帧；
- zero-length、missing decoration、cancel、dispose；
- Stagger 与 active temporary-visual budget。

## 10. 实施门禁

### M01-A

- 只做 clock、typed Track、Tween/Keyframes、composition、mode、cancel/retarget；
- 必须有外部 property channel consumer；
- 证明 no-active zero work；
- 不引入 temporary visual 或领域专用 timer。

### M01-B

状态：`complete`。证据见
[P2-M01B 实现记录](p2-m01b-animation-presentation-evidence.md)。

- 已加入 presentation override、envelope、temporary visual 与 lifecycle cleanup；
- 已证明 source/presentation 分离；
- 已覆盖 suspend/dispose/provider failure。

### M01-C

- 加入 capture、bounds、route、viewport、procedural continuous；
- 验证 layout/route/viewport/dash/pulse；
- 明确 Presentation hit-test 支持范围。

### M01-D

- Headless fixed-time replay；
- Native/Web 关键帧；
- DPI 1/2、nested transform、scroll/zoom；
- Disabled/ReducedMotion；
- 1/64/1,024 active track 工作量。

## 11. 已接受决策

用户确认继续建设，以下边界已接受：

1. Runtime-owned 单一动画服务与单一单调时钟；
2. committed/presentation 分层；
3. 四维正交模型；
4. 显式 one-shot capture；
5. cancel 清 override、retarget 原子接管；
6. Core 默认无隐式 Trigger；
7. 首批按 M01-A 至 M01-D 推进；
8. 通用 path morph、shared element、force solver 过程动画和 backend compositor 快路径
   不属于首批。
