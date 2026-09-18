# Draw2D API 语义覆盖清单

类型：`parity-contract`

本文档记录 draw2d API 到 Novadraw 的语义覆盖关系，用作架构设计、功能迭代和验收补充的覆盖账本。

Draw2D 源码事实基线：eclipse/gef-classic commit `4463d9d0c`（2026-01-01）。
Novadraw 覆盖状态随本仓库演进单独更新。

它不是 Java API 迁移待办，也不是进度管理或任务编排文件。
当前核心收口与功能迁移顺序见
`doc/archive/core-completion-and-draw2d-migration-plan.md`。

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
    owner: Bounded / FigureTree
    graph_access: FigureId
    difference_reason: Novadraw 使用 ID 引用树；bounds 通过 Bounded trait 访问，FigureNode 只提供只读运行时视图
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
| P0 | Figure 树结构 | `IFigure.add/remove/getChildren/getParent/setParent` | `FigureTree` / `FigureId` 树 | 父子关系、顺序、删除、重挂载、逆序遍历 |
| P0 | Figure 生命周期 | `addNotify/removeNotify` | 节点挂载/卸载 hook | 子树 attach/detach、资源初始化、事件解绑 |
| P0 | Bounds 几何 | `getBounds/setBounds/getLocation/setLocation/getSize/setSize/translate` | `Bounded` / `FigureTree` bounds API | bounds 改变后的 repaint、layout invalidation、子坐标影响 |
| P0 | ClientArea / Insets | `getClientArea/getInsets` | border-inset 后的内容区 | paint clip、layout area、hit-test descent 共用同一 inset 逻辑 |
| P0 | Paint 主协议 | `paint(Graphics)` | `FigureTree::render` / 内部递归 traversal + `Figure::paint_figure` | 背景、border、client area、children paint 顺序 |
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
| P1 | 监听器系统 | `FigureListener/AncestorListener/CoordinateListener/LayoutListener/PropertyChangeListener/ActionListener` | event/listener hooks | bounds 变化、ancestor 变化、layout 生命周期、属性变更、控件 action |
| P1 | 输入事件 | `MouseListener/MouseMotionListener/MouseWheelListener/KeyListener/FocusListener` | engine event dispatch | target/source 点转换、capture、hover、drag、wheel、key |
| P1 | EventDispatcher | `SWTEventDispatcher/EventDispatcher` | 平台输入适配 + 引擎分发 | apps 只适配平台事件，引擎负责命中和派发 |
| P1 | Focus | `requestFocus/hasFocus/isFocusTraversable` | focus manager | focus owner、tab traversal、focus gained/lost |
| P1 | ClippingStrategy | `getClippingStrategy/setClippingStrategy` | child clip policy | 子节点是否被 parent/client area 裁剪，可替换策略 |
| P1 | 基础 Figure 类型 | `Figure/Label/ImageFigure/RectangleFigure/Ellipse` | builtin figures | 基础图元、文本、图片、容器 figure |
| P1 | SWT 宿主控件 | `FigureCanvas/LightweightSystem` | app/host integration | `FigureCanvas` 是承载 Draw2D 的 SWT Canvas，不是 Figure 子类 |
| P1 | 几何 Primitive | `Rectangle/Point/Dimension/Insets/PointList/Precision*` | `novadraw-math` / `novadraw-geometry` | 整数/浮点策略、包含/相交/扩张/平移 |
| P1 | 具体布局 | `XYLayout/StackLayout/BorderLayout/GridLayout/FlowLayout/ToolbarLayout` | layout implementations | 绝对布局、栈布局、边界布局、网格、流式、工具栏 |
| P1 | Freeform / Layer | `Layer/FreeformLayer/FreeformLayout/FreeformViewport` | 大画布/自由坐标层 | 负坐标、内容范围、root/layer 分层 |
| P1 | Viewport / Scroll | `Viewport/ScrollPane/ScrollBar/RangeModel` | 视口组件，当前可延后 | viewport clip、scroll offset、content extent |
| P1 | Connection | `Connection/PolylineConnection` | edge figure / connection figure | source/target、point list、重路由、连接子 figure |
| P1 | Anchor | `ConnectionAnchor/ChopboxAnchor/EllipseAnchor/XYAnchor` | anchor trait | owner bounds 变化触发连接重算、参考点计算 |
| P1 | Router | `ConnectionRouter/Manhattan/Bendpoint/ShortestPath` | router trait | route 输入输出、constraint、invalidate/remove |
| P1 | Locator | `Locator/ConnectionLocator/EndpointLocator/MidpointLocator` | decoration/label placement | 在线段、端点、相对 bounds 上定位 child |
| P1 | Widget 辅助 | `Button/Clickable/Toggle` | 基础交互 Figure | action、selected、pressed、rollover、focus |
| P1 | Accessibility bridge | `Accessible/AccessibilityDispatcher` | engine snapshot + platform bridge | name、role、state、bounds、children、focus、action |
| P2 | 文本 Flow | `FlowFigure/TextFlow/ParagraphTextLayout` | 富文本/段落布局 | inline/block flow、换行、文本测量 |
| P2 | 完整 Widget Toolkit | `ButtonGroup/CheckBox/RadioButton/Slider`、repeat firing | 可选控件库 | 不污染核心 draw2d 协议 |
| P2 | 图布局 | `DirectedGraphLayout/CompoundDirectedGraphLayout` | 后续自动布局能力 | DAG/复合图布局，可作为独立算法模块 |
| P2 | 打印 / 缩放 Graphics | `PrinterGraphics/ScaledGraphics` | backend adapter | 缩放代理、打印目标、非主线渲染后端 |
| GEF 层 | Viewer 映射 | `EditPartViewer.findObjectAt` | 编辑器层，不进 draw2d core | Figure 到 app object / EditPart 映射 |
| GEF 层 | Request / Tool | `Request/SelectionRequest/LocationRequest` | 编辑器交互层 | selection、drag、create、reconnect 请求 |
| GEF 层 | EditPolicy / Command | `EditPolicy/getCommand` | 后续 GEF-like 层 | 命令生成、交互策略，不属于 M1-M10 核心 |

