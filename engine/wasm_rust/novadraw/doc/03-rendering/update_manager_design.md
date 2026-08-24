# UpdateManager 设计文档

## 概述

本文档描述 Novadraw 的 UpdateManager 实现，参考 Eclipse Draw2D (g2) 的设计，并说明从 Java 到 Rust 的迁移决策。

## g2 UpdateManager 机制

### 核心组件

| 组件 | g2 类 | 职责 |
|------|-------|------|
| 更新管理器 | `UpdateManager` | 抽象基类 |
| 延迟更新管理器 | `DeferredUpdateManager` | 具体实现，批量处理更新 |
| 脏区域 | `dirtyRegions: Map<IFigure, Rectangle>` | 需要重绘的区域 |
| 失效队列 | `invalidFigures: List<IFigure>` | 需要重新布局的图形 |
| 更新标志 | `updateQueued: boolean` | 是否有待处理更新 |

### 两阶段更新流程

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                         g2 UpdateManager 流程                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Figure.repaint() ──────────► UpdateManager.addDirtyRegion()                │
│       │                                      │                              │
│       │                                      ▼                              │
│       │                           ┌─────────────────────┐                  │
│       │                           │  dirtyRegions Map   │                  │
│       │                           │  (figure -> rect)  │                  │
│       │                           └─────────────────────┘                  │
│       │                                      │                              │
│       │                               queueWork()                          │
│       │                                      │                              │
│  Figure.revalidate() ───────► UpdateManager.addInvalidFigure()              │
│       │                                      │                              │
│       │                                      ▼                              │
│       │                           ┌─────────────────────┐                  │
│       │                           │  invalidFigures     │                  │
│       │                           │  (List<IFigure>)    │                  │
│       │                           └─────────────────────┘                  │
│       │                                      │                              │
│       │                               queueWork()                          │
│       │                                      │                              │
│       ▼                                                                    │
│  performUpdate() ────────────────► Phase 1: performValidation()             │
│                                              │                              │
│                                              ▼                              │
│                                    ┌─────────────────────┐                │
│                                    │  Phase 2: repair   │                │
│                                    │  Damage()           │                │
│                                    │  - 合并脏区域       │                │
│                                    │  - 坐标变换到父节点 │                │
│                                    │  - root.paint()    │                │
│                                    └─────────────────────┘                │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### repairDamage 核心算法

g2 的 `repairDamage` 有一个关键逻辑：**脏区域坐标变换到父节点**。

```java
// g2 DeferredUpdateManager.java
protected void repairDamage() {
    oldRegions.forEach((figure, contribution) -> {
        IFigure walker = figure.getParent();
        // 脏区域与当前 figure bounds 取交集
        contribution.intersect(figure.getBounds());

        // 向上遍历父节点，变换坐标并取交集
        while (!contribution.isEmpty() && walker != null) {
            walker.translateToParent(contribution);  // 坐标变换到父节点
            contribution.intersect(walker.getBounds());  // 与父节点 bounds 取交集
            walker = walker.getParent();
        }

        // 累加到总 damage
        damage.union(contribution);
    });
}
```

这个设计的目的是：脏区域需要逐级向上传播，只有在每个祖先节点的 bounds 范围内的部分才需要重绘。

## 本项目实现

### 核心概念映射

| g2 概念 | 本项目实现 | 说明 |
|----------|-----------|------|
| `UpdateManager` | `SceneUpdateManager` | 更新管理器 |
| `dirtyRegions` (Map) | `dirty_regions` (HashMap) | 脏区域映射 |
| `invalidFigures` (List) | `invalid_blocks` (Vec) | 失效块队列 |
| `updateQueued` | `update_queued` | 是否有待处理更新 |
| `addDirtyRegion()` | `add_dirty_region()` | 添加脏区域 |
| `addInvalidFigure()` | `add_invalid_figure()` | 添加失效块 |
| `performUpdate()` | `perform_update()` | 执行两阶段更新 |

### 数据结构

```rust
// novadraw-scene/src/runtime/update/deferred.rs

pub struct SceneUpdateManager {
    /// 脏区域映射：block_id -> 脏区域
    pub(crate) dirty_regions: std::collections::HashMap<BlockId, Rectangle>,

    /// 失效块队列
    pub(crate) invalid_blocks: Vec<BlockId>,

    /// 是否有更新待处理
    pub(crate) update_queued: bool,

    /// 是否正在执行更新事务
    pub(crate) updating: bool,

    notification_effects: NotificationQueue,
    listeners: Vec<Box<dyn UpdateListener>>,
}
```

### FigureGraph 协作

