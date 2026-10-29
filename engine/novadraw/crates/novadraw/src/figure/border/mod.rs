//! Border 边框系统
//!
//! 参考 Eclipse Draw2D 的 Border 设计。
//! Border 是可附加到 Figure 的装饰器，用于绘制边框效果。
//!
//! ```compile_fail
//! use novadraw::{figure::border::LineBorder, graphics::Color};
//! let mut border = LineBorder::new(Color::BLACK, 1.0).unwrap();
//! border.width = f64::NAN;
//! ```
//! ```compile_fail
//! use novadraw::{figure::border::RectangleBorder, graphics::Color, geometry::Insets};
//! let border = RectangleBorder { color: Color::BLACK, width: 1.0, insets: Insets::ZERO };
//! ```
//! ```compile_fail
//! use novadraw::{figure::border::MarginBorder, graphics::Color};
//! let border = MarginBorder::new(Color::BLACK, 0.0);
//! ```
//! ```compile_fail
//! use novadraw::{figure::border::BorderBuilder, graphics::Color};
//! let border = BorderBuilder::new(Color::BLACK, 1.0).unwrap().build();
//! ```
//! ```compile_fail
//! use novadraw::figure::border::BorderStyle;
//! ```
//! ```compile_fail
//! use novadraw::{figure::border::LineBorder, graphics::Color};
//! let border: LineBorder = LineBorder::new(Color::BLACK, 1.0);
//! ```

mod bevel_border;
mod compound_border;
mod etched_border;
mod line_border;
mod margin_border;
mod rectangle_border;
mod scheme_border;
mod title_bar_border;

pub use bevel_border::{BevelBorder, BevelStyle};
pub use compound_border::CompoundBorder;
pub use etched_border::EtchedBorder;
pub use line_border::LineBorder;
pub use margin_border::MarginBorder;
pub use rectangle_border::{RectangleBorder, RectangleShapeBorder};
pub use scheme_border::SchemeBorder;
pub use title_bar_border::TitleBarBorder;

use crate::Color;
use crate::geometry::{Dimension, Insets, Rectangle};
use crate::graphics::{GraphicsError, GraphicsInputError, PaintContext, StrokeStyle};
use crate::render::{FontDescriptor, TextError, TextLayoutEngine};

/// A border replacement whose base metrics were captured and checked together.
///
/// Owner-dependent metrics are checked by Runtime before publication.
#[derive(Clone, Default)]
pub struct BorderCandidate {
    border: Option<std::sync::Arc<dyn Border>>,
    metrics: BorderMetrics,
}

impl BorderCandidate {
    pub fn new(border: Option<std::sync::Arc<dyn Border>>) -> Result<Self, BorderInputError> {
        let metrics = border
            .as_deref()
            .map(Border::prepare_metrics)
            .transpose()?
            .unwrap_or_default();
        Ok(Self { border, metrics })
    }

    pub fn border(&self) -> Option<&std::sync::Arc<dyn Border>> {
        self.border.as_ref()
    }

    pub const fn metrics(&self) -> BorderMetrics {
        self.metrics
    }

    pub fn into_border(self) -> Option<std::sync::Arc<dyn Border>> {
        self.border
    }
}

/// Captured and checked metrics from one preparation attempt.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BorderMetrics {
    insets: Insets,
    preferred: Dimension,
}

impl BorderMetrics {
    pub fn new(insets: Insets, preferred: Dimension) -> Result<Self, BorderInputError> {
        validate_insets(insets)?;
        validate_preferred(preferred)?;
        Ok(Self { insets, preferred })
    }

    pub const fn insets(self) -> Insets {
        self.insets
    }

    pub const fn preferred_size(self) -> Dimension {
        self.preferred
    }

