//! Stack layout: every child occupies the container client area.

use super::{LayoutContext, LayoutManager};
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
        ctx: &dyn LayoutContext,
        minimum: bool,
    ) -> (f64, f64) {
        ctx.get_children(container)
            .into_iter()
            .map(|(child, _)| {
                if minimum {
                    ctx.get_minimum_size(child, w_hint, h_hint)
                } else {
                    ctx.get_preferred_size(child, w_hint, h_hint)
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
        let client_area = ctx.get_container_bounds(container);
        for (child, _) in ctx.get_children(container) {
            ctx.set_child_bounds(child, client_area);
        }
    }
}
