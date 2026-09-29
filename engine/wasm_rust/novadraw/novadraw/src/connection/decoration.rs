use std::{error::Error, fmt};

use crate::figure::normalize_points;
use crate::geometry::{Point, PointList, Rectangle};
use crate::render::{LineJoin, NdCanvas};
use crate::{
    Color, Figure, FigureStyle, LocatorPlacement, PointListFigureBehavior, PolygonFigure,
    PolylineFigure,
};

const DEFAULT_ARROW_LENGTH: f64 = 10.0;
const DEFAULT_ARROW_HALF_WIDTH: f64 = 5.0;

/// Geometry prepared for a route-oriented Connection decoration.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedDecorationGeometry {
    bounds: Rectangle,
    local_points: Vec<Point>,
}

impl PreparedDecorationGeometry {
    /// Returns child bounds in the Connection's local coordinate space.
    pub const fn bounds(&self) -> Rectangle {
        self.bounds
    }

    /// Returns normalized child-local points.
    pub fn local_points(&self) -> &[Point] {
        &self.local_points
    }

    pub(crate) fn into_parts(self) -> (Rectangle, Vec<Point>) {
        (self.bounds, self.local_points)
    }
}

/// Failure to construct or orient a Connection decoration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecorationError {
    /// The template cannot form the requested open or closed shape.
    TooFewTemplatePoints { minimum: usize, actual: usize },
    /// A template coordinate or scale is non-finite.
    NonFiniteGeometry,
    /// A scale is negative.
    NegativeScale,
    /// The route placement has no usable direction.
    DegenerateDirection,
}

impl fmt::Display for DecorationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooFewTemplatePoints { minimum, actual } => write!(
                formatter,
                "decoration template requires at least {minimum} points, got {actual}"
            ),
            Self::NonFiniteGeometry => {
                formatter.write_str("decoration geometry and scale must be finite")
            }
            Self::NegativeScale => formatter.write_str("decoration scale must be non-negative"),
            Self::DegenerateDirection => {
                formatter.write_str("decoration placement requires a non-zero direction")
            }
        }
    }
}

impl Error for DecorationError {}

/// Runtime-controlled geometry behavior for endpoint decorations.
pub trait ConnectionDecorationBehavior {
    /// Prepares complete child geometry without mutating Figure or Runtime state.
    fn prepare_decoration_geometry(
        &self,
        placement: LocatorPlacement,
    ) -> Result<PreparedDecorationGeometry, DecorationError>;

    /// Commits geometry that was successfully preflighted.
    fn commit_decoration_geometry(&mut self, geometry: PreparedDecorationGeometry);
}

/// Filled, closed endpoint decoration backed by a canonical polygon template.
#[derive(Clone)]
pub struct PolygonDecorationFigure {
    template: PointList,
    scale_x: f64,
    scale_y: f64,
    polygon: PolygonFigure,
}

impl PolygonDecorationFigure {
    /// Creates a decoration whose template points in the positive X direction.
    ///
    /// The route endpoint is placed at template origin. Templates normally keep
    /// their body on the negative X side.
    pub fn from_template(template: PointList) -> Result<Self, DecorationError> {
        validate_template(&template, 3)?;
        Ok(Self {
            template,
            scale_x: 1.0,
            scale_y: 1.0,
            polygon: PolygonFigure::from_points(Vec::new()),
        })
    }

    /// Creates the standard triangular arrow decoration.
    pub fn triangle() -> Self {
        Self::from_template(PointList::from_points(vec![
            Point::ZERO,
            Point::new(-DEFAULT_ARROW_LENGTH, -DEFAULT_ARROW_HALF_WIDTH),
            Point::new(-DEFAULT_ARROW_LENGTH, DEFAULT_ARROW_HALF_WIDTH),
        ]))
        .expect("built-in decoration template is valid")
    }

    /// Applies independent finite, non-negative template scales.
    pub fn with_scale(mut self, scale_x: f64, scale_y: f64) -> Result<Self, DecorationError> {
        validate_scale(scale_x, scale_y)?;
        self.scale_x = scale_x;
        self.scale_y = scale_y;
        Ok(self)
    }

