# Connection / Anchor / Router 候选契约

类型：`normative-design`

状态：`approved`

本文定义 M9 的规范架构，由
[`ADR-005`](../../adr/adr-005-connection-routing-contract.md) 接受，并由
[`ADR-008`](../../adr/adr-008-m9-contract-recovery.md) 细化 shared Manhattan scope
与 Core 1.0 viewport topology。各分批只有在实现与验证完成后才视为已交付。

## 1. 目标与边界

M9 提供：

- 由 Anchor 描述端点的 Connection；
- Direct、Bendpoint、Manhattan 与 Fan 路由策略；
- route generation、依赖失效、精确命中和 damage；
- Locator、Decoration 与 ConnectionLayer；
- Viewport、Zoom、Freeform 和深树组合。

M9 不提供 GEF EditPart、EditPolicy、Tool、Request、Command、selection、交互式折点
编辑器或避障图搜索。`ShortestPathConnectionRouter` 继续延后。

M9.1 只接受纯计算边界和错误模型，不建立 ConnectionRuntime 或修改 FigureTree。

## 2. Draw2D 基线与 Novadraw 变体

源码事实见
[`../../reference/draw2d/figure/connection-routing.md`](../../reference/draw2d/figure/connection-routing.md)。

Novadraw 保留：

- source / target anchor 的双向 reference / location 语义；
- route points 驱动绘制、命中、布局和 damage；
- owner 或 ancestor 几何变化触发 reroute；
- router-specific constraint；
- locator / decoration 依赖稳定 route；
- Fan 路由按相同 anchor pair 的 connection 组工作。

Novadraw 不复制：

- Anchor 长期持有 Figure 对象和 listener；
- Router 直接修改 Connection Figure；
- Router 以 Connection 对象地址作为缓存 key；
- Figure 在 layout callback 内主动访问 UpdateManager；
- `Object` 类型的无检查 routing constraint。

对应变体：

- 使用 `FigureId` 和 `ConnectionId` 表达稳定身份；
- Anchor 与 Router 只读取短生命周期、可追踪依赖的 `SceneQuery`；
- route 是返回值，只有 Runtime 能原子提交；
- dependency tokens、constraint、route generation 和 resolution status 属于
  `ConnectionRuntime`；
- Router identity、共享配置和 routing group state 属于 Runtime 私有
  `RouterRegistry`；
- 具体 Connection Figure 保存已提交的局部 point list，只负责表现几何、样式、绘制
  和精确命中；
- 类型不匹配、失效引用、不可逆变换和非有限几何均返回结构化错误。

## 3. 身份与状态归属

`ConnectionId` 与承载它的 `FigureId` 一一对应，但使用独立 newtype，避免把任意
Figure 误传给 connection API：

```rust
pub struct ConnectionId(FigureId);
```

M9.2 的目标状态：

```text
ConnectionRuntime
├── states: ConnectionId -> ConnectionState
├── by_dependency: DependencySubject -> ordered set<ConnectionId>
├── routers: RouterRegistry
└── layer_defaults: FigureId -> RouterId

ConnectionState
├── source: Option<AnchorBinding>
├── target: Option<AnchorBinding>
├── router: RouterBinding
├── constraint: Option<Box<dyn RoutingConstraint>>
├── dependencies: ordered map<DependencySubject, observed generation>
└── route generation / dirty reason / resolution status

RouterRegistry
├── entries: RouterId -> RouterEntry
└── groups: (RouterId, RouterGroupKey) -> RouterGroupState
```

该 registry 不进入 `FigureTree` 或 `FigureNode`。FigureTree 仍只保存 topology、
NodeState、LayoutState 与 Figure 行为。已提交的局部 point list 属于
`ConnectionFigure` 的表现几何，不在 ConnectionState 中复制。

`RouterBinding` 明确区分：

```text
Inherited { layer: FigureId }
Explicit { router: RouterId }
```

