# ADR-005: Connection 路由与依赖状态边界

类型：`architecture-decision`

## 状态

已通过

## 背景

D2 已稳定 Layer、Freeform、负坐标、Viewport 与 Zoom。M9 需要在不同 layer 和坐标域
之间连接 Figure，并在 owner move、resize、reparent、remove 或 transform 变化后
重新路由。

Draw2D 通过 Anchor listener、Router 直接改写 Connection points，以及 Router 内部
以 Connection 对象为 key 的缓存完成该链路。直接复制会让 Figure、Router 和
UpdateManager 互相持有可变引用，破坏 Novadraw 的 Runtime 原子事务和 FigureTree
职责边界。

## 决策

接受：

1. connection parent 的 child content domain 是 route points 的唯一规范坐标域；
2. Anchor 是不可变、可替换的只读策略，不注册 listener；Runtime 使用 tracked
   `SceneQuery` 记录一次成功解析实际读取的几何、named region 和相对坐标链，并以
   dependency token 建立反向索引；
3. Anchor 自身值变化只能通过 Runtime replacement/setter 提交；自定义 Anchor 的
   `owner()` 只用于身份和诊断，不作为完整依赖声明；
4. Router 读取短生命周期 `SceneQuery` 并返回 `RouteOutput`，不直接修改 FigureTree、
   Figure 或 UpdateManager；
5. Router 由 `RouterRegistry` 通过 `RouterId` 共享。Connection 保存
   `RouterBinding::{Inherited, Explicit}`，ConnectionLayer 保存默认 Router；routing
   constraint 仍归单条 Connection；Router 切换与 constraint 兼容性校验原子执行；
6. Direct、Bendpoint 和单连接 Manhattan 是确定性计算；Fan 与共享 reservation 使用
   `RouterId + routing domain + AnchorGroupKey` 分组，通过稳定
   `RoutingGroupSnapshot` 批量计算和原子提交；自定义 Anchor 没有 semantic key 时
   使用 Runtime 分配的 `AnchorId`；
7. source / target 是可选 `AnchorBinding`。任一端缺失、owner 失效或依赖不可解析时，
   Connection 进入显式 unresolved 状态，清除旧表现几何并 damage 旧区域；
8. Router 输出先位于 connection parent child content domain；Runtime 从 points 与
   stroke 派生稳定 `path_bounds`，再把 points 规范化到 Figure local domain，并与
   NodeState bounds 同事务提交；
9. Locator 在 route commit 后布局普通 child；decoration 和 locator child 的 visual
   envelope 参与 subtree damage/freeform extent，但不反向改变 `path_bounds`，从而
   打断 route → bounds → locator → bounds 循环；decoration 可声明 paint-only line
   inset，用于回退粗线中心线，但不得修改 committed route endpoint；
10. validation 顺序固定为 owner layout → route → path commit → locator child layout
    → freeform extent → viewport range → damage/notification；检测到 Anchor/Locator
    依赖环时整项失败；
11. ConnectionLayer 复用 D2 topology，并定义 router 继承与 nested viewport clipping；
    不把 shared router state 或第二棵 connection topology 放入 Layer；
12. 内部一致性不依赖 `AnchorListener` 或 `RoutingListener`。外部观察使用提交后的
    typed route/property event；动画可消费 old/new route snapshot，但不能拦截路由；
13. owner 失效、不可逆变换、非有限几何、constraint 类型错误和无效 locator index
    均结构化失败，不保留看似有效的旧 route；
14. M9 内置 Anchor 与当前 Draw2D 基线对齐，使用 `RoundedRectangleAnchor` 替换无
    源码证据的 `SlopeAnchor`；Anchor trait 保持开放并支持 Runtime 原子替换。

完整接口、坐标、错误和验证候选见
`doc/design/architecture/connection-routing.md`。

## 后果

### 正面

- Anchor、Router 和 Runtime 的读写边界明确；
- route points、bounds、damage 和 locator 使用同一提交结果；
- 自定义 Anchor 的真实读取依赖可追踪，不受单 owner 模型限制；
- Router 可以同时支持 per-connection override、ConnectionLayer 默认值和跨连接状态；
- 跨 layer、Freeform、Viewport 和 Zoom 不需要坐标特例；
- owner remove/reparent 可原子清理依赖；
- Fan 等共享策略具有稳定、可测试的组顺序；
- 自定义 Anchor / Router 不需要访问平台或可变 FigureTree。

### 负面

- 相比 Draw2D 对象模型，需要额外的 SceneQuery facade 和 Runtime registry；
- tracked query、RouterRegistry 和 dependency token 增加 Runtime 内部复杂度；
- route 提交前需要坐标与有限性校验；
- Fan/Manhattan 共享状态不能按单条 connection 独立重算；
- nested viewport connection 需要显式 clipping policy；
- named anchor region 需要 Figure 或布局结果提供稳定 geometry generation。

## 不采用的方案

### Anchor listener 对象链

不采用。owner/ancestor 变化已经进入 Runtime mutation、typed effect 与 validation
流程；tracked SceneQuery 能记录 owner、named region 和相对坐标链的实际读取。再建立
listener cache 链会重复表达依赖并引入重入。

### 只按 owner 建立依赖索引

不采用。自定义 Anchor 可能读取多个 Figure、named region 或 owner 到 routing domain
的相对变换；`owner()` 不是完整依赖声明。

### Router 直接修改 Connection Figure

不采用。它无法保证 route、bounds、damage、locator 和通知原子提交。

### Connection 自身 local domain 保存规范 points

不采用。Connection bounds 由 points 派生，以自身 bounds 为 points 的前置坐标基准会
形成循环依赖。

### 每条 Connection 独占 Router

不采用。ConnectionLayer 默认 router、Manhattan reservation 和 Fan 分组要求多个
Connection 共享同一个 Router 身份与 group state。

### 进程级全局 Router 缓存

不采用。它违反无全局状态约束，也无法与 Runtime 生命周期和事务回滚一致。

### Decoration 反向扩大 path bounds

不采用。Locator 依赖 route，而 route normalization 又依赖 path bounds；让 decoration
反向改变 path bounds 会形成几何收敛环。Decoration 只进入 subtree visual envelope。

### 可拦截的 RoutingListener

不采用。路由拦截会破坏确定性和 Runtime 原子提交。动画与外部观察改为消费提交后的
typed route event。

## 参考

- `doc/reference/draw2d/figure/connection-routing.md`
- `doc/design/architecture/connection-routing.md`
- `doc/design/architecture/layer-and-freeform.md`
- `doc/design/coordinates/coordinate-system.md`
- `doc/design/rendering/update-manager.md`
- Eclipse GEF Classic commit `4463d9d0ce13c19d10fbe769d29f28b7345a8cba`

## 日期

2026-09-06
