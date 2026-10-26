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
    fn snapshot_bounds_drawing(
        &self,
        _snapshot: Option<&super::BorderSnapshot>,
    ) -> Result<crate::figure::BoundsDrawing, GraphicsError> {
        #[derive(Clone, Copy)]
        struct Layer {
            color: Color,
            highlight: bool,
        }
        const LAYER_RECORDING_BYTES: usize = 4096;
        let mut parts = Vec::new();
        for (colors, highlight) in [(&self.shadow, false), (&self.highlight, true)] {
            for (index, color) in colors.iter().enumerate() {
                let Some(color) = color else { continue };
                parts.push((
                    Insets::uniform(index as f64),
                    crate::figure::BoundsDrawing::new(
                        Layer {
                            color: *color,
                            highlight,
                        },
                        LAYER_RECORDING_BYTES,
                        |layer, bounds, context| {
                            let left = bounds.x + 0.5;
                            let top = bounds.y + 0.5;
                            let corner = if layer.highlight { 1.0 } else { 0.0 };
                            let right = bounds.x + bounds.width - 0.5 - corner;
                            let bottom = bounds.y + bounds.height - 0.5 - corner;
                            if right < left || bottom < top {
                                return Ok(());
                            }
                            context.set_stroke_style(StrokeStyle::default());
                            context.set_stroke_paint(layer.color);
                            if layer.highlight {
                                context
                                    .stroke_line(Point::new(left, top), Point::new(right, top))?;
                                context
                                    .stroke_line(Point::new(left, top), Point::new(left, bottom))?;
                            } else {
                                context.stroke_line(
                                    Point::new(right, bottom),
                                    Point::new(right, top),
                                )?;
                                context.stroke_line(
                                    Point::new(right, bottom),
                                    Point::new(left, bottom),
                                )?;
                            }
                            Ok(())
                        },
                        |_, bounds| Ok(bounds),
                    ),
                ));
            }
        }
        crate::figure::BoundsDrawing::compose(parts)
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_layers_match_static_endpoints_at_each_sampled_size() {
        let border = SchemeBorder::new(
            vec![Some(Color::WHITE), None, Some(Color::BLACK)],
            vec![Some(Color::BLACK), Some(Color::WHITE)],
        );
        let frozen = border.snapshot_bounds_drawing(None).unwrap();
        for bounds in [
            Rectangle::new(0.0, 0.0, 80.0, 40.0),
            Rectangle::new(0.0, 0.0, 20.0, 10.0),
            Rectangle::new(0.0, 0.0, 1.0, 1.0),
            Rectangle::new(0.0, 0.0, 0.0, 0.0),
        ] {
            let mut static_recorder = crate::render::CommandRecorder::new();
            border.paint(bounds, &mut static_recorder.paint()).unwrap();
            let mut frozen_recorder = crate::render::CommandRecorder::new();
            frozen
                .materialize(bounds, Color::BLACK, Color::WHITE)
                .unwrap()
                .paint(&mut frozen_recorder.paint())
                .unwrap();
            let lines = |commands: &[crate::render::RenderCommand]| {
                commands
                    .iter()
                    .filter(|command| {
                        matches!(command.kind, crate::render::RenderCommandKind::Line { .. })
                    })
                    .map(|command| command.kind.clone())
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                lines(static_recorder.commands()),
                lines(frozen_recorder.commands())
            );
        }
    }
}
