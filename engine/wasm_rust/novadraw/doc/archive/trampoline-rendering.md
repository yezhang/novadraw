# Trampoline 渲染任务管理

类型：`archive`

> **历史方案，非当前主线。** 对应实现已归档到
> `archive/render-iterative-poc-20260617`；当前主线以递归渲染为唯一行为基准。
> 本文只保留方案背景，不应作为当前 API 或 Draw2D 源码事实使用。

## 1. 概述

本方案使用 Trampoline 模式实现 Figure 树的渲染遍历，解决了递归导致的栈溢出问题，同时保持了与 draw2d 相同的扩展能力。

Trampoline 模式的核心思想是：**将所有操作转换为任务（Task），放入队列中顺序执行，而非直接调用**。这与 `async/await` 的原理类似，将调用栈转换为堆上的任务队列。

## 2. 核心设计原则

### 2.1 任务而非调用

```
直接调用（递归）                   Trampoline 模式
    │                                   │
    ├── f()                             ├── 生成任务
    │       │                           │       │
    │       └── f() ────────────────►   │       └── 任务入队
    │               │                   │
    │               └── f() ───────►    └── while pop 任务
    │                                       │
    └── 返回                               └── 执行任务

问题：深度大时栈溢出               优势：堆上队列，无栈溢出风险
```

### 2.2 Figure 职责分离

| 职责     | 负责方           | 说明                          |
| -------- | ---------------- | ----------------------------- |
| 任务生成 | `Figure`         | 定义绘制什么，生成哪些任务    |
| 任务调度 | `FigureRenderer` | 从队列取任务，执行            |
| 状态管理 | `NdCanvas`       | 实际的 gc.save()/restore() 等 |
| 绘制执行 | `FigureRenderer` | 调用 figure.paint_figure() 等 |

### 2.3 模板方法模式

```
Figure Trait
    │
    ├── 任务生成方法（定义"绘制流程"，how）
    │       │
    │       ├── generate_paint_tasks() ── 完整绘制流程
    │       ├── paint_client_area() ───── 客户区域流程
    │       └── paint_children() ──────── 子元素遍历
    │
    └── 直接绘制方法（定义"绘制什么"，what）
            │
            ├── paint_figure() ────────── 绘制自身形状
            ├── paint_border() ────────── 绘制边框
            └── paint_highlight() ─────── 绘制高亮
```

## 3. Figure 方法分类说明

### 3.1 两类方法的区别

Figure Trait 中的方法分为两类，这是核心设计要点：

| 分类 | 方法 | 职责 | 调用时机 |
|------|------|------|----------|
| **任务生成** | `generate_paint_tasks`, `paint_client_area`, `paint_children` | 定义"绘制流程"（how） | 生成任务，延迟执行 |
| **直接绘制** | `paint_figure`, `paint_border`, `paint_highlight` | 定义"绘制什么"（what） | 任务执行时立即调用 gc |

### 3.2 为什么这样设计

**任务生成方法（how）**：
- 属于"流程控制"层面
- 需要被子类 override 以自定义行为
- 如 Viewport 需要修改平移逻辑，Clickable 需要添加额外偏移
- 返回 `Vec<PaintTask>`，描述要做什么

**直接绘制方法（what）**：
- 属于"具体实现"层面
- 定义 Figure 的视觉外观
- 由 `FigureRenderer.execute()` 调用
- 直接操作 gc

```rust
// 任务生成方法 - 定义"绘制流程"（how）
// 子类可 override 自定义流程
fn paint_client_area(
    &self,
    renderer: &mut FigureRenderer<'_>,
    block_id: BlockId,
) -> Vec<PaintTask> {
    // Viewport 可以 override 添加滚动平移
    // Clickable 可以 override 添加按下偏移
}

// 直接绘制方法 - 定义"绘制什么"（what）
// 子类实现具体的视觉外观
fn paint_figure(&self, gc: &mut NdCanvas) {
    // RectangleFigure: 绘制矩形
    // LabelFigure: 绘制文本
    // ShapeFigure: 绘制路径
}
```

