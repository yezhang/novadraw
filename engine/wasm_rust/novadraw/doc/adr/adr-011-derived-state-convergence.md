# ADR-011: Runtime 派生状态收敛事务

类型：`architecture-decision`

## 状态

提议

## 背景

长期架构审计复现两个正常 Runtime frame 无法自行闭合的问题：

- owner geometry 改变后 Connection 保持 Dirty，必须由应用显式 resolve；
- Label 在 parent layout 前按旧 client width shaping，首帧 command 与最终 bounds 不一致。

此外，built-in Viewport layout 在 LayoutOutput 校验前写共享 range state，使“layout
output 原子提交”只覆盖 child bounds，没有覆盖容器派生模型。

局部增加一次 reroute 或额外 full redraw 不能解决 layout、route、freeform、viewport
之间的因果顺序，也会继续让不同 mutation 入口维护不同 invalidation 规则。

## 决策

### 1. 保留两阶段更新，增加稳定化内核

保留 Draw2D 的 Validation -> Damage Repair。Runtime 在两者之间运行受预算保护的
derived-state convergence，固定阶段为：

```text
pending mutations
-> pre-layout metrics
-> layout validation
-> dependency invalidation
-> dirty routing groups
-> repeat until stable
-> final presentation snapshots
-> damage repair / recording
-> notification flush
```

所有产品 frame 入口复用同一 stabilization 内核。

### 2. 使用有序 committed change log

FigureTree 的 direct mutation、LayoutOutput commit、route commit 和 callback mutation
产出统一内部 change log。Connection invalidation、cache invalidation、damage 与 typed
notification 都消费该已提交事实，避免 API-specific 手工旁路。

### 3. 文本拆分 intrinsic 与 presentation

intrinsic metrics 不依赖最终 client width，用于 preferred/minimum size 与 parent layout。
最终 geometry 稳定后才产生 constrained/ellipsis GlyphRun snapshot。presentation refresh
只能 repaint，不能再次改变 layout contribution。

### 4. Routing space 由树推导

Connection 的规范 routing space 是其 parent child-content domain。正常 frame 自动消费
dirty groups；显式 resolve 只保留为诊断工具，不再是应用正确性的必要条件。

### 5. Layout side state 延迟提交

Layout calculation 不得写共享 RangeModel/ViewportRuntime 或同步通知。built-in container
使用 sealed typed layout effects，并与 child bounds 在完整 output 校验后原子提交。

### 6. 有限收敛与结构化失败

使用命名的 `DERIVED_STATE_ROUND_LIMIT`，初始值 16。正常支持链路必须在 4 round
内稳定。超限不提交 partial frame，并记录 `DerivationDidNotConverge`。D4.4 负责将
该错误纳入最终 frame preparation public outcome。

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
- Runtime frame preparation 增加有限循环和 dirty category 诊断；
- route 自动调度会扩大现有测试的行为面。

## 不采用

### prepare_submission 末尾无条件 reroute 一次

不能处理 route commit 反向改变 freeform/viewport geometry，也不能覆盖 LayoutOutput
绕过 Runtime setter 的 invalidation。

### 首帧后强制 full redraw

会提交已知错误帧，且首帧后 pending=false 的问题仍被掩盖。

### 通用依赖 DAG

当前只有少数固定派生系统。DAG 增加注册、拓扑排序和循环诊断复杂度，尚无收益证据。

### LayoutManager 获得 Runtime callback

会允许任意副作用、重入和部分提交，破坏 LayoutSnapshot/LayoutOutput 边界。

### Router 直接修改 ConnectionFigure

违反 ADR-005 的纯计算和 Runtime 原子提交边界。

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
