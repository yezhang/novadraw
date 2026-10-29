//! RectangleBorder 矩形边框
//!
//! 绘制矩形边框。

use crate::Color;
use crate::geometry::{Insets, Rectangle};
use crate::graphics::{GraphicsError, PaintContext, StrokeStyle};

use super::{Border, BorderInputError, StrokeBorderConfig, inset_rectangle};

/// 矩形边框
///
/// 绘制矩形边框。
#[derive(Clone, Debug, PartialEq)]
pub struct RectangleBorder {
    /// 边框颜色
    pub color: Color,
    pub(super) config: StrokeBorderConfig,
}

impl RectangleBorder {
    /// 创建矩形边框
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

    /// 创建默认矩形边框
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

    pub fn with_corners(self, corners: crate::figure::RectangleCorners) -> RectangleShapeBorder {
        RectangleShapeBorder {
            base: self,
            corners,
            edges: crate::figure::RectangleEdges::INHERIT,
        }
    }

    /// 分边或圆角模式使用实线区域填充。
    pub fn with_edges(self, edges: crate::figure::RectangleEdges) -> RectangleShapeBorder {
        RectangleShapeBorder {
            base: self,
            corners: crate::figure::RectangleCorners::ZERO,
            edges,
        }
    }
}

impl Border for RectangleBorder {
    fn snapshot_bounds_drawing(
        &self,
        _snapshot: Option<&super::BorderSnapshot>,
    ) -> Result<crate::figure::BoundsDrawing, GraphicsError> {
        const RECTANGLE_BORDER_RECORDING_BYTES: usize = 4096;
        crate::figure::BoundsDrawing::stroked(
            self.color,
            self.stroke().clone(),
            RECTANGLE_BORDER_RECORDING_BYTES,
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

/// 矩形 Border 的逐角/分边配置。
#[derive(Clone, Debug, PartialEq)]
pub struct RectangleShapeBorder {
    base: RectangleBorder,
    corners: crate::figure::RectangleCorners,
    edges: crate::figure::RectangleEdges,
}

impl RectangleShapeBorder {
    pub fn with_corners(mut self, corners: crate::figure::RectangleCorners) -> Self {
        self.corners = corners;
        self
    }

    pub fn with_edges(mut self, edges: crate::figure::RectangleEdges) -> Self {
        self.edges = edges;
        self
    }

    pub fn with_insets(mut self, insets: Insets) -> Result<Self, BorderInputError> {
        self.base = self.base.with_insets(insets)?;
        Ok(self)
    }
}

impl Border for RectangleShapeBorder {
    fn snapshot_bounds_drawing(
        &self,
        _snapshot: Option<&super::BorderSnapshot>,
    ) -> Result<crate::figure::BoundsDrawing, GraphicsError> {
        const RECTANGLE_SHAPE_BORDER_RECORDING_BYTES: usize = 8192;
        Ok(crate::figure::BoundsDrawing::new(
            (
                self.base.color,
                self.base.get_width(),
                self.corners,
                self.edges,
            ),
            RECTANGLE_SHAPE_BORDER_RECORDING_BYTES,
            |(color, width, corners, edges), bounds, context| {
                let geometry =
                    crate::figure::RectangleBorderGeometry::new(bounds, *corners, *width)
                        .map_err(crate::figure::ShapeGeometryError::drawing_error)?;
                geometry.paint(context, *edges, Some((*color).into()))
            },
            |_, bounds| Ok(bounds),
        ))
    }

    fn get_insets(&self) -> Insets {
        self.base.get_insets()
    }

    fn paint(
        &self,
        bounds: Rectangle,
        context: &mut PaintContext<'_>,
    ) -> Result<(), GraphicsError> {
        self.paint_with_insets(bounds, Insets::ZERO, context)
    }

    fn paint_with_insets(
        &self,
        bounds: Rectangle,
        incoming: Insets,
        context: &mut PaintContext<'_>,
    ) -> Result<(), GraphicsError> {
        let geometry = crate::figure::RectangleBorderGeometry::new(
            inset_rectangle(bounds, incoming),
            self.corners,
            self.base.get_width(),
        )
        .map_err(crate::figure::ShapeGeometryError::drawing_error)?;
        geometry.paint(context, self.edges, Some(self.base.color.into()))
    }

    fn is_opaque(&self) -> bool {
        self.edges.0.iter().all(|edge| {
            edge.resolve(Some(self.base.color))
                .is_some_and(|color| color.is_opaque())
        })
    }

    fn get_color(&self) -> Color {
        self.base.color
    }
    fn get_width(&self) -> f64 {
        self.base.get_width()
    }
}