### 3.3 与 draw2d 的对应

```
draw2d                          Novadraw
─────────────────────────────────────────────────────
流程控制（how）                  任务生成方法
    │                               │
    ├── paint() final          ───► generate_paint_tasks() final
    ├── paintClientArea()      ───► paint_client_area()
    └── paintChildren()        ───► paint_children()

具体绘制（what）                 直接绘制方法
    │                               │
    ├── paintFigure()          ───► paint_figure()
    ├── paintBorder()          ───► paint_border()
    └── paintHighlight()       ───► paint_highlight()
```

### 3.4 调用时机

```
FigureRenderer.execute(task)
    │
    ├── PaintTask::PaintFigure ──► block.figure.paint_figure(gc)  ◄── 直接调用
    ├── PaintTask::PaintBorder ──► block.figure.paint_border(gc)  ◄── 直接调用
    └── PaintTask::PaintHighlight ──► block.figure.paint_highlight(gc)  ◄── 直接调用

任务生成方法的调用时机：
    │
    └── FigureRenderer.render(root_id)
            │
            └── root.generate_paint_tasks()  ◄── 一次生成完整队列
                    │
                    ├── root.paint_client_area()
                    │       │
                    │       └── child.generate_paint_tasks()  ◄── 递归生成
                    │
                    ├── PaintBorder (root)
                    └── PaintHighlight (root)
```

### 3.5 子类自定义方式

```rust
// 方式一：覆盖任务生成方法（修改流程）
impl Figure for ViewportFigure {
    // 覆盖 paint_client_area，添加滚动平移
    fn paint_client_area(
        &self,
        renderer: &mut FigureRenderer<'_>,
        block_id: BlockId,
    ) -> Vec<PaintTask> {
        let mut tasks = Vec::new();
        // 添加滚动平移...
        tasks.extend(self.paint_children(renderer, block_id));
        tasks
    }
}

// 方式二：覆盖直接绘制方法（修改外观）
impl Figure for RoundedRectangleFigure {
    // 覆盖 paint_figure，绘制圆角矩形
    fn paint_figure(&self, gc: &mut NdCanvas) {
        gc.draw_rounded_rect(self.bounds, self.corner_radius);
    }
}

// 方式三：两者都覆盖（同时修改流程和外观）
impl Figure for CustomFigure {
    fn paint_figure(&self, gc: &mut NdCanvas) {
        // 自定义外观
    }

    fn paint_client_area(&self, renderer: &mut FigureRenderer<'_>, block_id: BlockId) -> Vec<PaintTask> {
        // 自定义流程
    }
}
```

## 3. 核心组件

### 3.1 PaintTask 枚举

所有绘制操作都通过任务表示：

```rust
pub enum PaintTask {
    // ===== 状态管理 =====
    Save,              // gc.save()
    Restore,           // gc.restore()

    // ===== 变换和裁剪 =====
    Translate { x: f64, y: f64 },  // gc.translate(x, y)
    Clip { x: f64, y: f64, w: f64, h: f64 },  // gc.clip_rect(x, y, w, h)

    // ===== 绘制任务 =====
    PaintFigure { block_id: BlockId },     // 绘制自身
    PaintBorder { block_id: BlockId },     // 绘制边框
    PaintHighlight { block_id: BlockId },  // 绘制高亮

    Nop,  // 空任务（占位）
}
```

### 3.2 FigureRenderer

任务调度器，只负责从队列取任务并执行：