    pub(crate) fn compose(outer: Self, inner: Self) -> Result<Self, BorderInputError> {
        let insets = add_insets(outer.insets, inner.insets);
        // Check additions before max can hide an invalid result.
        let width = inner.preferred.width + outer.insets.width();
        let height = inner.preferred.height + outer.insets.height();
        for (field, value) in [
            ("compound top inset", insets.top),
            ("compound left inset", insets.left),
            ("compound bottom inset", insets.bottom),
            ("compound right inset", insets.right),
            ("compound horizontal insets", insets.width()),
            ("compound vertical insets", insets.height()),
            ("compound preferred width", width),
            ("compound preferred height", height),
        ] {
            if !value.is_finite() {
                return Err(BorderInputError::Overflow { field });
            }
        }
        Self::new(
            insets,
            Dimension::new(
                outer.preferred.width.max(width),
                outer.preferred.height.max(height),
            ),
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BorderSnapshot {
    kind: BorderSnapshotKind,
    insets: Insets,
    preferred: Dimension,
}

#[derive(Clone, Debug, PartialEq)]
enum BorderSnapshotKind {
    TitleBar(Box<title_bar_border::TitleBarMetrics>),
    Compound {
        outer: Option<Box<BorderSnapshot>>,
        inner: Option<Box<BorderSnapshot>>,
    },
}

impl BorderSnapshot {
    pub(crate) fn title_bar(metrics: title_bar_border::TitleBarMetrics) -> Self {
        let insets = metrics.insets;
        let preferred = metrics.preferred;
        Self {
            kind: BorderSnapshotKind::TitleBar(Box::new(metrics)),
            insets,
            preferred,
        }
    }

    pub(crate) fn compound(
        outer: Option<BorderSnapshot>,
        inner: Option<BorderSnapshot>,
        insets: Insets,
        preferred: Dimension,
    ) -> Self {
        Self {
            kind: BorderSnapshotKind::Compound {
                outer: outer.map(Box::new),
                inner: inner.map(Box::new),
            },
            insets,
            preferred,
        }
    }

    pub(crate) fn insets(&self) -> Insets {
        self.insets
    }

    pub(crate) fn preferred_size(&self) -> Dimension {
        self.preferred
    }

    pub(crate) fn text_layout(&self) -> &crate::render::TextLayout {
        match &self.kind {
            BorderSnapshotKind::TitleBar(metrics) => &metrics.layout,
            BorderSnapshotKind::Compound { outer, inner } => outer
                .as_deref()
                .or(inner.as_deref())
                .expect("compound snapshots contain a dynamic child")
                .text_layout(),
        }
    }

    fn title_bar_metrics(&self) -> Option<&title_bar_border::TitleBarMetrics> {
        match &self.kind {
            BorderSnapshotKind::TitleBar(metrics) => Some(metrics),
            BorderSnapshotKind::Compound { .. } => None,
        }
    }

    fn compound_parts(&self) -> Option<(Option<&BorderSnapshot>, Option<&BorderSnapshot>)> {
        match &self.kind {
            BorderSnapshotKind::TitleBar(_) => None,
            BorderSnapshotKind::Compound { outer, inner } => {
                Some((outer.as_deref(), inner.as_deref()))
            }
        }
    }
}

/// Border 边框 trait
///
/// 参考 draw2d: Border 接口
/// 所有边框类型都需要实现此 trait。
pub trait Border: Send + Sync {
    /// Captures base callbacks once and checks the same values that the caller installs.
    /// Composites use this protocol recursively for both built-in and external children.
    fn prepare_metrics(&self) -> Result<BorderMetrics, BorderInputError> {
        let insets = self.get_insets();
        let preferred = self.preferred_size();
        BorderMetrics::new(insets, preferred)
    }

    /// Freezes bounded paint inputs for sampled border-box geometry.
    ///
    /// The ordinary static protocol remains available without this descriptor.
    fn snapshot_bounds_drawing(
        &self,
        _snapshot: Option<&BorderSnapshot>,
    ) -> Result<super::BoundsDrawing, GraphicsError> {
        Err(GraphicsError::ImmutableDrawingUnavailable)
    }

    /// Freezes inherited colors alongside size-dependent border content.
    fn snapshot_bounds_drawing_with_colors(
        &self,
        snapshot: Option<&BorderSnapshot>,
        _foreground: Color,
        _background: Color,
    ) -> Result<super::BoundsDrawing, GraphicsError> {
        self.snapshot_bounds_drawing(snapshot)
    }

    /// 获取边框内边距
    ///
    /// 对应 draw2d: getInsets()
    fn get_insets(&self) -> Insets;

    /// 绘制边框
    ///
    /// 对应 draw2d: paint(Figure, Graphics)
    /// 在给定的图形边界内绘制边框
    fn paint(
        &self,
        figure_bounds: Rectangle,
        context: &mut PaintContext<'_>,
    ) -> Result<(), GraphicsError>;

    /// 在调用方已经累计的 inset 内绘制。
    fn paint_with_insets(
        &self,
        figure_bounds: Rectangle,
        incoming: Insets,
        context: &mut PaintContext<'_>,
    ) -> Result<(), GraphicsError> {
        self.paint(inset_rectangle(figure_bounds, incoming), context)
    }

    fn paint_snapshot(
        &self,
        figure_bounds: Rectangle,
        snapshot: &BorderSnapshot,
        context: &mut PaintContext<'_>,
    ) -> Result<(), GraphicsError> {
        self.paint_snapshot_with_insets(figure_bounds, Insets::ZERO, snapshot, context)
    }

    /// Paints an owner-scoped snapshot inside the caller's accumulated insets.
    #[doc(hidden)]
    fn paint_snapshot_with_insets(
        &self,
        figure_bounds: Rectangle,
        incoming: Insets,
        _snapshot: &BorderSnapshot,
        context: &mut PaintContext<'_>,
    ) -> Result<(), GraphicsError> {
        self.paint_with_insets(figure_bounds, incoming, context)
    }

    /// Resolves owner-dependent metrics without storing owner state in the Border.
    #[doc(hidden)]
    fn resolve_owner_snapshot(
        &self,
        _previous: Option<&BorderSnapshot>,
        _font: &FontDescriptor,
        _text: &mut dyn TextLayoutEngine,
    ) -> Result<Option<BorderSnapshot>, TextError> {
        Ok(None)
    }

    /// Returns whether this Border has owner-dependent metrics or paint state.
    #[doc(hidden)]
    fn has_owner_snapshot(&self) -> bool {
        false
    }

    /// Border 自身正确显示所需的最小外部尺寸。
    fn preferred_size(&self) -> Dimension {
        Dimension::ZERO
    }

    /// Border 配置是否完全覆盖其 border ring。
    fn is_opaque(&self) -> bool {
        false
    }

    /// 获取边框颜色
    fn get_color(&self) -> Color {
        Color::TRANSPARENT
    }

    /// 获取边框宽度
    fn get_width(&self) -> f64 {
        0.0
    }

    /// Returns TitleBar-specific behavior when this Border owns a text-derived header.
    fn title_bar(&self) -> Option<&TitleBarBorder> {
        None
    }
}

pub(crate) fn add_insets(first: Insets, second: Insets) -> Insets {
    Insets::new(
        first.top + second.top,
        first.left + second.left,
        first.bottom + second.bottom,
        first.right + second.right,
    )
}

pub(crate) fn inset_rectangle(bounds: Rectangle, insets: Insets) -> Rectangle {
    Rectangle::new(
        bounds.x + insets.left,
        bounds.y + insets.top,
        (bounds.width - insets.width()).max(0.0),
        (bounds.height - insets.height()).max(0.0),
    )
}

pub(super) fn paint_stroke_border(
    color: Color,
    stroke: &StrokeStyle,
    bounds: Rectangle,
    context: &mut PaintContext<'_>,
) -> Result<(), GraphicsError> {
    let half = stroke.width() / 2.0;
    let width = bounds.width - stroke.width();
    let height = bounds.height - stroke.width();
    if width <= 0.0 || height <= 0.0 {
        return Ok(());
    }
    context.set_stroke_paint(color);
    context.set_stroke_style(stroke.clone());
    context.stroke_rect(Rectangle::new(
        bounds.x + half,
        bounds.y + half,
        width,
        height,
    ))
}

/// Stable identity of an invalid inset component.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BorderSide {
    Top,
    Left,
    Bottom,
    Right,
}

/// Invalid border configuration, before a usable candidate is returned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BorderInputError {
    NonFinite {
        field: &'static str,
        side: Option<BorderSide>,
    },
    Negative {
        field: &'static str,
        side: Option<BorderSide>,
    },
    NonPositiveWidth,
    Overflow {
        field: &'static str,
    },
    Stroke(GraphicsInputError),
    Child {
        role: &'static str,
        source: Box<BorderInputError>,
    },
}

impl std::fmt::Display for BorderInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonFinite { field, side } => write!(f, "{field} {side:?} must be finite"),
            Self::Negative { field, side } => write!(f, "{field} {side:?} must be non-negative"),
            Self::NonPositiveWidth => f.write_str("border width must be positive"),
            Self::Overflow { field } => write!(f, "{field} overflow"),
            Self::Stroke(source) => write!(f, "border stroke: {source}"),
            Self::Child { role, source } => write!(f, "{role} border: {source}"),
        }
    }
}

