use kurbo::Shape as _;
use novadraw_geometry::{Dimension, Point, Rectangle};

use crate::command::PathOp;

const ARC_APPROXIMATION_TOLERANCE: f64 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CubicSegment {
    pub control1: Point,
    pub control2: Point,
    pub end: Point,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NormalizedPathOp {
    MoveTo(Point),
    LineTo(Point),
    CubicTo(CubicSegment),
    QuadTo { control: Point, end: Point },
    Close,
}

pub(crate) fn center_arc_cubics(
    center: Point,
    radii: Dimension,
    start_angle: f64,
    sweep_angle: f64,
) -> Vec<CubicSegment> {
    let mut segments = Vec::new();
    kurbo::Arc::new(
        (center.x(), center.y()),
        (radii.width, radii.height),
        start_angle,
        sweep_angle,
        0.0,
    )
    .to_cubic_beziers(ARC_APPROXIMATION_TOLERANCE, |control1, control2, end| {
        segments.push(CubicSegment {
            control1: from_kurbo_point(control1),
            control2: from_kurbo_point(control2),
            end: from_kurbo_point(end),
        });
    });
    segments
}

pub fn for_each_normalized(
    operations: &[PathOp],
    scale: f64,
    mut emit: impl FnMut(NormalizedPathOp),
) {
    let mut current = None;
    let mut subpath_start = None;

    for operation in operations {
        match operation {
            PathOp::MoveTo(point) => {
                let point = scaled_point(*point, scale);
                emit(NormalizedPathOp::MoveTo(point));
                current = Some(point);
                subpath_start = Some(point);
            }
            PathOp::LineTo(point) => {
                let point = scaled_point(*point, scale);
                emit(NormalizedPathOp::LineTo(point));
                current = Some(point);
            }
            PathOp::HLineTo(x) => {
                let point = Point::new(*x * scale, current.map_or(0.0, Point::y));
                emit(NormalizedPathOp::LineTo(point));
                current = Some(point);
            }
            PathOp::VLineTo(y) => {
                let point = Point::new(current.map_or(0.0, Point::x), *y * scale);
                emit(NormalizedPathOp::LineTo(point));
                current = Some(point);
            }
            PathOp::CubicTo(control1, control2, end) => {
                let segment = CubicSegment {
                    control1: scaled_point(*control1, scale),
                    control2: scaled_point(*control2, scale),
                    end: scaled_point(*end, scale),
                };
                emit(NormalizedPathOp::CubicTo(segment));
                current = Some(segment.end);
            }
            PathOp::QuadTo(control, end) => {
                let control = scaled_point(*control, scale);
                let end = scaled_point(*end, scale);
                emit(NormalizedPathOp::QuadTo { control, end });
                current = Some(end);
            }
            PathOp::Arc {
                radii,
                rotation,
                large_arc,
                sweep,
                dest,
            } => {
                let destination = scaled_point(*dest, scale);
                let Some(source) = current else {
                    emit(NormalizedPathOp::MoveTo(destination));
                    current = Some(destination);
                    subpath_start = Some(destination);
                    continue;
                };
                let svg_arc = kurbo::SvgArc {
                    from: to_kurbo_point(source),
                    to: to_kurbo_point(destination),
                    radii: kurbo::Vec2::new(radii.width * scale, radii.height * scale),
                    x_rotation: *rotation,
                    large_arc: *large_arc,
                    sweep: *sweep,
                };
                if let Some(arc) = kurbo::Arc::from_svg_arc(&svg_arc) {
                    arc.to_cubic_beziers(ARC_APPROXIMATION_TOLERANCE, |control1, control2, end| {
                        emit(NormalizedPathOp::CubicTo(CubicSegment {
                            control1: from_kurbo_point(control1),
                            control2: from_kurbo_point(control2),
                            end: from_kurbo_point(end),
                        }));
                    });
                } else {
                    emit(NormalizedPathOp::LineTo(destination));
                }
                current = Some(destination);
            }
            PathOp::Close => {
                emit(NormalizedPathOp::Close);
                current = subpath_start;
            }
        }
    }
}

pub(crate) fn bounding_box(operations: &[PathOp]) -> Option<Rectangle> {
    if operations.is_empty() {
        return None;
    }

    let mut path = kurbo::BezPath::new();
    for_each_normalized(operations, 1.0, |operation| match operation {
        NormalizedPathOp::MoveTo(point) => path.move_to(to_kurbo_point(point)),
        NormalizedPathOp::LineTo(point) => path.line_to(to_kurbo_point(point)),
        NormalizedPathOp::CubicTo(segment) => path.curve_to(
            to_kurbo_point(segment.control1),
            to_kurbo_point(segment.control2),
            to_kurbo_point(segment.end),
        ),
        NormalizedPathOp::QuadTo { control, end } => {
            path.quad_to(to_kurbo_point(control), to_kurbo_point(end));
        }
        NormalizedPathOp::Close => path.close_path(),
    });
    let bounds = path.bounding_box();
    Some(Rectangle::new(
        bounds.x0,
        bounds.y0,
        bounds.width(),
        bounds.height(),
    ))
}

fn scaled_point(point: Point, scale: f64) -> Point {
    Point::new(point.x() * scale, point.y() * scale)
}

fn to_kurbo_point(point: Point) -> kurbo::Point {
    kurbo::Point::new(point.x(), point.y())
}

fn from_kurbo_point(point: kurbo::Point) -> Point {
    Point::new(point.x, point.y)
}
