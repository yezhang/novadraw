//! 逐角与分边配置同样走开放的 typed component update。

use super::{FigureComponentContext, FigureComponentUpdate, PreparedFigureUpdate};
use crate::event::property;
use crate::figure::{
    Bounded, PolygonFigure, RectangleCorners, RectangleEdges, RoundedRectangleFigure,
    ShapeGeometryError, TriangleFigure,
};
use crate::geometry::{Point, Rectangle};

pub struct SetRectangleCorners(Option<RectangleCorners>);

impl SetRectangleCorners {
    /// None 恢复 legacy corner dimensions。
    pub const fn new(corners: Option<RectangleCorners>) -> Self {
        Self(corners)
    }
}

impl FigureComponentUpdate for SetRectangleCorners {
    type Figure = RoundedRectangleFigure;
    type Prepared = RoundedRectangleFigure;
    type Error = ShapeGeometryError;

    fn prepare(
        self,
        current: &Self::Figure,
        context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        let old = current.corners();
        if old == self.0 {
            return Ok(PreparedFigureUpdate::unchanged(current.clone()));
        }
        let mut candidate = current.clone();
        let bounds = context.bounds;
        candidate.set_bounds(bounds.x, bounds.y, bounds.width, bounds.height)?;
        candidate.replace_corners(self.0)?;
        let bounds = current.bounds();
        candidate.set_bounds(bounds.x, bounds.y, bounds.width, bounds.height)?;
        Ok(
            PreparedFigureUpdate::geometry_and_paint(candidate).with_property_change(
                property::RECTANGLE_CORNERS,
                old,
                self.0,
            ),
        )
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        *target = prepared;
    }
}

pub struct SetRectangleEdges(RectangleEdges);

impl SetRectangleEdges {
    pub const fn new(edges: RectangleEdges) -> Self {
        Self(edges)
    }
}

impl FigureComponentUpdate for SetRectangleEdges {
    type Figure = RoundedRectangleFigure;
    type Prepared = RoundedRectangleFigure;
    type Error = ShapeGeometryError;

    fn prepare(
        self,
        current: &Self::Figure,
        context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        let old = current.edges();
        if old == self.0 {
            return Ok(PreparedFigureUpdate::unchanged(current.clone()));
        }
        let mut candidate = current.clone();
        let bounds = context.bounds;
        candidate.set_bounds(bounds.x, bounds.y, bounds.width, bounds.height)?;
        // Preflight geometry even for a transition from the legacy path.
        candidate.replace_corners(current.corners())?;
        candidate.replace_edges(self.0);
        let bounds = current.bounds();
        candidate.set_bounds(bounds.x, bounds.y, bounds.width, bounds.height)?;
        Ok(PreparedFigureUpdate::paint(candidate).with_property_change(
            property::RECTANGLE_EDGES,
            old,
            self.0,
        ))
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        *target = prepared;
    }
}

pub struct SetPolygonCornerRadii(Vec<f64>);

impl SetPolygonCornerRadii {
    pub fn new(radii: Vec<f64>) -> Self {
        Self(radii)
    }
}

impl FigureComponentUpdate for SetPolygonCornerRadii {
    type Figure = PolygonFigure;
    type Prepared = PolygonFigure;
    type Error = ShapeGeometryError;

    fn prepare(
        self,
        current: &Self::Figure,
        context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        if current.corner_radii() == self.0 {
            return Ok(PreparedFigureUpdate::unchanged(current.clone()));
        }
        let old = current.corner_radii().to_vec();
        let mut candidate = current.clone();
        candidate.replace_corner_radii(self.0.clone())?;
        candidate.contour_in(context.bounds)?;
        Ok(
            PreparedFigureUpdate::geometry_and_paint(candidate).with_property_change(
                property::POLYGON_CORNER_RADII,
                old,
                self.0,
            ),
        )
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        *target = prepared;
    }
}

pub struct SetTriangleCornerRadii([f64; 3]);

impl SetTriangleCornerRadii {
    pub const fn new(radii: [f64; 3]) -> Self {
        Self(radii)
    }
}

impl FigureComponentUpdate for SetTriangleCornerRadii {
    type Figure = TriangleFigure;
    type Prepared = TriangleFigure;
    type Error = ShapeGeometryError;

    fn prepare(
        self,
        current: &Self::Figure,
        context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        let old = current.corner_radii();
        if old == self.0 {
            return Ok(PreparedFigureUpdate::unchanged(current.clone()));
        }
        let mut candidate = current.clone();
        let bounds = context.bounds;
        candidate.set_bounds(bounds.x, bounds.y, bounds.width, bounds.height)?;
        candidate.invalidate();
        candidate.replace_corner_radii(self.0)?;
        candidate.validate();
        Ok(
            PreparedFigureUpdate::geometry_and_paint(candidate).with_property_change(
                property::TRIANGLE_CORNER_RADII,
                old,
                self.0,
            ),
        )
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        *target = prepared;
    }
}

/// 点和半径在同一次事务更新，points 使用 parent child-content 坐标。
pub struct ReplacePolygonGeometry {
    points: Vec<Point>,
    radii: Vec<f64>,
}

impl ReplacePolygonGeometry {
    pub fn new(points: Vec<Point>, radii: Vec<f64>) -> Self {
        Self { points, radii }
    }
}

struct PolygonBounds(Rectangle);

impl super::FigureUpdateEffect for PolygonBounds {
    fn prepare(
        self: Box<Self>,
        context: &mut super::FigureUpdateEffectContext<'_>,
    ) -> Result<(), super::RuntimeMutationError> {
        context.set_bounds(self.0)
    }
}

impl FigureComponentUpdate for ReplacePolygonGeometry {
    type Figure = PolygonFigure;
    type Prepared = PolygonFigure;
    type Error = ShapeGeometryError;

    fn prepare(
        self,
        current: &Self::Figure,
        context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        // Validate before normalization, which otherwise accepts degenerate straight lists.
        crate::figure::ShapeContour::polygon(&self.points, &self.radii)?;
        let old_points: Vec<_> = current
            .get_points()
            .iter()
            .map(|p| Point::new(p.x() + context.bounds.x, p.y() + context.bounds.y))
            .collect();
        if old_points == self.points && current.corner_radii() == self.radii {
            return Ok(PreparedFigureUpdate::unchanged(current.clone()));
        }
        let (bounds, points) =
            crate::figure::normalize_points(self.points.clone(), current.stroke_style(), 3)?;
        let mut candidate = current.clone();
        // Old radii need not match the new count.
        candidate.clear_corner_radii();
        candidate.commit_geometry(bounds, points);
        candidate.replace_corner_radii(self.radii.clone())?;
        let old_radii = current.corner_radii().to_vec();
        Ok(PreparedFigureUpdate::layout_geometry_and_paint(candidate)
            .with_property_change(property::POINTS, old_points, self.points)
            .with_property_change(property::POLYGON_CORNER_RADII, old_radii, self.radii)
            .with_effect(PolygonBounds(bounds)))
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        *target = prepared;
    }
}
