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
mod image;
mod label;
mod polygon;
mod polyline;
pub(crate) mod preparation;
pub use preparation::{FigureDrawing, FigurePreparation, FigurePresentation};
mod rectangle;
mod root;
mod rounded_rectangle;
mod scalable_polygon;
mod text_flow;
mod triangle;
pub(crate) mod widget;

pub mod border;

pub use crate::style::{CursorIcon, FigureStyle, ResolvedStyle};
pub use border::Border;
pub use ellipse::EllipseFigure;
pub use image::{ImageDisplayState, ImageFigure};
pub use label::{Alignment, LabelFigure, TextPlacement};
pub use polygon::PolygonFigure;
pub use polyline::PolylineFigure;
pub(crate) use polyline::normalize_points;
pub use rectangle::RectangleFigure;
pub use root::RootFigure;
pub use rounded_rectangle::RoundedRectangleFigure;
pub use scalable_polygon::{
    PolygonScaleMode, ScalablePolygonBehavior, ScalablePolygonError, ScalablePolygonFigure,
};
pub use text_flow::{
    FlowPage, FlowParagraph, FlowTextPosition, FlowTextRange, FlowWrapping, InlineTextFragment,
    TextFlowBehavior, TextFlowFigure, TextFlowViewport,
};
pub use triangle::{Direction, TriangleFigure};
pub use widget::{
    ButtonFigure, ClickableBehavior, ClickableFigure, ClickableKind, ClickableModel,
    ClickableSnapshot, ClickableVisualState, ToggleFigure, WidgetError,
};

use std::{any::Any, sync::Arc};

use crate::Color;
use crate::geometry::{Affine2D, Dimension, Point, Rectangle, Translatable};
use crate::render::NdCanvas;
use crate::render::command::{LineCap, LineJoin};

use crate::{EventContext, FigureId, FocusEvent, KeyEvent, MouseEvent, WheelEvent};
use border::BorderSnapshot;

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
    /// Freeform 策略：继承 ancestor clip，不应用当前 clientArea 或 child bounds clip。
    OverflowVisible,
}

/// Figure 可接受的直接子节点数量策略。
///
/// 该策略由 FigureTree 在所有 add/reparent 入口统一执行。它用于表达
/// Viewport 等单 contents 容器的结构不变量，同时避免图层代码依赖具体 Figure 类型。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChildPolicy {
    Multiple,
    Single,
    Layered,
}

/// Controls whether a Figure may be returned by hit-testing after its children.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum HitParticipation {
    #[default]
    SelfAndDescendants,
    DescendantsOnly,
}

/// 构造期几何兼容 trait。
///
/// 新的 Figure 运行时契约不依赖该 trait。它暂时保留给独立图元操作和旧调用方；
/// Figure 加入树后，位置、尺寸和 insets 的权威来源是 NodeState。
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

    /// Converts parent constraints into this Figure's unscaled layout domain.
    fn layout_constraints(&self, constraints: MeasureConstraints) -> MeasureConstraints {
        constraints
    }

    /// Projects an unscaled preferred measurement into the parent layout domain.
    fn project_preferred_measurement(&self, measurement: FigureMeasurement) -> FigureMeasurement {
        measurement
    }

    /// 获取最小大小
    ///
    /// 对应 draw2d: getMinimumSize()
    /// 默认返回首选大小
    fn minimum_size(&self) -> (f64, f64) {
        self.preferred_size()
    }

    /// Projects an unscaled minimum size into the parent layout domain.
    fn project_minimum_size(&self, size: Dimension) -> Dimension {
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
// Figure Trait: 渲染接口
// ============================================================================

/// Figure 渲染 trait
///
/// 所有图形对象都需要实现此 trait。
/// 只包含具体 Figure 行为；运行时几何与 validation 状态由 FigureNode 管理，
/// 需要派生缓存的 Figure 通过 [`FigureLifecycle`] 接入 validation。
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
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn type_name(&self) -> &'static str;
}

impl<T: Any> AsAny for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn type_name(&self) -> &'static str {
        std::any::type_name::<T>()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MeasureConstraints {
    max_width: Option<f64>,
    max_height: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeasureConstraintsError {
    InvalidMaximumWidth(f64),
    InvalidMaximumHeight(f64),
}

impl std::fmt::Display for MeasureConstraintsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidMaximumWidth(width) => {
                write!(
                    formatter,
                    "maximum measurement width must be finite and non-negative, got {width}"
                )
            }
            Self::InvalidMaximumHeight(height) => {
                write!(
                    formatter,
                    "maximum measurement height must be finite and non-negative, got {height}"
                )
            }
        }
    }
}

