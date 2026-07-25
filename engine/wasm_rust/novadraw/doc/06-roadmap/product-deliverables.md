# 产品交付清单

> 本文档承载每个 draw2d 核心 milestone 下要交付的**产品策略层清单**（具体图元数、布局数、边框数等）。
>
> **编号约定**：本文统一使用 `M1-M10`，并与 `doc/01-architecture/draw2d_api_coverage.md` 中的 milestone 映射保持一致。
> **完成判据**：API 语义覆盖（`draw2d_api_coverage.md`）+ 产品层清单（本文）+ Demo 验证（`demo-matrix.md`）三者齐过。

## 与 milestone 的挂接关系

| Milestone | 标题（协议层） | 本文章节 | 产品策略要点 |
|------|----------------|----------|--------------|
| M1 | 几何与 Graphics 基础 | [§M1](#m1-几何与-graphics-基础) | 几何类型 + Graphics 状态栈 |
| M2 | Figure 树与盒模型 | [§M2](#m2-figure-树与盒模型) | active core Figure + 三段式 paint |
| M3 | 绘制遍历与裁剪闭环 | [§M3](#m3-绘制遍历与裁剪闭环) | 嵌套裁剪 + paint/hit-test 一致性 |
| M4 | 坐标域与变换闭环 | [§M4](#m4-坐标域与变换闭环) | translate* 协议 + 入口域降域 |
| M5 | Layout + Validation + UpdateManager | [§M5](#m5-layout--validation--updatemanager) | 6 布局 + 两阶段事务 |
| M6 | 事件分发与交互状态机 | [§M6](#m6-事件分发与交互状态机) | 输入状态机 + 端口 |
| M7 | 通知语义分层 | [§M7](#m7-通知语义分层) | 六类 listener + UpdateListener |
| M8 | Viewport / Scroll / Zoom | [§M8](#m8-viewport--scroll--zoom) | ScrollPane + RangeModel |
| M9 | Connection / Anchor / Router | [§M9](#m9-connection--anchor--router) | 连线 + 4 anchor + 3 router |
| M10 | 常用 Figure 与文本/控件 | [§M10](#m10-常用-figure-与文本控件) | 6 边框 + 文本 + Tooltip + Accessible 基础 |

---

## M1 几何与 Graphics 基础

**协议层要求**：见 `draw2d_api_coverage.md` 中 M1 相关 family。本文只列产品层清单。

### 几何类型清单

- `Point` / `PrecisionPoint`
- `Rectangle` / `PrecisionRectangle`
- `Dimension` / `PrecisionDimension`
- `Insets`
- `PointList`
- `Vector`
- `Transform` / `AffineTransform`

### Graphics API 清单

- 状态栈：`pushState` / `popState` / `restoreState`
- 变换：`translate` / `scale` / `rotate`
- 裁剪：`clipRect` / `setClip`
- 填充/描边：`fillRectangle` / `drawRectangle` / `fillOval` / `drawOval` / `fillPolygon` / `drawPolygon` / `fillPath` / `drawPath`
- 文本：`drawText` / `drawString`
- 图像：`drawImage`
- 属性：`setForegroundColor` / `setBackgroundColor` / `setLineWidth` / `setLineStyle` / `setAlpha`

### 测试增量预期

+30

---

## M2 Figure 树与盒模型

**协议层要求**：见 YAML M2 `contracts`（A1/A2/A3/A5）。本节只列产品层清单。

### Active core Figure

- `RootFigure`
- `RectangleFigure`
- 测试 mock Figure

M2 只以 active surface 验证通用 Figure 机制。`EllipseFigure`、
`RoundedRectangleFigure`、`PolylineFigure`、`PolygonFigure` 和
`TriangleFigure` 的 reusable surface 验收归入 M10。

### 三段式 paint 协议

- `paint_figure` / `paint_client_area` / `paint_border`
- 顺序：`paintFigure → paintClientArea(含子) → paintBorder`

### Figure / FigureBlock / FigureGraph 角色

- `Figure` trait：几何 + 绘制
- `FigureBlock`：运行时状态（父子、可见、selected、layout、preferred/min/max size）
- `FigureGraph`：SlotMap<BlockId, FigureBlock>

### 测试增量预期

+60

---

## M3 绘制遍历与裁剪闭环

**协议层要求**：见 YAML M3 `contracts`。本节只列产品层清单。

### 渲染主循环

- `render_recursive.rs`（递归实现）
- 迭代渲染历史 POC 已归档到 git tag `archive/render-iterative-poc-20260617`
- 当前阶段不交付 I 键运行时切换或递归/迭代等价门禁

### 裁剪策略

- 父 `clientArea` 裁剪
- 子 `bounds` 裁剪
- `optimizeClip` 优化路径

### 测试增量预期

+40

---

## M4 坐标域与变换闭环

**协议层要求**：见 YAML M4 `contracts`（A4）。本节只列产品层清单。

### 协议 API

- `useLocalCoordinates` / `isCoordinateSystem`
- `translateToParent` / `translateFromParent`
- `translateToAbsolute` / `translateToRelative`
- `SceneDispatchContext`（入口域 → target/source 域降域）

### 测试增量预期

+50

---

## M5 Layout + Validation + UpdateManager

**协议层要求**：见 YAML M5 `contracts`（A6/A7/A8）。本节只列产品层清单。

### LayoutManager 实现（6 个）

| 布局 | g2 对应 | 用途 |
|------|---------|------|
| `FlowLayout` | `FlowLayout` | 水平/垂直流式排列（验证基线） |
| `BorderLayout` | `BorderLayout` | 5 区（N/S/E/W/Center） |
| `GridLayout` | `GridLayout` | 网格 |
| `ToolbarLayout` | `ToolbarLayout` | 工具栏单行/单列 |
| `XYLayout` | `XYLayout` | 显式坐标布局 |
| `StackLayout` | `StackLayout` | 层叠（占满父） |

### 约束系统

- `LayoutConstraint` trait
- `set_constraint(figure, constraint)`

### UpdateManager 能力

- 区域失效 / clip 失效 / 整图失效
- repaint coalescing
- damage repair 全图元覆盖
- 1k+ Figure stress

### 测试增量预期

+250（含 6 布局 +150、UpdateManager +100）

---

## M6 事件分发与交互状态机

**协议层要求**：见 YAML M6 `contracts`（A9）。本节只列产品层清单。

### 输入分发状态

- `mouseTarget` / `cursorTarget` / `hoverSource` / `focusOwner` / `capture`

### 事件端口

- `MouseListener` / `MouseMotionListener` / `MouseWheelListener`
- `KeyListener` / `FocusListener`
- hit-test 全图元覆盖（含 Ellipse 精确命中）

### 测试增量预期

+100

---

## M7 通知语义分层

**协议层要求**：见 YAML M7 `contracts`。本节只列产品层清单。

### 六类 listener（不允许压扁成单一总线）

1. `FigureListener` —— 几何变化（`figureMoved`）
2. `CoordinateListener` —— 坐标域变化（`coordinateSystemChanged`）
3. `PropertyChangeListener` —— 通用属性变化
4. `AncestorListener` —— 祖先链变化
5. `LayoutListener` —— 布局生命周期 hook
6. Input listeners —— 输入分发末端 hook（与 M6 共享）

### UpdateListener（挂 UpdateManager，不挂 Figure）

- `notifyValidating()`
- `notifyPainting(damage, dirty_regions)`

### 测试增量预期

+80

---

## M8 Viewport / Scroll / Zoom

**协议层要求**：见 YAML M8 `contracts`。本节只列产品层清单。

### Viewport 体系

- `Viewport` Figure（坐标系节点）
- `ScrollPane` 容器
- `RangeModel` 抽象
- `ScrollBar` 模型（H+V）
- 鼠标滚轮
- `AutoexposeHelper`

### Scalable 能力

- `ScalableFigure`
- `ScalableLayeredPane`
- Zoom transform 集成

### 收口要求

- 历史暂停项不阻塞当前 M2 恢复热路径；仅在执行 M8 时纳入验收。

### 测试增量预期

+120

---

## M9 Connection / Anchor / Router

**协议层要求**：见 YAML M9 `contracts`。本节只列产品层清单。

### Connection 图形

- `Connection`
- `PolylineConnection`

### Anchor 实现（4 个）

| Anchor | g2 对应 |
|--------|---------|
| `ChopboxAnchor` | `ChopboxAnchor` |
| `EllipseAnchor` | `EllipseAnchor` |
| `SlopeAnchor` | `SlopeAnchor` |
| `LabelAnchor` | `LabelAnchor` |
| `XYAnchor` | `XYAnchor` |

### Router 实现（3 个，简化版）

| Router | g2 对应 | 说明 |
|--------|---------|------|
| `BendpointConnectionRouter` | `BendpointConnectionRouter` | 用户折点 |
| `ManhattanConnectionRouter` | `ManhattanConnectionRouter` | 直角路由 |
| `FanRouter` | `FanRouter` | 多线分散 |

**显式不做**：`ShortestPathConnectionRouter`（图算法复杂，留到 Year 2）

### 装饰与层

- Decorations（箭头等）
- `ConnectionLayer`

### 测试增量预期

+150

---

## M10 常用 Figure 与文本/控件

**协议层要求**：见 YAML M10 `contracts`。本节只列产品层清单。

### Reusable Figure surface

- `RectangleFigure` 作为 active baseline 复查
- `EllipseFigure`
- `RoundedRectangleFigure`
- `PolylineFigure`
- `PolygonFigure`
- `TriangleFigure`

这些 deferred builtin Figure 在 M10 必须补齐与 `RectangleFigure` 等价的
paint、hit-test、border、style 和更新协议证据；类型可导出不等于 M10 已完成。

### Border 实现（6 个）

| Border | g2 对应 |
|--------|---------|
| `LineBorder` | `LineBorder` |
| `MarginBorder` | `MarginBorder` |
| `TitleBarBorder` | `TitleBarBorder` |
| `CompoundBorder` | `CompoundBorder` |
| `EtchedBorder` | `EtchedBorder` |
| `BevelBorder` | `BevelBorder` |

可选附加：`FocusedBorder`

### 文本 + 图像

- `Label`（cosmic-text 集成）
- 文本布局 + 截断 + align
- `ImageFigure`
- 字体管理
- 文本与边框布局交互

### 控件类 Figure

- Button-like / Toggle-like

### Tooltip + Accessibility 基础

- `TooltipHelper`（悬停延迟 + show/hide）
- `Accessible` bridge **接口骨架**（仅键盘可达性，不做完整 ARIA）

### 测试增量预期

+220（含 6 边框 +80、文本图像 +100、Tooltip+Accessible +40）

---

## 测试增量汇总

| Milestone | 增量 |
|-----------|------|
| M1 | +30 |
| M2 | +60 |
| M3 | +40 |
| M4 | +50 |
| M5 | +250 |
| M6 | +100 |
| M7 | +80 |
| M8 | +120 |
| M9 | +150 |
| M10 | +220 |
| **合计** | **+1,100** |

当前基线 146，目标 ~1,250。

---

## 边界声明

### 本文不持有的内容

- API 语义覆盖（在 `draw2d_api_coverage.md`）
- 细粒度验证项（在相关测试与 `demo-matrix.md`）
- Demo 名称与验证策略（在 `demo-matrix.md`）

### 不在 draw2d 核心的能力（明确排除）

以下能力**不挂在 M1-M10 任何 milestone 下**，按当前路线图边界明确排除：

- EditPart / EditPolicy / Tool / Command / Request / Viewer
- Palette / Selection provider
- Undo-redo command stack
- "节点编辑器毕业 demo"（创建/拖拽/连接/删除节点）—— 这是 GEF 层能力，详见 `demo-matrix.md` 附录"GEF 层早期探索"

如果未来要做这些，需要新开 `doc/07-gef-roadmap/` 或类似目录，不污染 draw2d 核心目标。
