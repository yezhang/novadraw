//! 圆角矩形图形
//!
//! ```compile_fail
//! use novadraw::figure::RoundedRectangleFigure;
//! let mut figure = RoundedRectangleFigure::new(0.0, 0.0, 10.0, 10.0, 4.0).unwrap();
//! figure.corner_dimensions = novadraw::geometry::Dimension::new(-1.0, 4.0);
//! ```

use std::sync::Arc;

use crate::Color;
use crate::geometry::{Dimension, Insets, Rectangle};
use crate::graphics::{GraphicsError, PaintContext, Path, StrokeStyle};

use super::{
    Border, BorderedFigure, Bounded, ChildClippingStrategy, Figure, FigureContainer,
    RectangleBorderGeometry, RectangleCorners, RectangleEdges, Shape, ShapeContour,
    ShapeGeometryError,
};

/// 圆角矩形图形
///
/// 参考 Eclipse Draw2D 的 RoundedRectangle 设计。
/// 在矩形基础上添加圆角半径，支持填充和描边。
#[derive(Clone)]
pub struct RoundedRectangleFigure {
    /// 边界矩形
    bounds: Rectangle,
    /// 圆角宽高，对应 Draw2D `corner.width/height`。
    corner_dimensions: Dimension,
    /// 填充颜色
    pub fill_color: Color,
    /// 边框颜色
    pub stroke_color: Option<Color>,
    /// 受检描边几何。
    stroke: StrokeStyle,
    /// 绘制子节点时使用的裁剪策略
    child_clipping_strategy: ChildClippingStrategy,
    /// 边框装饰器
    border: Option<Arc<dyn Border>>,
    corners: Option<RectangleCorners>,
    edges: RectangleEdges,
    prepared: Option<Arc<PreparedRectangle>>,
}

#[derive(Clone)]
struct PreparedRectangle {
    bounds: Rectangle,
    corners: RectangleCorners,
    width: f64,
    contour: ShapeContour,
    border: RectangleBorderGeometry,
}

impl RoundedRectangleFigure {
    /// 使用真实的逐角半径；不改变旧 corner dimensions 的单位。
    pub fn with_corners(mut self, corners: RectangleCorners) -> Result<Self, ShapeGeometryError> {
        self.corners = Some(corners);
        self.refresh_contour()?;
        Ok(self)
    }

    pub fn with_edges(mut self, edges: RectangleEdges) -> Result<Self, ShapeGeometryError> {
        self.edges = edges;
        self.refresh_contour()?;
        Ok(self)
    }

    pub fn corners(&self) -> Option<RectangleCorners> {
        self.corners
    }
    pub fn edges(&self) -> RectangleEdges {
        self.edges
    }

    fn effective_corners(&self) -> Result<RectangleCorners, ShapeGeometryError> {
        self.corners.map_or_else(
            || {
                RectangleCorners::new(
                    [Dimension::new(
                        self.corner_dimensions.width / 2.0,
                        self.corner_dimensions.height / 2.0,
                    ); 4],
                )
            },
            Ok,
        )
    }

    fn contour(&self) -> Result<Arc<PreparedRectangle>, ShapeGeometryError> {
        let bounds = Rectangle::new(0.0, 0.0, self.bounds.width, self.bounds.height);
        let corners = self.effective_corners()?;
        if let Some(cache) = &self.prepared
            && cache.bounds == bounds
            && cache.corners == corners
            && cache.width == self.stroke.width()
        {
            return Ok(Arc::clone(cache));
        }
        Ok(Arc::new(PreparedRectangle {
            bounds,
            corners,
            width: self.stroke.width(),
            contour: ShapeContour::rectangle(bounds, corners)?,
            border: RectangleBorderGeometry::new(bounds, corners, self.stroke.width())?,
        }))
    }

    fn refresh_contour(&mut self) -> Result<(), ShapeGeometryError> {
        self.prepared = Some(self.contour()?);
        Ok(())
    }

    pub(crate) fn replace_corners(
        &mut self,
        corners: Option<RectangleCorners>,
    ) -> Result<(), ShapeGeometryError> {
        let mut candidate = self.clone();
        candidate.corners = corners;
        candidate.refresh_contour()?;
        *self = candidate;
        Ok(())
    }

    pub(crate) fn replace_edges(&mut self, edges: RectangleEdges) {
        self.edges = edges;
    }

