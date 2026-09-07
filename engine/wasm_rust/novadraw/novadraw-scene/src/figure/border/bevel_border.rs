use glam::DVec2;
use novadraw_core::Color;
use novadraw_geometry::Rectangle;
use novadraw_render::{
    NdCanvas,
    command::{LineCap, LineJoin},
};

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
    fn get_insets(&self) -> (f64, f64, f64, f64) {
        let width = f64::from(self.width);
        (width, width, width, width)
    }

    fn is_opaque(&self) -> bool {
        self.highlight.is_opaque() && self.shadow.is_opaque()
    }

    fn paint(&self, figure_bounds: Rectangle, gc: &mut NdCanvas) {
        self.paint_with_insets(figure_bounds, (0.0, 0.0, 0.0, 0.0), gc);
    }

    fn paint_with_insets(
        &self,
        figure_bounds: Rectangle,
        incoming: (f64, f64, f64, f64),
        gc: &mut NdCanvas,
    ) {
        let bounds = inset_rectangle(figure_bounds, incoming);
        let (top_left, bottom_right) = self.colors();
        for layer in 0..self.width {
            let offset = f64::from(layer) + 0.5;
            let left = bounds.x + offset;
            let top = bounds.y + offset;
            let right = bounds.x + bounds.width - offset;
            let bottom = bounds.y + bounds.height - offset;
            if right < left || bottom < top {
                break;
            }
            gc.line(
                DVec2::new(left, bottom),
                DVec2::new(left, top),
                top_left,
                1.0,
                LineCap::Butt,
                LineJoin::Miter,
            );
            gc.line(
                DVec2::new(left, top),
                DVec2::new(right, top),
                top_left,
                1.0,
                LineCap::Butt,
                LineJoin::Miter,
            );
            gc.line(
                DVec2::new(right, top),
                DVec2::new(right, bottom),
                bottom_right,
                1.0,
                LineCap::Butt,
                LineJoin::Miter,
            );
            gc.line(
                DVec2::new(right, bottom),
                DVec2::new(left, bottom),
                bottom_right,
                1.0,
                LineCap::Butt,
                LineJoin::Miter,
            );
        }
    }
}
