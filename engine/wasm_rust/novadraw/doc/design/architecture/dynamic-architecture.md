# 理想架构 - 动态结构

类型：`normative-design`

本文档描述 Novadraw 理想架构的动态结构，包括事件分发流程、更新流程、数据流。

## 1. 事件分发流程

### 1.1 完整事件分发链路

```d2
direction: down

# 用户输入
UserInput: {
  shape: hexagon
  label: "用户输入\n(winit 事件)"
}

# 平台输入适配
PlatformInputAdapter: {
  shape: rect
  label: "apps 输入适配\nphysical / scale_factor"
}

# 引擎分发上下文
SceneDispatchContext: {
  shape: rect
  label: "SceneDispatchContext\n(引擎层)"
}

SceneDispatchContext.step1: "1. find_mouse_event_target_at(entry_x, entry_y)"
SceneDispatchContext.step2: "2. MouseEvent::new(entry point)"
SceneDispatchContext.step3: "3. translate_to_relative(target_id)"
SceneDispatchContext.step4: "4. dispatch_to_target(target_id, event)"

# Figure 回调上下文
SceneNovadrawContext: {
  shape: rect
  label: "SceneNovadrawContext\n(引擎层)"
}

SceneNovadrawContext.step1: "target_id()"
SceneNovadrawContext.step2: "repaint() / invalidate()"

# FigureGraph
FigureGraph: {
  shape: rect
  label: "FigureGraph"
}

FigureGraph.step1: "dispatch_to_target(target, event, ctx)"
FigureGraph.step2: "block(target).handle_event()"
FigureGraph.step3: "返回 target handler 的 consumed 状态"

# FigureBlock
FigureBlock: {
  shape: rect
  label: "FigureBlock"
}

FigureBlock.step1: "handle_event(event, ctx)"
FigureBlock.step2: "figure.on_mouse_pressed(event, ctx)"

# dyn Figure
dynFigure: {
  shape: rect
  label: "dyn Figure"
}

dynFigure.step1: "on_mouse_pressed(event, ctx)"
dynFigure.step2: "ctx.repaint() / invalidate()"

# 连接
UserInput -> PlatformInputAdapter
PlatformInputAdapter -> SceneDispatchContext: "entry-domain point"
SceneDispatchContext.step1 -> SceneDispatchContext.step2
SceneDispatchContext.step2 -> SceneDispatchContext.step3
SceneDispatchContext.step3 -> SceneDispatchContext.step4
SceneDispatchContext -> FigureGraph: "target-domain MouseEvent"
SceneDispatchContext -> SceneNovadrawContext: "create figure ctx"
FigureGraph -> FigureBlock: "handle_event()"
FigureBlock -> dynFigure: "on_mouse_pressed()"

# 返回路径
dynFigure.step2 --> UpdateManager: "add_dirty_region()"
```

### 1.2 事件类型

```d2
direction: left-right

MouseEvent: {
  x: f64
  y: f64
  entry_point: Point
  button: MouseButton
  click_count: u32
  modifiers: Modifiers
}

KeyEvent: {
  key: KeyCode
  modifiers: Modifiers
}

MouseButton: {
  Left
  Middle
  Right
  None
}

Modifiers: {
  ctrl: bool
  shift: bool
  alt: bool
  meta: bool
}

Event: {
  shape: interface
  event_type(): EventType
}

EventType: {
  MousePressed
  MouseReleased
  MouseMoved
  KeyPressed
  KeyReleased
}

MouseEvent --> Event: "实现"
KeyEvent --> Event: "实现"
```

## 2. setCapture 鼠标捕获机制

### 2.1 核心逻辑

```d2
direction: down

# 鼠标按下
MouseDown: {
  shape: hexagon
  label: "MouseDown"
}

FindTarget: {
  shape: rect
  label: "findFigureAt(x, y)"
}

SetCapture: {
  shape: rect
  label: "若事件 consumed\nsetCapture(target)"
}

HandleMouseDown: {
  shape: rect
  label: "handleMousePressed()"
}

# 鼠标移动
MouseMove: {
  shape: hexagon
  label: "MouseMove"
}

CapturedCheck: {
  shape: diamond
  label: "captured != null?"
}

SendToCaptured: {
  shape: rect
  label: "发给 captured Figure"
}

SendToTarget: {
  shape: rect
  label: "正常 findFigureAt"
}

# 鼠标释放
MouseUp: {
  shape: hexagon
  label: "MouseUp"
}

ReleaseCapture: {
  shape: rect
  label: "releaseCapture()"
}

# 流程
MouseDown -> FindTarget
FindTarget -> HandleMouseDown
HandleMouseDown -> SetCapture: "consumed"
MouseMove -> CapturedCheck
CapturedCheck -> SendToCaptured: "是"
CapturedCheck -> SendToTarget: "否"
MouseUp -> SendToCaptured
SendToCaptured -> ReleaseCapture
```