    fn paint_geometry(&self) -> Option<(Rectangle, Dimension)> {
        let line_inset = if self.outline_enabled() {
            (1.0_f64).max(self.stroke.width()) / 2.0
        } else {
            0.0
        };
        let bounds = Rectangle::new(
            line_inset,
            line_inset,
            self.bounds.width - line_inset * 2.0,
            self.bounds.height - line_inset * 2.0,
        );
        if bounds.width <= 0.0 || bounds.height <= 0.0 {
            return None;
        }

        let corner_dimensions = Dimension::new(
            (self.corner_dimensions.width - line_inset).max(0.0),
            (self.corner_dimensions.height - line_inset).max(0.0),
        );
        Some((bounds, corner_dimensions))
    }

    /// 创建圆角矩形
    ///
    /// 旧参数 `corner_radius` 设置等宽高 corner dimensions（直径）。
    /// 真实半径使用 [`Self::with_corners`]；0 表示直角。
    pub fn new(
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        corner_radius: f64,
    ) -> Result<Self, ShapeGeometryError> {
        Self::from_bounds(Rectangle::new(x, y, width, height), corner_radius)
    }

    /// 从 Rectangle 创建圆角矩形
    pub fn from_bounds(bounds: Rectangle, corner_radius: f64) -> Result<Self, ShapeGeometryError> {
        let dimensions = Dimension::new(corner_radius, corner_radius);
        RectangleCorners::new([dimensions; 4])?;
        Ok(Self {
            bounds: ShapeGeometryError::validate_bounds(bounds)?,
            corner_dimensions: dimensions,
            fill_color: Color::from_hex("#9b59b6").expect("valid color literal"),
            stroke_color: None,
            stroke: StrokeStyle::default()
                .with_width(0.0)
                .expect("zero shape stroke"),
            child_clipping_strategy: ChildClippingStrategy::ClipToChildBounds,
            border: None,
            corners: None,
            edges: RectangleEdges::INHERIT,
            prepared: None,
        })
    }

    /// 创建指定颜色的圆角矩形
    pub fn new_with_color(
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        corner_radius: f64,
        color: Color,
    ) -> Result<Self, ShapeGeometryError> {
        let mut figure = Self::new(x, y, width, height, corner_radius)?;
        figure.fill_color = color;
        Ok(figure)
    }

    /// 添加边框
    pub fn with_stroke(
        mut self,
        color: Color,
        width: f64,
    ) -> Result<Self, crate::graphics::GraphicsInputError> {
        self.stroke = self.stroke.with_width(width)?;
        self.stroke_color = Some(color);
        Ok(self)
    }

    pub fn with_stroke_style(mut self, stroke: StrokeStyle) -> Self {
        self.stroke = stroke;
        self
    }

    pub fn stroke_style(&self) -> &StrokeStyle {
        &self.stroke
    }

    /// 设置子节点绘制裁剪策略。
    pub fn with_child_clipping_strategy(mut self, strategy: ChildClippingStrategy) -> Self {
        self.child_clipping_strategy = strategy;
        self
    }

    /// 添加边框装饰器。
    pub fn with_border(
        mut self,
        border: impl Border + 'static,
    ) -> Result<Self, super::border::BorderInputError> {
        border.prepare_metrics()?;
        self.border = Some(Arc::new(border));
        Ok(self)
    }

    /// 平移
    pub fn translate(&mut self, dx: f64, dy: f64) -> Result<(), ShapeGeometryError> {
        self.set_bounds(
            self.bounds.x + dx,
            self.bounds.y + dy,
            self.bounds.width,
            self.bounds.height,
        )
    }

