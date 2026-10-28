//! Bounded logical-space stroke outlines for device-center sampling.
//! Curves are subdivided using positional and radius-weighted normal projection bounds.
//! No call to Kurbo's privately allocated stroke/dash/offset contexts is made.

use super::antialias::{AntialiasWorkLimits, Work, limit_failure, reserve};
use novadraw::render::RenderFailure;
use vello::kurbo::{
    Arc, BezPath, Cap, Join, Line, ParamCurve, ParamCurveExtrema, PathEl, PathSeg, Point, Vec2,
};

// Fixed device-space precision, independent of mask size or damage clipping.
pub(super) const DEVICE_PRECISION: f64 = 0.0001;
const MAX_SUBDIVISION_DEPTH: usize = 32;

pub(super) struct Style<'a> {
    pub width: f64,
    pub join: Join,
    pub start_cap: Cap,
    pub end_cap: Cap,
    pub miter_limit: f64,
    pub dash_pattern: &'a [f64],
    pub dash_offset: f64,
}

struct Budget {
    work: Work,
    limits: AntialiasWorkLimits,
    index: usize,
    tolerance: f64,
}

impl Budget {
    fn segment(&mut self) -> Result<(), RenderFailure> {
        self.work.add(
            Work {
                segments: 1,
                ..Work::default()
            },
            self.limits,
            self.index,
        )
    }

    fn element(&mut self, output: &mut Vec<PathEl>, element: PathEl) -> Result<(), RenderFailure> {
        self.work.add(
            Work {
                elements: 1,
                ..Work::default()
            },
            self.limits,
            self.index,
        )?;
        reserve(output, 1)?;
        output.push(element);
        Ok(())
    }
}

struct Contour {
    points: Vec<Point>,
    closed: bool,
}

fn flatten(
    segment: PathSeg,
    radius: f64,
    tolerance: f64,
    depth: usize,
    points: &mut Vec<Point>,
    budget: &mut Budget,
) -> Result<(), RenderFailure> {
    budget.segment()?;
    let (controls, count) = match segment {
        PathSeg::Line(line) => ([line.p0, line.p1, line.p1, line.p1], 2),
        PathSeg::Quad(quad) => ([quad.p0, quad.p1, quad.p2, quad.p2], 3),
        PathSeg::Cubic(cubic) => ([cubic.p0, cubic.p1, cubic.p2, cubic.p3], 4),
    };
    // Extrema splitting can leave numerically collapsed endpoint ranges. Their
    // tangent has no stable direction; retaining it would invent a large join.
    // Drop only a control hull entirely within the fixed positional tolerance.
    if controls[..count]
        .iter()
        .all(|p| (*p - controls[0]).hypot() <= tolerance)
    {
        return Ok(());
    }
    let chord = controls[count - 1] - controls[0];
    let length = chord.hypot();
    let straight = if length > 0.0 {
        let direction = chord / length;
        let position_ok = controls[1..count - 1].iter().all(|point| {
            let delta = *point - controls[0];
            delta.cross(direction).abs() <= tolerance
                && delta.dot(direction) >= -tolerance
                && delta.dot(direction) <= length + tolerance
        });
        let normal_ok = controls[..count].windows(2).all(|pair| {
            let tangent = pair[1] - pair[0];
            let norm = tangent.hypot();
            norm == 0.0 || (1.0 - (tangent / norm).dot(direction)) * radius <= tolerance
        });
        position_ok && normal_ok
    } else {
        controls[..count]
            .iter()
            .all(|point| (*point - controls[0]).hypot() <= tolerance)
    };
    if straight {
        let end = segment.end();
        if points.last() != Some(&end) {
            reserve(points, 1)?;
            points.push(end);
        }
        return Ok(());
    }
    if depth == MAX_SUBDIVISION_DEPTH {
        return Err(limit_failure(
            budget.index,
            "stroke subdivision depth",
            depth + 1,
            MAX_SUBDIVISION_DEPTH,
        ));
    }
    flatten(
        segment.subsegment(0.0..0.5),
        radius,
        tolerance,
        depth + 1,
        points,
        budget,
    )?;
    flatten(
        segment.subsegment(0.5..1.0),
        radius,
        tolerance,
        depth + 1,
        points,
        budget,
    )
}

