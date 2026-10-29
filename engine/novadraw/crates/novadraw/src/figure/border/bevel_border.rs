use crate::Color;
use crate::geometry::{Insets, Point, Rectangle};
use crate::graphics::{GraphicsError, PaintContext, StrokeStyle};
use crate::render::command::{LineCap, LineJoin};

use super::{Border, BorderInputError, inset_rectangle};

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
    pub fn new(
        style: BevelStyle,
        highlight: Color,
        shadow: Color,
        width: u32,
    ) -> Result<Self, BorderInputError> {
        if width == 0 {
            return Err(BorderInputError::NonPositiveWidth);
        }
        Ok(Self {
            style,
            highlight,
            shadow,
            width,
        })
    }

    pub(super) fn single_layer(style: BevelStyle, highlight: Color, shadow: Color) -> Self {
        Self {
            style,
            highlight,
            shadow,
            width: 1,
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
    fn snapshot_bounds_drawing(
        &self,
        _snapshot: Option<&super::BorderSnapshot>,
    ) -> Result<crate::figure::BoundsDrawing, GraphicsError> {
        const BEVEL_LAYER_RECORDING_BYTES: usize = 4096;
        let bytes = usize::try_from(self.width)
            .ok()
            .and_then(|layers| layers.checked_mul(BEVEL_LAYER_RECORDING_BYTES))
            .ok_or(GraphicsError::ImmutableDrawingUnavailable)?;
        Ok(crate::figure::BoundsDrawing::new(
            *self,
            bytes,
            |inputs, bounds, context| inputs.paint(bounds, context),
            |_, bounds| Ok(bounds),
        ))
    }

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