Connection 加入带默认 router 的 ConnectionLayer 时，默认使用 `Inherited`；显式
override 不被 layer 后续更新覆盖。移出 layer 后，Inherited binding 回退到内置
Direct Router，Explicit binding 保持不变。修改 layer 默认 router 会原子失效所有
Inherited child，但不修改 Explicit child。切换 Router 或 layer 默认 Router 前必须
先验证所有受影响 constraint；任一类型不兼容时整项操作失败并保留旧 binding、route
和 group membership，不能隐式丢弃 constraint。

## 4. 坐标域

### 4.1 Routing Domain

每条 connection 的规范 routing domain 固定为其 parent 的 child content domain。
ConnectionLayer 中的 connection 因而与同层 freeform child 使用同一逻辑坐标。

Router 返回的所有 points、bendpoints 和 metadata geometry 都位于该 domain。
不以 connection 自身 local domain 作为规范域，因为 connection bounds 由 route
派生；先依赖 bounds 转点、再由点计算 bounds 会形成循环。

提交 route 时：

1. 验证所有 point 有限且至少包含两个点；
2. Connection Figure 通过纯 `prepare_route_geometry()` 预计算 parent-domain
   `path_bounds` 与 local points；内置折线按 stroke、cap/join 与显式 miter limit
   计算，扩展 Figure 可提供自己的几何边界；
3. Runtime 对整批 route geometry 与 Locator placement 完成无副作用预检；
4. 只有整批预检成功后，才在同一 Runtime mutation 中更新 connection NodeState bounds 和
   `ConnectionFigure` 的局部 point list；
5. 随后提交 Runtime dependencies/generation/resolution；任一预检失败时整批进入
   unresolved 并清除旧 route，同时合并各 calculation 已读取的当前 dependency
   observations，使失败状态稳定且输入恢复后可自动重路由；
6. 保存 old path/subtree envelope，待 locator child 完成后统一计算 projected damage。

规范化后的 local point list 是已提交的表现几何真源，NodeState bounds 是同一次
提交得到的 placement；ConnectionState 只保存 generation、dirty reason 和 resolution
status，不复制 point list。调用方不能独立设置 Connection bounds 或 points。

### 4.2 SceneQuery

Anchor 与 Router 使用只读、不可逃逸的查询接口：

```rust
pub trait SceneQuery {
    fn is_attached(&mut self, figure: FigureId) -> bool;
    fn parent_id(&mut self, figure: FigureId) -> Option<FigureId>;
    fn border_box(&mut self, figure: FigureId) -> Result<Rectangle, SceneQueryError>;
    fn anchor_geometry(
        &mut self,
        figure: FigureId,
        key: &AnchorGeometryKey,
    ) -> Result<AnchorGeometry, SceneQueryError>;
    fn map_point(
        &mut self,
        point: Point,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<Point, SceneQueryError>;
    fn map_rect(
        &mut self,
        rect: Rectangle,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<Rectangle, SceneQueryError>;
}
```

`CoordinateSpace` 至少区分：

- `FigureLocal(FigureId)`；
- `ChildContent(FigureId)`；
- `LogicalSurface`。

该接口对 scene 是只读的，但方法使用 `&mut self`，用于显式记录本次读取的 dependency
subjects；不依赖 `RefCell` 等隐藏内部可变性。它不暴露 mutation、Runtime、
UpdateManager、listener 注册或平台类型。

M9.1 可以由 FigureTree 的短生命周期 facade 实现，但不把 FigureTree 本身写入
Anchor / Router trait 签名，以保留测试 fixture 和未来 scene snapshot 的替换空间。

M9.2 路由时使用 `TrackedSceneQuery` 包装真实查询。反向索引使用稳定
`DependencySubject`，ConnectionState 另存本次读取时的 observed generation：

```text
FigureGeometry(FigureId)
NamedAnchorRegion(FigureId, AnchorGeometryKey)
RelativeTransform(CoordinateSpace, CoordinateSpace)
Topology(FigureId)
```

