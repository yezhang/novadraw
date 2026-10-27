use std::{any::TypeId, sync::Arc};

use crate::{
    Color, Dimension, FigureId, FigureMeasurement, Point, Rectangle,
    figure::{FigureDrawing, FigurePresentation},
    graphics::{GraphicsError, PaintContext},
};

use super::{AnimationBudgetKind, AnimationError, AnimationValue};

/// One immutable line in owner-local coordinates.
#[derive(Clone, Copy)]
pub struct GeometryLine {
    /// First endpoint.
    pub start: Point,
    /// Second endpoint.
    pub end: Point,
    /// Unscaled local stroke width.
    pub width: f64,
    /// Stroke color.
    pub color: Color,
}

/// Checked drawing output. The declaration is frozen before provider admission.
///
/// No arbitrary paint callback or mutable application data enters a derived drawing.
pub struct GeometryDrawingBuilder {
    lines: Vec<GeometryLine>,
    limit: usize,
    envelope: Option<Rectangle>,
    figure: FigureId,
    value_type: TypeId,
    rejected: Option<AnimationError>,
}

impl GeometryDrawingBuilder {
    pub(crate) fn new(
        figure: FigureId,
        value_type: TypeId,
        limit: usize,
    ) -> Result<Self, AnimationError> {
        let mut lines = Vec::new();
        lines
            .try_reserve_exact(limit)
            .map_err(|_| AnimationError::BudgetExceeded(AnimationBudgetKind::SnapshotBytes))?;
        Ok(Self {
            lines,
            limit,
            envelope: None,
            figure,
            value_type,
            rejected: None,
        })
    }

    /// Appends a finite line, checking the frozen item bound before allocation.
    pub fn line(&mut self, line: GeometryLine) -> Result<(), AnimationError> {
        if let Some(error) = &self.rejected {
            return Err(error.clone());
        }
        if self.lines.len() == self.limit {
            return self.reject(AnimationError::ProviderOutputExceeded {
                kind: AnimationBudgetKind::GeometryItems,
                declared: self.limit,
                actual: self.lines.len().saturating_add(1),
                figure: self.figure,
                value_type: self.value_type,
            });
        }
        if ![
            line.start.x(),
            line.start.y(),
            line.end.x(),
            line.end.y(),
            line.width,
        ]
        .into_iter()
        .all(f64::is_finite)
            || line.width < 0.0
        {
            return self.reject(AnimationError::InvalidValue);
        }
        let radius = line.width / 2.0;
        let envelope = Rectangle::new(
            line.start.x().min(line.end.x()) - radius,
            line.start.y().min(line.end.y()) - radius,
            (line.start.x() - line.end.x()).abs() + line.width,
            (line.start.y() - line.end.y()).abs() + line.width,
        );
        if !envelope.is_valid() {
            return self.reject(AnimationError::InvalidValue);
        }
        self.envelope = Some(self.envelope.map_or(envelope, |old| old.union(envelope)));
        self.lines.push(line);
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<FigurePresentation, AnimationError> {
        if let Some(error) = self.rejected {
            return Err(error);
        }
        FigurePresentation::new(
            FigureMeasurement::new(0.0, 0.0, None),
            Dimension::new(0.0, 0.0),
            self.envelope.unwrap_or(Rectangle::new(0.0, 0.0, 0.0, 0.0)),
            Arc::new(LineDrawing {
                lines: self.lines.into_boxed_slice(),
            }),
        )
        .map_err(|_| AnimationError::InvalidValue)
    }

    fn reject(&mut self, error: AnimationError) -> Result<(), AnimationError> {
        self.rejected = Some(error.clone());
        Err(error)
    }
}

struct LineDrawing {
    lines: Box<[GeometryLine]>,
}

impl FigureDrawing for LineDrawing {
    fn paint(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        context.set_stroke_style(crate::graphics::StrokeStyle::default());
        for line in &self.lines {
            context.set_stroke_paint(line.color);
            context.set_stroke_width(line.width)?;
            context.stroke_line(line.start, line.end)?;
        }
        Ok(())
    }
}
