# Draw2D API 语义覆盖清单

本文档记录 draw2d API 到 Novadraw 的语义覆盖关系，用作架构 delta 发现、milestone 验收补充和缺口治理的覆盖账本。

它不是 Java API 迁移待办，也不是 milestone 编号源。Milestone 编号仍以 `agent/draw2d-core-milestones.yaml` 为唯一来源。

## 使用原则

- 先记录 draw2d 的语义契约，再记录 Novadraw 的合理变体。
- 不按方法名机械照搬；同一组协作 API 可以合并为一个 API family。
- 每个 family 必须能落到验证方式：contract test、probe、demo 或视觉断言。
- P0 family 优先进入 M1-M10 核心契约；P1 作为能力补齐；P2 和 GEF 层只保留对照，不进入当前 draw2d core 主线。

## Figure Surface 分层

为先验证框架核心机制，当前 Figure 能力分为 active surface 与 deferred surface：

| Surface | 当前类型 | 里程碑归属 | 门禁规则 |
|---|---|---|---|
| Core mechanism | `RootFigure`, `RectangleFigure`, 测试 mock Figure | M1-M5 | 通用 Figure 机制和 API 语义必须完整覆盖 |
| Deferred viewport | `ViewportFigure` | M8 | 保留代码与导出，M8 前不计入 M1-M5 完成门禁 |
| Deferred builtin figures | `EllipseFigure`, `RoundedRectangleFigure`, `PolylineFigure`, `PolygonFigure`, `TriangleFigure` | M10 | 保留代码与导出，M10 前不计入 M1-M5 完成门禁 |

这不是删除能力，也不是降低通用机制要求。规则是：当前 milestone 只对 active surface
判定完成；当 deferred Figure 被纳入对应 milestone 时，必须补齐与 active surface
等价的协议覆盖和测试证据，不能只由 `RectangleFigure` 代表。

## 记录模板

```yaml
- family: figure.geometry.bounds
  priority: P0
  draw2d_apis:
    - IFigure#getBounds
    - IFigure#setBounds
    - IFigure#translate
  semantic_contract:
    - bounds 是 Figure 本地坐标域的外框
    - bounds 改变影响 paint、hit-test、layout、dirty region
  novadraw_variant:
    owner: FigureBlock
    graph_access: BlockId
    difference_reason: Novadraw 使用 ID 引用树，状态不直接塞进 Figure trait
  coverage:
    status: unknown
    milestone: M2
    probes:
      - bounds_change_repaints_old_and_new_region
      - child_absolute_position_changes_after_parent_move
```

## 覆盖矩阵

| 优先级 | API 家族 | draw2d 代表 API | Novadraw 对照方向 | 覆盖检查点 |
|---|---|---|---|---|
| P0 | Figure 树结构 | `IFigure.add/remove/getChildren/getParent/setParent` | `FigureGraph` / `BlockId` 树 | 父子关系、顺序、删除、重挂载、逆序遍历 |
| P0 | Figure 生命周期 | `addNotify/removeNotify` | 节点挂载/卸载 hook | 子树 attach/detach、资源初始化、事件解绑 |
| P0 | Bounds 几何 | `getBounds/setBounds/getLocation/setLocation/getSize/setSize/translate` | `FigureBlock` 几何状态 | bounds 改变后的 repaint、layout invalidation、子坐标影响 |
| P0 | ClientArea / Insets | `getClientArea/getInsets` | border-inset 后的内容区 | paint clip、layout area、hit-test descent 共用同一 inset 逻辑 |
| P0 | Paint 主协议 | `paint(Graphics)` | `Figure::paint` / render traversal | 背景、border、client area、children paint 顺序 |
| P0 | Graphics 绘制上下文 | `Graphics.draw*/fill*/clip*/translate/pushState/popState` | `NdCanvas` / Vello backend | 状态栈、clip、坐标平移、线宽、颜色、文本、路径 |
| P0 | 坐标转换 | `translateToAbsolute/Relative/Parent/FromParent` | graph 坐标域转换 API | parent/local/root 坐标互转、嵌套偏移、clip 下命中 |
| P0 | Hit-test | `containsPoint/findFigureAt/findMouseEventTargetAt` | hit-test traversal | 可见性、启用状态、逆 child 顺序、client area、event target |
| P0 | 可见性 / 启用 | `isVisible/setVisible/isShowing/isEnabled/setEnabled` | Figure 状态位 | hidden 不绘制、不命中；disabled 事件策略 |
| P0 | Border | `Border.getInsets/paint/isOpaque/getPreferredSize` | `Border` trait / border registry | inset 影响 client area；border 绘制顺序；opaque 背景语义 |
| P0 | LayoutManager | `layout/getPreferredSize/getMinimumSize/setConstraint/invalidate` | `LayoutManager` trait | layout area、child constraint、preferred/min size、缓存失效 |
| P0 | Validation | `invalidate/invalidateTree/revalidate/validate` | update/validation pipeline | 自下而上失效、自上而下 validate、重复失效合并 |
| P0 | Damage / Repaint | `repaint/intersects/getUpdateManager` | dirty region / render invalidation | dirty region 合并、局部重绘、bounds 外裁剪 |
| P0 | UpdateManager | `addInvalidFigure/addDirtyRegion/performValidation/performUpdate` | 两阶段更新调度 | Validation -> Damage Repair 顺序、批处理、root update |
| P1 | Figure 属性 | `foreground/background/font/cursor/tooltip/opaque` | style/state storage | 本地属性、继承属性、opaque 背景、tooltip 查询 |
| P1 | 监听器系统 | `FigureListener/AncestorListener/CoordinateListener/LayoutListener/PropertyChangeListener` | event/listener hooks | bounds 变化、ancestor 变化、layout 生命周期、属性变更 |
| P1 | 输入事件 | `MouseListener/MouseMotionListener/MouseWheelListener/KeyListener/FocusListener` | engine event dispatch | target/source 点转换、capture、hover、drag、wheel、key |
| P1 | EventDispatcher | `SWTEventDispatcher/EventDispatcher` | 平台输入适配 + 引擎分发 | apps 只适配平台事件，引擎负责命中和派发 |
| P1 | Focus | `requestFocus/hasFocus/isFocusTraversable` | focus manager | focus owner、tab traversal、focus gained/lost |
| P1 | ClippingStrategy | `getClippingStrategy/setClippingStrategy` | child clip policy | 子节点是否被 parent/client area 裁剪，可替换策略 |
| P1 | 基础 Figure 类型 | `Figure/Label/ImageFigure/RectangleFigure/Ellipse/FigureCanvas` | builtin figures | 基础图元、文本、图片、容器 figure |
| P1 | 几何 Primitive | `Rectangle/Point/Dimension/Insets/PointList/Precision*` | `novadraw-math` / `novadraw-geometry` | 整数/浮点策略、包含/相交/扩张/平移 |
| P1 | 具体布局 | `XYLayout/StackLayout/BorderLayout/GridLayout/FlowLayout/ToolbarLayout` | layout implementations | 绝对布局、栈布局、边界布局、网格、流式、工具栏 |
| P1 | Freeform / Layer | `Layer/FreeformLayer/FreeformLayout/FreeformViewport` | 大画布/自由坐标层 | 负坐标、内容范围、root/layer 分层 |
| P1 | Viewport / Scroll | `Viewport/ScrollPane/ScrollBar/RangeModel` | 视口组件，当前可延后 | viewport clip、scroll offset、content extent |
| P1 | Connection | `Connection/PolylineConnection` | edge figure / connection figure | source/target、point list、重路由、连接子 figure |
| P1 | Anchor | `ConnectionAnchor/ChopboxAnchor/EllipseAnchor/XYAnchor` | anchor trait | owner bounds 变化触发连接重算、参考点计算 |
| P1 | Router | `ConnectionRouter/Manhattan/Bendpoint/ShortestPath` | router trait | route 输入输出、constraint、invalidate/remove |
| P1 | Locator | `Locator/ConnectionLocator/EndpointLocator/MidpointLocator` | decoration/label placement | 在线段、端点、相对 bounds 上定位 child |
| P2 | 文本 Flow | `FlowFigure/TextFlow/ParagraphTextLayout` | 富文本/段落布局 | inline/block flow、换行、文本测量 |
| P2 | Widget 辅助 | `Button/Clickable/Toggle/Slider` 等 widgets | 可选控件库 | 交互控件，不应污染核心 draw2d 协议 |
| P2 | 图布局 | `DirectedGraphLayout/CompoundDirectedGraphLayout` | 后续自动布局能力 | DAG/复合图布局，可作为独立算法模块 |
| P2 | 打印 / 缩放 Graphics | `PrinterGraphics/ScaledGraphics` | backend adapter | 缩放代理、打印目标、非主线渲染后端 |
| GEF 层 | Viewer 映射 | `EditPartViewer.findObjectAt` | 编辑器层，不进 draw2d core | Figure 到 app object / EditPart 映射 |
| GEF 层 | Request / Tool | `Request/SelectionRequest/LocationRequest` | 编辑器交互层 | selection、drag、create、reconnect 请求 |
| GEF 层 | EditPolicy / Command | `EditPolicy/getCommand` | 后续 GEF-like 层 | 命令生成、交互策略，不属于 M1-M10 核心 |

