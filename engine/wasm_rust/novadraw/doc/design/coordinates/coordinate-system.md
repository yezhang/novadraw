# Novadraw 坐标系统原理

类型：`normative-design`

## 1. 坐标系统概述

Novadraw 绘图引擎涉及多种坐标系统，理解它们之间的转换关系是掌握引擎工作原理的关键。

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           坐标系统层级                                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────┐                                                     │
│  │ 屏幕像素坐标        │  Physical Pixels (winit 原始事件)                    │
│  │ (screen_x, screen_y)│  - 物理像素单位                                      │
│  │                     │  - 显示器实际像素                                     │
│  └──────────┬──────────┘                                                      │
│             │                                                                │
│             │  ÷ scale_factor                                                │
│             ▼                                                                │
│  ┌─────────────────────┐                                                     │
│  │ 入口域逻辑坐标      │  Entry-domain Logical Coordinates                    │
│  │ (entry_x, entry_y)  │  - 与设备无关的坐标                                  │
│  │                     │  - 缩放因子调整后的坐标                              │
│  └──────────┬──────────┘                                                      │
│             │                                                                │
│             │  命中测试时按父链 translateFromParent 降域                     │
│             ▼                                                                │
│  ┌─────────────────────┐                                                     │
│  │ Figure 所属坐标域   │  Target Figure Coordinate Domain                      │
│  │ (x, y)              │  - Figure 回调收到的 MouseEvent.x/y                  │
│  │                     │  - 由引擎层 SceneDispatchContext 转换                 │
│  └──────────┬──────────┘                                                      │
│             │                                                                │
│             │  Figure bounds (x, y)                                          │
│             ▼                                                                │
│  ┌─────────────────────┐                                                     │
│  │ 坐标根分段值        │  Bounds relative to nearest coordinate root           │
│  │ (bounds.x, y)       │  - 相对最近坐标根的绝对值                            │
│  │                     │  - 不是应用层或全局坐标                              │
│  └─────────────────────┘                                                      │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## 2. 坐标转换公式

### 2.1 屏幕像素 → 逻辑像素

```rust
logical_x = physical_x / scale_factor
logical_y = physical_y / scale_factor
```

**应用场景**：winit 鼠标事件处理

```rust
let scale_factor = window.scale_factor();
let logical_x = position.x / scale_factor;
let logical_y = position.y / scale_factor;
let mouse_pos = DVec2::new(logical_x, logical_y);
```

### 2.2 入口域逻辑坐标 → Figure 所属坐标域

```rust
let mut point = Point::new(entry_x, entry_y);
graph.translate_to_relative(target_id, &mut point);
let event = MouseEvent::new(kind, entry_x, entry_y, button)
    .with_target_point(point.x(), point.y());
```

**应用场景**：引擎层 `SceneDispatchContext` 在投递事件前，将入口域鼠标点转换为 target/source Figure 所属坐标域中的点。

### 2.3 Figure 所属坐标域 → 入口域逻辑坐标

```rust
let mut point = Point::new(target_x, target_y);
graph.translate_to_absolute_mut(target_id, &mut point);
```

**应用场景**：调试、录制回放、跨 target 手势分析等需要回到入口域时使用。

### 2.4 Figure 所属坐标域 → 渲染输出坐标

```rust
// 渲染时：GC 的变换栈累积变换
render_point = cumulative_transform.multiply_point_2d(figure_domain_point)
output_x = render_point.x * scale_factor
output_y = render_point.y * scale_factor
```

**应用场景**：Vello 渲染器绘制

### 2.5 Viewport 坐标域 ↔ Content 坐标域

```rust
content = viewport_point / zoom + origin
viewport_point = (content - origin) * zoom
```

**应用场景**：Viewport/Scroll 作为坐标系统节点时，`origin` 表示 viewport 左上角对应的 content 坐标。它必须通过 `translateToParent/translateFromParent` 协议接入 Figure 父链，不能恢复旧式全局空间特判。

## 3. 变换矩阵

### 3.1 Transform 类型

Novadraw 使用 `DMat4`（4x4 齐次坐标矩阵）存储变换：

```rust
pub struct Transform {
    matrix: DMat4,
}
```