impl std::error::Error for MeasureConstraintsError {}

impl MeasureConstraints {
    pub const UNBOUNDED: Self = Self {
        max_width: None,
        max_height: None,
    };

    pub fn new(
        max_width: Option<f64>,
        max_height: Option<f64>,
    ) -> Result<Self, MeasureConstraintsError> {
        if let Some(width) = max_width
            && (!width.is_finite() || width < 0.0)
        {
            return Err(MeasureConstraintsError::InvalidMaximumWidth(width));
        }
        if let Some(height) = max_height
            && (!height.is_finite() || height < 0.0)
        {
            return Err(MeasureConstraintsError::InvalidMaximumHeight(height));
        }
        Ok(Self {
            max_width,
            max_height,
        })
    }

    pub fn width(max_width: f64) -> Result<Self, MeasureConstraintsError> {
        Self::new(Some(max_width), None)
    }

    pub fn height(max_height: f64) -> Result<Self, MeasureConstraintsError> {
        Self::new(None, Some(max_height))
    }

    pub fn bounded(max_width: f64, max_height: f64) -> Result<Self, MeasureConstraintsError> {
        Self::new(Some(max_width), Some(max_height))
    }

    pub const fn max_width(self) -> Option<f64> {
        self.max_width
    }

    pub const fn max_height(self) -> Option<f64> {
        self.max_height
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FigureMeasurement {
    pub width: f64,
    pub height: f64,
    pub baseline: Option<f64>,
}

impl FigureMeasurement {
    pub const fn new(width: f64, height: f64, baseline: Option<f64>) -> Self {
        Self {
            width,
            height,
            baseline,
        }
    }

    pub const fn size(self) -> Dimension {
        Dimension {
            width: self.width,
            height: self.height,
        }
    }
}

pub trait Figure: AsAny {
    /// Returns the construction-time placement copied into NodeState on attach.
    fn initial_bounds(&self) -> Rectangle;

    /// Stable diagnostic name.
    fn name(&self) -> &'static str;

    /// Returns construction-time insets copied into NodeState on attach.
    fn initial_insets(&self) -> (f64, f64, f64, f64) {
        (0.0, 0.0, 0.0, 0.0)
    }

    /// Returns construction-time local style copied into NodeState on attach.
    fn initial_style(&self) -> crate::FigureStyle {
        crate::FigureStyle::default()
    }

    fn initial_focusable(&self) -> bool {
        false
    }

    fn initial_focus_traversable(&self) -> bool {
        false
    }

    /// Optional immutable measure/arrange/paint preparation capability.
    fn preparation(&self) -> Option<&dyn FigurePreparation> {
        None
    }

    /// Drawing-only callback for Figures that do not need a prepared presentation.
    ///
    /// The default preserves the legacy Shape/Border adapters during migration.
    fn paint(&self, gc: &mut crate::graphics::PaintContext<'_>, bounds: Rectangle) {
        self.paint_figure_in_bounds(gc.canvas, bounds);
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
        let bounds = self.initial_bounds();
        let Some(border) = self.get_border() else {
            return (bounds.width, bounds.height);
        };
        let (top, left, bottom, right) = border.get_insets();
        let preferred = border.preferred_size();
        (
            (bounds.width + left + right).max(preferred.0),
            (bounds.height + top + bottom).max(preferred.1),
        )
    }

    /// Returns intrinsic content size before Border metrics are applied.
    ///
    /// Figures with owner-dependent Borders and custom intrinsic measurement
    /// should override this together with `intrinsic_content_measurement`.
    fn intrinsic_content_size(&self) -> (f64, f64) {
        let bounds = self.initial_bounds();
        (bounds.width, bounds.height)
    }

    fn intrinsic_measurement(&self, _constraints: MeasureConstraints) -> FigureMeasurement {
        let (width, height) = self.intrinsic_size();
        FigureMeasurement::new(width, height, None)
    }

    fn intrinsic_content_measurement(&self, _constraints: MeasureConstraints) -> FigureMeasurement {
        let (width, height) = self.intrinsic_content_size();
        FigureMeasurement::new(width, height, None)
    }

    /// Returns the Figure's intrinsic minimum size when no LayoutManager supplies one.
    fn intrinsic_minimum_size(&self) -> (f64, f64) {
        self.intrinsic_size()
    }

    fn intrinsic_content_minimum_size(&self) -> (f64, f64) {
        self.intrinsic_content_size()
    }

    fn intrinsic_minimum_measurement(&self, _constraints: MeasureConstraints) -> FigureMeasurement {
        let (width, height) = self.intrinsic_minimum_size();
        FigureMeasurement::new(width, height, None)
    }

    fn intrinsic_content_minimum_measurement(
        &self,
        _constraints: MeasureConstraints,
    ) -> FigureMeasurement {
        let (width, height) = self.intrinsic_content_minimum_size();
        FigureMeasurement::new(width, height, None)
    }

    /// 在 NodeState 当前 border-box 中执行精确命中。
    fn precise_hit(&self, x: f64, y: f64, bounds: Rectangle) -> bool {
        x >= 0.0 && x <= bounds.width && y >= 0.0 && y <= bounds.height
    }

    fn hit_participation(&self) -> HitParticipation {
        HitParticipation::SelfAndDescendants
    }

    /// 返回当前 NodeState border-box 对应的 node-local 可见边界。
    fn visual_bounds_in(&self, bounds: Rectangle) -> Rectangle {
        Rectangle::new(0.0, 0.0, bounds.width, bounds.height)
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
            let bounds = self.initial_bounds();
            border.paint(Rectangle::new(0.0, 0.0, bounds.width, bounds.height), gc);
        }
    }

    /// 使用 NodeState 提供的当前 border-box 绘制边框。
    fn paint_border_in_bounds(&self, gc: &mut NdCanvas, bounds: Rectangle) {
        if let Some(border) = self.get_border() {
            border.paint(Rectangle::new(0.0, 0.0, bounds.width, bounds.height), gc);
        } else {
            self.paint_border(gc);
        }
    }

    fn paint_border_snapshot_in_bounds(
        &self,
        gc: &mut NdCanvas,
        bounds: Rectangle,
        snapshot: Option<&BorderSnapshot>,
    ) {
        if let (Some(border), Some(snapshot)) = (self.get_border(), snapshot) {
            border.paint_snapshot(
                Rectangle::new(0.0, 0.0, bounds.width, bounds.height),
                snapshot,
                gc,
            );
        } else {
            self.paint_border_in_bounds(gc, bounds);
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

    /// 返回可选的容器坐标与布局投影能力。
    fn container(&self) -> Option<&dyn FigureContainer> {
        None
    }

    fn layer(&self) -> Option<&dyn Layer> {
        None
    }

    fn freeform(&self) -> Option<&dyn Freeform> {
        None
    }

    fn content_scale(&self) -> Option<f64> {
        None
    }

    /// Returns optional Connection geometry behavior.
    fn connection(&self) -> Option<&dyn crate::ConnectionFigureBehavior> {
        None
    }

    /// Returns mutable Connection geometry behavior.
    fn connection_mut(&mut self) -> Option<&mut dyn crate::ConnectionFigureBehavior> {
        None
    }

    /// Returns optional route-oriented Connection decoration behavior.
    fn connection_decoration(&self) -> Option<&dyn crate::ConnectionDecorationBehavior> {
        None
    }

    /// Returns mutable route-oriented Connection decoration behavior.
    fn connection_decoration_mut(
        &mut self,
    ) -> Option<&mut dyn crate::ConnectionDecorationBehavior> {
        None
    }

    /// Returns optional point-list geometry behavior.
    fn point_list(&self) -> Option<&dyn PointListFigureBehavior> {
        None
    }

    /// Returns mutable point-list geometry behavior.
    fn point_list_mut(&mut self) -> Option<&mut dyn PointListFigureBehavior> {
        None
    }

    /// Returns optional bounds-driven polygon template behavior.
    fn scalable_polygon(&self) -> Option<&dyn ScalablePolygonBehavior> {
        None
    }

    /// Returns mutable bounds-driven polygon template behavior.
    fn scalable_polygon_mut(&mut self) -> Option<&mut dyn ScalablePolygonBehavior> {
        None
    }

    /// Returns optional paragraph text-flow behavior.
    fn text_flow(&self) -> Option<&dyn TextFlowBehavior> {
        None
    }

    /// Returns mutable paragraph text-flow behavior.
    fn text_flow_mut(&mut self) -> Option<&mut dyn TextFlowBehavior> {
        None
    }

    /// Returns mutable Border ownership behavior.
    fn bordered_mut(&mut self) -> Option<&mut dyn BorderedFigure> {
        None
    }

    /// Returns optional label content, including labels composed into widgets.
    fn label(&self) -> Option<&LabelFigure> {
        None
    }

    /// Returns mutable label content, including labels composed into widgets.
    fn label_mut(&mut self) -> Option<&mut LabelFigure> {
        None
    }

    /// Returns optional button-like interaction behavior.
    fn clickable(&self) -> Option<&dyn ClickableBehavior> {
        None
    }

    /// Returns mutable button-like interaction behavior.
    fn clickable_mut(&mut self) -> Option<&mut dyn ClickableBehavior> {
        None
    }
}

/// Runtime-controlled point-list geometry capability.
pub trait PointListFigureBehavior {
    fn local_points(&self) -> &[Point];
    fn stroke_style(&self) -> &crate::render::StrokeStyle;
    fn painted_minimum(&self) -> usize;
    fn commit_stroke_style(&mut self, stroke: crate::render::StrokeStyle);
    fn commit_geometry(&mut self, bounds: Rectangle, local_points: Vec<Point>);
}

/// Runtime-controlled immutable Border replacement capability.
pub trait BorderedFigure {
    fn border(&self) -> Option<&Arc<dyn Border>>;
    fn replace_border(&mut self, border: Option<Arc<dyn Border>>) -> Option<Arc<dyn Border>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeMutationError {
    Faulted,
    UnknownFigure(FigureId),
    WrongCapability(FigureId),
    NonFiniteGeometry,
    NegativeMetric,
    InvalidStroke(crate::render::GraphicsInputError),
    PointIndexOutOfRange { index: usize, len: usize },
}

impl std::fmt::Display for ShapeMutationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Faulted => formatter.write_str("Runtime is faulted"),
            Self::UnknownFigure(id) => write!(formatter, "unknown Figure: {id:?}"),
            Self::WrongCapability(id) => {
                write!(
                    formatter,
                    "Figure does not support this shape mutation: {id:?}"
                )
            }
            Self::NonFiniteGeometry => write!(formatter, "geometry must be finite"),
            Self::NegativeMetric => write!(formatter, "shape metrics must be non-negative"),
            Self::InvalidStroke(error) => error.fmt(formatter),
            Self::PointIndexOutOfRange { index, len } => {
                write!(
                    formatter,
                    "point index {index} is outside list length {len}"
                )
            }
        }
    }
}

impl std::error::Error for ShapeMutationError {}

/// Marker capability for Figures accepted by a LayeredPane.
pub trait Layer {}

/// Marker capability for Figures whose content extent is derived from descendants.
pub trait Freeform {}

/// Figure 的可选容器能力。
pub trait FigureContainer {
    fn child_transform(&self) -> ChildTransform {
        ChildTransform::IDENTITY
    }

    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        ChildClippingStrategy::ClipToChildBounds
    }

