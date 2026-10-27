//! Stack layout: every child occupies the container client area.

use super::{LayoutError, LayoutManager, LayoutOutput, LayoutSceneQuery};
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
        snapshot: &LayoutSceneQuery<'_>,
        minimum: bool,
    ) -> Result<Dimension, LayoutError> {
        let mut size = Dimension::ZERO;
        for (child, _) in snapshot.children(container)? {
            let child = if minimum {
                snapshot.minimum_size(child, constraints)?
            } else {
                snapshot.preferred_measurement(child, constraints)?.size()
            };
            size = Dimension::new(size.width.max(child.width), size.height.max(child.height));
        }
        Ok(size)
    }
}

impl LayoutManager for StackLayout {
    fn preferred_measurement(
        &self,
        container: FigureId,
        constraints: MeasureConstraints,
        snapshot: &LayoutSceneQuery<'_>,
    ) -> Result<FigureMeasurement, LayoutError> {
        let size = self.aggregate_size(container, constraints, snapshot, false)?;
        Ok(FigureMeasurement::new(size.width, size.height, None))
    }

    fn minimum_size(
        &self,
        container: FigureId,
        constraints: MeasureConstraints,
        snapshot: &LayoutSceneQuery<'_>,
    ) -> Result<Dimension, LayoutError> {
        self.aggregate_size(container, constraints, snapshot, true)
    }

    fn layout(
        &mut self,
        container: FigureId,
        snapshot: &LayoutSceneQuery<'_>,
        out: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        let client_area = snapshot.container_bounds(container)?;
        for (child, _) in snapshot.children(container)? {
            out.set_child_bounds(child, client_area);
        }
        Ok(())
    }
}
