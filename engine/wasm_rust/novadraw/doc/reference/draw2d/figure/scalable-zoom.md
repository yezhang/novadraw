# Draw2D Scalable Figure 与 Zoom 处理

类型：`reference-analysis`

## 1. 结论

Draw2D/GEF 将缩放拆成三个独立协议：

1. `ScalableFigure` 定义 scale 契约；`ScalableLayeredPane` 等实现保存 scale，
   并覆盖绘制、尺寸与坐标变换。
2. Figure 的未缩放布局结果经过 scale 后形成 preferred/minimum size。
3. `Viewport` 根据 scaled preferred size 设置 contents bounds 和 RangeModel；
   `ZoomManager` 负责缩放前后的 view location。

`setScale()` 不按比例改写当前 bounds。bounds 是 ViewportLayout 的布局结果，不能
反向成为下一次缩放的尺寸基准。

## 2. Scalable Pane

源码：

- `org.eclipse.draw2d/IScalablePane.java`
- `org.eclipse.draw2d/ScalableLayeredPane.java`
- `org.eclipse.draw2d/ScalableFreeformLayeredPane.java`

`ScalableLayeredPane.setScale()` 只执行：

```text
scale = newZoom
fireMoved()
revalidate()
repaint()
```

`IScalablePaneHelper` 统一提供以下语义：

| 协议 | Draw2D 处理 |
|---|---|
| preferred/minimum size | 先用 `hint / scale` 查询父实现；扣除 insets 后缩放内容尺寸，再加回 insets |
| client area | 将普通 client area 乘 `1 / scale` |
| paint | 在 children 绘制前调用 `Graphics.scale(scale)` |
| child -> parent | `performScale(scale)` |
| parent -> child | `performScale(1 / scale)` |

因此 paint、layout、hit-test 和事件点转换使用同一个 scale，不允许只缩放渲染。

## 3. Viewport 与 RangeModel

源码：

- `org.eclipse.draw2d/ViewportLayout.java`
- `org.eclipse.draw2d/Viewport.java`
- `org.eclipse.draw2d/FreeformViewport.java`

普通 `ViewportLayout` 的核心行为：

```text
location = clientArea.location - viewLocation
size.width  = max(clientArea.width,  contents.scaledPreferredWidth)
size.height = max(clientArea.height, contents.scaledPreferredHeight)
contents.setBounds(location, size)
```

随后 `Viewport.validate()` 调用 `readjustScrollBars()`：

```text
horizontal.setAll(0, clientArea.width,  contents.bounds.width)
vertical.setAll(0, clientArea.height, contents.bounds.height)
```

当 scaled preferred size 小于 viewport 时，contents bounds 被扩展到 viewport，
但子树仍保持左上对齐。Draw2D 默认不自动居中；内容不能越过边框依赖的是
Viewport clip。

`FreeformViewport` 是另一套明确能力：它使用子树 `freeformExtent`，允许负坐标，
并把 extent 与 viewport client area 做 union。不能把 Freeform 的范围规则隐式
塞进普通 ScalableLayeredPane。

## 4. ZoomManager

源码：

- `org.eclipse.draw2d.zoom/AbstractZoomManager.java`
- `org.eclipse.draw2d.zoom/DefaultScrollPolicy.java`
- `org.eclipse.draw2d.zoom/MouseLocationZoomScrollPolicy.java`

`AbstractZoomManager.primSetZoom()` 的顺序是：

```text
newLocation = scrollPolicy.calcNewViewLocation(viewport, oldZoom, newZoom)
pane.setScale(newZoom)
viewport.validate()
viewport.setViewLocation(newLocation)
```

必须先计算新位置，再改变 scale；必须在设置 view location 前完成 viewport
validation，使 RangeModel 已经反映新的 scaled contents bounds。

`ZoomManager` 同时持有有序 zoom levels，`zoomIn/zoomOut` 选择相邻等级；
fit width、fit height 和 fit all 根据 viewport client area 与未缩放 preferred
size 计算比例。

默认策略保持 viewport 中心：

```text
newLocation = oldLocation + center * (newZoom / oldZoom - 1)
```

鼠标位置策略保持指针下内容不动：

```text
(mouse + oldLocation) / oldZoom
    = (mouse + newLocation) / newZoom
```

## 5. Novadraw 对应

| Draw2D | Novadraw |
|---|---|
| `ScalableFigure.scale` | `ScaleRuntime::scale` |
| 未缩放 super preferred size | `ScaleRuntime::unscaled_preferred_width/height` |
| scaled preferred size | `unscaled_preferred_size * scale` |
| scaled layout hints | `Bounded::layout_size_hints` |
| scaled layout result | `Bounded::project_preferred_size/project_minimum_size` |
| `Graphics.scale` + translate APIs | `ChildTransform::uniform` |
| `ViewportLayout.layout` | `ViewportLayout` |
| `Viewport.readjustScrollBars` | `ViewportLayout` 更新共享 RangeModel |
| `AbstractZoomManager` | `ZoomManager` |
| `DefaultScrollPolicy` | `DefaultScrollPolicy` |
| `MouseLocationZoomScrollPolicy` | `MouseLocationZoomScrollPolicy` |

Novadraw 当前显式保存非 freeform scalable pane 的未缩放 preferred size，这是对
现有 Figure API 的合理变体。布局分配的 bounds 不得修改该值。

连续手势要求同一输入事务立即处理后续 pan，因此 `ZoomManager` 调用
`FigureGraph::validate_with_update(viewport)`，同步完成 Draw2D
`viewport.validate()` 所承担的 contents bounds 和 RangeModel 更新，再设置新的
view location。

实现入口：

- `novadraw-scene/src/container/scalable.rs`
- `novadraw-scene/src/container/viewport.rs`
- `novadraw-scene/src/container/zoom.rs`

## 6. 约束

- `set_scale` 只改变 scale、失效和重绘，不直接累计缩放旧 bounds。
- scaled preferred size 永远从未缩放 preferred size 推导。
- Viewport 是 scroll SSOT，Scalable pane 是 scale SSOT。
- ZoomManager 是 scale、validation 和 view location 的唯一协调入口。
- Viewport 不提供 `zoom`、`zoom_at` 或 `zoom_to_fit`，避免第二个 zoom SSOT。
- 普通 scalable 内容小于 viewport 时左上对齐，不添加隐式居中特例。
- paint、hit-test、事件点、damage 和 scroll range 必须共享同一 ChildTransform。
- 负坐标和四向 extent 属于 Freeform 能力，需显式实现。

源码基线：eclipse/gef-classic commit `4463d9d0c`（2026-01-01）。