## API Family ID 约定

下列 ID 用于稳定引用本文档中的 API family。后续设计、实现、测试和验收说明应优先使用这些 ID，避免同一语义被重复命名。

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
| `layout.manager` | LayoutManager | layout、constraint、preferred/min size、invalidate；maximum size 属于 IFigure |
| `validation.protocol` | Validation | invalidate、invalidateTree、revalidate、validate、validation root |
| `update_manager.two_phase` | UpdateManager | Validation -> Damage Repair 两阶段更新事务 |
| `damage.repaint` | Damage / Repaint | repaint、dirty region、intersects、damage parent-chain 映射 |
| `frame.preparation` | Frame preparation | 稳定派生状态、validation、damage、resource 与 in-flight submission 的因果边界 |
| `runtime.identity` | Runtime 身份域 | 公开 handle 的 Runtime 归属与跨 Runtime 误用拒绝 |
| `resource.lifecycle` | Resource 生命周期 | Pending/Ready/Failed/Removed、revision、提交顺序与 retry 恢复 |
| `render.backend_session` | Backend session | backend 建立/重建、Ready 资源快照与完整场景恢复 |
| `notification.figure` | Figure 通知 | Figure moved / bounds changed 等对象状态通知 |
| `notification.coordinate` | Coordinate 通知 | coordinate root 或坐标系统变化通知 |
| `notification.property` | Property 通知 | property change 语义 |
| `notification.action` | Action 通知 | button-like action 发生事实与稳定 revision |
| `notification.ancestor` | Ancestor 通知 | parent-chain add/remove/move 通知 |
| `notification.layout_update` | Layout / Update 通知 | layout lifecycle、validating、painting phase 通知 |
| `accessibility.bridge` | Accessibility 桥接 | engine-owned accessibility snapshot/delta 与平台 adapter |
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

本文只提供人工可读的阶段映射，用于理解不同能力组之间的语义依赖；具体交付节奏以当前开发计划为准。

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
| M10 常用 Figure 与文本/控件 | `builtin.figures`, `border.protocol`, `text.flow`, `widgets.basic`, `notification.action`, `accessibility.bridge` | `layout.manager`, `event.input_listeners`, `figure.properties` | deferred builtin Figure 升级为完整 reusable surface；具体 Figure 只能消费核心协议，不引入特例 |

## 方法级 API 跟踪矩阵

本节记录从 draw2d 源码抽取的方法级 API 语义，用于后续把 family 级覆盖拆成可执行的
architecture delta、contract test 和产品入口检查。它不是要求逐方法照搬 Java API；Novadraw
可以使用 trait、`FigureId`、图 API、上下文对象或命令对象表达等价语义。

校准规则：

- 如果本项目已有合理 public API 命名，优先记录实际 Rust 名称和导出路径。
- 如果本项目尚未声明 API，而本文档已有合理 family 命名，则继续使用本文档 family 名称作为目标契约名。
- 状态只描述当前外部可达 public API 与语义闭环，不把私有 helper、TODO/no-op 或未来草案记为已完成。

状态含义：

- `verified`：已有 public API、实现路径和测试或 milestone 完成证据。
- `partial`：已有骨架或部分实现，但语义闭环、测试、导出或产品入口不完整。
- `missing`：没有可定位的 Novadraw public API。
- `deferred`：已有代码或设计方向，但按 Figure Surface 分层暂不计入当前 milestone 门禁。

### M1 Graphics / Geometry

