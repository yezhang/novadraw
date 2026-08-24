//! Figure 渲染接口
//!
//! 定义图形渲染的通用接口，遵循 Eclipse Draw2D 设计模式。
//! Figure 只负责渲染接口，不包含运行时状态（状态在 FigureBlock 中）。
//!
//! # Trait 层级
//!
//! ```text
//! Bounded        - 边界相关方法（bounds, set_bounds, name 等）
//!   |
//!   v
//! Figure         - 渲染接口（继承 Bounded）
//!   |
//!   v
//! Shape          - 描边/填充（继承 Figure）
//! ```

mod ellipse;
mod polygon;
mod polyline;
mod rectangle;
mod root;
mod rounded_rectangle;
mod triangle;

pub mod border;

pub use ellipse::EllipseFigure;
pub use polygon::PolygonFigure;
pub use polyline::PolylineFigure;
pub use rectangle::RectangleFigure;
pub use root::RootFigure;
pub use rounded_rectangle::RoundedRectangleFigure;
pub use triangle::{Direction, TriangleFigure};

use novadraw_core::Color;
use novadraw_geometry::{Rectangle, Translatable};
use novadraw_render::NdCanvas;
use novadraw_render::command::{LineCap, LineJoin};

use crate::{BlockId, FocusEvent, KeyEvent, MouseEvent, NovadrawContext, WheelEvent};
use border::Border;

const DEFAULT_MAXIMUM_DIMENSION: f64 = i32::MAX as f64;

// ============================================================================
// Bounded Trait: 边界相关方法
// ============================================================================

/// 当前 Figure 提供给子树的坐标变换。
///
/// 表达 `child -> parent` 的统一缩放和平移：`parent = child * scale + translate`。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChildTransform {
    /// 统一缩放因子。
    pub scale: f64,
    /// X 方向平移。
    pub translate_x: f64,
    /// Y 方向平移。
    pub translate_y: f64,
}

impl ChildTransform {
    /// 恒等变换。
    pub const IDENTITY: Self = Self {
        scale: 1.0,
        translate_x: 0.0,
        translate_y: 0.0,
    };

    /// 创建只有平移的变换。
    pub const fn translation(translate_x: f64, translate_y: f64) -> Self {
        Self {
            scale: 1.0,
            translate_x,
            translate_y,
        }
    }

    /// 创建统一缩放和平移变换。
    pub const fn uniform(scale: f64, translate_x: f64, translate_y: f64) -> Self {
        Self {
            scale,
            translate_x,
            translate_y,
        }
    }

    /// 应用 `child -> parent` 变换。
    pub fn apply_to<T: Translatable>(self, target: &mut T) {
        target.scale(self.scale);
        target.translate(self.translate_x, self.translate_y);
    }

    /// 应用 `parent -> child` 逆变换。
    pub fn apply_inverse_to<T: Translatable>(self, target: &mut T) {
        target.translate(-self.translate_x, -self.translate_y);
        target.scale(1.0 / self.scale);
    }
}

/// Figure 绘制子节点时使用的裁剪策略。
///
/// 对应 Draw2D `ClippingStrategy` 的核心语义：父 Figure 可以决定
/// `paintChildren` 阶段是否把每个 child 限制在 child bounds 内。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChildClippingStrategy {
    /// 默认策略：先受父 clientArea 限制，再把每个 child 裁剪到自身 bounds。
    ClipToChildBounds,
    /// 只保留父 clientArea 裁剪，不额外裁剪到 child bounds。
    DoNotClipChildBounds,
}