## API Family ID 约定

`agent/draw2d-core-milestones.yaml` 使用下列 ID 引用本文档中的 API family。后续新增 delta 时，`api_semantics` 应优先引用这些 ID。

| Family ID | 对应 API 家族 | 语义边界 |
|---|---|---|
| `geometry.primitives` | 几何 Primitive | Point、Dimension、Rectangle、Insets、PointList、Transform 等平台无关几何 |
| `graphics.context` | Graphics 绘制上下文 | draw/fill、clip、translate、scale、state stack、颜色、线型、文本、图片 |
| `figure.tree` | Figure 树结构 | parent/children、child order、reparent、remove、树拓扑不变量 |
| `figure.lifecycle` | Figure 生命周期 | attach/detach、addNotify/removeNotify、资源与监听解绑边界 |
| `figure.geometry.bounds` | Bounds 几何 | bounds、location、size、translate 及其对 paint/layout/hit-test 的影响 |
| `figure.box.client_area` | ClientArea / Insets | border inset 后的内容区，paint/layout/hit-test 共享盒模型 |
| `figure.visibility.enabled` | 可见性 / 启用 | visible/showing/enabled 对 paint、hit-test、event target 的影响 |
| `figure.properties` | Figure 属性 | foreground/background/font/cursor/tooltip/opaque 等本地或继承属性 |
| `paint.protocol` | Paint 主协议 | paintFigure、paintClientArea/children、paintBorder 的顺序和坐标域 |
| `border.protocol` | Border | insets、preferred size、opaque、paint 及其对 client area 的影响 |
| `clipping.strategy` | ClippingStrategy | parent/client area/child bounds 裁剪策略 |
| `coordinate.conversion` | 坐标转换 | parent/local/root/absolute/relative 坐标域转换 |
| `hit_test.search` | Hit-test | containsPoint、findFigureAt、findMouseEventTargetAt、TreeSearch |
| `event.point_reduction` | 事件点降域 | 平台或 root 坐标事件点转换为 target Figure 本地坐标 |
| `event.dispatcher` | EventDispatcher | 平台输入适配之后的引擎层 target 查找、capture、focus、hover 状态机 |
| `event.input_listeners` | 输入事件监听 | mouse、motion、wheel、key、focus listener 与 Figure callback |
| `event.focus` | Focus | focus owner、focus traversal、focus gained/lost |
| `layout.manager` | LayoutManager | layout、constraint、preferred/min/max size、invalidate |
| `validation.protocol` | Validation | invalidate、invalidateTree、revalidate、validate、validation root |
| `update_manager.two_phase` | UpdateManager | Validation -> Damage Repair 两阶段更新事务 |
| `damage.repaint` | Damage / Repaint | repaint、dirty region、intersects、damage parent-chain 映射 |
| `notification.figure` | Figure 通知 | Figure moved / bounds changed 等对象状态通知 |
| `notification.coordinate` | Coordinate 通知 | coordinate root 或坐标系统变化通知 |
| `notification.property` | Property 通知 | property change 语义 |
| `notification.ancestor` | Ancestor 通知 | parent-chain add/remove/move 通知 |
| `notification.layout_update` | Layout / Update 通知 | layout lifecycle、validating、painting phase 通知 |
| `viewport.scroll_zoom` | Viewport / Scroll / Zoom | viewport、scroll pane、range model、zoom transform、content clip |
| `layer.freeform` | Freeform / Layer | Layer、FreeformLayer、自由坐标内容范围 |
| `connection.figure` | Connection | Connection、PolylineConnection、point list、connection layer |
| `connection.anchor` | Anchor | source/target anchor、owner bounds 变化、reference point |
| `connection.router` | Router | routing constraint、route/invalidate/remove、Manhattan/Bendpoint 等 router |
| `connection.locator` | Locator | connection labels、decorations、endpoint/midpoint placement |
| `builtin.figures` | 基础 Figure 类型 | rectangle、ellipse、rounded rectangle、polygon、polyline、label、image |
| `text.flow` | 文本 Flow | FlowFigure、TextFlow、ParagraphTextLayout、基础文本测量与换行 |
| `widgets.basic` | Widget 辅助 | button-like、toggle-like、clickable 等基础交互 figure |