### 2.2 Novadraw 实现

```d2
direction: left-right

BasicEventDispatcher.dispatch_mouse_pressed: {
  "self.receive(ctx, entry_x, entry_y)"
  "let event = MouseEvent::new(Pressed, entry_x, entry_y, button)"
  "ctx.dispatch_to_target(target_id, &event)"
}

BasicEventDispatcher.dispatch_mouse_moved: {
  shape: code_block
  label: "dispatch_mouse_moved"
}

dispatch_mouse_moved.code: |
  if let Some(captured_id) = ctx.captured() {
      ctx.dispatch_to_target(Some(captured_id), &event)
  } else {
      let target_id = ctx.mouse_target()
      ctx.dispatch_to_target(target_id, &event)
  }

BasicEventDispatcher.dispatch_mouse_released: {
  shape: code_block
  label: "dispatch_mouse_released"
}

dispatch_mouse_released.code: |
  if let Some(captured_id) = ctx.captured() {
      ctx.dispatch_to_target(Some(captured_id), &event)
      ctx.set_captured(None)
  } else {
      let target_id = ctx.mouse_target()
      ctx.dispatch_to_target(target_id, &event)
  }
```

## 3. 两阶段更新流程

```d2
direction: down

# 用户触发
UserTrigger: {
  shape: hexagon
  label: "用户触发\n(invalidate / repaint)"
}

# Phase 1: Validation
Phase1: {
  shape: rect
  label: "Phase 1: Validation"

  step1: "遍历 invalid_figures 列表"
  step2: "调用 fig.validate()"
  step3: "执行 layout()"
  step4: "递归验证子节点"
}

# Phase 2: Damage Repair
Phase2: {
  shape: rect
  label: "Phase 2: Damage Repair"

  step1: "脏区传播到父链"
  step2: "与父节点 bounds 取交集"
  step3: "root.paint(gc)"
}

# 渲染
Render: {
  shape: rect
  label: "RenderBackend\n(Vello/WebGPU)"
}

# 连接
UserTrigger -> Phase1
Phase1.step1 -> Phase1.step2
Phase1.step2 -> Phase1.step3
Phase1.step3 -> Phase1.step4
Phase1 -> Phase2
Phase2.step1 -> Phase2.step2
Phase2.step2 -> Phase2.step3
Phase2 -> Render
```

## 4. UpdateManager Trait

```d2
direction: left-right

UpdateManager: {
  shape: interface
  tooltip: "两阶段更新调度"

  + add_dirty_region(block_id, rect)
  + add_invalid_figure(block_id)
  + perform_update(graph, canvas)
  + perform_validation(graph)
  + is_updating(): bool
}

UpdateListener: {
  shape: interface
  tooltip: "更新监听器"

  + notify_painting(damage, regions)
  + notify_validating()
}

UpdateManager -> UpdateListener: "通知"
```

## 5. 数据流关系

```d2
direction: down

# 用户输入
UserInput: {
  shape: hexagon
  label: "用户输入\n(鼠标/键盘/窗口事件)"
}

# 事件处理
EventHandling: {
  shape: rect
  label: "WinitEventDispatcher\n→ FigureGraph\n→ FigureBlock\n→ dyn Figure"
}

# 触发更新
TriggerUpdate: {
  shape: rect
  label: "ctx.repaint()\nctx.invalidate()"
}

# 更新管理
UpdateManagement: {
  shape: rect
  label: "UpdateManager"

  Phase1: "Phase 1: Validation\nperform_validation()"
  Phase2: "Phase 2: Damage Repair\nrepair_damage()"
}

# 渲染
SceneHost: {
  shape: rect
  label: "SceneHost\nexecute_update()"
}

RenderBackend: {
  shape: rect
  label: "RenderBackend\n(Vello/WebGPU)"
}

NdCanvas: {
  shape: rect
  label: "NdCanvas\n(渲染命令)"
}

# 连接
UserInput -> EventHandling
EventHandling -> TriggerUpdate
TriggerUpdate -> UpdateManagement.Phase1
UpdateManagement.Phase1 -> UpdateManagement.Phase2
UpdateManagement.Phase2 -> SceneHost
SceneHost -> RenderBackend
RenderBackend -> NdCanvas
```

## 6. 单目标分发流程

```d2
direction: down

# 目标分发
DispatchStart: {
  shape: hexagon
  label: "Target Figure\n收到事件"
}

HandleSelf: {
  shape: rect
  label: "block.handle_event()\nfigure.on_mouse_pressed()"
}

ReturnConsumed: {
  shape: rect
  label: "返回 consumed\n不自动投递给 parent"
}

# 连接
DispatchStart -> HandleSelf
HandleSelf -> ReturnConsumed
```

