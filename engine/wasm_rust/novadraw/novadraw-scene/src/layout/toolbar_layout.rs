//! Single-row or single-column toolbar layout.

use super::{LayoutContext, LayoutManager};
use crate::graph::BlockId;
use novadraw_geometry::Rectangle;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ToolbarOrientation {
    Horizontal,
    #[default]
    Vertical,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MinorAlignment {
    #[default]
    Start,
    Center,
    End,
}

#[derive(Clone, Debug)]
pub struct ToolbarLayout {
    orientation: ToolbarOrientation,
    spacing: f64,
    stretch_minor_axis: bool,
    minor_alignment: MinorAlignment,
}

impl ToolbarLayout {
    pub fn new() -> Self {
        Self {
            orientation: ToolbarOrientation::Vertical,
            spacing: 0.0,
            stretch_minor_axis: true,
            minor_alignment: MinorAlignment::Start,
        }
    }

    pub fn horizontal() -> Self {
        Self {
            orientation: ToolbarOrientation::Horizontal,
            stretch_minor_axis: false,
            ..Self::new()
        }
    }

    pub fn with_orientation(mut self, orientation: ToolbarOrientation) -> Self {
        self.orientation = orientation;
        self
    }

    pub fn with_spacing(mut self, spacing: f64) -> Self {
        self.spacing = spacing.max(0.0);
        self
    }

    pub fn with_stretch_minor_axis(mut self, stretch: bool) -> Self {
        self.stretch_minor_axis = stretch;
        self
    }

    pub fn with_minor_alignment(mut self, alignment: MinorAlignment) -> Self {
        self.minor_alignment = alignment;
        self
    }

    fn oriented_size(&self, size: (f64, f64)) -> (f64, f64) {
        match self.orientation {
            ToolbarOrientation::Horizontal => size,
            ToolbarOrientation::Vertical => (size.1, size.0),
        }
    }

    fn physical_size(&self, main: f64, minor: f64) -> (f64, f64) {
        match self.orientation {
            ToolbarOrientation::Horizontal => (main, minor),
            ToolbarOrientation::Vertical => (minor, main),
        }
    }

    fn aggregate_size(
        &self,
        container: BlockId,
        w_hint: f64,
        h_hint: f64,
        ctx: &dyn LayoutContext,
        minimum: bool,
    ) -> (f64, f64) {
        let children = ctx.get_children(container);
        let mut main = 0.0_f64;
        let mut minor = 0.0_f64;
        for (child, _) in &children {
            let size = if minimum {
                ctx.get_minimum_size(*child, w_hint, h_hint)
            } else {
                ctx.get_preferred_size(*child, w_hint, h_hint)
            };
            let size = self.oriented_size(size);
            main += size.0;
            minor = minor.max(size.1);
        }
        main += self.spacing * children.len().saturating_sub(1) as f64;
        self.physical_size(main, minor)
    }
}

impl Default for ToolbarLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutManager for ToolbarLayout {
    fn get_preferred_size(
        &self,
        container: BlockId,
        w_hint: f64,
        h_hint: f64,
        ctx: &dyn LayoutContext,
    ) -> (f64, f64) {
        self.aggregate_size(container, w_hint, h_hint, ctx, false)
    }

    fn get_minimum_size(
        &self,
        container: BlockId,
        w_hint: f64,
        h_hint: f64,
        ctx: &dyn LayoutContext,
    ) -> (f64, f64) {
        self.aggregate_size(container, w_hint, h_hint, ctx, true)
    }

    fn layout(&self, container: BlockId, ctx: &mut dyn LayoutContext) {
        let children = ctx.get_children(container);
        if children.is_empty() {
            return;
        }

        let client = ctx.get_container_bounds(container);
        let available = self.oriented_size((client.width, client.height));
        let origin = match self.orientation {
            ToolbarOrientation::Horizontal => (client.x, client.y),
            ToolbarOrientation::Vertical => (client.y, client.x),
        };

        let mut preferred = Vec::with_capacity(children.len());
        let mut minimum = Vec::with_capacity(children.len());
        let mut maximum = Vec::with_capacity(children.len());
        for (child, _) in &children {
            preferred.push(self.oriented_size(ctx.get_preferred_size(*child, -1.0, -1.0)));
            minimum.push(self.oriented_size(ctx.get_minimum_size(*child, -1.0, -1.0)));
            maximum.push(self.oriented_size(ctx.get_maximum_size(*child)));
        }

        let spacing_total = self.spacing * children.len().saturating_sub(1) as f64;
        let preferred_main = preferred.iter().map(|size| size.0).sum::<f64>() + spacing_total;
        let minimum_main = minimum.iter().map(|size| size.0).sum::<f64>() + spacing_total;
        let shrink = (preferred_main - available.0.max(minimum_main)).max(0.0);
        let shrink_capacity = preferred
            .iter()
            .zip(&minimum)
            .map(|(preferred, minimum)| (preferred.0 - minimum.0).max(0.0))
            .sum::<f64>();

        let mut main_cursor = origin.0;
        for (index, (child, _)) in children.iter().enumerate() {
            let capacity = (preferred[index].0 - minimum[index].0).max(0.0);
            let child_shrink = if shrink_capacity > 0.0 {
                shrink * capacity / shrink_capacity
            } else {
                0.0
            };
            let main = (preferred[index].0 - child_shrink).max(minimum[index].0);

            let natural_minor = preferred[index]
                .1
                .min(maximum[index].1)
                .max(minimum[index].1);
            let minor = if self.stretch_minor_axis {
                available.1.min(maximum[index].1).max(minimum[index].1)
            } else {
                natural_minor.min(available.1)
            };
            let minor_offset = match self.minor_alignment {
                MinorAlignment::Start => 0.0,
                MinorAlignment::Center => (available.1 - minor) / 2.0,
                MinorAlignment::End => available.1 - minor,
            }
            .max(0.0);

            let (width, height) = self.physical_size(main, minor);
            let (x, y) = match self.orientation {
                ToolbarOrientation::Horizontal => (main_cursor, origin.1 + minor_offset),
                ToolbarOrientation::Vertical => (origin.1 + minor_offset, main_cursor),
            };
            ctx.set_child_bounds(*child, Rectangle::new(x, y, width, height));
            main_cursor += main + self.spacing;
        }
    }
}
