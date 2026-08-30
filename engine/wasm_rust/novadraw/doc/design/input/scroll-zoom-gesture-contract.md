# Scroll / Zoom 输入手势契约

类型：`normative-design`

## 1. 目标与边界

Novadraw 将鼠标滚轮、触控板双指滚动和双指缩放归一为平台无关的输入事件。
引擎 crate 不依赖 `winit`、AppKit、Win32、Wayland 或 Web DOM 类型。

```text
平台事件 -> PlatformInputAdapter -> WheelEvent / ZoomEvent
         -> EventDispatcher -> Figure callback -> 默认 Scroll / Zoom controller
```

- apps / 宿主层只负责单位、DPI、阶段和修饰键适配。
- `novadraw-scene` 负责命中、target 固定、事件点降域，以及 Scroll/Zoom
  专用的祖先 fallback 和默认行为；这不是通用输入事件冒泡。
- Viewport 仍是 scroll origin 的唯一真源。
- Scalable pane 仍是 scale 的唯一真源。
- 鼠标 pointer capture 与 gesture session 相互独立。

## 2. 统一事件

`WheelEvent` 保留二维 delta，并新增：

- `ScrollDeltaKind::Lines`：离散滚轮刻度，由 ScrollPane 的 line step 转为逻辑距离。
- `ScrollDeltaKind::LogicalPixels`：平台适配器完成 DPI 转换后的连续逻辑像素。
- `GesturePhase::{Begin, Update, End, Cancel, Impulse}`。
- `GestureSessionId`：宿主进程内有效的连续手势标识。
- `KeyModifiers`：支持策略层实现 `Ctrl/Cmd + wheel` 等绑定。

`ZoomEvent` 使用每次更新的乘法 `scale_factor`。平台适配器负责把原生
magnification delta 转成有限且大于零的 factor。

## 3. 分发规则

1. `Begin` 在入口锚点执行 hit-test，并为 session 固定 target。
2. `Update`、`End`、`Cancel` 继续投递给固定 target，不受光标移动影响。
3. `Impulse` 每次独立 hit-test，不创建 session。
4. 事件只回调一次固定 target；未消费时，Scroll/Zoom controller 沿 Figure
   祖先链查找最近的对应容器并执行默认行为。中间祖先不得依次收到 Figure 回调。
   这是手势协议的显式 fallback，不是通用事件冒泡。
5. `End` 和 `Cancel` 在投递后清理 session。
6. 无对应 `Begin` 的 `Update` 按当前锚点重新命中，以容忍平台缺失阶段。

普通鼠标按压拖动继续使用 pointer capture，不转换成滚动手势。macOS 触控板双指
移动由 winit `MouseWheel::PixelDelta` 适配为连续 `WheelEvent`。

### 3.1 为什么禁止祖先逐级回调

Novadraw 普通输入采用显式 target，而不是 DOM 风格冒泡。Scroll/Zoom 的容器默认
行为确实需要查看祖先链，但“查找容器”和“向所有祖先分发事件”是两种不同协议：

- target Figure 可以通过返回 `true` 覆盖默认行为；
- 未消费时，引擎只选择最近的 ScrollPane 或 Scalable pane；
- 普通中间容器不会因为层级位置获得隐式输入处理权；
- fallback 的类型选择、坐标转换和事务边界集中在引擎上下文中。

因此 `EventDispatcher` 不得用循环调用 `dispatch_to_target(parent)` 模拟 fallback。
实现应提供专用的 `apply_scroll_fallback` / `apply_zoom_fallback` 上下文操作。

## 4. 默认 Scroll 与 Zoom

- 未消费的 Scroll 由最近的 ScrollPane 处理。
- Lines 使用 ScrollPane line step；LogicalPixels 直接使用逻辑距离。
- 未消费的 Zoom 由最近的 ScalableLayeredPane 对应 `ZoomManager` 处理，触控板
  使用 `MouseLocationZoomScrollPolicy`。
- 若 scalable pane 位于 Viewport 内，scale、RangeModel content extent 与 view location
  必须在同一更新事务修改，不能等待下一帧 validation 才更新滚动范围。
- RangeModel 范围变化必须同时重绘 viewport 与其 ScrollPane，使 scrollbar thumb
  在同一帧反映新的 extent/maximum。
- Scalable pane 的未缩放 preferred size 是缩放范围计算的基准。ViewportLayout
  为填满 viewport 分配的 bounds 不得反向覆盖该值，否则缩小后再次放大会累积范围误差。
- 对齐 Draw2D `ViewportLayout`：当 scaled preferred size 小于 viewport 时，
  contents bounds 扩展到 viewport，但子树保持 client-area 左上对齐；越界由
  viewport clip 阻止，不额外引入居中坐标特例。
- `ZoomManager` 严格执行 `calcNewViewLocation → setScale → viewport.validate →
  setViewLocation`；默认 zoom levels 为 `0.5, 0.75, 1, 1.5, 2, 2.5, 3, 4`。

锚点缩放保持入口锚点下的 content point 不变：

```text
content_anchor = inverse(old_scalable_transform, old_view_location, anchor)
new_view_location = project(new_scalable_transform, content_anchor) - viewport_anchor
```

实现必须复用 Figure 父链 `translateToParent/translateFromParent` 协议，不能建立
独立的全局坐标换算。

## 5. 平台映射

| 平台输入 | 引擎事件 |
|---|---|
| winit `LineDelta` | Lines + Impulse |
| winit `PixelDelta` | LogicalPixels + TouchPhase 对应阶段 |
| winit macOS `PinchGesture` | ZoomEvent |
| 鼠标按钮 + CursorMoved | MouseEvent + pointer capture |
| Web wheel / pointer gesture | 由 Web adapter 映射到同一事件 |

winit `DeviceId` 不进入引擎 API。适配器为每个原生连续流分配
`GestureSessionId`，因此不同鼠标和触控板输入不会共享全局手势状态。

## 6. 失败模式

- 非有限坐标、delta 或 scale factor：忽略事件，不修改状态。
- 非正 scale factor：忽略事件。
- target 在手势中被删除：停止投递并清理 session。
- `Cancel`：保留已提交的增量，不回滚，只清理 session。
- 缩放或滚动到达模型边界：保持模型 clamp 语义。

## 7. 验证门禁

- Lines 与 LogicalPixels 使用不同的距离语义。
- 连续手势 target 在 Begin 后固定，End/Cancel 后释放。
- Wheel、Zoom 的 target callback 点位于 target Figure 坐标域。
- 锚点缩放前后同一 content point 的入口坐标不变。
- 缩放返回后、下一帧 validation 前，RangeModel 已覆盖缩放后的完整内容。
- 连续滚动和滚动条在缩放后仍能到达画布四边。
- 缩小到 viewport 以下再放大时，范围必须仍由 unscaled preferred size × scale
  唯一确定。
- Figure 自身、显式 preferred override 和 LayoutManager 三种尺寸来源都必须经过
  同一 hint 反缩放与 preferred/minimum 投影协议。
- 鼠标 pointer capture 不阻塞独立 gesture session。
- winit adapter 不向引擎暴露任何平台事件类型。
