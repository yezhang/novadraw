# M8-M9 Viewport 与 Connection 契约计划

类型：`normative-design`

状态：M8 `behavior_verified`，等待手工窗口验收；M9 `not_started`

本文定义 M8 和 M9 的实施契约。M8 必须先完成自动验证与手工验收，M9 才进入
实现阶段。

## 1. 设计目标

- M8：Viewport、RangeModel、ScrollPane、ScrollBar、滚轮和 zoom
  进入同一 Figure 树、坐标转换、Validation 与 Damage Repair 协议。
- M9：Connection、Anchor、Router、Locator、Decoration 和 ConnectionLayer
  复用 M1-M8 的通用机制，不在 apps 层增加坐标或刷新特例。
- 保持 `BlockId` 引用树，不引入对象引用环、Singleton 或全局状态。
- 不修改受保护的递归渲染主循环；新增能力通过 Figure 协议、图运行时和
  UpdateManager 接入。

## 2. Draw2D 对标与合理变体

参考：

- `Viewport.java`、`ViewportLayout.java`
- `RangeModel.java`、`DefaultRangeModel.java`
- `ScrollPane.java`、`ScrollPaneLayout.java`、`ScrollBar.java`
- `ScalableFigure.java`、`IScalablePane.java`、`ScalableLayeredPane.java`
- `Connection.java`、`PolylineConnection.java`
- `ConnectionAnchor.java`、`ConnectionRouter.java`

`AutoexposeHelper` 和 `ViewportMouseWheelHelper` 位于 GEF，而不是 Draw2D。前者依赖
EditPart/Tool 的拖拽生命周期，不进入 M8；后者只作为“未消费 wheel 沿宿主层寻找
Viewport”的行为参考。

Novadraw 保留以下语义：

1. RangeModel 是滚动范围和值的唯一真源。
2. Viewport 是单 contents 的裁剪窗口和坐标根。
3. ScrollPane 组合 Viewport 与水平/垂直 ScrollBar。
4. Anchor 输出绝对坐标，Connection 在所属父坐标域中保存路由点。
5. owner 移动、祖先变化、滚动和缩放都会使依赖 Connection 失效。

Rust 变体：

- 使用 `BlockId` 代替 Java Figure 引用。
- RangeModel 使用 `f64`，与 Novadraw 几何系统一致。
- Router 是纯计算策略；约束和依赖关系由 FigureGraph 运行时持有。
- 公开 typed handle/controller 执行跨 Figure 原子更新，不暴露 `dyn Figure`
  downcast，也不把 Viewport/Connection 特例塞进通用 Figure trait。
- Figure 通过通用 `ChildPolicy` 声明单子节点或多子节点约束，由 FigureGraph 在
  add/reparent 入口统一执行；Viewport 不依赖类型名判断来维护单 contents 不变量。

## 3. M8 公开契约

### 3.1 RangeModel

```rust
pub trait RangeModel: Send + Sync {
    fn minimum(&self) -> f64;
    fn maximum(&self) -> f64;
    fn extent(&self) -> f64;
    fn value(&self) -> f64;
    fn is_enabled(&self) -> bool;

    fn set_all(
        &self,
        minimum: f64,
        extent: f64,
        maximum: f64,
    ) -> Result<RangeChangeSet, RangeModelError>;
    fn set_value(&self, value: f64) -> Result<RangeChangeSet, RangeModelError>;
}
```

不变量：

- 所有输入必须有限。
- `minimum <= maximum`，`extent` 限制在 `[0, maximum - minimum]`。
- `value` 限制在 `[minimum, maximum - extent]`。
- `set_all` 原子更新后再产生变化集合，禁止暴露中间非法状态。

`DefaultRangeModel` 不使用全局状态。Viewport 与 ScrollBar 在 crate 内通过同一个
`ViewportRuntime` 共享水平/垂直 `Arc<dyn RangeModel>`；公共 API 只暴露 snapshot
和 typed handle，不暴露可绕过 FigureGraph/UpdateManager 的可变模型引用。

### 3.2 Viewport

- `ViewportFigure` 仍是 Figure 树中的坐标根。
- 一个 Viewport 最多有一个 contents；设置新 contents 必须原子移除旧 contents。
- view location 直接来自水平/垂直 RangeModel。
- `client_area` 位于 content 坐标域。
- child transform 统一表达：

