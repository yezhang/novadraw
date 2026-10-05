//! Border 边框系统
//!
//! 参考 Eclipse Draw2D 的 Border 设计。
//! Border 是可附加到 Figure 的装饰器，用于绘制边框效果。

mod bevel_border;
mod compound_border;
mod etched_border;
mod line_border;
mod margin_border;
mod rectangle_border;
mod title_bar_border;

pub use bevel_border::{BevelBorder, BevelStyle};
pub use compound_border::CompoundBorder;
pub use etched_border::EtchedBorder;
pub use line_border::LineBorder;
pub use margin_border::MarginBorder;
pub use rectangle_border::RectangleBorder;
pub use title_bar_border::TitleBarBorder;

use crate::Color;
use crate::geometry::{Dimension, Insets, Rectangle};
use crate::render::{FontDescriptor, NdCanvas, TextError, TextLayoutEngine};

#[derive(Clone, Debug, PartialEq)]
pub struct BorderSnapshot {
    kind: BorderSnapshotKind,
    insets: Insets,
    preferred: Dimension,
}

#[derive(Clone, Debug, PartialEq)]
enum BorderSnapshotKind {
    TitleBar(title_bar_border::TitleBarMetrics),
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
            kind: BorderSnapshotKind::TitleBar(metrics),
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
    /// 获取边框内边距
    ///
    /// 对应 draw2d: getInsets()
    fn get_insets(&self) -> Insets;

    /// 绘制边框
    ///
    /// 对应 draw2d: paint(Figure, Graphics)
    /// 在给定的图形边界内绘制边框
    fn paint(&self, figure_bounds: Rectangle, gc: &mut NdCanvas);

    /// 在调用方已经累计的 inset 内绘制。
    fn paint_with_insets(&self, figure_bounds: Rectangle, incoming: Insets, gc: &mut NdCanvas) {
        self.paint(inset_rectangle(figure_bounds, incoming), gc);
    }

    fn paint_snapshot(
        &self,
        figure_bounds: Rectangle,
        snapshot: &BorderSnapshot,
        gc: &mut NdCanvas,
    ) {
        self.paint_snapshot_with_insets(figure_bounds, Insets::ZERO, snapshot, gc);
    }

    /// Paints an owner-scoped snapshot inside the caller's accumulated insets.
    #[doc(hidden)]
    fn paint_snapshot_with_insets(
        &self,
        figure_bounds: Rectangle,
        incoming: Insets,
        _snapshot: &BorderSnapshot,
        gc: &mut NdCanvas,
    ) {
        self.paint_with_insets(figure_bounds, incoming, gc);
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

/// Border 样式
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum BorderStyle {
    /// 实线
    #[default]
    Solid,
    /// 虚线
    Dash,
    /// 点线
    Dot,
    /// dash-dot 交替
    DashDot,
}

pub(crate) const fn render_line_style(style: BorderStyle) -> crate::render::LineStyle {
    match style {
        BorderStyle::Solid => crate::render::LineStyle::Solid,
        BorderStyle::Dash | BorderStyle::DashDot => crate::render::LineStyle::Dash,
        BorderStyle::Dot => crate::render::LineStyle::Dot,
    }
}

/// 通用边框构建器
///
/// 用于创建常见的边框类型。
pub struct BorderBuilder {
    color: Color,
    width: f64,
    style: BorderStyle,
    insets: Insets,
}

impl BorderBuilder {
    /// 创建新的构建器
    pub fn new(color: Color, width: f64) -> Self {
        Self {
            color,
            width,
            style: BorderStyle::Solid,
            insets: Insets::ZERO,
        }
    }

    /// 设置边框样式
    pub fn with_style(mut self, style: BorderStyle) -> Self {
        self.style = style;
        self
    }

    /// 设置内边距
    pub fn with_insets(mut self, top: f64, left: f64, bottom: f64, right: f64) -> Self {
        self.insets = Insets::new(top, left, bottom, right);
        self
    }

    /// 创建矩形边框
    pub fn build_rectangle(self) -> RectangleBorder {
        RectangleBorder {
            color: self.color,
            width: self.width,
            style: self.style,
            insets: self.insets,
        }
    }

    /// 创建线条边框
    pub fn build_line(self) -> LineBorder {
        LineBorder {
            color: self.color,
            width: self.width,
            style: self.style,
            insets: self.insets,
        }
    }
}

/// 默认边框宽度
pub const DEFAULT_BORDER_WIDTH: f64 = 1.0;
