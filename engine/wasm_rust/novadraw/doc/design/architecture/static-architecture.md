# 理想架构：静态结构

类型：`normative-design`

本文定义 Novadraw 核心类型的所有权、职责和依赖方向。总体原则见
[overview.md](overview.md)，运行时时序见
[dynamic-architecture.md](dynamic-architecture.md)。

文中的 Rust 代码用于表达契约，不承诺具体字段名或模块路径。

## 1. 总体结构

```text
Application
    │ commands / normalized input
    ▼
Runtime
├── FigureTree
│   └── SlotMap<FigureId, FigureNode>
├── InteractionState
├── EventDispatcher
├── UpdateManager
└── MutationQueue
    │
    ├── RecordingCanvas ──> RenderSubmission ──> RenderBackend
    └── FrameRequest ──────────────────────────> PlatformHost
```

依赖方向固定为：

```text
geometry
  ↑
figure + layout
  ↑
tree
  ↑
runtime
  ↑
platform adapters / application

render protocol <── runtime
render backend  ──> render protocol
```

平台层和具体后端不能被底层领域模块反向依赖。

## 2. 身份与树

### 2.1 FigureId

```rust
pub struct FigureId(/* generational arena key */);
```

`FigureId` 只在所属 Runtime 生命周期内有效。arena 必须使用代际身份，确保删除后的
旧 ID 不会错误命中新节点。

UUID、文档对象 ID 和业务 ID 通过可选映射关联，不属于核心节点身份。
为避免长期 crate 循环依赖，`FigureId` 应定义在不依赖 FigureTree 的基础协议模块中。

### 2.2 FigureTree

```rust
pub struct FigureTree {
    nodes: SlotMap<FigureId, FigureNode>,
    root: FigureId,
    contents: Option<FigureId>,
}
```

FigureTree 负责：

- parent/children 双向一致性；
- children 的稳定顺序和 Z-order；
- 防止环和跨 Runtime ID；
- attach、detach、remove 和 reparent；
- 祖先、后代和树序查询。

FigureTree 不拥有：

- pointer、focus、hover 或 gesture session；
- invalid/dirty 队列；
- 平台窗口和 GPU 资源；
- frame scheduling。

## 3. FigureNode

```rust
pub struct FigureNode {
    parent: Option<FigureId>,
    children: Vec<FigureId>,
    state: NodeState,
    layout: Option<LayoutState>,
    figure: Box<dyn Figure>,
}
```

### 3.1 NodeState

```rust
pub struct NodeState {
    bounds: Rect,
    insets: Insets,
    visible: bool,
    enabled: bool,
    opaque: bool,
    validity: Validity,
    size_override: SizeOverride,
    style: StyleOverride,
    local_transform: Affine2D,
    child_transform: Affine2D,
    visual_effect: Option<VisualEffect>,
}
```

字段语义：

| 字段 | 语义 |
|---|---|
| `bounds` | parent content domain 中的 border box |
| `insets` | border box 到 content box 的内缩 |
| `visible` | 是否参与绘制和默认命中 |
| `enabled` | 是否有资格接收输入，不改变可见性 |
| `opaque` | 是否覆盖自身背景区域 |
| `validity` | 布局和派生缓存是否有效 |
| `size_override` | 用户显式 preferred/minimum/maximum 设置 |
| `style` | 本地样式覆盖；未设置项沿父链继承 |
| `local_transform` | layout 后应用于当前节点的二维视觉变换 |
| `child_transform` | content domain 到 children domain 的二维变换 |
| `visual_effect` | 可选的合成期 2D/投影视觉效果 |

`selected` 不属于 Draw2D Figure 核心状态。选择属于编辑器/viewer 层，除非未来有明确
的引擎级选择协议，否则不进入 NodeState。

### 3.2 Figure

`Figure` 只包含具体图形的差异化行为：

```rust
pub trait Figure {
    fn paint(&self, ctx: &mut PaintContext<'_>);
    fn intrinsic_size(&self, constraints: MeasureConstraints) -> Size;
    fn hit_test(&self, local_point: Point) -> bool;

    fn event_handler(&mut self) -> Option<&mut dyn FigureEventHandler> {
        None
    }

    fn lifecycle(&mut self) -> Option<&mut dyn FigureLifecycle> {
        None
    }
}
```