| Family ID | Draw2D 方法级 API | Novadraw 实际 / 目标 API | 状态 | 后续跟踪 |
|---|---|---|---|---|
| `graphics.context` | `Graphics.pushState/popState/restoreState` | `novadraw_render::NdCanvas::{push_state,pop_state,restore_state}` | verified | 保持 M1 state stack probe |
| `graphics.context` | `Graphics.clipRect/setClip/getClip`; `clipPath` | `NdCanvas::{clip_rect,set_clip,reset_clip,clip_depth}` 覆盖 Core 1.0 矩形裁剪；`getClip/clipPath` 未提供 | partial | 矩形 clip 已验证；clip 查询与 path clip 明确延后到出现真实产品需求 |
| `graphics.context` | `Graphics.translate/scale/rotate/shear`; `getAbsoluteScale` | `NdCanvas::{translate,scale,rotate,transform,set_transform,reset_transform}`；任意 affine 可表达 shear，未提供 `getAbsoluteScale` convenience | partial | transform 语义已验证；状态查询 convenience 明确延后 |
| `graphics.context` | `drawLine/drawRectangle/drawOval/drawPolygon/drawPolyline/drawPath` | `NdCanvas::{line,draw_rectangle,draw_oval,draw_polygon,polyline}`；路径使用 `begin_path/move_to/line_to/.../stroke/fill` | verified | M1 状态栈与 M10.1 reusable Figure 已覆盖实际消费路径 |
| `graphics.context` | `fillRectangle/fillOval/fillPolygon/fillPath/fillGradient` | `NdCanvas::{fill_rectangle,fill_rect,fill_oval,fill_polygon,fill}`；`fillPath` 对应 path + `fill()` | partial | solid fill 已验证；gradient 明确延后 |
| `graphics.context` | `drawRoundRectangle/fillRoundRectangle` | `RoundedRectangleFigure` 通过通用 path + stroke/fill 等价表达，不增加 convenience primitive | verified | 除非出现新的跨 Figure 复用证据，否则不机械增加同名 API |
| `graphics.context` | `drawString/drawText/drawTextLayout/fillText/getFont/getFontMetrics/setFont` | raw-string API 已删除；`NdCanvas::{draw_text_layout,fill_text_layout,stroke_text_layout}` 只消费 Runtime shaping 后的 `TextLayout` / `DrawGlyphRun` | verified | M10.2 backend-neutral layout metadata、字体 revision 与 Vello glyph adapter 已覆盖 |
| `graphics.context` | `drawImage(...)` | `NdCanvas::{draw_image,draw_image_with_size}` 消费 `ImageResourceRef`；command 保留 `src_rect` 字段但当前产品入口只绘制完整 source | partial | 资源 revision、缩放与 ImageFigure 已验证；source rectangle 明确延后 |
| `graphics.context` | `setAlpha/setAntialias/setLineDash/setLineCap/setLineJoin/setLineMiterLimit/setXORMode` | `NdCanvas::{set_alpha,line_cap,line_join}`；line dash/miter、antialias、XOR 暂无 public parity | deferred | 高级 stroke/style 不进入 M1 完成门禁；未实现 API 不暴露静默 no-op |
| `geometry.primitives` | `Point/Dimension/Rectangle/Insets/PointList/Precision*` | `novadraw_geometry::{Point,Dimension,Rectangle,Insets,PointList,Transform,Precision*}`；`novadraw_math::{Mat3,Vec3}` | verified | `PointList` 需在 M9/M10 connection/point-list shape 中复查 |

Draw2D 证据入口：`Graphics.java`、`SWTGraphics.java`、`ScaledGraphics.java`、`PrinterGraphics.java`。

### M2 Figure Tree / Box Model

| Family ID | Draw2D 方法级 API | Novadraw 实际 / 目标 API | 状态 | 后续跟踪 |
|---|---|---|---|---|
| `figure.tree` | `IFigure.add(IFigure)` | 构建期 `FigureTreeBuilder::{set_contents,add_child_to,try_add_child_to}`；运行期 `Runtime::{set_contents,add_figure}` | verified | 保持 child order、single/layer admission、no-cycle 与 10,000 层深度门禁 |
| `figure.tree` | `add(IFigure,int)`, `add(IFigure,Object,int)` | 构建期可组合 add + index/constraint；Runtime 提供现有 child 的 `move_child_to_index/bring_child_to_front/send_child_to_back`，尚无原子 indexed add | partial | D3.2 已闭合动态 reorder；原子 indexed/constraint add 等待真实调用需求 |
| `figure.tree` | `remove(IFigure)`, `removeAll()`, `getParent()`, `setParent(IFigure)` | `Runtime::{remove_figure,reparent,dispose_subtree}`、`FigureTree::parent_id`；remove 释放 arena slot 与 side state，reparent 保持同 Runtime 身份 | partial | D4.3 已闭合 dispose/reparent；通用活对象迁移撤回，`removeAll` convenience 延后 |
| `figure.tree` | `getChildren()` | `FigureNode::children_count`、`FigureTree::child_order/descendant_ids` 提供稳定只读查询 | verified | 不暴露可修改内部 children 集合的引用 |
| `figure.lifecycle` | `addNotify()`, `removeNotify()` | `FigureLifecycle::{on_attached,on_detached}`、parent-first activation、descendant-first disposal、旧 visual damage 与 side-state 清理 | verified | D4.3 自动契约覆盖 10,000 层 dispose 与 panic/fault 边界 |
| `figure.geometry.bounds` | `getBounds/setBounds/getLocation/getSize/setSize/translate` | `NodeState` 是运行时几何真源；`FigureTree::figure_bounds` 只读，`Runtime::{set_bounds,translate}` update-aware 修改 | verified | `Bounded` 仅保留构造期和独立图元兼容，不是树内真源 |
| `figure.box.client_area` | `getClientArea()`, `getClientArea(Rectangle)`, `getInsets()` | `Bounded::{client_area,insets}` | verified | M5 layout area、M8 viewport client area 继续复查 |
| `figure.visibility.enabled` | `isVisible/setVisible/isShowing/isEnabled/setEnabled` | `FigureTree::{set_visible,set_enabled,is_visible,is_enabled,is_effectively_visible,is_effectively_enabled}` | verified | M6 复查 disabled 对 event target 的策略 |
| `hit_test.search` | `containsPoint`, `intersects`, `findFigureAt`, `findMouseEventTargetAt` | `Bounded::{contains_point,intersects}`, `FigureTree::{hit_test,hit_test_simple,find_mouse_event_target_at}` | verified | 保持逆序命中和 visible/enabled probes |
| `hit_test.search` | `findFigureAtExcluding`, `TreeSearch.accept/prune` | `TreeSearch`、`TreeSearchContext`、`ExclusionSearch`、`FigureTree::{hit_test_with,hit_test_excluding,find_in_subtree,ancestor_ids,descendant_ids,is_ancestor_of}` | verified | D1.5a：共享 hit-test traversal、prune 子树与稳定结构查询已有契约测试 |
| `figure.properties` | `foreground/background/font/cursor/opaque` | `FigureStyle`、`ResolvedStyle`、Runtime update-aware style/opaque mutation 与 cursor 查询 | verified | D1.4：继承、局部覆盖、通知、绘制应用与 macOS 人工验收已完成 |
| `figure.properties` | `tooltip` | FigureStyle tooltip 三态继承；Runtime-owned waiting/visible/hidden、单调 deadline、Show/Replace/Hide update 与共享边界 placement | verified | `m10_tooltip_contract`、Native overlay 截图与 Web DOM hover 验证 |

