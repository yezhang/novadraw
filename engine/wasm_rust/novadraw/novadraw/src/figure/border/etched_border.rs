use std::sync::Arc;

use crate::Color;
use crate::geometry::{Dimension, Insets, Rectangle};
use crate::render::NdCanvas;

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
    fn get_insets(&self) -> Insets {
        self.compound.get_insets()
    }

    fn preferred_size(&self) -> Dimension {
        self.compound.preferred_size()
    }

    fn is_opaque(&self) -> bool {
        self.opaque
    }

    fn paint(&self, figure_bounds: Rectangle, gc: &mut NdCanvas) {
        self.compound.paint(figure_bounds, gc);
    }

    fn paint_with_insets(&self, figure_bounds: Rectangle, incoming: Insets, gc: &mut NdCanvas) {
        self.compound.paint_with_insets(figure_bounds, incoming, gc);
    }
}