## Milestone 到 API 语义映射

Milestone 编号仍以 `agent/draw2d-core-milestones.yaml` 为唯一来源。本文只提供人工可读映射，机器可读字段在 YAML 的 `api_semantics.primary` 和 `api_semantics.secondary`。

| Milestone | 主 API 语义 | 次级关联 | 推进时必须检查 |
|---|---|---|---|
| M1 几何与 Graphics 基础 | `geometry.primitives`, `graphics.context` | `paint.protocol`, `clipping.strategy` | 几何平台无关性、Graphics 状态栈、clip/transform 组合 |
| M2 Figure 树与盒模型 | `figure.tree`, `figure.lifecycle`, `figure.geometry.bounds`, `figure.box.client_area`, `figure.visibility.enabled` | `hit_test.search`, `coordinate.conversion`, `notification.ancestor` | active surface 的 parent/child 不变量、z-order、bounds/clientArea/visible/enabled 一致性 |
| M3 绘制遍历与裁剪闭环 | `paint.protocol`, `graphics.context`, `clipping.strategy`, `border.protocol`, `hit_test.search` | `figure.box.client_area`, `coordinate.conversion`, `damage.repaint` | active surface 的 paint 顺序、clientArea clip、border 顺序、paint 与 hit-test 可见性一致 |
| M4 坐标域与变换闭环 | `coordinate.conversion`, `figure.box.client_area`, `event.point_reduction` | `hit_test.search`, `damage.repaint`, `viewport.scroll_zoom` | local/parent/absolute/relative 往返、事件点降域、dirty rect parent-chain 映射 |
| M5 Layout + Validation + UpdateManager | `layout.manager`, `validation.protocol`, `update_manager.two_phase`, `damage.repaint` | `figure.geometry.bounds`, `figure.box.client_area`, `notification.layout_update` | layout constraint、preferred/min/max size、Validation 先于 Damage Repair、dirty region 合并 |
| M6 事件分发与交互状态机 | `event.dispatcher`, `event.input_listeners`, `event.focus`, `hit_test.search`, `event.point_reduction` | `coordinate.conversion`, `figure.visibility.enabled` | target 查找、capture、hover/enter/exit、focus、target-domain event |
| M7 通知语义分层 | `notification.figure`, `notification.coordinate`, `notification.property`, `notification.ancestor`, `notification.layout_update` | `figure.lifecycle`, `validation.protocol`, `update_manager.two_phase` | Figure/Coordinate/Property/Ancestor/Input/Update 通知不混层 |
| M8 Viewport / Scroll / Zoom | `viewport.scroll_zoom`, `clipping.strategy`, `coordinate.conversion`, `hit_test.search` | `damage.repaint`, `update_manager.two_phase`, `layer.freeform` | viewport 作为 Figure 树语义参与 paint、hit-test、坐标转换和 damage repair |
| M9 Connection / Anchor / Router | `connection.figure`, `connection.anchor`, `connection.router`, `connection.locator` | `coordinate.conversion`, `damage.repaint`, `notification.ancestor`, `hit_test.search` | anchor 端点、router point list、node movement reroute、connection damage/hit-test |
| M10 常用 Figure 与文本/控件 | `builtin.figures`, `border.protocol`, `text.flow`, `widgets.basic` | `layout.manager`, `event.input_listeners`, `figure.properties` | deferred builtin Figure 升级为完整 reusable surface；具体 Figure 只能消费核心协议，不引入特例 |

## 方法级 API 跟踪矩阵

本节记录从 draw2d 源码抽取的方法级 API 语义，用于后续把 family 级覆盖拆成可执行的
architecture delta、contract test 和产品入口检查。它不是要求逐方法照搬 Java API；Novadraw
可以使用 trait、`BlockId`、图 API、上下文对象或命令对象表达等价语义。

状态含义：

- `verified`：已有 public API、实现路径和测试或 milestone 完成证据。
- `partial`：已有骨架或部分实现，但语义闭环、测试、导出或产品入口不完整。
- `missing`：没有可定位的 Novadraw public API。
- `deferred`：已有代码或设计方向，但按 Figure Surface 分层暂不计入当前 milestone 门禁。

### M1 Graphics / Geometry