Draw2D 证据入口：`IFigure.java`、`Figure.java`。

### M3 Paint / Clip / Border

| Family ID | Draw2D 方法级 API | Novadraw 实际 / 目标 API | 状态 | 后续跟踪 |
|---|---|---|---|---|
| `paint.protocol` | `IFigure.paint(Graphics)` | `FigureTree::render` 递归承载 traversal；local style 通过 Graphics state 栈继承，self/children/border 与兄弟隔离保持固定 | verified | D4.5 关闭 ancestor style O(N²)，1k/10k 命令等价与深度门禁通过 |
| `paint.protocol` | `Figure.paintFigure/paintClientArea/paintBorder` 扩展点 | `Figure::{paint_figure,paint_border}`；client-area 与 child traversal 只由递归 renderer 固定执行，误导性的 `Figure::paint_children` no-op 已删除 | verified | 不开放绕过树遍历、坐标和 clip 协议的 child paint |
| `clipping.strategy` | `getClippingStrategy/setClippingStrategy` | Figure capability 提供默认值；`NodeState` 保存 Runtime override；`Runtime::set_child_clipping_strategy` 受控替换三种核心策略 | partial | Core 1.0 replacement 已验证；任意多矩形 provider 延后到真实需求 |
| `border.protocol` | `Border.getInsets/paint` | `Border::{get_insets,paint,get_color,get_width}`；`paint` 显式接收 owner bounds 与 `NdCanvas` | verified | Rust trait 不复制 Draw2D owner object 参数 |
| `border.protocol` | `Border.getPreferredSize/isOpaque` | `Border::{preferred_size,is_opaque}`，FigureTree 将 owner-scoped metrics 合并进盒模型与 opaque 判断 | verified | Compound 递归快照与共享 TitleBar 多 owner 契约已覆盖 |
| `border.protocol` | concrete border implementations | `LineBorder`、`MarginBorder`、`RectangleBorder`、`CompoundBorder`、`EtchedBorder`、`BevelBorder`、`TitleBarBorder` | verified | `border-app` 与 M10 border/text 契约测试；动态子 Border 可位于任意组合深度 |
| `damage.repaint` | `erase()`, `repaint()`, `repaint(Rectangle)` | Runtime/FigureTree update-aware repaint、全量 repaint、old/new visual erase 与 `UpdateManager::add_dirty_region` | verified | `erase` 保持 Runtime 内部 helper；dirty merge、parent-chain 投影和 partial repair 已验证 |

Draw2D 证据入口：`Figure.java`、`Border.java`、`AbstractBorder.java`、`LabeledBorder.java`。

### M4 Coordinates / Event Point Reduction

| Family ID | Draw2D 方法级 API | Novadraw 实际 / 目标 API | 状态 | 后续跟踪 |
|---|---|---|---|---|
| `coordinate.conversion` | `translateToAbsolute(Translatable)`, `translateToRelative(Translatable)` | `FigureTree::{translate_to_absolute_mut,translate_to_relative}` | verified | M4 contract 覆盖多层坐标根、Insets 与 Point/Rectangle 往返 |
| `coordinate.conversion` | `translateToParent(Translatable)`, `translateFromParent(Translatable)` | `FigureTree::{translate_to_parent,translate_from_parent}`，`Viewport::{translate_to_parent,translate_from_parent}` | verified | active core 已验证；Viewport 扩展仍归 M8 |
| `coordinate.conversion` | `isCoordinateSystem()` | 无对应模式开关；所有树边统一使用 parent-local bounds，容器通过 `FigureContainer::child_transform` 提供额外 Affine2D | verified | 坐标根语义由显式变换边表达；移动 ancestor 不改写 descendant bounds |
| `coordinate.conversion` | `isMirrored()` | 暂无 public 等价 API | deferred | 可延后，当前不阻塞 M4 |
| `event.point_reduction` | MouseEvent target point 转为 target local 域 | `MouseEvent::with_target_point`, `MouseEvent::entry_point`, `EventDispatcher` dispatch 路径 | verified | M4 contract 验证 hit-test、entry point 与 target-domain callback 同源 |

Draw2D 证据入口：`IFigure.java`、`Figure.java`、`Viewport.java`。
Novadraw 验证入口：`novadraw-scene/tests/m4_coordinate_contract.rs`、`apps/native/transform-app`。

### M5 Layout / Validation / UpdateManager

