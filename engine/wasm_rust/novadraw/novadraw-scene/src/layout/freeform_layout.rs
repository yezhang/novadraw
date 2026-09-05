use std::{error::Error, fmt};

use novadraw_geometry::{Point, Rectangle};

use super::{LayoutError, LayoutManager, LayoutOutput, LayoutSnapshot};
use crate::graph::FigureId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FreeformConstraint {
    origin: Point,
    width: Option<f64>,
    height: Option<f64>,
}

impl FreeformConstraint {
    pub fn new(
        origin: Point,
        width: Option<f64>,
        height: Option<f64>,
    ) -> Result<Self, FreeformConstraintError> {
        if !origin.x().is_finite() || !origin.y().is_finite() {
            return Err(FreeformConstraintError::NonFiniteOrigin);
        }
        validate_axis(width).map_err(|()| FreeformConstraintError::InvalidWidth)?;
        validate_axis(height).map_err(|()| FreeformConstraintError::InvalidHeight)?;
        Ok(Self {
            origin,
            width,
            height,
        })
    }

    pub fn at(origin: Point) -> Result<Self, FreeformConstraintError> {
        Self::new(origin, None, None)
    }

    pub fn fixed(origin: Point, width: f64, height: f64) -> Result<Self, FreeformConstraintError> {
        Self::new(origin, Some(width), Some(height))
    }

    pub fn origin(self) -> Point {
        self.origin
    }

    pub fn width(self) -> Option<f64> {
        self.width
    }

    pub fn height(self) -> Option<f64> {
        self.height
    }
}

fn validate_axis(value: Option<f64>) -> Result<(), ()> {
    if value.is_some_and(|value| !value.is_finite() || value < 0.0) {
        Err(())
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FreeformConstraintError {
    NonFiniteOrigin,
    InvalidWidth,
    InvalidHeight,
}

impl fmt::Display for FreeformConstraintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFiniteOrigin => write!(formatter, "freeform origin must be finite"),
            Self::InvalidWidth => {
                write!(formatter, "freeform width must be finite and non-negative")
            }
            Self::InvalidHeight => {
                write!(formatter, "freeform height must be finite and non-negative")
            }
        }
    }
}

impl Error for FreeformConstraintError {}

#[derive(Clone, Copy, Debug, Default)]
pub struct FreeformLayout;

impl FreeformLayout {
    pub fn new() -> Self {
        Self
    }

    fn measure(
        self,
        container: FigureId,
        snapshot: &LayoutSnapshot<'_>,
        minimum: bool,
    ) -> (f64, f64) {
        let mut extent = Rectangle::ZERO;
        for (child, current) in snapshot.children(container) {
            let bounds = match freeform_constraint(snapshot, container, child) {
                Ok(Some(constraint)) => {
                    let intrinsic = if minimum {
                        snapshot.minimum_size(child, -1.0, -1.0)
                    } else {
                        snapshot.preferred_size(child, -1.0, -1.0)
                    };
                    Rectangle::new(
                        constraint.origin.x(),
                        constraint.origin.y(),
                        constraint.width.unwrap_or(intrinsic.0),
                        constraint.height.unwrap_or(intrinsic.1),
                    )
                }
                Ok(None) | Err(_) => current,
            };
            extent = extent.union(bounds);
        }
        (extent.width, extent.height)
    }
}

impl LayoutManager for FreeformLayout {
    fn get_preferred_size(
        &self,
        container: FigureId,
        _w_hint: f64,
        _h_hint: f64,
        snapshot: &LayoutSnapshot<'_>,
    ) -> (f64, f64) {
        self.measure(container, snapshot, false)
    }

    fn get_minimum_size(
        &self,
        container: FigureId,
        _w_hint: f64,
        _h_hint: f64,
        snapshot: &LayoutSnapshot<'_>,
    ) -> (f64, f64) {
        self.measure(container, snapshot, true)
    }

    fn layout(
        &mut self,
        container: FigureId,
        snapshot: &LayoutSnapshot<'_>,
        out: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        for (child, _) in snapshot.children(container) {
            let Some(constraint) = freeform_constraint(snapshot, container, child)? else {
                continue;
            };
            let intrinsic = snapshot.preferred_size(child, -1.0, -1.0);
            out.set_child_bounds(
                child,
                Rectangle::new(
                    constraint.origin.x(),
                    constraint.origin.y(),
                    constraint.width.unwrap_or(intrinsic.0),
                    constraint.height.unwrap_or(intrinsic.1),
                ),
            );
        }
        Ok(())
    }
}

fn freeform_constraint(
    snapshot: &LayoutSnapshot<'_>,
    container: FigureId,
    child: FigureId,
) -> Result<Option<FreeformConstraint>, LayoutError> {
    snapshot
        .constraint_as::<FreeformConstraint>(container, child)
        .map(|constraint| constraint.copied())
}