成功 route 后，Runtime 用本次读取集合原子替换 Connection 的旧 dependencies；
失败时保留旧 dependencies，并合并本次失败前已读取或缺失的 subjects，以便相关输入
修复后仍能重新调度。generation 不进入反向索引 key，否则 generation 提升后无法命中
旧 key。`owner()` 只提供 primary owner 身份、诊断和默认 group key，不是完整依赖
声明。

`AnchorGeometry` 至少支持矩形、椭圆、带 corner radii 的圆角矩形和 named local
region。`LabelAnchor` 查询 `icon` named region；若 owner 未提供该 region，返回
`MissingAnchorGeometry`，不能缓存构造时 rectangle。这样 M10 Label 布局变化只需提升
geometry generation，无需替换 Anchor。

## 5. Anchor

### 5.1 接口

Anchor 是不可变策略描述：

```rust
pub trait ConnectionAnchor {
    fn owner(&self) -> Option<FigureId>;
    fn semantic_group_key(&self) -> Option<AnchorSemanticKey> {
        None
    }

    fn reference_point(
        &self,
        scene: &mut dyn SceneQuery,
        output: CoordinateSpace,
    ) -> Result<Point, AnchorError>;

    fn location(
        &self,
        scene: &mut dyn SceneQuery,
        reference: Point,
        reference_space: CoordinateSpace,
        output: CoordinateSpace,
    ) -> Result<AnchorSite, AnchorError>;
}

pub struct AnchorSite {
    pub point: Point,
    pub outward_normal: Option<Vector>,
}
```

`outward_normal` 是 Manhattan 和 decoration 可消费的显式 metadata。缺失时 Router
可从 owner box 与 point 推导；不得用 magic direction integer。

Anchor 不注册 listener。具体依赖来自 tracked SceneQuery，不由 `owner()` 推断；
owner 为 `None` 的 XYAnchor 由 anchor replacement 显式触发失效。

`AnchorBinding` 由 Runtime 分配稳定 `AnchorId`。有效 `AnchorGroupKey` 为 Fan 等跨
连接策略提供等价语义：

- Anchor 返回 semantic key 时，使用 owner、anchor kind 与不可变参数组成 value key；
- 共享的 named port 使用 owner 与 port key 作为 semantic key；
- 未返回 semantic key 的自定义 Anchor 使用 `AnchorId` 实例身份；
- source/target pair 作为无向 key 规范化，不能依赖 trait object 地址或 HashMap
  迭代顺序。

Anchor 创建后不可原位修改参数。XY location、fixed offset 或 named port 改变时，
调用 Runtime endpoint handle 替换 Anchor；该事务同步更新 group membership、
dependency index、route generation 和 damage。

### 5.2 双向求值

默认 endpoint 求值保持 Draw2D 语义：

```text
source_reference = target.reference_point(routing_domain)
target_reference = source.reference_point(routing_domain)
source_site = source.location(target_reference)
target_site = target.location(source_reference)
```

Router 可用首尾 bendpoint 替换相应 reference，但不能改变 Anchor 自身状态。

### 5.3 内置 Anchor

建议 M9 产品集合：

1. `XYAnchor`；
2. `ChopboxAnchor`；
3. `EllipseAnchor`；
4. `RoundedRectangleAnchor`；
5. `LabelAnchor`。

当前 Draw2D 基线不存在路线图中的 `SlopeAnchor`，推荐改为
`RoundedRectangleAnchor`。`LabelAnchor` 不直接依赖 M10 `LabelFigure` 类型，而通过
`AnchorGeometryKey::icon()` 查询 owner-local region；M9 使用 fixture provider
验证协议，M10 的 LabelFigure 再提供真实 icon region。这样 M9 不反向依赖具体 Label
类型，也不会因文本/图标布局变化持有陈旧 rectangle。

source / target 使用 `Option<AnchorBinding>`。任一端未绑定时 Connection 为
`Unresolved::MissingEndpoint`；允许构建和重连流程存在半连接状态，但不得绘制旧
route。两端就绪后才进入 route validation。

## 6. Router

### 6.1 单连接纯计算接口

