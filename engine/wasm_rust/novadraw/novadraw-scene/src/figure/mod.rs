//! Figure 渲染接口
//!
//! 定义图形渲染的通用接口，遵循 Eclipse Draw2D 设计模式。
//! Figure 只负责具体图形行为；公共节点状态由 FigureNode/NodeState 承载。
//!
//! # Trait 层级
//!
//! ```text
//! Figure                  - 绘制与精确几何
//! Shape                   - 可复用的描边/填充辅助
//! FigureEventHandler      - 可选输入能力
//! FigureLifecycle         - 可选挂载生命周期
//! AccessibleFigure       - 可选 accessibility 能力
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

use std::any::Any;

use novadraw_core::Color;
use novadraw_geometry::{Affine2D, Rectangle, Translatable};
use novadraw_render::NdCanvas;
use novadraw_render::command::{LineCap, LineJoin};

use crate::{BlockId, FocusEvent, KeyEvent, MouseEvent, NovadrawContext, WheelEvent};
use border::Border;

const DEFAULT_MAXIMUM_DIMENSION: f64 = i32::MAX as f64;

// ============================================================================
// Bounded Trait: 边界相关方法
// ============================================================================

/// 当前 Figure 提供的 `child content -> node local` 仿射变换。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChildTransform {
    affine: Affine2D,
}

impl ChildTransform {
    /// 恒等变换。
    pub const IDENTITY: Self = Self {
        affine: Affine2D::IDENTITY,
    };

    /// 创建只有平移的变换。
    pub fn translation(translate_x: f64, translate_y: f64) -> Self {
        Self {
            affine: Affine2D::from_translation(translate_x, translate_y),
        }
    }

    /// 创建统一缩放和平移变换。
    pub fn uniform(scale: f64, translate_x: f64, translate_y: f64) -> Self {
        Self {
            affine: Affine2D::from_translation(translate_x, translate_y)
                * Affine2D::from_uniform_scale(scale),
        }
    }

    /// 从任意二维仿射变换创建。
    pub const fn from_affine(affine: Affine2D) -> Self {
        Self { affine }
    }

    /// 返回规范二维仿射变换。
    pub const fn affine(self) -> Affine2D {
        self.affine
    }

    /// 应用 `child content -> node local` 变换。
    pub fn apply_to<T: Translatable>(self, target: &mut T) {
        target.transform(self.affine);
    }

    /// 应用 `node local -> child content` 逆变换。
    pub fn apply_inverse_to<T: Translatable>(self, target: &mut T) -> bool {
        let Some(inverse) = self.affine.inverse() else {
            return false;
        };
        target.transform(inverse);
        true
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

/// Figure 可接受的直接子节点数量策略。
///
/// 该策略由 FigureGraph 在所有 add/reparent 入口统一执行。它用于表达
/// Viewport 等单 contents 容器的结构不变量，同时避免图层代码依赖具体 Figure 类型。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChildPolicy {
    Multiple,
    Single,
}

/// 边界相关方法 trait
///
/// 包含图形的边界、名称、位置检测等基础方法。
/// 所有图形类型都需要实现此 trait。
///
/// # 坐标模型契约
///
/// `bounds()` 返回 parent content domain 中的布局矩形。Figure 自身绘制和
/// 精确命中使用 node-local domain。
pub trait Bounded {
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
        x >= 0.0 && x <= b.width && y >= 0.0 && y <= b.height
    }

    /// 检查矩形是否与图形边界相交
    ///
    /// 对应 draw2d: intersects(Rectangle)
    fn intersects(&self, rect: Rectangle) -> bool {
        let b = self.bounds();
        0.0 < rect.x + rect.width
            && b.width > rect.x
            && 0.0 < rect.y + rect.height
            && b.height > rect.y
    }

    /// 返回 node-local domain 中的保守可见边界。
    ///
    /// 阴影、滤镜或允许越界绘制的 Figure 应覆盖此方法；damage 会将该矩形
    /// 沿与 paint/hit-test 相同的父链变换投影到 logical surface domain。
    fn visual_bounds(&self) -> Rectangle {
        let bounds = self.bounds();
        Rectangle::new(0.0, 0.0, bounds.width, bounds.height)
    }

    /// 获取内边距 (top, left, bottom, right)
    fn insets(&self) -> (f64, f64, f64, f64) {
        (0.0, 0.0, 0.0, 0.0)
    }

    /// 当前 Figure 提供的 `child content -> node local` 坐标变换。
    ///
    /// NodeState 统一应用 client origin；Figure 只提供额外的 scroll 或 scale。
    fn child_transform(&self) -> ChildTransform {
        ChildTransform::IDENTITY
    }