```rust
pub struct FigureRenderer<'a> {
    scene: &'a FigureGraph,      // 场景图引用
    gc: &'a mut NdCanvas,       // 渲染输出
    tasks: Vec<PaintTask>,      // 任务队列
}

impl<'a> FigureRenderer<'a> {
    pub fn render(&mut self, root_id: BlockId) {
        // 1. 从根元素生成完整任务队列
        if let Some(root) = self.scene.blocks.get(root_id) {
            self.tasks = root.figure.generate_paint_tasks(self, root_id);
        }

        // 2. 执行任务队列（后进先出）
        while let Some(task) = self.tasks.pop() {
            self.execute(task);
        }
    }

    fn execute(&mut self, task: PaintTask) {
        match task {
            PaintTask::Save => self.gc.save(),
            PaintTask::Restore => self.gc.restore(),
            PaintTask::Translate { x, y } => self.gc.translate(x, y),
            PaintTask::Clip { x, y, w, h } => self.gc.clip_rect(x, y, w, h),

            PaintTask::PaintFigure { block_id } => {
                if let Some(block) = self.scene.blocks.get(block_id) {
                    block.figure.paint_figure(self.gc);
                }
            }
            PaintTask::PaintBorder { block_id } => {
                if let Some(block) = self.scene.blocks.get(block_id) {
                    block.figure.paint_border(self.gc);
                }
            }
            PaintTask::PaintHighlight { block_id } => {
                if let Some(block) = self.scene.blocks.get(block_id) {
                    block.figure.paint_highlight(self.gc);
                }
            }
            PaintTask::Nop => {}
        }
    }

    pub fn block(&self, id: BlockId) -> Option<&FigureBlock> {
        self.scene.blocks.get(id)
    }
}
```

### 3.3 Figure Trait

```rust
pub trait Figure: Send + Sync {
    // ===== 基础信息（必须实现） =====
    fn name(&self) -> &'static str;
    fn bounds(&self) -> Rect;

    // ===== 绘制方法（子类必须实现） =====
    fn paint_figure(&self, gc: &mut NdCanvas);

    // ===== 可选方法（子类可 override） =====
    fn paint_border(&self, _gc: &mut NdCanvas) {}
    fn paint_highlight(&self, _gc: &mut NdCanvas) {}

    // ===== 坐标与裁剪 =====
    fn use_local_coordinates(&self) -> bool { false }
    fn optimize_clip(&self) -> bool { true }
    fn client_area(&self) -> Rect { self.bounds() }
    fn insets(&self) -> (f64, f64, f64, f64) { (0.0, 0.0, 0.0, 0.0) }

    // ===== 辅助方法 =====
    fn is_visible(&self) -> bool { true }
    fn is_selected(&self) -> bool { false }
    fn is_viewport_container(&self) -> bool { false }

    // ===== 模板方法（核心） =====
    //
    // generate_paint_tasks 是 final，不能覆盖
    // 子类应该覆盖 paint_client_area 或 paint_children

    fn generate_paint_tasks(
        &self,
        renderer: &mut FigureRenderer<'_>,
        block_id: BlockId,
    ) -> Vec<PaintTask> {
        let mut tasks = Vec::new();

        // 1. 客户区域（包含子元素）
        tasks.extend(self.paint_client_area(renderer, block_id));

        // 2. 边框
        tasks.push(PaintTask::PaintBorder { block_id });

        // 3. 高亮
        if renderer.block(block_id).map(|b| b.is_selected).unwrap_or(false) {
            tasks.push(PaintTask::PaintHighlight { block_id });
        }

        tasks
    }

    fn paint_client_area(
        &self,
        renderer: &mut FigureRenderer<'_>,
        block_id: BlockId,
    ) -> Vec<PaintTask> {
        let mut tasks = Vec::new();
        let block = match renderer.block(block_id) {
            Some(b) if b.is_visible => b,
            _ => return tasks,
        };

        // 1. 保存状态
        tasks.push(PaintTask::Save);

        // 2. 应用变换
        let trans = block.transform.translation();
        let bounds = self.bounds();
        tasks.push(PaintTask::Translate {
            x: trans.x() + bounds.x,
            y: trans.y() + bounds.y,
        });

        // 3. 裁剪
        if !self.optimize_clip() {
            let area = self.client_area();
            tasks.push(PaintTask::Clip {
                x: area.x,
                y: area.y,
                w: area.width,
                h: area.height,
            });
        }

        // 4. 子元素
        tasks.extend(self.paint_children(renderer, block_id));

        // 5. 恢复状态
        tasks.push(PaintTask::Restore);

        tasks
    }

    fn paint_children(
        &self,
        renderer: &mut FigureRenderer<'_>,
        block_id: BlockId,
    ) -> Vec<PaintTask> {
        let mut tasks = Vec::new();
        let block = match renderer.block(block_id) {
            Some(b) if b.is_visible => b,
            _ => return tasks,
        };

        // 逆序遍历（后添加的在上面）
        for &child_id in block.children.iter().rev() {
            if let Some(child) = renderer.block(child_id) {
                if child.is_visible {
                    // 子元素生成完整任务队列
                    tasks.extend(
                        child.figure.generate_paint_tasks(renderer, child_id)
                    );
                }
            }
        }

        tasks
    }
}
```

