//! 多边形图形

use novadraw_core::Color;
use novadraw_geometry::Rectangle;
use novadraw_render::NdCanvas;

use super::{Border, Bounded, ChildClippingStrategy, Figure, PolylineFigure, Shape, Updatable};

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
        Self {
            polyline: PolylineFigure::from_points(points),
            fill_color: Color::hex("#3498db"),
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
        self.polyline.stroke_color = color;
        self.polyline.stroke_width = width;
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
        // 多边形通过点定义，set_bounds 需要重新计算点位置
        // 这里简化处理：平移现有点
        let current_bounds = Bounded::bounds(&self.polyline);
        if current_bounds.width == 0.0 || current_bounds.height == 0.0 {
            return;
        }
        let scale_x = width / current_bounds.width;
        let scale_y = height / current_bounds.height;
        let dx = x - current_bounds.x;
        let dy = y - current_bounds.y;

        let new_points: Vec<novadraw_geometry::Vec2> = self
            .polyline
            .get_points()
            .iter()
            .map(|p| novadraw_geometry::Vec2::new((p.0.x + dx) * scale_x, (p.0.y + dy) * scale_y))
            .collect();
        self.polyline.set_points(new_points);
    }

    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        self.polyline.child_clipping_strategy()
    }

    fn insets(&self) -> (f64, f64, f64, f64) {
        self.polyline.insets()
    }

    fn name(&self) -> &'static str {
        "PolygonFigure"
    }
}

// 实现 Updatable trait
impl Updatable for PolygonFigure {
    fn validate(&mut self) {}
    fn invalidate(&mut self) {}
}

impl Figure for PolygonFigure {
    fn paint_figure(&self, gc: &mut NdCanvas) {
        Shape::paint_figure(self, gc);
    }

    fn paint_figure_in_bounds(&self, gc: &mut NdCanvas, bounds: Rectangle) {
        let mut local = self.clone();
        Bounded::set_bounds(&mut local, 0.0, 0.0, bounds.width, bounds.height);
        Shape::paint_figure(&local, gc);
    }

    fn get_border(&self) -> Option<&dyn Border> {
        Shape::get_border(self)
    }
}

// 实现 Shape trait
impl Shape for PolygonFigure {
    fn stroke_color(&self) -> Option<Color> {
        self.polyline.stroke_color()
    }

    fn stroke_width(&self) -> f64 {
        self.polyline.stroke_width()
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
        self.fill_color.a > 0.0
    }

    fn outline_enabled(&self) -> bool {
        self.polyline.stroke_color.a > 0.0
    }

    fn fill_shape(&self, gc: &mut NdCanvas) {
        let points = self.polyline.get_points();
        if points.len() < 3 {
            return;
        }

        // 使用 path API 构建闭合路径
        gc.begin_path();
        if let Some(first) = points.first() {
            let bounds = self.bounds();
            gc.move_to(first.0.x - bounds.x, first.0.y - bounds.y);
        }
        let bounds = self.bounds();
        for point in points.iter().skip(1) {
            gc.line_to(point.0.x - bounds.x, point.0.y - bounds.y);
        }
        gc.close_path();

        // 设置填充颜色并填充
        gc.fill_style(self.fill_color);
        gc.fill();
    }

    fn outline_shape(&self, gc: &mut NdCanvas) {
        let points = self.polyline.get_points();
        if points.len() < 2 {
            return;
        }

        // 使用 path API 构建闭合路径（与 fill_shape 统一）
        gc.begin_path();
        if let Some(first) = points.first() {
            let bounds = self.bounds();
            gc.move_to(first.0.x - bounds.x, first.0.y - bounds.y);
        }
        let bounds = self.bounds();
        for point in points.iter().skip(1) {
            gc.line_to(point.0.x - bounds.x, point.0.y - bounds.y);
        }
        gc.close_path();

        gc.stroke_style(self.polyline.stroke_color);
        gc.line_width(self.polyline.stroke_width);
        gc.line_cap(self.polyline.line_cap);
        gc.line_join(self.polyline.line_join);
        gc.stroke();
    }
}