```rust
pub trait ConnectionRouter {
    fn route(&self, request: RouteRequest<'_>) -> Result<RouteOutput, RouteError>;
    fn constraint_type(&self) -> Option<TypeId>;
}

pub struct RouteRequest<'a> {
    pub connection: ConnectionId,
    pub routing_space: CoordinateSpace,
    pub source: &'a dyn ConnectionAnchor,
    pub target: &'a dyn ConnectionAnchor,
    pub constraint: Option<&'a dyn RoutingConstraint>,
    pub scene: &'a mut dyn SceneQuery,
    pub group: Option<&'a RoutingGroupSnapshot>,
}

pub struct RouteOutput {
    pub points: PointList,
    pub metadata: RouteMetadata,
}
```

`RouteRequest` 中的 Router 由 `RouterBinding` 解析为 `RouterId`，再从
`RouterRegistry` 取得。ConnectionState 不持有独占 Router 对象。`RouterEntry`
包含不可变配置以及可选的 Runtime 私有 group state；删除仍被 Connection 或 Layer
引用的 RouterId 必须失败。

`RoutingConstraint` 沿用 LayoutConstraint 的受控 type-erasure 模式：

- 必须支持 runtime type identity；
- Router 必须显式校验；
- 类型错误返回 `ConstraintTypeMismatch`；
- 不允许忽略未知 constraint 后返回成功。

Bendpoint constraint 必须自带坐标语义：

- `AbsoluteBendpoint` 位于 connection routing domain；
- `RelativeBendpoint` 由 source/target reference、两端 offset 和 `[0, 1]` weight
  计算；
- connection reparent 时，Runtime 要么把 absolute bendpoint 映射到新 routing
  domain，要么原子拒绝 reparent，不能保持数值不变却改变空间含义。

Direct、Bendpoint 和单连接 Manhattan 计算必须是确定性的：相同 snapshot、anchor、
constraint 和 router config 产生相同输出。

Router 组合使用显式 pipeline，而不是 Draw2D `RoutingListener` 拦截：

```text
endpoint resolution
→ base router (Direct / Bendpoint / Manhattan)
→ optional route post-processors (Fan)
→ output validation
```

Fan 只处理 base route 恰有两个有效端点的情况；已有 bendpoint 的 route 保持不变。
同一无向 AnchorGroupKey pair 中，Fan 先按主轴将几何方向规范为向西或向北，再从稳定
child order 推导 lane offset。反向 connection 仍保留自己的 source/target 与 metadata
顺序，但不能因端点方向翻转而同时翻转法向，导致不同 lane 重合。
动画和诊断观察最终提交事件，不进入 pipeline，也不能替代 Router 输出。

### 6.2 跨连接策略状态

Fan 和 Draw2D Manhattan row/column reservation 依赖多 connection 状态。它们不得
读取全局单例或在 Figure 内维护隐藏表。两者的共享范围不同：

```text
ManhattanScope = RouterId + routing domain
FanScope = RouterId + routing domain + unordered AnchorGroupKey pair
```

Manhattan reservation 覆盖同一 RouterId、同一 routing domain 的全部 connection，
不能按 anchor pair 切组；Fan 只处理 source/target anchor pair 相同的 collision。

M9.4 使用两阶段协议：

```text
Runtime builds stable RoutingGroupSnapshot
→ router computes one or more RouteOutput values
→ Runtime validates the complete batch
→ Runtime commits all routes atomically
```

group 顺序由 routing domain parent 的 FigureTree child order 决定。不同 RouterId
或 routing domain 永不共享 reservation；不同 AnchorGroupKey pair 不共享 Fan index。

若单条 connection 的改变会影响组内其他 route，整个受影响 group 同事务失效并重算。

### 6.3 Shared Manhattan Reservation

Runtime 每次从完整 ManhattanScope snapshot 重新派生 row/column reservation，不在
Router 内增量保存 `rowsUsed/colsUsed`：

