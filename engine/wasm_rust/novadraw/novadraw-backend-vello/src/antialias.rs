//! Bounded device-center coverage. Geometry is independent of paint and clip.

use novadraw::graphics::FillRule;
use novadraw::render::{
    GraphicsInputError, InvalidGraphicsInput, RenderFailure, RenderFailureDiagnostic,
};
use vello::kurbo::{BezPath, Line, ParamCurve, ParamCurveExtrema, PathEl, PathSeg, Point};

/// Limits apply independently to each operation and to all operations in a frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AntialiasWorkLimits {
    pub rows: usize,
    pub segments: usize,
    pub crossings: usize,
    pub runs: usize,
    pub elements: usize,
}

/// Session-owned limits for CPU preparation of `AntialiasMode::Off`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AntialiasConfig {
    pub operation: AntialiasWorkLimits,
    pub frame: AntialiasWorkLimits,
    pub cache_bytes: usize,
    /// Maximum scratch allocation for one font outline.
    pub glyph_scratch_bytes: usize,
}

// Finite session defaults; measured consumer costs are recorded in the ADR-047
// implementation review. Continuous high-DPI changes need a consumer latency budget.
impl Default for AntialiasConfig {
    fn default() -> Self {
        Self {
            operation: AntialiasWorkLimits {
                rows: 8_192,
                segments: 65_536,
                crossings: 4_194_304,
                runs: 524_288,
                elements: 2_621_440,
            },
            frame: AntialiasWorkLimits {
                rows: 262_144,
                segments: 1_048_576,
                crossings: 16_777_216,
                runs: 2_097_152,
                elements: 10_485_760,
            },
            cache_bytes: 32 * 1_024 * 1_024,
            glyph_scratch_bytes: 8 * 1_024 * 1_024,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Work {
    pub rows: usize,
    pub segments: usize,
    pub crossings: usize,
    pub runs: usize,
    pub elements: usize,
}

impl Work {
    pub fn remaining(self, limits: AntialiasWorkLimits) -> AntialiasWorkLimits {
        AntialiasWorkLimits {
            rows: limits.rows.saturating_sub(self.rows),
            segments: limits.segments.saturating_sub(self.segments),
            crossings: limits.crossings.saturating_sub(self.crossings),
            runs: limits.runs.saturating_sub(self.runs),
            elements: limits.elements.saturating_sub(self.elements),
        }
    }

    pub fn add(
        &mut self,
        amount: Self,
        limits: AntialiasWorkLimits,
        command_index: usize,
    ) -> Result<(), RenderFailure> {
        let candidates = [
            ("rows", self.rows, amount.rows, limits.rows),
            ("segments", self.segments, amount.segments, limits.segments),
            (
                "crossings",
                self.crossings,
                amount.crossings,
                limits.crossings,
            ),
            ("runs", self.runs, amount.runs, limits.runs),
            ("elements", self.elements, amount.elements, limits.elements),
        ];
        for (kind, used, additional, limit) in candidates {
            let requested = used.checked_add(additional);
            if requested.is_none_or(|value| value > limit) {
                return Err(limit_failure(
                    command_index,
                    kind,
                    requested.unwrap_or(usize::MAX),
                    limit,
                ));
            }
        }
        self.rows += amount.rows;
        self.segments += amount.segments;
        self.crossings += amount.crossings;
        self.runs += amount.runs;
        self.elements += amount.elements;
        Ok(())
    }
}

pub(super) fn limit_failure(
    command_index: usize,
    kind: &'static str,
    requested: usize,
    limit: usize,
) -> RenderFailure {
    RenderFailure::InvalidInput(InvalidGraphicsInput {
        command_index,
        reason: GraphicsInputError::AntialiasPreparationLimit {
            kind,
            requested,
            limit,
        },
    })
}

pub(super) fn reserve<T>(values: &mut Vec<T>, additional: usize) -> Result<(), RenderFailure> {
    values
        .try_reserve(additional)
        .map_err(|error| RenderFailure::OutOfMemory(RenderFailureDiagnostic::from_error(&error)))
}

#[derive(Clone, Copy)]
struct Edge {
    segment: PathSeg,
    first: u32,
    end: u32,
    direction: i32,
}

// Left/bottom boundaries are included; right/top boundaries are excluded.
// A crossing at x owns centers x <= center on its right side.
fn center_bound(value: f64, limit: u32) -> u32 {
    (value - 0.5).ceil().clamp(0.0, f64::from(limit)) as u32
}

fn crossing(segment: PathSeg, y: f64) -> f64 {
    if let PathSeg::Line(line) = segment {
        return line.p0.x + (y - line.p0.y) / (line.p1.y - line.p0.y) * (line.p1.x - line.p0.x);
    }
    let increasing = segment.end().y > segment.start().y;
    let mut low = 0.0;
    let mut high = 1.0;
    // Bisection on a monotonic polynomial does not flatten or change the mask.
    for _ in 0..64 {
        let middle = (low + high) * 0.5;
        if (segment.eval(middle).y < y) == increasing {
            low = middle;
        } else {
            high = middle;
        }
    }
    segment.eval((low + high) * 0.5).x
}

fn push_edge(
    segment: PathSeg,
    height: u32,
    edges: &mut Vec<Edge>,
    work: &mut Work,
    limits: AntialiasWorkLimits,
    command_index: usize,
) -> Result<(), RenderFailure> {
    for range in segment.extrema_ranges() {
        let part = segment.subsegment(range);
        let start = part.start().y;
        let end = part.end().y;
        if start == end {
            continue;
        }
        let first = center_bound(start.min(end), height);
        let last = center_bound(start.max(end), height);
        if first == last {
            continue;
        }
        work.add(
            Work {
                segments: 1,
                ..Work::default()
            },
            limits,
            command_index,
        )?;
        reserve(edges, 1)?;
        edges.push(Edge {
            segment: part,
            first,
            end: last,
            direction: if end > start { 1 } else { -1 },
        });
    }
    Ok(())
}

/// The original outline must be retained: offscreen x crossings affect winding.
/// Open fill contours close implicitly, as in the ordinary Vello fill path.
pub(super) fn sample(
    path: &BezPath,
    rule: FillRule,
    width: u32,
    height: u32,
    limits: AntialiasWorkLimits,
    command_index: usize,
) -> Result<(BezPath, Work), RenderFailure> {
    let mut work = Work::default();
    let mut edges = Vec::new();
    let mut first = Point::ZERO;
    let mut last = Point::ZERO;
    let mut open = false;
    for element in path.elements() {
        let segment = match *element {
            PathEl::MoveTo(point) => {
                if open && first != last {
                    push_edge(
                        PathSeg::Line(Line::new(last, first)),
                        height,
                        &mut edges,
                        &mut work,
                        limits,
                        command_index,
                    )?;
                }
                first = point;
                last = point;
                open = true;
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
                open = false;
                Some(PathSeg::Line(Line::new(last, first)))
            }
        };
        if let Some(segment) = segment {
            last = segment.end();
            push_edge(
                segment,
                height,
                &mut edges,
                &mut work,
                limits,
                command_index,
            )?;
        }
    }
    if open && first != last {
        push_edge(
            PathSeg::Line(Line::new(last, first)),
            height,
            &mut edges,
            &mut work,
            limits,
            command_index,
        )?;
    }
    edges.sort_unstable_by_key(|edge| edge.first);
    let mut active: Vec<Edge> = Vec::new();
    let mut crossings: Vec<(f64, i32)> = Vec::new();
    let mut output = Vec::new();
    let mut next = 0;
    let mut row = edges.first().map_or(height, |edge| edge.first);
    while row < height {
        active.retain(|edge| edge.end > row);
        while next < edges.len() && edges[next].first <= row {
            reserve(&mut active, 1)?;
            active.push(edges[next]);
            next += 1;
        }
        if active.is_empty() {
            if next == edges.len() {
                break;
            }
            row = edges[next].first;
            continue;
        }
        work.add(
            Work {
                rows: 1,
                crossings: active.len(),
                ..Work::default()
            },
            limits,
            command_index,
        )?;
        crossings.clear();
        reserve(&mut crossings, active.len())?;
        let y = f64::from(row) + 0.5;
        crossings.extend(
            active
                .iter()
                .map(|edge| (crossing(edge.segment, y), edge.direction)),
        );
        crossings.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
        let mut winding: i64 = 0;
        let mut start = 0;
        for (x, direction) in &crossings {
            let inside_before = match rule {
                FillRule::NonZero => winding != 0,
                FillRule::EvenOdd => winding & 1 != 0,
            };
            winding += i64::from(*direction);
            let inside_after = match rule {
                FillRule::NonZero => winding != 0,
                FillRule::EvenOdd => winding & 1 != 0,
            };
            let bound = center_bound(*x, width);
            if !inside_before && inside_after {
                start = bound;
            } else if inside_before && !inside_after && bound > start {
                work.add(
                    Work {
                        runs: 1,
                        elements: 5,
                        ..Work::default()
                    },
                    limits,
                    command_index,
                )?;
                reserve(&mut output, 5)?;
                let x0 = f64::from(start);
                let x1 = f64::from(bound);
                let y0 = f64::from(row);
                let y1 = y0 + 1.0;
                output.extend([
                    PathEl::MoveTo((x0, y0).into()),
                    PathEl::LineTo((x1, y0).into()),
                    PathEl::LineTo((x1, y1).into()),
                    PathEl::LineTo((x0, y1).into()),
                    PathEl::ClosePath,
                ]);
            }
        }
        row += 1;
    }
    Ok((BezPath::from_vec(output), work))
}

#[cfg(test)]
mod tests {
    use super::*;
    use vello::kurbo::{Affine, Circle, Shape};

    fn compare(path: BezPath, rule: FillRule) {
        let (mask, _) =
            sample(&path, rule, 48, 48, AntialiasConfig::default().operation, 7).unwrap();
        for y in 0..48 {
            for x in 0..48 {
                let point = Point::new(f64::from(x) + 0.5, f64::from(y) + 0.5);
                let winding = path.winding(point);
                let expected = match rule {
                    FillRule::NonZero => winding != 0,
                    FillRule::EvenOdd => winding & 1 != 0,
                };
                assert_eq!(mask.winding(point) != 0, expected, "at {point:?}");
            }
        }
    }

    #[test]
    fn curved_transformed_mask_matches_independent_winding() {
        let circle = Circle::new((16.1, 15.2), 12.3).to_path(1e-6);
        let transform = Affine::new([1.2, 0.3, -0.4, 0.8, 1.7, 3.3]);
        compare(transform * circle, FillRule::NonZero);
    }

    #[test]
    fn holes_self_intersections_and_offsurface_edges() {
        let mut path = vello::kurbo::Rect::new(-20.2, -10.3, 42.4, 41.7).to_path(0.1);
        path.extend(vello::kurbo::Rect::new(12.1, 11.2, 29.3, 28.4).path_elements(0.1));
        compare(path.clone(), FillRule::EvenOdd);
        compare(path, FillRule::NonZero);
        let mut bow = BezPath::new();
        bow.move_to((1.1, 2.2));
        bow.line_to((40.3, 43.4));
        bow.line_to((2.1, 42.2));
        bow.line_to((42.3, 1.4));
        bow.close_path();
        compare(bow, FillRule::EvenOdd);
    }

    #[test]
    fn exact_half_open_centers_and_implicit_close() {
        let mut path = BezPath::new();
        path.move_to((0.5, 0.5));
        path.line_to((2.5, 0.5));
        path.line_to((2.5, 2.5));
        path.line_to((0.5, 2.5));
        let (mask, work) = sample(
            &path,
            FillRule::NonZero,
            8,
            8,
            AntialiasConfig::default().operation,
            0,
        )
        .unwrap();
        assert_eq!(work.runs, 2);
        assert_ne!(mask.winding((0.5, 0.5).into()), 0);
        assert_eq!(mask.winding((2.5, 0.5).into()), 0);
        assert_eq!(mask.winding((0.5, 2.5).into()), 0);
    }

    #[test]
    fn rejects_before_exceeding_output_budget() {
        let path = Circle::new((16.0, 16.0), 12.0).to_path(1e-6);
        let limits = AntialiasWorkLimits {
            elements: 4,
            ..AntialiasConfig::default().operation
        };
        let error = sample(&path, FillRule::NonZero, 48, 48, limits, 19).unwrap_err();
        assert!(matches!(
            error,
            RenderFailure::InvalidInput(InvalidGraphicsInput {
                command_index: 19,
                reason: GraphicsInputError::AntialiasPreparationLimit {
                    kind: "elements",
                    requested: 5,
                    limit: 4
                },
            })
        ));
    }
}
