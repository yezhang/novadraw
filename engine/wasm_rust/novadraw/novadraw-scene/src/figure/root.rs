//! 根图形

use std::sync::Arc;

use novadraw_core::Color;
use novadraw_geometry::Rectangle;
use novadraw_render::NdCanvas;

use super::{Border, Bounded, ChildClippingStrategy, Figure, Shape, Updatable};

/// 根图形（内部使用）
///
/// 参考 draw2d 的 LightweightSystem.RootFigure 设计。
/// 用于表示 FigureGraph 内部的根容器，与用户设置的图形根区分。
///
/// 特点：
/// - 透明（不渲染自身）
/// - 使用 trait 默认的本地坐标模式（false）
/// - 不需要填充/描边属性
#[derive(Clone)]
pub struct RootFigure {
    /// 边界矩形
    bounds: Rectangle,
    /// 绘制子节点时使用的裁剪策略
    child_clipping_strategy: ChildClippingStrategy,
    /// 边框装饰器
    border: Option<Arc<dyn Border>>,
}

impl RootFigure {
    /// 创建新的根图形
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            bounds: Rectangle::new(x, y, width, height),
            child_clipping_strategy: ChildClippingStrategy::ClipToChildBounds,
            border: None,
        }
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

    /// 设置边界
    pub fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }
}

// 实现 Bounded trait
impl Bounded for RootFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }

    fn name(&self) -> &'static str {
        "RootFigure"
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
impl Updatable for RootFigure {
    fn validate(&mut self) {}
    fn invalidate(&mut self) {}
}

impl Figure for RootFigure {
    fn paint_figure(&self, gc: &mut NdCanvas) {
        Shape::paint_figure(self, gc);
    }

    fn paint_figure_in_bounds(&self, gc: &mut NdCanvas, bounds: Rectangle) {
        let mut local = self.clone();
        local.bounds = Rectangle::new(0.0, 0.0, bounds.width, bounds.height);
        Shape::paint_figure(&local, gc);
    }

    fn get_border(&self) -> Option<&dyn Border> {
        Shape::get_border(self)
    }
}

// 实现 Shape trait：根图形透明，不渲染自身
impl Shape for RootFigure {
    fn stroke_color(&self) -> Option<Color> {
        None
    }

    fn stroke_width(&self) -> f64 {
        0.0
    }

    fn fill_color(&self) -> Option<Color> {
        None
    }

    fn line_cap(&self) -> novadraw_render::command::LineCap {
        novadraw_render::command::LineCap::default()
    }

    fn line_join(&self) -> novadraw_render::command::LineJoin {
        novadraw_render::command::LineJoin::default()
    }

    fn get_border(&self) -> Option<&dyn Border> {
        self.border.as_deref()
    }

    fn fill_enabled(&self) -> bool {
        false
    }

    fn outline_enabled(&self) -> bool {
        false
    }

    fn fill_shape(&self, _gc: &mut NdCanvas) {}

    fn outline_shape(&self, _gc: &mut NdCanvas) {}
}
