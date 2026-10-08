//! 折线图形

use std::sync::Arc;

use crate::Color;
use crate::geometry::{Insets, Rectangle};
use crate::render::{NdCanvas, StrokeStyle};

use super::{
    Border, BorderedFigure, Bounded, ChildClippingStrategy, Figure, FigureContainer, Shape,
};

const DEFAULT_HIT_TOLERANCE: f64 = 2.0;
const DEFAULT_STROKE_WIDTH: f64 = 2.0;

/// 折线图形
///
/// 参考 Eclipse Draw2D 的 Polyline 设计。
/// 使用点列表存储多个顶点，可以绘制任意折线。
/// bounds 是自动计算的，基于点列表并扩展线宽。
///
/// 注意：不能通过 set_bounds 定位，应该通过 add_point/set_points 操作点。
#[derive(Clone)]
pub struct PolylineFigure {
    /// Node-local 点列表
    points: Vec<crate::geometry::Point>,
    /// 构建期 bounds；进入 FigureTree 后由 NodeState 接管。
    bounds: Rectangle,
    /// 线条颜色
    pub stroke_color: Color,
    /// 经过校验的完整描边。
    stroke: StrokeStyle,
    /// 命中测试使用的最小容差
    hit_tolerance: f64,
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
        Self::from_points(vec![
            crate::geometry::Point::new(x1, y1),
            crate::geometry::Point::new(x2, y2),
        ])
    }

    /// 从点列表创建折线
    pub fn from_points(points: Vec<crate::geometry::Point>) -> Self {
        let stroke = StrokeStyle::default()
            .with_width(DEFAULT_STROKE_WIDTH)
            .expect("valid default stroke");
        let (bounds, points) = normalize_points(points, &stroke, 2);
        Self {
            points,
            bounds,
            stroke_color: Color::from_hex("#2c3e50").expect("valid color literal"),
            stroke,
            hit_tolerance: DEFAULT_HIT_TOLERANCE,
            child_clipping_strategy: ChildClippingStrategy::ClipToChildBounds,
            border: None,
        }
    }

    /// 创建指定颜色的折线
    pub fn new_with_color(x1: f64, y1: f64, x2: f64, y2: f64, color: Color) -> Self {
        Self::new(x1, y1, x2, y2).with_color(color)
    }

    /// 添加点
    pub fn add_point(&mut self, x: f64, y: f64) {
        let mut points = self.parent_points();
        points.push(crate::geometry::Point::new(x, y));
        self.set_points(points);
    }

    /// 获取点列表（引用）
    pub fn get_points(&self) -> &[crate::geometry::Point] {
        &self.points
    }

    /// 设置点列表
    pub fn set_points(&mut self, points: Vec<crate::geometry::Point>) {
        (self.bounds, self.points) = normalize_points(points, &self.stroke, 2);
    }

    /// 获取起点
    pub fn start_point(&self) -> Option<crate::geometry::Point> {
        self.points.first().copied()
    }

    /// 获取终点
    pub fn end_point(&self) -> Option<crate::geometry::Point> {
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
        let points = self.parent_points();
        self.stroke = self
            .stroke
            .with_width(width.max(0.0))
            .expect("valid stroke width");
        (self.bounds, self.points) = normalize_points(points, &self.stroke, 2);
        self
    }

    /// Replaces the complete checked stroke and recomputes its envelope.
    pub fn with_stroke_style(mut self, stroke: StrokeStyle) -> Self {
        let points = self.parent_points();
        (self.bounds, self.points) = normalize_points(points, &stroke, 2);
        self.stroke = stroke;
        self
    }

    pub fn stroke_style(&self) -> &StrokeStyle {
        &self.stroke
    }

    /// 设置线帽样式
    pub fn with_cap(mut self, cap: crate::render::command::LineCap) -> Self {
        let points = self.parent_points();
        self.stroke = self.stroke.with_cap(cap);
        (self.bounds, self.points) = normalize_points(points, &self.stroke, 2);
        self
    }

    /// 设置连接样式
    pub fn with_join(mut self, join: crate::render::command::LineJoin) -> Self {
        let points = self.parent_points();
        self.stroke = self.stroke.with_join(join);
        (self.bounds, self.points) = normalize_points(points, &self.stroke, 2);
        self
    }

    /// 设置精确命中的最小容差。
    pub fn with_hit_tolerance(mut self, tolerance: f64) -> Self {
        self.hit_tolerance = tolerance.max(0.0);
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
    pub(crate) fn parent_points(&self) -> Vec<crate::geometry::Point> {
        self.points
            .iter()
            .map(|point| {
                crate::geometry::Point::new(point.x() + self.bounds.x, point.y() + self.bounds.y)
            })
            .collect()
    }

    pub(crate) fn commit_geometry(
        &mut self,
        bounds: Rectangle,
        local_points: Vec<crate::geometry::Point>,
    ) {
        self.bounds = bounds;
        self.points = local_points;
    }

    pub(crate) fn local_points(&self) -> &[crate::geometry::Point] {
        &self.points
    }

    pub(crate) fn painted_minimum(&self) -> usize {
        2
    }

    pub(crate) fn commit_stroke_style(&mut self, stroke: StrokeStyle) {
        self.stroke = stroke;
    }

    pub(crate) fn renormalize_for_minimum(&mut self, painted_minimum: usize) {
        let points = self.parent_points();
        (self.bounds, self.points) = normalize_points(points, &self.stroke, painted_minimum);
    }
}

