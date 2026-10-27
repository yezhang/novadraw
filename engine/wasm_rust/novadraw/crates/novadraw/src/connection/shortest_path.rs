use std::{cmp::Ordering, collections::BinaryHeap, error::Error, fmt};

use crate::FigureId;
use crate::geometry::{ApproxEq, Point, PointList, Precision, Rectangle, Vec2};

use super::router::{remove_adjacent_duplicates, resolve_endpoints};
use super::{ConnectionRouter, RouteError, RouteOutput, RouteRequest, RoutingGroupScope};

/// Default clearance between a shortest-path route and an obstacle.
pub const SHORTEST_PATH_DEFAULT_CLEARANCE: f64 = 8.0;
/// Default additive cost for every direction change.
pub const SHORTEST_PATH_DEFAULT_BEND_PENALTY: f64 = 4.0;
/// Default outward endpoint stub length.
pub const SHORTEST_PATH_DEFAULT_MINIMUM_STUB: f64 = 10.0;

/// Deterministic orthogonal Router over an immutable obstacle snapshot.
#[derive(Clone, Debug)]
pub struct ShortestPathConnectionRouter {
    obstacles: Vec<FigureId>,
    clearance: f64,
    bend_penalty: f64,
    minimum_stub: f64,
}

impl ShortestPathConnectionRouter {
    /// Creates a Router from an ordered set of obstacle Figure identities.
    pub fn new(obstacles: impl IntoIterator<Item = FigureId>) -> Self {
        let mut unique = Vec::new();
        for obstacle in obstacles {
            if !unique.contains(&obstacle) {
                unique.push(obstacle);
            }
        }
        Self {
            obstacles: unique,
            clearance: SHORTEST_PATH_DEFAULT_CLEARANCE,
            bend_penalty: SHORTEST_PATH_DEFAULT_BEND_PENALTY,
            minimum_stub: SHORTEST_PATH_DEFAULT_MINIMUM_STUB,
        }
    }

    /// Replaces route clearance.
    pub fn with_clearance(mut self, clearance: f64) -> Result<Self, ShortestPathRouterError> {
        validate_non_negative(clearance)?;
        self.clearance = clearance;
        Ok(self)
    }

    /// Replaces the additive bend penalty.
    pub fn with_bend_penalty(mut self, bend_penalty: f64) -> Result<Self, ShortestPathRouterError> {
        validate_non_negative(bend_penalty)?;
        self.bend_penalty = bend_penalty;
        Ok(self)
    }

    /// Replaces the outward endpoint stub length.
    pub fn with_minimum_stub(mut self, minimum_stub: f64) -> Result<Self, ShortestPathRouterError> {
        validate_non_negative(minimum_stub)?;
        self.minimum_stub = minimum_stub;
        Ok(self)
    }
}

impl ConnectionRouter for ShortestPathConnectionRouter {
    fn route(&self, request: RouteRequest<'_>) -> Result<RouteOutput, RouteError> {
        if let Some(constraint) = request.constraint {
            return Err(RouteError::ConstraintTypeMismatch {
                expected: None,
                actual: constraint.type_name(),
            });
        }
        let group = request.group.ok_or(RouteError::UnsupportedRoutingGroup)?;
        let metadata = resolve_endpoints(
            request.source,
            request.target,
            request.scene,
            request.routing_space,
            None,
            None,
        )?;
        let excluded = [
            request.connection.figure(),
            request
                .source
                .owner()
                .unwrap_or(request.connection.figure()),
            request
                .target
                .owner()
                .unwrap_or(request.connection.figure()),
        ];
        let obstacles = group
            .obstacles()
            .iter()
            .filter(|obstacle| !excluded.contains(&obstacle.figure))
            .map(|obstacle| obstacle.bounds.inflate(self.clearance, self.clearance))
            .collect::<Vec<_>>();

        let source = metadata.source.site.point;
        let target = metadata.target.site.point;
        let source_stub = terminal_stub(
            source,
            metadata.source.site.outward_normal,
            self.minimum_stub,
        );
        let target_stub = terminal_stub(
            target,
            metadata.target.site.outward_normal,
            self.minimum_stub,
        );
        if obstacles.iter().any(|obstacle| {
            point_inside(*obstacle, source_stub) || point_inside(*obstacle, target_stub)
        }) || !segment_is_clear(source, source_stub, &obstacles)
            || !segment_is_clear(target_stub, target, &obstacles)
        {
            return Err(RouteError::NoObstacleFreePath);
        }

        let mut points = vec![source];
        if !source.approx_eq(source_stub, Precision::DEFAULT) {
            points.push(source_stub);
        }
        let path =
            shortest_orthogonal_path(source_stub, target_stub, &obstacles, self.bend_penalty)
                .ok_or(RouteError::NoObstacleFreePath)?;
        points.extend(path.into_iter().skip(1));
        if !target_stub.approx_eq(target, Precision::DEFAULT) {
            points.push(target);
        }
        remove_adjacent_duplicates(&mut points);
        remove_collinear_points(&mut points);
        RouteOutput::new(PointList::from_points(points), metadata)
    }

