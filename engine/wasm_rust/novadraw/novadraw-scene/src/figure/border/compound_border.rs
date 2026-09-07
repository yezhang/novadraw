use std::sync::Arc;

use novadraw_geometry::Rectangle;
use novadraw_render::NdCanvas;

use super::{Border, add_insets};

/// Draw2D-compatible outer/inner Border composition.
#[derive(Clone, Default)]
pub struct CompoundBorder {
    outer: Option<Arc<dyn Border>>,
    inner: Option<Arc<dyn Border>>,
}

impl CompoundBorder {
    pub fn new(outer: impl Border + 'static, inner: impl Border + 'static) -> Self {
        Self {
            outer: Some(Arc::new(outer)),
            inner: Some(Arc::new(inner)),
        }
    }

    pub fn from_optional(outer: Option<Arc<dyn Border>>, inner: Option<Arc<dyn Border>>) -> Self {
        Self { outer, inner }
    }

    pub fn outer(&self) -> Option<&dyn Border> {
        self.outer.as_deref()
    }

    pub fn inner(&self) -> Option<&dyn Border> {
        self.inner.as_deref()
    }
}

impl Border for CompoundBorder {
    fn get_insets(&self) -> (f64, f64, f64, f64) {
        add_insets(
            self.outer
                .as_deref()
                .map(Border::get_insets)
                .unwrap_or_default(),
            self.inner
                .as_deref()
                .map(Border::get_insets)
                .unwrap_or_default(),
        )
    }

    fn preferred_size(&self) -> (f64, f64) {
        let outer_insets = self
            .outer
            .as_deref()
            .map(Border::get_insets)
            .unwrap_or_default();
        let outer = self
            .outer
            .as_deref()
            .map(Border::preferred_size)
            .unwrap_or_default();
        let inner = self
            .inner
            .as_deref()
            .map(Border::preferred_size)
            .unwrap_or_default();
        (
            outer.0.max(inner.0 + outer_insets.1 + outer_insets.3),
            outer.1.max(inner.1 + outer_insets.0 + outer_insets.2),
        )
    }

    fn is_opaque(&self) -> bool {
        self.outer.as_deref().is_some_and(Border::is_opaque)
            && self.inner.as_deref().is_some_and(Border::is_opaque)
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
        if let Some(outer) = self.outer.as_deref() {
            gc.push_state();
            outer.paint_with_insets(figure_bounds, incoming, gc);
            gc.pop_state();
        }
        if let Some(inner) = self.inner.as_deref() {
            let outer_insets = self
                .outer
                .as_deref()
                .map(Border::get_insets)
                .unwrap_or_default();
            inner.paint_with_insets(figure_bounds, add_insets(incoming, outer_insets), gc);
        }
    }
}
