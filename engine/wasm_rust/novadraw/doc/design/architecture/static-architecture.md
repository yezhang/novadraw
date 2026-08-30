# 理想架构 - 静态结构

类型：`normative-design`

本文档描述 Novadraw 理想架构的静态结构，包括组件关系、数据结构定义、Trait 层级。

## 1. 组件总览

```d2
direction: left-right

# 顶层入口
NovadrawSystem: {
  shape: package
  tooltip: "统一入口，platform::create_system() 创建"

  scene: FigureGraph
  update_manager: UpdateManager
  dispatcher: EventDispatcher
  scene_host: SceneHost
}

# 核心组件
FigureGraph: {
  shape: package
  tooltip: "树结构管理 + 交互状态"

  nodes: "SlotMap<BlockId, FigureBlock>"
  root: BlockId
  contents: Option<BlockId>
  mouse_target: Option<BlockId>
  focus_owner: Option<BlockId>
  captured: Option<BlockId>
}

FigureBlock: {
  shape: package
  tooltip: "单节点运行时状态"

  id: BlockId
  parent: Option<BlockId>
  children: Vec<BlockId>
  is_valid: bool
  layout_manager: Option<Arc<dyn LayoutManager>>
  figure: "Box<dyn Figure>"
}

# Trait 定义
dyn_Figure: {
  shape: interface
  tooltip: "渲染接口定义"

  bounds()
  paint_figure()
  validate()
  preferred_size()
}

dyn_LayoutManager: {
  shape: interface
  tooltip: "布局策略接口"

  layout()
  get_preferred_size()
}

dyn_UpdateManager: {
  shape: interface
  tooltip: "两阶段更新调度"

  add_invalid_figure()
  perform_validation()
  repair_damage()
}

dyn_EventDispatcher: {
  shape: interface
  tooltip: "事件分发 trait"

  dispatch_mouse_pressed()
  dispatch_key_pressed()
}

SceneHost: {
  shape: interface
  tooltip: "渲染入口协调"

  execute_update()
  viewport_size()
  request_update()
}

# 关系
NovadrawSystem -> FigureGraph: "持有"
NovadrawSystem -> dyn_UpdateManager: "持有"
NovadrawSystem -> dyn_EventDispatcher: "持有"
NovadrawSystem -> SceneHost: "持有"

FigureGraph -> FigureBlock: "SlotMap 存储"
FigureBlock -> dyn_Figure: "持有"
FigureBlock -> dyn_LayoutManager: "每个容器独立"

dyn_Figure <- dyn_LayoutManager: "依赖"
```

## 2. Trait 层级关系

```d2
direction: down

# Bounded - 基础层
Bounded: {
  shape: interface
  tooltip: "边界 + 坐标系统 + 布局属性"

  + bounds(): Rectangle
  + set_bounds(x, y, w, h)
  + name(): &'static str
  + contains_point(x, y): bool
  + intersects(rect): bool
  + insets(): (f64, f64, f64, f64)
  + use_local_coordinates(): bool
  + client_area(): Rectangle
  + preferred_size(): (f64, f64)
  + minimum_size(): (f64, f64)
  + maximum_size(): (f64, f64)
}

# Figure - 渲染层
Figure: {
  shape: interface
  tooltip: "渲染接口 + 验证"

  + paint_figure(gc)
  + paint_border(gc)
  + get_border(): Option<&dyn Border>
  + validate()
  + on_mouse_pressed(event, ctx): bool
  + on_mouse_released(event, ctx): bool
  + on_mouse_moved(event, ctx): bool
  + on_key_pressed(event, ctx): bool
  + on_key_released(event, ctx): bool
  + on_focus_gained(ctx): bool
  + on_focus_lost(ctx): bool
}

# Shape - 图形层
Shape: {
  shape: interface
  tooltip: "描边 + 填充"

  + fill_shape(gc)
  + outline_shape(gc)
  + stroke_color(): Option<Color>
  + fill_color(): Option<Color>
  + stroke_width(): f64
  + line_cap(): LineCap
  + line_join(): LineJoin
  + alpha(): f64
}

# 继承关系
Bounded -> Figure: "继承"
Figure -> Shape: "继承"

# 实现示例
RectangleFigure: {
  tooltip: "实现示例"
}
EllipseFigure: {
  tooltip: "实现示例"
}
TriangleFigure: {
  tooltip: "实现示例"
}

Shape -> RectangleFigure: "实现"
Shape -> EllipseFigure: "实现"
Shape -> TriangleFigure: "实现"
```