    fn child_policy(&self) -> ChildPolicy {
        ChildPolicy::Multiple
    }

    fn layout_constraints(&self, constraints: MeasureConstraints) -> MeasureConstraints {
        constraints
    }

    fn project_preferred_measurement(&self, measurement: FigureMeasurement) -> FigureMeasurement {
        measurement
    }

    fn project_minimum_size(&self, size: Dimension) -> Dimension {
        size
    }
}

/// Figure 的可选输入能力。
///
/// 非交互 Figure 不实现该 trait，也不需要携带空事件方法。
pub trait FigureEventHandler {
    fn wants_mouse_events(&self) -> bool {
        true
    }

    fn on_mouse_pressed(&self, _event: &MouseEvent, _ctx: &mut EventContext<'_>) -> bool {
        false
    }

    fn on_mouse_released(&self, _event: &MouseEvent, _ctx: &mut EventContext<'_>) -> bool {
        false
    }

    fn on_mouse_moved(&self, _event: &MouseEvent, _ctx: &mut EventContext<'_>) -> bool {
        false
    }

    fn on_mouse_dragged(&self, _event: &MouseEvent, _ctx: &mut EventContext<'_>) -> bool {
        false
    }

    fn on_mouse_hover(&self, _event: &MouseEvent, _ctx: &mut EventContext<'_>) -> bool {
        false
    }