**优势**：
- 支持 2D 和 3D 变换
- 变换累积无精度损失
- 未来可扩展到 3D 渲染

### 3.2 变换操作

```rust
// 平移
let translate = Transform::from_translation_2d(dx, dy);

// 缩放
let scale = Transform::from_scale_2d(sx, sy);

// 旋转（绕原点）
let rotate = Transform::from_rotation_z(angle);

// 绕点旋转/缩放
let rotate_around = Transform::from_rotation_around(angle, center);
let scale_around = Transform::from_scale_around(sx, sy, center);

// 组合（矩阵乘法）
let combined = transform1 * transform2;

// 逆变换
let inverse = transform.inverse();
```

### 3.3 变换顺序

**重要**：变换组合顺序影响结果

```rust
// 新变换在右边：先应用当前变换，再应用新变换
self.transform = self.transform * translate;

// 示例：从 (0,0) 平移 100px，再平移 50px
// 结果 = T(50) * T(100) = T(150)
// 点 P(10,20) -> T(100)*P = (110,120) -> T(50)*(110,120) = (160,140)
// 正确：先移动到 (110,120)，再移动到 (160,140)
```

## 4. RenderContext 变换栈

### 4.1 Draw2D 模式

遵循 Eclipse Draw2D 的变换管理模式：

```rust
pub struct RenderContext {
    commands: Vec<RenderCommand>,
    current_fill: Option<Color>,
    current_stroke: Option<(Color, f64)>,
    transform_stack: TransformStack,
}
```

### 4.2 渲染时的坐标处理

与 g2 一致，渲染时使用 Graphics 状态栈管理变换：

```rust
// 渲染遍历时（参考 g2 的 paint 流程）
gc.push_state();              // 保存状态
figure.paint_figure(gc);     // 绘制自身
gc.pop_state();              // 恢复状态
```

### 4.3 Figure 坐标系

Figure.bounds 存储的是**相对最近坐标根的绝对值**，渲染时直接使用 bounds 中的坐标：

```rust
// Figure 绘制时直接使用 bounds 坐标（相对最近坐标根的绝对值）
fn paint_figure(&self, gc: &mut NdCanvas) {
    gc.fill_rect(
        self.bounds.x,
        self.bounds.y,
        self.bounds.width,
        self.bounds.height,
        self.fill_color,
    );
}
```

## 5. 拖拽实现原理

与 g2 一致，拖拽通过直接修改 Figure.bounds 实现，不使用独立的 transform 字段。

### 5.1 拖拽状态记录

```rust
struct DragState {
    block_id: BlockId,
    start_pos: DVec2,           // 鼠标开始位置（逻辑像素）
    start_bounds: Rectangle,     // 拖拽开始时的 bounds
}
```

### 5.2 拖拽时位置更新

```rust
// 鼠标移动时
let dx = current_pos.x - drag_state.start_pos.x;
let dy = current_pos.y - drag_state.start_pos.y;

// 直接修改 Figure.bounds（与 g2 primTranslate 一致）
scene.prim_translate(block_id, dx, dy);
```

### 5.3 渲染时坐标应用

渲染直接使用 Figure.bounds，无需额外变换：

```
Figure.paint_figure():
    │
    ├── gc.fill_rect(bounds.x, bounds.y, bounds.width, bounds.height)
    │       │
    │       └── 直接使用 bounds 坐标
    │
    └── gc.stroke_rect(...)  // 描边
```

## 6. HiDPI 坐标处理

### 6.1 坐标流

```
winit 事件 (物理像素)
        │
        ▼
    ÷ scale_factor
        │
        ▼
   入口域逻辑坐标 ← apps 只负责平台输入适配
        │
        ├── 引擎层 SceneDispatchContext 负责 hit-test 与事件点降域
        │
        └── render → * scale_factor
                   │
                   ▼
            Vello 渲染（物理像素）
```

### 6.2 Vello 渲染器

