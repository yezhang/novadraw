use crate::Color;
use crate::geometry::{Insets, Point, Rectangle};
use crate::graphics::{GraphicsError, PaintContext, StrokeStyle};
use crate::render::command::{LineCap, LineJoin};

use super::{Border, inset_rectangle};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BevelStyle {
    Raised,
    Lowered,
}

/// Multi-line highlight/shadow border.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BevelBorder {
    style: BevelStyle,
    highlight: Color,
    shadow: Color,
    width: u32,
}

impl BevelBorder {
    pub fn new(style: BevelStyle, highlight: Color, shadow: Color, width: u32) -> Self {
        assert!(width > 0, "bevel border width must be positive");
        Self {
            style,
            highlight,
            shadow,
            width,
        }
    }

    fn colors(self) -> (Color, Color) {
        match self.style {
            BevelStyle::Raised => (self.highlight, self.shadow),
            BevelStyle::Lowered => (self.shadow, self.highlight),
        }
    }
}

impl Border for BevelBorder {
    fn get_insets(&self) -> Insets {
        let width = f64::from(self.width);
        Insets::uniform(width)
    }

    fn is_opaque(&self) -> bool {
        self.highlight.is_opaque() && self.shadow.is_opaque()
    }

    fn paint(
        &self,
        figure_bounds: Rectangle,
        context: &mut PaintContext<'_>,
    ) -> Result<(), GraphicsError> {
        self.paint_with_insets(figure_bounds, Insets::ZERO, context)
    }

    fn paint_with_insets(
        &self,
        figure_bounds: Rectangle,
        incoming: Insets,
        context: &mut PaintContext<'_>,
    ) -> Result<(), GraphicsError> {
        let bounds = inset_rectangle(figure_bounds, incoming);
        let (top_left, bottom_right) = self.colors();
        context.set_stroke_style(
            StrokeStyle::default()
                .with_cap(LineCap::Butt)
                .with_join(LineJoin::Miter),
        );
        for layer in 0..self.width {
            let offset = f64::from(layer) + 0.5;
            let left = bounds.x + offset;
            let top = bounds.y + offset;
            let right = bounds.x + bounds.width - offset;
            let bottom = bounds.y + bounds.height - offset;
            if right < left || bottom < top {
                break;
            }
            context.set_stroke_paint(top_left);
            context.stroke_line(Point::new(left, bottom), Point::new(left, top))?;
            context.stroke_line(Point::new(left, top), Point::new(right, top))?;
            context.set_stroke_paint(bottom_right);
            context.stroke_line(Point::new(right, top), Point::new(right, bottom))?;
            context.stroke_line(Point::new(right, bottom), Point::new(left, bottom))?;
        }
        Ok(())
    }
}
