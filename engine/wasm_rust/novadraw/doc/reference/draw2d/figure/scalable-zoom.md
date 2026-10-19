# Draw2D Scalable Figure 与 Zoom

类型：`reference-analysis`

源码基线：GEF Classic `4463d9d0c`（2026-01-01）。
范围：Draw2D ScalableFigure、IScalablePane/Helper、ScalableLayeredPane、
ScalableFreeformLayeredPane、Viewport/ViewportLayout/FreeformViewport，
以及 Draw2D zoom 包与 GEF ScalableRootEditPart。

## 状态、投影和协调

| 对象 | 责任 |
|---|---|
| ScalableFigure | `getScale/setScale` 窄执行契约 |
| Scalable Pane / IScalablePaneHelper | 把同一 scale 投影到测量、绘制、坐标和失效 |
| Viewport | client clip、contents bounds、RangeModel、view location |
| AbstractZoomManager | zoom levels、fit、监听及缩放前后位置协调 |

scale 通常作用在粗粒度坐标根；children 使用未缩放逻辑坐标。
`setScale` 不按比例改写已有 bounds。普通 ScalableLayeredPane 设置 scale，
发 moved、revalidate、repaint；bounds 仍由 ViewportLayout 分配。

GEF ScalableRootEditPart 把 Grid、Printable 和 SCALED_FEEDBACK 放在 SCALABLE_LAYERS，
HANDLE、普通 FEEDBACK 和 GUIDE 位于其外，从而区分内容域与屏幕装饰域。

## IScalablePaneHelper

| 协议 | 源码行为 |
|---|---|
| preferred/minimum size | hint / scale 后查询父实现；扣 Insets，缩放内容，再加回 Insets |
| client area | 普通 client area 乘 1 / scale |
| paint | children 绘制前 Graphics.scale |
| child → parent | performScale(scale) |
| parent → child | performScale(1 / scale) |

同一 scale 约束绘制、测量和输入坐标；仅缩放像素不能满足该协议。

## Viewport 范围

普通 ViewportLayout：

```text
location = clientArea.location - viewLocation
size.width = max(clientArea.width, contents.scaledPreferredWidth)
size.height = max(clientArea.height, contents.scaledPreferredHeight)
contents.setBounds(location, size)
```

Viewport.validate 随后 readjustScrollBars，按 client extent 与 contents bounds
更新 horizontal/vertical RangeModel。内容小于 viewport 时扩展 contents bounds，
默认子树仍左上对齐。
FreeformViewport 使用 freeform extent 与 client area 的 union，允许负坐标；
不能将其范围规则隐式塞进普通 scalable pane。

## Zoom 的发生顺序

`AbstractZoomManager.primSetZoom`：

```text
scrollPolicy.calcNewViewLocation(oldZoom, newZoom)
→ pane.setScale
→ viewport.validate
→ viewport.setViewLocation
```

新位置先计算，range 更新后再设置 view location。
zoomIn/out 选择有序相邻等级；fit width/height/all 使用 viewport client area
及未缩放 preferred size。

默认策略保持中心：

```text
newLocation = oldLocation + center * (newZoom / oldZoom - 1)
```

鼠标策略保持指针下内容：

```text
(mouse + oldLocation) / oldZoom = (mouse + newLocation) / newZoom
```

## Novadraw 采用关系

项目的 ScaleCapability/typed registry、FigureMut、范围单位及所有权由
[Layer/Freeform 规范](../../../design/architecture/layer-and-freeform.md)、
[输入协议](../../../design/input/scroll-zoom-gesture-contract.md)、
[Capability 模型](../../../design/architecture/figure-capability-model.md)和
[parity](../../../parity/draw2d/api-coverage.md)维护。
本页不再保留旧 Figure accessor、Bounded 或 Runtime::scalable 迁移表。
Editor 的手势、反馈和 auto-expose 见
[Viewport 专题](../../../design/editor/g5-viewport-autoexpose.md)。
