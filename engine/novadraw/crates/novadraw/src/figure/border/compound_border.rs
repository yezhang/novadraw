use std::sync::Arc;

use crate::geometry::{Dimension, Insets, Rectangle};
use crate::graphics::{GraphicsError, PaintContext};
use crate::render::{FontDescriptor, TextError, TextLayoutEngine};

use super::{Border, BorderInputError, BorderMetrics, BorderSnapshot, add_insets};

/// Draw2D-compatible outer/inner Border composition.
#[derive(Clone, Default)]
pub struct CompoundBorder {
    outer: Option<Arc<dyn Border>>,
    inner: Option<Arc<dyn Border>>,
}

impl CompoundBorder {
    pub fn new(
        outer: impl Border + 'static,
        inner: impl Border + 'static,
    ) -> Result<Self, BorderInputError> {
        Self::from_optional(Some(Arc::new(outer)), Some(Arc::new(inner)))
    }

    pub fn from_optional(
        outer: Option<Arc<dyn Border>>,
        inner: Option<Arc<dyn Border>>,
    ) -> Result<Self, BorderInputError> {
        let candidate = Self { outer, inner };
        candidate.prepare_metrics()?;
        Ok(candidate)
    }

    /// A fixed, two-unit configuration with no raw numeric input.
    pub(super) fn etched(highlight: crate::Color, shadow: crate::Color) -> Self {
        Self {
            outer: Some(Arc::new(super::BevelBorder::single_layer(
                super::BevelStyle::Lowered,
                highlight,
                shadow,
            ))),
            inner: Some(Arc::new(super::BevelBorder::single_layer(
                super::BevelStyle::Raised,
                highlight,
                shadow,
            ))),
        }
    }

    pub fn outer(&self) -> Option<&dyn Border> {
        self.outer.as_deref()
    }

    pub fn inner(&self) -> Option<&dyn Border> {
        self.inner.as_deref()
    }

    fn resolved_insets(border: Option<&dyn Border>, snapshot: Option<&BorderSnapshot>) -> Insets {
        snapshot
            .map(BorderSnapshot::insets)
            .or_else(|| border.map(Border::get_insets))
            .unwrap_or_default()
    }
}

impl Border for CompoundBorder {
    fn prepare_metrics(&self) -> Result<BorderMetrics, BorderInputError> {
        let capture = |border: Option<&dyn Border>, role| {
            border
                .map(Border::prepare_metrics)
                .transpose()
                .map(|metrics| metrics.unwrap_or_default())
                .map_err(|source| BorderInputError::Child {
                    role,
                    source: Box::new(source),
                })
        };
        BorderMetrics::compose(
            capture(self.outer(), "outer")?,
            capture(self.inner(), "inner")?,
        )
    }

    fn snapshot_bounds_drawing(
        &self,
        snapshot: Option<&BorderSnapshot>,
    ) -> Result<crate::figure::BoundsDrawing, GraphicsError> {
        self.snapshot_bounds_drawing_with_colors(snapshot, crate::Color::BLACK, crate::Color::WHITE)
    }

    fn snapshot_bounds_drawing_with_colors(
        &self,
        snapshot: Option<&BorderSnapshot>,
        foreground: crate::Color,
        background: crate::Color,
    ) -> Result<crate::figure::BoundsDrawing, GraphicsError> {
        let (outer_snapshot, inner_snapshot) = snapshot
            .and_then(BorderSnapshot::compound_parts)
            .unwrap_or((None, None));
        let mut parts = Vec::with_capacity(2);
        if let Some(outer) = self.outer.as_deref() {
            parts.push((
                Insets::ZERO,
                outer.snapshot_bounds_drawing_with_colors(
                    outer_snapshot,
                    foreground,
                    background,
                )?,
            ));
        }
        if let Some(inner) = self.inner.as_deref() {
            let incoming = Self::resolved_insets(self.outer.as_deref(), outer_snapshot);
            parts.push((
                incoming,
                inner.snapshot_bounds_drawing_with_colors(
                    inner_snapshot,
                    foreground,
                    background,
                )?,
            ));
        }
        crate::figure::BoundsDrawing::compose(parts)
    }

