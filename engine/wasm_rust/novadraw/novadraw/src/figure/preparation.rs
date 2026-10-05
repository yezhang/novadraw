//! Immutable Figure preparation with a Runtime-owned measurement service.
use super::FigureMeasurement;
use crate::graphics::{GraphicsError, PaintContext};
use crate::text::{MeasureContext, TextError};
use crate::{Dimension, Rectangle};
use std::sync::Arc;

/// Optional preparation capability. It cannot mutate the tree or publish a frame.
pub trait FigurePreparation {
    /// Changes whenever inputs held outside the Figure's component state change.
    fn revision(&self) -> u64 {
        0
    }

    /// Builds a candidate in node-local coordinates. `bounds` is the current client area.
    /// Runtime publishes it only after the complete preparation batch succeeds.
    fn prepare(
        &self,
        context: &mut MeasureContext<'_>,
        bounds: Rectangle,
    ) -> Result<FigurePresentation, TextError>;
}

/// Immutable drawing payload paired with the measurement that produced it.
pub trait FigureDrawing: Send + Sync {
    fn paint(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError>;
}

/// Checked content metrics, visual envelope and drawing published as one value.
#[derive(Clone)]
pub struct FigurePresentation {
    measurement: FigureMeasurement,
    minimum: Dimension,
    visual_bounds: Rectangle,
    drawing: Arc<dyn FigureDrawing>,
}

impl FigurePresentation {
    pub fn new(
        measurement: FigureMeasurement,
        minimum: Dimension,
        visual_bounds: Rectangle,
        drawing: Arc<dyn FigureDrawing>,
    ) -> Result<Self, TextError> {
        if [
            measurement.width,
            measurement.height,
            minimum.width,
            minimum.height,
        ]
        .iter()
        .any(|v| !v.is_finite() || *v < 0.0)
            || measurement
                .baseline
                .is_some_and(|b| !b.is_finite() || b < 0.0 || b > measurement.height)
            || [
                visual_bounds.x,
                visual_bounds.y,
                visual_bounds.width,
                visual_bounds.height,
                visual_bounds.x + visual_bounds.width,
                visual_bounds.y + visual_bounds.height,
            ]
            .iter()
            .any(|v| !v.is_finite())
            || visual_bounds.width < 0.0
            || visual_bounds.height < 0.0
        {
            return Err(TextError::InvalidMetric);
        }
        Ok(Self {
            measurement,
            minimum,
            visual_bounds,
            drawing,
        })
    }
    pub fn measurement(&self) -> FigureMeasurement {
        self.measurement
    }
    pub fn minimum_size(&self) -> Dimension {
        self.minimum
    }
    pub fn visual_bounds(&self) -> Rectangle {
        self.visual_bounds
    }
    pub(crate) fn paint(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        self.drawing.paint(context)
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct PreparationKey {
    pub component: u64,
    pub provider: u64,
    pub text: u64,
    pub font: String,
    pub bounds: Rectangle,
}

pub(crate) struct PreparedFigure {
    pub key: PreparationKey,
    pub presentation: FigurePresentation,
}
