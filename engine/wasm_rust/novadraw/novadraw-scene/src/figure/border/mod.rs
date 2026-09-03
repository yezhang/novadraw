//! Border 边框系统
//!
//! 参考 Eclipse Draw2D 的 Border 设计。
//! Border 是可附加到 Figure 的装饰器，用于绘制边框效果。

mod line_border;
mod margin_border;
mod rectangle_border;

pub use line_border::LineBorder;
pub use margin_border::MarginBorder;
pub use rectangle_border::RectangleBorder;

use novadraw_core::Color;
use novadraw_geometry::Rectangle;
use novadraw_render::NdCanvas;

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

    /// 获取边框颜色
    fn get_color(&self) -> Color;

    /// 获取边框宽度
    fn get_width(&self) -> f64;
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
