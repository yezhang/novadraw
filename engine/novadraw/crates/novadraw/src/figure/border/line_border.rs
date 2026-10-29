//! LineBorder 线条边框
//!
//! 绘制简单的线条边框。

use crate::Color;
use crate::geometry::{Insets, Rectangle};
use crate::graphics::{GraphicsError, PaintContext, StrokeStyle};

use super::{Border, BorderInputError, StrokeBorderConfig, inset_rectangle};

/// 线条边框
///
/// 绘制简单的矩形边框，支持不同样式和宽度。
#[derive(Clone, Debug, PartialEq)]
pub struct LineBorder {
    /// 边框颜色
    pub color: Color,
    pub(super) config: StrokeBorderConfig,
}

impl LineBorder {
    /// 创建线条边框
    pub fn new(color: Color, width: f64) -> Result<Self, BorderInputError> {
        Ok(Self {
            color,
            config: StrokeBorderConfig::new(width)?,
        })
    }

    pub fn from_stroke(color: Color, stroke: StrokeStyle) -> Result<Self, BorderInputError> {
        Ok(Self {
            color,
            config: StrokeBorderConfig::from_stroke(stroke)?,
        })
    }

    pub fn stroke(&self) -> &StrokeStyle {
        &self.config.stroke
    }

    pub fn with_stroke(mut self, stroke: StrokeStyle) -> Result<Self, BorderInputError> {
        self.config = self.config.with_stroke(stroke)?;
        Ok(self)
    }

    /// 创建默认线条边框
    ///
    /// 使用黑色，宽度为 1
    pub fn default_border() -> Self {
        Self {
            color: Color::BLACK,
            config: StrokeBorderConfig::default(),
        }
    }

    /// 设置内边距
    pub fn with_insets(mut self, insets: Insets) -> Result<Self, BorderInputError> {
        self.config = self.config.with_insets(insets)?;
        Ok(self)
    }
}

impl Border for LineBorder {
    fn snapshot_bounds_drawing(
        &self,
        _snapshot: Option<&super::BorderSnapshot>,
    ) -> Result<crate::figure::BoundsDrawing, GraphicsError> {
        const LINE_BORDER_RECORDING_BYTES: usize = 4096;
        crate::figure::BoundsDrawing::stroked(
            self.color,
            self.stroke().clone(),
            LINE_BORDER_RECORDING_BYTES,
            |color, stroke, bounds, context| {
                super::paint_stroke_border(*color, stroke, bounds, context)
            },
            |_, _, bounds| Ok(bounds),
        )
    }

    fn get_insets(&self) -> Insets {
        self.config.insets()
    }

    fn paint(
        &self,
        figure_bounds: Rectangle,
        context: &mut PaintContext<'_>,
    ) -> Result<(), GraphicsError> {
        self.paint_with_insets(figure_bounds, Insets::ZERO, context)
    }

    fn paint_with_insets(
        &self,
        figure_bounds: Rectangle,
        incoming: Insets,
        context: &mut PaintContext<'_>,
    ) -> Result<(), GraphicsError> {
        super::paint_stroke_border(
            self.color,
            self.stroke(),
            inset_rectangle(figure_bounds, incoming),
            context,
        )
    }

    fn is_opaque(&self) -> bool {
        self.color.is_opaque()
    }

    fn get_color(&self) -> Color {
        self.color
    }

    fn get_width(&self) -> f64 {
        self.stroke().width()
    }
}