这是概念接口，不要求使用上述精确的 capability 查询形式。稳定要求是：

- 公共节点状态不在每个 Figure 实现中重复；
- 输入、生命周期和 accessibility 是可选能力；
- 不用一组默认空回调扩张基础 Figure trait；
- 不使用 blanket impl 锁死具体 Shape 的定制空间；
- 核心 Figure 不默认要求 `Send + Sync`。

### 3.3 盒模型

```text
border_box = Rect(origin = 0, size = bounds.size)
client_box = border_box.inset(insets)
```

布局、children paint clip 和 hit-test descent 必须调用同一个盒模型函数，禁止分别
重算。

## 4. 布局状态

```rust
pub struct LayoutState {
    manager: Box<dyn LayoutManager>,
    constraints: HashMap<FigureId, Box<dyn LayoutConstraint>>,
    cache: LayoutCache,
}
```

### 4.1 所有权

- LayoutState 属于容器节点；
- constraint 属于 parent 与 child 的关系；
- remove/reparent 必须清除旧 parent 的 constraint；
- cache 属于具体容器的布局结果，不属于 FigureTree 全局状态；
- LayoutManager 不长期持有 FigureTree。

### 4.2 LayoutManager

```rust
pub trait LayoutManager {
    fn measure(
        &mut self,
        container: FigureId,
        snapshot: &LayoutSnapshot,
        constraints: MeasureConstraints,
    ) -> Size;

    fn layout(
        &mut self,
        container: FigureId,
        snapshot: &LayoutSnapshot,
        out: &mut LayoutOutput,
    );

    fn invalidate(&mut self, reason: LayoutInvalidation);
}
```

Runtime 在借用 LayoutManager 前构造不可变 `LayoutSnapshot`；`LayoutOutput` 收集
child bounds 和后续 invalidation。这样布局计算不需要同时借用 manager 和整棵树，
也不会重入修改树。

约束可以类型擦除，但 LayoutManager 必须验证 constraint 类型并返回结构化错误，
不能依靠 unchecked downcast。

### 4.3 尺寸解析

```text
explicit override
→ layout measurement
→ figure intrinsic size
→ current bounds size
```

`None` 表示“没有结果”；`Size::ZERO` 是合法尺寸，不能被当作 fallback sentinel。

## 5. 交互状态

```rust
pub struct InteractionState {
    pointers: HashMap<PointerId, PointerState>,
    focus_owner: Option<FigureId>,
    hover_owner: Option<FigureId>,
    gestures: HashMap<GestureSessionId, GestureState>,
}

pub struct PointerState {
    position: Point,
    target: Option<FigureId>,
    captured: Option<FigureId>,
    pressed_buttons: ButtonSet,
}
```

InteractionState 与 FigureTree 同属 Runtime，但二者是独立组件：

- FigureTree 回答“节点是什么、如何连接”；
- InteractionState 回答“当前交互进行到哪里”；
- EventDispatcher 是读取二者并推进状态机的算法；
- 删除节点时由 Runtime 协调清理所有悬空交互引用。

使用 `PointerId` 而不是单一 `captured`，为 Web pointer events、触控和笔输入保留
自然扩展路径。

## 6. Runtime

```rust
pub struct Runtime {
    tree: FigureTree,
    interaction: InteractionState,
    dispatcher: EventDispatcher,
    updates: UpdateManager,
    mutations: MutationQueue,
    resources: ResourceRegistry,
}
```

默认 `EventDispatcher` 和 `UpdateManager` 是具体类型，因为其时序属于核心不变量。
内部算法可以通过小策略替换；只有出现真实的完整替代实现时，才把整体提升为 trait。

Runtime 对外提供命名操作，例如：

```text
set_contents
add_figure / remove_figure / reparent
set_bounds / set_visible / set_enabled
dispatch_input
resize_logical_viewport
prepare_frame
```

不得同时向调用者暴露 `&mut FigureTree`、`&mut InteractionState` 和
`&mut UpdateManager`，否则调用者可以绕过原子事务。

## 7. 回调上下文

Figure 回调使用无 Runtime 可变借用的上下文：

```rust
pub struct EventContext<'a> {
    target: FigureId,
    snapshot: &'a EventSnapshot,
    effects: &'a mut EffectQueue,
}
```

它可以：