    fn requires_group(&self) -> bool {
        true
    }

    fn routing_group_scope(&self) -> RoutingGroupScope {
        RoutingGroupScope::RoutingDomain
    }

    fn obstacle_figures(&self) -> &[FigureId] {
        &self.obstacles
    }
}

/// Failure to configure shortest-path routing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShortestPathRouterError;

impl fmt::Display for ShortestPathRouterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("shortest-path metrics must be finite and non-negative")
    }
}

impl Error for ShortestPathRouterError {}

fn shortest_orthogonal_path(
    source: Point,
    target: Point,
    obstacles: &[Rectangle],
    bend_penalty: f64,
) -> Option<Vec<Point>> {
    if source.approx_eq(target, Precision::DEFAULT) {
        return None;
    }
    if let Some(path) = simple_orthogonal_path(source, target, obstacles) {
        return Some(path);
    }

    let mut xs = vec![source.x(), target.x()];
    let mut ys = vec![source.y(), target.y()];
    for obstacle in obstacles {
        xs.extend([obstacle.x, obstacle.x + obstacle.width]);
        ys.extend([obstacle.y, obstacle.y + obstacle.height]);
    }
    sort_and_deduplicate(&mut xs);
    sort_and_deduplicate(&mut ys);

    let width = xs.len();
    let height = ys.len();
    let mut valid = vec![false; width * height];
    for (y_index, y) in ys.iter().copied().enumerate() {
        for (x_index, x) in xs.iter().copied().enumerate() {
            valid[y_index * width + x_index] = !obstacles
                .iter()
                .any(|obstacle| point_inside(*obstacle, Point::new(x, y)));
        }
    }
    let start = grid_index(source, &xs, &ys)?;
    let goal = grid_index(target, &xs, &ys)?;
    if !valid[start] || !valid[goal] {
        return None;
    }

    let state_count = valid.len() * Axis::COUNT;
    let mut costs = vec![f64::INFINITY; state_count];
    let mut bends = vec![usize::MAX; state_count];
    let mut previous = vec![None; state_count];
    let start_state = state_index(start, Axis::None);
    costs[start_state] = 0.0;
    bends[start_state] = 0;
    let mut queue = BinaryHeap::new();
    queue.push(QueueEntry {
        cost: 0.0,
        bends: 0,
        state: start_state,
    });

    while let Some(entry) = queue.pop() {
        if entry.cost > costs[entry.state] || entry.bends > bends[entry.state] {
            continue;
        }
        let node = entry.state / Axis::COUNT;
        let incoming = Axis::from_index(entry.state % Axis::COUNT);
        let x_index = node % width;
        let y_index = node / width;
        for (neighbor, axis) in grid_neighbors(x_index, y_index, width, height) {
            if !valid[neighbor] {
                continue;
            }
            let start_point = Point::new(xs[x_index], ys[y_index]);
            let neighbor_x = neighbor % width;
            let neighbor_y = neighbor / width;
            let end_point = Point::new(xs[neighbor_x], ys[neighbor_y]);
            if !segment_is_clear(start_point, end_point, obstacles) {
                continue;
            }
            let bend = incoming != Axis::None && incoming != axis;
            let next_bends = entry.bends + usize::from(bend);
            let next_cost = entry.cost
                + (end_point - start_point).length()
                + if bend { bend_penalty } else { 0.0 };
            let next_state = state_index(neighbor, axis);
            let better = next_cost < costs[next_state]
                || (next_cost == costs[next_state] && next_bends < bends[next_state]);
            if better {
                costs[next_state] = next_cost;
                bends[next_state] = next_bends;
                previous[next_state] = Some(entry.state);
                queue.push(QueueEntry {
                    cost: next_cost,
                    bends: next_bends,
                    state: next_state,
                });
            }
        }
    }

    let goal_state = [Axis::None, Axis::Horizontal, Axis::Vertical]
        .into_iter()
        .map(|axis| state_index(goal, axis))
        .filter(|state| costs[*state].is_finite())
        .min_by(|left, right| {
            costs[*left]
                .total_cmp(&costs[*right])
                .then_with(|| bends[*left].cmp(&bends[*right]))
                .then_with(|| left.cmp(right))
        })?;
    let mut nodes = Vec::new();
    let mut current = Some(goal_state);
    while let Some(state) = current {
        nodes.push(state / Axis::COUNT);
        current = previous[state];
    }
    nodes.reverse();
    let mut path = nodes
        .into_iter()
        .map(|node| Point::new(xs[node % width], ys[node / width]))
        .collect();
    remove_collinear_points(&mut path);
    Some(path)
}