```rust
pub struct VelloRenderer {
    scale_factor: f64,
}

fn render_command(scene: &mut Scene, cmd: &RenderCommand, scale: f64) {
    // 将逻辑像素转换为物理像素
    let x0 = rect[0].x * scale;
    let y0 = rect[0].y * scale;
    let x1 = rect[1].x * scale;
    let y1 = rect[1].y * scale;

    let kurbo_rect = vello::kurbo::Rect::new(x0, y0, x1, y1);
    scene.fill(Fill::NonZero, Affine::IDENTITY, color, None, &kurbo_rect);
}
```

## 7. 3D 扩展设计

### 7.1 齐次坐标

使用 4x4 矩阵为 3D 预留：

```
[x']   [m00 m01 m02 m03]   [x]
[y'] = [m10 m11 m12 m13] * [y]
[z']   [m20 m21 m22 m23]   [z]
[w]    [m30 m31 m32 m33]   [1]
```

### 7.2 坐标类型

```rust
// 当前 2D
pub type Point = DVec2;

// 未来 3D 扩展（只需修改类型别名）
// pub type Point = DVec3;

// 始终使用 Transform（已是 4x4 矩阵）
pub struct Transform {
    matrix: DMat4,
}
```

### 7.3 3D 扩展接口

```rust
// 当前 2D 方法
fn multiply_point_2d(&self, point: DVec2) -> DVec2;

// 未来 3D 方法
fn multiply_point_3d(&self, point: DVec3) -> DVec3;
fn multiply_point_3d(&self, point: DVec3) -> DVec3;
```

## 8. 历史原型代码（非当前实现）

本节代码来自早期单 crate 原型，只保留用于解释坐标计算的演进背景，不是当前 API
或文件布局。当前实现入口如下：

| 职责 | 当前入口 |
|---|---|
| 仿射变换 | `novadraw-geometry/src/transform.rs` |
| 绘图状态与命令记录 | `novadraw-render/src/context.rs` |
| Figure 与坐标边协议 | `novadraw-scene/src/figure/mod.rs` |
| 父链转换、命中与事件状态 | `novadraw-scene/src/graph/mod.rs` |
| target 事件点降域 | `novadraw-scene/src/runtime/context.rs` |
| Vello 后端 | `novadraw-render/src/backend/vello/` |
| 平台输入适配 | `novadraw-apps/src/input.rs`、`apps/editor/src/app_window.rs` |

后续维护坐标契约时应修改前述当前入口和契约测试，不得从下列旧代码恢复第二套
Transform、RenderContext 或应用层命中逻辑。

### 8.1 旧 Transform 原型

```rust
use glam::{DMat4, DVec2, DVec3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    matrix: DMat4,
}

impl Transform {
    pub fn identity() -> Self {
        Self {
            matrix: DMat4::IDENTITY,
        }
    }

    pub fn from_translation_2d(tx: f64, ty: f64) -> Self {
        Self {
            matrix: DMat4::from_translation(DVec3::new(tx, ty, 0.0)),
        }
    }

    pub fn from_scale_2d(sx: f64, sy: f64) -> Self {
        Self {
            matrix: DMat4::from_scale(DVec3::new(sx, sy, 1.0)),
        }
    }

    pub fn from_rotation_z(angle: f64) -> Self {
        Self {
            matrix: DMat4::from_rotation_z(angle),
        }
    }

    pub fn compose(&self, other: &Self) -> Self {
        Self {
            matrix: other.matrix * self.matrix,
        }
    }

    pub fn multiply_point_2d(&self, point: DVec2) -> DVec2 {
        let transformed = self.matrix.mul_vec4(DVec3::new(point.x, point.y, 1.0).extend(1.0));
        DVec2::new(transformed.x, transformed.y)
    }

    pub fn inverse(&self) -> Self {
        Self {
            matrix: self.matrix.inverse(),
        }
    }

    pub fn to_mat4(&self) -> DMat4 {
        self.matrix
    }
}

impl std::ops::Mul for Transform {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        self.compose(&other)
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::identity()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TransformStack {
    stack: Vec<Transform>,
}

impl TransformStack {
    pub fn new() -> Self {
        Self {
            stack: vec![Transform::identity()],
        }
    }

    pub fn push(&mut self, transform: Transform) {
        let current = self.current();
        self.stack.push(current.compose(&transform));
    }

    pub fn pop(&mut self) -> Option<Transform> {
        if self.stack.len() > 1 {
            Some(self.stack.pop().unwrap())
        } else {
            None
        }
    }

    pub fn current(&self) -> Transform {
        self.stack.last().copied().unwrap_or(Transform::identity())
    }

    pub fn reset(&mut self) {
        self.stack.clear();
        self.stack.push(Transform::identity());
    }
}

impl Default for TransformStack {
    fn default() -> Self {
        Self::new()
    }
}
```

