# Draw2D Connection / Anchor / Router 源码语义

类型：`reference-analysis`

本文记录 M9 所需的 Draw2D 源码事实，不定义 Novadraw 架构。

参考仓库：

- `https://github.com/eclipse-gef/gef-classic.git`
- 基线提交：`4463d9d0ce13c19d10fbe769d29f28b7345a8cba`

## 1. Connection 与 PolylineConnection

`Connection` 在 `IFigure` 之上增加：

- source / target `ConnectionAnchor`；
- `ConnectionRouter`；
- router-specific routing constraint；
- 构成连线的 `PointList`。

`PolylineConnection` 同时承担对象关系、缓存失效和 Figure 行为：

1. `addNotify()` 向 source / target anchor 注册 `AnchorListener`；
2. `anchorMoved()` 调用 `revalidate()`；
3. `revalidate()` 继续调用 router 的 `invalidate(connection)`；
4. `layout()` 调用 router，router 直接通过 `setPoints()` 改写连接点；
5. points 与 children 的 union 决定 bounds；
6. `removeNotify()` 注销 anchor listener，并调用 router 的 `remove(connection)`；
7. source / target decoration 作为 connection child，由 `ArrowLocator` 布局。

旧 bounds 不再被新 bounds 包含时，`layout()` 显式把旧区域加入 UpdateManager，
随后 repaint 并发送 figure moved。

`Polyline` 的 points 是自身几何真源：

- points 变化清空 bounds cache 并发送 `PROPERTY_POINTS`；
- bounds 从 points 与 line width 派生；
- hit-test tolerance 至少为 line width 的一半；
- `PolylineConnection` 再把 locator/decorations child bounds union 到自身 bounds。

## 2. Anchor

`ConnectionAnchor` 提供：

```text
getOwner()
getReferencePoint()
getLocation(reference)
addAnchorListener()
removeAnchorListener()
```

`getReferencePoint()` 与 `getLocation(reference)` 返回 absolute coordinates。
`AbstractConnectionAnchor` 保存 owner 对象；第一个 listener 注册时订阅 owner 的
`AncestorListener`，最后一个 listener 删除时解除订阅。owner 或 ancestor 移动会触发
`anchorMoved`。

`AnchorNotificationTest` 还验证了相对坐标语义：只移动同时包含 connection 和两个
endpoint 的公共坐标祖先不会通知；移动仅包含某个 endpoint 的嵌套坐标祖先会通知。
因此需要观察的是 endpoint 相对 connection routing domain 的有效变换，不是任意祖先
发生过 mutation。

内置语义：

| 类型 | owner | location |
|---|---|---|
| `XYAnchor` | 无 | 固定 absolute point，与 reference 无关 |
| `ChopboxAnchor` | Figure | owner box 中心到 reference 射线与矩形边界的交点 |
| `EllipseAnchor` | Figure | owner box 中心到 reference 射线与椭圆边界的交点 |
| `RoundedRectangleAnchor` | Figure | 矩形边与圆角椭圆分段边界的交点 |
| `LabelAnchor` | Label | 与 Chopbox 相同，但 box 使用 Label icon bounds |

当前基线不存在 `SlopeAnchor`。路线图若要求与该基线对齐，应使用
`RoundedRectangleAnchor`，或明确把 `SlopeAnchor` 标为 Novadraw 自定义扩展。

## 3. Router

`ConnectionRouter` 提供：

```text
getConstraint(connection)
setConstraint(connection, constraint)
route(connection)
invalidate(connection)
remove(connection)
```

Router 接收可变 `Connection` 并直接改写 points。`AbstractRouter` 的 endpoint
算法是双向引用：

```text
source = sourceAnchor.getLocation(targetAnchor.getReferencePoint())
target = targetAnchor.getLocation(sourceAnchor.getReferencePoint())
```

Router 随后把 absolute endpoint 转换到 connection relative coordinates。

### 3.1 NullConnectionRouter

清空 points，仅写入 source / target 两点，即 direct router。

### 3.2 BendpointConnectionRouter

- 以 `Connection` 对象为 key 保存 bendpoint constraint；
- 无 bendpoint 时使用另一端 anchor 的 reference point；
- 有 bendpoint 时，首尾 bendpoint 分别作为 source / target 的 reference；
- 输出 source、全部 bendpoint、target。

Draw2D `Bendpoint` 的 location 位于 connection relative coordinates。

### 3.3 ManhattanConnectionRouter

- 根据 endpoint 相对 owner box 的方向产生正交路径；
- 保存全局已占用 row / column 和每条 connection 的 reservation；
- `invalidate/remove` 释放 reservation；
- router 实例状态会让多条 connection 的路由结果互相影响。

因此它不是纯函数；若迁移到 Novadraw，reservation 必须成为显式策略状态，不能隐藏在
Figure 或全局单例中。

### 3.4 FanRouter

`FanRouter` 继承 `AutomaticRouter`：

- 可先委托另一个 router；
- 按无向 anchor pair 对 connection 分组；
- 同组重叠直线按稳定 index 在中点处错开；
- invalidate/remove 会使后续成员重新验证。

其输出依赖同组 connection 的稳定顺序，不能只给单条 connection 输入而保持 Draw2D
语义。

分组 key 是无向 source/target Anchor pair，并依赖具体 Anchor 的
`equals/hashCode`。Chopbox/Ellipse 等 owner Anchor 提供 value equality；默认
ConnectionAnchorBase 和 XYAnchor 使用对象身份。