    fn get_insets(&self) -> Insets {
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

    fn preferred_size(&self) -> Dimension {
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
        Dimension::new(
            outer.width.max(inner.width + outer_insets.width()),
            outer.height.max(inner.height + outer_insets.height()),
        )
    }

    fn is_opaque(&self) -> bool {
        self.outer.as_deref().is_some_and(Border::is_opaque)
            && self.inner.as_deref().is_some_and(Border::is_opaque)
    }

    fn paint(
        &self,
        figure_bounds: Rectangle,
        context: &mut PaintContext<'_>,
    ) -> Result<(), GraphicsError> {
        self.paint_with_insets(figure_bounds, Insets::ZERO, context)
    }

    fn paint_with_insets(
        &self,
        figure_bounds: Rectangle,
        incoming: Insets,
        context: &mut PaintContext<'_>,
    ) -> Result<(), GraphicsError> {
        if let Some(outer) = self.outer.as_deref() {
            context.push_state();
            let paint = outer.paint_with_insets(figure_bounds, incoming, context);
            let pop = context.pop_state();
            paint?;
            pop?;
        }
        if let Some(inner) = self.inner.as_deref() {
            let outer_insets = self
                .outer
                .as_deref()
                .map(Border::get_insets)
                .unwrap_or_default();
            inner.paint_with_insets(figure_bounds, add_insets(incoming, outer_insets), context)?;
        }
        Ok(())
    }

    fn paint_snapshot_with_insets(
        &self,
        figure_bounds: Rectangle,
        incoming: Insets,
        snapshot: &BorderSnapshot,
        context: &mut PaintContext<'_>,
    ) -> Result<(), GraphicsError> {
        let Some((outer_snapshot, inner_snapshot)) = snapshot.compound_parts() else {
            return self.paint_with_insets(figure_bounds, incoming, context);
        };
        if let Some(outer) = self.outer.as_deref() {
            context.push_state();
            let paint = if let Some(snapshot) = outer_snapshot {
                outer.paint_snapshot_with_insets(figure_bounds, incoming, snapshot, context)
            } else {
                outer.paint_with_insets(figure_bounds, incoming, context)
            };
            let pop = context.pop_state();
            paint?;
            pop?;
        }
        if let Some(inner) = self.inner.as_deref() {
            let outer_insets = Self::resolved_insets(self.outer.as_deref(), outer_snapshot);
            let incoming = add_insets(incoming, outer_insets);
            if let Some(snapshot) = inner_snapshot {
                inner.paint_snapshot_with_insets(figure_bounds, incoming, snapshot, context)?;
            } else {
                inner.paint_with_insets(figure_bounds, incoming, context)?;
            }
        }
        Ok(())
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
            Some(border) if border.has_owner_snapshot() => border
                .resolve_owner_snapshot(previous_outer, font, text)
                .map_err(|source| TextError::BorderChild {
                    role: "outer",
                    source: Box::new(source),
                })?,
            _ => None,
        };
        let inner = match self.inner.as_deref() {
            Some(border) if border.has_owner_snapshot() => border
                .resolve_owner_snapshot(previous_inner, font, text)
                .map_err(|source| TextError::BorderChild {
                    role: "inner",
                    source: Box::new(source),
                })?,
            _ => None,
        };
        let capture = |border: Option<&dyn Border>, snapshot: Option<&BorderSnapshot>, role| {
            let result = match snapshot {
                Some(snapshot) => BorderMetrics::new(snapshot.insets(), snapshot.preferred_size()),
                None => border
                    .map(Border::prepare_metrics)
                    .transpose()
                    .map(|metrics| metrics.unwrap_or_default()),
            };
            result.map_err(|source| {
                TextError::BorderInput(Box::new(BorderInputError::Child {
                    role,
                    source: Box::new(source),
                }))
            })
        };
        let metrics = BorderMetrics::compose(
            capture(self.outer(), outer.as_ref(), "outer")?,
            capture(self.inner(), inner.as_ref(), "inner")?,
        )
        .map_err(|source| TextError::BorderInput(Box::new(source)))?;
        if outer.is_none() && inner.is_none() {
            return Ok(None);
        }
        Ok(Some(BorderSnapshot::compound(
            outer,
            inner,
            metrics.insets(),
            metrics.preferred_size(),
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