fn contours(
    path: &BezPath,
    radius: f64,
    tolerance: f64,
    budget: &mut Budget,
) -> Result<Vec<Contour>, RenderFailure> {
    let mut output = Vec::new();
    let mut points = Vec::new();
    let mut last = Point::ZERO;
    for element in path.elements() {
        let segment = match *element {
            PathEl::MoveTo(point) => {
                if !points.is_empty() {
                    reserve(&mut output, 1)?;
                    output.push(Contour {
                        points: std::mem::take(&mut points),
                        closed: false,
                    });
                }
                reserve(&mut points, 1)?;
                points.push(point);
                last = point;
                None
            }
            PathEl::LineTo(point) => Some(PathSeg::Line(Line::new(last, point))),
            PathEl::QuadTo(control, point) => Some(PathSeg::Quad(vello::kurbo::QuadBez::new(
                last, control, point,
            ))),
            PathEl::CurveTo(a, b, point) => Some(PathSeg::Cubic(vello::kurbo::CubicBez::new(
                last, a, b, point,
            ))),
            PathEl::ClosePath => {
                if let Some(first) = points.first().copied() {
                    if points.last() != Some(&first) {
                        budget.segment()?;
                        reserve(&mut points, 1)?;
                        points.push(first);
                    }
                    reserve(&mut output, 1)?;
                    output.push(Contour {
                        points: std::mem::take(&mut points),
                        closed: true,
                    });
                    last = first;
                }
                None
            }
        };
        if let Some(segment) = segment {
            // Splitting at extrema handles collinear reversals and zero tangents.
            for range in segment.extrema_ranges() {
                flatten(
                    segment.subsegment(range),
                    radius,
                    tolerance,
                    0,
                    &mut points,
                    budget,
                )?;
            }
            last = segment.end();
        }
    }
    if !points.is_empty() {
        reserve(&mut output, 1)?;
        output.push(Contour {
            points,
            closed: false,
        });
    }
    Ok(output)
}

fn dash(
    contour: Contour,
    stroke: &Style<'_>,
    budget: &mut Budget,
) -> Result<Vec<Contour>, RenderFailure> {
    if stroke.dash_pattern.is_empty() {
        let mut result = Vec::new();
        reserve(&mut result, 1)?;
        result.push(contour);
        return Ok(result);
    }
    let pattern = &stroke.dash_pattern;
    let period: f64 = pattern.iter().sum();
    let mut phase = stroke.dash_offset.rem_euclid(period);
    let mut index = 0;
    while phase >= pattern[index] {
        phase -= pattern[index];
        index = (index + 1) % pattern.len();
    }
    let mut remaining = pattern[index] - phase;
    let initially_on = index % 2 == 0;
    let mut points = Vec::new();
    let mut output: Vec<Contour> = Vec::new();
    for pair in contour.points.windows(2) {
        let vector = pair[1] - pair[0];
        let length = vector.hypot();
        if length == 0.0 {
            continue;
        }
        let mut used = 0.0;
        while used < length {
            budget.segment()?;
            let amount = remaining.min(length - used);
            let end = used + amount;
            if end == used {
                return Err(limit_failure(
                    budget.index,
                    "dash numeric progress",
                    usize::MAX,
                    budget.limits.segments,
                ));
            }
            if index % 2 == 0 {
                if points.is_empty() {
                    reserve(&mut points, 1)?;
                    points.push(pair[0] + vector * (used / length));
                }
                reserve(&mut points, 1)?;
                points.push(if end == length {
                    pair[1]
                } else {
                    pair[0] + vector * (end / length)
                });
            }
            used = end;
            remaining -= amount;
            if remaining <= 0.0 {
                if !points.is_empty() {
                    reserve(&mut output, 1)?;
                    output.push(Contour {
                        points: std::mem::take(&mut points),
                        closed: false,
                    });
                }
                index = (index + 1) % pattern.len();
                remaining = pattern[index];
            }
        }
    }
    if !points.is_empty() {
        reserve(&mut output, 1)?;
        output.push(Contour {
            points,
            closed: false,
        });
    }
    let reaches_seam = output
        .last()
        .is_some_and(|last| last.points.last() == contour.points.last());
    if contour.closed && initially_on && index % 2 == 0 && reaches_seam {
        if output.len() == 1 {
            output[0].closed = true;
        } else {
            let first = output.remove(0);
            let last = output.last_mut().expect("at least one remaining dash");
            reserve(&mut last.points, first.points.len().saturating_sub(1))?;
            last.points.extend(first.points.into_iter().skip(1));
        }
    }
    Ok(output)
}