| Family ID | Draw2D 方法级 API | Novadraw 实际 / 目标 API | 状态 | 后续跟踪 |
|---|---|---|---|---|
| `layout.manager` | `LayoutManager.getConstraint/setConstraint/remove` | parent-owned typed constraint；`Runtime::{set_layout_constraint,remove_layout_constraint}` 与 callback deferred mutation 在提交前校验 manager compatibility | verified | D3.2 原子失败、FIFO 与 validation queue 契约测试通过 |
| `layout.manager` | `getPreferredSize(IFigure,wHint,hHint)`, `getMinimumSize(...)` | LayoutManager/LayoutContext 测量；Runtime preferred/minimum/maximum override set/clear | verified | D3.2 覆盖有限非负校验、clear fallback 与 update-aware invalidation |
| `layout.manager` | `LayoutManager.invalidate(IFigure)`, `layout(IFigure)` | 无缓存布局采用图级 invalid path；`layout` 通过 `LayoutContext` 操作 children | verified | 缓存布局未来需重新声明 invalidate hook |
| `layout.manager` | concrete layout implementations | 六类布局算法、构建期配置与 Runtime 动态 replacement 已验证；新 manager 提交前校验已有 constraints | verified | `m5_layout_contract` 覆盖 child-content client area、inset 单次应用与 Border 非对称区域分配；`d3_runtime_mutation` 与 `layout-app` |
| `validation.protocol` | `IFigure.invalidate`, `invalidateTree`, `revalidate`, `validate`, `setValid` | `FigureTree::{invalidate,mark_invalid,revalidate,perform_validation_cycle,is_valid}`，validation root、重复失效与回调延迟失效已闭合 | verified | hidden/disabled 子树恢复时经 update-aware setter 重新入队 |
| `update_manager.two_phase` | `addInvalidFigure`, `performValidation`, `performUpdate`, `runWithUpdate` | `UpdateManager` 串联 Validation -> Damage Repair；支持非重入、panic 恢复、周期快照和因果通知 | verified | `runWithUpdate` 由组合根事务表达 |
| `damage.repaint` | `UpdateManager.addDirtyRegion`, `performUpdate(Rectangle exposed)` | dirty 合并、根域传播、`DamageMode::{None,Full,Partial}` 与 retained frame 提交已闭合 | verified | exposed-rect overload 作为 P1 扩展 |

Draw2D 证据入口：`LayoutManager.java`、`UpdateManager.java`、`DeferredUpdateManager.java`。

### M6 Event Dispatcher / Focus / Capture

| Family ID | Draw2D 方法级 API | Novadraw 实际 / 目标 API | 状态 | 后续跟踪 |
|---|---|---|---|---|
| `event.dispatcher` | `dispatchMousePressed/Released/Moved` | `EventDispatcher::{receive,dispatch_mouse_pressed,dispatch_mouse_released,dispatch_mouse_moved}`，`Event::Mouse`, `MouseEventKind` | verified | target-domain callback 与 capture 状态测试 |
| `event.dispatcher` | dispatcher consumed / capture result | `DispatchOutcome::{target,is_handled,capture}` 从 Runtime 返回平台无关的分发结果 | verified | Editor G3 P2 delta；不改变既有 Figure callback 或 capture 状态机 |
| `event.dispatcher` | `dispatchMouseDragged/Entered/Exited/Hover/DoubleClicked` | entered/exited 与 hover callback 由 `mouseTarget` 路由；`hoverSource` 专用于 tooltip source；pointer leave 清理 capture/pressed | verified | 交互父 + 非交互子、capture release/leave、mouse/cursor/tooltip target 分轨 |
| `event.dispatcher` | `setRoot`, `setControl` | `Runtime::set_contents` 管理 root；`PlatformHost` 注入平台服务，避免 dispatcher 持有原生 control | verified | 接受组合根 + host adapter 变体，apps 只做平台输入适配 |
| `event.focus` | `requestFocus`, `requestRemoveFocus`, `getFocusOwner`, `hasFocus`, `isFocusTraversable` | `Runtime::{request_focus,clear_focus,traverse_focus}`、`InteractionState::focus_owner`、`FocusTraversalPolicy`、`FocusEvent` | verified | D1.5b 引擎 focus model 与 D1.5c Native/Web Tab traversal 已通过自动验证及 macOS/Web 人工验收 |
| `event.dispatcher` | `setCapture`, `releaseCapture`, `isCaptured` | handled press 自动 capture，release 自动释放；`FigureTree::{captured,set_captured}` | verified | captured target 与 hoverSource 独立 |
| `event.input_listeners` | `MouseWheelListener`, `KeyListener`, `FocusListener` | `WheelEvent`、`ZoomEvent`、`KeyEvent`、`FocusEvent` 与 Figure callback 端口；scroll/zoom session 固定 target | verified | Winit 只在 `novadraw-apps` 适配单位、DPI 与 phase；pointer capture 与 gesture session 分轨 |
| `event.dispatcher` | `updateCursor`, `getAccessibilityDispatcher` | cursor 与 Tooltip 经 PlatformHost effect；Runtime 从 stable scene 发布 accessibility Snapshot/Delta，Host 只做平台映射 | verified | M10.5 保留 engine semantics / platform adapter 分层 |

Draw2D 证据入口：`EventDispatcher.java`、`SWTEventDispatcher.java`、`MouseEvent.java`、listener 接口。

### M7 Notification / Listener Semantics

| Family ID | Draw2D 方法级 API | Novadraw 实际 / 目标 API | 状态 | 后续跟踪 |
|---|---|---|---|---|
| `notification.figure` | `add/removeFigureListener`; figure moved / bounds changed | `Runtime::add_figure_listener` + 统一 `remove_listener`；typed committed-state event | verified | D3.3 Runtime 注册、触发、注销与 self-removal 已验证 |
| `notification.ancestor` | `add/removeAncestorListener` | `Runtime::add_ancestor_listener`；Added/Moved/Removed typed event | verified | D3.3 Runtime 公共路径已验证 |
| `notification.coordinate` | `add/removeCoordinateListener` | `Runtime::add_coordinate_listener`；`CoordinateSystemChanged` typed event | verified | D3.3 Runtime 公共路径已验证 |
| `notification.property` | `add/removePropertyChangeListener`, 按 property name 监听 | `Runtime::add_property_listener` + typed old/new value；Toggle selected 与 editor selection 分离 | verified | D3.3 统一 ListenerId、注销和 callback self-removal 已验证 |
| `notification.action` | `ActionListener.actionPerformed` | `Runtime::add_action_listener` + `ActionEvent { block_id, revision }`；与 property change 进入同一 effect queue | verified | M10.4 顺序与 D3.3 self-removal 契约测试通过 |
| `notification.layout_update` | `add/removeLayoutListener`, validating/painting | typed listener + `NotificationRecord { source_epoch, sequence }`；`StableSceneQuery` 只读 flush 时最新稳定场景 | verified | D4.4 验证历史事件 FIFO 与最新 stable query 不混淆；阶段记录不是事前 hook |

