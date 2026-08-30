# Novadraw 架构设计（历史文档）

类型：`archive`

> ⚠️ **历史文档** — 本文档为早期架构设计记录，内容已被`doc/design/architecture/overview.md`取代。
>
> **关键冲突点**：
> - 交互状态归属：本文档将 mouse_target/focus_owner/captured 放在 EventDispatcher；`doc/design/architecture/overview.md`已改为统一收敛到 FigureGraph
> - EventDispatcher 与 FigureGraph 的持有关系：`doc/design/architecture/overview.md`要求 EventDispatcher 通过 DispatchContext 访问 Graph，不直接持有引用
> - UpdateManager 层级：本文档未明确上移；`doc/design/architecture/overview.md`确定在 NovadrawSystem 层级
>
> **仍有参考价值的部分**：Figure API 映射表（Section 5）、Phase 里程碑、Connection/Layer 早期草案。请对照`doc/design/architecture/overview.md`确认后再使用。

---

本文档描述 Novadraw 图形工具包的架构设计，目标是实现功能完整、性能优越的 draw2d/GEF 精神继承。

---

## 目录

1. [设计原则](#1-设计原则)
2. [模块结构](#2-模块结构)
3. [g2 核心机制与 Novadraw 设计](#3-g2-核心机制与-novadraw-设计)
   - [3.1 更新管线（UpdateManager）](#31-更新管线updatemanager)
   - [3.2 渲染管线（Figure.paint）](#32-渲染管线figurepaint)
   - [3.3 状态管理（Graphics）](#33-状态管理graphics)
   - [3.4 事件分发（EventDispatcher）](#34-事件分发eventdispatcher)
   - [3.5 布局系统（LayoutManager）](#35-布局系统layoutmanager)
   - [3.6 连接系统（Connection）](#36-连接系统connection)
   - [3.7 图层系统（Layer）](#37-图层系统layer)
4. [模块文件拆分](#4-模块文件拆分)
5. [Figure API 完整映射](#5-figure-api-完整映射)
   - [5.1 设计原则](#51-设计原则)
   - [5.2 层级管理 API](#52-层级管理-api)
   - [5.3 Bounds API](#53-bounds-api)
   - [5.4 坐标变换 API](#54-坐标变换-api)
   - [5.5 渲染 API](#55-渲染-api)
   - [5.6 状态查询 API](#56-状态查询-api)
   - [5.7 布局 API](#57-布局-api)
   - [5.8 事件 API](#58-事件-api)
   - [5.9 剪裁 API](#59-剪裁-api)
   - [5.10 命中测试 API](#510-命中测试-api)
   - [5.11 简化决策汇总](#511-简化决策汇总)
   - [5.12 g2 核心设计模式继承](#512-g2-核心设计模式继承)
6. [Phase 规划](#6-phase-规划)

---

## 1. 设计原则

1. **核心优先**：先实现核心功能，再逐步扩展
2. **接口抽象**：模块间通过 trait 解耦
3. **数据分离**：状态数据与图形数据分离
4. **迭代遍历**：树操作必须使用迭代，禁止递归
5. **两阶段更新**：布局（validation）与渲染（repaint）分离

---

## 2. 模块结构

```text
novadraw/
├── novadraw-core/       # 核心类型：Color、Point、Rect
├── novadraw-geometry/   # 几何计算：Transform、Path
├── novadraw-math/        # 数学运算：Vec3、Mat3
│
├── novadraw-figure/     # Figure 核心
│   ├── figure.rs        # Figure、Bounded、Updatable、Shape trait
│   └── types/          # 基础 Figure 实现
│
├── novadraw-scene/      # 场景图管理
│   ├── scene/
│   │   ├── mod.rs      # FigureGraph 结构体
│   │   ├── tree_ops.rs # 树操作
│   │   ├── layout.rs   # 布局相关
│   │   ├── update.rs   # 更新编排
│   │   ├── render.rs   # 渲染编排
│   │   └── hit_test.rs # 命中测试
│   └── viewport.rs      # 视口管理
│
├── novadraw-graphics/   # 绘图上下文
│   ├── context.rs       # NdCanvas - 命令生成
│   ├── state.rs         # 状态栈
│   └── clip.rs          # 裁剪区域
│
├── novadraw-layout/     # 布局系统
│   ├── layout.rs        # LayoutManager trait
│   ├── flow.rs          # FlowLayout
│   └── border.rs        # BorderLayout
│
├── novadraw-render/     # 渲染后端
│   ├── backend.rs       # RenderBackend trait
│   └── vello.rs        # Vello 实现
│
├── novadraw-connection/ # 连接系统 [Phase 2]
│   ├── connection.rs    # Connection trait
│   ├── anchor.rs        # ConnectionAnchor
│   └── router.rs        # ConnectionRouter
│
└── novadraw-layer/     # 图层系统 [Phase 2]
    ├── layer.rs         # Layer trait
    └── layered_pane.rs   # LayeredPane
```

---

## 3. g2 核心机制与 Novadraw 设计

### 3.1 更新管线（UpdateManager）

#### g2 设计

g2 的更新管线是**两阶段分离设计**：

```text
┌─────────────────────────────────────────────────────────────┐
│                    UpdateManager                             │
│                                                              │
│   用户修改 bounds/添加删除 figure                            │
│           │                                                 │
│           ▼                                                 │
│   ┌─────────────────────────────────────────────┐           │
│   │  Phase 1: Validation（布局验证）            │           │
│   │                                              │           │
│   │  for each invalid_figure:                    │           │
│   │      fig.validate()                         │           │
│   │          → LayoutManager.layout()          │           │
│   │          → Figure.validate()                │           │
│   │                                              │           │
│   │  原地清空 + swap 模式避免并发问题            │           │
│   └─────────────────────────────────────────────┘           │
│           │                                                 │
│           ▼                                                 │
│   ┌─────────────────────────────────────────────┐           │
│   │  Phase 2: Repair（脏区域重绘）              │           │
│   │                                              │           │
│   │  1. 脏区域向父级传播并合并                   │           │
│   │  2. 调用 root.paint(graphics)               │           │
│   │  3. 双缓冲 swap 避免读写冲突                 │           │
│   └─────────────────────────────────────────────┘           │
└─────────────────────────────────────────────────────────────┘
```

**关键数据结构**：

```java
// DeferredUpdateManager.java
public class DeferredUpdateManager extends UpdateManager {
    private Rectangle damage;                           // 合并后的全局脏区域
    private Map<IFigure, Rectangle> dirtyRegions;     // figure → 脏区域
    private List<IFigure> invalidFigures;            // 待验证块
    private boolean updateQueued;                     // 防重复入队
    private boolean updating;                          // 防重入
}
```

**原地清空模式（避免并发问题）**：

```java
// performValidation 中
for (int i = 0; i < invalidFigures.size(); i++) {
    IFigure fig = invalidFigures.get(i);
    invalidFigures.set(i, null);    // 原地置空
    fig.validate();                 // validate 可能新增 invalid figure
}
invalidFigures.clear();             // 遍历完后清空
```

#### Novadraw 设计

```rust
// novadraw-scene/src/update/mod.rs

pub struct SceneUpdateManager {
    invalid_figures: Vec<BlockId>,      // 待验证块队列
    dirty_regions: Vec<Rectangle>,      // 脏区域队列
    update_queued: bool,               // 防重复入队
    updating: bool,                    // 防重入
}

impl SceneUpdateManager {
    /// 两阶段更新
    pub fn perform_update(&mut self, graph: &mut FigureGraph) -> NdCanvas {
        // Phase 1: Validation
        self.perform_validation(graph);

        // Phase 2: Repair
        self.repair_damage(graph)
    }

    /// 验证所有失效块（原地清空模式）
    fn perform_validation(&mut self, graph: &mut FigureGraph) {
        let blocks: Vec<_> = self.invalid_figures.drain(..).collect();
        for block_id in blocks {
            if let Some(block) = graph.blocks.get(block_id) {
                if block.is_visible && block.is_enabled {
                    graph.revalidate(block_id);
                }
            }
        }
    }

    /// 修复脏区域
    fn repair_damage(&mut self, graph: &mut FigureGraph) -> NdCanvas {
        // 合并脏区域
        let damage = self.compute_damage();

        let mut canvas = NdCanvas::new();
        if damage.width > 0.0 && damage.height > 0.0 {
            canvas.clip_rect(damage.x, damage.y, damage.width, damage.height);
        }

        // 渲染场景
        graph.render_to_iterative(&mut canvas);

        // 清空脏区域
        self.clear_dirty_and_flag();

        canvas
    }
}
```

#### 设计要点对比

| 特性 | g2 | Novadraw |
|------|-----|----------|
| 两阶段分离 | ✅ Validation + Repair | ✅ |
| 原地清空 | ✅ `set(i, null)` | ✅ `drain(..)` |
| 防重入 | ✅ `updating` flag | ✅ `updating` flag |
| 防重复入队 | ✅ `updateQueued` flag | ✅ `update_queued` flag |
| 脏区域合并 | ✅ `damage.union()` | ✅ `compute_damage()` |
| 延迟批处理 | ✅ `Display.asyncExec` | 待实现 |

---

### 3.2 渲染管线（Figure.paint）

#### g2 设计

g2 的 `paint()` 是模板方法模式，分三个阶段：

```java
// Figure.java paint() 模板
public void paint(Graphics graphics) {
    // 1. 设置本地属性（颜色、字体）
    if (getLocalBackgroundColor() != null)
        graphics.setBackgroundColor(getLocalBackgroundColor());

    // 2. pushState：保存完整状态
    graphics.pushState();
    try {
        // 3. paintFigure：绘制自身
        paintFigure(graphics);

        // 4. restoreState：恢复颜色（不恢复变换）
        graphics.restoreState();

        // 5. paintClientArea：绘制子节点
        paintClientArea(graphics);

        // 6. paintBorder：绘制边框
        paintBorder(graphics);
    } finally {
        // 7. popState：恢复完整状态
        graphics.popState();
    }
}
```

**paintChildren 递归遍历**：

```java
protected void paintChildren(Graphics graphics) {
    for (IFigure child : getChildren()) {
        if (!child.isVisible()) continue;
        graphics.clipRect(child.getBounds());
        child.paint(graphics);    // 递归
    }
}
```

#### Novadraw 设计

```rust
// novadraw-scene/src/scene/render_iterative.rs

/// 迭代渲染器
pub struct FigureRendererIter<'a> {
    scene: &'a FigureGraphRenderRef<'a>,
    gc: &'a mut NdCanvas,
    stack: Vec<Frame<'a>>,    // 显式栈替代递归
}

struct Frame<'a> {
    block_id: BlockId,
    children_iter: std::slice::Iter<'a, BlockId>,
}

impl<'a> FigureRendererIter<'a> {
    pub fn render(&mut self, root_id: BlockId) {
        self.stack.push(Frame {
            block_id: root_id,
            children_iter: self.scene.get_children(root_id).into_iter(),
        });

        while let Some(frame) = self.stack.pop() {
            let block_id = frame.block_id;

            // pushState：保存完整状态
            self.gc.push_state();

            // 获取 bounds 并 translate
            let bounds = self.scene.get_bounds(block_id);
            self.gc.translate(bounds.x, bounds.y);

            // clipRect + paintFigure
            self.gc.clip_rect(0.0, 0.0, bounds.width, bounds.height);
            self.paint_figure(block_id);

            // restoreState：恢复颜色
            self.gc.restore_state();

            // paintChildren：绘制子节点
            self.paint_children(block_id);

            // paintBorder：绘制边框
            self.paint_border(block_id);

            // popState：恢复完整状态
            self.gc.pop_state();
        }
    }
}
```

#### 设计要点对比

| 特性 | g2 | Novadraw |
|------|-----|----------|
| 模板方法 | ✅ paint() | ✅ |
| pushState/popState | ✅ | ✅ |
| restoreState | ✅ 颜色恢复 | ✅ |
| paintFigure | ✅ 抽象方法 | ✅ `paint_figure()` |
| paintClientArea | ✅ 含裁剪 | ✅ 含裁剪 |
| paintBorder | ✅ 最后绘制 | ✅ 最后绘制 |
| 递归遍历 | ✅ `child.paint()` | ❌ 迭代栈 |
| 子节点裁剪 | ✅ clipRect | ✅ clip_rect |

---

### 3.3 状态管理（Graphics）

#### g2 设计

g2 的 `SWTGraphics` 使用**三状态模型**：

```java
// SWTGraphics.java 三状态
static class State extends LazyState {
    float[] affineMatrix;       // 变换矩阵
    int alpha;                // 透明度
    Clipping relativeClip;     // 裁剪区域
    // ...
}

State currentState;           // 当前逻辑状态
LazyState appliedState;       // 已应用到 GC 的状态
List<State> stack;           // 状态栈
int stackPointer;
```

**三操作语义**：

| 操作 | 行为 |
|------|------|
| `pushState()` | 复制当前状态到栈顶，`stackPointer++` |
| `restoreState()` | 从栈顶下方读取状态恢复到 `currentState`，`stackPointer` 不变 |
| `popState()` | 恢复栈顶状态，`stackPointer--` |

**paint() 中的使用**：

```java
graphics.pushState();           // 保存完整状态
paintFigure(graphics);          // 绘制（可能修改颜色）
graphics.restoreState();        // 恢复颜色（保留变换）
paintClientArea(graphics);      // 绘制子节点（在子坐标中）
paintBorder(graphics);          // 绘制边框
graphics.popState();           // 恢复完整状态
```

#### Novadraw 设计

```rust
// novadraw-render/src/context.rs

pub struct NdCanvas {
    commands: Vec<RenderCommand>,
    current_path: Option<Path>,
    // 状态由 RenderCommand 隐式管理
}

impl NdCanvas {
    /// 保存当前状态
    pub fn push_state(&mut self) {
        self.create_command(RenderCommandKind::PushState);
    }

    /// 恢复状态（不弹出栈）
    pub fn restore_state(&mut self) {
        self.create_command(RenderCommandKind::RestoreState);
    }

    /// 弹出并恢复状态
    pub fn pop_state(&mut self) {
        self.create_command(RenderCommandKind::PopState);
    }
}
```

**状态命令处理（在渲染后端）**：

```rust
// novadraw-render/src/backend/vello/mod.rs

fn apply_command(scene: &mut vello::Scene, cmd: &RenderCommand) {
    match &cmd.kind {
        RenderCommandKind::PushState => {
            // Vello scene.push_layer()
        }
        RenderCommandKind::PopState => {
            // Vello scene.pop_layer()
        }
        RenderCommandKind::RestoreState => {
            // Vello：恢复到上次 push 后的状态
        }
        // ...
    }
}
```

#### 设计要点对比

| 特性 | g2 | Novadraw |
|------|-----|----------|
| 三状态模型 | ✅ current/applied/stack | ⚠️ 简化 |
| pushState | ✅ 复制到栈顶 | ✅ 命令化 |
| restoreState | ✅ 不改指针 | ✅ 命令化 |
| popState | ✅ 恢复+指针-- | ✅ 命令化 |
| 延迟应用 | ✅ LazyState | ⚠️ 立即应用 |

---

### 3.4 事件分发（EventDispatcher）

#### g2 设计

g2 的事件分发通过 `EventDispatcher` 实现：

```java
// 鼠标目标确定
private void receive(MouseEvent me) {
    if (captured) {
        // 鼠标捕获模式
        currentEvent = new MouseEvent(this, mouseTarget, me);
    } else {
        // findFigureAt 找到最上层可见 figure
        IFigure f = root.findFigureAt(me.x, me.y);
        if (f == mouseTarget) {
            currentEvent = new MouseEvent(this, mouseTarget, me);
            return;
        }
        // 处理 exit/enter 过渡
        if (mouseTarget != null) {
            mouseTarget.handleMouseExited(currentEvent);
        }
        setMouseTarget(f);
        if (mouseTarget != null) {
            mouseTarget.handleMouseEntered(currentEvent);
        }
    }
}
```

**事件分发流程**：

```text
Canvas 事件
    │
    ▼
EventHandler.handleEvent()
    │
    ▼
EventDispatcher.dispatch*(event)
    │
    ├── MouseDown ────► receive() → findFigureAt() → setCapture()
    │                      │
    │                      └── target.handleMousePressed()
    │
    ├── MouseUp ──────► target.handleMouseReleased() + releaseCapture()
    │
    ├── MouseMove ────► receive() → findFigureAt() → handleMouseMoved()
    │
    ├── KeyDown ──────► focusOwner.handleKeyPressed()
    │
    └── Traverse ─────► FocusTraverseManager → setFocus(next)
```

**鼠标捕获（Capture）**：

- `setCapture(figure)` 后，所有鼠标事件绕过 `findFigureAt`，直接发给捕获目标
- `releaseCapture()` 解除捕获
- 用于拖拽操作：按下时捕获，释放时解除

#### Novadraw 设计（待实现）

```rust
// novadraw-scene/src/event.rs

pub trait EventHandler {
    fn handle_mouse_pressed(&mut self, pos: (f64, f64), button: MouseButton);
    fn handle_mouse_released(&mut self, pos: (f64, f64), button: MouseButton);
    fn handle_mouse_moved(&mut self, pos: (f64, f64));
    fn handle_key_pressed(&mut self, key: KeyCode);
    fn handle_key_released(&mut self, key: KeyCode);
}

pub struct EventDispatcher {
    scene: FigureGraph,
    mouse_target: Option<BlockId>,
    focus_owner: Option<BlockId>,
    captured: Option<BlockId>,     // 鼠标捕获
}

impl EventDispatcher {
    /// 确定鼠标目标
    fn find_target(&self, pos: (f64, f64)) -> Option<BlockId> {
        if let Some(cap) = self.captured {
            return Some(cap);
        }
        self.scene.find_figure_at(pos).map(|(id, _)| id)
    }

    /// 鼠标按下
    pub fn dispatch_mouse_pressed(&mut self, pos: (f64, f64), button: MouseButton) {
        let target = self.find_target(pos);

        if target != self.mouse_target {
            // 处理 exit/enter 过渡
            if let Some(old) = self.mouse_target {
                self.send_mouse_exited(old);
            }
            if let Some(new) = target {
                self.send_mouse_entered(new);
            }
        }

        self.mouse_target = target;

        if let Some(t) = target {
            if self.send_mouse_pressed(t, pos, button) {
                self.set_capture(t);  // 捕获鼠标
            }
        }
    }

    /// 鼠标释放
    pub fn dispatch_mouse_released(&mut self, pos: (f64, f64), button: MouseButton) {
        if let Some(t) = self.mouse_target {
            self.send_mouse_released(t, pos, button);
        }
        self.release_capture();
    }

    /// 设置鼠标捕获
    fn set_capture(&mut self, block_id: BlockId) {
        self.captured = Some(block_id);
    }

    /// 释放鼠标捕获
    fn release_capture(&mut self) {
        self.captured = None;
    }
}
```

#### 设计要点对比

| 特性 | g2 | Novadraw |
|------|-----|----------|
| 事件路由 | ✅ EventDispatcher | ✅ 待实现 |
| 命中测试 | ✅ `findFigureAt` | ✅ `find_figure_at` |
| 鼠标捕获 | ✅ setCapture | ✅ 待实现 |
| 焦点管理 | ✅ focusOwner | ✅ 待实现 |
| Enter/Exit | ✅ handleMouseEntered | ✅ 待实现 |

---

### 3.5 布局系统（LayoutManager）

#### g2 设计

```java
public interface LayoutManager {
    Object getConstraint(IFigure child);
    void setConstraint(IFigure child, Object constraint);
    Dimension getPreferredSize(IFigure container, int wHint, int hHint);
    void layout(IFigure container);
    void invalidate();
    void remove(IFigure child);
}
```

**主要布局器**：

| 布局器 | 约束类型 | 说明 |
|--------|----------|------|
| FlowLayout | 无 | 横向排列，自动换行 |
| BorderLayout | BorderRegion | 东西南北中五区域 |
| XYLayout | Point | 自由坐标 |
| GridLayout | GridData | 网格排列 |
| StackLayout | 无 | 所有子元素堆叠 |

#### Novadraw 设计

```rust
// novadraw-scene/src/layout/mod.rs

pub trait LayoutManager: Send + Sync {
    fn layout(&self, container: BlockId, ctx: &mut dyn LayoutContext);

    fn get_preferred_size(
        &self,
        container: BlockId,
        w_hint: f64,
        h_hint: f64,
        ctx: &dyn LayoutContext,
    ) -> (f64, f64);

    fn get_minimum_size(
        &self,
        container: BlockId,
        ctx: &dyn LayoutContext,
    ) -> (f64, f64);
}

pub trait LayoutContext: Send + Sync {
    fn get_children(&self, parent_id: BlockId) -> Vec<(BlockId, Rectangle)>;
    fn get_constraint(&self, child_id: BlockId) -> Option<Rectangle>;
    fn get_preferred_size(&self, block_id: BlockId) -> (f64, f64);
    fn set_child_bounds(&mut self, child_id: BlockId, bounds: Rectangle);
    fn get_container_bounds(&self, container_id: BlockId) -> Rectangle;
}
```

**已实现布局器**：

- ✅ FlowLayout
- ✅ BorderLayout
- ✅ XYLayout
- ✅ FillLayout
- ⚠️ GridLayout（待实现）

---

### 3.6 连接系统（Connection）

#### g2 设计

Connection 是 g2/GEF 的核心特性，用于连接线：

```java
public interface Connection extends IFigure {
    ConnectionAnchor getSourceAnchor();
    void setSourceAnchor(ConnectionAnchor anchor);
    ConnectionAnchor getTargetAnchor();
    void setTargetAnchor(ConnectionAnchor anchor);
    ConnectionRouter getConnectionRouter();
    void setConnectionRouter(ConnectionRouter router);
    PointList getPoints();
    void setPoints(PointList list);
}

public interface ConnectionAnchor {
    Point getLocation(Point reference);
    IFigure getOwner();
}

public interface ConnectionRouter {
    void route(Connection connection);
    void invalidate(Connection connection);
}
```

**主要路由器**：

| 路由器 | 说明 |
|--------|------|
| NullConnectionRouter | 直线连接 |
| ManhattanConnectionRouter | 曼哈顿路由（水平/垂直正交） |
| BendpointConnectionRouter | 折点路由 |

**PolylineConnection**：

```java
public class PolylineConnection extends Polyline implements Connection {
    private ConnectionAnchor startAnchor;
    private ConnectionAnchor endAnchor;
    private ConnectionRouter router;

    public void layout() {
        if (getSourceAnchor() != null && getTargetAnchor() != null) {
            connectionRouter.route(this);  // 计算路径点
        }
        super.layout();
    }
}
```

#### Novadraw 设计（Phase 2）

```rust
// novadraw-connection/src/lib.rs

/// 连接线 trait
pub trait Connection: Figure {
    fn get_source_anchor(&self) -> Option<&dyn ConnectionAnchor>;
    fn set_source_anchor(&mut self, anchor: Arc<dyn ConnectionAnchor>);
    fn get_target_anchor(&self) -> Option<&dyn ConnectionAnchor>;
    fn set_target_anchor(&mut self, anchor: Arc<dyn ConnectionAnchor>);
    fn get_router(&self) -> &dyn ConnectionRouter;
    fn set_router(&mut self, router: Arc<dyn ConnectionRouter>);
    fn get_points(&self) -> Vec<Point>;
}

/// 连接锚点
pub trait ConnectionAnchor: Send + Sync {
    fn get_location(&self, reference: Point) -> Point;
    fn get_owner(&self) -> BlockId;
}

/// 连接路由器
pub trait ConnectionRouter: Send + Sync {
    fn route(&self, connection: &mut dyn Connection);
    fn invalidate(&self, connection: BlockId);
}

/// 折线连接
pub struct PolylineConnection {
    bounds: Rectangle,
    source_anchor: Option<Arc<dyn ConnectionAnchor>>,
    target_anchor: Option<Arc<dyn ConnectionAnchor>>,
    router: Arc<dyn ConnectionRouter>,
    points: Vec<Point>,
}

impl Figure for PolylineConnection {
    fn paint_figure(&self, gc: &mut NdCanvas) {
        if self.points.len() >= 2 {
            gc.polyline(&self.points, ...);
        }
    }
}

impl Connection for PolylineConnection {
    fn get_router(&self) -> &dyn ConnectionRouter {
        &*self.router
    }

    fn set_router(&mut self, router: Arc<dyn ConnectionRouter>) {
        self.router = router;
    }
}

/// 直线路由器
pub struct NullRouter;

impl ConnectionRouter for NullRouter {
    fn route(&self, connection: &mut dyn Connection) {
        // 直接连接起点和终点
    }
}

/// 曼哈顿路由器
pub struct ManhattanRouter;

impl ConnectionRouter for ManhattanRouter {
    fn route(&self, connection: &mut dyn Connection) {
        // 生成正交折线
    }
}
```

---

### 3.7 图层系统（Layer）

#### g2 设计

Layer 系统支持多层面板：

```java
public class Layer extends Figure {
    // 透明层，可添加图形
}

public class LayeredPane extends Layer {
    private Map<Object, Layer> layers;     // 层级映射
    private List<Layer> findOrder;        // 查找顺序

    public void add(IFigure figure, Object constraint, int index);
    public void remove(IFigure figure);
}

public class ScalableLayeredPane extends LayeredPane
    implements ScalableFigure
{
    // 支持缩放的层级面板
}
```

#### Novadraw 设计（Phase 2）

```rust
// novadraw-layer/src/layer.rs

/// 图层 trait
pub trait Layer: Figure {
    fn get_layer_id(&self) -> &str;
    fn set_visible(&mut self, visible: bool);
    fn is_visible(&self) -> bool;
}

/// 层级面板
pub struct LayeredPane {
    layers: LinkedHashMap<String, BlockId>,  // 层 ID → 块 ID
    find_order: Vec<String>,                  // 查找顺序
}

impl LayeredPane {
    /// 添加图层
    pub fn add_layer(&mut self, layer_id: &str, block_id: BlockId) {
        self.layers.insert(layer_id.to_string(), block_id);
        self.find_order.push(layer_id.to_string());
    }

    /// 获取图层块
    pub fn get_layer(&self, layer_id: &str) -> Option<BlockId> {
        self.layers.get(layer_id).copied()
    }

    /// 按顺序渲染图层
    pub fn render_layers(&self, graph: &FigureGraph, gc: &mut NdCanvas) {
        for layer_id in &self.find_order {
            if let Some(block_id) = self.layers.get(layer_id) {
                if let Some(block) = graph.blocks.get(*block_id) {
                    if block.is_visible {
                        graph.render_block(*block_id, gc);
                    }
                }
            }
        }
    }
}
```

---

## 4. 模块文件拆分

### 4.1 为什么不拆分数据结构

**过早拆分的成本**：

| 成本 | 说明 |
|------|------|
| 管理复杂度增加 | 添加/删除节点需同步多个结构 |
| 边界定义模糊 | 布局数据与状态数据的边界不清晰 |
| 性能收益不明显 | 当前无性能瓶颈需要通过拆分解决 |

**g2 的选择**：将所有状态放在 Figure 内部，简单直接。Novadraw 的 FigureBlock 设计与 g2 一致。

**结论**：保持 FigureBlock 单一结构，不拆分数据结构。

### 4.2 文件拆分方案

**问题**：FigureGraph 当前 2000+ 行，单个文件过大。

**方案**：按功能拆分为多个文件，而非拆分数据结构。

```text
novadraw-scene/src/
├── mod.rs              # 导出 + 组合
├── scene/
│   ├── mod.rs         # FigureGraph 结构体定义
│   ├── tree_ops.rs    # 树操作（add_child、remove、prim_translate）
│   ├── layout.rs      # 布局相关方法
│   ├── update.rs      # 更新编排（perform_update）
│   ├── render.rs      # 渲染编排
│   └── hit_test.rs    # 命中测试（find_figure_at）
```

**拆分原则**：

| 文件 | 内容 |
|------|------|
| `mod.rs` | `FigureGraph` 结构体定义 + `SlotMap` key type |
| `tree_ops.rs` | 树操作：添加/删除/查找/平移 |
| `layout.rs` | 布局编排：revalidate、apply_layout |
| `update.rs` | 更新编排：mark_invalid、repaint、perform_update |
| `render.rs` | 渲染编排：render、render_iterative |
| `hit_test.rs` | 命中测试：find_figure_at |

**好处**：

- 保持 FigureBlock 单一数据结构
- 降低单个文件的代码行数
- 按功能组织，代码更易导航
- 符合 Rust 模块化惯例

---

## 5. Figure API 完整映射

### 5.1 设计原则

**精神继承**：g2 的架构模式（两阶段更新、模板方法、迭代遍历）是核心精神，必须继承。

**简化策略**：具体实现细节可以简化，但需标注"当前简化，未来需实现"。

### 5.2 层级管理 API

| g2 API | 职责 | Novadraw 实现 | 状态 |
|--------|------|---------------|------|
| `add(child)` | 添加子节点 | `FigureGraph.add_child_to()` | ✅ |
| `remove(child)` | 移除子节点 | 待实现 | ⚠️ 简化 |
| `getChildren()` | 获取子节点列表 | `blocks[id].children` | ✅ |
| `getParent()` | 获取父节点 | `blocks[id].parent` | ✅ |
| `setParent(p)` | 设置父节点 | 内部方法 | ✅ |

**简化说明**：

- `remove()` 当前简化：直接移除，不调用 `removeNotify()` 递归通知
- `add()` 当前简化：不检查循环引用

**未来需实现**：

- `removeNotify()` 递归通知机制
- 添加循环引用检查

### 5.3 Bounds API

| g2 API | 职责 | Novadraw 实现 | 状态 |
|--------|------|---------------|------|
| `getBounds()` | 获取边界 | `figure.bounds()` | ✅ |
| `setBounds(rect)` | 设置边界 | `FigureGraph.set_bounds()` | ✅ |
| `setSize(w, h)` | 设置尺寸 | 内部方法 | ✅ |
| `setLocation(p)` | 设置位置 | 内部方法 | ✅ |
| `erase()` | 擦除旧位置 | 无 | ⚠️ 简化 |

**简化说明**：

- `erase()` 当前省略：Vello 使用脏区域重绘，不需要显式擦除
- `setBounds` 当前简化：只更新 bounds，不触发 erase/fire 事件

**未来需实现**：

- `erase()` 机制用于双缓冲场景
- `setBounds` 完整副作用链

### 5.4 坐标变换 API

| g2 API | 职责 | Novadraw 实现 | 状态 |
|--------|------|---------------|------|
| `translate(dx, dy)` | 公共 API | `prim_translate()` | ✅ |
| `primTranslate(dx, dy)` | 内部传播 | 迭代实现 | ✅ |
| `translateToParent(t)` | 转父坐标 | `translate_to_parent()` | ✅ |
| `translateToAbsolute(t)` | 转绝对坐标 | `translate_to_absolute_mut()` | ✅ |
| `useLocalCoordinates()` | 坐标根标志 | `use_local_coordinates()` | ✅ |

**状态**：✅ 完整实现

### 5.5 渲染 API

| g2 API | 职责 | Novadraw 实现 | 状态 |
|--------|------|---------------|------|
| `paint(gc)` | 模板方法入口 | `FigureRendererIter.render()` | ✅ |
| `paintFigure(gc)` | 绘制自身 | `figure.paint_figure()` | ✅ |
| `paintClientArea(gc)` | 绘制子节点 | 迭代渲染器内 | ✅ |
| `paintBorder(gc)` | 绘制边框 | `figure.paint_border()` | ✅ |
| `paintChildren(gc)` | 遍历子节点 | 迭代渲染器内 | ✅ |

**状态**：✅ 完整实现

### 5.6 状态查询 API

| g2 API | 职责 | Novadraw 实现 | 状态 |
|--------|------|---------------|------|
| `isVisible()` | 是否可见 | `is_visible` | ✅ |
| `isEnabled()` | 是否启用 | `is_enabled` | ✅ |
| `isOpaque()` | 是否不透明 | `Shape.alpha()` | ✅ |
| `isShowing()` | 是否显示（递归） | 无 | ⚠️ 简化 |
| `hasFocus()` | 是否拥有焦点 | EventDispatcher 管理 | ✅ |

**简化说明**：

- `isShowing()` 当前省略：渲染/事件时直接检查 `is_visible`
- 递归可见性检查在渲染时通过 `is_visible` 过滤实现

**未来需实现**：

- `isShowing()` 递归检查

### 5.7 布局 API

| g2 API | 职责 | Novadraw 实现 | 状态 |
|--------|------|---------------|------|
| `setLayoutManager(lm)` | 设置布局器 | `set_block_layout_manager()` | ✅ |
| `layout()` | 执行布局 | `LayoutManager.layout()` | ✅ |
| `invalidate()` | 标记失效 | `mark_invalid()` | ✅ |
| `validate()` | 执行验证 | `revalidate()` | ✅ |
| `revalidate()` | 公共失效入口 | 相同 | ✅ |
| `isValidationRoot()` | 验证根标志 | 无 | ⚠️ 简化 |

**简化说明**：

- `isValidationRoot()` 当前省略：SceneUpdateManager 直接管理所有失效块

**未来需实现**：

- `isValidationRoot()` 支持嵌套验证链

### 5.8 事件 API

| g2 API | 职责 | Novadraw 实现 | 状态 |
|--------|------|---------------|------|
| `addMouseListener(l)` | 鼠标监听 | EventDispatcher | ⚠️ 部分 |
| `handleMousePressed(e)` | 鼠标按下处理 | EventDispatcher | ⚠️ 部分 |
| `fireFigureMoved()` | 触发移动事件 | 无 | ⚠️ 简化 |
| `addPropertyChangeListener(l)` | 属性变化监听 | 无 | ⚠️ 简化 |

**简化说明**：

- Listener 模式当前省略：事件在 SceneGraph 层级统一分发
- `fireFigureMoved()` 等事件通知当前省略

**未来需实现**：

- Figure 内部 Listener 机制
- 属性变化事件通知

### 5.9 剪裁 API

| g2 API | 职责 | Novadraw 实现 | 状态 |
|--------|------|---------------|------|
| `getClientArea()` | 获取客户区 | `figure.client_area()` | ✅ |
| `getInsets()` | 获取内边距 | `figure.insets()` | ✅ |
| `clipRect(rect)` | 裁剪矩形 | `NdCanvas.clip_rect()` | ✅ |

**状态**：✅ 完整实现

### 5.10 命中测试 API

| g2 API | 职责 | Novadraw 实现 | 状态 |
|--------|------|---------------|------|
| `containsPoint(x, y)` | 简单边界检测 | `contains_point()` | ✅ |
| `findFigureAt(x, y)` | 找到目标 Figure | `find_figure_at()` | ✅ |
| `findFigureAt(x, y, TreeSearch)` | 带过滤的查找 | 无 | ⚠️ 简化 |
| `findMouseEventTargetAt(x, y)` | 鼠标事件目标 | EventDispatcher 使用 | ✅ |

**简化说明**：

- `TreeSearch` 接口当前省略：直接在 `find_figure_at` 内处理可见性/启用状态

**未来需实现**：

- `TreeSearch` 灵活性（ExclusionSearch 等）

### 5.11 简化决策汇总

| 类别 | 当前简化项 | 未来需实现 |
|------|-----------|------------|
| 层级管理 | `remove()` 不递归通知 | `removeNotify()` 机制 |
| Bounds | `setBounds` 无 erase/fire | 完整副作用链 |
| 状态查询 | 无 `isShowing()` | 递归可见性检查 |
| 布局 | 无 `isValidationRoot()` | 嵌套验证链 |
| 事件 | 无 Figure 内部 Listener | Listener 机制 |
| 命中测试 | 无 `TreeSearch` | 过滤策略 |

### 5.12 g2 核心设计模式继承

| 模式 | g2 实现 | Novadraw 实现 | 状态 |
|------|---------|---------------|------|
| **模板方法** | `paint()` → `paintFigure/paintBorder` | 迭代渲染器 | ✅ |
| **两阶段更新** | Validation → Repaint | `perform_validation` → `repair_damage` | ✅ |
| **迭代遍历** | `paintChildren()` 递归 | `FigureRendererIter` 栈 | ✅ |
| **策略模式** | `LayoutManager`/`TreeSearch` | trait | ✅ |
| **观察者模式** | Listener 列表 | EventDispatcher | ⚠️ 部分 |
| **状态机** | Flag bitset | 布尔字段 | ✅ |

---

## 6. Phase 规划

### Phase 1: 核心完成

| 模块 | 状态 | 说明 |
|------|------|------|
| Figure trait | ✅ 完成 | Bounded → Figure → Shape |
| 基本 Figure | ✅ 完成 | Rectangle、Ellipse、Polygon、Polyline |
| UpdateManager | ✅ 完成 | 两阶段更新 |
| NdCanvas | ⚠️ 部分 | 基础绘图命令完整 |
| Layout | ⚠️ 部分 | 缺少 GridLayout |
| EventDispatcher | ⚠️ 部分 | 需完善捕获/焦点 |
| FigureGraph 重构 | ⚠️ 待开始 | 拆分 graph/state/layout |

### Phase 2: Connection + Layer

| 模块 | 说明 |
|------|------|
| Connection 系统 | Connection + Anchor + Router |
| Layer 系统 | Layer + LayeredPane |

### Phase 3: 高级特性

| 模块 | 说明 |
|------|------|
| SVG 导入 | 矢量图形解析 |
| 导出功能 | PNG/PDF 导出 |
| 动画系统 | 插值动画 |
| 命令历史 | Undo/Redo |

---

## 参考文档

- [Draw2D UpdateManager](../reference/draw2d/rendering/update-manager.md) - g2 UpdateManager 完整分析
- [渲染管线检查清单](../verification/checklists/rendering-pipeline.md) - 渲染管线开发清单
- [Draw2D Figure 核心概念](../reference/draw2d/figure/core-concepts.md) - Figure 核心概念
- [GEF 核心原理](../reference/gef/core-principles.md) - GEF 框架原理