1. 按稳定 child order 解析 scope 内全部 endpoint 和单连接正交候选；
2. 只调整可移动的内部水平/垂直 segment，不改变 endpoint 和首尾 stub；
3. row 与 column 独立占用，候选 lane 与已占用 lane 的距离不得小于配置的
   `lane_spacing`；
4. 从期望 lane 按 `0, -1, +1, -2, +2, ...` 乘以 `lane_spacing` 的顺序搜索；
5. 完整验证所有输出后一次性提交；任一成员失败则拒绝整个成功 batch。
   运行期随后原子提交整组 unresolved、清空旧 route/reservation 并保留恢复依赖，
   不保留伪装为有效的旧几何；与 ADR-014 一致。

`lane_spacing` 与 `minimum_stub` 是 Router 的命名配置，单位为 routing-domain logical
units。该机制只提供 Draw2D 风格的共享 lane reservation，不宣称提供 obstacle
avoidance。

## 7. M9.1 错误模型

```rust
pub enum SceneQueryError {
    UnknownFigure(FigureId),
    DetachedFigure(FigureId),
    MissingAnchorGeometry(FigureId, AnchorGeometryKey),
    NonInvertibleTransform(FigureId),
    NonFiniteGeometry(FigureId),
}

pub enum AnchorError {
    Scene(SceneQueryError),
    MissingOwner,
    InvalidGeometry,
    NonFiniteResult,
}

pub enum RouteError {
    Source(AnchorError),
    Target(AnchorError),
    ConstraintTypeMismatch,
    InvalidConstraint,
    TooFewPoints,
    NonFinitePoint { index: usize },
    UnsupportedRoutingGroup,
    UnsupportedViewportTopology,
    DependencyCycle,
}

pub enum LocatorError {
    UnknownConnection(ConnectionId),
    UnresolvedConnection(ConnectionId),
    PointIndexOutOfRange { index: usize, point_count: usize },
    DegenerateEndpoint,
    DependencyCycle,
}
```

M9.1 的纯计算调用失败时不产生 `RouteOutput`，也没有任何 mutation。

M9.2 validation 遇到缺少 endpoint、owner remove 或依赖解析失败时，不静默冻结旧
route，而是原子提交带原因的 unresolved 状态：清空已提交 points、damage old visual
bounds，并发送 typed property change。除该显式状态迁移外，不提交部分 route、
bounds 或 locator 结果。重新绑定或依赖恢复后，连接可回到 resolved 状态。

若同一 topology mutation 同时删除 Connection，自身 state、dependency、router group
membership 和 locator constraints 直接原子清理，不再产生中间 unresolved 状态；只有
Connection 存活而其依赖失效时才提交 unresolved。

## 8. 失效与事务边界

M9.2 依赖索引由 tracked SceneQuery 的读取结果产生，至少覆盖：

- owner bounds / visual geometry change；
- owner add、remove、reparent；
- owner 或 ancestor edge transform / coordinate-system change；
- anchor replacement；
- router / constraint replacement；
- routing group membership 或 Z-order change；
- scale / viewport 变化仅改变投影时，不重复计算 content-domain route；
- 影响 anchor 几何的 layout 输出。

validation 使用以下固定顺序：

```text
mutation
→ collect affected ConnectionId / routing groups
→ mark route generation dirty
→ validate owner geometry and named anchor regions
→ validation computes RouteOutput
→ validate complete outputs
→ atomically commit local points and path_bounds
→ layout locator children
→ recompute subtree visual envelope and freeform extent
→ update dependent viewport ranges
→ enqueue old/new projected damage
→ emit typed property notifications
```

callback 中发起的 connection mutation 进入既有 effect queue，不允许 route callback
重入 Runtime mutation。

Anchor 或 Locator 不得依赖其 Connection 自身、该 Connection 的 locator child，或
任何会反向依赖 route 结果的 geometry。Runtime 在注册 binding 和 validation 时检测
dependency graph；发现环返回 `DependencyCycle`，不能依赖 validation 次数上限兜底。

相同 mutation 同时移动 connection layer、source owner 和 target owner 时，只有相对
变换发生变化才需要产生新 route。允许保守重算，但 points 未变化时不得发送虚假的
RouteChanged，也不得扩大 damage。

