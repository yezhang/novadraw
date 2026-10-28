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

## 2. `ScalableFigure` 的真实职责

源码：

- `org.eclipse.draw2d/ScalableFigure.java`
- `org.eclipse.draw2d/IScalablePane.java`
- `org.eclipse.gef/editparts/ScalableRootEditPart.java`
- `org.eclipse.draw2d.zoom/AbstractZoomManager.java`

`ScalableFigure` 本身只有两个方法：

```text
double getScale()
void setScale(double scale)
```

它的作用是为缩放控制器提供一个不依赖具体 Pane 类型的**窄执行契约**。完整缩放系统
实际分成四层：

| 层次 | 职责 |
|---|---|
| `ZoomManager` | zoom levels、fit 策略、监听通知、缩放前后的 Viewport 位置协调 |
| `ScalableFigure` | 暴露当前 scale，并接受新的 scale |
| `IScalablePane` / 具体 Pane | 将 scale 同时投影到绘制、坐标转换、测量与失效 |
| `Viewport` | clip、contents bounds、RangeModel 与 view location |

因此 `ScalableFigure` 不是：

- 完整的 zoom controller；
- 遍历并改写所有子 Figure bounds 的 resize 协议；
- Viewport 的替代品；
- 要求每个 Figure 都独立保存 scale 的通用基类。

它通常落在 `ScalableLayeredPane` 或 `ScalableFreeformLayeredPane` 这样的粗粒度
**坐标根**上。子 Figure 继续使用未缩放逻辑坐标，跨越该坐标根时才应用 scale。

GEF 还利用 Figure 树层次选择缩放范围。`ScalableRootEditPart` 将 Grid、Printable
Layers 和 `SCALED_FEEDBACK_LAYER` 放入 `SCALABLE_LAYERS`；`HANDLE_LAYER`、
普通 `FEEDBACK_LAYER` 与 `GUIDE_LAYER` 位于其外部。这样模型内容与需要跟随内容的
反馈一起缩放，而部分交互装饰可以保持屏幕空间尺寸。

依赖方向是：

```text
用户缩放操作
    -> ZoomManager 计算策略和新 view location
    -> ScalableFigure.setScale()
    -> Scalable Pane 更新绘制/坐标/测量并失效
    -> Viewport validate 后恢复目标 view location
```

这一区分很重要：`ScalableFigure` 是缩放的**执行点**，`ZoomManager` 才是缩放的
**策略与协调者**。

## 3. Scalable Pane

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

## 4. Viewport 与 RangeModel

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

## 5. ZoomManager

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

## 6. Novadraw 对应

| Draw2D | Novadraw |
|---|---|
| `ScalableFigure.scale` | `Figure::scale_model` + `ScaleModel::scale` |
| 可写 scale 协调 | `FigureTree::scale_handle` 从 capability 克隆 `ScaleModel`，`Runtime::scalable` 提交 mutation |
| 未缩放 super preferred size | Figure 的 intrinsic/preferred measurement |
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
`FigureTree::validate_with_update(viewport)`，同步完成 Draw2D
`viewport.validate()` 所承担的 contents bounds 和 RangeModel 更新，再设置新的
view location。

Editor 的 `GraphicalViewer` 长期持有一个绑定 root scalable pane 与 root viewport
的 `ZoomManager`。默认安装 `MouseLocationZoomScrollPolicy`；无 anchor 时该策略退化为
中心保持，因此同时覆盖普通缩放和指针锚点缩放。Viewer 只暴露只读 manager 查询以及
受控的 zoom levels / scroll policy 配置，不提供 `zoom_manager_mut()`。这样配置、
策略对象和未来 listener 生命周期都与 Viewer 一致，不会在每次缩放操作中重建。

### 6.1 能力迁移与代码精简

Draw2D 中值得迁移的是 `ScalableFigure` 提供的依赖倒置与坐标根语义，不是 Java
接口继承形式。Novadraw 将该能力拆为：

| 关注点 | Draw2D | Novadraw |
|---|---|---|
| scale 状态 | 具体 Scalable Pane 的字段 | 可克隆共享 `ScaleModel` |
| 只读能力发现 | `instanceof ScalableFigure` / 接口调用 | `Figure::scale_model()` accessor |
| 可写入口 | `ScalableFigure.setScale()` | `Runtime::scalable(id)` 返回受控 mutation facade |
| 副作用 | Pane 内 `fireMoved/revalidate/repaint` | Runtime 统一校验、更新、失效、damage 与通知 |
| zoom 策略 | `ZoomManager` | `ZoomManager` |
| 子树投影 | `IScalablePane` 覆盖绘制、坐标与测量 | 共享 `ScaleModel` 驱动 child transform 与 measurement |

这种迁移允许删除仅靠具体类型 downcast 才能工作的 `ScalableFigure` 伪扩展 trait，
但不能删除其承载的语义边界。精简后的不变量是：

1. scale 只有一份权威状态；
2. 外部 Figure 类型可通过 accessor 提供相同 capability；
3. 运行期 mutation 只能经过 Runtime facade；
4. 绘制、坐标转换、测量、damage 和 scroll range 读取同一 scale；
5. `ZoomManager` 不直接拥有第二份 Figure scale；
6. Viewer 生命周期内只保留一个绑定 root scalable/viewport 的 `ZoomManager`。

换言之，精简目标不是把缩放退化成一个绘制 transform，而是将“状态模型、能力发现、
副作用入口、策略控制”从 Java 宽对象协议拆成可验证的 Rust 边界。

实现入口：

- `novadraw/src/container/scalable.rs`
- `novadraw/src/container/viewport.rs`
- `novadraw/src/container/zoom.rs`

## 7. 约束

- `set_scale` 只改变 scale、失效和重绘，不直接累计缩放旧 bounds。
- scaled preferred size 永远从未缩放 preferred size 推导。
- Viewport 是 scroll SSOT，Scalable pane 是 scale SSOT。
- ZoomManager 是 scale、validation 和 view location 的唯一协调入口。
- Editor Viewer 长期拥有 ZoomManager；单次缩放不得临时重建 manager。
- Viewport 不提供 `zoom`、`zoom_at` 或 `zoom_to_fit`，避免第二个 zoom SSOT。
- 普通 scalable 内容小于 viewport 时左上对齐，不添加隐式居中特例。
- paint、hit-test、事件点、damage 和 scroll range 必须共享同一 ChildTransform。
- 负坐标和四向 extent 属于 Freeform 能力，需显式实现。

源码基线：eclipse/gef-classic commit `4463d9d0c`（2026-01-01）。