impl std::error::Error for BorderInputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Stroke(source) => Some(source),
            Self::Child { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}

impl From<GraphicsInputError> for BorderInputError {
    fn from(source: GraphicsInputError) -> Self {
        Self::Stroke(source)
    }
}

pub(crate) fn validate_nonnegative(
    value: f64,
    field: &'static str,
    side: Option<BorderSide>,
) -> Result<(), BorderInputError> {
    if !value.is_finite() {
        return Err(BorderInputError::NonFinite { field, side });
    }
    if value < 0.0 {
        return Err(BorderInputError::Negative { field, side });
    }
    Ok(())
}

pub(crate) fn validate_insets(insets: Insets) -> Result<(), BorderInputError> {
    for (side, value) in [
        (BorderSide::Top, insets.top),
        (BorderSide::Left, insets.left),
        (BorderSide::Bottom, insets.bottom),
        (BorderSide::Right, insets.right),
    ] {
        validate_nonnegative(value, "inset", Some(side))?;
    }
    for (field, value) in [
        ("horizontal insets", insets.width()),
        ("vertical insets", insets.height()),
    ] {
        if !value.is_finite() {
            return Err(BorderInputError::Overflow { field });
        }
    }
    Ok(())
}

pub(crate) fn validate_preferred(size: Dimension) -> Result<(), BorderInputError> {
    validate_nonnegative(size.width, "preferred width", None)?;
    validate_nonnegative(size.height, "preferred height", None)
}