## 3. FigureBlock 数据结构

```d2
direction: left-right

FigureBlock: {
  # 节点身份
  id: BlockId
  uuid: Uuid
  parent: Option<BlockId>
  children: Vec<BlockId>

  # 运行时状态
  is_visible: bool
  is_enabled: bool
  is_selected: bool
  is_valid: bool  # 合并 layout_valid

  # 布局状态
  layout_manager: "Option<Arc<dyn LayoutManager>>"
  constraints: "HashMap<BlockId, Box<dyn Constraint>>"
  preferred_size: "Option<(f64, f64)>"
  minimum_size: "Option<(f64, f64)>"
  maximum_size: "Option<(f64, f64)>"

  # 渲染接口
  figure: "Box<dyn Figure>"
}
```

### 三级回退机制

```d2
direction: down

get_preferred_size: {
  label: "get_preferred_size() 三级回退"

  step1: "FigureBlock.preferred_size 缓存"
  step2: "LayoutManager.get_preferred_size() 计算"
  step3: "Figure.preferred_size() 回退"

  step1 -> step2: "缓存为空"
  step2 -> step3: "LayoutManager 返回 (0,0)"
  step1 -> step3: "无 LayoutManager"
}
```

## 4. FigureGraph 数据结构

```d2
direction: left-right

FigureGraph: {
  # 节点存储
  nodes: "SlotMap<BlockId, FigureBlock>"
  uuid_map: "HashMap<Uuid, BlockId>"

  # 根节点
  root: BlockId
  contents: "Option<BlockId>"

  # 交互状态
  mouse_target: "Option<BlockId>"
  focus_owner: "Option<BlockId>"
  captured: "Option<BlockId>"
}

# 节点关系
FigureBlock_A: "FigureBlock"
FigureBlock_B: "FigureBlock"
FigureBlock_C: "FigureBlock"

FigureGraph.nodes -> FigureBlock_A: "id=1"
FigureGraph.nodes -> FigureBlock_B: "id=2"
FigureGraph.nodes -> FigureBlock_C: "id=3"

FigureBlock_B.parent -> FigureBlock_A: "parent"
FigureBlock_C.parent -> FigureBlock_B: "parent"

FigureBlock_A.children -> FigureBlock_B: "children[0]"
FigureBlock_B.children -> FigureBlock_C: "children[0]"
```

## 5. NovadrawSystem 组合结构

```d2
direction: left-right

NovadrawSystem: {
  shape: package

  scene: FigureGraph
  update_manager: "Arc<dyn UpdateManager>"
  dispatcher: "Arc<dyn EventDispatcher>"
  scene_host: "Arc<dyn SceneHost>"
}

# FigureGraph 展开
FigureGraph: {
  nodes: "SlotMap<BlockId, FigureBlock>"
  root: BlockId
  contents: Option<BlockId>
  mouse_target: Option<BlockId>
  focus_owner: Option<BlockId>
  captured: Option<BlockId>
}

# FigureBlock 展开
FigureBlock: {
  id: BlockId
  parent: Option<BlockId>
  children: "Vec<BlockId>"
  is_valid: bool
  layout_manager: "Option<Arc<dyn LayoutManager>>"
  figure: "Box<dyn Figure>"
}

# Trait 实现
SceneUpdateManager: {
  tooltip: "UpdateManager 实现"
}
WinitEventDispatcher: {
  tooltip: "EventDispatcher 实现"
}
WinitSceneHost: {
  tooltip: "SceneHost 实现"
}

# 关系
NovadrawSystem -> FigureGraph: "scene"
NovadrawSystem -> SceneUpdateManager: "update_manager"
NovadrawSystem -> WinitEventDispatcher: "dispatcher"
NovadrawSystem -> WinitSceneHost: "scene_host"

FigureGraph -> FigureBlock: "nodes"
FigureBlock -> dyn_Figure: "figure"
FigureBlock -> dyn_LayoutManager: "layout_manager"
```

