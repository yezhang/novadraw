# 派生状态收敛事务

类型：`normative-design`

状态：`approved`

范围：D4.1

## 1. 目标

Runtime 接受一次 source mutation 后，应在同一次 frame preparation 中得到一致的：

- Figure bounds 与布局；
- intrinsic text/border metrics；
- Connection route、locator 与 decoration placement；
- Freeform extent、Viewport range 与 coordinate transform；
- constrained text/image presentation snapshot；
- damage、RenderCommand 与提交后通知。

应用不需要手工调用 route resolve，也不需要请求第二次 full redraw 才得到正确结果。

## 2. Draw2D 基线与 Novadraw 变体

Draw2D 的 DeferredUpdateManager 固定执行：

```text
performValidation
-> repairDamage
```

Figure validation 内同步执行 layout；ConnectionRouter 由 Connection validation 路径消费；
绘制只发生在 Figure 均已 valid 后。

Novadraw 保留“validation 先于 damage repair”的因果顺序，但不复制 Figure 主动修改
Connection 或访问 UpdateManager 的对象网络。Route、text shaping、resource snapshot 等
派生服务仍由 Runtime 拥有，通过只读输入计算，再由 Runtime 原子提交。

## 3. 状态分层

### 3.1 Source state

Source state 只由用户、平台输入或已排队 callback mutation 改变：

- Figure topology、style、content 与显式 size/constraint；
- Resource Pending/Ready/Failed 输入；
- Anchor、Router 与 routing constraint 绑定；
- Viewport/Zoom 的显式用户输入。

### 3.2 Pre-layout derived state

Pre-layout state 不依赖最终 container width：

- Label 完整文本和图标的 natural metrics；
- TitleBarBorder 的 intrinsic title metrics；
- Image ready 状态与 natural dimensions；
- 由 source/resource revision 决定的 measurement cache key。

这些值可以改变 preferred/minimum size，因此变化必须进入 validation。

### 3.3 Geometry derived state

Geometry state 由 LayoutSnapshot/LayoutOutput 与规范坐标链得到：

- NodeState bounds；
- Freeform extent；
- Viewport range、view location 与 child transform；
- 依赖最终 owner geometry 的 named anchor region。

### 3.4 Route derived state

Route 只读取已提交 geometry snapshot。成功结果按 routing group 原子提交 Connection
points 与 bounds；失败结果将受影响 group 转为 unresolved 并清除旧 route。

### 3.5 Presentation derived state

Presentation snapshot 只依赖已经稳定的 geometry：

- constrained/ellipsis text layout；
- glyph origin 与 alignment；
- ImageResourceRef 与最终 destination rectangle；
- owner-scoped Border paint snapshot。

Presentation refresh 只能产生 repaint，不能反向改变 intrinsic size、NodeState bounds、
constraint 或 parent layout。违反该约束属于引擎 invariant error。

## 4. 静态依赖层级与 typed worklist

Runtime 使用单一 stabilization 入口，供产品 submission 与低层 frame 入口复用：

```text
ApplyPendingMutations
-> DrainCommittedChanges
-> drain typed work by fixed priority
   -> IntrinsicMetrics
   -> LayoutRoot
   -> GeometryChangePropagation
   -> RoutingGroup
   -> PostRouteGeometry
   -> stop when every work queue is empty
-> FinalizePresentationSnapshots
-> RepairDamageAndRecord
-> FreezeSubmission
-> FlushCommittedNotifications
```

依赖方向固定为：

```text
source/resource
  -> intrinsic metrics
  -> layout geometry
  -> route
  -> locator / visual envelope / freeform extent / viewport range
  -> presentation
```

Viewport 的 view/scale 投影变化在相同有效 viewport chain 内不得反向触发 content-domain
route。Connection 的 route-derived bounds 不得反向成为 owner layout 的 intrinsic
contribution。Anchor/Locator 的结构依赖环继续在注册或计算时拒绝。

每类 work item 至少包含：

```text
kind
subject
source_revision
observed_generation
```

