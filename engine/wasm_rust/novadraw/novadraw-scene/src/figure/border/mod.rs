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

use novadraw_core::Color;
use novadraw_geometry::Rectangle;
use novadraw_render::NdCanvas;

#[derive(Clone, Debug, PartialEq)]
pub struct BorderSnapshot {
    kind: BorderSnapshotKind,
}

#[derive(Clone, Debug, PartialEq)]
enum BorderSnapshotKind {
    TitleBar(title_bar_border::TitleBarMetrics),
}

impl BorderSnapshot {
    pub(crate) fn title_bar(metrics: title_bar_border::TitleBarMetrics) -> Self {
        Self {
            kind: BorderSnapshotKind::TitleBar(metrics),
        }
    }

    pub(crate) fn insets(&self) -> (f64, f64, f64, f64) {
        match &self.kind {
            BorderSnapshotKind::TitleBar(metrics) => metrics.insets,
        }
    }

    pub(crate) fn preferred_size(&self) -> (f64, f64) {
        match &self.kind {
            BorderSnapshotKind::TitleBar(metrics) => metrics.preferred,
        }
    }

    pub(crate) fn text_layout(&self) -> &novadraw_render::TextLayout {
        match &self.kind {
            BorderSnapshotKind::TitleBar(metrics) => &metrics.layout,
        }
    }

    fn title_bar_metrics(&self) -> Option<&title_bar_border::TitleBarMetrics> {
        match &self.kind {
            BorderSnapshotKind::TitleBar(metrics) => Some(metrics),
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
    /// 返回 (top, left, bottom, right)
    fn get_insets(&self) -> (f64, f64, f64, f64);

    /// 绘制边框
    ///
    /// 对应 draw2d: paint(Figure, Graphics)
    /// 在给定的图形边界内绘制边框
    fn paint(&self, figure_bounds: Rectangle, gc: &mut NdCanvas);

    /// 在调用方已经累计的 inset 内绘制。
    fn paint_with_insets(
        &self,
        figure_bounds: Rectangle,
        incoming: (f64, f64, f64, f64),
        gc: &mut NdCanvas,
    ) {
        self.paint(inset_rectangle(figure_bounds, incoming), gc);
    }

    fn paint_snapshot(
        &self,
        figure_bounds: Rectangle,
        snapshot: &BorderSnapshot,
        gc: &mut NdCanvas,
    ) {
        match (self.title_bar(), snapshot.title_bar_metrics()) {
            (Some(border), Some(metrics)) => {
                border.paint_metrics(figure_bounds, metrics, gc);
            }
            _ => self.paint(figure_bounds, gc),
        }
    }

    /// Border 自身正确显示所需的最小外部尺寸。
    fn preferred_size(&self) -> (f64, f64) {
        (0.0, 0.0)
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

pub(crate) fn add_insets(
    first: (f64, f64, f64, f64),
    second: (f64, f64, f64, f64),
) -> (f64, f64, f64, f64) {
    (
        first.0 + second.0,
        first.1 + second.1,
        first.2 + second.2,
        first.3 + second.3,
    )
}

pub(crate) fn inset_rectangle(bounds: Rectangle, insets: (f64, f64, f64, f64)) -> Rectangle {
    Rectangle::new(
        bounds.x + insets.1,
        bounds.y + insets.0,
        (bounds.width - insets.1 - insets.3).max(0.0),
        (bounds.height - insets.0 - insets.2).max(0.0),
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

pub(crate) const fn render_line_style(style: BorderStyle) -> novadraw_render::LineStyle {
    match style {
        BorderStyle::Solid => novadraw_render::LineStyle::Solid,
        BorderStyle::Dash | BorderStyle::DashDot => novadraw_render::LineStyle::Dash,
        BorderStyle::Dot => novadraw_render::LineStyle::Dot,
    }
}

/// 通用边框构建器
///
/// 用于创建常见的边框类型。
pub struct BorderBuilder {
    color: Color,
    width: f64,
    style: BorderStyle,
    insets: (f64, f64, f64, f64),
}

impl BorderBuilder {
    /// 创建新的构建器
    pub fn new(color: Color, width: f64) -> Self {
        Self {
            color,
            width,
            style: BorderStyle::Solid,
            insets: (0.0, 0.0, 0.0, 0.0),
        }
    }

    /// 设置边框样式
    pub fn with_style(mut self, style: BorderStyle) -> Self {
        self.style = style;
        self
    }

    /// 设置内边距
    pub fn with_insets(mut self, top: f64, left: f64, bottom: f64, right: f64) -> Self {
        self.insets = (top, left, bottom, right);
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