Draw2D 证据入口：`IFigure.java`、`Figure.java`、`UpdateManager.java`、listener 接口。

### M8 Viewport / Scroll / Zoom

| Family ID | Draw2D 方法级 API | Novadraw 实际 / 目标 API | 状态 | 后续跟踪 |
|---|---|---|---|---|
| `viewport.scroll_zoom` | `Viewport.getContents/setContents` | `ViewportHandle::{contents,set_contents}`；`ChildPolicy::Single` 在 add/reparent 入口强制单 contents | verified | `m8_viewport_contract` 覆盖替换与原子拒绝 |
| `viewport.scroll_zoom` | `get/setHorizontalRangeModel`, `get/setVerticalRangeModel` | `RangeModel` / `DefaultRangeModel`；Viewport 与 ScrollBar 在私有 runtime 中共享模型，公开 handle 提供 snapshot | verified | ViewportLayout 通过 sealed typed effect 在完整 LayoutOutput 校验后提交 range/content scale |
| `viewport.scroll_zoom` | `getViewLocation`, `setViewLocation`, `setHorizontalLocation`, `setVerticalLocation` | `ViewportHandle::{view_location,set_view_location,set_horizontal_location,set_vertical_location,scroll_by}` | verified | clamp、property/coordinate effect 与 repaint 已覆盖 |
| `viewport.scroll_zoom` | `ScalableFigure`、`AbstractZoomManager`、`IZoomScrollPolicy`、zoom levels、fit | `ScalableFigure`、`ScaleHandle`、`ZoomManager`、`ZoomScrollPolicy`、`DefaultScrollPolicy`、`MouseLocationZoomScrollPolicy` | verified | `setScale` 只失效；manager 执行 location → scale → validate → scroll；Viewport 不保存 zoom |
| `viewport.scroll_zoom` | `get/setContentsTracksWidth/Height` | `ViewportHandle::{contents_tracks_width,contents_tracks_height,set_tracks_width,set_tracks_height}` + `ViewportLayout` | verified | minimum/preferred size 与 range extent 已覆盖 |
| `viewport.scroll_zoom` | `ScrollPane.getViewport/setViewport`, `setContents`, `scrollTo`, scrollbar visibility | `ScrollPaneHandle::{viewport,set_contents,scroll_to,set_scroll_bar_visibility}` + `ScrollPaneLayout` | verified | 标准组合固定持有一个 viewport，不开放破坏组合不变量的 setViewport |
| `viewport.scroll_zoom` | `ScrollBar.get/setRangeModel`, `get/setValue`, `stepUp/stepDown`, increments | `ScrollBarFigure` 与 Viewport 共享 RangeModel，支持单次 press step、page 与 thumb drag；Lines/LogicalPixels 分级，wheel 未消费时沿祖先 fallback | verified | `m8_viewport_contract` + `scroll-pane-demo --verify`；Draw2D `REPEAT_FIRING` 按住连发随 P2 widget repeat scheduler 延后 |
| `layer.freeform` | `Layer.containsPoint/findFigureAt`, `LayeredPane.add/getLayer/removeLayer`, `FreeformLayer.getFreeformExtent/setFreeformBounds`, `ScalableFreeformLayeredPane` | Layer、keyed pane、Freeform 类型、派生 extent、nested 映射、OverflowVisible、`FreeformLayout` 与 content-domain Viewport/Zoom 集成 | verified | D2.1-D2.5 自动契约与 Native/Web 人工验收通过；不递归改写普通 child bounds |

Draw2D 证据入口：`Viewport.java`、`ScrollPane.java`、`RangeModel.java`、`ScrollBar.java`、`ScalableFigure.java`、`Layer.java`、`FreeformLayer.java`。

### Runtime / Resource / Submission 横切契约

| Family ID | Draw2D 方法级 API / 合理变体 | Novadraw 实际 / 目标 API | 状态 | 后续跟踪 |
|---|---|---|---|---|
| `runtime.identity` | LightweightSystem/Figure 所属图实例隔离；跨图引用不得命中 | Figure/Resource/Listener/Router/Anchor/BackendSession handle 均携带 Runtime namespace，跨 Runtime mutation 结构化拒绝 | verified | D4.3 已移除 `Runtime::into_tree` 有损重包装并验证完整 Runtime move |
| `resource.lifecycle` | 图像/字体状态变化必须按提交因果顺序到达 renderer | ResourceRegistry revision + ordered `ResourceOp`；delta retry 恢复精确前缀，Ready -> Failed -> Ready 顺序闭合 | verified | D4.2 自动契约与 Vello cache 测试 |
| `render.backend_session` | 新 Graphics/backend 必须能恢复当前资源与完整场景 | Host 串行接管以 Snapshot + Full 建立基线；拒绝缺基线 Delta 与同域 stale generation | verified | D4.2/D4.4 已验证；并发 producer activation token 不在 Core 1.0 范围 |
| `frame.preparation` | validate -> derived state -> damage repair 的稳定更新边界 | Runtime typed worklist 在 stable epoch 前收敛；公开 Ready/Idle/Suspended/AwaitingCompletion/Error，失败不提交 frame | verified | D4.1/D4.4 自动契约 |