    /// 设置边界
    pub fn set_bounds(
        &mut self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Result<(), ShapeGeometryError> {
        let bounds = ShapeGeometryError::validate_bounds(Rectangle::new(x, y, width, height))?;
        let mut candidate = self.clone();
        candidate.bounds = bounds;
        candidate.prepared = None;
        if candidate.corners.is_some() || candidate.edges != RectangleEdges::INHERIT {
            candidate.refresh_contour()?;
        }
        *self = candidate;
        Ok(())
    }

    /// 设置等宽高 corner dimensions（旧参数名 radius，单位为直径），恢复统一圆角模式。
    pub fn set_corner_radius(&mut self, radius: f64) -> Result<(), ShapeGeometryError> {
        self.set_corner_dimensions(Dimension::new(radius, radius))
    }

    pub fn set_corner_dimensions(
        &mut self,
        dimensions: Dimension,
    ) -> Result<(), ShapeGeometryError> {
        RectangleCorners::new([dimensions; 4])?;
        self.corners = None;
        self.corner_dimensions = dimensions;
        self.prepared = None;
        Ok(())
    }

    pub fn corner_dimensions(&self) -> Dimension {
        self.corner_dimensions
    }
}

// 实现 Bounded trait
impl Bounded for RoundedRectangleFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(
        &mut self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Result<(), ShapeGeometryError> {
        Self::set_bounds(self, x, y, width, height)
    }

    fn name(&self) -> &'static str {
        "RoundedRectangleFigure"
    }

    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        self.child_clipping_strategy
    }

    fn insets(&self) -> Insets {
        self.border
            .as_ref()
            .map(|border| border.get_insets())
            .unwrap_or(Insets::ZERO)
    }
}

impl Figure for RoundedRectangleFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "RoundedRectangleFigure"
    }

    fn initial_insets(&self) -> Insets {
        Bounded::insets(self)
    }

    fn initial_style(&self) -> crate::FigureStyle {
        crate::FigureStyle {
            foreground: self.stroke_color,
            background: Some(self.fill_color),
            ..crate::FigureStyle::default()
        }
    }

    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        let mut local = self.clone();
        local.bounds = Rectangle::new(0.0, 0.0, bounds.width, bounds.height);
        Shape::paint_shape(&local, context)
    }

    fn precise_hit(&self, x: f64, y: f64, bounds: Rectangle) -> bool {
        if self.corners.is_some() || self.edges != RectangleEdges::INHERIT {
            let mut local = self.clone();
            local.bounds = bounds;
            return local
                .contour()
                .is_ok_and(|c| c.contour.contains(crate::geometry::Point::new(x, y)));
        }
        if x < 0.0 || y < 0.0 || x > bounds.width || y > bounds.height {
            return false;
        }
        let radius_x = (self.corner_dimensions.width.min(bounds.width).max(0.0)) / 2.0;
        let radius_y = (self.corner_dimensions.height.min(bounds.height).max(0.0)) / 2.0;
        if radius_x == 0.0
            || radius_y == 0.0
            || (x >= radius_x && x <= bounds.width - radius_x)
            || (y >= radius_y && y <= bounds.height - radius_y)
        {
            return true;
        }

        let center_x = if x < radius_x {
            radius_x
        } else {
            bounds.width - radius_x
        };
        let center_y = if y < radius_y {
            radius_y
        } else {
            bounds.height - radius_y
        };
        let dx = (x - center_x) / radius_x;
        let dy = (y - center_y) / radius_y;
        dx * dx + dy * dy <= 1.0
    }

    fn get_border(&self) -> Option<&dyn Border> {
        Shape::get_border(self)
    }

    fn register_capabilities(
        &self,
        out: &mut crate::FigureCapabilityBuilder,
    ) -> Result<(), crate::FigureCapabilityRegistrationError> {
        out.register(crate::CONTAINER, crate::ContainerCapability::of::<Self>())?;
        out.register(crate::LIFECYCLE, crate::LifecycleCapability::of::<Self>())?;
        out.register(crate::BORDER, crate::BorderCapability::of::<Self>())
    }
}

impl BorderedFigure for RoundedRectangleFigure {
    fn border(&self) -> Option<&Arc<dyn Border>> {
        self.border.as_ref()
    }

    fn replace_border(
        &mut self,
        border: super::border::BorderCandidate,
    ) -> Option<Arc<dyn Border>> {
        std::mem::replace(&mut self.border, border.into_border())
    }
}

impl FigureContainer for RoundedRectangleFigure {
    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        self.child_clipping_strategy
    }
}

// 实现 Shape trait
impl Shape for RoundedRectangleFigure {
    fn stroke_color(&self) -> Option<Color> {
        self.stroke_color
    }

    fn stroke_width(&self) -> f64 {
        self.stroke.width()
    }

    fn fill_color(&self) -> Option<Color> {
        Some(self.fill_color)
    }

    fn line_cap(&self) -> crate::render::command::LineCap {
        self.stroke.cap()
    }

    fn line_join(&self) -> crate::render::command::LineJoin {
        self.stroke.join()
    }

    fn get_border(&self) -> Option<&dyn Border> {
        self.border.as_deref()
    }

