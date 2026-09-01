//! 折线图形

use std::sync::Arc;

use novadraw_core::Color;
use novadraw_geometry::Rectangle;
use novadraw_render::NdCanvas;

use super::{Border, Bounded, ChildClippingStrategy, Figure, Shape, Updatable};

/// 折线图形
///
/// 参考 Eclipse Draw2D 的 Polyline 设计。
/// 使用点列表存储多个顶点，可以绘制任意折线。
/// bounds 是自动计算的，基于点列表并扩展线宽。
///
/// 注意：不能通过 set_bounds 定位，应该通过 add_point/set_points 操作点。
#[derive(Clone)]
pub struct PolylineFigure {
    /// 点列表
    points: Vec<novadraw_geometry::Vec2>,
    /// 线条颜色
    pub stroke_color: Color,
    /// 线条宽度
    pub stroke_width: f64,
    /// 线帽样式
    pub line_cap: novadraw_render::command::LineCap,
    /// 连接样式
    pub line_join: novadraw_render::command::LineJoin,
    /// 绘制子节点时使用的裁剪策略
    child_clipping_strategy: ChildClippingStrategy,
    /// 边框装饰器
    border: Option<Arc<dyn Border>>,
}

impl PolylineFigure {
    /// 创建两点折线（直线）
    ///
    /// 从 (x1, y1) 到 (x2, y2)
    pub fn new(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        Self {
            points: vec![
                novadraw_geometry::Vec2::new(x1, y1),
                novadraw_geometry::Vec2::new(x2, y2),
            ],
            stroke_color: Color::hex("#2c3e50"),
            stroke_width: 2.0,
            line_cap: novadraw_render::command::LineCap::default(),
            line_join: novadraw_render::command::LineJoin::default(),
            child_clipping_strategy: ChildClippingStrategy::ClipToChildBounds,
            border: None,
        }
    }

    /// 从点列表创建折线
    pub fn from_points(points: Vec<novadraw_geometry::Vec2>) -> Self {
        Self {
            points,
            stroke_color: Color::hex("#2c3e50"),
            stroke_width: 2.0,
            line_cap: novadraw_render::command::LineCap::default(),
            line_join: novadraw_render::command::LineJoin::default(),
            child_clipping_strategy: ChildClippingStrategy::ClipToChildBounds,
            border: None,
        }
    }

    /// 创建指定颜色的折线
    pub fn new_with_color(x1: f64, y1: f64, x2: f64, y2: f64, color: Color) -> Self {
        Self {
            points: vec![
                novadraw_geometry::Vec2::new(x1, y1),
                novadraw_geometry::Vec2::new(x2, y2),
            ],
            stroke_color: color,
            stroke_width: 2.0,
            line_cap: novadraw_render::command::LineCap::default(),
            line_join: novadraw_render::command::LineJoin::default(),
            child_clipping_strategy: ChildClippingStrategy::ClipToChildBounds,
            border: None,
        }
    }

    /// 添加点
    pub fn add_point(&mut self, x: f64, y: f64) {
        self.points.push(novadraw_geometry::Vec2::new(x, y));
    }

    /// 获取点列表（引用）
    pub fn get_points(&self) -> &[novadraw_geometry::Vec2] {
        &self.points
    }

    /// 设置点列表
    pub fn set_points(&mut self, points: Vec<novadraw_geometry::Vec2>) {
        self.points = points;
    }

    /// 获取起点
    pub fn start_point(&self) -> Option<novadraw_geometry::Vec2> {
        self.points.first().copied()
    }

    /// 获取终点
    pub fn end_point(&self) -> Option<novadraw_geometry::Vec2> {
        self.points.last().copied()
    }

    /// 获取点数量
    pub fn point_count(&self) -> usize {
        self.points.len()
    }

    /// 设置线条颜色
    pub fn with_color(mut self, color: Color) -> Self {
        self.stroke_color = color;
        self
    }

    /// 设置线条宽度
    pub fn with_width(mut self, width: f64) -> Self {
        self.stroke_width = width;
        self
    }