### M9 Connection / Anchor / Router / Locator

| Family ID | Draw2D 方法级 API | Novadraw 实际 / 目标 API | 状态 | 后续跟踪 |
|---|---|---|---|---|
| `connection.figure` | `Connection.get/setSourceAnchor`, `get/setTargetAnchor` | `ConnectionRuntime` 持有 optional AnchorId、tracked dependencies 和 unresolved/rebind；route batch 先完成 Figure geometry/Locator preflight，再原子提交 geometry 与 resolution | verified | 非有限 geometry 不得形成伪 Resolved |
| `connection.figure` | `get/setConnectionRouter`, `get/setRoutingConstraint` | `RouterRegistry` + `RouterId` + inherited/explicit binding 已实现；typed constraint 归 Connection | verified | ConnectionLayer 默认 Router、显式 override 和 Fan shared group 已覆盖 |
| `connection.figure` | `getPoints/setPoints` | `RouteOutput` 经 Runtime 规范化为 ConnectionFigure local points，并同步 NodeState path bounds、paint、hit-test 与 damage | verified | 外部 setPoints 不开放；route truth 与 child visual envelope 分离 |
| `connection.anchor` | `ConnectionAnchor.getLocation`, `getOwner`, `getReferencePoint`, `add/removeAnchorListener` | 只读 Anchor 协议、5 个内置 Anchor、TrackedSceneQuery dependency tokens 已实现 | verified | 不复制 Anchor listener；依赖变化由 Runtime 精确失效 |
| `connection.router` | `ConnectionRouter.route`, `invalidate`, `remove`, `get/setConstraint` | Direct/Bendpoint/Fan 与 shared Manhattan 算法及批量提交已实现；normal frame 自动按规范 parent routing space 消费 dirty group | verified | dependency generation 覆盖 direct、callback、LayoutOutput geometry 与 routing-domain child order；ShortestPath 继续延后 |
| `clipping.strategy` | nested viewport connection clipping / unsupported topology | Core 1.0 严格比较 connection parent 与两端 owner 的 viewport chain；divergent chain 返回 `UnsupportedViewportTopology` 并清除旧 route | verified | nearest-common-viewport 多矩形 clipping 明确延后 |
| `connection.locator` | `Locator.relocate`, `ConnectionLocator`, `EndpointLocator`, `MidpointLocator` | Runtime-owned direct-child binding 消费 prepared local route；实现 endpoint、middle、indexed midpoint 和 path fraction，并随 route 提交 child bounds | partial | label/普通 child relocation 已闭合；RotatableDecoration reference/orientation capability 待 P2 delta |

规范 Rust 契约、坐标域和错误模型见
[`design/architecture/connection-routing.md`](../../design/architecture/connection-routing.md)；
由 ADR-005 接受。

Draw2D 证据入口：`Connection.java`、`PolylineConnection.java`、`ConnectionAnchor.java`、
`ConnectionRouter.java`、`Locator.java`、`AbstractRouter.java`；方法级分析见
[`reference/draw2d/figure/connection-routing.md`](../../reference/draw2d/figure/connection-routing.md)。

### M10 Reusable Figures / Text / Widgets

