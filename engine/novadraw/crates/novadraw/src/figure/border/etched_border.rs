use std::sync::Arc;

use crate::Color;
use crate::geometry::{Dimension, Insets, Rectangle};
use crate::graphics::{GraphicsError, PaintContext};

use super::{BevelBorder, BevelStyle, Border, CompoundBorder};

/// Two-layer etched border with opposing light and shadow bands.
#[derive(Clone)]
pub struct EtchedBorder {
    compound: CompoundBorder,
    opaque: bool,
}

impl EtchedBorder {
    pub fn new(highlight: Color, shadow: Color) -> Self {
        let outer = BevelBorder::new(BevelStyle::Lowered, highlight, shadow, 1);
        let inner = BevelBorder::new(BevelStyle::Raised, highlight, shadow, 1);
        Self {
            compound: CompoundBorder::new(outer, inner),
            opaque: highlight.is_opaque() && shadow.is_opaque(),
        }
    }

    pub fn from_shared(highlight: Color, shadow: Color) -> Arc<dyn Border> {
        Arc::new(Self::new(highlight, shadow))
    }
}

impl Border for EtchedBorder {
    fn snapshot_bounds_drawing(
        &self,
        snapshot: Option<&super::BorderSnapshot>,
    ) -> Result<crate::figure::BoundsDrawing, GraphicsError> {
        self.compound.snapshot_bounds_drawing(snapshot)
    }

    fn get_insets(&self) -> Insets {
        self.compound.get_insets()
    }

    fn preferred_size(&self) -> Dimension {
        self.compound.preferred_size()
    }

    fn is_opaque(&self) -> bool {
        self.opaque
    }

    fn paint(
        &self,
        figure_bounds: Rectangle,
        context: &mut PaintContext<'_>,
    ) -> Result<(), GraphicsError> {
        self.compound.paint(figure_bounds, context)
    }

    fn paint_with_insets(
        &self,
        figure_bounds: Rectangle,
        incoming: Insets,
        context: &mut PaintContext<'_>,
    ) -> Result<(), GraphicsError> {
        self.compound
            .paint_with_insets(figure_bounds, incoming, context)
    }
}