## 6. 平台解耦设计

```d2
direction: left-right

# 平台无关层
Platform_Agnostic: {
  shape: package

  NovadrawSystem: {
    shape: interface
  }

  FigureGraph: {
    shape: interface
  }

  UpdateManager: {
    shape: interface
  }

  EventDispatcher: {
    shape: interface
  }

  SceneHost: {
    shape: interface
  }

  Figure: {
    shape: interface
  }

  LayoutManager: {
    shape: interface
  }
}

# 平台实现层
Platform_Impl: {
  shape: package

  WinitNovadrawSystem: {
    tooltip: "winit 平台实现"
  }

  WinitEventDispatcher: {
    tooltip: "winit 平台实现"
  }

  WinitSceneHost: {
    tooltip: "winit 平台实现"
  }

  VelloRenderBackend: {
    tooltip: "Vello/WebGPU 渲染"
  }
}

# 实现关系
Platform_Impl.WinitNovadrawSystem --> Platform_Agnostic.NovadrawSystem: "实现"
Platform_Impl.WinitEventDispatcher --> Platform_Agnostic.EventDispatcher: "实现"
Platform_Impl.WinitSceneHost --> Platform_Agnostic.SceneHost: "实现"
Platform_Impl.VelloRenderBackend --> Platform_Agnostic.RenderBackend: "实现"
```

## 7. 组件职责表

| 组件 | 对应 g2 | 职责 | 关键字段/方法 |
|------|---------|------|--------------|
| **FigureGraph** | Figure 类树 | 树结构管理 + 交互状态 | `SlotMap<BlockId, FigureBlock>`, `root`, `contents`, `mouse_target`, `focus_owner` |
| **NovadrawSystem** | LightweightSystem | 单个宿主实例的组合根 | `scene`, `update_manager`, `dispatcher`, `scene_host` |
| **FigureBlock** | Figure 状态 | 单节点运行时状态 | `id`, `parent`, `children`, `is_valid`, `layout_manager`, `figure` |
| **dyn Figure** | IFigure | 渲染接口定义 | `bounds()`, `paint_figure()`, `validate()`, `preferred_size()` |
| **LayoutManager** | LayoutManager | 布局策略接口 | `layout()`, `get_preferred_size()` |
| **UpdateManager** | UpdateManager | 两阶段更新调度 | `add_invalid_figure()`, `perform_validation()`, `repair_damage()` |
| **EventDispatcher** | EventDispatcher | 事件分发（trait） | `dispatch_mouse_pressed()`, `dispatch_key_pressed()` |
| **SceneHost** | LightweightSystem | 渲染入口协调 | `execute_update()`, `viewport_size()`, `request_update()` |

## 8. 与 g2 组件对应

| draw2d 组件 | Novadraw 对应 | 说明 |
|---|---|---|
| `IFigure` / `Figure` | `dyn Figure` + `FigureBlock` | Trait 定义渲染接口，FigureBlock 管理运行时状态 |
| `Figure.parent` / `children` | `FigureBlock.parent` / `children` | 改为 ID 引用，避免嵌套借用 |
| `Figure.paint()` | `FigureGraph` 遍历调度 | 模板方法在渲染器中实现 |
| `Figure.getPreferredSize()` | `Figure.preferred_size()` | Figure 的自然属性，三级回退 |
| `LayoutManager` | `LayoutManager` trait | 保持策略接口；具体实现可缓存尺寸或保存约束 |
| `UpdateManager` | `NovadrawSystem.update_manager` | system 实例级服务，与 EventDispatcher 同级 |
| `LightweightSystem` | `NovadrawSystem` | 统一组合根，持有该 system 实例的服务 |
| `RootFigure` | **RootFigure（保留）** | 平台资源桥接层（背景色/字体委托） |
| `EventDispatcher` | `NovadrawSystem.dispatcher` | system 实例级服务，三层分离设计 |
