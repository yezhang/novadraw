//! 多边形图形

use std::borrow::Cow;
use std::sync::Arc;

use crate::Color;
use crate::geometry::{Insets, Rectangle};
use crate::graphics::{GraphicsError, PaintContext, Path};
use crate::render::{StrokeStyle, command::LineJoin};

use super::{
    Border, BorderedFigure, Bounded, ChildClippingStrategy, Figure, FigureContainer,
    PolylineFigure, Shape, polyline::point_segment_distance_squared,
};

/// 多边形图形
///
/// 参考 Eclipse Draw2D 的 Polygon 设计。
/// 继承自 PolylineFigure，但支持填充（闭合路径）。
#[derive(Clone)]
pub struct PolygonFigure {
    /// 内部使用 PolylineFigure 存储点
    polyline: PolylineFigure,
    /// 填充颜色
    fill_color: Color,
    corner_radii: Vec<f64>,
    contour: Option<Arc<super::ShapeContour>>,
    contour_size: crate::geometry::Dimension,
}

impl PolygonFigure {
    /// 创建多边形（从点列表）
    pub fn from_points(points: Vec<crate::geometry::Point>) -> Self {
        let mut polyline = PolylineFigure::from_points(points);
        polyline.renormalize_for_minimum(3);
        Self {
            polyline,
            fill_color: Color::from_hex("#3498db").expect("valid color literal"),
            corner_radii: Vec::new(),
            contour: None,
            contour_size: crate::geometry::Dimension::ZERO,
        }
    }

    /// 添加点
    pub fn add_point(&mut self, x: f64, y: f64) {
        self.polyline.add_point(x, y);
        // Legacy point edits restore straight corners. Rounded edits use checked replacement.
        self.corner_radii.clear();
        self.contour = None;
    }

    /// 获取点列表
    pub fn get_points(&self) -> &[crate::geometry::Point] {
        self.polyline.get_points()
    }

    /// 按顶点顺序设置真实的圆形半径。
    pub fn with_corner_radii(mut self, radii: Vec<f64>) -> Result<Self, super::ShapeGeometryError> {
        self.replace_corner_radii(radii)?;
        Ok(self)
    }

    pub fn corner_radii(&self) -> &[f64] {
        &self.corner_radii
    }

    pub(crate) fn clear_corner_radii(&mut self) {
        self.corner_radii.clear();
        self.contour = None;
    }

    pub(crate) fn replace_corner_radii(
        &mut self,
        radii: Vec<f64>,
    ) -> Result<(), super::ShapeGeometryError> {
        let contour = super::ShapeContour::polygon(self.get_points(), &radii)?;
        self.corner_radii = radii;
        self.contour = Some(Arc::new(contour));
        self.contour_size = bounds_size(Bounded::bounds(self));
        Ok(())
    }

    fn refresh_contour(&mut self) {
        if !self.corner_radii.is_empty() {
            self.contour = super::ShapeContour::polygon(self.get_points(), &self.corner_radii)
                .ok()
                .map(Arc::new);
            self.contour_size = bounds_size(Bounded::bounds(self));
        }
    }

    pub(crate) fn contour_in(
        &self,
        bounds: Rectangle,
    ) -> Result<Cow<'_, super::ShapeContour>, super::ShapeGeometryError> {
        let old = Bounded::bounds(self);
        if self.contour_size == bounds_size(bounds) {
            return self
                .contour
                .as_deref()
                .map(Cow::Borrowed)
                .ok_or(super::ShapeGeometryError::NonFinite);
        }
        if bounds.width <= 0.0 || bounds.height <= 0.0 {
            return super::ShapeContour::polygon(&[], &[]).map(Cow::Owned);
        }
        let sx = if old.width == 0.0 {
            0.0
        } else {
            bounds.width / old.width
        };
        let sy = if old.height == 0.0 {
            0.0
        } else {
            bounds.height / old.height
        };
        let points: Vec<_> = self
            .get_points()
            .iter()
            .map(|p| crate::geometry::Point::new(p.x() * sx, p.y() * sy))
            .collect();
        super::ShapeContour::polygon(&points, &self.corner_radii).map(Cow::Owned)
    }

    /// 设置填充颜色
    pub fn with_fill_color(mut self, color: Color) -> Self {
        self.fill_color = color;
        self
    }

    /// 设置线条样式
    pub fn with_stroke(mut self, color: Color, width: f64) -> Self {
        self.polyline = self.polyline.with_color(color).with_width(width);
        self.polyline.renormalize_for_minimum(3);
        self.refresh_contour();
        self
    }

    /// Sets the complete checked outline and updates its envelope.
    pub fn with_stroke_style(mut self, stroke: StrokeStyle) -> Self {
        self.polyline = self.polyline.with_stroke_style(stroke);
        self.polyline.renormalize_for_minimum(3);
        self.refresh_contour();
        self
    }

    /// 设置轮廓连接样式。
    pub fn with_join(mut self, join: LineJoin) -> Self {
        self.polyline = self.polyline.with_join(join);
        self.polyline.renormalize_for_minimum(3);
        self.refresh_contour();
        self
    }

    /// 设置子节点绘制裁剪策略。
    pub fn with_child_clipping_strategy(mut self, strategy: ChildClippingStrategy) -> Self {
        self.polyline = self.polyline.with_child_clipping_strategy(strategy);
        self
    }

    /// 添加边框装饰器。
    pub fn with_border(mut self, border: impl Border + 'static) -> Self {
        self.polyline = self.polyline.with_border(border);
        self
    }

    pub(crate) fn local_points(&self) -> &[crate::geometry::Point] {
        self.polyline.local_points()
    }

    pub(crate) fn stroke_style(&self) -> &StrokeStyle {
        self.polyline.stroke_style()
    }

    pub(crate) fn commit_geometry(
        &mut self,
        bounds: Rectangle,
        local_points: Vec<crate::geometry::Point>,
    ) {
        self.polyline.commit_geometry(bounds, local_points);
        self.refresh_contour();
    }
}

