//! 圆角矩形图形

use std::sync::Arc;

use crate::Color;
use crate::geometry::{Dimension, Rectangle};
use crate::render::NdCanvas;

use super::{
    Border, BorderedFigure, Bounded, ChildClippingStrategy, Figure, FigureContainer, Shape,
};

/// 圆角矩形图形
///
/// 参考 Eclipse Draw2D 的 RoundedRectangle 设计。
/// 在矩形基础上添加圆角半径，支持填充和描边。
#[derive(Clone)]
pub struct RoundedRectangleFigure {
    /// 边界矩形
    pub bounds: Rectangle,
    /// 圆角宽高，对应 Draw2D `corner.width/height`。
    pub corner_dimensions: Dimension,
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
    border: Option<Arc<dyn Border>>,
}

impl RoundedRectangleFigure {
    /// 创建圆角矩形
    ///
    /// `corner_radius` 为圆角半径，如果为 0 则退化为普通矩形
    pub fn new(x: f64, y: f64, width: f64, height: f64, corner_radius: f64) -> Self {
        Self {
            bounds: Rectangle::new(x, y, width, height),
            corner_dimensions: Dimension::new(corner_radius.max(0.0), corner_radius.max(0.0)),
            fill_color: Color::from_hex("#9b59b6").expect("valid color literal"),
            stroke_color: None,
            stroke_width: 0.0,
            line_cap: crate::render::command::LineCap::default(),
            line_join: crate::render::command::LineJoin::default(),
            child_clipping_strategy: ChildClippingStrategy::ClipToChildBounds,
            border: None,
        }
    }

    /// 从 Rectangle 创建圆角矩形
    pub fn from_bounds(bounds: Rectangle, corner_radius: f64) -> Self {
        Self {
            bounds,
            corner_dimensions: Dimension::new(corner_radius.max(0.0), corner_radius.max(0.0)),
            fill_color: Color::from_hex("#9b59b6").expect("valid color literal"),
            stroke_color: None,
            stroke_width: 0.0,
            line_cap: crate::render::command::LineCap::default(),
            line_join: crate::render::command::LineJoin::default(),
            child_clipping_strategy: ChildClippingStrategy::ClipToChildBounds,
            border: None,
        }
    }

