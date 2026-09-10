# ADR-011: Runtime 派生状态收敛事务

类型：`architecture-decision`

## 状态

已通过

## 背景

长期架构审计复现两个正常 Runtime frame 无法自行闭合的问题：

- owner geometry 改变后 Connection 保持 Dirty，必须由应用显式 resolve；
- Label 在 parent layout 前按旧 client width shaping，首帧 command 与最终 bounds 不一致。

此外，built-in Viewport layout 在 LayoutOutput 校验前写共享 range state，使“layout
output 原子提交”只覆盖 child bounds，没有覆盖容器派生模型。

局部增加一次 reroute 或额外 full redraw 不能解决 layout、route、freeform、viewport
之间的因果顺序，也会继续让不同 mutation 入口维护不同 invalidation 规则。

## 决策

### 1. 保留两阶段更新，增加类型化稳定化内核

保留 Draw2D 的 Validation -> Damage Repair。Runtime 在两者之间运行受预算保护的
derived-state convergence。主算法不是每轮无条件重跑全部阶段，而是由 Runtime
固定定义静态依赖层级，并使用稳定去重的 typed worklist 增量求闭包：

```text
pending mutations
-> pre-layout metrics
-> layout validation
-> dependency invalidation
-> dirty routing groups
-> post-route geometry / freeform / viewport
-> final presentation snapshots
-> damage repair / recording
-> notification flush
```

每类工作携带 subject generation；同一 generation 的重复工作合并。后置阶段产生
前置阶段工作时，从对应优先级继续排空，而不是重新扫描全部阶段。正常支持链路应由
静态依赖方向一次推进到稳定；有限重复预算只用于捕获未知反馈环或 invariant 破坏。

所有产品 frame 入口复用同一 stabilization 内核。

### 2. 分离内部失效、外部通知与 damage

FigureTree 的 direct mutation、LayoutOutput commit、route commit 和 callback mutation
产出统一 committed facts，避免 API-specific 手工旁路。事实按用途投影为三个容器：

- typed dirty worklist：按 subject、reason 与 generation 合并，驱动派生计算；
- ordered notification journal：保留提交后外部可观察事件的因果 FIFO；
- damage accumulator：合并 old/new visual envelope。

内部失效不依赖 notification 回流，外部事件也不承担调度职责。

### 3. 文本区分自然测量、约束测量与 presentation

2026-09-10 按 ADR-014 修订：Label/TitleBar 自然尺寸可独立于最终 client width；
换行文本必须在 layout 阶段按父级约束测量高度/baseline 并保存对应 TextLayout，
再 arrange。最终 ellipsis/alignment 等纯 presentation 只能 repaint；
不能把需要反馈布局的换行放到 presentation，也不要求自然/约束测量共用同一快照。

### 4. Routing space 由树推导

Connection 的规范 routing space 是其 parent child-content domain。正常 frame 自动消费
dirty groups；显式 resolve 只保留为诊断工具，不再是应用正确性的必要条件。

### 5. Layout side state 延迟提交

Layout calculation 不得写共享 RangeModel/ViewportRuntime 或同步通知。built-in container
使用 sealed typed layout effects，并与 child bounds 在完整 output 校验后原子提交。

sealed 限制引擎状态写入，不禁止外部组件通过受校验 prepared output 发布私有派生
快照。不可变输入是行为契约，不表示 Rust `&self` 已禁止所有共享可变副作用。

### 6. Stable epoch、有限收敛与结构化失败

每次 source transaction 分配 derivation epoch。只有 worklist 排空后，该 epoch
才晋升为 stable epoch；render、listener 与 Accessibility 只能消费 stable snapshot。

使用命名预算限制 state-changing commit 总数，以及同一 subject 在同一 source
revision 下的重复计算次数。可保留 16 次 epoch/feedback 上限作为最终保险，但正常
支持链路不得依赖多轮重跑。超限不晋升 stable epoch、不生成 partial frame，也不
flush notification，并记录 `DerivationDidNotConverge`，包含阶段、subject、generation
和剩余 dirty categories。D4.4 负责将该错误纳入最终 frame preparation public outcome。

## 后果

### 正面

- 应用只修改 source state 即可得到同帧一致画面；
- direct、callback 与 layout mutation 共享失效语义；
- route/text/viewport 的中间状态不再对 listener 可见；
- 不需要引入通用 DAG、全局状态或 FigureTree 可变逃逸；
- 后续 Tooltip/Accessibility 可消费稳定 scene snapshot。

### 代价

- UpdateManager 的 validation 与 damage recording 需要显式分开；
- Label/TitleBar cache 需要拆为 intrinsic 与 presentation 两层；
- Viewport/RangeModel 写入需要迁移到 validated commit effect；
- Runtime frame preparation 增加 typed worklist、generation 与稳定 epoch 诊断；
- route 自动调度会扩大现有测试的行为面。

## 不采用

### prepare_submission 末尾无条件 reroute 一次

不能处理 route commit 反向改变 freeform/viewport geometry，也不能覆盖 LayoutOutput
绕过 Runtime setter 的 invalidation。可以保留集中式 route drain，但只能作为 typed
worklist 的一个阶段，由 committed geometry facts 自动入队，并继续传播其后续影响。

### 首帧后强制 full redraw

会提交已知错误帧，且首帧后 pending=false 的问题仍被掩盖。
Full redraw 只改变 damage 范围，不得绕过 stabilization；任何 full-frame recording
入口在 typed worklist 未排空或当前 derivation epoch 未晋升为 stable epoch 时必须失败，
不能把 `full_redraw_pending` 作为派生状态正确性的补救手段。

### 运行时可注册的通用依赖 DAG

不允许插件注册任意节点、任意边和任意执行回调。当前只有少数固定派生系统，动态 DAG
会增加注册、拓扑排序和循环诊断复杂度，尚无收益证据。

这不表示拒绝依赖建模。Runtime 必须显式维护编译期固定的 typed dependency graph，
并按其优先级驱动 worklist；已知结构环在注册或提交前拒绝，预算只兜底未知动态反馈。

### LayoutManager 获得 Runtime callback

会允许任意副作用、重入和部分提交，破坏 LayoutSnapshot/LayoutOutput 边界。
允许的替代是只写 `LayoutOutputBuilder` 与 sealed typed layout effects；它们不能读取
Runtime 可变状态，也不能自行提交或同步通知。

### Router 直接修改 ConnectionFigure

违反 ADR-005 的纯计算和 Runtime 原子提交边界。
Router 可以构造临时 RouteOutput/RouteBatch，最终仍由 Runtime 校验并按 group 原子提交。

## 关系

- 延续 ADR-002 effect queue；
- 延续 ADR-003 Runtime 组合根；
- 补齐 ADR-005/ADR-008 的自动 route 调度；
- 修订 ADR-007 文本 cache 的阶段顺序；
- 对应 D4.1 与 `frame.preparation`、`layout.manager`、
  `update_manager.two_phase`、`connection.router`、`text.flow`、
  `viewport.scroll_zoom`、`damage.repaint`。

规范细节见
[`../design/architecture/derived-state-convergence.md`](../design/architecture/derived-state-convergence.md)。

## 日期

2026-09-09