### 8.2 旧 RenderContext 原型

```rust
use crate::color::Color;
use crate::render_ir::RenderCommand;
use crate::transform::{Transform, TransformStack};
use glam::DVec2;

pub struct RenderContext {
    pub commands: Vec<RenderCommand>,
    current_fill: Option<Color>,
    current_stroke: Option<(Color, f64)>,
    transform_stack: TransformStack,
}

impl RenderContext {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            current_fill: None,
            current_stroke: None,
            transform_stack: TransformStack::new(),
        }
    }

    pub fn set_fill_style(&mut self, color: Color) {
        self.current_fill = Some(color);
    }

    pub fn set_stroke_style(&mut self, color: Color, width: f64) {
        self.current_stroke = Some((color, width));
    }

    pub fn draw_rect(&mut self, x: f64, y: f64, width: f64, height: f64) {
        let color = self.current_fill.take();
        let stroke = self.current_stroke.take();

        self.commands.push(RenderCommand::FillRect {
            rect: [DVec2::new(x, y), DVec2::new(x + width, y + height)],
            color,
            stroke_color: stroke.map(|s| s.0),
            stroke_width: stroke.map(|s| s.1).unwrap_or(0.0),
        });
    }

    pub fn push_transform(&mut self, transform: Transform) {
        self.transform_stack.push(transform);
    }

    pub fn pop_transform(&mut self) {
        self.transform_stack.pop();
    }

    pub fn current_transform(&self) -> Transform {
        self.transform_stack.current()
    }

    pub fn transform_point(&self, point: DVec2) -> DVec2 {
        self.transform_stack.current().multiply_point_2d(point)
    }
}
```

### 8.3 旧 FigureBlock 摘要

与 g2 设计一致，FigureBlock 存储运行时状态，Figure 存储几何定义：

```rust
pub struct FigureBlock {
    pub id: BlockId,
    pub uuid: Uuid,
    pub parent: Option<BlockId>,      // 父节点
    pub children: Vec<BlockId>,      // 子节点（Z-order 由数组顺序决定）
    pub figure: Box<dyn Figure>,      // 图形几何数据
    pub layout_manager: Option<Arc<dyn LayoutManager>>, // 布局管理器
    pub is_selected: bool,
    pub is_visible: bool,
    pub is_enabled: bool,
    pub preferred_size: Option<(f64, f64)>,
    pub minimum_size: Option<(f64, f64)>,
    pub maximum_size: Option<(f64, f64)>,
}
```

### 8.4 旧 Figure Trait 摘要

Figure 存储几何定义（bounds），与 g2 Figure 一致：

```rust
pub trait Bounded {
    fn bounds(&self) -> Rectangle;
    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64);
}

pub trait Figure: Bounded {
    fn paint_figure(&self, gc: &mut NdCanvas);
    fn paint_border(&self, gc: &mut NdCanvas);
}
```

### 8.5 旧 RectangleFigure 摘要

```rust
pub struct RectangleFigure {
    pub bounds: Rectangle,    // 位置信息存储在这里
    pub fill_color: Color,
    pub stroke_color: Option<Color>,
    pub stroke_width: f64,
}

impl Bounded for RectangleFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }
}

impl Shape for RectangleFigure {
    fn fill_shape(&self, gc: &mut NdCanvas) {
        // 直接使用 bounds 坐标绘制
        gc.fill_rect(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
            self.fill_color,
        );
    }

    fn outline_shape(&self, gc: &mut NdCanvas) {
        // 描边实现...
    }
}
```

### 8.6 旧 VelloRenderer 原型