    /// 创建指定颜色的圆角矩形
    pub fn new_with_color(
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        corner_radius: f64,
        color: Color,
    ) -> Self {
        Self {
            bounds: Rectangle::new(x, y, width, height),
            corner_dimensions: Dimension::new(corner_radius.max(0.0), corner_radius.max(0.0)),
            fill_color: color,
            stroke_color: None,
            stroke_width: 0.0,
            line_cap: crate::render::command::LineCap::default(),
            line_join: crate::render::command::LineJoin::default(),
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

    /// 设置圆角半径
    pub fn set_corner_radius(&mut self, radius: f64) {
        self.corner_dimensions = Dimension::new(radius.max(0.0), radius.max(0.0));
    }

    pub fn set_corner_dimensions(&mut self, dimensions: Dimension) {
        self.corner_dimensions =
            Dimension::new(dimensions.width.max(0.0), dimensions.height.max(0.0));
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

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }

    fn name(&self) -> &'static str {
        "RoundedRectangleFigure"
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

impl Figure for RoundedRectangleFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "RoundedRectangleFigure"
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

    fn container(&self) -> Option<&dyn FigureContainer> {
        Some(self)
    }

    fn bordered_mut(&mut self) -> Option<&mut dyn BorderedFigure> {
        Some(self)
    }
}

impl BorderedFigure for RoundedRectangleFigure {
    fn border(&self) -> Option<&Arc<dyn Border>> {
        self.border.as_ref()
    }

    fn replace_border(&mut self, border: Option<Arc<dyn Border>>) -> Option<Arc<dyn Border>> {
        std::mem::replace(&mut self.border, border)
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

    fn get_border(&self) -> Option<&dyn Border> {
        self.border.as_deref()
    }

    fn fill_enabled(&self) -> bool {
        self.fill_color.alpha() > 0.0
    }

    fn outline_enabled(&self) -> bool {
        self.stroke_color.map(|c| c.alpha() > 0.0).unwrap_or(false)
    }

    fn fill_shape(&self, gc: &mut NdCanvas) {
        let mut local = self.clone();
        local.bounds.x = 0.0;
        local.bounds.y = 0.0;
        local.draw_rounded_rect(gc, Some(self.fill_color), None);
    }

    fn outline_shape(&self, gc: &mut NdCanvas) {
        if let Some(color) = self.stroke_color {
            // 参考 draw2d RectangleFigure.outlineShape:
            // 描边向内缩，使描边完全在 bounds 内部
            let line_inset = (1.0_f64).max(self.stroke_width) / 2.0;

            // 向内缩 bounds
            let x = line_inset;
            let y = line_inset;
            let width = self.bounds.width - line_inset * 2.0;
            let height = self.bounds.height - line_inset * 2.0;
            let corner_dimensions = Dimension::new(
                (self.corner_dimensions.width - line_inset).max(0.0),
                (self.corner_dimensions.height - line_inset).max(0.0),
            );

            if width <= 0.0 || height <= 0.0 {
                return;
            }

            // 创建临时圆角矩形进行描边
            let temp_rect = RoundedRectangleFigure {
                bounds: Rectangle::new(x, y, width, height),
                corner_dimensions,
                fill_color: Color::TRANSPARENT,
                stroke_color: Some(color),
                stroke_width: self.stroke_width, // 使用原始描边宽度
                line_cap: self.line_cap,
                line_join: self.line_join,
                child_clipping_strategy: self.child_clipping_strategy,
                border: None,
            };
            temp_rect.draw_rounded_rect(gc, None, Some(color));
        }
    }
}

impl RoundedRectangleFigure {
    /// 绘制圆角矩形（填充和/或描边）
    fn draw_rounded_rect(
        &self,
        gc: &mut NdCanvas,
        fill_color: Option<Color>,
        stroke_color: Option<Color>,
    ) {
        let x = self.bounds.x;
        let y = self.bounds.y;
        let width = self.bounds.width;
        let height = self.bounds.height;
        let radius_x = self.corner_dimensions.width.min(width).max(0.0) / 2.0;
        let radius_y = self.corner_dimensions.height.min(height).max(0.0) / 2.0;

        // 边界检查
        if width <= 0.0 || height <= 0.0 {
            return;
        }

        // 如果没有圆角，退化为普通矩形
        if radius_x <= 0.0 || radius_y <= 0.0 {
            if let Some(color) = fill_color {
                gc.fill_rect_with_color(x, y, width, height, color);
            }
            if let Some(color) = stroke_color {
                gc.stroke_rect_with_style(
                    x,
                    y,
                    width,
                    height,
                    color,
                    self.stroke_width,
                    self.line_cap,
                    self.line_join,
                );
            }
            return;
        }

        // 使用 Path API 构建圆角矩形
        gc.begin_path();

        // 从左上角开始
        gc.move_to(x + radius_x, y);

        // 上边
        gc.line_to(x + width - radius_x, y);

        // 右上角圆弧
        gc.quadratic_curve_to(x + width, y, x + width, y + radius_y);

        // 右边
        gc.line_to(x + width, y + height - radius_y);

        // 右下角圆弧
        gc.quadratic_curve_to(x + width, y + height, x + width - radius_x, y + height);

        // 下边
        gc.line_to(x + radius_x, y + height);

        // 左下角圆弧
        gc.quadratic_curve_to(x, y + height, x, y + height - radius_y);

        // 左边
        gc.line_to(x, y + radius_y);

        // 左上角圆弧
        gc.quadratic_curve_to(x, y, x + radius_x, y);

        gc.close_path();

        // 填充
        if let Some(color) = fill_color {
            let _ = color;
            gc.fill();
        }

        // 描边
        if let Some(color) = stroke_color {
            let _ = color;
            gc.line_width(self.stroke_width);
            gc.line_cap(self.line_cap);
            gc.line_join(self.line_join);
            gc.stroke();
        }
    }
}
