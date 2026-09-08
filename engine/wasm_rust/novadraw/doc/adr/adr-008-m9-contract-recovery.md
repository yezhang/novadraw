# ADR-008: M9 共享 Manhattan 与 Viewport Topology 收口

类型：`architecture-decision`

## 状态

提议

## 背景

ADR-005 已接受 RouterId、routing domain、稳定 group snapshot、批量原子提交和
nested viewport policy，但 M9 当前实现只完成单连接 Manhattan 路由，并未实现
Draw2D `ManhattanConnectionRouter` 的共享 row/column reservation，也没有让
`RouteError::UnsupportedViewportTopology` 进入真实执行路径。

2026-09-08 Draw2D Core 能力审计因此将 M9 从 `complete` 回退为 `in_progress`。
D3.1 需要恢复已接受契约，同时避免把 ShortestPath obstacle routing 或完整
nearest-common-viewport clipping 扩入 Core 1.0。

Draw2D 源码还表明两类共享路由状态具有不同作用域：

- Manhattan reservation 由同一个 router 实例的全部 connection 共享；
- Fan collision index 只在无向 source/target anchor pair 相同的 connection 之间共享。

因此 ADR-005 中把二者统一描述为
`RouterId + routing domain + AnchorGroupKey` 的表述需要细化。

## 决策

### 1. 分离两种 group scope

Manhattan reservation scope：

```text
ManhattanScope = RouterId + routing domain
```

Fan collision scope：

```text
FanScope = RouterId + routing domain + unordered AnchorGroupKey pair
```

不同 RouterId 或 routing domain 永不共享 reservation。Manhattan 不按 anchor pair
切组，否则不同端点 connection 仍会占用同一 row/column，无法保持 Draw2D 语义。

### 2. 使用完整快照批量计算

Runtime 为受影响的 ManhattanScope 构建稳定快照：

```text
RoutingGroupSnapshot
├── router_id
├── routing_space
└── members ordered by FigureTree child order
```

快照必须包含该 scope 中所有仍有效的 connection，而不只是本次 dirty connection。
任一成员的 endpoint、owner geometry、constraint、membership 或 child order 变化，
都使整个 scope 失效。

计算顺序固定为：

```text
resolve every member input
→ compute unreserved orthogonal candidates
→ allocate shared rows/columns in stable member order
→ validate every output
→ atomically commit the complete batch
```

任一成员失败时不得提交部分 route 或部分 reservation。

### 3. Reservation 是派生快照

Runtime 不增量维护“释放后可能残留”的可变 row/column 表。每次 scope 重算时从完整
group snapshot 重新派生 reservation：

- 只 reservation 可移动的内部水平/垂直 segment；
- endpoint 和首尾 stub 保持 Anchor 真值；
- 候选 lane 使用 routing-domain logical units；
- lane spacing 与 minimum stub 使用命名配置，不复制 Draw2D 的 magic number；
- 从期望 lane 按 `0, -1, +1, -2, +2, ...` 的稳定偏移顺序搜索；
- row 与 column 独立占用，候选坐标与既有 lane 的距离必须不小于 lane spacing；
- 同一 routing domain 的 connection 都是该 parent 的直接 child，因此 child order
  已提供无歧义的稳定顺序；
- 输出保持正交、有限、无相邻重复点。

这保留 Draw2D 的共享避让行为，但使用不可变输入和原子提交替代 Router 内部可变缓存。
该能力是 lane reservation，不宣称提供通用 obstacle avoidance。

### 4. Core 1.0 使用严格 Viewport Topology

D3.1 不实现 nearest-common-viewport clipping。Core 1.0 采用保守规则：

1. 从 routing domain 到 connection、source owner、target owner 分别提取有序
   viewport chain；
2. 三者 chain 完全相同时允许路由，绘制继续使用现有 ancestor clipping；
3. 任一 owner 位于不同 nested viewport chain 时返回
   `RouteError::UnsupportedViewportTopology`；
4. ownerless anchor 不声明 nested viewport 可见性，只受 connection 自身 ancestor
   chain 裁剪，不据此推断另一端 viewport；