## 4. 与 draw2d 的对应关系

| draw2d 方法                 | Novadraw 对应                                     | 说明               |
| --------------------------- | ------------------------------------------------- | ------------------ |
| `paint(Graphics)`（可覆盖） | `generate_paint_tasks()`                          | 历史方案将其收紧为不可覆盖 |
| `paintFigure(Graphics)`     | `PaintTask::PaintFigure` → `paint_figure()`       | 子类必须实现       |
| `paintClientArea(Graphics)` | `paint_client_area()`                             | 子类可 override    |
| `paintChildren(Graphics)`   | `paint_children()`                                | 子类可 override    |
| `graphics.translate()`      | `PaintTask::Translate`                            | 变换               |
| `graphics.pushState()`      | `PaintTask::Save`                                 | 保存状态           |
| `graphics.popState()`       | `PaintTask::Restore`                              | 恢复状态           |
| `graphics.clipRect()`       | `PaintTask::Clip`                                 | 裁剪               |
| `paintBorder(Graphics)`     | `PaintTask::PaintBorder` → `paint_border()`       | 子类可 override    |
| 无 Figure 基类直接对应项    | `PaintTask::PaintHighlight` → `paint_highlight()` | Novadraw 历史方案扩展 |
| `useLocalCoordinates()`     | `use_local_coordinates()`                         | 坐标模式           |
| `optimizeClip()`            | `optimize_clip()`                                 | 裁剪优化           |
| `getClientArea()`           | `client_area()`                                   | 客户区域           |

## 5. 子类自定义示例

### 5.1 RectangleFigure

```rust
impl Figure for RectangleFigure {
    fn name(&self) -> &'static str { "RectangleFigure" }
    fn bounds(&self) -> Rect { self.bounds }
    fn use_local_coordinates(&self) -> bool { true }

    fn paint_figure(&self, gc: &mut NdCanvas) {
        if let Some(color) = self.fill_color {
            gc.set_fill_style(color);
            gc.fill_rect(self.bounds.x, self.bounds.y, self.bounds.width, self.bounds.height);
        }

        if let Some(color) = self.stroke_color {
            gc.set_stroke_style(color);
            gc.set_line_width(self.stroke_width);
            gc.stroke_rect(self.bounds.x, self.bounds.y, self.bounds.width, self.bounds.height);
        }
    }

    // 使用默认 paint_client_area 和 paint_children
}
```

### 5.2 ClickableFigure（按下时平移）