    fn on_mouse_double_clicked(&self, _event: &MouseEvent, _ctx: &mut EventContext<'_>) -> bool {
        false
    }

    fn on_mouse_wheel(&self, _event: &WheelEvent, _ctx: &mut EventContext<'_>) -> bool {
        false
    }

    fn on_zoom(&self, _event: &crate::ZoomEvent, _ctx: &mut EventContext<'_>) -> bool {
        false
    }

    fn on_key_pressed(&self, _event: &KeyEvent, _ctx: &mut EventContext<'_>) -> bool {
        false
    }

    fn on_key_released(&self, _event: &KeyEvent, _ctx: &mut EventContext<'_>) -> bool {
        false
    }

    fn on_focus_gained(&self, _event: &FocusEvent, _ctx: &mut EventContext<'_>) -> bool {
        false
    }

    fn on_focus_lost(&self, _event: &FocusEvent, _ctx: &mut EventContext<'_>) -> bool {
        false
    }

    fn on_mouse_entered(&self, _event: &MouseEvent, _ctx: &mut EventContext<'_>) -> bool {
        false
    }

    fn on_mouse_exited(&self, _event: &MouseEvent, _ctx: &mut EventContext<'_>) -> bool {
        false
    }
}

/// Figure 的可选树挂载生命周期能力。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FigureLifecycleContext {
    pub figure_id: FigureId,
    pub parent_id: FigureId,
    pub runtime_namespace: crate::RuntimeNamespace,
}