5. topology 失败进入既有 unresolved transaction：清除旧 route、damage 旧 visual
   bounds，并保留可恢复依赖。

未来若确有跨 nested viewport 连线需求，再通过独立 ADR 增加
nearest-common-viewport clipping；不得在 D3.1 中扩张通用 clipping provider。

### 5. 事务与状态边界

- group membership、snapshot 和 reservation 归 Runtime 私有 ConnectionRuntime；
- Router 仍是只读计算策略，不直接修改 FigureTree；
- ConnectionFigure 只保存最终提交的局部 points；
- FigureTree child order 是 scope 内稳定顺序真源；
- 不新增进程级全局状态；
- 不修改 `render_recursive.rs`；
- route、bounds、locator、freeform extent、viewport range、damage、notification
  继续遵守 ADR-005 的提交顺序。

## 错误与恢复

- divergent viewport chain：`RouteError::UnsupportedViewportTopology`；
- group snapshot 中任一输入失效：整组不提交；
- 输出非有限、非正交或少于两个点：整组不提交；
- topology 或输入恢复后，dependency invalidation 使整组重新计算；
- connection 删除或切换 RouterId 后，下一次完整 snapshot 自然释放其 reservation。

## 验证

### Shared Manhattan

- 同 scope 多条 connection 的内部 row/column 不冲突；
- 不同 RouterId 或 routing domain 的 reservation 相互隔离；
- child order 重排得到确定的新分配；
- connection add/remove、endpoint move 和 router replacement 触发整组重算；
- 任一输出失败时 route、bounds、reservation 和 generation 均无部分提交；
- repeated validation 在输入未变化时保持相同输出。

### Viewport Topology

- connection 与两端 owner 均无 nested viewport 时通过；
- 三者处于相同 viewport chain 时通过；
- source/target 或 connection 位于不同 chain 时返回
  `UnsupportedViewportTopology`；
- 已提交 route 转为 unsupported 时清除旧 geometry 并 damage；
- reparent 回受支持 chain 后重新 resolved；
- ownerless anchor 不产生虚假的 viewport 可见性保证。

### 产品证据

- 扩展 `m9_connection_runtime`；
- `connections-demo` 增加 shared Manhattan 与 unsupported viewport topology 场景；
- macOS 截图与人工交互复核；
- Web/Headless 使用同一 Runtime route 结果。

## 后果

### 正面

- 恢复 ADR-005 与 M9 完成状态的一致性；
- Manhattan 共享状态具有确定作用域和原子回滚边界；
- 不需要把 mutable reservation 放入 Router 或全局单例；
- 通过显式拒绝防止跨 nested viewport 的不可见连线泄漏。

### 代价

- 单条 connection 变化可能触发整个 ManhattanScope 重算；
- Core 1.0 暂不支持跨不同 viewport chain 的 connection；
- child order 成为共享 lane 分配的可观察输入，Runtime Z-order mutation 必须使
  routing group 失效。

## 不采用

### 按 AnchorGroupKey 划分 Manhattan reservation

不采用。它只适合 Fan collision，同一 RouterId 下不同 endpoint pair 仍可能竞争
相同 row/column。

### Router 内部增量保存 rowsUsed/colsUsed

不采用。它依赖调用顺序，失败回滚和 connection remove 容易留下陈旧 reservation。

### 直接实现通用 obstacle avoidance

不采用。Draw2D Manhattan reservation 不是 ShortestPath obstacle routing，后者仍是
明确延后能力。

### 在 D3.1 修改递归渲染或通用 ClippingStrategy

不采用。严格 topology 拒绝可以先保证正确性，通用多矩形 clipping 留待独立需求。

## 关系

- 细化并补充 ADR-005，不替换其 routing domain、依赖追踪和原子提交决策；
- D3.1 完成后，M9.4/M9.6 与 `connection.router` 可恢复为 `complete/verified`；
- D3.2 的 child-order Runtime mutation 必须触发本 ADR 定义的 scope 失效。

## 日期

2026-09-08
