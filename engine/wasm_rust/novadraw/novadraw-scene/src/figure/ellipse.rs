//! 椭圆图形

use std::sync::Arc;

use novadraw_core::Color;
use novadraw_geometry::Rectangle;
use novadraw_render::NdCanvas;

use super::{
    Border, BorderedFigure, Bounded, ChildClippingStrategy, Figure, FigureContainer, Shape,
};

/// 椭圆图形
///
/// 用于渲染椭圆形状。
/// 椭圆外切于 bounds 矩形。
#[derive(Clone)]
pub struct EllipseFigure {
    /// 边界矩形（外切矩形）
    pub bounds: Rectangle,
    /// 填充颜色
    pub fill_color: Color,
    /// 边框颜色
    pub stroke_color: Option<Color>,
    /// 边框宽度
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

impl EllipseFigure {
    fn optimized_bounds(&self) -> Rectangle {
        let line_inset = 1.0_f64.max(self.stroke_width) / 2.0;
        Rectangle::new(
            line_inset,
            line_inset,
            (self.bounds.width - line_inset * 2.0).max(0.0),
            (self.bounds.height - line_inset * 2.0).max(0.0),
        )
    }

    /// 创建椭圆
    ///
    /// 椭圆外切于指定的 bounds 矩形
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            bounds: Rectangle::new(x, y, width, height),
            fill_color: Color::hex("#e74c3c"),
            stroke_color: None,
            stroke_width: 0.0,
            line_cap: novadraw_render::command::LineCap::default(),
            line_join: novadraw_render::command::LineJoin::default(),
            child_clipping_strategy: ChildClippingStrategy::ClipToChildBounds,
            border: None,
        }
    }

    /// 从 Rectangle 创建椭圆
    pub fn from_bounds(bounds: Rectangle) -> Self {
        Self {
            bounds,
            fill_color: Color::hex("#e74c3c"),
            stroke_color: None,
            stroke_width: 0.0,
            line_cap: novadraw_render::command::LineCap::default(),
            line_join: novadraw_render::command::LineJoin::default(),
            child_clipping_strategy: ChildClippingStrategy::ClipToChildBounds,
            border: None,
        }
    }

    /// 创建指定颜色的椭圆
    pub fn new_with_color(x: f64, y: f64, width: f64, height: f64, color: Color) -> Self {
        Self {
            bounds: Rectangle::new(x, y, width, height),
            fill_color: color,
            stroke_color: None,
            stroke_width: 0.0,
            line_cap: novadraw_render::command::LineCap::default(),
            line_join: novadraw_render::command::LineJoin::default(),
            child_clipping_strategy: ChildClippingStrategy::ClipToChildBounds,
            border: None,
        }
    }

    /// 添加边框
    pub fn with_stroke(mut self, color: Color, width: f64) -> Self {
        self.stroke_color = Some(color);
        self.stroke_width = width;
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

    /// 平移
    pub fn translate(&mut self, dx: f64, dy: f64) {
        self.bounds.x += dx;
        self.bounds.y += dy;
    }

    /// 设置边界
    pub fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }

    /// 获取椭圆中心 x
    pub fn cx(&self) -> f64 {
        self.bounds.x + self.bounds.width / 2.0
    }

    /// 获取椭圆中心 y
    pub fn cy(&self) -> f64 {
        self.bounds.y + self.bounds.height / 2.0
    }

    /// 获取 x 轴半径
    pub fn rx(&self) -> f64 {
        self.bounds.width / 2.0
    }

    /// 获取 y 轴半径
    pub fn ry(&self) -> f64 {
        self.bounds.height / 2.0
    }
}

