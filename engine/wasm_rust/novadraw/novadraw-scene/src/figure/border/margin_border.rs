//! MarginBorder 边距边框
//!
//! 参考 Eclipse Draw2D 的 MarginBorder 实现。
//!
//! 主要用于提供内边距（insets），影响子元素布局。
//! paint() 方法为空实现，不绘制任何可见内容。

use novadraw_core::Color;
use novadraw_geometry::Rectangle;
use novadraw_render::NdCanvas;

use super::{Border, BorderStyle, DEFAULT_BORDER_WIDTH};

/// 边距边框
///
/// 参考 draw2d: MarginBorder 提供内边距，不绘制可见内容。
///
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarginBorder {
    /// 边框颜色
    pub color: Color,
    /// 边框宽度
    pub width: f64,
    /// 边框样式
    pub style: super::BorderStyle,
    /// 上边距（内边距）
    pub top: f64,
    /// 左边距（内边距）
    pub left: f64,
    /// 下边距（内边距）
    pub bottom: f64,
    /// 右边距（内边距）
    pub right: f64,
}

impl MarginBorder {
    /// 创建边距边框
    pub fn new(color: Color, width: f64) -> Self {
        assert!(
            width.is_finite() && width >= 0.0,
            "margin border compatibility width must be finite and non-negative"
        );
        Self {
            color,
            width,
            style: BorderStyle::Solid,
            top: 0.0,
            left: 0.0,
            bottom: 0.0,
            right: 0.0,
        }
    }

    /// 创建默认边距边框
    pub fn default_border() -> Self {
        Self::new(Color::rgba(0.0, 0.0, 0.0, 1.0), DEFAULT_BORDER_WIDTH)
    }

    /// 设置上边距
    pub fn with_top(mut self, top: f64) -> Self {
        assert_valid_margin(top);
        self.top = top;
        self
    }

    /// 设置左边距
    pub fn with_left(mut self, left: f64) -> Self {
        assert_valid_margin(left);
        self.left = left;
        self
    }

    /// 设置下边距
    pub fn with_bottom(mut self, bottom: f64) -> Self {
        assert_valid_margin(bottom);
        self.bottom = bottom;
        self
    }

    /// 设置右边距
    pub fn with_right(mut self, right: f64) -> Self {
        assert_valid_margin(right);
        self.right = right;
        self
    }

    /// 设置所有边距
    pub fn with_margins(mut self, top: f64, left: f64, bottom: f64, right: f64) -> Self {
        for margin in [top, left, bottom, right] {
            assert_valid_margin(margin);
        }
        self.top = top;
        self.left = left;
        self.bottom = bottom;
        self.right = right;
        self
    }

    /// 设置边框样式
    pub fn with_style(mut self, style: BorderStyle) -> Self {
        self.style = style;
        self
    }
}

fn assert_valid_margin(margin: f64) {
    assert!(
        margin.is_finite() && margin >= 0.0,
        "margin border insets must be finite and non-negative"
    );
}

impl Border for MarginBorder {
    fn get_insets(&self) -> (f64, f64, f64, f64) {
        (self.top, self.left, self.bottom, self.right)
    }

    /// 绘制边框
    ///
    /// 参考 draw2d: MarginBorder.paint() 为空实现，不绘制任何可见内容。
    /// 主要用于提供内边距（insets），影响子元素布局。
    fn paint(&self, _figure_bounds: Rectangle, _gc: &mut NdCanvas) {
        // 空实现：MarginBorder 不绘制可见内容
        // insets 由 get_insets() 提供，用于布局
    }

    fn get_color(&self) -> Color {
        self.color
    }

    fn get_width(&self) -> f64 {
        self.width
    }
}