// 实现 Bounded trait
impl Bounded for PolylineFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        // 折线通过点定义，set_bounds 需要重新计算点位置
        let current_bounds = self.bounds;
        if current_bounds.width == 0.0 || current_bounds.height == 0.0 {
            return;
        }
        let scale_x = width / current_bounds.width;
        let scale_y = height / current_bounds.height;

        let new_points: Vec<crate::geometry::Point> = self
            .points
            .iter()
            .map(|p| crate::geometry::Point::new(p.x() * scale_x, p.y() * scale_y))
            .collect();
        self.points = new_points;
        self.bounds = Rectangle::new(x, y, width, height);
    }

    fn name(&self) -> &'static str {
        "PolylineFigure"
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

impl Figure for PolylineFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "PolylineFigure"
    }

    fn initial_insets(&self) -> Insets {
        Bounded::insets(self)
    }

    fn initial_style(&self) -> crate::FigureStyle {
        crate::FigureStyle {
            foreground: Some(self.stroke_color),
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
        let tolerance = (self.stroke.width() / 2.0).max(self.hit_tolerance);
        self.points.windows(2).any(|segment| {
            point_segment_distance_squared(
                x,
                y,
                segment[0].x(),
                segment[0].y(),
                segment[1].x(),
                segment[1].y(),
            ) <= tolerance * tolerance
        })
    }

    fn get_border(&self) -> Option<&dyn Border> {
        Shape::get_border(self)
    }

    fn register_capabilities(
        &self,
        out: &mut crate::FigureCapabilityBuilder,
    ) -> Result<(), crate::FigureCapabilityRegistrationError> {
        out.register(crate::CONTAINER, crate::ContainerCapability::of::<Self>())?;
        out.register(crate::BORDER, crate::BorderCapability::of::<Self>())
    }
}

impl BorderedFigure for PolylineFigure {
    fn border(&self) -> Option<&Arc<dyn Border>> {
        self.border.as_ref()
    }

    fn replace_border(&mut self, border: Option<Arc<dyn Border>>) -> Option<Arc<dyn Border>> {
        std::mem::replace(&mut self.border, border)
    }
}

pub(crate) fn point_segment_distance_squared(
    px: f64,
    py: f64,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
) -> f64 {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let length_squared = dx * dx + dy * dy;
    if length_squared == 0.0 {
        return (px - x1).powi(2) + (py - y1).powi(2);
    }
    let projection = (((px - x1) * dx + (py - y1) * dy) / length_squared).clamp(0.0, 1.0);
    let nearest_x = x1 + projection * dx;
    let nearest_y = y1 + projection * dy;
    (px - nearest_x).powi(2) + (py - nearest_y).powi(2)
}

impl FigureContainer for PolylineFigure {
    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        self.child_clipping_strategy
    }
}

// 实现 Shape trait
impl Shape for PolylineFigure {
    fn stroke_color(&self) -> Option<Color> {
        Some(self.stroke_color)
    }

    fn stroke_width(&self) -> f64 {
        self.stroke.width()
    }

    fn fill_color(&self) -> Option<Color> {
        None // Polyline 不支持填充
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
        gc.set_stroke(self.stroke.clone());

        gc.begin_path();
        gc.move_to(self.points[0].x(), self.points[0].y());
        for point in &self.points[1..] {
            gc.line_to(point.x(), point.y());
        }
        gc.stroke();
    }
}

pub(crate) fn normalize_points(
    points: Vec<crate::geometry::Point>,
    stroke: &StrokeStyle,
    painted_minimum: usize,
) -> (Rectangle, Vec<crate::geometry::Point>) {
    if points.is_empty() {
        return (Rectangle::ZERO, points);
    }
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for point in &points {
        min_x = min_x.min(point.x());
        min_y = min_y.min(point.y());
        max_x = max_x.max(point.x());
        max_y = max_y.max(point.y());
    }
    let expansion = if points.len() >= painted_minimum {
        stroke.visual_outset()
    } else {
        0.0
    };
    let bounds = Rectangle::new(
        min_x - expansion,
        min_y - expansion,
        max_x - min_x + expansion * 2.0,
        max_y - min_y + expansion * 2.0,
    );
    let local = points
        .into_iter()
        .map(|point| crate::geometry::Point::new(point.x() - bounds.x, point.y() - bounds.y))
        .collect();
    (bounds, local)
}