## 4. Locator 与 Decoration

`ConnectionLocator` 从 points 读取 source、target 或 polyline 的中部位置：

- `MIDDLE` 在奇数 points 时取中央 point，偶数时取中央两个 point 的线段中点；
- `MidpointLocator(index)` 取指定相邻 point pair 的线段中点；
- `BendpointLocator(index)` 取指定 point；
- `ConnectionEndpointLocator` 根据 endpoint owner 方位、末端 segment、u/v distance
  放置 child。

`PolylineConnection` 默认使用 `DelegatingLayout`，Locator 负责重定位普通 child
Figure。箭头 decoration 也是 child Figure，由 `ArrowLocator` 设置 endpoint 和相邻
point 作为方向参考。Polygon/Polyline decoration 使用 template + scale + rotation
生成自身 points。

Draw2D 不裁短 Connection 主线：`ArrowLocator` 只把 decoration tip 放在 route
endpoint，`PolygonDecoration` 默认三角模板为 `(0,0),(-1,1),(-1,-1)`，默认缩放为
`(7,3)`。它主要依靠 background 继承 foreground、1px outline 和 SWT flat cap 隐藏
主线与箭头的重叠。对于 Novadraw 的粗线/Vello 抗锯齿，该方式会在尖端留下可见的
butt-cap 截面，因此 Novadraw 合理变体是在 paint/hit-test 阶段按 decoration inset
回退中心线，同时保留 endpoint route geometry。

因此 decoration 不需要进入 Connection 的基础绘制协议；它可以保留为普通 Figure，
通过 route-derived locator 参与布局。

## 5. ConnectionLayer 与裁剪

`ConnectionLayer` 继承 `FreeformLayer` 并持有共享 `ConnectionRouter`：

- 新增 Connection child 时把 layer router 设置给 child；
- 修改 layer router 时更新全部现有 Connection children；
- 移除 Connection child 时把 router 恢复为 NullConnectionRouter；
- layer router 因此可以持有 Manhattan reservation 或 Fan grouping 等跨连接状态。

Draw2D 另有 `ViewportAwareConnectionLayerClippingStrategy`。当 connection layer 位于
公共祖先，而 endpoints 位于不同 nested viewport 时，它计算 nearest common viewport
及两端各自 viewport path；端点不可见时连接可能完全不绘制。这不是普通
Freeform overflow clip 自动具备的语义。

## 6. RoutingListener

`PolylineConnection` 可用 `RoutingNotifier` 包装真实 Router。listener 能观察
invalidate、constraint、remove、post-route，甚至在 route 前返回 true 拦截真实
Router。Draw2D 的 `RoutingAnimator` 使用该能力捕获 old/new points 并在播放期间替代
路由。

该机制是动画扩展点，也是非确定性和重入风险来源；Novadraw 可以保留 route committed
观察能力而拒绝 route interception，但必须明确记录为语义变体。

## 7. 对 M9 的约束事实

- endpoint 计算依赖两个 anchor 的 reference / location 双向协作；
- anchor owner 的 move、resize、reparent 和 ancestor transform 都可能改变 endpoint；
- router constraint 的类型由具体 router 决定；
- direct 与 bendpoint router 可表达为纯计算；
- Manhattan 与 Fan 需要显式跨 connection 状态或批量上下文；
- ConnectionLayer router 是共享默认策略，不能降级为每条 Connection 独占 Router；
- Anchor 等价关系影响 Fan 分组；
- points 同时影响 paint、bounds、hit-test、locator、decoration 和 damage；
- route-derived child 与 connection path bounds 之间存在布局顺序约束；
- nested viewport 中的 connection 需要专用 clipping 语义；
- Draw2D 的对象 listener 链和 router 直接写 Figure 是实现方式，不是必须复制的行为
  契约。

## 8. 源码证据

- `org.eclipse.draw2d/Connection.java`
- `org.eclipse.draw2d/PolylineConnection.java`
- `org.eclipse.draw2d/ConnectionAnchor.java`
- `org.eclipse.draw2d/AbstractConnectionAnchor.java`
- `org.eclipse.draw2d/ConnectionAnchorBase.java`
- `org.eclipse.draw2d/XYAnchor.java`
- `org.eclipse.draw2d/ChopboxAnchor.java`
- `org.eclipse.draw2d/EllipseAnchor.java`
- `org.eclipse.draw2d/RoundedRectangleAnchor.java`
- `org.eclipse.draw2d/LabelAnchor.java`
- `org.eclipse.draw2d/ConnectionRouter.java`
- `org.eclipse.draw2d/AbstractRouter.java`
- `org.eclipse.draw2d/BendpointConnectionRouter.java`
- `org.eclipse.draw2d/ManhattanConnectionRouter.java`
- `org.eclipse.draw2d/AutomaticRouter.java`
- `org.eclipse.draw2d/FanRouter.java`
- `org.eclipse.draw2d/ConnectionLocator.java`
- `org.eclipse.draw2d/MidpointLocator.java`
- `org.eclipse.draw2d/ConnectionEndpointLocator.java`
- `org.eclipse.draw2d/ArrowLocator.java`
- `org.eclipse.draw2d/ConnectionLayer.java`
- `org.eclipse.draw2d/ViewportAwareConnectionLayerClippingStrategy.java`
- `org.eclipse.draw2d/RoutingListener.java`
- `org.eclipse.draw2d/RoutingAnimator.java`
- `org.eclipse.draw2d.tests/.../AnchorNotificationTest.java`
