//! 矩形图形

use std::sync::Arc;

use crate::Color;
use crate::geometry::Rectangle;
use crate::render::NdCanvas;

use super::{
    Border, BorderedFigure, Bounded, ChildClippingStrategy, Figure, FigureContainer, Shape,
};

/// 矩形图形
///
/// 用于渲染矩形形状。
/// 遵循 draw2d 设计：使用 `bounds: Rectangle` 统一管理边界，而非独立 x/y/width/height 字段。
#[derive(Clone)]
pub struct RectangleFigure {
    /// 边界矩形（包含 x, y, width, height）
    pub bounds: Rectangle,
    /// 填充颜色
    pub fill_color: Color,
    /// 边框颜色
    pub stroke_color: Option<Color>,
    /// 边框宽度
    pub stroke_width: f64,
    /// 线帽样式
    pub line_cap: crate::render::command::LineCap,
    /// 连接样式
    pub line_join: crate::render::command::LineJoin,
    /// 绘制子节点时使用的裁剪策略
    child_clipping_strategy: ChildClippingStrategy,
    /// 边框装饰器
    pub border: Option<Arc<dyn Border>>,
}

impl RectangleFigure {
    /// 创建矩形
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            bounds: Rectangle::new(x, y, width, height),
            fill_color: Color::from_hex("#3498db").expect("valid color literal"),
            stroke_color: None,
            stroke_width: 0.0,
            line_cap: crate::render::command::LineCap::default(),
            line_join: crate::render::command::LineJoin::default(),
            child_clipping_strategy: ChildClippingStrategy::ClipToChildBounds,
            border: None,
        }
    }

    /// 从 Rectangle 创建矩形
    pub fn from_bounds(bounds: Rectangle) -> Self {
        Self {
            bounds,
            fill_color: Color::from_hex("#3498db").expect("valid color literal"),
            stroke_color: None,
            stroke_width: 0.0,
            line_cap: crate::render::command::LineCap::default(),
            line_join: crate::render::command::LineJoin::default(),
            child_clipping_strategy: ChildClippingStrategy::ClipToChildBounds,
            border: None,
        }
    }

    /// 创建指定颜色的矩形
    pub fn new_with_color(x: f64, y: f64, width: f64, height: f64, color: Color) -> Self {
        Self {
            bounds: Rectangle::new(x, y, width, height),
            fill_color: color,
            stroke_color: None,
            stroke_width: 0.0,
            line_cap: crate::render::command::LineCap::default(),
            line_join: crate::render::command::LineJoin::default(),
            child_clipping_strategy: ChildClippingStrategy::ClipToChildBounds,
            border: None,
        }
    }

    /// 添加描边（Shape 级别）
    pub fn with_stroke(mut self, color: Color, width: f64) -> Self {
        self.stroke_color = Some(color);
        self.stroke_width = width;
        self
    }

    /// 设置子节点绘制裁剪策略。
    ///
    /// 对应 draw2d: setClippingStrategy(...)
    pub fn with_child_clipping_strategy(mut self, strategy: ChildClippingStrategy) -> Self {
        self.child_clipping_strategy = strategy;
        self
    }

    /// 添加边框装饰器（Border 级别）
    ///
    /// 对应 draw2d: setBorder()
    pub fn with_border(mut self, border: impl Border + 'static) -> Self {
        self.border = Some(Arc::new(border));
        self
    }

    /// 平移
    pub fn translate(&mut self, dx: f64, dy: f64) {
        self.bounds.x += dx;
        self.bounds.y += dy;
    }

    /// 设置边界（对应 draw2d: setBounds）
    pub fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }
}

// 实现 Bounded trait：边界相关方法
impl Bounded for RectangleFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }

    fn name(&self) -> &'static str {
        "RectangleFigure"
    }

    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        self.child_clipping_strategy
    }

    fn insets(&self) -> (f64, f64, f64, f64) {
        self.border
            .as_deref()
            .map(Border::get_insets)
            .unwrap_or((0.0, 0.0, 0.0, 0.0))
    }
}

impl Figure for RectangleFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "RectangleFigure"
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

impl BorderedFigure for RectangleFigure {
    fn border(&self) -> Option<&Arc<dyn Border>> {
        self.border.as_ref()
    }

    fn replace_border(&mut self, border: Option<Arc<dyn Border>>) -> Option<Arc<dyn Border>> {
        std::mem::replace(&mut self.border, border)
    }
}

impl FigureContainer for RectangleFigure {
    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        self.child_clipping_strategy
    }
}

// 实现 Shape trait：描边/填充相关方法
impl Shape for RectangleFigure {
    fn get_border(&self) -> Option<&dyn Border> {
        self.border.as_deref()
    }

    fn stroke_color(&self) -> Option<Color> {
        self.stroke_color
    }

    fn stroke_width(&self) -> f64 {
        self.stroke_width
    }

    fn fill_color(&self) -> Option<Color> {
        Some(self.fill_color)
    }

    fn line_cap(&self) -> crate::render::command::LineCap {
        self.line_cap
    }

    fn line_join(&self) -> crate::render::command::LineJoin {
        self.line_join
    }

    fn fill_enabled(&self) -> bool {
        self.fill_color.alpha() > 0.0
    }

    fn outline_enabled(&self) -> bool {
        self.stroke_color.map(|c| c.alpha() > 0.0).unwrap_or(false)
    }

    fn fill_shape(&self, gc: &mut NdCanvas) {
        gc.fill_rectangle(0.0, 0.0, self.bounds.width, self.bounds.height);
    }

    fn outline_shape(&self, gc: &mut NdCanvas) {
        if let Some(color) = self.stroke_color {
            // 参考 draw2d RectangleFigure.outlineShape:
            // 描边向内缩（inset），使描边完全在 bounds 内部
            // lineInset = max(1.0, strokeWidth) / 2.0
            let line_inset = (1.0_f64).max(self.stroke_width) / 2.0;

            // 向内缩 bounds（使用浮点数避免 floor/ceil 不对称）
            let x = line_inset;
            let y = line_inset;
            let width = self.bounds.width - line_inset * 2.0;
            let height = self.bounds.height - line_inset * 2.0;

            // 使用原始描边宽度
            // 数学原理：
            // - inset 后矩形：[x + sw/2, x + w - sw/2]
            // - 绘制宽为 sw 的描边，中心在 inset 矩形上
            // - 内边缘：x + sw/2 - sw/2 = x（原始左边界）
            // - 外边缘：x + w - sw/2 + sw/2 = x + w（原始右边界）
            // 这样描边正好填满原始 bounds
            let _ = color;
            gc.line_width(self.stroke_width);
            gc.line_cap(self.line_cap);
            gc.line_join(self.line_join);
            gc.draw_rectangle(x, y, width.max(0.0), height.max(0.0));
        }
    }
}