| Family ID | Draw2D 方法级 API | Novadraw 等价 API / 方向 | 状态 | 后续跟踪 |
|---|---|---|---|---|
| `graphics.context` | `Graphics.pushState/popState/restoreState` | `NdCanvas` 状态栈与命令快照 | verified | 保持 M1 state stack probe |
| `graphics.context` | `Graphics.clipRect/clipPath/setClip/getClip` | `NdCanvas` clip command + backend clip 执行 | verified | M3/M8 继续检查 parent/client/viewport clip 组合 |
| `graphics.context` | `Graphics.translate/scale/rotate/shear`, `getAbsoluteScale` | `NdCanvas` transform command；`Viewport`/坐标 API 消费 transform | partial | `rotate/shear/absolute scale` 先作为 P1/P2 状态，不阻塞 M1 |
| `graphics.context` | `drawLine/drawRectangle/drawRoundRectangle/drawOval/drawPolygon/drawPolyline/drawPath` | shape figure 通过 `NdCanvas` 绘制路径/基础图形 | partial | M10 deferred Figure 升级时补 shape contract tests |
| `graphics.context` | `fillRectangle/fillRoundRectangle/fillOval/fillPolygon/fillPath/fillGradient` | fill command / Vello backend | partial | `fillGradient` 可延后；基础 fill 归入 M10 shape tests |
| `graphics.context` | `drawString/drawText/drawTextLayout/fillText/getFont/getFontMetrics/setFont` | 文本命令入口；未来 `LabelFigure` / `TextFlowFigure` 消费 | partial | M10 前补 preferred size + text measure contract |
| `graphics.context` | `drawImage(...)` | 图片绘制命令入口；未来 `ImageFigure` 消费 | partial | M10 前补 image resource id、preferred size、paint contract |
| `graphics.context` | `setAlpha/setAntialias/setLineDash/setLineCap/setLineJoin/setLineMiterLimit/setXORMode` | 渲染状态扩展 | deferred | 作为高级 stroke/style，不进入 M1 完成门禁 |
| `geometry.primitives` | `Point/Dimension/Rectangle/Insets/PointList/Precision*` | `novadraw-geometry` / `novadraw-math` 几何类型 | verified | `PointList` 需在 M9/M10 connection/point-list shape 中复查 |

Draw2D 证据入口：`Graphics.java`、`SWTGraphics.java`、`ScaledGraphics.java`、`PrinterGraphics.java`。

### M2 Figure Tree / Box Model

| Family ID | Draw2D 方法级 API | Novadraw 等价 API / 方向 | 状态 | 后续跟踪 |
|---|---|---|---|---|
| `figure.tree` | `IFigure.add(IFigure)`, `add(IFigure,int)`, `add(IFigure,Object,int)` | `FigureGraph::set_contents`, `add_child_to`, `add_child_with_bounds`, `try_add_child_to` | verified | 保持 child order / reparent / no-cycle probes |
| `figure.tree` | `remove(IFigure)`, `removeAll()`, `getChildren()`, `getParent()`, `setParent(IFigure)` | `FigureGraph` 拓扑 API + `BlockId` parent/children 状态 | verified | 未来 public remove API 若暴露需补 removeAll 等价语义 |
| `figure.lifecycle` | `addNotify()`, `removeNotify()` | `Figure::on_attached`, `Figure::on_detached` | verified | 监听解绑与资源释放在 M7 再复查 |
| `figure.geometry.bounds` | `getBounds/setBounds/getLocation/setLocation/getSize/setSize/translate` | `FigureGraph::figure_bounds`, `set_bounds`, `prim_translate`, `Bounded::set_bounds` | verified | M4/M5 复查 dirty rect 与 validation 联动 |
| `figure.box.client_area` | `getClientArea()`, `getClientArea(Rectangle)`, `getInsets()` | border inset 后的 client area 计算 | verified | M5 layout area、M8 viewport client area 继续复查 |
| `figure.visibility.enabled` | `isVisible/setVisible/isShowing/isEnabled/setEnabled` | `FigureGraph::set_visible`, `set_enabled`, `is_effectively_visible`, `is_effectively_enabled` | verified | M6 复查 disabled 对 event target 的策略 |
| `hit_test.search` | `containsPoint`, `intersects`, `findFigureAt`, `findFigureAtExcluding`, `findMouseEventTargetAt` | `Bounded::contains_point`, `FigureGraph::hit_test`, `hit_test_simple`, `find_mouse_event_target_at` | verified | `findFigureAtExcluding`/TreeSearch 策略作为 P1 扩展 |
| `figure.properties` | `get/setForegroundColor`, `get/setBackgroundColor`, `get/setFont`, `get/setCursor`, `get/setToolTip`, `isOpaque/setOpaque` | 建议新增 `FigureProperties` / `FigureStyle`，由 `FigureBlock` 或 style store 持有 | missing | M10 前定义本地属性、继承属性和 opaque 背景语义 |

Draw2D 证据入口：`IFigure.java`、`Figure.java`。

### M3 Paint / Clip / Border

| Family ID | Draw2D 方法级 API | Novadraw 等价 API / 方向 | 状态 | 后续跟踪 |
|---|---|---|---|---|
| `paint.protocol` | `IFigure.paint(Graphics)`；`Figure.paintFigure/paintClientArea/paintBorder` 扩展点 | `Figure::paint_figure`, `paint_children`, `paint_border` + recursive traversal | verified | 保护 `render_recursive.rs` 主流程，只在协议不符时调整 |
| `clipping.strategy` | `getClippingStrategy/setClippingStrategy` | `ChildClippingStrategy` | verified | M8 viewport、M10 deferred figures 纳入时复查覆盖 |
| `border.protocol` | `Border.getInsets`, `getPreferredSize`, `isOpaque`, `paint` | `Border` trait；`LineBorder`, `MarginBorder`, `RectangleBorder` | partial | M10 补更多 border 实现和 opaque/preferred size tests |
| `damage.repaint` | `erase()`, `repaint()`, `repaint(Rectangle)`, `repaint(x,y,w,h)` | `FigureGraph::repaint`, `repaint_all`; runtime dirty region | partial | M5 必须闭合 old/new bounds damage、dirty merge 和 parent-chain 坐标 |

Draw2D 证据入口：`Figure.java`、`Border.java`、`AbstractBorder.java`、`LabeledBorder.java`。

### M4 Coordinates / Event Point Reduction