// 实现 Bounded trait
impl Bounded for PolygonFigure {
    fn bounds(&self) -> Rectangle {
        Bounded::bounds(&self.polyline)
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        let old = Bounded::bounds(self);
        Bounded::set_bounds(&mut self.polyline, x, y, width, height);
        if old.width != width || old.height != height {
            self.refresh_contour();
        }
    }

    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        Bounded::child_clipping_strategy(&self.polyline)
    }

    fn insets(&self) -> Insets {
        self.polyline.insets()
    }

    fn name(&self) -> &'static str {
        "PolygonFigure"
    }
}

impl Figure for PolygonFigure {
    fn initial_bounds(&self) -> Rectangle {
        Bounded::bounds(self)
    }

    fn name(&self) -> &'static str {
        "PolygonFigure"
    }

    fn initial_insets(&self) -> Insets {
        Bounded::insets(self)
    }

    fn initial_style(&self) -> crate::FigureStyle {
        crate::FigureStyle {
            foreground: Some(self.polyline.stroke_color),
            background: Some(self.fill_color),
            ..crate::FigureStyle::default()
        }
    }

    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        if !self.corner_radii.is_empty() {
            let contour = self
                .contour_in(bounds)
                .map_err(super::ShapeGeometryError::drawing_error)?;
            if self.fill_enabled() {
                context.fill_path(contour.path())?;
            }
            if self.outline_enabled() {
                context.set_stroke_style(self.polyline.stroke_style().clone());
                context.stroke_path(contour.path())?;
            }
            return Ok(());
        }
        let mut local = self.clone();
        Bounded::set_bounds(&mut local, 0.0, 0.0, bounds.width, bounds.height);
        Shape::paint_shape(&local, context)
    }

    fn precise_hit(&self, x: f64, y: f64, bounds: Rectangle) -> bool {
        if !self.corner_radii.is_empty() {
            return self
                .contour_in(bounds)
                .is_ok_and(|c| c.contains(crate::geometry::Point::new(x, y)));
        }
        let points = self.polyline.get_points();
        if points.len() < 3 {
            return false;
        }
        let local = |point: crate::geometry::Point| (point.x(), point.y());
        let edge_tolerance = f64::EPSILON.sqrt();
        if points.iter().enumerate().any(|(index, point)| {
            let next = points[(index + 1) % points.len()];
            let (x1, y1) = local(*point);
            let (x2, y2) = local(next);
            point_segment_distance_squared(x, y, x1, y1, x2, y2) <= edge_tolerance * edge_tolerance
        }) {
            return true;
        }

        let mut inside = false;
        let mut previous = points.len() - 1;
        for current in 0..points.len() {
            let (current_x, current_y) = local(points[current]);
            let (previous_x, previous_y) = local(points[previous]);
            if (current_y > y) != (previous_y > y)
                && x < (previous_x - current_x) * (y - current_y) / (previous_y - current_y)
                    + current_x
            {
                inside = !inside;
            }
            previous = current;
        }
        inside
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

impl BorderedFigure for PolygonFigure {
    fn border(&self) -> Option<&Arc<dyn Border>> {
        BorderedFigure::border(&self.polyline)
    }

    fn replace_border(&mut self, border: Option<Arc<dyn Border>>) -> Option<Arc<dyn Border>> {
        BorderedFigure::replace_border(&mut self.polyline, border)
    }
}

impl FigureContainer for PolygonFigure {
    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        Bounded::child_clipping_strategy(self)
    }
}