    /// Sets fill color.
    pub fn with_fill_color(mut self, color: Color) -> Self {
        self.polygon = self.polygon.with_fill_color(color);
        self
    }

    /// Sets outline color and width.
    pub fn with_stroke(mut self, color: Color, width: f64) -> Self {
        self.polygon = self.polygon.with_stroke(color, width);
        self
    }

    /// Sets outline join.
    pub fn with_join(mut self, join: LineJoin) -> Self {
        self.polygon = self.polygon.with_join(join);
        self
    }
}

impl ConnectionDecorationBehavior for PolygonDecorationFigure {
    fn prepare_decoration_geometry(
        &self,
        placement: LocatorPlacement,
    ) -> Result<PreparedDecorationGeometry, DecorationError> {
        prepare_geometry(
            &self.template,
            self.scale_x,
            self.scale_y,
            placement,
            PointListFigureBehavior::stroke_width(&self.polygon),
            PointListFigureBehavior::line_join(&self.polygon),
            3,
        )
    }

    fn commit_decoration_geometry(&mut self, geometry: PreparedDecorationGeometry) {
        let (bounds, points) = geometry.into_parts();
        PointListFigureBehavior::commit_geometry(&mut self.polygon, bounds, points);
    }
}

impl Figure for PolygonDecorationFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.polygon.initial_bounds()
    }

    fn name(&self) -> &'static str {
        "PolygonDecorationFigure"
    }

    fn initial_style(&self) -> FigureStyle {
        self.polygon.initial_style()
    }

    fn paint_figure_in_bounds(&self, gc: &mut NdCanvas, bounds: Rectangle) {
        self.polygon.paint_figure_in_bounds(gc, bounds);
    }

    fn precise_hit(&self, x: f64, y: f64, bounds: Rectangle) -> bool {
        self.polygon.precise_hit(x, y, bounds)
    }

    fn connection_decoration(&self) -> Option<&dyn ConnectionDecorationBehavior> {
        Some(self)
    }

    fn connection_decoration_mut(&mut self) -> Option<&mut dyn ConnectionDecorationBehavior> {
        Some(self)
    }
}

/// Open endpoint decoration backed by a canonical polyline template.
#[derive(Clone)]
pub struct PolylineDecorationFigure {
    template: PointList,
    scale_x: f64,
    scale_y: f64,
    polyline: PolylineFigure,
}

impl PolylineDecorationFigure {
    /// Creates a decoration whose template points in the positive X direction.
    pub fn from_template(template: PointList) -> Result<Self, DecorationError> {
        validate_template(&template, 2)?;
        Ok(Self {
            template,
            scale_x: 1.0,
            scale_y: 1.0,
            polyline: PolylineFigure::from_points(Vec::new()),
        })
    }

    /// Creates the standard open V arrow decoration.
    pub fn arrow() -> Self {
        Self::from_template(PointList::from_points(vec![
            Point::new(-DEFAULT_ARROW_LENGTH, -DEFAULT_ARROW_HALF_WIDTH),
            Point::ZERO,
            Point::new(-DEFAULT_ARROW_LENGTH, DEFAULT_ARROW_HALF_WIDTH),
        ]))
        .expect("built-in decoration template is valid")
    }

    /// Applies independent finite, non-negative template scales.
    pub fn with_scale(mut self, scale_x: f64, scale_y: f64) -> Result<Self, DecorationError> {
        validate_scale(scale_x, scale_y)?;
        self.scale_x = scale_x;
        self.scale_y = scale_y;
        Ok(self)
    }

    /// Sets line color.
    pub fn with_color(mut self, color: Color) -> Self {
        self.polyline = self.polyline.with_color(color);
        self
    }

    /// Sets line width.
    pub fn with_width(mut self, width: f64) -> Self {
        self.polyline = self.polyline.with_width(width);
        self
    }

    /// Sets line join.
    pub fn with_join(mut self, join: LineJoin) -> Self {
        self.polyline = self.polyline.with_join(join);
        self
    }
}