/// 边界相关方法 trait
///
/// 包含图形的边界、名称、位置检测等基础方法。
/// 所有图形类型都需要实现此 trait。
///
/// # 坐标模型契约
///
/// `bounds()` 返回的是**相对于最近坐标根的绝对值**，而不是相对于父节点的偏移。
/// 当父链上出现 `use_local_coordinates() = true` 的节点时，
/// 其后代会切换到新的坐标域。
///
/// `use_local_coordinates()` 只控制 `prim_translate` 是否传播到子节点，
/// 以及渲染时是否对 children 做 `translate(x+left, y+top)`，
/// 同时决定该节点是否为其子树的坐标根。
///
/// 与 g2/draw2d 的对齐点：
/// - 坐标根会在 `translateToParent/FromParent` 时进行 offset 变换
/// - hit-test / repair / render 都必须遵循父链坐标变换协议
pub trait Bounded: Send + Sync {
    /// 获取图形边界
    ///
    /// 默认实现返回零矩形，子类应覆盖
    fn bounds(&self) -> Rectangle;

    /// 设置图形边界
    ///
    /// 对应 draw2d: setBounds(Rectangle)
    /// 注意：本实现只更新 bounds 本身，不触发事件通知
    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64);

    /// 获取名称（用于调试）
    fn name(&self) -> &'static str;

    /// 检查点是否在图形边界内
    ///
    /// 对应 draw2d: containsPoint(int, int)
    fn contains_point(&self, x: f64, y: f64) -> bool {
        let b = self.bounds();
        x >= b.x && x <= b.x + b.width && y >= b.y && y <= b.y + b.height
    }

    /// 检查矩形是否与图形边界相交
    ///
    /// 对应 draw2d: intersects(Rectangle)
    fn intersects(&self, rect: Rectangle) -> bool {
        let b = self.bounds();
        b.x < rect.x + rect.width
            && b.x + b.width > rect.x
            && b.y < rect.y + rect.height
            && b.y + b.height > rect.y
    }

    /// 获取内边距 (top, left, bottom, right)
    fn insets(&self) -> (f64, f64, f64, f64) {
        (0.0, 0.0, 0.0, 0.0)
    }

    /// 是否使用本地坐标
    ///
    /// 对应 draw2d: useLocalCoordinates()
    /// - true: `prim_translate` 不传播到子节点，渲染时子节点会做 translate 变换
    /// - false: 默认模式，`prim_translate` 会传播到所有子孙节点
    ///
    /// 注意：设为 true 后，当前节点会成为其子树的坐标根。
    /// 子节点的 bounds 将处于该坐标根的坐标域中。
    fn use_local_coordinates(&self) -> bool {
        false
    }

    /// 当前 Figure 提供给子树的 `child -> parent` 坐标变换。
    ///
    /// 默认只表达 draw2d `useLocalCoordinates()` 的 client-area 平移；Viewport 等
    /// Figure 可以覆盖此方法，把 content offset / zoom 纳入同一父链协议。
    fn child_transform(&self) -> ChildTransform {
        if self.use_local_coordinates() {
            let bounds = self.bounds();
            let (top, left, _, _) = self.insets();
            ChildTransform::translation(bounds.x + left, bounds.y + top)
        } else {
            ChildTransform::IDENTITY
        }
    }

    /// 获取绘制子节点时使用的裁剪策略。
    ///
    /// 对应 draw2d: `Figure#getClippingStrategy()` / `setClippingStrategy(...)`
    /// 的默认行为。具体 Figure 可以覆盖或暴露 builder 来改变策略。
    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        ChildClippingStrategy::ClipToChildBounds
    }

    // ==================== 布局相关方法 ====================

    /// 获取客户区域
    ///
    /// 对应 draw2d: getClientArea()
    ///
    /// 返回值位于当前 Figure 为其子节点提供的坐标域中：
    /// - `use_local_coordinates() == true` 时，当前 Figure 是子树坐标根，client area 原点重置为 `(0, 0)`；
    /// - 否则 client area 仍位于当前 Figure 所属坐标域，原点为 `bounds.x/y + insets`。
    fn client_area(&self) -> Rectangle {
        let b = self.bounds();
        let (top, left, bottom, right) = self.insets();
        let width = b.width - left - right;
        let height = b.height - top - bottom;
        if self.use_local_coordinates() {
            Rectangle::new(0.0, 0.0, width, height)
        } else {
            Rectangle::new(b.x + left, b.y + top, width, height)
        }
    }

    /// 获取首选大小
    ///
    /// 对应 draw2d: getPreferredSize()
    /// 默认返回 bounds 的尺寸
    fn preferred_size(&self) -> (f64, f64) {
        let b = self.bounds();
        (b.width, b.height)
    }

    /// 获取最小大小
    ///
    /// 对应 draw2d: getMinimumSize()
    /// 默认返回首选大小
    fn minimum_size(&self) -> (f64, f64) {
        self.preferred_size()
    }

    /// 获取最大大小
    ///
    /// 对应 draw2d: getMaximumSize()
    /// 默认不限制布局增长，对齐 Draw2D Figure.MAX_DIMENSION。
    fn maximum_size(&self) -> (f64, f64) {
        (DEFAULT_MAXIMUM_DIMENSION, DEFAULT_MAXIMUM_DIMENSION)
    }
}