队列按树顺序或 routing group 稳定顺序处理，并按
`(kind, subject, source_revision, observed_generation)` 去重。处理后置工作时如果提交
事实产生更高优先级工作，调度器回到该优先级继续排空；不重新扫描无 dirty work 的阶段。

阶段不可由 app 重排。Runtime 不能在 worklist 排空前记录 command，也不能在稳定化
中途 flush listener。固定优先级是内部静态依赖图，不是插件可注册的通用 DAG。

## 5. Committed facts 与三类日志

直接 Runtime API、callback mutation、LayoutOutput commit、route commit 与资源完成必须
产出同一种 committed fact。最小 fact 类型包括：

```text
GeometryChanged
TopologyChanged
CoordinateSystemChanged
NamedAnchorRegionChanged
StyleOrContentChanged
ResourceDependencyChanged
```

committed fact 是已提交事实，不是可拦截回调。它按用途投影为三个彼此独立的容器：

1. `DerivedWorkSet`
   - 内部调度使用；
   - 按 subject、dirty reason 与 generation 合并；
   - 驱动 Connection dependency、intrinsic/presentation cache、freeform 与 viewport。
2. `NotificationJournal`
   - 外部观察使用；
   - 保持 committed mutation 的因果 FIFO；
   - 只在 stable epoch 晋升后 flush。
3. `DamageAccumulator`
   - 保存 old/new visual envelope；
   - 可以空间合并，但不得承担派生状态调度。

同一事实不能由每个 Runtime API 各自手写一套 invalidation。Layout commit 绕过
Runtime public setter 仍会产出相同 committed fact。

## 6. Layout 原子提交

Layout calculation 只能读取 immutable snapshot，并产出待校验 output。它不得直接：

- 写共享 RangeModel/ViewportRuntime；
- 同步调用 RangeListener；
- 修改 FigureTree；
- flush notification。

内置容器需要更新 range/view state 时，可在 crate 内产出 sealed typed layout effect。
提交顺序为：

```text
calculate child bounds + built-in effects
-> validate complete output
-> capture old geometry/state
-> commit child bounds and effects atomically
-> append committed facts
-> enqueue notifications
```

第三方 LayoutManager 不获得任意副作用 callback；新的通用 effect 只有形成稳定领域契约
后才能公开。

## 7. 自动路由

- Connection 的规范 routing space 由其 parent child-content domain 推导；
- 正常动态更新不要求应用传入 routing space；
- dirty connections 按 RouterId、routing domain 与 router-specific group key 形成稳定组；
- 每组先用同一 stable scene snapshot 计算完整 batch，再原子提交；
- geometry/topology fact 由 `DerivedWorkSet` 命中 dependency reverse index；
- route commit 可能使 locator/freeform/viewport 再次 invalid，因此将对应 typed work
  按固定优先级入队；
- explicit resolve 仅保留为诊断/同步工具，不是产品正确性的前置步骤。

一个 route group 失败不会回滚无关 group；失败 group 清除旧 geometry，进入稳定
unresolved 状态并保留用于恢复的 dependencies。

## 8. 文本双阶段

文本 API 明确分为：

1. `measure_intrinsic`：不使用最终 client width，产出 parent layout 所需 natural
   metrics；
2. `layout_presentation`：使用稳定 client area，产出 wrapping/ellipsis/alignment 与
   GlyphRun snapshot。

Label/TitleBar preferred size 只读取 intrinsic metrics。presentation cache key 可以包含
最终 width/height，但 cache refresh 不得使 Figure revalidate。

这不要求通用 TextFlow 在 D4.1 完整实现；现有 Label 与 TitleBar 先遵守同一分层，
后续 TextFlow 复用该协议。

## 9. Stable epoch、预算与错误

每次 source transaction 分配单调 `derivation_epoch`。Runtime 可以在该 epoch 内提交
工作状态，但只有所有 typed work queue 排空后才将其晋升为 `stable_epoch`。
RenderSubmission、listener 和 Accessibility snapshot 只能消费 stable epoch。

预算至少同时覆盖：

- 本次 epoch 的 state-changing commit 总数；
- 同一 `(kind, subject, source_revision)` 的重复计算次数；
- 最终 feedback/epoch 次数上限。