```text
parent = content - view_location + viewport_origin
content = parent - viewport_origin + view_location
```

- scroll 变更重绘固定的 viewport 可见区域，并产生 `viewLocation` property effect。
- contents track width/height 在 ViewportLayout 中生效，不由 app 修正 bounds。

`ViewportHandle` 是图内 typed handle，保存私有 `BlockId` 和共享状态。所有会影响
图的 setter 都要求传入 `FigureGraph` 与 `UpdateManager`，保证状态变化、通知、
Validation 和 repaint 位于同一事务。

### 3.3 ScrollPane 与 ScrollBar

- `ScrollPaneFigure` 是组合容器；标准构造器一次创建 pane、viewport、两条 scrollbar。
- `ScrollPaneLayout` 根据 visibility policy（Never/Automatic/Always）计算三者 bounds。
- `ScrollBarFigure` 持有共享 RangeModel，负责自身 step/page/thumb drag 交互。
- step/page/thumb drag 都写入同一个 RangeModel。
- wheel 先按 Draw2D 语义投递给当前 mouse target；未消费时由引擎层
  `ViewportWheelController` 沿 target 祖先寻找最近 ScrollPane，行为对标 GEF
  `ViewportMouseWheelHelper`，apps 只做 winit 输入适配。
- `NovadrawContext` 增加通用的指定 Figure repaint/invalidate 请求，使 ScrollBar
  更新共享模型后可在同一事务重绘 viewport 与 pane；该能力不绑定具体 Figure 类型。

### 3.4 Scalable

- `ScalableFigure` 是独立能力接口，`ScalableLayeredPaneFigure` 提供标准实现。
- zoom 由 scalable pane 承担，Viewport 只管理 scroll，与 Draw2D 所有权一致。
- `ZoomManager` 绑定 scalable pane 与 viewport，并通过 `ZoomScrollPolicy` 协调
  中心缩放、鼠标锚点缩放、zoom levels 与 fit 操作。
- Viewport 与 scalable pane 的变换继续由嵌套 `ChildTransform` 组合，不建立第二套
  坐标 API。
- Viewport 不保存 zoom，也不提供 zoom 操作；旧原型入口已删除，不能形成第二个
  zoom SSOT。

`AutoexposeHelper` 属于 GEF Tool/EditPart 交互层，移出 M8 完成门禁。未来进入
GEF roadmap 时再基于 Viewport typed handle 实现。

### 3.5 可行性结论

| 方案点 | Draw2D 依据 | Novadraw 落点 | 结论 |
|---|---|---|---|
| RangeModel 作为 SSOT | Viewport 和 ScrollBar 共享同一 RangeModel | crate-private `ViewportRuntime` + public typed handle | 可行 |
| Viewport 单 contents | `Viewport.setContents` 原子 remove/add | 通用 `ChildPolicy::Single` 由 graph add/reparent 统一校验 | 可行 |
| Viewport scroll 坐标 | `translateToParent/FromParent` 使用 view location | 现有 `ChildTransform` 平移 | 可行 |
| Scalable pane zoom | `ScalableLayeredPane` 独立于 Viewport | 嵌套 `ChildTransform` 统一缩放 | 可行 |
| Zoom 协调 | `AbstractZoomManager` + `IZoomScrollPolicy` | `ZoomManager` + `ZoomScrollPolicy` | 可行 |
| ScrollPane 自动布局 | `ScrollPaneLayout` + `ScrollPaneSolver` | 扩展 `LayoutContext` 控制子节点 bounds/visibility | 可行 |
| ScrollBar 交互 | ScrollBar 自有 step/page/thumb drag | Figure callback + 指定 Figure repaint 请求 | 可行 |
| wheel fallback | GEF `ViewportMouseWheelHelper` 沿 owner 关系寻找 viewport | 未消费 wheel 沿 Figure ancestor 查找 ScrollPane | 可行，属于 Novadraw 引擎层合理变体 |
| Autoexpose | GEF `AutoexposeHelper` 依赖 Tool/EditPart | 延后到 GEF roadmap | 不属于 M8 |

主要实现风险不是渲染能力，而是事务一致性：共享 RangeModel 的任何变化都必须通过
typed handle 或 Figure callback 进入 UpdateManager。若暴露裸可变模型，Viewport
可能移动但宿主没有 dirty region，因此该入口明确禁止。