// ============================================================================
// Updatable Trait: 更新/验证接口
// ============================================================================

/// 可更新 trait
///
/// 定义图形验证和更新的接口，参考 Eclipse Draw2D 的 IFigure 设计。
/// 负责布局后的验证、失效标记等生命周期管理。
///
/// # 与 FigureGraph 的关系
///
/// - FigureGraph.revalidate() 会调用 Figure.validate()
/// - UpdateManager 跟踪需要验证的块
pub trait Updatable: Send + Sync {
    /// 布局验证
    ///
    /// 对应 draw2d: IFigure.validate()
    /// 在布局计算完成后被调用，用于：
    /// - 预计算依赖布局的几何属性（如 Triangle 顶点）
    /// - 缓存布局相关的计算结果
    ///
    /// 注意：本方法在 FigureGraph.revalidate() 流程中被调用。
    fn validate(&mut self);

    /// 标记为无效
    ///
    /// 对应 draw2d: IFigure.invalidate()
    /// 标记图形需要重新验证。通常由 setBounds() 等操作触发。
    ///
    /// 默认实现为空，子类可覆盖以通知 FigureGraph。
    fn invalidate(&mut self) {}
}

// ============================================================================
// Figure Trait: 渲染接口
// ============================================================================

/// Figure 渲染 trait
///
/// 所有图形对象都需要实现此 trait。
/// 只包含渲染相关方法，边界方法在 Bounded trait 中。
/// 布局验证方法在 Updatable trait 中定义。
///
/// # 渲染流程（参考 Draw2D）
///
/// ```text
/// paint(Graphics) [模板方法]
///   ├─> setLocalBackgroundColor()  [InitProperties]
///   ├─> setLocalForegroundColor()  [InitProperties]
///   ├─> setLocalFont()             [InitProperties]
///   └─> paintFigure()              [PaintSelf]
///         ├─> paintClientArea()    [PaintChildren]
///         │     └─> paintChildren()
///         └─> paintBorder()        [PaintBorder]
/// ```
pub trait Figure: Bounded + Updatable + Send + Sync {
    /// Figure 挂载到父节点后的生命周期 hook。
    ///
    /// 对应 draw2d: addNotify()。
    fn on_attached(&mut self, _parent_id: BlockId) {}

    /// Figure 从父节点移除前的生命周期 hook。
    ///
    /// 对应 draw2d: removeNotify()。
    fn on_detached(&mut self, _parent_id: BlockId) {}

    /// ===== 模板方法 =====
    /// 初始化本地属性
    ///
    /// 对应 draw2d: setLocalBackgroundColor/ForegroundColor/Font
    /// 设置图形的本地渲染属性（颜色、字体等）
    fn init_properties(&self, _gc: &mut NdCanvas) {
        // 默认空实现，子类可覆盖
    }

    /// ===== PaintSelf 阶段方法 =====
    /// 绘制自身（背景）
    ///
    /// 对应 draw2d: paintFigure(Graphics)
    /// 默认空实现，由 Shape trait 覆盖
    fn paint_figure(&self, _gc: &mut NdCanvas) {}