| Family ID | Draw2D 方法级 API | Novadraw 等价 API / 方向 | 状态 | 后续跟踪 |
|---|---|---|---|---|
| `coordinate.conversion` | `translateToAbsolute(Translatable)`, `translateToRelative(Translatable)` | `FigureGraph::translate_to_absolute_mut`, `translate_to_relative` | partial | M4 deep nested roundtrip 和 coordinate root movement probe |
| `coordinate.conversion` | `translateToParent(Translatable)`, `translateFromParent(Translatable)` | `FigureGraph::translate_to_parent`, `translate_from_parent` | partial | parent/clientArea/viewport offset 需同源 |
| `coordinate.conversion` | `isCoordinateSystem()`, `isMirrored()` | `FigureGraph::is_coordinate_system`; mirroring 暂无 | partial | `isMirrored` 可延后，先保证 coordinate root 语义 |
| `event.point_reduction` | MouseEvent target point 转为 target local 域 | `MouseEvent::with_target_point`, `BasicEventDispatcher` target dispatch | partial | M4/M6 共同验证 target-domain event point |

Draw2D 证据入口：`IFigure.java`、`Figure.java`、`Viewport.java`。

### M5 Layout / Validation / UpdateManager

| Family ID | Draw2D 方法级 API | Novadraw 等价 API / 方向 | 状态 | 后续跟踪 |
|---|---|---|---|---|
| `layout.manager` | `LayoutManager.getConstraint/setConstraint/remove` | `FigureGraph::set_constraint/get_constraint/remove_constraint`; `LayoutManager` trait | partial | 约束生命周期和 child remove hook 未完全闭合 |
| `layout.manager` | `getPreferredSize(IFigure,wHint,hHint)`, `getMinimumSize(...)` | `LayoutManager::get_preferred_size`, `get_minimum_size`; `FigureBlock` preferred/min/max accessors | partial | size hint、border/client area 参与尺寸计算需补 probe |
| `layout.manager` | `LayoutManager.invalidate(IFigure)`, `layout(IFigure)` | `FigureGraph::validate`, `apply_layout`, `revalidate` | partial | layout cache invalidation 与 validation 阶段边界需明确 |
| `validation.protocol` | `IFigure.invalidate`, `invalidateTree`, `revalidate`, `validate`, `setValid` | `FigureGraph::invalidate`, `mark_invalid`, `revalidate`, `validate`, `perform_validation_cycle` | partial | M5 必须定义 validation root 和重复失效合并 |
| `update_manager.two_phase` | `addInvalidFigure`, `performValidation`, `performUpdate`, `runWithUpdate` | `SceneUpdateManager::add_invalid_figure`, `FigureGraph::perform_validation_cycle`, `perform_update` | partial | 明确 Validation -> Damage Repair 事务顺序 |
| `damage.repaint` | `UpdateManager.addDirtyRegion`, `performUpdate(Rectangle exposed)` | `SceneUpdateManager::add_dirty_region`, `compute_damage` | partial | 暴露区局部更新可作为 M5/P1 扩展 |

Draw2D 证据入口：`LayoutManager.java`、`UpdateManager.java`、`DeferredUpdateManager.java`。

### M6 Event Dispatcher / Focus / Capture

| Family ID | Draw2D 方法级 API | Novadraw 等价 API / 方向 | 状态 | 后续跟踪 |
|---|---|---|---|---|
| `event.dispatcher` | `dispatchMousePressed/Released/Moved/Dragged/Entered/Exited/Hover/DoubleClicked` | `Event::Mouse`, `MouseEventKind`, `BasicEventDispatcher`, `Figure::on_mouse_*` | partial | 缺 double click、drag、hover 细分语义 |
| `event.dispatcher` | `setRoot`, `setControl` | app 平台适配 + `SceneHost`/runtime context | partial | apps 只做平台输入适配，root dispatch 留在引擎层 |
| `event.focus` | `requestFocus`, `requestRemoveFocus`, `getFocusOwner`, `hasFocus`, `isFocusTraversable` | `FigureGraph::focus_owner`, `set_focus_owner`; 建议新增 focus traversal API | partial | M6 补 focus gained/lost event 与 traversal |
| `event.dispatcher` | `setCapture`, `releaseCapture`, `isCaptured` | `FigureGraph::captured`, `set_captured` | partial | M6 补 capture 生命周期和 release 条件 |
| `event.input_listeners` | `MouseWheelListener`, `KeyListener`, `FocusListener` | 建议新增 `WheelEvent`, `KeyEvent`, `FocusEvent` 与 Figure callback/listener | missing | M6 明确 listener 与 callback 的 Rust 表达 |
| `event.dispatcher` | `updateCursor`, `getAccessibilityDispatcher` | cursor/accessibility 扩展 | deferred | cursor 可随 Figure properties；accessibility 不进当前核心门禁 |

Draw2D 证据入口：`EventDispatcher.java`、`SWTEventDispatcher.java`、`MouseEvent.java`、listener 接口。

### M7 Notification / Listener Semantics

| Family ID | Draw2D 方法级 API | Novadraw 等价 API / 方向 | 状态 | 后续跟踪 |
|---|---|---|---|---|
| `notification.figure` | `add/removeFigureListener`; figure moved / bounds changed | `FigureEvent`, `UpdateListener::on_figure_event` | partial | 需要区分 move、resize、bounds changed |
| `notification.ancestor` | `add/removeAncestorListener` | 建议新增 ancestor notification channel | missing | M7 处理 add/remove/reparent/move ancestor 链 |
| `notification.coordinate` | `add/removeCoordinateListener` | 建议新增 coordinate notification channel | missing | coordinate root 或 parent conversion 变化时触发 |
| `notification.property` | `add/removePropertyChangeListener`, 按 property name 监听 | 建议新增 typed property event 或 property key | missing | 与 `figure.properties` 一起设计 |
| `notification.layout_update` | `add/removeLayoutListener`, `UpdateListener`, validating/painting | `UpdateListener`, `NotificationEffect` | partial | M7 拆清 layout lifecycle 与 update lifecycle |

Draw2D 证据入口：`IFigure.java`、`Figure.java`、`UpdateManager.java`、listener 接口。

### M8 Viewport / Scroll / Zoom / Freeform