fn arc(
    center: Point,
    normal: Vec2,
    angle: f64,
    output: &mut Vec<PathEl>,
    budget: &mut Budget,
) -> Result<(), RenderFailure> {
    let start = normal.y.atan2(normal.x);
    let arc = Arc::new(center, (normal.hypot(), normal.hypot()), start, angle, 0.0);
    // Stop the iterator immediately when capacity or work admission fails.
    for element in arc.append_iter(budget.tolerance) {
        budget.element(output, element)?;
    }
    Ok(())
}

fn cap(
    center: Point,
    outward: Vec2,
    from: Vec2,
    cap: Cap,
    output: &mut Vec<PathEl>,
    budget: &mut Budget,
) -> Result<(), RenderFailure> {
    match cap {
        Cap::Butt => budget.element(output, PathEl::LineTo(center - from)),
        Cap::Round => arc(center, from, std::f64::consts::PI, output, budget),
        Cap::Square => {
            let extension = outward * from.hypot();
            budget.element(output, PathEl::LineTo(center + from + extension))?;
            budget.element(output, PathEl::LineTo(center - from + extension))?;
            budget.element(output, PathEl::LineTo(center - from))
        }
    }
}

fn join(
    center: Point,
    incoming: Vec2,
    outgoing: Vec2,
    stroke: &Style<'_>,
    forward: &mut Vec<PathEl>,
    backward: &mut Vec<PathEl>,
    budget: &mut Budget,
) -> Result<(), RenderFailure> {
    let radius = stroke.width * 0.5;
    let cross = incoming.cross(outgoing);
    let dot = incoming.dot(outgoing);
    if dot > 0.0 && cross.abs() < 2.0 * budget.tolerance / stroke.width {
        return Ok(());
    }
    let a = incoming.turn_90() * radius;
    let b = outgoing.turn_90() * radius;
    match stroke.join {
        Join::Round => {
            let angle = cross.atan2(dot);
            if angle > 0.0 {
                budget.element(backward, PathEl::LineTo(center + b))?;
                arc(center, -a, angle, forward, budget)
            } else {
                budget.element(forward, PathEl::LineTo(center - b))?;
                arc(center, a, angle, backward, budget)
            }
        }
        _ => {
            if stroke.join == Join::Miter
                && cross != 0.0
                && 2.0 < (1.0 + dot) * stroke.miter_limit.powi(2)
            {
                let sign = if cross > 0.0 { -1.0 } else { 1.0 };
                let offset = incoming.cross((b - a) * sign) / cross;
                let tip = center + b * sign - outgoing * offset;
                if cross > 0.0 {
                    budget.element(forward, PathEl::LineTo(tip))?;
                    budget.element(backward, PathEl::LineTo(center))?;
                } else {
                    budget.element(backward, PathEl::LineTo(tip))?;
                    budget.element(forward, PathEl::LineTo(center))?;
                }
            }
            budget.element(forward, PathEl::LineTo(center - b))?;
            budget.element(backward, PathEl::LineTo(center + b))
        }
    }
}

fn reverse(
    path: &[PathEl],
    output: &mut Vec<PathEl>,
    budget: &mut Budget,
) -> Result<(), RenderFailure> {
    for index in (1..path.len()).rev() {
        let end = path[index - 1]
            .end_point()
            .expect("stroke contour endpoint");
        let element = match path[index] {
            PathEl::LineTo(_) => PathEl::LineTo(end),
            PathEl::QuadTo(control, _) => PathEl::QuadTo(control, end),
            PathEl::CurveTo(a, b, _) => PathEl::CurveTo(b, a, end),
            _ => unreachable!("stroke side contains only segments"),
        };
        budget.element(output, element)?;
    }
    Ok(())
}