    /// ===== PaintChildren 相关方法 =====
    /// 绘制子元素
    ///
    /// 对应 draw2d paintChildren(Graphics)
    /// 默认行为由渲染器调度 PaintChildren 任务
    fn paint_children(&self) {
        // 默认行为由渲染器处理
    }

    /// ===== PaintBorder 阶段方法 =====
    /// 获取边框
    ///
    /// 对应 draw2d: getBorder()
    fn get_border(&self) -> Option<&dyn Border> {
        None
    }

    /// 绘制边框
    ///
    /// 对应 draw2d: paintBorder(Graphics)
    /// 默认实现调用 Border::paint()
    fn paint_border(&self, gc: &mut NdCanvas) {
        if let Some(border) = self.get_border() {
            border.paint(self.bounds(), gc);
        }
    }

    fn wants_mouse_events(&self) -> bool {
        false
    }

    fn wants_key_events(&self) -> bool {
        false
    }

    fn on_mouse_pressed(&self, _event: &MouseEvent, _ctx: &mut dyn NovadrawContext) -> bool {
        false
    }

    fn on_mouse_released(&self, _event: &MouseEvent, _ctx: &mut dyn NovadrawContext) -> bool {
        false
    }

    fn on_mouse_moved(&self, _event: &MouseEvent, _ctx: &mut dyn NovadrawContext) -> bool {
        false
    }

    fn on_mouse_dragged(&self, _event: &MouseEvent, _ctx: &mut dyn NovadrawContext) -> bool {
        false
    }

    fn on_mouse_hover(&self, _event: &MouseEvent, _ctx: &mut dyn NovadrawContext) -> bool {
        false
    }

    fn on_mouse_double_clicked(&self, _event: &MouseEvent, _ctx: &mut dyn NovadrawContext) -> bool {
        false
    }

    fn on_mouse_wheel(&self, _event: &WheelEvent, _ctx: &mut dyn NovadrawContext) -> bool {
        false
    }

    fn on_key_pressed(&self, _event: &KeyEvent, _ctx: &mut dyn NovadrawContext) -> bool {
        false
    }

    fn on_key_released(&self, _event: &KeyEvent, _ctx: &mut dyn NovadrawContext) -> bool {
        false
    }

    fn on_focus_gained(&self, _event: &FocusEvent, _ctx: &mut dyn NovadrawContext) -> bool {
        false
    }

    fn on_focus_lost(&self, _event: &FocusEvent, _ctx: &mut dyn NovadrawContext) -> bool {
        false
    }

    fn on_mouse_entered(&self, _event: &MouseEvent, _ctx: &mut dyn NovadrawContext) -> bool {
        false
    }

    fn on_mouse_exited(&self, _event: &MouseEvent, _ctx: &mut dyn NovadrawContext) -> bool {
        false
    }
}

// ============================================================================
// Shape Trait: 描边/填充
// ============================================================================

/// Shape 图形 trait
///
/// 参考 Eclipse Draw2D 的 Shape 类设计。
/// 提供描边、填充、透明度等图形通用属性。
///
/// # 渲染流程
///
/// ```text
/// paint_figure()            [覆盖 Figure trait]
///   +-> paint_fill()       [内部方法]
///   |     +-> fill_shape()    [抽象方法]
///   +-> paint_outline()    [内部方法]
///         +-> outline_shape() [抽象方法]
/// ```
pub trait Shape: Figure {
    /// ===== Shape 特有方法 =====
    /// 获取边框装饰器（覆盖 Figure 的默认实现）
    ///
    /// 对应 draw2d: getBorder()
    fn get_border(&self) -> Option<&dyn Border> {
        None
    }

    /// 获取描边颜色
    fn stroke_color(&self) -> Option<Color>;

    /// 获取描边宽度
    fn stroke_width(&self) -> f64;

    /// 获取填充颜色
    fn fill_color(&self) -> Option<Color>;

    /// 获取线帽样式
    fn line_cap(&self) -> LineCap;

    /// 获取线连接样式
    fn line_join(&self) -> LineJoin;

    /// 是否启用填充
    fn fill_enabled(&self) -> bool {
        true
    }