fn simple_orthogonal_path(
    source: Point,
    target: Point,
    obstacles: &[Rectangle],
) -> Option<Vec<Point>> {
    if source.x().approx_eq(target.x(), Precision::DEFAULT)
        || source.y().approx_eq(target.y(), Precision::DEFAULT)
    {
        return segment_is_clear(source, target, obstacles).then(|| vec![source, target]);
    }
    let horizontal_first = Point::new(target.x(), source.y());
    if segment_is_clear(source, horizontal_first, obstacles)
        && segment_is_clear(horizontal_first, target, obstacles)
        && !obstacles
            .iter()
            .any(|obstacle| point_inside(*obstacle, horizontal_first))
    {
        return Some(vec![source, horizontal_first, target]);
    }
    let vertical_first = Point::new(source.x(), target.y());
    if segment_is_clear(source, vertical_first, obstacles)
        && segment_is_clear(vertical_first, target, obstacles)
        && !obstacles
            .iter()
            .any(|obstacle| point_inside(*obstacle, vertical_first))
    {
        return Some(vec![source, vertical_first, target]);
    }
    None
}

fn grid_neighbors(
    x: usize,
    y: usize,
    width: usize,
    height: usize,
) -> impl Iterator<Item = (usize, Axis)> {
    let horizontal = [
        x.checked_sub(1)
            .map(|next| (y * width + next, Axis::Horizontal)),
        (x + 1 < width).then(|| (y * width + x + 1, Axis::Horizontal)),
    ];
    let vertical = [
        y.checked_sub(1)
            .map(|next| (next * width + x, Axis::Vertical)),
        (y + 1 < height).then(|| ((y + 1) * width + x, Axis::Vertical)),
    ];
    horizontal.into_iter().chain(vertical).flatten()
}

fn grid_index(point: Point, xs: &[f64], ys: &[f64]) -> Option<usize> {
    let x = xs
        .iter()
        .position(|candidate| candidate.approx_eq(point.x(), Precision::DEFAULT))?;
    let y = ys
        .iter()
        .position(|candidate| candidate.approx_eq(point.y(), Precision::DEFAULT))?;
    Some(y * xs.len() + x)
}

fn segment_is_clear(start: Point, end: Point, obstacles: &[Rectangle]) -> bool {
    obstacles.iter().all(|obstacle| {
        if start.y().approx_eq(end.y(), Precision::DEFAULT) {
            let y = start.y();
            let segment_min = start.x().min(end.x());
            let segment_max = start.x().max(end.x());
            !(strictly_between(y, obstacle.y, obstacle.y + obstacle.height)
                && intervals_overlap_interior(
                    segment_min,
                    segment_max,
                    obstacle.x,
                    obstacle.x + obstacle.width,
                ))
        } else {
            let x = start.x();
            let segment_min = start.y().min(end.y());
            let segment_max = start.y().max(end.y());
            !(strictly_between(x, obstacle.x, obstacle.x + obstacle.width)
                && intervals_overlap_interior(
                    segment_min,
                    segment_max,
                    obstacle.y,
                    obstacle.y + obstacle.height,
                ))
        }
    })
}

