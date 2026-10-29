//! MarginBorder 边距边框
//!
//! 参考 Eclipse Draw2D 的 MarginBorder 实现。
//!
//! 主要用于提供内边距（insets），影响子元素布局。
//! paint() 方法为空实现，不绘制任何可见内容。

use crate::geometry::{Insets, Rectangle};
use crate::graphics::{GraphicsError, PaintContext};

use super::{Border, BorderInputError, validate_insets};

/// 边距边框
///
/// 参考 draw2d: MarginBorder 提供内边距，不绘制可见内容。
///
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MarginBorder {
    insets: Insets,
}

impl MarginBorder {
    /// 创建边距边框
    pub fn new(insets: Insets) -> Result<Self, BorderInputError> {
        validate_insets(insets)?;
        Ok(Self { insets })
    }

    /// 创建默认边距边框
    pub fn default_border() -> Self {
        Self::default()
    }

    /// 设置上边距
    pub fn with_top(self, top: f64) -> Result<Self, BorderInputError> {
        self.with_margins(Insets { top, ..self.insets })
    }

    /// 设置左边距
    pub fn with_left(self, left: f64) -> Result<Self, BorderInputError> {
        self.with_margins(Insets {
            left,
            ..self.insets
        })
    }

    /// 设置下边距
    pub fn with_bottom(self, bottom: f64) -> Result<Self, BorderInputError> {
        self.with_margins(Insets {
            bottom,
            ..self.insets
        })
    }

    /// 设置右边距
    pub fn with_right(self, right: f64) -> Result<Self, BorderInputError> {
        self.with_margins(Insets {
            right,
            ..self.insets
        })
    }

    /// 设置所有边距
    pub fn with_margins(self, insets: Insets) -> Result<Self, BorderInputError> {
        Self::new(insets)
    }
}

impl Border for MarginBorder {
    fn snapshot_bounds_drawing(
        &self,
        _snapshot: Option<&super::BorderSnapshot>,
    ) -> Result<crate::figure::BoundsDrawing, GraphicsError> {
        Ok(crate::figure::BoundsDrawing::new(
            *self,
            0,
            |inputs, bounds, context| inputs.paint(bounds, context),
            |_, bounds| Ok(bounds),
        ))
    }

    fn get_insets(&self) -> Insets {
        self.insets
    }

    /// 绘制边框
    ///
    /// 参考 draw2d: MarginBorder.paint() 为空实现，不绘制任何可见内容。
    /// 主要用于提供内边距（insets），影响子元素布局。
    fn paint(
        &self,
        _figure_bounds: Rectangle,
        _context: &mut PaintContext<'_>,
    ) -> Result<(), GraphicsError> {
        // 空实现：MarginBorder 不绘制可见内容
        // insets 由 get_insets() 提供，用于布局
        Ok(())
    }
}