    /// 是否启用描边
    fn outline_enabled(&self) -> bool {
        true
    }

    /// 获取透明度 (0.0 - 1.0)
    fn alpha(&self) -> f64 {
        1.0
    }

    fn wants_mouse_events(&self) -> bool {
        false
    }

    fn wants_key_events(&self) -> bool {
        false
    }

    fn on_mouse_pressed(&self, _event: &MouseEvent, _ctx: &mut dyn NovadrawContext) -> bool {
        false
    }

    fn on_mouse_released(&self, _event: &MouseEvent, _ctx: &mut dyn NovadrawContext) -> bool {
        false
    }

    fn on_mouse_moved(&self, _event: &MouseEvent, _ctx: &mut dyn NovadrawContext) -> bool {
        false
    }

    fn on_mouse_dragged(&self, _event: &MouseEvent, _ctx: &mut dyn NovadrawContext) -> bool {
        false
    }

    fn on_mouse_hover(&self, _event: &MouseEvent, _ctx: &mut dyn NovadrawContext) -> bool {
        false
    }

    fn on_mouse_double_clicked(&self, _event: &MouseEvent, _ctx: &mut dyn NovadrawContext) -> bool {
        false
    }

    fn on_mouse_wheel(&self, _event: &WheelEvent, _ctx: &mut dyn NovadrawContext) -> bool {
        false
    }

    fn on_key_pressed(&self, _event: &KeyEvent, _ctx: &mut dyn NovadrawContext) -> bool {
        false
    }

    fn on_key_released(&self, _event: &KeyEvent, _ctx: &mut dyn NovadrawContext) -> bool {
        false
    }

    fn on_focus_gained(&self, _event: &FocusEvent, _ctx: &mut dyn NovadrawContext) -> bool {
        false
    }

    fn on_focus_lost(&self, _event: &FocusEvent, _ctx: &mut dyn NovadrawContext) -> bool {
        false
    }

    fn on_mouse_entered(&self, _event: &MouseEvent, _ctx: &mut dyn NovadrawContext) -> bool {
        false
    }

    fn on_mouse_exited(&self, _event: &MouseEvent, _ctx: &mut dyn NovadrawContext) -> bool {
        false
    }

    /// ===== 渲染方法 =====
    /// 绘制自身（覆盖 Figure trait 的实现）
    ///
    /// 参考 draw2d: Shape.paintFigure()
    /// 调用 paint_fill() 和 paint_outline()
    fn paint_figure(&self, gc: &mut NdCanvas) {
        self.paint_fill(gc);
        self.paint_outline(gc);
    }

    /// 绘制填充
    ///
    /// 参考 draw2d: paintFill()
    /// 如果 fill_enabled() 为 true，调用 fill_shape()
    fn paint_fill(&self, gc: &mut NdCanvas) {
        if self.fill_enabled() {
            self.fill_shape(gc);
        }
    }

    /// 绘制描边
    ///
    /// 参考 draw2d: paintOutline()
    /// 如果 outline_enabled() 为 true，调用 outline_shape()
    fn paint_outline(&self, gc: &mut NdCanvas) {
        if self.outline_enabled() {
            self.outline_shape(gc);
        }
    }

    /// 填充形状（抽象方法）
    ///
    /// 对应 draw2d: fillShape(Graphics)
    /// 具体图形必须实现此方法
    fn fill_shape(&self, gc: &mut NdCanvas);

    /// 描边形状（抽象方法）
    ///
    /// 对应 draw2d: outlineShape(Graphics)
    /// 具体图形必须实现此方法
    fn outline_shape(&self, gc: &mut NdCanvas);
}

