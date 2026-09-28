//! 多边形图形

use std::sync::Arc;

use novadraw_core::Color;
use novadraw_geometry::Rectangle;
use novadraw_render::NdCanvas;

use super::{
    Border, BorderedFigure, Bounded, ChildClippingStrategy, Figure, FigureContainer,
    PointListFigureBehavior, PolylineFigure, Shape, polyline::point_segment_distance_squared,
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
}

impl PolygonFigure {
    /// 创建多边形（从点列表）
    pub fn from_points(points: Vec<novadraw_geometry::Vec2>) -> Self {
        let mut polyline = PolylineFigure::from_points(points);
        polyline.renormalize_for_minimum(3);
        Self {
            polyline,
            fill_color: Color::from_hex("#3498db").expect("valid color literal"),
        }
    }

    /// 添加点
    pub fn add_point(&mut self, x: f64, y: f64) {
        self.polyline.add_point(x, y);
    }

    /// 获取点列表
    pub fn get_points(&self) -> &[novadraw_geometry::Vec2] {
        self.polyline.get_points()
    }

    /// 设置填充颜色
    pub fn with_fill_color(mut self, color: Color) -> Self {
        self.fill_color = color;
        self
    }

    /// 设置线条样式
    pub fn with_stroke(mut self, color: Color, width: f64) -> Self {
        let points = self.polyline.parent_points();
        self.polyline.stroke_color = color;
        self.polyline.stroke_width = width.max(0.0);
        let (bounds, local_points) =
            super::polyline::normalize_points(points, self.polyline.stroke_width, 3);
        self.polyline.commit_geometry(bounds, local_points);
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
}

// 实现 Bounded trait
impl Bounded for PolygonFigure {
    fn bounds(&self) -> Rectangle {
        Bounded::bounds(&self.polyline)
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        Bounded::set_bounds(&mut self.polyline, x, y, width, height);
    }

    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        Bounded::child_clipping_strategy(&self.polyline)
    }

    fn insets(&self) -> (f64, f64, f64, f64) {
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

    fn initial_insets(&self) -> (f64, f64, f64, f64) {
        Bounded::insets(self)
    }

    fn initial_style(&self) -> crate::FigureStyle {
        crate::FigureStyle {
            foreground: Some(self.polyline.stroke_color),
            background: Some(self.fill_color),
            ..crate::FigureStyle::default()
        }
    }

    fn paint_figure(&self, gc: &mut NdCanvas) {
        Shape::paint_figure(self, gc);
    }

    fn paint_figure_in_bounds(&self, gc: &mut NdCanvas, bounds: Rectangle) {
        let mut local = self.clone();
        Bounded::set_bounds(&mut local, 0.0, 0.0, bounds.width, bounds.height);
        Shape::paint_figure(&local, gc);
    }

    fn precise_hit(&self, x: f64, y: f64, _bounds: Rectangle) -> bool {
        let points = self.polyline.get_points();
        if points.len() < 3 {
            return false;
        }
        let local = |point: novadraw_geometry::Vec2| (point.x(), point.y());
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

    fn container(&self) -> Option<&dyn FigureContainer> {
        Some(self)
    }

    fn point_list(&self) -> Option<&dyn PointListFigureBehavior> {
        Some(self)
    }

    fn point_list_mut(&mut self) -> Option<&mut dyn PointListFigureBehavior> {
        Some(self)
    }

    fn bordered_mut(&mut self) -> Option<&mut dyn BorderedFigure> {
        Some(self)
    }
}

impl PointListFigureBehavior for PolygonFigure {
    fn local_points(&self) -> &[novadraw_geometry::Vec2] {
        self.polyline.get_points()
    }

    fn stroke_width(&self) -> f64 {
        self.polyline.stroke_width
    }

    fn painted_minimum(&self) -> usize {
        3
    }

    fn commit_geometry(&mut self, bounds: Rectangle, local_points: Vec<novadraw_geometry::Vec2>) {
        self.polyline.commit_geometry(bounds, local_points);
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

    fn line_cap(&self) -> novadraw_render::command::LineCap {
        self.polyline.line_cap()
    }

    fn line_join(&self) -> novadraw_render::command::LineJoin {
        self.polyline.line_join()
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

    fn fill_shape(&self, gc: &mut NdCanvas) {
        let points = self.polyline.get_points();
        if points.len() < 3 {
            return;
        }

        // 使用 path API 构建闭合路径
        gc.begin_path();
        if let Some(first) = points.first() {
            gc.move_to(first.x(), first.y());
        }
        for point in points.iter().skip(1) {
            gc.line_to(point.x(), point.y());
        }
        gc.close_path();

        gc.fill();
    }

    fn outline_shape(&self, gc: &mut NdCanvas) {
        let points = self.polyline.get_points();
        if points.len() < 3 {
            return;
        }

        // 使用 path API 构建闭合路径（与 fill_shape 统一）
        gc.begin_path();
        if let Some(first) = points.first() {
            gc.move_to(first.x(), first.y());
        }
        for point in points.iter().skip(1) {
            gc.line_to(point.x(), point.y());
        }
        gc.close_path();

        gc.line_width(self.polyline.stroke_width);
        gc.line_cap(self.polyline.line_cap);
        gc.line_join(self.polyline.line_join);
        gc.stroke();
    }
}