```
use std::sync::Arc;
use vello::kurbo::{Affine, Stroke};
use vello::peniko::Color;
use vello::util::{RenderContext, RenderSurface};
use vello::{AaConfig, Renderer, RendererOptions};
use winit::dpi::PhysicalSize;
use winit::window::Window;

use crate::render_ir::RenderCommand;

pub struct VelloRenderer {
    render_context: RenderContext,
    renderers: Vec<Option<Renderer>>,
    scene: Scene,
    surface: RenderSurface<'static>,
    window: Arc<Window>,
    scale_factor: f64,
}

type Scene = vello::Scene;

impl VelloRenderer {
    pub fn new(window: Arc<Window>, logical_width: f64, logical_height: f64) -> Self {
        let scale_factor = window.scale_factor();
        let width = (logical_width * scale_factor) as u32;
        let height = (logical_height * scale_factor) as u32;

        let mut render_context = RenderContext::new();
        let size = PhysicalSize::new(width, height);
        let surface_future = render_context.create_surface(
            Arc::clone(&window),
            size.width,
            size.height,
            vello::wgpu::PresentMode::AutoVsync,
        );
        let surface = pollster::block_on(surface_future).expect("Failed to create surface");

        let mut renderers = vec![];
        renderers.resize_with(render_context.devices.len(), || None);
        renderers[surface.dev_id]
            .get_or_insert_with(|| create_renderer(&render_context, &surface));

        VelloRenderer {
            render_context,
            renderers,
            scene: Scene::new(),
            surface,
            window,
            scale_factor,
        }
    }

    pub fn render(&mut self, commands: &[RenderCommand]) {
        self.scene.reset();

        let scale = self.scale_factor;
        for cmd in commands {
            Self::render_command(&mut self.scene, cmd, scale);
        }

        let device_handle = &self.render_context.devices[self.surface.dev_id];
        let width = self.surface.config.width;
        let height = self.surface.config.height;

        self.renderers[self.surface.dev_id]
            .as_mut()
            .unwrap()
            .render_to_texture(
                &device_handle.device,
                &device_handle.queue,
                &self.scene,
                &self.surface.target_view,
                &vello::RenderParams {
                    base_color: Color::new([0.0, 0.0, 0.0, 0.0]),
                    width,
                    height,
                    antialiasing_method: AaConfig::Msaa16,
                },
            )
            .expect("Failed to render to texture");

        let surface_texture = self
            .surface
            .surface
            .get_current_texture()
            .expect("Failed to get surface texture");

        let mut encoder = device_handle
            .device
            .create_command_encoder(&vello::wgpu::CommandEncoderDescriptor {
                label: Some("Surface Blit"),
            });

        self.surface.blitter.copy(
            &device_handle.device,
            &mut encoder,
            &self.surface.target_view,
            &surface_texture
                .texture
                .create_view(&vello::wgpu::TextureViewDescriptor::default()),
        );

        device_handle.queue.submit([encoder.finish()]);
        surface_texture.present();
    }

    fn render_command(scene: &mut Scene, cmd: &RenderCommand, scale: f64) {
        match cmd {
            RenderCommand::FillRect { rect, color, stroke_color, stroke_width } => {
                let x0 = rect[0].x * scale;
                let y0 = rect[0].y * scale;
                let x1 = rect[1].x * scale;
                let y1 = rect[1].y * scale;
                let kurbo_rect = vello::kurbo::Rect::new(x0, y0, x1, y1);

                let vello_color = color.map(|c| {
                    Color::new([c.r as f32, c.g as f32, c.b as f32, c.a as f32])
                }).unwrap_or_else(|| Color::new([0.2, 0.6, 0.86, 1.0]));

                let alpha = vello_color.components[3];
                if alpha == 0.0 {
                    let stroke_vello_color = stroke_color.map(|c| {
                        Color::new([c.r as f32, c.g as f32, c.b as f32, c.a as f32])
                    }).unwrap_or_else(|| Color::new([0.95, 0.61, 0.07, 1.0]));

                    let stroke_width = *stroke_width;
                    if stroke_width > 0.0 {
                        let inset = stroke_width / 2.0;
                        let stroke_rect = vello::kurbo::Rect::new(
                            x0 + inset, y0 + inset,
                            x1 - inset, y1 - inset,
                        );
                        scene.stroke(
                            &Stroke::new(stroke_width as f64),
                            Affine::IDENTITY,
                            stroke_vello_color,
                            None,
                            &stroke_rect,
                        );
                    }
                } else {
                    scene.fill(
                        vello::peniko::Fill::NonZero,
                        Affine::IDENTITY,
                        vello_color,
                        None,
                        &kurbo_rect,
                    );
                }
            }
        }
    }

    pub fn window(&self) -> &Arc<Window> {
        &self.window
    }
}
```

