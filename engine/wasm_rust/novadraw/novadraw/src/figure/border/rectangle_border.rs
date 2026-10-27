//! RectangleBorder 矩形边框
//!
//! 绘制矩形边框。

use crate::Color;
use crate::geometry::{Insets, Rectangle};
use crate::graphics::{GraphicsError, PaintContext, StrokeStyle};

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
    /// 内边距
    pub insets: Insets,
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
            insets: Insets::uniform(width),
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
        self.insets = Insets::new(top, left, bottom, right);
        self
    }

    /// 设置边框样式
    pub fn with_style(mut self, style: BorderStyle) -> Self {
        self.style = style;
        self
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
        Ok(crate::figure::BoundsDrawing::new(
            *self,
            RECTANGLE_BORDER_RECORDING_BYTES,
            |inputs, bounds, context| inputs.paint(bounds, context),
            |_, bounds| Ok(bounds),
        ))
    }

    fn get_insets(&self) -> Insets {
        self.insets
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
        let figure_bounds = inset_rectangle(figure_bounds, incoming);
        let half_width = self.width / 2.0;
        let x = figure_bounds.x + half_width;
        let y = figure_bounds.y + half_width;
        let width = figure_bounds.width - self.width;
        let height = figure_bounds.height - self.width;

        if width <= 0.0 || height <= 0.0 {
            return Ok(());
        }

        let cap = crate::render::command::LineCap::Butt;
        let join = crate::render::command::LineJoin::Miter;

        context.set_stroke_paint(self.color);
        context.set_stroke_style(
            StrokeStyle::default()
                .with_width(self.width)?
                .with_cap(cap)
                .with_join(join)
                .with_dash_pattern(render_line_style(self.style).into()),
        );
        context.stroke_rect(Rectangle::new(x, y, width, height))
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

/// 矩形 Border 的逐角/分边配置；原 RectangleBorder 结构体字面量保持兼容。
#[derive(Clone, Copy, Debug, PartialEq)]
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

    pub fn with_insets(mut self, top: f64, left: f64, bottom: f64, right: f64) -> Self {
        self.base = self.base.with_insets(top, left, bottom, right);
        self
    }
}

impl Border for RectangleShapeBorder {
    fn snapshot_bounds_drawing(
        &self,
        _snapshot: Option<&super::BorderSnapshot>,
    ) -> Result<crate::figure::BoundsDrawing, GraphicsError> {
        const RECTANGLE_SHAPE_BORDER_RECORDING_BYTES: usize = 8192;
        Ok(crate::figure::BoundsDrawing::new(
            *self,
            RECTANGLE_SHAPE_BORDER_RECORDING_BYTES,
            |inputs, bounds, context| inputs.paint(bounds, context),
            |_, bounds| Ok(bounds),
        ))
    }

    fn get_insets(&self) -> Insets {
        self.base.insets
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
            self.base.width,
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
        self.base.width
    }
}
