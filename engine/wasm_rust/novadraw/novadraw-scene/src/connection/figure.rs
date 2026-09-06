use novadraw_core::Color;
use novadraw_geometry::{Point, PointList, Rectangle};
use novadraw_render::{
    NdCanvas,
    command::{LineCap, LineJoin},
};

use crate::{Bounded, ChildClippingStrategy, Figure, FigureContainer, Layer};

const DEFAULT_CONNECTION_COLOR: Color = Color {
    r: 44.0 / 255.0,
    g: 62.0 / 255.0,
    b: 80.0 / 255.0,
    a: 1.0,
};
const DEFAULT_CONNECTION_WIDTH: f64 = 2.0;
const DEFAULT_HIT_TOLERANCE: f64 = 3.0;

/// Mutable geometry boundary implemented by Connection Figures.
pub trait ConnectionFigureBehavior {
    /// Returns committed node-local route points.
    fn route_points(&self) -> &PointList;

    /// Replaces committed node-local route points.
    fn commit_route_points(&mut self, points: PointList);

    /// Returns stroke width used to derive path bounds and hit tolerance.
    fn connection_stroke_width(&self) -> f64;

    /// Returns the stroke color inherited by endpoint decorations.
    fn connection_stroke_color(&self) -> Color;
}

/// Polyline-backed Figure whose geometry is committed only by Runtime.
#[derive(Clone, Debug)]
pub struct ConnectionFigure {
    points: PointList,
    stroke_color: Color,
    stroke_width: f64,
    line_cap: LineCap,
    line_join: LineJoin,
    hit_tolerance: f64,
    source_decoration_inset: f64,
    target_decoration_inset: f64,
}

impl Default for ConnectionFigure {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionFigure {
    /// Creates an unresolved, non-painting Connection Figure.
    pub fn new() -> Self {
        Self {
            points: PointList::new(),
            stroke_color: DEFAULT_CONNECTION_COLOR,
            stroke_width: DEFAULT_CONNECTION_WIDTH,
            line_cap: LineCap::default(),
            line_join: LineJoin::default(),
            hit_tolerance: DEFAULT_HIT_TOLERANCE,
            source_decoration_inset: 0.0,
            target_decoration_inset: 0.0,
        }
    }

    /// Sets immutable construction-time stroke style.
    pub fn with_stroke(mut self, color: Color, width: f64) -> Self {
        self.stroke_color = color;
        self.stroke_width = width.max(0.0);
        self
    }

    /// Sets line cap and join style.
    pub fn with_line_style(mut self, cap: LineCap, join: LineJoin) -> Self {
        self.line_cap = cap;
        self.line_join = join;
        self
    }

    /// Sets additional hit tolerance without changing visual bounds.
    pub fn with_hit_tolerance(mut self, tolerance: f64) -> Self {
        self.hit_tolerance = tolerance.max(0.0);
        self
    }

    /// Stops the painted centerline before endpoint decorations.
    ///
    /// Route points remain unchanged and continue to represent Anchor truth.
    pub fn with_decoration_insets(mut self, source: f64, target: f64) -> Self {
        self.source_decoration_inset = finite_non_negative(source);
        self.target_decoration_inset = finite_non_negative(target);
        self
    }

    fn painted_points(&self) -> Vec<Point> {
        trim_polyline(
            self.points.as_slice(),
            self.source_decoration_inset,
            self.target_decoration_inset,
        )
    }
}

impl ConnectionFigureBehavior for ConnectionFigure {
    fn route_points(&self) -> &PointList {
        &self.points
    }

    fn commit_route_points(&mut self, points: PointList) {
        self.points = points;
    }

    fn connection_stroke_width(&self) -> f64 {
        self.stroke_width
    }

    fn connection_stroke_color(&self) -> Color {
        self.stroke_color
    }
}

impl Figure for ConnectionFigure {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::ZERO
    }

