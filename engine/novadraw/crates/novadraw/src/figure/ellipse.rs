//! 椭圆图形

use std::sync::Arc;

use crate::Color;
use crate::geometry::{Insets, Rectangle};
use crate::graphics::{GraphicsError, GraphicsInputError, PaintContext, StrokeStyle};

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
    bounds: Rectangle,
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
}

impl EllipseFigure {
    fn optimized_bounds(&self) -> Rectangle {
        let line_inset = 1.0_f64.max(self.stroke.width()) / 2.0;
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
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Result<Self, super::ShapeGeometryError> {
        Self::from_bounds(Rectangle::new(x, y, width, height))
    }

    /// 从 Rectangle 创建椭圆
    pub fn from_bounds(bounds: Rectangle) -> Result<Self, super::ShapeGeometryError> {
        Ok(Self {
            bounds: super::ShapeGeometryError::validate_bounds(bounds)?,
            fill_color: Color::from_hex("#e74c3c").expect("valid color literal"),
            stroke_color: None,
            stroke: StrokeStyle::default()
                .with_width(0.0)
                .expect("zero shape stroke"),
            child_clipping_strategy: ChildClippingStrategy::ClipToChildBounds,
            border: None,
        })
    }

    /// 创建指定颜色的椭圆
    pub fn new_with_color(
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        color: Color,
    ) -> Result<Self, super::ShapeGeometryError> {
        let mut figure = Self::new(x, y, width, height)?;
        figure.fill_color = color;
        Ok(figure)
    }

    /// 添加边框
    pub fn with_stroke(mut self, color: Color, width: f64) -> Result<Self, GraphicsInputError> {
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
    pub fn translate(&mut self, dx: f64, dy: f64) -> Result<(), super::ShapeGeometryError> {
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
    ) -> Result<(), super::ShapeGeometryError> {
        self.bounds =
            super::ShapeGeometryError::validate_bounds(Rectangle::new(x, y, width, height))?;
        Ok(())
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

    fn set_bounds(
        &mut self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Result<(), super::ShapeGeometryError> {
        Self::set_bounds(self, x, y, width, height)
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

    fn insets(&self) -> Insets {
        self.border
            .as_ref()
            .map(|border| border.get_insets())
            .unwrap_or(Insets::ZERO)
    }
}

impl Figure for EllipseFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "EllipseFigure"
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

    fn register_capabilities(
        &self,
        out: &mut crate::FigureCapabilityBuilder,
    ) -> Result<(), crate::FigureCapabilityRegistrationError> {
        out.register(crate::CONTAINER, crate::ContainerCapability::of::<Self>())?;
        out.register(
            super::BOUNDS_SNAPSHOT,
            super::BoundsSnapshotCapability::of::<Self>(),
        )?;
        out.register(crate::BORDER, crate::BorderCapability::of::<Self>())
    }
}

impl super::FigureBoundsSnapshot for EllipseFigure {
    fn snapshot_bounds_drawing(&self) -> Result<super::BoundsDrawing, GraphicsError> {
        #[derive(Clone, Copy)]
        struct Inputs {
            fill: Color,
            stroke: Option<Color>,
        }
        const ELLIPSE_RECORDING_BYTES: usize = 4096;
        super::BoundsDrawing::stroked(
            Inputs {
                fill: self.fill_color,
                stroke: self.stroke_color,
            },
            self.stroke.clone(),
            ELLIPSE_RECORDING_BYTES,
            |inputs, stroke, bounds, context| {
                let figure = EllipseFigure {
                    bounds,
                    fill_color: inputs.fill,
                    stroke_color: inputs.stroke,
                    stroke: stroke.clone(),
                    child_clipping_strategy: ChildClippingStrategy::ClipToChildBounds,
                    border: None,
                };
                Shape::paint_shape(&figure, context)
            },
            |_, _, bounds| Ok(bounds),
        )
    }
}

impl BorderedFigure for EllipseFigure {
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
        self.stroke_color.map(|c| c.alpha() > 0.0).unwrap_or(false)
    }

    fn fill_shape(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        let bounds = self.optimized_bounds();
        context.fill_ellipse(bounds)
    }

    fn outline_shape(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        if self.stroke_color.is_some() {
            let bounds = self.optimized_bounds();
            context.set_stroke_style(self.stroke.clone());
            context.stroke_ellipse(bounds)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_ellipse_uses_sampled_radii_without_scaling_stroke() {
        let ellipse = EllipseFigure::new(0.0, 0.0, 100.0, 60.0)
            .expect("valid figure bounds")
            .with_stroke(Color::BLACK, 3.0)
            .expect("valid detached figure input");
        let frozen = super::super::FigureBoundsSnapshot::snapshot_bounds_drawing(&ellipse).unwrap();
        for bounds in [
            Rectangle::new(0.0, 0.0, 50.0, 30.0),
            Rectangle::new(0.0, 0.0, 2.0, 1.0),
            Rectangle::new(0.0, 0.0, 0.0, 0.0),
        ] {
            let mut expected = crate::render::CommandRecorder::new();
            expected.canvas.set_foreground_color(Color::BLACK);
            expected.canvas.set_background_color(Color::WHITE);
            Figure::paint(&ellipse, &mut expected.paint(), bounds).unwrap();
            let mut actual = crate::render::CommandRecorder::new();
            frozen
                .materialize(bounds, Color::BLACK, Color::WHITE)
                .unwrap()
                .paint(&mut actual.paint())
                .unwrap();
            let ellipses = |commands: &[crate::render::RenderCommand]| {
                commands
                    .iter()
                    .filter(|command| {
                        matches!(
                            command.kind,
                            crate::render::RenderCommandKind::Ellipse { .. }
                        )
                    })
                    .map(|command| command.kind.clone())
                    .collect::<Vec<_>>()
            };
            assert_eq!(ellipses(expected.commands()), ellipses(actual.commands()));
        }
    }

    #[test]
    fn contains_point_uses_ellipse_geometry_not_only_bounds() {
        let ellipse = EllipseFigure::new(10.0, 20.0, 100.0, 60.0).expect("valid figure bounds");

        assert!(ellipse.contains_point(50.0, 30.0));
        assert!(ellipse.contains_point(0.0, 30.0));
        assert!(!ellipse.contains_point(0.0, 0.0));
        assert!(!ellipse.contains_point(99.0, 1.0));
    }
}