```rust
impl Figure for ClickableFigure {
    fn name(&self) -> &'static str { "ClickableFigure" }
    fn bounds(&self) -> Rect { self.bounds }
    fn use_local_coordinates(&self) -> bool { true }

    fn paint_figure(&self, gc: &mut NdCanvas) {
        gc.set_fill_style(self.fill_color);
        gc.fill_rect(self.bounds.x, self.bounds.y, self.bounds.width, self.bounds.height);
    }

    fn paint_border(&self, gc: &mut NdCanvas) {
        if self.is_selected {
            gc.set_stroke_style(Color::new(0.0, 0.5, 0.8, 1.0));
            gc.set_line_width(2.0);
            gc.stroke_rect(self.bounds.x, self.bounds.y, self.bounds.width, self.bounds.height);
        }
    }

    // 自定义 paint_client_area：按下时平移
    fn paint_client_area(
        &self,
        renderer: &mut FigureRenderer<'_>,
        block_id: BlockId,
    ) -> Vec<PaintTask> {
        let mut tasks = Vec::new();
        let block = match renderer.block(block_id) {
            Some(b) if b.is_visible => b,
            _ => return tasks,
        };

        tasks.push(PaintTask::Save);

        // 按下时额外平移
        if self.is_pressed {
            tasks.push(PaintTask::Translate { x: 1.0, y: 1.0 });
        }

        // 标准变换
        let trans = block.transform.translation();
        let bounds = self.bounds();
        tasks.push(PaintTask::Translate {
            x: trans.x() + bounds.x,
            y: trans.y() + bounds.y,
        });

        if !self.optimize_clip() {
            let area = self.client_area();
            tasks.push(PaintTask::Clip {
                x: area.x, y: area.y, w: area.width, h: area.height,
            });
        }

        tasks.extend(self.paint_children(renderer, block_id));

        tasks.push(PaintTask::Restore);

        tasks
    }
}
```

### 5.3 ViewportFigure（滚动平移）

```rust
impl Figure for ViewportFigure {
    fn name(&self) -> &'static str { "ViewportFigure" }
    fn is_viewport_container(&self) -> bool { true }

    fn paint_figure(&self, _gc: &mut NdCanvas) {
        // 视口容器不绘制自身
    }

    // 自定义 paint_client_area：应用滚动平移
    fn paint_client_area(
        &self,
        renderer: &mut FigureRenderer<'_>,
        block_id: BlockId,
    ) -> Vec<PaintTask> {
        let mut tasks = Vec::new();
        let block = match renderer.block(block_id) {
            Some(b) if b.is_visible => b,
            _ => return tasks,
        };

        tasks.push(PaintTask::Save);

        // 视口滚动平移
        tasks.push(PaintTask::Translate {
            x: -self.view_location.x(),
            y: -self.view_location.y(),
        });

        tasks.extend(self.paint_children(renderer, block_id));

        tasks.push(PaintTask::Restore);

        tasks
    }
}
```

## 6. 任务队列示例

```
RootFigure.generate_paint_tasks()
    │
    ├── PaintTask::Save
    ├── PaintTask::Translate (root.x, root.y)
    ├── PaintTask::Clip (root)
    │
    ├── PaintTask::PaintFigure (root) ──► root.paint_figure()
    │
    ├── ChildFigure.generate_paint_tasks()
    │       │
    │       ├── PaintTask::Save
    │       ├── PaintTask::Translate (child.x, child.y)
    │       ├── PaintTask::PaintFigure (child) ──► child.paint_figure()
    │       │
    │       ├── GrandchildFigure.generate_paint_tasks()
    │       │       ├── PaintTask::Save
    │       │       ├── PaintTask::Translate (gc.x, gc.y)
    │       │       ├── PaintTask::PaintFigure (gc) ──► gc.paint_figure()
    │       │       ├── PaintTask::PaintBorder (gc) ──► gc.paint_border()
    │       │       └── PaintTask::Restore
    │       │
    │       ├── PaintTask::PaintBorder (child) ──► child.paint_border()
    │       └── PaintTask::Restore
    │
    ├── PaintTask::PaintBorder (root) ──► root.paint_border()
    └── PaintTask::Restore
```