// ============================================================================
// Blanket Impl: 让所有实现 Bounds 的类型自动实现 Figure
// ============================================================================
//
// 设计原理：
// 1. Bounds trait 定义边界相关方法（bounds, set_bounds, name, use_local_coordinates 等）
// 2. Figure trait 继承 Bounds，定义渲染接口（paint_figure, paint_border 等）
// 3. Shape trait 继承 Figure，添加描边/填充属性和 fill_shape/outline_shape 抽象方法
// 4. 所有实现 Bounds 的类型自动获得 Figure 的实现
// 5. Shape 类型会覆盖 paint_figure 实现，调用 paint_fill 和 paint_outline
//
// 关键点：
// - 具体图形类型需要实现 Bounds 和 Shape
// - Shape: Figure，所以所有实现 Shape 的类型也实现 Figure
// - Blanket impl 让所有实现 Shape 的类型自动获得 Figure 实现

/// Blanket Impl：所有实现 Shape trait 的类型自动获得 Figure trait 的实现
///
/// 具体图形类型只需要实现 Shape，不需要显式实现 Figure。
/// Shape 继承 Figure，paint_figure 由 Shape 提供。
impl<T: Shape> Figure for T
where
    T: Bounded,
{
    /// 绘制自身：调用 Shape 的 paint_figure
    ///
    /// 当通过 Box<dyn Figure> 调用时，会正确分派到 Shape 的实现
    fn paint_figure(&self, gc: &mut NdCanvas) {
        Shape::paint_figure(self, gc);
    }

    /// 获取边框：调用 Shape 的 get_border
    ///
    /// 当通过 Box<dyn Figure> 调用时，会正确分派到 Shape 的实现
    fn get_border(&self) -> Option<&dyn super::Border> {
        Shape::get_border(self)
    }

    fn wants_mouse_events(&self) -> bool {
        Shape::wants_mouse_events(self)
    }

    fn wants_key_events(&self) -> bool {
        Shape::wants_key_events(self)
    }

    fn on_mouse_pressed(&self, event: &MouseEvent, ctx: &mut dyn NovadrawContext) -> bool {
        Shape::on_mouse_pressed(self, event, ctx)
    }

    fn on_mouse_released(&self, event: &MouseEvent, ctx: &mut dyn NovadrawContext) -> bool {
        Shape::on_mouse_released(self, event, ctx)
    }

    fn on_mouse_moved(&self, event: &MouseEvent, ctx: &mut dyn NovadrawContext) -> bool {
        Shape::on_mouse_moved(self, event, ctx)
    }

    fn on_mouse_dragged(&self, event: &MouseEvent, ctx: &mut dyn NovadrawContext) -> bool {
        Shape::on_mouse_dragged(self, event, ctx)
    }

    fn on_mouse_hover(&self, event: &MouseEvent, ctx: &mut dyn NovadrawContext) -> bool {
        Shape::on_mouse_hover(self, event, ctx)
    }

    fn on_mouse_double_clicked(&self, event: &MouseEvent, ctx: &mut dyn NovadrawContext) -> bool {
        Shape::on_mouse_double_clicked(self, event, ctx)
    }

    fn on_mouse_wheel(&self, event: &WheelEvent, ctx: &mut dyn NovadrawContext) -> bool {
        Shape::on_mouse_wheel(self, event, ctx)
    }

    fn on_key_pressed(&self, event: &KeyEvent, ctx: &mut dyn NovadrawContext) -> bool {
        Shape::on_key_pressed(self, event, ctx)
    }

    fn on_key_released(&self, event: &KeyEvent, ctx: &mut dyn NovadrawContext) -> bool {
        Shape::on_key_released(self, event, ctx)
    }

    fn on_focus_gained(&self, event: &FocusEvent, ctx: &mut dyn NovadrawContext) -> bool {
        Shape::on_focus_gained(self, event, ctx)
    }

    fn on_focus_lost(&self, event: &FocusEvent, ctx: &mut dyn NovadrawContext) -> bool {
        Shape::on_focus_lost(self, event, ctx)
    }

    fn on_mouse_entered(&self, event: &MouseEvent, ctx: &mut dyn NovadrawContext) -> bool {
        Shape::on_mouse_entered(self, event, ctx)
    }

    fn on_mouse_exited(&self, event: &MouseEvent, ctx: &mut dyn NovadrawContext) -> bool {
        Shape::on_mouse_exited(self, event, ctx)
    }
}
