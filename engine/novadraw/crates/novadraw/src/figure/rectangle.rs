//! 矩形图形
//!
//! ```compile_fail
//! use novadraw::{figure::RectangleFigure, geometry::Rectangle};
//! let mut figure = RectangleFigure::from_bounds(Rectangle::ZERO).unwrap();
//! figure.bounds = Rectangle::new(0.0, 0.0, f64::NAN, 1.0);
//! ```
//! ```compile_fail
//! use novadraw::figure::RectangleFigure;
//! let mut figure = RectangleFigure::new(0.0, 0.0, 1.0, 1.0).unwrap();
//! figure.stroke_width = -1.0;
//! ```
//! ```compile_fail
//! use novadraw::figure::RectangleFigure;
//! let figure: RectangleFigure = RectangleFigure::new(0.0, 0.0, 1.0, 1.0);
//! ```

use std::sync::Arc;

use crate::Color;
use crate::geometry::{Insets, Rectangle};
use crate::graphics::{GraphicsError, GraphicsInputError, PaintContext, StrokeStyle};

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

impl RectangleFigure {
    /// 创建矩形
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Result<Self, super::ShapeGeometryError> {
        Self::from_bounds(Rectangle::new(x, y, width, height))
    }

    /// 从 Rectangle 创建矩形
    pub fn from_bounds(bounds: Rectangle) -> Result<Self, super::ShapeGeometryError> {
        Ok(Self {
            bounds: super::ShapeGeometryError::validate_bounds(bounds)?,
            fill_color: Color::from_hex("#3498db").expect("valid color literal"),
            stroke_color: None,
            stroke: StrokeStyle::default()
                .with_width(0.0)
                .expect("zero shape stroke"),
            child_clipping_strategy: ChildClippingStrategy::ClipToChildBounds,
            border: None,
        })
    }

    /// 创建指定颜色的矩形
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

    /// 添加描边（Shape 级别）
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
    ///
    /// 对应 draw2d: setClippingStrategy(...)
    pub fn with_child_clipping_strategy(mut self, strategy: ChildClippingStrategy) -> Self {
        self.child_clipping_strategy = strategy;
        self
    }

    /// 添加边框装饰器（Border 级别）
    ///
    /// 对应 draw2d: setBorder()
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

    /// 设置边界（对应 draw2d: setBounds）
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
}

// 实现 Bounded trait：边界相关方法
impl Bounded for RectangleFigure {
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
        "RectangleFigure"
    }

    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        self.child_clipping_strategy
    }

    fn insets(&self) -> Insets {
        self.border
            .as_deref()
            .map(Border::get_insets)
            .unwrap_or(Insets::ZERO)
    }
}

impl Figure for RectangleFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "RectangleFigure"
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

impl super::FigureBoundsSnapshot for RectangleFigure {
    fn snapshot_bounds_drawing(&self) -> Result<super::BoundsDrawing, GraphicsError> {
        #[derive(Clone, Copy)]
        struct Inputs {
            fill: Color,
            stroke: Option<Color>,
        }
        const RECTANGLE_RECORDING_BYTES: usize = 4096;
        super::BoundsDrawing::stroked(
            Inputs {
                fill: self.fill_color,
                stroke: self.stroke_color,
            },
            self.stroke.clone(),
            RECTANGLE_RECORDING_BYTES,
            |inputs, stroke, bounds, context| {
                let figure = RectangleFigure {
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

impl BorderedFigure for RectangleFigure {
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

    fn fill_enabled(&self) -> bool {
        self.fill_color.alpha() > 0.0
    }

    fn outline_enabled(&self) -> bool {
        self.stroke_color.map(|c| c.alpha() > 0.0).unwrap_or(false)
    }

    fn fill_shape(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        context.fill_rect(Rectangle::new(
            0.0,
            0.0,
            self.bounds.width,
            self.bounds.height,
        ))
    }

    fn outline_shape(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        if self.stroke_color.is_some() {
            // 参考 draw2d RectangleFigure.outlineShape:
            // 描边向内缩（inset），使描边完全在 bounds 内部
            // lineInset = max(1.0, strokeWidth) / 2.0
            let line_inset = (1.0_f64).max(self.stroke.width()) / 2.0;

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
            context.set_stroke_style(self.stroke.clone());
            context.stroke_rect(Rectangle::new(x, y, width.max(0.0), height.max(0.0)))?;
        }
        Ok(())
    }
}