pub trait FigureLifecycle {
    /// Figure 挂载到父节点后的 hook，对应 Draw2D `addNotify()`。
    fn on_attached(&mut self, _context: FigureLifecycleContext) {}

    /// Figure 从父节点移除前的 hook，对应 Draw2D `removeNotify()`。
    fn on_detached(&mut self, _context: FigureLifecycleContext) {}

    /// Recomputes Figure-specific derived data after node geometry changes.
    fn validate(&mut self, _bounds: Rectangle) {}

    /// Invalidates Figure-specific derived data.
    fn invalidate(&mut self) {}
}

/// Figure 的可选 accessibility 能力。
pub trait AccessibleFigure {
    fn accessible_name(&self) -> Option<&str> {
        None
    }

    fn accessible_description(&self) -> Option<&str> {
        None
    }

    fn accessible_value(&self) -> Option<&str> {
        None
    }

    fn accessible_role(&self) -> crate::AccessibilityRole {
        crate::AccessibilityRole::Group
    }

    fn accessible_default_action(&self) -> Option<crate::AccessibilityAction> {
        None
    }

    fn accessibility_hidden(&self) -> bool {
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
    use std::cell::Cell;

    use super::{ChildTransform, Figure, MeasureConstraints, MeasureConstraintsError, Shape};
    use crate::Color;
    use crate::geometry::{Affine2D, Point, Rectangle};
    use crate::render::{
        NdCanvas,
        command::{LineCap, LineJoin},
    };

    struct MinimalFigure;

    impl Figure for MinimalFigure {
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, 10.0, 10.0)
        }

        fn name(&self) -> &'static str {
            "MinimalFigure"
        }
    }

    struct CustomShapeFigure {
        custom_paint_called: Cell<bool>,
    }

    impl Shape for CustomShapeFigure {
        fn stroke_color(&self) -> Option<Color> {
            None
        }

        fn stroke_width(&self) -> f64 {
            0.0
        }

        fn fill_color(&self) -> Option<Color> {
            None
        }

        fn line_cap(&self) -> LineCap {
            LineCap::default()
        }

        fn line_join(&self) -> LineJoin {
            LineJoin::default()
        }

        fn fill_shape(&self, _gc: &mut NdCanvas) {}

        fn outline_shape(&self, _gc: &mut NdCanvas) {}
    }

    impl Figure for CustomShapeFigure {
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, 10.0, 10.0)
        }

        fn name(&self) -> &'static str {
            "CustomShapeFigure"
        }

        fn paint_figure(&self, _gc: &mut NdCanvas) {
            self.custom_paint_called.set(true);
        }
    }

    #[test]
    fn singular_child_transform_has_no_inverse_mapping() {
        let transform = ChildTransform::from_affine(Affine2D::from_scale(0.0, 1.0));
        let mut point = Point::new(12.0, 8.0);

        assert!(!transform.apply_inverse_to(&mut point));
        assert_eq!(point, Point::new(12.0, 8.0));
    }

    #[test]
    fn measurement_constraints_reject_invalid_axis_limits() {
        assert_eq!(
            MeasureConstraints::width(-1.0),
            Err(MeasureConstraintsError::InvalidMaximumWidth(-1.0))
        );
        assert!(matches!(
            MeasureConstraints::height(f64::NAN),
            Err(MeasureConstraintsError::InvalidMaximumHeight(value)) if value.is_nan()
        ));
        assert_eq!(
            MeasureConstraints::bounded(120.0, 80.0)
                .unwrap()
                .max_width(),
            Some(120.0)
        );
    }

    #[test]
    fn non_interactive_figure_needs_no_event_or_lifecycle_methods() {
        let mut figure = MinimalFigure;
        assert!(figure.event_handler().is_none());
        assert!(figure.lifecycle().is_none());
        assert!(figure.accessible().is_none());
    }

    #[test]
    fn shape_can_customize_figure_paint_behavior() {
        let figure = CustomShapeFigure {
            custom_paint_called: Cell::new(false),
        };
        Figure::paint_figure(&figure, &mut NdCanvas::new());
        assert!(figure.custom_paint_called.get());
    }
}