### 8.7 旧应用层鼠标处理原型

当前事件命中、target 固定和事件点降域已下沉引擎层。以下代码只说明早期拖拽
原型，不能作为当前应用职责示例：

```
struct DragState {
    block_id: BlockId,
    start_pos: DVec2,
    start_bounds: Rectangle,  // 记录拖拽开始时的 bounds
}

// 鼠标移动事件
WindowEvent::CursorMoved { position, .. } => {
    let scale_factor = renderer.window().scale_factor();
    let logical_x = position.x / scale_factor;
    let logical_y = position.y / scale_factor;
    let current_pos = DVec2::new(logical_x, logical_y);

    if tool == Tool::Select {
        if let Some(drag_state) = &self.drag_state {
            // 计算位移
            let dx = current_pos.x - drag_state.start_pos.x;
            let dy = current_pos.y - drag_state.start_pos.y;

            // 直接修改 Figure.bounds（与 g2 primTranslate 一致）
            scene_manager.scene_mut().prim_translate(drag_state.block_id, dx, dy);
        } else if self.selection_state.is_some() {
            scene_manager.update_selection_box(current_pos);
        } else {
            let hit_id = scene_manager.scene().hit_test(current_pos);
            scene_manager.set_hovered(hit_id);
        }
    }
}

// 鼠标按下事件
WindowEvent::MouseInput { button: MouseButton::Left, state, .. } => {
    if let Some(mouse_pos) = self.last_mouse_pos {
        if tool == Tool::Select {
            let hover_id = scene_manager.scene().hit_test(mouse_pos);
            if let Some(id) = hover_id {
                scene_manager.scene_mut().set_selected(Some(id));

                // 记录拖拽开始时的 bounds
                let start_bounds = scene_manager.scene()
                    .get_block(id)
                    .map(|b| b.figure.bounds());

                self.drag_state = Some(DragState {
                    block_id: id,
                    start_pos: mouse_pos,
                    start_bounds,
                });
            }
        }
    }
}
```

## 9. 常见问题

### Q: 拖拽时坐标错位？

A: 检查以下可能：
1. 变换累积方向是否正确（`transform * translate`）
2. `start_transform` 是否在拖拽开始时正确保存
3. 鼠标坐标是否除以了 `scale_factor`

### Q: 高清屏上图形模糊？

A: 确保 Vello 渲染器正确处理了 `scale_factor`：
1. Surface 创建时使用物理像素尺寸
2. RenderCommand 坐标乘以 `scale_factor` 后传给 Vello

### Q: hit_test 不准确？

A: 确保入口域点与父链降域协议一致：
1. apps 只将物理像素除以 `scale_factor`，得到入口域逻辑坐标
2. `FigureGraph::find_mouse_event_target_at()` 在引擎层按 `translateFromParent` 逐层降域
3. `SceneDispatchContext` 在投递前将 `MouseEvent.x/y` 转换到 target/source Figure 所属坐标域

## 10. 总结