| Family ID | Draw2D 方法级 API | Novadraw 等价 API / 方向 | 状态 | 后续跟踪 |
|---|---|---|---|---|
| `viewport.scroll_zoom` | `Viewport.getContents/setContents` | `ViewportFigure` + FigureGraph child tree；建议明确 contents API | partial | M8 将 viewport 纳入 Figure tree 语义门禁 |
| `viewport.scroll_zoom` | `get/setHorizontalRangeModel`, `get/setVerticalRangeModel` | 建议新增 `RangeModel` | missing | RangeModel 是 scroll 状态 SSOT，不应只靠 origin 字段 |
| `viewport.scroll_zoom` | `getViewLocation`, `setViewLocation`, `setHorizontalLocation`, `setVerticalLocation` | `Viewport::set_origin`, `pan`, `viewport_to_content`, `content_to_viewport` | partial | 与 coordinate conversion/damage repair 联动 |
| `viewport.scroll_zoom` | `get/setContentsTracksWidth/Height` | 建议新增 track width/height 策略 | missing | M8 layout/validation 中处理内容尺寸 |
| `viewport.scroll_zoom` | `ScrollPane.getViewport/setViewport`, `setContents`, `scrollTo`, scrollbar visibility | 建议新增 `ScrollPaneFigure` 或延后为 widget 组合 | missing | 先做 Viewport 核心，再做 ScrollPane 组合 |
| `viewport.scroll_zoom` | `ScrollBar.get/setRangeModel`, `get/setValue`, `stepUp/stepDown`, `setPageIncrement/setStepIncrement` | 建议作为 M8/M10 交互控件补充 | missing | 不应阻塞 Viewport 坐标与 clip 闭环 |
| `viewport.scroll_zoom` | `ScalableFigure.getScale/setScale` | `Viewport::set_zoom`, `zoom_at`, `zoom_to_fit`; 可新增 `ScalableFigure` trait | partial | zoom transform 与 Graphics scale/坐标转换统一 |
| `layer.freeform` | `Layer.containsPoint/findFigureAt`, `FreeformLayer.getFreeformExtent/setFreeformBounds` | 建议新增 `LayerFigure` / `FreeformLayerFigure` | missing | 作为大画布/负坐标能力，M8 后半段或独立 delta |

Draw2D 证据入口：`Viewport.java`、`ScrollPane.java`、`RangeModel.java`、`ScrollBar.java`、`ScalableFigure.java`、`Layer.java`、`FreeformLayer.java`。

### M9 Connection / Anchor / Router / Locator

| Family ID | Draw2D 方法级 API | Novadraw 等价 API / 方向 | 状态 | 后续跟踪 |
|---|---|---|---|---|
| `connection.figure` | `Connection.get/setSourceAnchor`, `get/setTargetAnchor` | 建议新增 `ConnectionFigure::source_anchor/target_anchor` | missing | M9 首个 contract delta |
| `connection.figure` | `get/setConnectionRouter`, `get/setRoutingConstraint` | 建议新增 `ConnectionRouter` trait + `RoutingConstraint` | missing | Router 不应塞进具体 connection 实现 |
| `connection.figure` | `getPoints/setPoints` | `PointList` + connection point cache | missing | point list 同时驱动 paint、hit-test、damage |
| `connection.anchor` | `ConnectionAnchor.getLocation`, `getOwner`, `getReferencePoint`, `add/removeAnchorListener` | 建议新增 `ConnectionAnchor` trait + anchor moved notification | missing | owner bounds 变化触发 reroute/repaint |
| `connection.router` | `ConnectionRouter.route`, `invalidate`, `remove`, `get/setConstraint` | 建议新增 route/invalidate/remove 协议 | missing | Manhattan/Bendpoint/ShortestPath/Fan router 可分批实现 |
| `connection.locator` | `Locator.relocate`, `ConnectionLocator`, `EndpointLocator`, `MidpointLocator` | 建议新增 `ConnectionLocator` trait | missing | 用于 label/decorations child placement |

建议首批 Rust 契约草案：

```rust
pub trait ConnectionAnchor {
    fn owner(&self) -> Option<BlockId>;
    fn location(&self, graph: &FigureGraph, reference: Point) -> Point;
    fn reference_point(&self, graph: &FigureGraph) -> Point;
}

pub trait ConnectionRouter {
    fn route(&self, graph: &FigureGraph, connection: BlockId) -> PointList;
    fn invalidate(&mut self, connection: BlockId);
    fn set_constraint(&mut self, connection: BlockId, constraint: RoutingConstraint);
    fn remove(&mut self, connection: BlockId);
}

pub trait ConnectionFigure: Figure {
    fn source_anchor(&self) -> Option<&dyn ConnectionAnchor>;
    fn target_anchor(&self) -> Option<&dyn ConnectionAnchor>;
    fn connection_router(&self) -> Option<&dyn ConnectionRouter>;
    fn points(&self) -> &PointList;
}
```

Draw2D 证据入口：`Connection.java`、`PolylineConnection.java`、`ConnectionAnchor.java`、`ConnectionRouter.java`、`Locator.java`、`AbstractRouter.java`。

### M10 Reusable Figures / Text / Widgets

| Family ID | Draw2D 方法级 API | Novadraw 等价 API / 方向 | 状态 | 后续跟踪 |
|---|---|---|---|---|
| `builtin.figures` | `Shape.setFill/setOutline/setLineWidth/setLineWidthFloat` | `Shape` trait + concrete figures | partial | M10 将 deferred figures 升级为 reusable surface |
| `builtin.figures` | `RectangleFigure`, `Ellipse.containsPoint/fillShape/outlineShape` | `RectangleFigure`, `EllipseFigure` | deferred | 椭圆精确 hit-test、line width inset 需补 |
| `builtin.figures` | `AbstractPointListShape.addPoint/insertPoint/removePoint/setPoint/setPoints/getStart/getEnd` | `PolylineFigure` / `PolygonFigure` point-list API | partial | point-list bounds、property change、damage invalidation 需补 |
| `builtin.figures` | `Polyline.containsPoint`, `Polygon.containsPoint`, `drawPolyline/drawPolygon/fillPolygon` | `PolylineFigure`, `PolygonFigure` | deferred | tolerance hit-test、closed/open path 语义需补 |
| `builtin.figures` | `Label` text/icon constructors, text/icon alignment, iconTextGap, `getPreferredSize`, truncate, paint text/image | 建议新增 `LabelFigure` | missing | M10 必须补 preferred size 与 layout 交互 |
| `builtin.figures` | `ImageFigure.getImage/setImage/getPreferredSize/setAlignment/paintFigure` | 建议新增 `ImageFigure` + image resource id | missing | image change 后 revalidate/repaint |
| `text.flow` | `TextFlow.getText/setText`, fragment paint, truncate, leading word width | 建议新增 `TextFlowFigure` 或 `TextFlow` layout object | missing | 先做 basic wrapping/measure，不做富文本编辑器 |
| `widgets.basic` | `Clickable.doClick`, action/change listener, model, selected, rollover, pressed/focus paint | 建议新增 `ClickableFigure` / button model | missing | 依赖 M6 event 与 M7 notification |
| `widgets.basic` | `Button` text/image constructors and default button style | 建议新增 `ButtonFigure` 作为 `ClickableFigure + LabelFigure` 组合 | missing | 不引入完整 widget toolkit |