    /// 获取绘制子节点时使用的裁剪策略。
    ///
    /// 对应 draw2d: `Figure#getClippingStrategy()` / `setClippingStrategy(...)`
    /// 的默认行为。具体 Figure 可以覆盖或暴露 builder 来改变策略。
    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        ChildClippingStrategy::ClipToChildBounds
    }

    /// 当前 Figure 可接受的直接子节点数量策略。
    fn child_policy(&self) -> ChildPolicy {
        ChildPolicy::Multiple
    }

    // ==================== 布局相关方法 ====================

    /// 获取客户区域
    ///
    /// 对应 draw2d: getClientArea()
    ///
    /// 返回值位于 node local domain。
    fn client_area(&self) -> Rectangle {
        let b = self.bounds();
        let (top, left, bottom, right) = self.insets();
        let width = b.width - left - right;
        let height = b.height - top - bottom;
        Rectangle::new(left, top, width, height)
    }

    /// 获取首选大小
    ///
    /// 对应 draw2d: getPreferredSize()
    /// 默认返回 bounds 的尺寸
    fn preferred_size(&self) -> (f64, f64) {
        let b = self.bounds();
        (b.width, b.height)
    }

    /// Converts parent/layout hints into this Figure's unscaled layout domain.
    fn layout_size_hints(&self, w_hint: f64, h_hint: f64) -> (f64, f64) {
        (w_hint, h_hint)
    }

    /// Projects an unscaled preferred size into the parent layout domain.
    fn project_preferred_size(&self, size: (f64, f64)) -> (f64, f64) {
        size
    }

    /// 获取最小大小
    ///
    /// 对应 draw2d: getMinimumSize()
    /// 默认返回首选大小
    fn minimum_size(&self) -> (f64, f64) {
        self.preferred_size()
    }

    /// Projects an unscaled minimum size into the parent layout domain.
    fn project_minimum_size(&self, size: (f64, f64)) -> (f64, f64) {
        size
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
pub trait Updatable {
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
pub trait AsAny {
    fn as_any(&self) -> &dyn Any;
}

impl<T: Any> AsAny for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub trait Figure: Bounded + Updatable + AsAny {
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

    /// 使用 NodeState 提供的当前 border-box 绘制。
    ///
    /// 旧 Figure 可继续实现 `paint_figure`；支持 resize 的 Figure 应覆盖此方法，
    /// 避免读取构造期 bounds。
    fn paint_figure_in_bounds(&self, gc: &mut NdCanvas, _bounds: Rectangle) {
        self.paint_figure(gc);
    }

    /// 返回 Figure 的内在尺寸，供无 LayoutManager 时测量。
    fn intrinsic_size(&self) -> (f64, f64) {
        self.preferred_size()
    }

    /// 在 NodeState 当前 border-box 中执行精确命中。
    fn precise_hit(&self, x: f64, y: f64, bounds: Rectangle) -> bool {
        x >= 0.0 && x <= bounds.width && y >= 0.0 && y <= bounds.height
    }

    /// 返回当前 NodeState border-box 对应的 node-local 可见边界。
    fn visual_bounds_in(&self, bounds: Rectangle) -> Rectangle {
        let initial = self.bounds();
        let visual = self.visual_bounds();
        Rectangle::new(
            visual.x,
            visual.y,
            (bounds.width + visual.width - initial.width).max(0.0),
            (bounds.height + visual.height - initial.height).max(0.0),
        )
    }

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
            let bounds = self.bounds();
            border.paint(Rectangle::new(0.0, 0.0, bounds.width, bounds.height), gc);
        }
    }

    /// 使用 NodeState 提供的当前 border-box 绘制边框。
    fn paint_border_in_bounds(&self, gc: &mut NdCanvas, bounds: Rectangle) {
        if let Some(border) = self.get_border() {
            border.paint(Rectangle::new(0.0, 0.0, bounds.width, bounds.height), gc);
        }
    }

    /// 返回可选的输入能力。
    fn event_handler(&self) -> Option<&dyn FigureEventHandler> {
        None
    }

    /// 返回可选的生命周期能力。
    fn lifecycle(&mut self) -> Option<&mut dyn FigureLifecycle> {
        None
    }

    /// 返回可选的 accessibility 能力。
    fn accessible(&self) -> Option<&dyn AccessibleFigure> {
        None
    }
}

/// Figure 的可选输入能力。
///
/// 非交互 Figure 不实现该 trait，也不需要携带空事件方法。
pub trait FigureEventHandler {
    fn wants_mouse_events(&self) -> bool {
        true
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

    fn on_zoom(&self, _event: &crate::ZoomEvent, _ctx: &mut dyn NovadrawContext) -> bool {
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

/// Figure 的可选树挂载生命周期能力。
pub trait FigureLifecycle {
    /// Figure 挂载到父节点后的 hook，对应 Draw2D `addNotify()`。
    fn on_attached(&mut self, _parent_id: BlockId) {}

    /// Figure 从父节点移除前的 hook，对应 Draw2D `removeNotify()`。
    fn on_detached(&mut self, _parent_id: BlockId) {}
}

/// Figure 的可选 accessibility 能力。
pub trait AccessibleFigure {
    fn accessible_name(&self) -> Option<&str> {
        None
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
pub trait Shape {
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

#[cfg(test)]
mod tests {
    use super::ChildTransform;
    use novadraw_geometry::{Affine2D, Point};

    #[test]
    fn singular_child_transform_has_no_inverse_mapping() {
        let transform = ChildTransform::from_affine(Affine2D::from_scale(0.0, 1.0));
        let mut point = Point::new(12.0, 8.0);

        assert!(!transform.apply_inverse_to(&mut point));
        assert_eq!(point, Point::new(12.0, 8.0));
    }
}