// 实现 Bounded trait
impl Bounded for EllipseFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }

    fn name(&self) -> &'static str {
        "EllipseFigure"
    }

    fn contains_point(&self, x: f64, y: f64) -> bool {
        let radius_x = self.bounds.width / 2.0;
        let radius_y = self.bounds.height / 2.0;
        if radius_x <= 0.0 || radius_y <= 0.0 {
            return false;
        }
        let center_x = radius_x;
        let center_y = radius_y;
        let normalized_x = (x - center_x) / radius_x;
        let normalized_y = (y - center_y) / radius_y;
        normalized_x * normalized_x + normalized_y * normalized_y <= 1.0
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

impl Figure for EllipseFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "EllipseFigure"
    }

    fn initial_insets(&self) -> (f64, f64, f64, f64) {
        Bounded::insets(self)
    }

    fn initial_style(&self) -> crate::FigureStyle {
        crate::FigureStyle {
            foreground: self.stroke_color,
            background: Some(self.fill_color),
            ..crate::FigureStyle::default()
        }
    }

    fn paint_figure(&self, gc: &mut NdCanvas) {
        Shape::paint_figure(self, gc);
    }

    fn paint_figure_in_bounds(&self, gc: &mut NdCanvas, bounds: Rectangle) {
        let mut local = self.clone();
        local.bounds = Rectangle::new(0.0, 0.0, bounds.width, bounds.height);
        Shape::paint_figure(&local, gc);
    }

    fn precise_hit(&self, x: f64, y: f64, bounds: Rectangle) -> bool {
        let radius_x = bounds.width / 2.0;
        let radius_y = bounds.height / 2.0;
        if radius_x <= 0.0 || radius_y <= 0.0 {
            return false;
        }
        let normalized_x = (x - radius_x) / radius_x;
        let normalized_y = (y - radius_y) / radius_y;
        normalized_x * normalized_x + normalized_y * normalized_y <= 1.0
    }

    fn get_border(&self) -> Option<&dyn Border> {
        Shape::get_border(self)
    }

    fn container(&self) -> Option<&dyn FigureContainer> {
        Some(self)
    }

    fn bordered_mut(&mut self) -> Option<&mut dyn BorderedFigure> {
        Some(self)
    }
}

impl BorderedFigure for EllipseFigure {
    fn border(&self) -> Option<&Arc<dyn Border>> {
        self.border.as_ref()
    }

    fn replace_border(&mut self, border: Option<Arc<dyn Border>>) -> Option<Arc<dyn Border>> {
        std::mem::replace(&mut self.border, border)
    }
}

impl FigureContainer for EllipseFigure {
    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        self.child_clipping_strategy
    }
}

// 实现 Shape trait
impl Shape for EllipseFigure {
    fn stroke_color(&self) -> Option<Color> {
        self.stroke_color
    }

    fn stroke_width(&self) -> f64 {
        self.stroke_width
    }

    fn fill_color(&self) -> Option<Color> {
        Some(self.fill_color)
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
        self.fill_color.a > 0.0
    }

    fn outline_enabled(&self) -> bool {
        self.stroke_color.map(|c| c.a > 0.0).unwrap_or(false)
    }

    fn fill_shape(&self, gc: &mut NdCanvas) {
        let bounds = self.optimized_bounds();
        gc.fill_oval(bounds.x, bounds.y, bounds.width, bounds.height);
    }

    fn outline_shape(&self, gc: &mut NdCanvas) {
        if let Some(color) = self.stroke_color {
            let bounds = self.optimized_bounds();
            let _ = color;
            gc.line_width(self.stroke_width);
            gc.line_cap(self.line_cap);
            gc.line_join(self.line_join);
            gc.draw_oval(bounds.x, bounds.y, bounds.width, bounds.height);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_point_uses_ellipse_geometry_not_only_bounds() {
        let ellipse = EllipseFigure::new(10.0, 20.0, 100.0, 60.0);

        assert!(ellipse.contains_point(50.0, 30.0));
        assert!(ellipse.contains_point(0.0, 30.0));
        assert!(!ellipse.contains_point(0.0, 0.0));
        assert!(!ellipse.contains_point(99.0, 1.0));
    }
}
