use std::sync::Arc;

use novadraw_geometry::Rectangle;
use novadraw_render::{FontDescriptor, NdCanvas, TextError, TextLayoutEngine};

use super::{Border, BorderSnapshot, add_insets};

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

    fn resolved_insets(
        border: Option<&dyn Border>,
        snapshot: Option<&BorderSnapshot>,
    ) -> (f64, f64, f64, f64) {
        snapshot
            .map(BorderSnapshot::insets)
            .or_else(|| border.map(Border::get_insets))
            .unwrap_or_default()
    }

    fn resolved_preferred_size(
        border: Option<&dyn Border>,
        snapshot: Option<&BorderSnapshot>,
    ) -> (f64, f64) {
        snapshot
            .map(BorderSnapshot::preferred_size)
            .or_else(|| border.map(Border::preferred_size))
            .unwrap_or_default()
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

    fn paint_snapshot_with_insets(
        &self,
        figure_bounds: Rectangle,
        incoming: (f64, f64, f64, f64),
        snapshot: &BorderSnapshot,
        gc: &mut NdCanvas,
    ) {
        let Some((outer_snapshot, inner_snapshot)) = snapshot.compound_parts() else {
            self.paint_with_insets(figure_bounds, incoming, gc);
            return;
        };
        if let Some(outer) = self.outer.as_deref() {
            gc.push_state();
            if let Some(snapshot) = outer_snapshot {
                outer.paint_snapshot_with_insets(figure_bounds, incoming, snapshot, gc);
            } else {
                outer.paint_with_insets(figure_bounds, incoming, gc);
            }
            gc.pop_state();
        }
        if let Some(inner) = self.inner.as_deref() {
            let outer_insets = Self::resolved_insets(self.outer.as_deref(), outer_snapshot);
            let incoming = add_insets(incoming, outer_insets);
            if let Some(snapshot) = inner_snapshot {
                inner.paint_snapshot_with_insets(figure_bounds, incoming, snapshot, gc);
            } else {
                inner.paint_with_insets(figure_bounds, incoming, gc);
            }
        }
    }

    fn resolve_owner_snapshot(
        &self,
        previous: Option<&BorderSnapshot>,
        font: &FontDescriptor,
        text: &mut dyn TextLayoutEngine,
    ) -> Result<Option<BorderSnapshot>, TextError> {
        let (previous_outer, previous_inner) = previous
            .and_then(BorderSnapshot::compound_parts)
            .unwrap_or((None, None));
        let outer = match self.outer.as_deref() {
            Some(border) => border.resolve_owner_snapshot(previous_outer, font, text)?,
            None => None,
        };
        let inner = match self.inner.as_deref() {
            Some(border) => border.resolve_owner_snapshot(previous_inner, font, text)?,
            None => None,
        };
        if outer.is_none() && inner.is_none() {
            return Ok(None);
        }

        let outer_insets = Self::resolved_insets(self.outer.as_deref(), outer.as_ref());
        let inner_insets = Self::resolved_insets(self.inner.as_deref(), inner.as_ref());
        let outer_preferred = Self::resolved_preferred_size(self.outer.as_deref(), outer.as_ref());
        let inner_preferred = Self::resolved_preferred_size(self.inner.as_deref(), inner.as_ref());
        let preferred = (
            outer_preferred
                .0
                .max(inner_preferred.0 + outer_insets.1 + outer_insets.3),
            outer_preferred
                .1
                .max(inner_preferred.1 + outer_insets.0 + outer_insets.2),
        );
        Ok(Some(BorderSnapshot::compound(
            outer,
            inner,
            add_insets(outer_insets, inner_insets),
            preferred,
        )))
    }

    fn has_owner_snapshot(&self) -> bool {
        self.outer
            .as_deref()
            .is_some_and(Border::has_owner_snapshot)
            || self
                .inner
                .as_deref()
                .is_some_and(Border::has_owner_snapshot)
    }
}