impl ConnectionDecorationBehavior for PolylineDecorationFigure {
    fn prepare_decoration_geometry(
        &self,
        placement: LocatorPlacement,
    ) -> Result<PreparedDecorationGeometry, DecorationError> {
        prepare_geometry(
            &self.template,
            self.scale_x,
            self.scale_y,
            placement,
            PointListFigureBehavior::stroke_width(&self.polyline),
            PointListFigureBehavior::line_join(&self.polyline),
            2,
        )
    }

    fn commit_decoration_geometry(&mut self, geometry: PreparedDecorationGeometry) {
        let (bounds, points) = geometry.into_parts();
        PointListFigureBehavior::commit_geometry(&mut self.polyline, bounds, points);
    }
}

impl Figure for PolylineDecorationFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.polyline.initial_bounds()
    }

    fn name(&self) -> &'static str {
        "PolylineDecorationFigure"
    }

    fn initial_style(&self) -> FigureStyle {
        self.polyline.initial_style()
    }

    fn paint_figure_in_bounds(&self, gc: &mut NdCanvas, bounds: Rectangle) {
        self.polyline.paint_figure_in_bounds(gc, bounds);
    }

    fn precise_hit(&self, x: f64, y: f64, bounds: Rectangle) -> bool {
        self.polyline.precise_hit(x, y, bounds)
    }

    fn connection_decoration(&self) -> Option<&dyn ConnectionDecorationBehavior> {
        Some(self)
    }

    fn connection_decoration_mut(&mut self) -> Option<&mut dyn ConnectionDecorationBehavior> {
        Some(self)
    }
}

fn prepare_geometry(
    template: &PointList,
    scale_x: f64,
    scale_y: f64,
    placement: LocatorPlacement,
    stroke_width: f64,
    line_join: LineJoin,
    painted_minimum: usize,
) -> Result<PreparedDecorationGeometry, DecorationError> {
    let direction = placement.point - placement.reference;
    let length = direction.length();
    if !length.is_finite() || length <= f64::EPSILON {
        return Err(DecorationError::DegenerateDirection);
    }
    let tangent = direction / length;
    let normal = crate::Vec2::new(-tangent.y(), tangent.x());
    let points = template
        .iter()
        .map(|point| {
            placement.point + tangent * (point.x() * scale_x) + normal * (point.y() * scale_y)
        })
        .collect::<Vec<_>>();
    if points
        .iter()
        .any(|point| !point.x().is_finite() || !point.y().is_finite())
    {
        return Err(DecorationError::NonFiniteGeometry);
    }
    let (bounds, local_points) = normalize_points(points, stroke_width, line_join, painted_minimum);
    if !finite_rectangle(bounds) {
        return Err(DecorationError::NonFiniteGeometry);
    }
    Ok(PreparedDecorationGeometry {
        bounds,
        local_points,
    })
}

fn validate_template(template: &PointList, minimum: usize) -> Result<(), DecorationError> {
    if template.len() < minimum {
        return Err(DecorationError::TooFewTemplatePoints {
            minimum,
            actual: template.len(),
        });
    }
    if template
        .iter()
        .any(|point| !point.x().is_finite() || !point.y().is_finite())
    {
        return Err(DecorationError::NonFiniteGeometry);
    }
    Ok(())
}

fn validate_scale(scale_x: f64, scale_y: f64) -> Result<(), DecorationError> {
    if !scale_x.is_finite() || !scale_y.is_finite() {
        return Err(DecorationError::NonFiniteGeometry);
    }
    if scale_x < 0.0 || scale_y < 0.0 {
        return Err(DecorationError::NegativeScale);
    }
    Ok(())
}

fn finite_rectangle(rectangle: Rectangle) -> bool {
    rectangle.x.is_finite()
        && rectangle.y.is_finite()
        && rectangle.width.is_finite()
        && rectangle.height.is_finite()
        && rectangle.width >= 0.0
        && rectangle.height >= 0.0
        && (rectangle.x + rectangle.width).is_finite()
        && (rectangle.y + rectangle.height).is_finite()
}