```
┌─────────────────────────────────────────────────────────────────┐
│                     坐标转换总览                                 │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  鼠标事件 → 物理像素                                            │
│       ↓                                                         │
│  ÷ scale_factor                                                 │
│       ↓                                                         │
│  入口域逻辑坐标 ← apps 只负责平台输入适配                       │
│       ↓                                                         │
│  父链 translateFromParent 降域 ← 引擎层 hit-test / dispatch     │
│       ↓                                                         │
│  Figure 所属坐标域中的 MouseEvent.x/y                          │
│       ↓                                                         │
│  相对最近坐标根的绝对值 ← Figure 使用                              │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**核心原则**：
1. Figure.bounds 存储的是相对最近坐标根的绝对值，渲染直接使用
2. 鼠标入口点始于入口域逻辑坐标，命中测试和事件分发的降域由引擎层负责
3. Figure 回调收到的 `MouseEvent.x/y` 已处于 target/source Figure 所属坐标域
4. 位置变更通过 Figure.set_bounds() 修改 bounds（与 g2 一致）
5. 不使用独立的 transform 字段存储运行时变换
6. 坐标转换使用 f64 双精度

## 11. Trampoline 渲染流水线坐标系

### 11.1 渲染流程中的坐标系

与 g2 一致，渲染直接使用 Figure.bounds，无需额外的 transform：

```
┌─────────────────────────────────────────────────────────────────┐
│                     Trampoline 渲染流水线                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   Figure.bounds              相对最近坐标根的绝对值                 │
│   (Rectangle.x, y)           (无需额外平移即可直接参与渲染)         │
│         │                                                     │
│         ▼                                                     │
│   RenderCommand              逻辑坐标                            │
│   (NdCanvas commands)        rect 使用 bounds 坐标              │
│         │                                                     │
│         ▼                                                     │
│   Vello Backend              像素坐标                            │
│   (最终渲染)                  乘以 scale_factor                   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 11.2 各层坐标系说明

| 层级 | 类型 | 坐标系 | 说明 |
|------|------|--------|------|
| **Figure.bounds** | 几何定义 | 相对最近坐标根的绝对值 | Rectangle 的 x, y 相对于最近坐标根 |
| **RenderCommand** | 命令 | 逻辑坐标 | rect 直接使用 bounds 坐标 |
| **Vello backend** | 渲染 | 像素坐标 | 最终需要乘以 scale_factor |

### 11.3 NdCanvas 与 RenderCommand

NdCanvas 维护变换状态栈，每个命令创建时携带当前变换：

```
fn create_command(&mut self, kind: RenderCommandKind) {
    let command = RenderCommand {
        kind,
        transform: self.current_state().transform,  // 当前累积变换
    };
    self.commands.push(command);
}
```

变换累积通过 `translate()` 方法：

```
pub fn translate(&mut self, x: f64, y: f64) {
    let t = Transform::from_translation(x, y);
    self.current_state_mut().transform = self.current_state().transform * t;
}
```

### 11.4 Vello 后端坐标转换

逻辑坐标到像素坐标的转换在 Vello 后端统一完成：

```
fn render_command(scene: &mut Scene, cmd: &RenderCommand, scale_factor: f64) {
    // Transform 平移乘以 scale_factor
    let affine = vello::kurbo::Affine::new([
        a, b,
        c, d,
        e * scale_factor, f * scale_factor,  // 平移转换为像素坐标
    ]);

    match &cmd.kind {
        RenderCommandKind::FillRect { rect, .. } => {
            // 矩形坐标也乘以 scale_factor
            let x0 = rect[0].x * scale_factor;
            let y0 = rect[0].y * scale_factor;
            let x1 = rect[1].x * scale_factor;
            let y1 = rect[1].y * scale_factor;
            let kurbo_rect = vello::kurbo::Rect::new(x0, y0, x1, y1);
            scene.fill(Fill::NonZero, affine, color, None, &kurbo_rect);
        }
    }
}
```

### 11.5 典型场景示例

与 g2 一致，渲染直接使用 Figure.bounds：

```
场景: 800×600 窗口，2x DPI，矩形 (700, 550) 100×50

Figure.bounds           →  (700, 550, 100, 50) 逻辑坐标
                         →  Rectangle.x=700, Rectangle.y=550
                         →  Rectangle.width=100, Rectangle.height=50

RenderCommand.rect      →  [700, 550], [800, 600] 逻辑坐标

Vello backend           →  rect: [1400, 1100], [1600, 1200] 像素坐标
                         →  × 2 (scale_factor)
```

### 11.6 坐标系设计原则

1. **逻辑坐标统一**：从 Figure 到 RenderCommand 全程使用逻辑坐标
2. **DPI 隔离**：Vello 后端负责逻辑坐标到像素坐标的转换
3. **与 g2 一致**：位置存储在 Figure.bounds 中，拖拽通过 set_bounds 实现
4. **无独立 transform**：不使用独立的 transform 字段存储运行时变换