#[derive(Clone, Debug, PartialEq)]
enum InsetConfig {
    Auto,
    Explicit(Insets),
}

/// One checked configuration shared by direct constructors and the builder.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct StrokeBorderConfig {
    stroke: StrokeStyle,
    insets: InsetConfig,
}

impl Default for StrokeBorderConfig {
    fn default() -> Self {
        Self {
            stroke: StrokeStyle::default(),
            insets: InsetConfig::Auto,
        }
    }
}

impl StrokeBorderConfig {
    fn new(width: f64) -> Result<Self, BorderInputError> {
        Self::from_stroke(StrokeStyle::default().with_width(width)?)
    }
    fn from_stroke(stroke: StrokeStyle) -> Result<Self, BorderInputError> {
        if stroke.width() <= 0.0 {
            return Err(BorderInputError::NonPositiveWidth);
        }
        validate_insets(Insets::uniform(stroke.width()))?;
        Ok(Self {
            stroke,
            insets: InsetConfig::Auto,
        })
    }
    fn insets(&self) -> Insets {
        match self.insets {
            InsetConfig::Auto => Insets::uniform(self.stroke.width()),
            InsetConfig::Explicit(value) => value,
        }
    }
    fn with_insets(mut self, insets: Insets) -> Result<Self, BorderInputError> {
        validate_insets(insets)?;
        self.insets = InsetConfig::Explicit(insets);
        Ok(self)
    }
    fn with_stroke(mut self, stroke: StrokeStyle) -> Result<Self, BorderInputError> {
        let checked = Self::from_stroke(stroke)?;
        self.stroke = checked.stroke;
        Ok(self)
    }
}

/// 通用边框构建器
///
/// 用于创建常见的边框类型。
pub struct BorderBuilder {
    color: Color,
    config: StrokeBorderConfig,
}

impl BorderBuilder {
    /// 创建新的构建器
    pub fn new(color: Color, width: f64) -> Result<Self, BorderInputError> {
        Ok(Self {
            color,
            config: StrokeBorderConfig::new(width)?,
        })
    }

    /// 设置边框样式
    pub fn from_stroke(color: Color, stroke: StrokeStyle) -> Result<Self, BorderInputError> {
        Ok(Self {
            color,
            config: StrokeBorderConfig::from_stroke(stroke)?,
        })
    }

    pub fn with_stroke(mut self, stroke: StrokeStyle) -> Result<Self, BorderInputError> {
        self.config = self.config.with_stroke(stroke)?;
        Ok(self)
    }

    /// 设置内边距
    pub fn with_insets(mut self, insets: Insets) -> Result<Self, BorderInputError> {
        self.config = self.config.with_insets(insets)?;
        Ok(self)
    }

    /// 创建矩形边框
    pub fn build_rectangle(self) -> RectangleBorder {
        RectangleBorder {
            color: self.color,
            config: self.config,
        }
    }

    /// 创建线条边框
    pub fn build_line(self) -> LineBorder {
        LineBorder {
            color: self.color,
            config: self.config,
        }
    }
}

/// 默认边框宽度
pub const DEFAULT_BORDER_WIDTH: f64 = 1.0;