建议首批 Rust 契约草案：

```rust
pub struct LabelFigure {
    text: String,
    icon: Option<ImageId>,
    text_alignment: Alignment,
    icon_text_gap: f64,
}

pub struct ImageFigure {
    image: ImageId,
    alignment: Alignment,
}

pub trait ClickableFigure: Figure {
    fn do_click(&mut self, ctx: &mut dyn NovadrawContext);
    fn is_selected(&self) -> bool;
    fn set_selected(&mut self, selected: bool);
}
```

Draw2D 证据入口：`Shape.java`、`RectangleFigure.java`、`Ellipse.java`、`Polyline.java`、`Polygon.java`、`AbstractPointListShape.java`、`Label.java`、`ImageFigure.java`、`Clickable.java`、`Button.java`、`text/TextFlow.java`、`text/FlowFigure.java`。

## Milestone 推进检查规则

每次推进 M1-M10 的 architecture 或 parity delta 时，必须执行以下检查：

1. 在 delta 中列出受影响的 `api_semantics` family ID。
2. 对照本文件确认 draw2d 语义契约是否仍完整。
3. 如果契约不完整，必须新增 probe、测试、demo 断言或 backlog debt，不能只更新 milestone 状态。
4. milestone 进入 `behavior_verified` 前，必须检查其 `api_semantics.primary` 已经有可重复验证证据。
5. milestone 进入 `complete` 前，必须确认主 API 语义、次级关联、文档和债务记录已对齐。
6. M1-M5 的完成判定只覆盖 active core Figure surface；M8/M10 才能把 Viewport 或 deferred builtin Figure 计入自身完成度。

## P0 核心契约分组

### Figure 树结构

代表 API：

- `IFigure.add(...)`
- `IFigure.remove(IFigure)`
- `IFigure.getChildren()`
- `IFigure.getParent()`
- `IFigure.setParent(IFigure)`

语义契约：

- Figure 是有序树节点，child 顺序同时影响绘制顺序和命中优先级。
- 子节点重挂载必须维护 parent 反向关系。
- 删除节点必须切断 parent/child 关系，并触发必要的 layout、paint、事件生命周期更新。

Novadraw 对照：

- 使用 `FigureGraph` 承载拓扑。
- 使用 `BlockId` 引用节点，避免 Java 引用树。
- Figure 属性、运行时状态、拓扑保持分离：`Figure` / `FigureBlock` / `FigureGraph`。

建议 probes：

- `child_order_controls_paint_and_hit_test_priority`
- `remove_child_detaches_parent_relation`
- `reparent_child_updates_old_and_new_parent`

### Bounds 几何

代表 API：

- `IFigure.getBounds()`
- `IFigure.setBounds(Rectangle)`
- `IFigure.getLocation()`
- `IFigure.setLocation(Point)`
- `IFigure.getSize()`
- `IFigure.setSize(Dimension)`
- `IFigure.translate(int, int)`

语义契约：

- bounds 是 Figure 在 parent 坐标域中的外框。
- bounds 改变影响 paint、hit-test、layout、dirty region 和子节点绝对位置。
- 位置变化和尺寸变化都可能触发布局失效，但影响范围不同。

Novadraw 对照：

- 几何状态归属 `FigureBlock`。
- 对外通过图 API 或 command 修改，避免直接可变引用穿透。

建议 probes：

- `bounds_change_invalidates_old_and_new_damage`
- `parent_move_changes_child_absolute_position`
- `resize_preserves_local_child_coordinates`

### ClientArea / Insets / Border

代表 API：

- `IFigure.getClientArea()`
- `IFigure.getInsets()`
- `IFigure.getBorder()`
- `IFigure.setBorder(Border)`
- `Border.getInsets(IFigure)`
- `Border.paint(IFigure, Graphics, Insets)`
- `Border.isOpaque()`

语义契约：

- border 不是纯装饰；它参与 Figure 盒模型。
- client area = bounds 去掉 border insets 后的内容区域。
- paint、layout 和 hit-test descent 必须共享同一套 inset/client area 逻辑。

Novadraw 对照：

- Border 应作为独立 trait 或 figure 装饰协议。
- client area 计算应集中在引擎层，避免 paint 和 hit-test 各自实现。

建议 probes：

- `border_inset_reduces_client_area`
- `paint_client_area_clip_matches_hit_test_descent`
- `opaque_border_affects_background_paint_order`

### Paint / Graphics

代表 API：

- `IFigure.paint(Graphics)`
- `Graphics.pushState()`
- `Graphics.popState()`
- `Graphics.translate(...)`
- `Graphics.clipRect(...)`
- `Graphics.draw*`
- `Graphics.fill*`

语义契约：

- Figure paint 是递归绘制协议，不只是单个节点 draw call。
- Graphics 维护绘制状态栈，包含 clip、transform、颜色、线宽、字体等状态。
- child paint 进入子坐标域，退出时恢复父级 Graphics 状态。

Novadraw 对照：

- `NdCanvas` 承载 draw2d Graphics 语义。
- Vello 后端只做渲染实现，不承载 Figure 树语义。
- 当前主线保护 `render_recursive.rs`，问题应优先定位 Figure 协议、坐标转换、canvas 命令或后端。

