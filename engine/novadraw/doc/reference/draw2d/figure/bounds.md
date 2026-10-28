# Draw2D Figure Bounds

类型：`reference-analysis`

源码：GEF Classic `org.eclipse.draw2d.Figure`、`IFigure`，
基线 `4463d9d0c`（2026-01-01）。本文负责 bounds 的存储和更新；
转换公式见 [树与坐标](tree-coordinates.md)，Insets/client area 见
[盒模型](box-model.md)，命中下降见 [树操作](tree-operations.md)。

## bounds 的坐标域

`Figure.bounds` 是 `(x, y, width, height)` 矩形，位于 parent 为 children 提供的坐标域。
普通 Figure 与 children 共享所属域；local-coordinate Figure 为 children 建立
client-area 局部域。当前 Figure 的 `useLocalCoordinates()` 不改变它自己的 bounds
存储域。

“相对于最近坐标根”可用于解释未跨边界的坐标值，不能把 bounds 一律称为画布绝对坐标。
Viewport、scalable pane 等覆盖公开坐标协议，跨边界求画布位置必须沿父链转换，
不能仅加减一个祖先的 bounds。

## 平移传播

`Figure.primTranslate` 先修改自身 bounds：

- 默认模式继续调用每个 child 的 `translate(dx, dy)`；
- local-coordinate 模式发送 `fireCoordinateSystemChanged()`，不改写后代 bounds。

`isCoordinateSystem()` 的基础实现返回 `useLocalCoordinates()`；scalable pane 可覆盖。
Viewport 返回 `useGraphicsTranslate() || super.isCoordinateSystem()`。
`FreeformLayer` 和 `FreeformLayeredPane` 本身不覆盖该方法，类型名不足以证明坐标根身份。

## setBounds 的发生顺序

`Figure.setBounds(Rectangle)`：

1. 比较位置和尺寸是否变化；
2. 可见且发生变化时 `erase()` 旧区域；
3. 位置变化时调用 `primTranslate`；
4. 写入 width/height；
5. 尺寸变化时 invalidate；
6. 位置或尺寸变化时发一次 `fireFigureMoved()`，再 repaint 新区域。

位置和尺寸同时改变仍使用一次 FigureMoved。完全相同的 bounds 不执行上述变化分支。
Draw2D 没有独立的 `figureResized` 回调。

## bounds 的消费者

| 消费者 | 语义 |
|---|---|
| `containsPoint` | 基础实现检查输入域中的 bounds；子类可提供更精确形状 |
| `paintFigure` | opaque 时填充 bounds |
| `paintClientArea` | 根据 local-coordinate 模式转换 children 绘制域 |
| `paintChildren` | 默认以 child bounds 裁剪；可由 IClippingStrategy 提供多个区域 |
| `LayoutManager.layout` | 把 child bounds 写入容器提供的 children 坐标域 |
| `erase/repaint` | 保持旧位置清除与新位置绘制闭环 |

输入命中下降先通过 `translateFromParent` 转点，再保存转换后的 x/y，检查 client area，
按逆序检查 visible/enabled children；未找到 descendant 时返回 null。
是否返回当前 Figure 由上层 `findMouseEventTargetAt` 及 handler 资格决定，
不能将 descendant 方法改写为 `return this`。

## 相关契约

Draw2D 的坐标与可覆盖行为由 [接口](ifigure-interface.md)及
[Figure 实现](figure-implementation.md)补充。Novadraw 采用 parent-content bounds
及统一边变换，其合理差异见
[坐标规范](../../../design/coordinates/coordinate-system.md)和
[语义账本](../../../parity/draw2d/api-coverage.md)。
