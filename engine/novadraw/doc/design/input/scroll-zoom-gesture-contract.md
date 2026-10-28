# Scroll / Zoom 输入手势契约

类型：`normative-design`

## 1. 边界

Novadraw 将鼠标滚轮、触控板滚动和 pinch zoom 归一为平台无关事件：

```text
native event
→ PlatformInputAdapter
→ ScrollEvent / ZoomEvent
→ Runtime EventDispatcher
→ target callback
→ typed fallback
```

- PlatformInputAdapter 负责平台单位、DPI、phase 和 modifier；
- EventDispatcher 负责 target、事件点和回调；
- InteractionState 负责 gesture session；
- FigureTree 只提供 hit-test、ancestor 和坐标查询；
- RangeModel 是 scroll origin 的唯一真源；
- ScalablePane 是 scale 的唯一真源。

## 2. 事件模型

```rust
pub enum GesturePhase {
    Begin,
    Update,
    End,
    Cancel,
    Impulse,
}

pub enum ScrollDelta {
    Lines { x: f64, y: f64 },
    LogicalPixels { x: f64, y: f64 },
}
```

ScrollEvent 和 ZoomEvent 还包含：

- logical-surface anchor；
- target-local anchor；
- modifiers；
- timestamp；
- device-neutral `GestureSessionId`。

Zoom 使用每次更新的乘法 factor。factor 必须有限且大于零。

## 3. Session

`InteractionState.gestures` 保存连续手势：

```text
Begin
→ hit-test
→ pin target and typed fallback controller
→ Update*
→ End or Cancel
→ remove session
```

规则：

1. Begin 固定 target；
2. Update、End、Cancel 不因 pointer 移动重新命中；
3. Impulse 每次独立命中，不创建持续 session；
4. 缺失 Begin 的 Update 可以建立恢复性 session；
5. target 删除后发送可选 Cancel，并清理 session；
6. pointer capture 与 gesture session 独立。

同时存在多个设备或触点时，每个 session 独立，不共享单一全局 gesture target。

## 4. 分发

每次事件：

```text
resolve pinned target
→ convert anchor to target local domain
→ call target once
→ apply callback effects
→ if unhandled, invoke pinned typed controller
```

普通 ancestor 不依次收到 Figure 回调。typed controller 查找属于 Scroll/Zoom
协议，不是 DOM 冒泡。

Begin 时同时固定 fallback controller，可以防止手势过程中树变化导致控制权在多个
嵌套 ScrollPane 之间跳动。若 controller 被删除，则重新选择或结束 session，具体
策略必须显式记录。

## 5. Scroll

- Lines 由目标 ScrollPane 的 line metrics 转换为 logical distance；
- LogicalPixels 直接使用；
- delta 保留二维分量；
- clamp 由 RangeModel 统一完成；
- 到达边界时可以把未消费余量交给显式 nested-scroll policy；
- nested scroll 是 typed policy，不是 Figure 事件冒泡。

Scroll transaction：

```text
old origin
→ apply delta and clamp
→ new origin
→ invalidate child transform
→ damage viewport
→ update scrollbar models
```

## 6. Zoom

ZoomManager 协调 scale 和 viewport origin，但不拥有第二份状态：

```text
capture content point under anchor
→ calculate new scale
→ update scalable content
→ validate content extent
→ calculate anchored viewport origin
→ clamp RangeModel
→ commit as one transaction
```

锚点不变量：

```text
project(old_transform, content_anchor) == anchor
project(new_transform, content_anchor) == anchor
```

缩放、content extent、view location 和 scrollbar 状态必须在同一 Runtime transaction
中一致，不能等待下一帧再修正。

## 7. 平台映射

| 平台输入 | 规范化事件 |
|---|---|
| 离散 mouse wheel | Lines + Impulse |
| 连续 trackpad scroll | LogicalPixels + phased session |
| native pinch | ZoomEvent + phased session |
| Web WheelEvent | adapter 根据 deltaMode 转换 |
| Web Pointer gesture | adapter 聚合为独立 gesture session |

平台 device handle 不进入引擎事件。需要区分设备时，由 adapter 分配稳定的运行时
session/pointer ID。

## 8. 2.5D

Scroll 和 Zoom 默认作用于二维 content transform。若 target 位于 projective visual
effect 下：

- logical-surface anchor 先按该 effect 的命中策略逆投影；
- 不可逆时不启动 gesture；
- 2.5D visual zoom 不得与内容 scale 混为同一个状态；
- 真正 Scene3D 的 camera zoom 使用独立 controller 和事件解释。

## 9. 失败

- NaN、Infinity、非正 zoom factor：拒绝；
- session target 不存在：Cancel 并清理；
- controller 不存在：仅保留 target callback 结果；
- RangeModel 无可滚动范围：返回 unhandled remainder；
- transaction 中任一步失败：不提交部分 scale/origin 状态。

## 10. 验证门禁

1. Begin 后 target 和 controller 稳定；
2. target callback 每次最多一次；
3. typed fallback 不触发普通 ancestor callback；
4. Lines 与 LogicalPixels 不混用；
5. pointer capture 不覆盖 gesture session；
6. anchor zoom 前后保持同一 content point；
7. scale、extent、origin 和 scrollbar 原子更新；
8. nested scroll remainder 遵循显式 policy；
9. target/controller 删除正确 Cancel；
10. 平台类型不泄漏到 Runtime。