提交后至少提供以下 typed observation：

```text
ConnectionChange::EndpointBindingChanged
ConnectionChange::RouterBindingChanged
ConnectionChange::ConstraintChanged
ConnectionChange::ResolutionChanged { old, new }
ConnectionChange::RouteChanged {
    old: Arc<RouteSnapshot>,
    new: Arc<RouteSnapshot>,
    old_bounds,
    new_bounds,
}
```

通知只描述已提交状态。内部 dependency invalidation 不通过这些 listener 回流；
`RouteChanged` 仅在规范 route points 实际变化时发送，style-only bounds 变化继续使用
既有 Figure/property notification。old/new snapshot 只存活于事件持有期，允许动画
或诊断读取，但不允许在通知中覆写 Router 输出；没有观察者时 Runtime 不必额外保留
历史 route。

## 9. Figure、命中与 Damage

`ConnectionFigure` / `PolylineConnection` 保存 Runtime 已提交的局部 point list，
但不持有 Anchor、Router、owner ID、依赖索引或 dirty generation。points 写入口是
crate-private connection capability，只能由 Runtime route commit 调用。

- paint 使用 route points 和 style 生成 polyline/path；
- hit-test 使用 point-to-segment distance，并包含 stroke width 与 hit tolerance；
- Connection 自身 visual bounds 只包含 path、stroke、line cap/join 与自身 effect
  overflow；
- locator/decorations 作为普通 children 单独贡献 subtree visual envelope、damage 和
  freeform extent，不反向扩大或平移 Connection 的 `path_bounds`；
- tolerance 只扩大命中，不污染 paint bounds；
- 空或 unresolved route 不绘制且不可命中；
- old/new subtree visual envelope 均参与 damage；
- partial damage 投影继续复用统一 edge transform 与 effective clip。

Connection 的 stroke width、cap/join 或 effect 改变时只重算 path bounds、local
normalization 与 damage，不重复执行 Router；points 改变才发送 typed
`RouteChanged`。

## 10. Locator、Decoration 与 Layer

- Locator 由 Runtime 绑定到 Connection 的直接 child，在 route preflight 中消费准备好的
  local route，输出 child placement；成功提交时保持 child 尺寸并更新 parent-local
  bounds；
- source / target locator 使用首尾点；
- `ConnectionLocator::Middle` 保留 Draw2D 语义：奇数点取中央点，偶数点取中央两个
  点所在 segment 的中点；
- `MidpointLocator(index)` 取 points[index] 与 points[index + 1] 的线段中点；
- 需要按总弧长定位时新增 `PathFractionLocator(f64)`，不改变上述 Draw2D 名称语义；
- locator index 越界返回结构化错误，不能 panic 或回退到原点；
- endpoint decoration 是普通 Figure，方向来自末端非零 segment 或 AnchorSite normal；
- route endpoint 始终保持 Anchor 真值；若 endpoint decoration 声明 line inset，
  Connection 仅在 paint/hit-test 中回退中心线，不修改 committed route points。该
  inset 应略小于 decoration 长度以保留抗锯齿覆盖，避免粗线 butt cap 把箭头尖端削平；
- 零长度末端 segment 返回结构化 locator 错误或使用最近非零 segment；
- `ConnectionLayer` 复用 D2 `Layer` capability、透明命中和 keyed membership；
- ConnectionLayer 不保存平行 topology 或 connection runtime state，只在
  ConnectionRuntime 中注册默认 RouterId；
- 同一 layer 的 children 顺序仍是 paint Z-order、逆序 hit-test 和 routing group
  稳定顺序；
- Core 1.0 使用严格 viewport topology：从 connection parent、source owner、
  target owner 分别向公共 Figure 树根提取有序 viewport chain，三者完全相同时允许
  路由；任一 owner 位于不同 chain 时返回 `UnsupportedViewportTopology`。这样允许
  ConnectionLayer 与节点作为公共 root 下的兄弟，同时拒绝真实 nested viewport
  分叉。ownerless anchor 不声明 nested viewport 可见性，只受 connection 自身
  ancestor chain 裁剪。