`SceneUpdateManager` 不由 `FigureGraph` 持有。组合根同时持有二者，并在更新事务中
把 `&mut FigureGraph` 传给 `UpdateManager::perform_update()`。这样更新服务可以访问
图语义，但不会成为图的内部状态。

### 公开 API

```rust
impl FigureGraph {
    /// 标记块需要重新布局
    pub fn mark_invalid(
        &mut self,
        update_manager: &mut dyn UpdateManager,
        block_id: BlockId,
    );

    /// 请求重绘指定块
    pub fn repaint(
        &mut self,
        update_manager: &mut dyn UpdateManager,
        block_id: BlockId,
        rect: Option<Rectangle>,
    );

    /// 请求重绘整个场景
    pub fn repaint_all(&mut self, update_manager: &mut dyn UpdateManager);

    /// 执行更新（两阶段：布局 + 重绘）
    pub fn perform_update(&mut self, update_manager: &mut dyn UpdateManager) -> NdCanvas;
}
```

## 关键设计决策

### 决策 1: Damage 在 repair phase 传播到根域

dirty region 以所属 Figure 的 bounds 坐标域入队。repair phase 冻结本轮 dirty
snapshot，随后沿 parent chain 应用坐标根 transform 并与祖先 bounds/clip 求交，
最终写入 `DamageSet.regions` 和强约束 `DamageSet.union`。

传播逻辑位于 `runtime/update/repair.rs`，而不是分散到 Figure 或渲染后端。

`DamageSet` 使用显式 `DamageMode`，禁止通过 `union == None` 猜测提交意图：

| 模式 | 含义 | 后端行为 |
|------|------|----------|
| `None` | 本轮没有像素更新 | 不提交、不修改 retained frame |
| `Full` | 首帧、resize、场景替换等强制全量重绘 | 使用完整 surface 作为 damage |
| `Partial` | UpdateManager 计算出的局部 damage | 只复制 `regions` 覆盖的区域 |

直接向空 `NdCanvas` 写入首条绘制命令会把未指定 damage 提升为 `Full`；UpdateManager
则在生成命令前写入 `Partial`，因此不会丢失局部更新语义。

### 决策 2: SceneUpdateManager 是独立系统服务

**g2 方式**：`UpdateManager` 是独立对象，通过 `setRoot(IFigure)` 关联根 Figure。

**本项目方式**：`SceneUpdateManager` 与 `FigureGraph` 由组合根分别持有。

**原因**：

- 避免 FigureGraph 同时成为树和更新服务 owner
- UpdateManager 通过公开图协议协作，不直接访问 SlotMap
- 便于 Headless/Winit/Web 宿主复用同一更新事务

### 决策 3: 调度异步，更新事务同步

**g2 方式**：使用 `Display.asyncExec()` 异步执行更新。

**本项目方式**：`SceneHost::request_update()` 请求平台下一帧；收到 redraw 后同步执行
`perform_update()`。

**原因**：

- 保留 Validation -> Damage Repair 的原子时序
- 由不同 SceneHost 适配 winit、Web 或 headless 调度
- 多次 request 可以由宿主合并

### 决策 4: 合并脏区域的方式

**g2 方式**：在 `repairDamage` 中合并所有脏区域为一个 `damage` 区域。

**本项目方式**：使用 `HashMap<BlockId, Rectangle>`，同一块的脏区域自动合并。

```rust
// 本项目：同一块的脏区域自动合并
if let Some(existing) = self.dirty_regions.get_mut(&block_id) {
    // 扩展区域
    existing.x = existing.x.min(rect.x);
    // ...
} else {
    self.dirty_regions.insert(block_id, rect);
}
```

**原因**：

- g2 需要支持任意 Figure 的脏区域
- 本项目以 Block 为单位，更简单

### 决策 5: 组合根负责触发更新

**g2 方式**：`figure.repaint()` 自动触发 UpdateManager。

**本项目方式**：FigureGraph API 产生 invalid/dirty 工作，组合根检测队列从空到非空
并调用 `SceneHost::request_update()`；redraw 入口调用
`SceneHost::execute_update(scene, update_manager, renderer)`。

**原因**：

- 通用调度语义位于组合根，不散落在 apps
- UpdateManager 不直接依赖平台窗口
- 同一帧内的多次变更可以合并

## API 对比