    /// 设置线帽样式
    pub fn with_cap(mut self, cap: novadraw_render::command::LineCap) -> Self {
        self.line_cap = cap;
        self
    }

    /// 设置连接样式
    pub fn with_join(mut self, join: novadraw_render::command::LineJoin) -> Self {
        self.line_join = join;
        self
    }

    /// 设置子节点绘制裁剪策略。
    pub fn with_child_clipping_strategy(mut self, strategy: ChildClippingStrategy) -> Self {
        self.child_clipping_strategy = strategy;
        self
    }

    /// 添加边框装饰器。
    pub fn with_border(mut self, border: impl Border + 'static) -> Self {
        self.border = Some(Arc::new(border));
        self
    }

    /// 计算包含线宽的边界矩形
    fn calculate_bounds(&self) -> Rectangle {
        if self.points.is_empty() {
            return Rectangle::ZERO;
        }

        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;

        for point in &self.points {
            min_x = min_x.min(point.0.x);
            min_y = min_y.min(point.0.y);
            max_x = max_x.max(point.0.x);
            max_y = max_y.max(point.0.y);
        }

        // 扩展边界以包含描边宽度
        let half_stroke = self.stroke_width / 2.0;
        Rectangle::new(
            min_x - half_stroke,
            min_y - half_stroke,
            (max_x - min_x) + self.stroke_width,
            (max_y - min_y) + self.stroke_width,
        )
    }
}

// 实现 Bounded trait
impl Bounded for PolylineFigure {
    fn bounds(&self) -> Rectangle {
        self.calculate_bounds()
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        // 折线通过点定义，set_bounds 需要重新计算点位置
        let current_bounds = self.calculate_bounds();
        if current_bounds.width == 0.0 || current_bounds.height == 0.0 {
            return;
        }
        let scale_x = width / current_bounds.width;
        let scale_y = height / current_bounds.height;
        let dx = x - current_bounds.x;
        let dy = y - current_bounds.y;

        let new_points: Vec<novadraw_geometry::Vec2> = self
            .points
            .iter()
            .map(|p| novadraw_geometry::Vec2::new((p.0.x + dx) * scale_x, (p.0.y + dy) * scale_y))
            .collect();
        self.points = new_points;
    }

    fn name(&self) -> &'static str {
        "PolylineFigure"
    }

    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        self.child_clipping_strategy
    }

    fn insets(&self) -> (f64, f64, f64, f64) {
        self.border
            .as_ref()
            .map(|border| border.get_insets())
            .unwrap_or((0.0, 0.0, 0.0, 0.0))
    }
}

// 实现 Updatable trait
impl Updatable for PolylineFigure {
    fn validate(&mut self) {}
    fn invalidate(&mut self) {}
}

impl Figure for PolylineFigure {
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
impl Shape for PolylineFigure {
    fn stroke_color(&self) -> Option<Color> {
        Some(self.stroke_color)
    }

    fn stroke_width(&self) -> f64 {
        self.stroke_width
    }

    fn fill_color(&self) -> Option<Color> {
        None // Polyline 不支持填充
    }

    fn line_cap(&self) -> novadraw_render::command::LineCap {
        self.line_cap
    }

    fn line_join(&self) -> novadraw_render::command::LineJoin {
        self.line_join
    }

    fn get_border(&self) -> Option<&dyn Border> {
        self.border.as_deref()
    }

    fn fill_enabled(&self) -> bool {
        false // Polyline 不支持填充
    }

    fn outline_enabled(&self) -> bool {
        true
    }

    fn fill_shape(&self, _gc: &mut NdCanvas) {
        // Polyline 不支持填充
    }

    fn outline_shape(&self, gc: &mut NdCanvas) {
        if self.points.len() < 2 {
            return;
        }

        // 直接使用 Polyline 命令
        let points: Vec<glam::DVec2> = self
            .points
            .iter()
            .map(|p| glam::DVec2::new(p.0.x - self.bounds().x, p.0.y - self.bounds().y))
            .collect();

        gc.polyline(
            &points,
            self.stroke_color,
            self.stroke_width,
            self.line_cap,
            self.line_join,
        );
    }
}