建议 probes：

- `graphics_state_restored_after_child_paint`
- `nested_translate_affects_child_commands`
- `clip_limits_child_paint_commands`

### Layout / Validation / Update

代表 API：

- `IFigure.setLayoutManager(LayoutManager)`
- `IFigure.setConstraint(IFigure, Object)`
- `IFigure.invalidate()`
- `IFigure.invalidateTree()`
- `IFigure.revalidate()`
- `IFigure.validate()`
- `UpdateManager.addInvalidFigure(IFigure)`
- `UpdateManager.performValidation()`
- `UpdateManager.performUpdate()`

语义契约：

- draw2d 的更新模型是两阶段：Validation -> Damage Repair。
- layout manager 负责在 validation 阶段设置 child bounds。
- repaint 只描述脏区，validate 负责结构和布局正确性，两者不能混为一次即时绘制。

Novadraw 对照：

- 引擎层需要显式承载 update pipeline。
- apps 不能直接手写通用 layout/validation 规则。

建议 probes：

- `invalidate_parent_schedules_validation_once`
- `validate_runs_layout_before_paint`
- `repaint_dirty_region_is_merged_before_render`

### 坐标转换

代表 API：

- `IFigure.translateToAbsolute(Translatable)`
- `IFigure.translateToRelative(Translatable)`
- `IFigure.translateToParent(Translatable)`
- `IFigure.translateFromParent(Translatable)`
- `IFigure.isCoordinateSystem()`
- `IFigure.isMirrored()`

语义契约：

- Figure 层的坐标转换是树结构语义，不是渲染后端细节。
- MouseEvent 进入 target Figure 时，事件点应从 root / absolute 域转换到 target local 域。
- parent bounds、coordinate root、mirroring 或 scale 都会改变转换链路。

Novadraw 对照：

- 坐标转换 API 应在 `novadraw-scene` 等引擎 crate。
- app 只负责平台输入点进入引擎前的最外层适配。

建议 probes：

- `nested_child_absolute_to_local_roundtrip`
- `mouse_event_point_is_relative_to_target`
- `clip_and_hit_test_use_same_coordinate_domain`

### Hit-test / Event Target

代表 API：

- `IFigure.containsPoint(...)`
- `IFigure.findFigureAt(...)`
- `IFigure.findFigureAtExcluding(...)`
- `IFigure.findMouseEventTargetAt(...)`
- `TreeSearch.accept/prune`

语义契约：

- 普通 Figure 搜索和鼠标事件目标搜索不是同一件事。
- 命中搜索按 child 逆序下降，使后绘制的 child 优先命中。
- invisible child 不参与绘制和命中；disabled child 的事件策略必须明确。

Novadraw 对照：

- hit-test traversal 是引擎通用机制。
- event target/source 点转换不能下放到 editor app。

建议 probes：

- `topmost_child_wins_hit_test`
- `hidden_child_is_not_hit`
- `mouse_event_target_differs_from_generic_find_when_disabled`

## P1 能力补齐分组

### 输入事件与 EventDispatcher

代表 API：

- `EventDispatcher`
- `SWTEventDispatcher`
- `MouseEvent`
- `MouseListener`
- `MouseMotionListener`
- `MouseWheelListener`
- `KeyListener`
- `FocusListener`

语义契约：

- 平台事件先进入 dispatcher，再由 root Figure 命中 target。
- target Figure 接收相对自身坐标的事件点。
- dispatcher 还负责 capture、focus owner、entered/exited、drag、hover 等状态机。

Novadraw 对照：

- apps 只负责 winit / 平台事件适配。
- 引擎层负责 target 查找、事件点降域、listener/handler 派发。

### Layout 实现族

代表 API：

- `XYLayout`
- `StackLayout`
- `BorderLayout`
- `GridLayout`
- `FlowLayout`
- `ToolbarLayout`
- `DelegatingLayout`

语义契约：

- LayoutManager 是可替换策略，不是 Figure 的继承分支。
- constraint 是 parent layout 对 child 的私有协议。
- preferred/minimum size 计算必须纳入 border/client area。

Novadraw 对照：

- 先保证 `LayoutManager` trait 与 validation 语义，再逐个落地具体布局。

### Connection / Anchor / Router / Locator

代表 API：

- `Connection`
- `PolylineConnection`
- `ConnectionAnchor`
- `ChopboxAnchor`
- `EllipseAnchor`
- `ConnectionRouter`
- `ManhattanConnectionRouter`
- `BendpointConnectionRouter`
- `Locator`
- `ConnectionLocator`

语义契约：

- Connection 本身也是 Figure。
- Anchor 根据 owner bounds 和 reference point 计算连接端点。
- Router 根据 anchors、constraints 和已有路径生成 `PointList`。
- Locator 用于连接线标签、端点装饰、箭头等 child placement。

Novadraw 对照：

- Connection 可以作为 P1 能力，但不应抢在 Figure 核心契约之前。
- Anchor moved 应触发 connection revalidate 或 reroute。

## P2 与 GEF 层边界

P2 能力和 GEF 层 API 只作为后续对照，不进入当前 draw2d core 主线：

- 文本 flow：`FlowFigure`、`TextFlow`、`ParagraphTextLayout`
- widget：`Button`、`Clickable`、`Toggle`、`Slider`
- 图布局：`DirectedGraphLayout`、`CompoundDirectedGraphLayout`
- 后端适配：`PrinterGraphics`、`ScaledGraphics`
- GEF 交互层：`EditPartViewer`、`Request`、`Tool`、`EditPolicy`、`Command`

这些能力可以在核心 draw2d 语义稳定后，以独立 milestone 或 GEF-like layer 形式设计。

## 后续落地方式

建议后续把本文档中的 family 逐步同步到机器可读治理文件，例如：

- `agent/draw2d-core-milestones.yaml`：只记录 milestone 协议和 probes。
- `agent/backlog/*.yaml`：记录从覆盖缺口拆出的最小 architecture delta。
- `doc/06-roadmap/*`：记录产品交付与 demo 验证矩阵。

本文档保持为人工可读的 draw2d 语义覆盖账本，避免与 milestone SSOT 抢职责。