| g2 API | 本项目 API | 差异 |
|--------|-----------|------|
| `figure.repaint()` | `scene.repaint(update_manager, block_id, rect)` | 图操作显式接收更新服务 |
| `figure.revalidate()` | `scene.mark_invalid(update_manager, block_id)` | 图维护 valid 状态，manager 维护队列 |
| `updateManager.performUpdate()` | `update_manager.perform_update(scene, canvas)` | 两阶段事务 |
| `updateManager.addDirtyRegion(figure, rect)` | `update_manager.add_dirty_region(block_id, rect)` | 使用 BlockId |
| `figure.invalidate()` | `scene.invalidate()` | 使 validation path 失效 |

Figure 事件回调中的 `NovadrawContext::invalidate()` 会先记录请求，待 Figure 不可变
借用释放后统一调用 `FigureGraph::mark_invalid()`。图状态失效和 invalid queue 入队
必须发生在同一个引擎事务边界，禁止只入队而不修改 `is_valid`。

## 使用示例

### 基本使用

```rust
use novadraw_scene::{FigureGraph, RectangleFigure, SceneUpdateManager, UpdateManager};

// 创建场景
let mut scene = FigureGraph::new();
let mut update_manager = SceneUpdateManager::new();
let container = RectangleFigure::new(0.0, 0.0, 200.0, 200.0);
let container_id = scene.set_contents(Box::new(container));

// 添加子块
let child = RectangleFigure::new(10.0, 10.0, 50.0, 50.0);
scene.add_child_to(container_id, Box::new(child));

// 修改块后，触发布局失效
scene.mark_invalid(&mut update_manager, container_id);

// 请求重绘
scene.repaint(&mut update_manager, container_id, None);

// 执行更新并渲染
if update_manager.is_update_queued() {
    let canvas = scene.perform_update(&mut update_manager);
    // ... 渲染到屏幕
}
```

### 批量修改

```rust
// 批量修改多个块
for child_id in children {
    scene.mark_invalid(&mut update_manager, child_id);
    scene.repaint(&mut update_manager, child_id, None);
}

// 一次更新和渲染
let canvas = scene.perform_update(&mut update_manager);
```

### 部分重绘

```rust
// 只重绘块的部分区域（用于小范围更新）
let dirty_rect = Rectangle::new(10.0, 10.0, 50.0, 50.0);
scene.repaint(&mut update_manager, block_id, Some(dirty_rect));
```

## 测试验证

### 单元测试

| 测试用例 | 验证内容 |
|---------|---------|
| `test_dirty_region_tracking` | 添加脏区域后 has_pending_repaint() 返回 true |
| `test_dirty_region_merge` | 同一块的多个脏区域自动合并 |
| `test_invalid_block_queue` | 添加失效块后 has_pending_layout() 返回 true |
| `test_invalid_block_dedup` | 重复添加同一失效块会自动去重 |
| `test_clear` | clear() 清空所有队列 |
| `test_invalid_region` | 无效区域（宽/高为0）被忽略 |

### 集成测试

| 测试用例 | 验证内容 |
|---------|---------|
| `test_add_child_marks_layout_invalid` | add_child 后布局自动失效 |
| `test_mark_invalid_adds_to_queue` | mark_invalid 添加到失效队列 |
| `test_repaints_adds_dirty_region` | repaint 添加脏区域 |
| `test_repaint_uses_specified_rect` | 指定区域重绘而非整个块 |
| `test_multiple_repaints_merge_regions` | 多次重绘合并区域 |
| `test_invisible_block_no_dirty_region` | 不可见块不产生脏区域 |
| `test_perform_update_two_phase` | 两阶段更新正确执行 |
| `test_update_panic_restores_manager_state_and_requeues_invalid_graph_nodes` | panic 后恢复非重入状态并重新排队 |
| `test_validation_figure_effects_preserve_causal_order` | validation 内 Figure effect 保持发生顺序 |
| `test_typed_listeners_dispatch_and_remove_independently` | typed listener 独立分发与移除 |

## 待增强功能

| 功能 | g2 实现 | 当前状态 | 改进方向 |
|------|---------|---------|----------|
| exposed region 输入 | `performUpdate(Rectangle)` | 尚无公开重载 | M5 决定是否纳入核心门禁 |
| 调度策略 | `asyncExec` | SceneHost request-driven | 增加 Web/Headless 实现 |
| Validation root | 支持 | 已验证 | 最高 invalid ancestor、重复失效与 panic 恢复 |
| UpdateListener | 可增删 | 已验证 | `ListenerId` 统一管理 typed listener 生命周期 |

## 参考资料

- Eclipse Draw2D 源码：`org.eclipse.draw2d.UpdateManager`
- Eclipse Draw2D 源码：`org.eclipse.draw2d.DeferredUpdateManager`
- 本项目源码：`novadraw-scene/src/runtime/update/`