这与 Draw2D 的 `SWTEventDispatcher -> target.handleXxx()` 一致：普通输入事件直达
命中 target，没有 DOM 式通用冒泡。需要祖先 fallback 的特定行为必须作为独立协议
显式实现。

## 7. 与 g2 的关键差异

| 阶段 | g2 | Novadraw |
|------|-----|----------|
| find_figure_at | EventDispatcher.receive() 内部 | EventDispatcher 通过 **NovadrawContext** 调用 |
| mouse_target 设置 | EventDispatcher 内部 | FigureGraph **内部** |
| 事件路由 | EventDispatcher 直达 target | FigureGraph.dispatch_to_target()，无通用冒泡 |
| 上下文传递 | 隐式 this | **显式 NovadrawContext** |
| setCapture | 默认 SWTEventDispatcher 直接持有 | FigureGraph.captured 字段 |

## 8. NovadrawContext 接口

```d2
direction: left-right

NovadrawContext: {
  shape: interface
  tooltip: "Figure 操作的执行环境"

  + graph(): "Arc<Mutex<FigureGraph>>"
  + update_manager(): "Arc<Mutex<dyn UpdateManager>>"
  + current_block_id(): BlockId
  + route_event(target_id, event)
  + set_bounds(x, y, w, h)
  + translate(dx, dy)
  + repaint()
  + invalidate()
}

FigureGraph: {
  shape: interface
  tooltip: "树结构 + 交互状态"

  + find_figure_at(x, y): Option<BlockId>
  + route_event(target, event, ctx): bool
  + set_mouse_target(id)
  + set_captured(id)
  + set_focus_owner(id)
}

UpdateManager: {
  shape: interface
  tooltip: "更新调度"

  + add_dirty_region(block_id, rect)
  + add_invalid_figure(block_id)
}

# 关系
NovadrawContext -> FigureGraph: "访问"
NovadrawContext -> UpdateManager: "访问"
```

## 9. 坐标转换流程

```d2
direction: left-right

# 坐标类型
LocalCoord: {
  shape: hexagon
  label: "本地坐标\n(相对于坐标根)"
}

ParentCoord: {
  shape: hexagon
  label: "父节点坐标"
}

AbsoluteCoord: {
  shape: hexagon
  label: "绝对值\n(最近坐标根)"
}

# 转换方法
TranslateToParent: {
  shape: rect
  label: "translate_to_parent()"
}

TranslateToAbsolute: {
  shape: rect
  label: "translate_to_absolute()"
}

TranslateToRelative: {
  shape: rect
  label: "translate_to_relative()"
}

# 转换
LocalCoord -> TranslateToParent: "累加父节点 insets"
TranslateToParent -> ParentCoord

ParentCoord -> TranslateToAbsolute: "累加所有祖先 bounds"
TranslateToAbsolute -> AbsoluteCoord

AbsoluteCoord -> TranslateToRelative: "减去所有坐标根 bounds"
TranslateToRelative -> LocalCoord

# 关键点
CoordinateRoot: {
  shape: rect
  label: "坐标根\n(use_local_coordinates=true)"
  style: dashed
}
```

## 10. 扩展点说明

### 10.1 捕获阶段扩展

当前只支持单 target 分发，不包含 DOM 风格的捕获或冒泡阶段。Scroll/Zoom 等需要
查看祖先链的能力使用独立 typed fallback，不能借此引入通用传播。

未来若产品确实需要三阶段传播，必须通过新的 ADR 和事件契约显式引入，不能在
`route_event()` 内部静默改变现有 Figure 回调次数：

```d2
direction: left-right

ExtensionPoint: {
  shape: rect
  label: "新的传播协议"

  current: "单 target"
  optional_future: "捕获 → target → 冒泡"
}

FigureTrait: {
  shape: interface
  label: "Figure trait"
}

on_capturing: {
  shape: method
  label: "on_capturing(event, ctx): bool"
  style: dashed
}

FigureTrait -> on_capturing: "可选方法"
```

### 10.2 布局扩展

```d2
direction: left-right

LayoutManager: {
  shape: interface

  + layout(container_id, graph)
  + get_preferred_size(container_id, graph, w, h)
}

FlowLayout: {
  tooltip: "流式布局"
}

BorderLayout: {
  tooltip: "边框布局"
}

FillLayout: {
  tooltip: "填充布局"
}

CustomLayout: {
  tooltip: "用户自定义"
  style: dashed
}

LayoutManager -> FlowLayout: "实现"
LayoutManager -> BorderLayout: "实现"
LayoutManager -> FillLayout: "实现"
LayoutManager -> CustomLayout: "实现"
```
