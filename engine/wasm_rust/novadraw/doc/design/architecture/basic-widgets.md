# Basic Widgets Contract

类型：`architecture`

## 范围

M10.4 提供最小 button-like Figure 能力：

- `ClickableFigure`
- `ButtonFigure`
- `ToggleFigure`
- pressed、rollover、selected、focus、disabled visual
- pointer、Enter、Space 激活
- typed action 与 property change notification

Repeat firing、ButtonGroup/radio 互斥、Slider 和完整 widget toolkit 不在本阶段。

`api_semantics`：`widgets.basic`

## 状态所有权

### Runtime InteractionState

瞬态输入状态仍由每个 Runtime 的 `InteractionState` 唯一拥有：

- pointer capture
- pointer pressed
- keyboard pressed
- hover source
- focus owner

pointer 与 keyboard pressed 必须保留来源，不能压缩为单一布尔值。pointer pressed
只有在指针仍位于控件内时产生 pressed visual；keyboard pressed 不依赖指针位置。

### ClickableModel

控件模型只持有持久语义：

- `ClickableKind`
- `selected`
- `rollover_enabled`
- `action_revision`

`selected` 不得写入编辑器 selection，也不得写入 `InteractionState`。

### ClickableVisualState

Figure 保存 Runtime 从交互状态派生的只读绘制快照：

- `hovered`
- `pressed`
- `focused`
- `enabled`

该快照不是第二套状态机。每次顶层 dispatch 或 enablement 变化后由 Runtime
重新派生；变化只触发 repaint。

## 激活因果链

### Pointer

```text
left press inside
  -> Figure handles event
  -> EventDispatcher assigns capture + focus + pointer pressed

drag outside
  -> capture remains
  -> pointer pressed remains
  -> derived pressed visual becomes false

drag inside
  -> derived pressed visual becomes true

left release
  -> only captured Figure receives release
  -> release-inside enqueues activation
  -> release-outside cancels activation
  -> dispatcher clears pointer pressed + capture
```

### Keyboard

```text
Enter/Space press on focus owner
  -> keyboard pressed = true

matching Enter/Space release
  -> activation
  -> keyboard pressed = false

focus lost
  -> cancel remaining pressed state
```

孤立 key release 不触发 action。disabled Figure 不参与 pointer target、不能保留 focus，
并拒绝 programmatic `do_click`。

## Notification

Action 是一次发生事实，不是属性，因此使用独立 typed event：

```rust
pub struct ActionEvent {
    pub figure_id: FigureId,
    pub revision: u64,
}
```

模型属性变化继续使用 `PropertyChangeEvent`。

Toggle 激活顺序固定为：

```text
selected property change
  -> ActionEvent
```

两者进入同一个 `NotificationQueue`，在稳定事务边界 flush，禁止跨队列重排。

## Figure 组合

- `ClickableFigure` 是无固定外观的单 child 交互容器。
- `ButtonFigure` 组合 `ClickableModel + LabelFigure`，提供 push button visual。
- `ToggleFigure` 组合 `ClickableModel + LabelFigure`，提供持久 selected visual。

Button/Toggle 通过 Figure label capability 暴露内部 Label，因此继续复用 M10.2 的：

- 文本 shaping 与 layout cache
- icon resource snapshot
- alignment、gap、ellipsis
- Runtime typed label mutation

递归渲染主循环不感知 widget 类型，也不包含 widget 特例。

## Draw2D 对应与合理差异

保留：

- press/armed/release-inside action 语义
- drag-out suspend 与 drag-back resume
- Enter/Space 激活
- focus、rollover、pressed、selected visual
- Toggle 在 action 前翻转 selected

差异：

- 不复制 `Clickable -> EventHandler -> ButtonModel -> ModelObserver` 的 Java 对象链；
  Rust 使用 Figure capability + Runtime effect transaction。
- pressed/hover/focus 不复制到 ButtonModel，继续由 `InteractionState` 统一拥有。
- `ActionEvent` 与属性 change 进入 M7 typed notification queue。
- M10.4 不提供 Draw2D repeat timer 和 ButtonGroup。

## 扩展点

- 新 button-like Figure 实现 `ClickableBehavior` 并暴露自己的 paint。
- 新内容类型可作为 `ClickableFigure` 的单 child，或像 Button 一样组合现有 Figure。
- Repeat firing 未来应由 Runtime scheduler 驱动，不能在 Figure 内创建平台线程或 Timer。
- ButtonGroup 未来应是独立 selection model，不能复用 editor selection。

## 错误模式

- 未知 Figure：`WidgetError::UnknownFigure`
- Figure 不支持 clickable capability：`WidgetError::WrongCapability`
- disabled `do_click`：返回 `Ok(false)`，不产生 notification
- 重复 selected/rollover 设置：返回 `Ok(false)`，不产生 notification

## 验证

- `novadraw/tests/m10_widget_contract.rs`
- `examples/native/widgets-app`
  - `Button_States`
  - `Toggle_States`
  - `Interactive_Widgets`