- 查询分发前构造的稳定只读快照；
- 请求 bounds、focus、capture、repaint 或 invalidate 变更；
- 排队结构 mutation；
- 返回 handled 状态。

它不能：

- 取得 FigureTree 或 UpdateManager；
- 在回调栈内直接改变 children；
- 递归发起另一个顶层输入事务；
- 持有超过回调生命周期的引用。

## 8. 更新与变更

```rust
pub struct UpdateManager {
    invalid: InvalidSet,
    dirty: DirtySet,
    phase: UpdatePhase,
    listeners: ListenerRegistry,
}

pub struct MutationQueue {
    pending: VecDeque<Mutation>,
}
```

UpdateManager 与 MutationQueue 使用 `FigureId` 引用节点，但不拥有树。

Mutation 至少覆盖：

- add/remove/reparent；
- child reorder；
- layout manager replacement；
- layout constraint replacement；
- contents replacement。

每个 mutation 的提交结果必须同时给出：

- topology changes；
- affected layout roots；
- old/new visual damage；
- invalid interaction references；
- lifecycle callbacks。

## 9. 平台边界

```rust
pub trait PlatformHost {
    fn request_redraw(&self);
    fn surface_info(&self) -> SurfaceInfo;
    fn set_cursor(&self, cursor: CursorIcon);
    fn set_ime_state(&self, state: ImeState);
}
```

具体 adapter：

```text
WinitAdapter  ── macOS / Windows / Linux
WebAdapter    ── DOM / Canvas
HeadlessHost  ── tests / deterministic replay
```

平台 adapter 负责 native event 到 `InputEvent` 的转换。EventDispatcher 始终是
平台无关运行时组件，不存在 `WinitEventDispatcher` 这一领域概念。

## 10. 渲染边界

```rust
pub struct RenderSubmission {
    commands: CommandStream,
    damage: Damage,
    resources: ResourceDelta,
    surface: SurfaceInfo,
}

pub trait RenderBackend {
    type Error;

    fn resize(&mut self, surface: SurfaceInfo) -> Result<(), Self::Error>;
    fn submit(&mut self, frame: &RenderSubmission) -> Result<(), Self::Error>;
}
```

`CommandStream` 是进程内语义协议，不等同于稳定二进制 ABI。后端可以在内部把二维
affine 提升为 `Mat4`，但不能反向改变 Figure 的二维布局语义。

资源通过稳定 handle 引用，生命周期由 `ResourceRegistry` 和 submission delta
协调，不让 Figure 持有具体 GPU 对象。

## 11. 2.5D 与 Scene3D

```rust
pub enum VisualEffect {
    Affine(Affine2D),
    Projective(ProjectiveTransform),
}
```

`ProjectiveTransform` 是合成能力，不改变二维 layout bounds。其规范必须明确：

- 投影后的绘制边界；
- clip 行为；
- 命中是否启用逆投影；
- 不可逆或穿过相机平面的失败处理；
- backend 不支持时的降级方式。

真正 3D 使用独立类型：

```text
Scene3D
├── SpatialNode
├── Transform3D
├── Camera
├── Bounds3D
├── RayHitTest
└── Depth/Material/Lighting
```

二维 FigureTree 与 Scene3D 通过明确的嵌入节点或合成 surface 连接，不共享一个
模糊的 `Point` 类型别名。

## 12. Trait 使用原则

使用 trait 的判断标准：

1. 是否存在多个真实实现；
2. 是否需要由调用方替换；
3. 是否能形成小而稳定的行为接口；
4. 动态分发是否位于非关键热路径，或其扩展收益是否足够。

因此：

| 组件 | 默认形态 |
|---|---|
| Figure | trait object |
| LayoutManager | trait object |
| PlatformHost | trait 或泛型边界 |
| RenderBackend | trait 或泛型边界 |
| FigureTree | concrete |
| InteractionState | concrete |
| EventDispatcher | concrete |
| UpdateManager | concrete |
| Runtime | concrete |

## 13. 长期 crate 边界

稳定后可演进为：

```text
novadraw-geometry
novadraw-figure
novadraw-layout
novadraw-tree
novadraw-runtime
novadraw-render
novadraw-platform
novadraw-3d          # 独立可选能力
novadraw             # facade
```

crate 拆分必须跟随稳定依赖方向，不能为了目录整齐预先制造 facade 和循环依赖。
