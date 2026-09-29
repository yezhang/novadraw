//! Stack layout: every child occupies the container client area.

use super::{LayoutError, LayoutManager, LayoutOutput, LayoutSnapshot};
use crate::geometry::Dimension;
use crate::{FigureMeasurement, MeasureConstraints, graph::FigureId};

#[derive(Debug, Clone, Copy, Default)]
pub struct StackLayout;

impl StackLayout {
    pub fn new() -> Self {
        Self
    }

    fn aggregate_size(
        &self,
        container: FigureId,
        constraints: MeasureConstraints,
        snapshot: &LayoutSnapshot<'_>,
        minimum: bool,
    ) -> Dimension {
        snapshot
            .children(container)
            .into_iter()
            .map(|(child, _)| {
                if minimum {
                    snapshot.minimum_size(child, constraints)
                } else {
                    snapshot.preferred_measurement(child, constraints).size()
                }
            })
            .fold(Dimension::ZERO, |size, child| {
                Dimension::new(size.width.max(child.width), size.height.max(child.height))
            })
    }
}

impl LayoutManager for StackLayout {
    fn preferred_measurement(
        &self,
        container: FigureId,
        constraints: MeasureConstraints,
        snapshot: &LayoutSnapshot<'_>,
    ) -> FigureMeasurement {
        let size = self.aggregate_size(container, constraints, snapshot, false);
        FigureMeasurement::new(size.width, size.height, None)
    }

    fn minimum_size(
        &self,
        container: FigureId,
        constraints: MeasureConstraints,
        snapshot: &LayoutSnapshot<'_>,
    ) -> Dimension {
        self.aggregate_size(container, constraints, snapshot, true)
    }

    fn layout(
        &mut self,
        container: FigureId,
        snapshot: &LayoutSnapshot<'_>,
        out: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        let client_area = snapshot.container_bounds(container);
        for (child, _) in snapshot.children(container) {
            out.set_child_bounds(child, client_area);
        }
        Ok(())
    }
}