| Family ID | Draw2D 方法级 API | Novadraw 实际 / 目标 API | 状态 | 后续跟踪 |
|---|---|---|---|---|
| `builtin.figures` | `RectangleFigure` | `RectangleFigure::{new,from_bounds,new_with_color,with_stroke,with_local_coordinates,with_child_clipping_strategy,with_border,translate,set_bounds}` | verified | Rectangle 是 active core baseline，不属于 deferred |
| `builtin.figures` | `Shape.setFill/setOutline/setLineWidth/setLineWidthFloat` | `Shape` trait 暴露只读能力；颜色走 Runtime FigureStyle，几何 stroke 参数由 concrete builders/mutation 提供 | partial | Core 1.0 接受具体类型入口；统一 Shape mutation 明确延后到跨 Figure 需求成立 |
| `builtin.figures` | `Ellipse` | `EllipseFigure` + Runtime FigureStyle；optimized fill/outline bounds、精确椭圆命中 | verified | `m10_reusable_shape_border_contract` |
| `builtin.figures` | rounded rectangle | `RoundedRectangleFigure::{set_corner_dimensions,corner_dimensions}` + `Runtime::set_corner_dimensions`；二维圆角 path 与精确命中 | verified | 单值 radius 仅为等宽高 convenience |
| `builtin.figures` | point-list shape mutators | `Runtime::{replace_points,insert_point,set_point,remove_point,clear_points}`；parent-domain 输入原子规范化为 local points + NodeState bounds | verified | 非有限输入与非法 index 无 partial commit |
| `builtin.figures` | `Polyline.containsPoint`, `Polygon.containsPoint`, paint | segment tolerance、closed polygon interior/edge、退化点数和 local point paint 已闭合 | verified | `m10_reusable_shape_border_contract` |
| `builtin.figures` | triangle figure | Draw2D client-box/resize/居中顶点语义、精确三角形命中、`Runtime::set_triangle_direction` | verified | 精确命中是 Novadraw 合理增强 |
| `border.protocol` | concrete border implementations | `LineBorder`, `MarginBorder`, `CompoundBorder`, `EtchedBorder`, `BevelBorder`；preferred size、ring opacity、累计 inset 与 Runtime replacement | verified | `TitleBarBorder` 留在 M10.2 |
| `border.protocol` | `LabeledBorder`, `TitleBarBorder` | TitleBarBorder 消费统一 `TextLayout` 与 resolved style；owner-scoped `BorderSnapshot` 按 Compound 结构递归组合并隔离共享实例 | verified | inner/outer/nested Compound 与 shared Compound 双 owner 字体指标契约测试 |
| `builtin.figures` | `Label` text/icon constructors, alignment, gap, preferred size, truncate, paint | `LabelFigure` 支持 backend-neutral text/image resource snapshot、alignment、gap、ellipsis、Border 盒模型和 icon named geometry | verified | cache/shaping、资源事务、LabelAnchor 与 `text-app` 截图 |
| `builtin.figures` | `ImageFigure.getImage/setImage/getPreferredSize/setAlignment/paintFigure` | `ImageFigure` + `ImageId`；Runtime typed replacement/alignment；PNG/SVG decode；Pending/Ready/Failed/Unavailable；resource-referenced Image command | verified | Ready resource 删除会清除所有 dependent 的旧引用；Vello revision cache、`m10_label_contract` 与 Image_Resources 截图 |
| `text.flow` | `TextFlow.getText/setText`, fragment paint, truncate, leading word width | `MeasureConstraints` / `FigureMeasurement` 支持外部受宽度约束 Figure；父 layout 使用高度/baseline arrange 并复用同约束 Glyph IR | partial | D4.4 扩展边界已验证；完整 TextFlow fragment/bidi 按 P2 延后 |
| `widgets.basic` | `Clickable.doClick`, action/change listener, model, selected, rollover, pressed/focus paint | `ClickableFigure` + `ClickableModel`；Runtime 唯一拥有 pointer/keyboard pressed、hover、focus、capture，Figure 仅消费派生 visual snapshot | verified | release-inside、drag-out/back、Enter/Space、disabled 与 typed action 契约测试 |
| `widgets.basic` | `Button` text/image constructors and default button style | `ButtonFigure` / `ToggleFigure` 组合 `ClickableModel + LabelFigure`；bevel、pressed offset、selected/focus/disabled visual | verified | `widgets-app` 三场景截图；repeat firing 与 ButtonGroup 不进入 M10.4 |
| `accessibility.bridge` | `Accessible`、AccessibilityDispatcher、focus/default action | namespaced node identity、name/description/value/role/state/bounds/children/focus、Snapshot/Delta 与受控 focus/default action | verified | `m10_accessibility_contract` 覆盖 stable publish、层级提升、Toggle action、dispose 与 10,000 层；完整原生 AT provider 延后 |

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

pub trait ClickableBehavior {
    fn clickable_model(&self) -> &ClickableModel;
    fn clickable_model_mut(&mut self) -> &mut ClickableModel;
}
```

Draw2D 证据入口：`Shape.java`、`RectangleFigure.java`、`Ellipse.java`、`Polyline.java`、`Polygon.java`、`AbstractPointListShape.java`、`Label.java`、`ImageFigure.java`、`Clickable.java`、`Button.java`、`Toggle.java`、`ButtonModel.java`、`ToggleModel.java`、`ClickableEventHandler.java`、`text/TextFlow.java`、`text/FlowFigure.java`。M10.2 文本细节见 [`../../reference/draw2d/figure/text-label.md`](../../reference/draw2d/figure/text-label.md)，Novadraw 契约见 [`../../design/architecture/text-layout.md`](../../design/architecture/text-layout.md) 与 [`../../design/architecture/basic-widgets.md`](../../design/architecture/basic-widgets.md)。

## Milestone 推进检查规则

每次推进 M1-M10 相关功能或架构改动时，必须执行以下检查：

1. 列出受影响的 API family ID。
2. 对照本文件确认 draw2d 语义契约是否仍完整。
3. 如果契约不完整，必须新增 probe、测试、demo 断言或后续债务记录，不能只更新状态描述。
4. milestone 进入 `behavior_verified` 前，必须检查其主 API 语义已经有可重复验证证据。
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
- 根节点深度为 0；add/reparent 后的最大深度不得超过 10,000，失败必须无副作用。

Novadraw 对照：

- 使用 `FigureTree` 承载拓扑。
- 使用 `FigureId` 引用节点，避免 Java 引用树。
- Figure 属性、运行时状态、拓扑保持分离：`Figure` / `FigureNode` / `FigureTree`。

建议 probes：

- `child_order_controls_paint_and_hit_test_priority`
- `remove_child_detaches_parent_relation`
- `reparent_child_updates_old_and_new_parent`
- `tree_depth_limit_rejects_add_and_reparent_atomically`

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

- bounds 通过 `Bounded` trait 和 `FigureTree` API 访问与修改。
- `FigureNode` 提供 `figure_bounds()` 等只读运行时视图，不是对外可变状态入口。
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
- event target/source 点转换不能下放到应用层。

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
- widget：`ButtonGroup`、radio/checkbox、repeat firing、`Slider`
- 图布局：`DirectedGraphLayout`、`CompoundDirectedGraphLayout`
- 后端适配：`PrinterGraphics`、`ScaledGraphics`
- GEF 交互层：`EditPartViewer`、`Request`、`Tool`、`EditPolicy`、`Command`

这些能力可以在核心 draw2d 语义稳定后，以独立 milestone 或 GEF-like layer 形式设计。

## 后续落地方式

本文档保持为人工可读的 draw2d 语义覆盖账本。后续功能迭代应从这里选取受影响的 API family，补齐对应的运行时代码、测试、demo 或视觉验证；产品交付与 demo 验证矩阵继续放在 `doc/roadmap/` 下维护。