`DERIVED_STATE_FEEDBACK_LIMIT = 16` 可以保留为最终 feedback 保险，但不得把“正常在
4 round 内稳定”作为算法正确性的依据。正常支持链路应沿静态依赖方向完成；已知结构环
必须提前拒绝，预算用于捕获 invariant 破坏、第三方策略不稳定或未知动态反馈。

达到预算仍有工作时：

- 不生成基于部分状态的新 submission；
- 保留 pending work 与 full redraw 请求；
- 不晋升 stable epoch；
- 记录结构化 `DerivationDidNotConverge`，包含阶段、subject、source revision、
  observed generation、state-changing commit 数和剩余 dirty categories；
- 不 flush 本次未形成稳定 frame 的 committed notification；
- 不在渲染热路径打印日志。

Source mutation 已经提交，不因派生失败回滚。后端继续保留上一 stable frame；当前
epoch 的派生查询返回明确的 `NotStable`/preparation error，而不是伪装成新稳定结果。
这不要求 Runtime 复制整棵旧场景。修复输入或策略后，Runtime 从保留的 dirty work
重试。

D4.1 形成内部 checked stabilization 结果；D4.4 再把 Idle、Suspended、
AwaitingCompletion 与全部 preparation error 收敛为最终公共 outcome。

## 10. Pending work

`Runtime::has_pending_update` 至少合并：

- pending callback mutations；
- 非空 typed derived work queues；
- resource delta；
- damage/full redraw；
- in-flight retry work。

只存在 Dirty Connection 时也必须请求 host 下一次 frame。

## 11. 通知顺序

- 通知描述 committed facts；
- 同一 source mutation 产生的 effects 保持因果 FIFO；
- 同一 epoch 中产生的 effects 暂存，stable epoch 晋升后一次 flush；
- listener 不能观察 route 已 dirty 但 geometry 尚未提交等中间状态；
- unresolved route 是可观察的稳定结果，不属于部分提交。

## 12. 扩展点

### 新 LayoutManager

只消费 LayoutSnapshot 并返回 LayoutOutput；不能获得 Runtime 或共享模型可变引用。
内置容器可通过只写 `LayoutOutputBuilder` 产生 sealed typed effect。effect 在完整 output
校验后由 Runtime 提交，不能自行读取可变 Runtime、重入 mutation 或同步通知。

### 新 Router

只消费 SceneQuery/RouteRequest 并返回 RouteOutput；group scope 和 constraint compatibility
由 Router trait 声明，提交仍归 Runtime。Router 可以维护计算所需的私有 scratch state，
但不能修改 ConnectionFigure 或 FigureTree。

### 新文本引擎

分别支持 intrinsic measurement 与 constrained presentation；产出同一不可变
TextLayout/GlyphRun IR。D4.4 补齐外部非空构造器。

## 13. 验证

- direct set_bounds 后，正常 submission 自动 reroute；
- LayoutManager、callback、reparent 与 layer reorder 触发同一自动 invalidation；
- endpoint remove 同帧清除旧 route 并稳定 unresolved；
- shared Manhattan group 在自动路径保持完整 batch；
- Label 首次 submission 使用最终 width，不需要第二次 full redraw；
- ellipsis/alignment 使用最终 client area，presentation refresh 不 revalidate；
- freeform + viewport + connection 组合在预算内稳定；
- 非法 LayoutOutput 不修改 RangeModel，也不发 listener；
- synthetic feedback cycle 命中预算且不提交 partial frame；
- 同一 generation 的重复 work item 被合并；
- route-only dirty work 使 `has_pending_update` 为 true；
- 收敛失败时 render/listener 仍停留在上一 stable epoch；
- prepare_submission、prepare_frame 与 record_full_frame 共用 stabilization 内核。

## 14. 非目标

- 运行时可注册任意节点、任意边和任意 callback 的通用依赖 DAG scheduler；
- 多线程/worker layout；
- ShortestPath routing；
- 完整 TextFlow/富文本编辑；
- backend session、dispose/identity 与递归渲染性能修复，它们分别属于 D4.2-D4.5。