执行顺序（后进先出）：

1. Restore (root)
2. PaintBorder (root)
3. Restore (child)
4. PaintBorder (child)
5. Restore (grandchild)
6. PaintFigure (grandchild)
7. Translate (grandchild)
8. Save (grandchild)
9. ...
10. Translate (root)
11. Save (root)

## 7. 执行流程图

```
FigureGraph.render()
    │
    └── FigureRenderer.render(root_id)
            │
            ├── 1. 从根元素生成任务队列
            │       │
            │       └── root.generate_paint_tasks()
            │               │
            │               ├── root.paint_client_area()
            │               │       │
            │               │       ├── Save
            │               │       ├── Translate
            │               │       ├── Clip
            │               │       ├── paint_children()
            │               │       │       │
            │               │       │       └── child.generate_paint_tasks() (递归)
            │               │       │               │
            │               │       │               └── ...
            │               │       └── Restore
            │               │
            │               ├── PaintBorder
            │               └── PaintHighlight
            │
            └── 2. 执行任务队列
                    │
                    ├── PaintTask::PaintFigure ──► paint_figure()
                    ├── PaintTask::PaintBorder ──► paint_border()
                    ├── PaintTask::PaintHighlight ──► paint_highlight()
                    ├── PaintTask::Save ───────────► gc.save()
                    ├── PaintTask::Restore ────────► gc.restore()
                    ├── PaintTask::Translate ─────► gc.translate()
                    └── PaintTask::Clip ──────────► gc.clip_rect()
```

## 8. 设计优势

| 优势           | 说明                         |
| -------------- | ---------------------------- |
| **避免栈溢出** | 任务在堆上，无深度限制       |
| **状态一致**   | gc 状态完全由任务队列控制    |
| **可组合**     | 子任务自然嵌入父任务队列     |
| **可预测**     | 任务执行顺序即状态变化顺序   |
| **易调试**     | 任务序列清晰，可打印日志     |
| **扩展性强**   | 子类可覆盖具体方法自定义行为 |
| **职责清晰**   | 各组件职责单一               |

## 9. 注意事项

### 9.1 禁止直接调用 gc

Figure 中所有 paint 相关方法只能生成任务，不能直接调用 gc：

```rust
// 错误
fn paint_logic(&self, renderer: &mut FigureRenderer<'_>, block_id: BlockId) -> Vec<PaintTask> {
    renderer.gc().save();  // 禁止！
    // ...
}

// 正确
fn paint_logic(&self, renderer: &mut FigureRenderer<'_>, block_id: BlockId) -> Vec<PaintTask> {
    vec![PaintTask::Save]  // 生成任务
}
```

### 9.2 任务入队顺序

子任务需要逆序入队，保证渲染顺序正确：

```rust
for &child_id in block.children.iter().rev() {
    tasks.extend(child.figure.generate_paint_tasks(renderer, child_id));
}
```

### 9.3 Save/Restore 配对

每个 `Save` 必须有对应的 `Restore`：

```rust
tasks.push(PaintTask::Save);
// ... 中间任务
tasks.push(PaintTask::Restore);
```

## 10. 与现有 FigureGraph 集成

```rust
impl FigureGraph {
    pub fn render(&mut self) -> NdCanvas {
        let mut gc = NdCanvas::new();
        let mut renderer = FigureRenderer::new(self, &mut gc);

        let start_id = self.contents.unwrap_or(self.root);
        renderer.render(start_id);

        gc
    }
}
```

## 11. 性能考虑

| 优化点         | 说明                    |
| -------------- | ----------------------- |
| 任务队列预分配 | 避免频繁内存分配        |
| 逆序入队       | 减少一次反转操作        |
| 避免 GC        | 使用枚举而非 trait 对象 |
| 状态复用       | NdCanvas 内部维护状态栈 |
