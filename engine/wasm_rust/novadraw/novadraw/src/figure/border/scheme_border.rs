use crate::geometry::{Insets, Point, Rectangle};
use crate::graphics::{Color, GraphicsError, PaintContext, StrokeStyle};

use super::Border;

/// Layered top/left and bottom/right colors, following Draw2D SchemeBorder.
///
/// A missing color reserves one logical pixel without painting it. Colors run
/// from outside to inside; the two sides may have different widths.
#[derive(Clone, Debug, PartialEq)]
pub struct SchemeBorder {
    highlight: Vec<Option<Color>>,
    shadow: Vec<Option<Color>>,
}

impl SchemeBorder {
    pub fn new(highlight: Vec<Option<Color>>, shadow: Vec<Option<Color>>) -> Self {
        Self { highlight, shadow }
    }
}

impl Border for SchemeBorder {
    fn get_insets(&self) -> Insets {
        let tl = self.highlight.len() as f64;
        let br = self.shadow.len() as f64;
        Insets::new(tl, tl, br, br)
    }

    fn is_opaque(&self) -> bool {
        // Unequal side widths leave corner pixels outside the thinner side.
        self.highlight.len() == self.shadow.len()
            && self
                .highlight
                .iter()
                .chain(&self.shadow)
                .all(|color| color.is_some_and(|color| color.is_opaque()))
    }

    fn paint(
        &self,
        bounds: Rectangle,
        context: &mut PaintContext<'_>,
    ) -> Result<(), GraphicsError> {
        context.set_stroke_style(StrokeStyle::default());
        for (layer, color) in self.shadow.iter().enumerate() {
            let offset = layer as f64 + 0.5;
            let left = bounds.x + offset;
            let top = bounds.y + offset;
            let right = bounds.x + bounds.width - offset;
            let bottom = bounds.y + bounds.height - offset;
            if right < left || bottom < top {
                break;
            }
            if let Some(color) = color {
                context.set_stroke_paint(*color);
                context.stroke_line(Point::new(right, bottom), Point::new(right, top))?;
                context.stroke_line(Point::new(right, bottom), Point::new(left, bottom))?;
            }
        }
        for (layer, color) in self.highlight.iter().enumerate() {
            let offset = layer as f64 + 0.5;
            let left = bounds.x + offset;
            let top = bounds.y + offset;
            // Draw2D gives top/left the shared starting corner but reserves the
            // last pixel for the shadow side.
            let right = bounds.x + bounds.width - offset - 1.0;
            let bottom = bounds.y + bounds.height - offset - 1.0;
            if right < left || bottom < top {
                break;
            }
            if let Some(color) = color {
                context.set_stroke_paint(*color);
                context.stroke_line(Point::new(left, top), Point::new(right, top))?;
                context.stroke_line(Point::new(left, top), Point::new(left, bottom))?;
            }
        }
        Ok(())
    }
}