### 3.6 M8 完成门禁

- `m8_viewport_contract.rs`：单 contents、RangeModel clamp、track policy、嵌套转换、
  scroll/zoom damage、wheel fallback。
- `scroll-pane-demo`：自动/始终/从不显示 scrollbar，wheel、thumb/page、zoom。
- `viewport-app`：现有四场景全部通过截图与人工验收。
- 更新 `viewport.scroll_zoom`、`clipping.strategy`、`coordinate.conversion`、
  `hit_test.search`、`damage.repaint`、`update_manager.two_phase`。
- M8 完成后先输出手工验证步骤和验收记录，再进入 M9。

## 4. M9 公开契约

### 4.1 Anchor 与 Router

```rust
pub trait ConnectionAnchor: Send + Sync {
    fn owner(&self) -> Option<BlockId>;
    fn location(&self, graph: &FigureGraph, reference: Point) -> Point;
    fn reference_point(&self, graph: &FigureGraph) -> Point;
}

pub trait ConnectionRouter: Send + Sync {
    fn route(&self, request: &RouteRequest<'_>) -> Result<PointList, RouteError>;
}
```

首批 Anchor：

- `ChopboxAnchor`
- `EllipseAnchor`
- `SlopeAnchor`
- `LabelAnchor`
- `XYAnchor`

首批 Router：

- `DirectConnectionRouter`（默认）
- `BendpointConnectionRouter`
- `ManhattanConnectionRouter`
- `FanRouter`

`ShortestPathConnectionRouter` 明确延后，不进入 M9 门禁。

### 4.2 Connection 运行时

- `PolylineConnectionFigure` 只持有可绘制的共享点集和样式。
- FigureGraph 持有 connection runtime：source/target anchor、router、constraint、
  dirty 标记与 owner 反向依赖索引。
- owner bounds、ancestor、viewport scroll/zoom 变化后，图级事务统一 reroute。
- reroute 先保存旧 bounds，再计算新点集，将 old/new bounds union 加入 damage。
- 路由器输入使用绝对坐标；输出在写入 Figure 前转换为 Connection 父坐标域。
- 删除或重挂 Connection/owner 时必须原子更新依赖索引。

该分层保持 Figure 只负责绘制，FigureBlock/FigureGraph 承载运行时关系，避免
Connection 自己持有 Figure 引用或回调环。

### 4.3 Locator、Decoration 与 Layer

- `ConnectionLocator`、`EndpointLocator`、`MidpointLocator` 作为布局策略定位
  Connection 子 Figure。
- source/target decoration 使用连接端点切线确定方向。
- `ConnectionLayerFigure` 使用 `DoNotClipChildBounds`，但仍受祖先 viewport clip。

### 4.4 M9 完成门禁

- `m9_connection_contract.rs`：五类 anchor、四类 router、owner move reroute、
  nested viewport 坐标、old/new damage、删除/重挂清理、精确 hit-test。
- `connections-demo`：anchor × router 组合矩阵、节点移动、viewport scroll/zoom、
  decoration 与 label locator。
- 更新 `connection.figure`、`connection.anchor`、`connection.router`、
  `connection.locator` 语义账本。

## 5. 实施顺序与提交边界

1. M8 RangeModel + Viewport contract。
2. M8 ScrollPane/ScrollBar + wheel/autoexpose。
3. M8 demo、截图、手工验收文档与路线图。
4. M9 Anchor + Router 纯计算契约。
5. M9 graph runtime + PolylineConnectionFigure。
6. M9 Locator/Decoration/Layer + demo。
7. 全仓门禁、语义账本和路线图收口。

每一步单独提交；M8 验证未通过时不得开始 M9。

## 6. 失败模式

- 非有限 range/zoom：返回错误，状态不变。
- contents、owner 或 connection BlockId 不存在：返回 typed error，无副作用。
- RangeModel 变化但图事务未提交：typed handle API 不暴露绕过更新的 mutable state。
- anchor owner 删除：Connection 进入 unresolved 状态并擦除旧路径，不保留悬空 ID。
- router 失败或结果不足两个点：保留旧路径并返回错误，不提交部分 geometry。
- 任何 panic：沿用 UpdateManager 恢复协议，恢复 dirty/invalid 队列后继续向外传播。
