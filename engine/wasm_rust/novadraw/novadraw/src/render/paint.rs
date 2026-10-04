//! Backend-neutral immutable paints in Canvas logical coordinates.

use std::sync::Arc;

use super::stroke::{GraphicsInputError, finite};
use crate::{Color, geometry::Point};

/// A validated transition position and color.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GradientStop {
    offset: f64,
    color: Color,
}

impl GradientStop {
    pub fn try_new(offset: f64, color: Color) -> Result<Self, GraphicsInputError> {
        finite(offset, "gradient stop offset")?;
        if !(0.0..=1.0).contains(&offset) {
            return Err(GraphicsInputError::GradientStopOutOfRange);
        }
        Ok(Self { offset, color })
    }

    pub fn offset(&self) -> f64 {
        self.offset
    }
    pub fn color(&self) -> Color {
        self.color
    }
}

/// A Pad-extended linear gradient, interpolated in premultiplied sRGB.
#[derive(Clone, Debug, PartialEq)]
pub struct LinearGradient {
    start: Point,
    end: Point,
    stops: Arc<[GradientStop]>,
}

impl LinearGradient {
    pub fn try_new(
        start: Point,
        end: Point,
        stops: &[GradientStop],
    ) -> Result<Self, GraphicsInputError> {
        for value in [start.x(), start.y(), end.x(), end.y()] {
            finite(value, "gradient endpoint")?;
        }
        if start == end {
            return Err(GraphicsInputError::DegenerateGradient);
        }
        if stops.len() < 2 {
            return Err(GraphicsInputError::InsufficientGradientStops);
        }
        if stops[0].offset != 0.0 || stops[stops.len() - 1].offset != 1.0 {
            return Err(GraphicsInputError::GradientEndpointStops);
        }
        for (index, pair) in stops.windows(2).enumerate() {
            if pair[0].offset > pair[1].offset {
                return Err(GraphicsInputError::GradientStopOrder { index: index + 1 });
            }
        }
        Ok(Self {
            start,
            end,
            stops: Arc::from(stops),
        })
    }

    pub fn start(&self) -> Point {
        self.start
    }
    pub fn end(&self) -> Point {
        self.end
    }
    pub fn stops(&self) -> &[GradientStop] {
        &self.stops
    }
}

/// Shared paint for vector shapes and glyphs.
#[derive(Clone, Debug, PartialEq)]
pub enum Paint {
    Solid(Color),
    LinearGradient(LinearGradient),
}

impl From<Color> for Paint {
    fn from(color: Color) -> Self {
        Self::Solid(color)
    }
}

impl From<LinearGradient> for Paint {
    fn from(gradient: LinearGradient) -> Self {
        Self::LinearGradient(gradient)
    }
}

impl Paint {
    pub fn is_visible(&self) -> bool {
        match self {
            Self::Solid(color) => color.alpha() > 0.0,
            Self::LinearGradient(gradient) => gradient.stops.iter().any(|s| s.color.alpha() > 0.0),
        }
    }

    pub fn requires_linear_gradients(&self) -> bool {
        matches!(self, Self::LinearGradient(_))
    }

    pub(super) fn with_global_alpha(&self, alpha: f64) -> Self {
        let apply = |color: Color| color.with_alpha((color.alpha() * alpha).clamp(0.0, 1.0));
        match self {
            Self::Solid(color) => Self::Solid(apply(*color)),
            Self::LinearGradient(gradient) => Self::LinearGradient(LinearGradient {
                start: gradient.start,
                end: gradient.end,
                stops: gradient
                    .stops
                    .iter()
                    .map(|stop| GradientStop {
                        offset: stop.offset,
                        color: apply(stop.color),
                    })
                    .collect(),
            }),
        }
    }
}
