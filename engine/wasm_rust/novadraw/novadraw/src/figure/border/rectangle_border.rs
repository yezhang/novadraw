//! RectangleBorder 矩形边框
//!
//! 绘制矩形边框。

use crate::Color;
use crate::geometry::Rectangle;
use crate::render::NdCanvas;

use super::{Border, BorderStyle, DEFAULT_BORDER_WIDTH, inset_rectangle, render_line_style};

/// 矩形边框
///
/// 绘制矩形边框。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RectangleBorder {
    /// 边框颜色
    pub color: Color,
    /// 边框宽度
    pub width: f64,
    /// 边框样式
    pub style: BorderStyle,
    /// 内边距 (top, left, bottom, right)
    pub insets: (f64, f64, f64, f64),
}

impl RectangleBorder {
    /// 创建矩形边框
    pub fn new(color: Color, width: f64) -> Self {
        assert!(
            width.is_finite() && width > 0.0,
            "rectangle border width must be finite and positive"
        );
        Self {
            color,
            width,
            style: BorderStyle::Solid,
            insets: (width, width, width, width),
        }
    }

    /// 创建默认矩形边框
    pub fn default_border() -> Self {
        Self::new(Color::rgba(0.0, 0.0, 0.0, 1.0), DEFAULT_BORDER_WIDTH)
    }

    /// 设置内边距
    pub fn with_insets(mut self, top: f64, left: f64, bottom: f64, right: f64) -> Self {
        assert!(
            [top, left, bottom, right]
                .into_iter()
                .all(|value| value.is_finite() && value >= 0.0),
            "rectangle border insets must be finite and non-negative"
        );
        self.insets = (top, left, bottom, right);
        self
    }

    /// 设置边框样式
    pub fn with_style(mut self, style: BorderStyle) -> Self {
        self.style = style;
        self
    }
}

impl Border for RectangleBorder {
    fn get_insets(&self) -> (f64, f64, f64, f64) {
        self.insets
    }

    fn paint(&self, figure_bounds: Rectangle, gc: &mut NdCanvas) {
        self.paint_with_insets(figure_bounds, (0.0, 0.0, 0.0, 0.0), gc);
    }

    fn paint_with_insets(
        &self,
        figure_bounds: Rectangle,
        incoming: (f64, f64, f64, f64),
        gc: &mut NdCanvas,
    ) {
        let figure_bounds = inset_rectangle(figure_bounds, incoming);
        let half_width = self.width / 2.0;
        let x = figure_bounds.x + half_width;
        let y = figure_bounds.y + half_width;
        let width = figure_bounds.width - self.width;
        let height = figure_bounds.height - self.width;

        if width <= 0.0 || height <= 0.0 {
            return;
        }

        let cap = crate::render::command::LineCap::Butt;
        let join = crate::render::command::LineJoin::Miter;

        gc.set_line_style(render_line_style(self.style));
        gc.stroke_rect_with_style(x, y, width, height, self.color, self.width, cap, join);
    }

    fn is_opaque(&self) -> bool {
        self.color.is_opaque()
    }

    fn get_color(&self) -> Color {
        self.color
    }

    fn get_width(&self) -> f64 {
        self.width
    }
}