- nearest-common-viewport clipping 留待独立 ADR；不能只依赖 Freeform
  `OverflowVisible` 而让跨 viewport connection 泄漏到不可见区域。

Decoration orientation 使用 routing domain 中最后一个非零 segment。若直接消费
AnchorSite normal，法向量跨非均匀 scale/skew 时必须使用 inverse-transpose 规则；
Manhattan Router 的正交轴固定在 routing domain，非轴对齐 normal 需按明确策略选择
dominant axis 或返回结构化错误。

## 11. 扩展点与稳定性

扩展点：

- 新 Anchor：实现只读 `ConnectionAnchor`；
- 新 Router：实现 `ConnectionRouter`，约束类型错误必须可诊断；
- 新 routing group policy：提供确定性 snapshot 与批量输出；
- 新 Decoration：普通 Figure + Locator；
- 新 Connection 外观：实现受 Runtime 控制的 connection geometry capability。
  capability 必须提供无副作用 geometry preparation，不能依赖 commit 后再修正 bounds；

稳定边界：

- `FigureId`、`ConnectionId`、`SceneQuery`、routing domain 与错误原子性属于公共契约；
- 具体依赖索引容器、generation 和 route scheduling 属于 Runtime 私有实现；
- 不承诺与 Draw2D Java 对象身份、整数舍入或 mutable PointList 引用等价。

## 12. 验证入口

### 12.1 M9.1 纯计算契约

- XY、Chopbox、Ellipse、RoundedRectangle 的 reference / location；
- owner local → nested transform → routing domain；
- source / target 双向 reference；
- direct route 两点；
- unknown/detached owner；
- non-invertible transform；
- non-finite geometry；
- constraint 类型错误；
- 失败不产生部分输出；
- 同输入确定性。

M9.1 不以截图为完成门禁。

### 12.2 M9.2-M9.6 Runtime 与产品门禁

- shared RouterId、layer inherited router 与 explicit override；
- bendpoint 首尾 reference、absolute/relative domain 与 reparent；
- Manhattan 每段正交、无重复相邻点和 shared reservation；
- Fan 的 AnchorGroupKey、稳定顺序与批量失效；
- missing source/target 与 unresolved → resolved 恢复；
- tracked geometry/named-region/relative-transform dependencies；
- locator middle、indexed midpoint、path fraction 与越界错误；
- path bounds 不受 decoration envelope 反向影响；
- nested viewport clipping 与 unsupported topology；
- dependency cycle rejection；
- route、bounds、locator、freeform extent、range、damage、notification 因果顺序；
- Figure paint/hit-test、ConnectionLayer 和 `connections-demo`。

## 13. 已接受决策

1. 接受 connection parent child content domain 作为唯一 routing domain；
2. 接受 Anchor / Router 只读查询与 tracked dependency，不复制 listener + direct
   mutation；
3. 接受 `RouterRegistry`、RouterId、layer inherited binding 与 explicit override；
4. 接受 dependency tokens、generation 和 resolution status 归
   `ConnectionRuntime`，已提交局部 points 归 `ConnectionFigure`；
5. 接受 Fan 与 shared Manhattan 使用不同 scope：Manhattan 为
   `RouterId + routing domain`，Fan 再增加无向 AnchorGroupKey pair；两者均使用稳定
   group snapshot 与批量原子提交；
6. 接受 path bounds 与 decoration/subtree visual envelope 分离；
7. 接受固定 validation DAG、dependency cycle 拒绝和 Core 1.0 严格 viewport
   topology；nearest-common-viewport clipping 延后；
8. 接受保留 Draw2D locator 名称语义，另增 PathFractionLocator；
9. 接受用 `RoundedRectangleAnchor` 替换无源码证据的 `SlopeAnchor`；
10. 接受 `LabelAnchor` 查询 named icon region，避免 M9 依赖具体 M10 Label 类型。