pub(super) fn expand(
    path: &BezPath,
    stroke: &Style<'_>,
    device_scale_bound: f64,
    limits: AntialiasWorkLimits,
    command_index: usize,
) -> Result<(BezPath, Work), RenderFailure> {
    let tolerance = DEVICE_PRECISION / device_scale_bound.max(1.0);
    let mut budget = Budget {
        work: Work::default(),
        limits,
        index: command_index,
        tolerance,
    };
    let mut output = Vec::new();
    if stroke.width == 0.0 {
        return Ok((BezPath::from_vec(output), budget.work));
    }
    let radius = stroke.width * 0.5;
    for contour in contours(path, radius, tolerance, &mut budget)? {
        for fragment in dash(contour, stroke, &mut budget)? {
            let points = &fragment.points;
            let mut forward = Vec::new();
            let mut backward = Vec::new();
            let mut previous = None;
            let mut first_direction = None;
            for pair in points.windows(2) {
                let delta = pair[1] - pair[0];
                if delta.hypot() == 0.0 {
                    continue;
                }
                let direction = delta.normalize();
                let normal = direction.turn_90() * radius;
                if let Some(incoming) = previous {
                    join(
                        pair[0],
                        incoming,
                        direction,
                        stroke,
                        &mut forward,
                        &mut backward,
                        &mut budget,
                    )?;
                } else {
                    budget.element(&mut forward, PathEl::MoveTo(pair[0] - normal))?;
                    budget.element(&mut backward, PathEl::MoveTo(pair[0] + normal))?;
                    first_direction = Some(direction);
                }
                budget.element(&mut forward, PathEl::LineTo(pair[1] - normal))?;
                budget.element(&mut backward, PathEl::LineTo(pair[1] + normal))?;
                previous = Some(direction);
            }
            let (Some(first), Some(last)) = (first_direction, previous) else {
                continue;
            };
            if fragment.closed {
                join(
                    points[0],
                    last,
                    first,
                    stroke,
                    &mut forward,
                    &mut backward,
                    &mut budget,
                )?;
            }
            for element in &forward {
                budget.element(&mut output, *element)?;
            }
            let end = *points.last().expect("nonempty contour");
            if fragment.closed {
                budget.element(&mut output, PathEl::ClosePath)?;
                budget.element(
                    &mut output,
                    PathEl::MoveTo(
                        backward
                            .last()
                            .expect("nonempty side")
                            .end_point()
                            .expect("endpoint"),
                    ),
                )?;
            } else {
                cap(
                    end,
                    last,
                    -last.turn_90() * radius,
                    stroke.end_cap,
                    &mut output,
                    &mut budget,
                )?;
            }
            reverse(&backward, &mut output, &mut budget)?;
            if !fragment.closed {
                cap(
                    points[0],
                    -first,
                    first.turn_90() * radius,
                    stroke.start_cap,
                    &mut output,
                    &mut budget,
                )?;
            }
            budget.element(&mut output, PathEl::ClosePath)?;
        }
    }
    Ok((BezPath::from_vec(output), budget.work))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::antialias::{AntialiasConfig, sample};
    use novadraw::graphics::FillRule;
    use vello::kurbo::{Shape, Stroke, StrokeOpts};

    fn view(stroke: &Stroke) -> Style<'_> {
        Style {
            width: stroke.width,
            join: stroke.join,
            start_cap: stroke.start_cap,
            end_cap: stroke.end_cap,
            miter_limit: stroke.miter_limit,
            dash_pattern: &stroke.dash_pattern,
            dash_offset: stroke.dash_offset,
        }
    }

    #[test]
    fn line_joins_caps_and_closed_dash_match_kurbo_reference() {
        let mut path = BezPath::new();
        path.move_to((4.17, 6.31));
        path.line_to((32.71, 12.13));
        path.line_to((17.19, 34.83));
        path.close_path();
        let limits = AntialiasConfig::default().operation;
        for join in [Join::Miter, Join::Round, Join::Bevel] {
            for cap in [Cap::Butt, Cap::Round, Cap::Square] {
                for dashed in [false, true] {
                    let mut stroke = Stroke::new(3.3).with_join(join).with_caps(cap);
                    if dashed {
                        stroke = stroke.with_dashes(1.7, [6.1, 3.9]);
                    }
                    let (bounded, _) = expand(&path, &view(&stroke), 1.0, limits, 0).unwrap();
                    let reference =
                        vello::kurbo::stroke(path.iter(), &stroke, &StrokeOpts::default(), 1e-4);
                    let (mask, _) = sample(&bounded, FillRule::NonZero, 48, 48, limits, 0).unwrap();
                    for y in 0..48 {
                        for x in 0..48 {
                            let point = Point::new(f64::from(x) + 0.5, f64::from(y) + 0.5);
                            assert_eq!(
                                mask.winding(point) != 0,
                                reference.winding(point) != 0,
                                "{join:?} {cap:?} dash={dashed} at {point:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn curve_expansion_is_bounded_and_preserves_sampled_stroke() {
        let path = vello::kurbo::Circle::new((23.17, 23.31), 14.13).to_path(1e-6);
        let limits = AntialiasConfig::default().operation;
        for dashed in [false, true] {
            let mut stroke = Stroke::new(2.3)
                .with_join(Join::Round)
                .with_caps(Cap::Round);
            if dashed {
                stroke = stroke.with_dashes(1.3, [4.7, 2.1]);
            }
            let (bounded, _) = expand(&path, &view(&stroke), 1.0, limits, 3).unwrap();
            let (mask, _) = sample(&bounded, FillRule::NonZero, 48, 48, limits, 3).unwrap();
            let reference =
                vello::kurbo::stroke(path.iter(), &stroke, &StrokeOpts::default(), 1e-4);
            for y in 0..48 {
                for x in 0..48 {
                    let point = Point::new(f64::from(x) + 0.5, f64::from(y) + 0.5);
                    assert_eq!(
                        mask.winding(point) != 0,
                        reference.winding(point) != 0,
                        "dash={dashed} {point:?}"
                    );
                }
            }
        }
        let stroke = Stroke::new(2.3).with_join(Join::Round);
        let tiny = AntialiasWorkLimits {
            segments: 2,
            ..limits
        };
        assert!(matches!(
            expand(&path, &view(&stroke), 1.0, tiny, 3),
            Err(RenderFailure::InvalidInput(_))
        ));
    }

    #[test]
    fn closed_dash_with_a_gap_ending_exactly_at_the_seam_keeps_open_caps() {
        let limits = AntialiasConfig::default().operation;
        let mut budget = Budget {
            work: Work::default(),
            limits,
            index: 0,
            tolerance: DEVICE_PRECISION,
        };
        let contour = Contour {
            points: vec![
                (0.0, 0.0).into(),
                (5.5, 0.0).into(),
                (5.5, 5.5).into(),
                (0.0, 5.5).into(),
                (0.0, 0.0).into(),
            ],
            closed: true,
        };
        let stroke = Stroke::new(1.0).with_dashes(0.0, [20.0, 2.0]);
        let pieces = dash(contour, &view(&stroke), &mut budget).unwrap();
        assert_eq!(pieces.len(), 1);
        assert!(!pieces[0].closed);
        assert_ne!(pieces[0].points.first(), pieces[0].points.last());
    }

    #[test]
    fn degenerate_tangents_reversals_and_zero_width_are_bounded() {
        let limits = AntialiasConfig::default().operation;
        let mut path = BezPath::new();
        path.move_to((8.17, 9.31));
        path.curve_to((8.17, 9.31), (40.19, 9.31), (8.17, 9.31));
        path.quad_to((8.17, 9.31), (8.17, 9.31));
        path.line_to((8.17, 9.31));
        for cap in [Cap::Butt, Cap::Round, Cap::Square] {
            let stroke = Stroke::new(2.3).with_caps(cap).with_join(Join::Round);
            let (bounded, work) = expand(&path, &view(&stroke), 2.0, limits, 3).unwrap();
            assert!(bounded.is_finite());
            assert!(work.segments < limits.segments);
            let reference =
                vello::kurbo::stroke(path.iter(), &stroke, &StrokeOpts::default(), 1e-5);
            for y in 0..48 {
                for x in 0..48 {
                    let p = Point::new(f64::from(x) + 0.5, f64::from(y) + 0.5);
                    assert_eq!(
                        bounded.winding(p) != 0,
                        reference.winding(p) != 0,
                        "{cap:?} {p:?}"
                    );
                }
            }
        }
        let (empty, work) = expand(&path, &view(&Stroke::new(0.0)), 2.0, limits, 0).unwrap();
        assert!(empty.elements().is_empty());
        assert_eq!(work.segments, 0);
    }
}