fn point_inside(rectangle: Rectangle, point: Point) -> bool {
    strictly_between(point.x(), rectangle.x, rectangle.x + rectangle.width)
        && strictly_between(point.y(), rectangle.y, rectangle.y + rectangle.height)
}

fn strictly_between(value: f64, minimum: f64, maximum: f64) -> bool {
    value > minimum + Precision::DEFAULT.epsilon() && value < maximum - Precision::DEFAULT.epsilon()
}

fn intervals_overlap_interior(
    left_min: f64,
    left_max: f64,
    right_min: f64,
    right_max: f64,
) -> bool {
    left_max > right_min + Precision::DEFAULT.epsilon()
        && left_min < right_max - Precision::DEFAULT.epsilon()
}

fn terminal_stub(point: Point, normal: Option<Vec2>, minimum_stub: f64) -> Point {
    let Some(normal) = normal else {
        return point;
    };
    if !normal.x().is_finite() || !normal.y().is_finite() || normal.length_squared() <= f64::EPSILON
    {
        point
    } else if normal.x().abs() >= normal.y().abs() {
        point + Vec2::new(normal.x().signum() * minimum_stub, 0.0)
    } else {
        point + Vec2::new(0.0, normal.y().signum() * minimum_stub)
    }
}

fn remove_collinear_points(points: &mut Vec<Point>) {
    let mut index = 1;
    while index + 1 < points.len() {
        let incoming = points[index] - points[index - 1];
        let outgoing = points[index + 1] - points[index];
        if incoming.cross(outgoing).abs() <= Precision::DEFAULT.epsilon() {
            points.remove(index);
        } else {
            index += 1;
        }
    }
}

fn sort_and_deduplicate(values: &mut Vec<f64>) {
    values.sort_by(|left, right| left.total_cmp(right));
    values.dedup_by(|left, right| left.approx_eq(*right, Precision::DEFAULT));
}

fn validate_non_negative(value: f64) -> Result<(), ShortestPathRouterError> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(ShortestPathRouterError)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Axis {
    None,
    Horizontal,
    Vertical,
}

impl Axis {
    const COUNT: usize = 3;

    const fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Horizontal,
            2 => Self::Vertical,
            _ => Self::None,
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::None => 0,
            Self::Horizontal => 1,
            Self::Vertical => 2,
        }
    }
}

fn state_index(node: usize, axis: Axis) -> usize {
    node * Axis::COUNT + axis.index()
}

#[derive(Clone, Copy, Debug)]
struct QueueEntry {
    cost: f64,
    bends: usize,
    state: usize,
}

impl PartialEq for QueueEntry {
    fn eq(&self, other: &Self) -> bool {
        self.cost == other.cost && self.bends == other.bends && self.state == other.state
    }
}

impl Eq for QueueEntry {}

impl PartialOrd for QueueEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for QueueEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .cost
            .total_cmp(&self.cost)
            .then_with(|| other.bends.cmp(&self.bends))
            .then_with(|| other.state.cmp(&self.state))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortest_path_uses_obstacle_boundary_and_removes_collinear_points() {
        let obstacle = Rectangle::new(40.0, -10.0, 20.0, 20.0);
        let path = shortest_orthogonal_path(Point::ZERO, Point::new(100.0, 0.0), &[obstacle], 4.0)
            .unwrap();
        assert_eq!(
            path,
            vec![
                Point::ZERO,
                Point::new(0.0, -10.0),
                Point::new(100.0, -10.0),
                Point::new(100.0, 0.0),
            ]
        );
        assert!(path.windows(2).all(|segment| segment_is_clear(
            segment[0],
            segment[1],
            &[obstacle]
        )));
    }

    #[test]
    fn configuration_rejects_negative_and_non_finite_metrics() {
        assert_eq!(
            ShortestPathConnectionRouter::new([])
                .with_clearance(-1.0)
                .unwrap_err(),
            ShortestPathRouterError
        );
        assert_eq!(
            ShortestPathConnectionRouter::new([])
                .with_bend_penalty(f64::NAN)
                .unwrap_err(),
            ShortestPathRouterError
        );
    }
}
