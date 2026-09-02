//! Stack layout: every child occupies the container client area.

use super::{LayoutError, LayoutManager, LayoutOutput, LayoutSnapshot};
use crate::graph::BlockId;

#[derive(Debug, Clone, Copy, Default)]
pub struct StackLayout;

impl StackLayout {
    pub fn new() -> Self {
        Self
    }

    fn aggregate_size(
        &self,
        container: BlockId,
        w_hint: f64,
        h_hint: f64,
        snapshot: &LayoutSnapshot<'_>,
        minimum: bool,
    ) -> (f64, f64) {
        snapshot
            .children(container)
            .into_iter()
            .map(|(child, _)| {
                if minimum {
                    snapshot.minimum_size(child, w_hint, h_hint)
                } else {
                    snapshot.preferred_size(child, w_hint, h_hint)
                }
            })
            .fold((0.0_f64, 0.0_f64), |size, child| {
                (size.0.max(child.0), size.1.max(child.1))
            })
    }
}

impl LayoutManager for StackLayout {
    fn get_preferred_size(
        &self,
        container: BlockId,
        w_hint: f64,
        h_hint: f64,
        snapshot: &LayoutSnapshot<'_>,
    ) -> (f64, f64) {
        self.aggregate_size(container, w_hint, h_hint, snapshot, false)
    }

    fn get_minimum_size(
        &self,
        container: BlockId,
        w_hint: f64,
        h_hint: f64,
        snapshot: &LayoutSnapshot<'_>,
    ) -> (f64, f64) {
        self.aggregate_size(container, w_hint, h_hint, snapshot, true)
    }

    fn layout(
        &mut self,
        container: BlockId,
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