impl super::FigureLifecycle for PolygonFigure {
    fn validate(&mut self, bounds: Rectangle) {
        if !self.corner_radii.is_empty() && self.contour_size != bounds_size(bounds) {
            // Cache presentation geometry without changing points or intrinsic bounds.
            self.contour = self
                .contour_in(bounds)
                .ok()
                .map(|contour| Arc::new(contour.into_owned()));
            self.contour_size = bounds_size(bounds);
        }
    }
}

// 实现 Shape trait
impl Shape for PolygonFigure {
    fn stroke_color(&self) -> Option<Color> {
        self.polyline.stroke_color()
    }

    fn stroke_width(&self) -> f64 {
        Shape::stroke_width(&self.polyline)
    }

    fn fill_color(&self) -> Option<Color> {
        Some(self.fill_color)
    }

    fn line_cap(&self) -> crate::render::command::LineCap {
        self.polyline.line_cap()
    }

    fn line_join(&self) -> crate::render::command::LineJoin {
        self.polyline.stroke_style().join()
    }

    fn get_border(&self) -> Option<&dyn Border> {
        Shape::get_border(&self.polyline)
    }

    fn fill_enabled(&self) -> bool {
        self.fill_color.alpha() > 0.0
    }

    fn outline_enabled(&self) -> bool {
        self.polyline.stroke_color.alpha() > 0.0
    }

    fn fill_shape(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        if !self.corner_radii.is_empty() {
            return context.fill_path(
                self.contour_in(Bounded::bounds(self))
                    .map_err(super::ShapeGeometryError::drawing_error)?
                    .path(),
            );
        }
        let points = self.polyline.get_points();
        if points.len() < 3 {
            return Ok(());
        }

        let mut path = Path::new();
        if let Some(first) = points.first() {
            path.move_to(first.x(), first.y());
        }
        for point in points.iter().skip(1) {
            path.line_to(point.x(), point.y());
        }
        path.close();
        context.fill_path(&path)
    }

    fn outline_shape(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        if !self.corner_radii.is_empty() {
            context.set_stroke_style(self.polyline.stroke_style().clone());
            return context.stroke_path(
                self.contour_in(Bounded::bounds(self))
                    .map_err(super::ShapeGeometryError::drawing_error)?
                    .path(),
            );
        }
        let points = self.polyline.get_points();
        if points.len() < 3 {
            return Ok(());
        }

        context.set_stroke_style(self.polyline.stroke_style().clone());
        let mut path = Path::new();
        if let Some(first) = points.first() {
            path.move_to(first.x(), first.y());
        }
        for point in points.iter().skip(1) {
            path.line_to(point.x(), point.y());
        }
        path.close();
        context.stroke_path(&path)
    }
}

fn bounds_size(bounds: Rectangle) -> crate::geometry::Dimension {
    crate::geometry::Dimension::new(bounds.width, bounds.height)
}

#[cfg(test)]
mod contour_cache_tests {
    use super::*;
    use crate::figure::FigureLifecycle;

    #[test]
    fn resized_contour_is_shared_without_changing_intrinsic_geometry() {
        let mut polygon = PolygonFigure::from_points(vec![
            crate::geometry::Point::new(0.0, 0.0),
            crate::geometry::Point::new(100.0, 0.0),
            crate::geometry::Point::new(50.0, 80.0),
        ])
        .with_corner_radii(vec![10.0; 3])
        .unwrap();
        let source_bounds = Bounded::bounds(&polygon);
        let source_points = polygon.get_points().to_vec();
        let bounds = Rectangle::new(0.0, 0.0, 300.0, 240.0);
        FigureLifecycle::validate(&mut polygon, bounds);
        assert_eq!(Bounded::bounds(&polygon), source_bounds);
        assert_eq!(polygon.get_points(), source_points);
        assert!(matches!(
            polygon.contour_in(bounds).unwrap(),
            Cow::Borrowed(_)
        ));
        let cached = Arc::clone(polygon.contour.as_ref().unwrap());
        FigureLifecycle::validate(&mut polygon, bounds);
        assert!(Arc::ptr_eq(&cached, polygon.contour.as_ref().unwrap()));
        assert!(
            polygon
                .contour_in(source_bounds)
                .unwrap()
                .contains(crate::geometry::Point::new(50.0, 30.0))
        );
    }
}