    fn name(&self) -> &'static str {
        "ConnectionFigure"
    }

    fn paint_figure_in_bounds(&self, gc: &mut NdCanvas, _bounds: Rectangle) {
        if self.points.len() < 2 || self.stroke_width <= 0.0 {
            return;
        }
        let points: Vec<_> = self
            .painted_points()
            .iter()
            .map(|point| glam::DVec2::new(point.x(), point.y()))
            .collect();
        gc.polyline(
            &points,
            self.stroke_color,
            self.stroke_width,
            self.line_cap,
            self.line_join,
        );
    }

    fn precise_hit(&self, x: f64, y: f64, _bounds: Rectangle) -> bool {
        if self.points.len() < 2 {
            return false;
        }
        let threshold = self.stroke_width / 2.0 + self.hit_tolerance;
        self.painted_points().windows(2).any(|segment| {
            point_segment_distance(Point::new(x, y), segment[0], segment[1]) <= threshold
        })
    }

    fn visual_bounds_in(&self, bounds: Rectangle) -> Rectangle {
        Rectangle::new(0.0, 0.0, bounds.width, bounds.height)
    }

    fn container(&self) -> Option<&dyn FigureContainer> {
        Some(self)
    }

    fn connection(&self) -> Option<&dyn ConnectionFigureBehavior> {
        Some(self)
    }

    fn connection_mut(&mut self) -> Option<&mut dyn ConnectionFigureBehavior> {
        Some(self)
    }
}

impl FigureContainer for ConnectionFigure {
    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        ChildClippingStrategy::OverflowVisible
    }
}

/// Transparent Layer dedicated to Connection children.
#[derive(Clone, Debug)]
pub struct ConnectionLayerFigure {
    bounds: Rectangle,
}

impl ConnectionLayerFigure {
    /// Creates a Connection Layer in its parent content domain.
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            bounds: Rectangle::new(x, y, width, height),
        }
    }
}

impl Bounded for ConnectionLayerFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }

    fn name(&self) -> &'static str {
        "ConnectionLayerFigure"
    }
}

impl Figure for ConnectionLayerFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "ConnectionLayerFigure"
    }

    fn hit_participation(&self) -> crate::HitParticipation {
        crate::HitParticipation::DescendantsOnly
    }

    fn container(&self) -> Option<&dyn FigureContainer> {
        Some(self)
    }

    fn layer(&self) -> Option<&dyn Layer> {
        Some(self)
    }
}

impl FigureContainer for ConnectionLayerFigure {
    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        ChildClippingStrategy::OverflowVisible
    }
}

impl Layer for ConnectionLayerFigure {}

fn point_segment_distance(point: Point, start: Point, end: Point) -> f64 {
    let segment = end - start;
    let length_squared = segment.length_squared();
    if length_squared <= f64::EPSILON {
        return (point - start).length();
    }
    let projection = ((point - start).dot(segment) / length_squared).clamp(0.0, 1.0);
    (point - (start + segment * projection)).length()
}

fn finite_non_negative(value: f64) -> f64 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

fn trim_polyline(points: &[Point], source_inset: f64, target_inset: f64) -> Vec<Point> {
    let mut points = points.to_vec();
    trim_end(&mut points, target_inset);
    points.reverse();
    trim_end(&mut points, source_inset);
    points.reverse();
    points
}

fn trim_end(points: &mut Vec<Point>, mut inset: f64) {
    while inset > 0.0 && points.len() >= 2 {
        let end = points[points.len() - 1];
        let previous = points[points.len() - 2];
        let segment = end - previous;
        let length = segment.length();
        if length <= f64::EPSILON {
            points.pop();
            continue;
        }
        if inset < length {
            let last = points.len() - 1;
            points[last] = end - segment * (inset / length);
            return;
        }
        inset -= length;
        points.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_decoration_inset_trims_only_painted_geometry() {
        let mut figure = ConnectionFigure::new().with_decoration_insets(0.0, 14.0);
        figure.commit_route_points(PointList::from_points(vec![
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
        ]));

        assert_eq!(
            figure.route_points().as_slice(),
            &[Point::new(0.0, 0.0), Point::new(100.0, 0.0)]
        );
        assert_eq!(
            figure.painted_points(),
            vec![Point::new(0.0, 0.0), Point::new(86.0, 0.0)]
        );
    }

    #[test]
    fn decoration_inset_can_cross_short_terminal_segments() {
        assert_eq!(
            trim_polyline(
                &[
                    Point::new(0.0, 0.0),
                    Point::new(90.0, 0.0),
                    Point::new(100.0, 0.0),
                ],
                0.0,
                15.0,
            ),
            vec![Point::new(0.0, 0.0), Point::new(85.0, 0.0)]
        );
    }
}
