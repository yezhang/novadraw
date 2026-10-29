use novadraw::geometry::{Affine2D, Point, Rectangle};

use super::ContainerEditError;

/// Explicit residual permitted when representing transformed corners as XY bounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeometryTolerance {
    absolute: f64,
}

impl GeometryTolerance {
    /// Rejects any nonzero corner residual.
    pub const EXACT: Self = Self { absolute: 0.0 };

    /// Creates a tolerance in target-parent logical units.
    pub fn new(absolute: f64) -> Result<Self, ContainerEditError> {
        if !absolute.is_finite() || absolute < 0.0 {
            return Err(ContainerEditError::Geometry("invalid geometry tolerance"));
        }
        Ok(Self { absolute })
    }

    /// Returns the permitted absolute residual.
    pub const fn absolute(self) -> f64 {
        self.absolute
    }
}

impl Default for GeometryTolerance {
    fn default() -> Self {
        Self::EXACT
    }
}

/// Application constraint corners in the source-parent domain and its local grab transform.
#[derive(Clone, Debug, PartialEq)]
pub struct ConstraintGeometry {
    corners: Vec<Point>,
    local_to_parent: Affine2D,
}

impl ConstraintGeometry {
    /// Validates application geometry without deriving it from visual bounds.
    pub fn new(corners: Vec<Point>, local_to_parent: Affine2D) -> Result<Self, ContainerEditError> {
        if corners.is_empty() {
            return Err(ContainerEditError::Geometry(
                "constraint geometry has no corners",
            ));
        }
        finite_transform(local_to_parent)?;
        for &point in &corners {
            finite_point(point)?;
        }
        Ok(Self {
            corners,
            local_to_parent,
        })
    }

    /// Creates XY geometry with a translation-only grab domain, including zero-sized bounds.
    pub fn xy(bounds: Rectangle) -> Result<Self, ContainerEditError> {
        if bounds.width < 0.0 || bounds.height < 0.0 {
            return Err(ContainerEditError::Geometry("negative XY size"));
        }
        Self::new(
            vec![
                Point::new(bounds.x, bounds.y),
                Point::new(bounds.x + bounds.width, bounds.y),
                Point::new(bounds.x + bounds.width, bounds.y + bounds.height),
                Point::new(bounds.x, bounds.y + bounds.height),
            ],
            Affine2D::from_translation(bounds.x, bounds.y),
        )
    }

    /// Returns source-parent constraint corners.
    pub fn corners(&self) -> &[Point] {
        &self.corners
    }

    /// Returns the application's local-grab to source-parent map.
    pub const fn local_to_parent(&self) -> Affine2D {
        self.local_to_parent
    }

    /// Applies the checked composite `target_inverse * drag * source`.
    pub fn mapped(&self, transform: Affine2D) -> Result<Self, ContainerEditError> {
        finite_transform(transform)?;
        let corners = self
            .corners
            .iter()
            .map(|&point| map_point(transform, point))
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(corners, finite_transform(transform * self.local_to_parent)?)
    }

    /// Represents four ordered rectangle corners as normalized XY bounds.
    /// Reflection and axis exchange are allowed; rotation/skew is never replaced by an AABB.
    pub fn xy_bounds(&self, tolerance: GeometryTolerance) -> Result<Rectangle, ContainerEditError> {
        let [a, b, c, d] = self.corners.as_slice() else {
            return Err(ContainerEditError::Geometry(
                "XY requires four rectangle corners",
            ));
        };
        let close = |left: f64, right: f64| {
            let residual = left - right;
            residual.is_finite() && residual.abs() <= tolerance.absolute
        };
        let horizontal = close(a.y(), b.y())
            && close(b.x(), c.x())
            && close(c.y(), d.y())
            && close(d.x(), a.x());
        let vertical = close(a.x(), b.x())
            && close(b.y(), c.y())
            && close(c.x(), d.x())
            && close(d.y(), a.y());
        if !horizontal && !vertical {
            return Err(ContainerEditError::Geometry(
                "constraint is not representable as XY",
            ));
        }
        let min_x = a.x().min(b.x()).min(c.x()).min(d.x());
        let min_y = a.y().min(b.y()).min(c.y()).min(d.y());
        let max_x = a.x().max(b.x()).max(c.x()).max(d.x());
        let max_y = a.y().max(b.y()).max(c.y()).max(d.y());
        let size = finite_point(Point::new(max_x - min_x, max_y - min_y))?;
        Ok(Rectangle::new(min_x, min_y, size.x(), size.y()))
    }
}

pub(crate) fn finite_point(point: Point) -> Result<Point, ContainerEditError> {
    if point.x().is_finite() && point.y().is_finite() {
        Ok(point)
    } else {
        Err(ContainerEditError::Geometry(
            "non-finite point or derived overflow",
        ))
    }
}

pub(crate) fn finite_transform(transform: Affine2D) -> Result<Affine2D, ContainerEditError> {
    if transform.coeffs().into_iter().all(f64::is_finite) {
        Ok(transform)
    } else {
        Err(ContainerEditError::Geometry(
            "non-finite transform or derived overflow",
        ))
    }
}

pub(crate) fn map_point(transform: Affine2D, point: Point) -> Result<Point, ContainerEditError> {
    finite_point(point)?;
    finite_transform(transform)?;
    finite_point(transform.transform_point(point))
}

pub(crate) fn inverse(transform: Affine2D) -> Result<Option<Affine2D>, ContainerEditError> {
    transform
        .checked_inverse()
        .map_err(ContainerEditError::Inverse)
}