    fn fill_enabled(&self) -> bool {
        self.fill_color.alpha() > 0.0
    }

    fn outline_enabled(&self) -> bool {
        self.edges.0.iter().any(|edge| {
            edge.resolve(self.stroke_color)
                .is_some_and(|c| c.alpha() > 0.0)
        })
    }

    fn fill_shape(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        if self.corners.is_some() || self.edges != RectangleEdges::INHERIT {
            let prepared = self.contour().map_err(ShapeGeometryError::drawing_error)?;
            return context.fill_path(prepared.contour.path());
        }
        let Some((bounds, corner_dimensions)) = self.paint_geometry() else {
            return Ok(());
        };
        let mut local = self.clone();
        local.bounds = bounds;
        local.corner_dimensions = corner_dimensions;
        local.draw_rounded_rect(context, Some(self.fill_color), None)
    }

    fn outline_shape(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        if self.corners.is_some() || self.edges != RectangleEdges::INHERIT {
            let prepared = self.contour().map_err(ShapeGeometryError::drawing_error)?;
            let inherited = context.stroke_paint().cloned();
            return prepared.border.paint(context, self.edges, inherited);
        }
        if let Some(color) = self.stroke_color {
            let Some((bounds, corner_dimensions)) = self.paint_geometry() else {
                return Ok(());
            };

            let temp_rect = RoundedRectangleFigure {
                bounds,
                corner_dimensions,
                fill_color: Color::TRANSPARENT,
                stroke_color: Some(color),
                stroke: self.stroke.clone(),
                child_clipping_strategy: self.child_clipping_strategy,
                border: None,
                corners: None,
                edges: RectangleEdges::INHERIT,
                prepared: None,
            };
            temp_rect.draw_rounded_rect(context, None, Some(color))?;
        }
        Ok(())
    }
}

impl super::FigureLifecycle for RoundedRectangleFigure {
    fn validate(&mut self, bounds: Rectangle) {
        if self.corners.is_some() || self.edges != RectangleEdges::INHERIT {
            // Bounds are Runtime-validated; a paint error reports any derived overflow.
            // Preserve source bounds: they also define the figure's intrinsic size.
            let mut local = self.clone();
            local.bounds = Rectangle::new(0.0, 0.0, bounds.width, bounds.height);
            self.prepared = local.contour().ok();
        }
    }
}

impl RoundedRectangleFigure {
    /// 绘制圆角矩形（填充和/或描边）
    fn draw_rounded_rect(
        &self,
        context: &mut PaintContext<'_>,
        fill_color: Option<Color>,
        stroke_color: Option<Color>,
    ) -> Result<(), GraphicsError> {
        let x = self.bounds.x;
        let y = self.bounds.y;
        let width = self.bounds.width;
        let height = self.bounds.height;
        let radius_x = self.corner_dimensions.width.min(width).max(0.0) / 2.0;
        let radius_y = self.corner_dimensions.height.min(height).max(0.0) / 2.0;

        // 边界检查
        if width <= 0.0 || height <= 0.0 {
            return Ok(());
        }

        // 如果没有圆角，退化为普通矩形
        if radius_x <= 0.0 || radius_y <= 0.0 {
            if let Some(color) = fill_color {
                context.set_fill_paint(color);
                context.fill_rect(self.bounds)?;
            }
            if let Some(color) = stroke_color {
                context.set_stroke_paint(color);
                context.set_stroke_style(self.stroke.clone());
                context.stroke_rect(self.bounds)?;
            }
            return Ok(());
        }

        let mut path = Path::new();
        path.move_to(x + radius_x, y);
        path.line_to(x + width - radius_x, y);
        path.quad_to(x + width, y, x + width, y + radius_y);
        path.line_to(x + width, y + height - radius_y);
        path.quad_to(x + width, y + height, x + width - radius_x, y + height);
        path.line_to(x + radius_x, y + height);
        path.quad_to(x, y + height, x, y + height - radius_y);
        path.line_to(x, y + radius_y);
        path.quad_to(x, y, x + radius_x, y);
        path.close();

        // 填充
        if let Some(color) = fill_color {
            context.set_fill_paint(color);
            context.fill_path(&path)?;
        }

        // 描边
        if let Some(color) = stroke_color {
            context.set_stroke_paint(color);
            context.set_stroke_style(self.stroke.clone());
            context.stroke_path(&path)?;
        }
        Ok(())
    }
}
