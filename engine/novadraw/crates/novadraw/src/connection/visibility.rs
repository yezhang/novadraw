// Visibility and shared-corner routing adapted from Eclipse Draw2D graph
// Path, ShortestPathRouter, Vertex and Segment (IBM Corporation and others).
// SPDX-License-Identifier: EPL-2.0

use super::{
    ConnectionRouter, RouteError, RouteGroupInput, RouteGroupOutput, RouteMetadata, RouteOutput,
    RouteRequest, RoutingGroupScope,
};
use crate::{
    FigureId,
    geometry::{Point as GeometryPoint, PointList, Rectangle},
};

/// Default separation of routes sharing an obstacle corner.
pub const VISIBILITY_DEFAULT_SPACING: u32 = 4;
const GROW_PASSES: usize = 2;
const PIXEL_EDGE: f64 = 1.0;

#[derive(Clone, Copy)]
struct Point {
    x: f64,
    y: f64,
}
impl Point {
    fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}
impl From<GeometryPoint> for Point {
    fn from(point: GeometryPoint) -> Self {
        Self::new(point.x(), point.y())
    }
}
impl From<Point> for GeometryPoint {
    fn from(point: Point) -> Self {
        Self::new(point.x, point.y)
    }
}

/// Euclidean visibility routing with group-wide shared-corner ordering.
///
/// Obstacles use Draw2D's local right/bottom-minus-one corner convention.
/// All workspaces are local to the invocation; the router retains only obstacle IDs.
#[derive(Clone, Debug)]
pub struct VisibilityGraphConnectionRouter {
    obstacles: Vec<FigureId>,
    spacing: u32,
}

impl VisibilityGraphConnectionRouter {
    /// Creates a router retaining the supplied stable obstacle order.
    pub fn new(obstacles: impl IntoIterator<Item = FigureId>) -> Self {
        let mut unique = Vec::new();
        for obstacle in obstacles {
            if !unique.contains(&obstacle) {
                unique.push(obstacle);
            }
        }
        Self {
            obstacles: unique,
            spacing: VISIBILITY_DEFAULT_SPACING,
        }
    }

    /// Sets the integer spacing used by the Draw2D shared-corner protocol.
    pub const fn with_spacing(mut self, spacing: u32) -> Self {
        self.spacing = spacing;
        self
    }
}

impl ConnectionRouter for VisibilityGraphConnectionRouter {
    fn route(&self, request: RouteRequest<'_>) -> Result<RouteOutput, RouteError> {
        if let Some(constraint) = request.constraint {
            return Err(RouteError::ConstraintTypeMismatch {
                expected: None,
                actual: constraint.type_name(),
            });
        }
        let group = request.group.ok_or(RouteError::UnsupportedRoutingGroup)?;
        let metadata = super::router::resolve_endpoints(
            request.source,
            request.target,
            request.scene,
            request.routing_space,
            None,
            None,
        )?;
        let bounds = group
            .obstacles()
            .iter()
            .map(|obstacle| obstacle.bounds)
            .collect::<Vec<_>>();
        let points = Workspace::solve(&bounds, &[metadata], self.spacing)?.remove(0);
        RouteOutput::new(
            PointList::from_points(
                points
                    .into_iter()
                    .map(GeometryPoint::from)
                    .collect::<Vec<_>>(),
            ),
            metadata,
        )
    }
    fn prepares_group_endpoints(&self) -> bool {
        true
    }
    fn routing_group_scope(&self) -> RoutingGroupScope {
        RoutingGroupScope::RoutingDomain
    }
    fn obstacle_figures(&self) -> &[FigureId] {
        &self.obstacles
    }
    fn solve_group(
        &self,
        input: RouteGroupInput<'_>,
    ) -> Result<Option<RouteGroupOutput>, RouteError> {
        for member in input.members() {
            if let Some(constraint) = member.constraint() {
                return Err(RouteError::ConstraintTypeMismatch {
                    expected: None,
                    actual: constraint.type_name(),
                });
            }
        }
        let metadata = input
            .members()
            .iter()
            .map(|member| *member.metadata())
            .collect::<Vec<_>>();
        let bounds = input
            .obstacles()
            .iter()
            .map(|obstacle| obstacle.bounds)
            .collect::<Vec<_>>();
        let points = Workspace::solve(&bounds, &metadata, self.spacing)?;
        let routes = input
            .members()
            .iter()
            .zip(points)
            .map(|(member, points)| {
                Ok((
                    member.connection(),
                    RouteOutput::new(
                        PointList::from_points(
                            points
                                .into_iter()
                                .map(GeometryPoint::from)
                                .collect::<Vec<_>>(),
                        ),
                        *member.metadata(),
                    )?,
                ))
            })
            .collect::<Result<Vec<_>, RouteError>>()?;
        Ok(Some(RouteGroupOutput::new(routes)))
    }
}

// Corner indices follow the clockwise perimeter: TL, TR, BR, BL.
const TL: usize = 0;
const TR: usize = 1;
const BR: usize = 2;
const BL: usize = 3;

#[derive(Clone, Copy, PartialEq)]
enum Turn {
    Unset,
    Innie,
    Outie,
}
impl Turn {
    fn invert(self) -> Self {
        match self {
            Self::Innie => Self::Outie,
            Self::Outie => Self::Innie,
            Self::Unset => Self::Unset,
        }
    }
}

struct Vertex {
    original: Point,
    point: Point,
    obstacle: Option<usize>,
    direction: Point,
    total: usize,
    count: usize,
    nearest: f64,
    checked: bool,
    offset: f64,
    turn: Turn,
    paths: Vec<(usize, f64)>,
}
impl Vertex {
    fn endpoint(point: Point) -> Self {
        Self {
            original: point,
            point,
            obstacle: None,
            direction: Point::new(0.0, 0.0),
            total: 0,
            count: 0,
            nearest: 0.0,
            checked: false,
            offset: 0.0,
            turn: Turn::Unset,
            paths: Vec::new(),
        }
    }
    fn bend(&self, modifier: usize) -> Point {
        Point::new(
            self.point.x + self.direction.x * modifier as f64 * self.offset,
            self.point.y + self.direction.y * modifier as f64 * self.offset,
        )
    }
    fn deformed(&self, extra: f64) -> Rectangle {
        let x = if self.direction.x > 0.0 {
            self.original.x
        } else {
            self.point.x - extra
        };
        let y = if self.direction.y > 0.0 {
            self.original.y
        } else {
            self.point.y - extra
        };
        Rectangle::new(
            x,
            y,
            (self.original.x - self.point.x).abs() + extra,
            (self.original.y - self.point.y).abs() + extra,
        )
    }
}

struct Obstacle {
    bounds: Rectangle,
    corners: [usize; 4],
    center: Point,
}
impl Obstacle {
    fn proper(&self, point: Point) -> bool {
        point.x > self.bounds.x
            && point.x < self.bounds.x + self.bounds.width - PIXEL_EDGE
            && point.y > self.bounds.y
            && point.y < self.bounds.y + self.bounds.height - PIXEL_EDGE
    }
}

#[derive(Clone)]
struct Path {
    vertices: Vec<usize>,
    excluded: Vec<bool>,
    inverted: bool,
    marked: bool,
    child: Option<usize>,
    points: Vec<Point>,
}

struct Workspace {
    vertices: Vec<Vertex>,
    obstacles: Vec<Obstacle>,
    paths: Vec<Path>,
    spacing: f64,
}

impl Workspace {
    fn solve(
        bounds: &[Rectangle],
        metadata: &[RouteMetadata],
        spacing: u32,
    ) -> Result<Vec<Vec<Point>>, RouteError> {
        let mut workspace = Self {
            vertices: Vec::new(),
            obstacles: Vec::new(),
            paths: Vec::new(),
            spacing: spacing as f64,
        };
        for (index, bounds) in bounds.iter().copied().enumerate() {
            // Snapshot validation belongs to Runtime; standalone invocations also reject overflow.
            if ![
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
                bounds.x + bounds.width,
                bounds.y + bounds.height,
            ]
            .iter()
            .all(|value| value.is_finite())
                || bounds.width < PIXEL_EDGE
                || bounds.height < PIXEL_EDGE
            {
                return Err(RouteError::NoObstacleFreePath);
            }
            let first = workspace.vertices.len();
            let right = bounds.x + bounds.width - PIXEL_EDGE;
            let bottom = bounds.y + bounds.height - PIXEL_EDGE;
            for (point, direction) in [
                (Point::new(bounds.x, bounds.y), Point::new(-1.0, -1.0)),
                (Point::new(right, bounds.y), Point::new(1.0, -1.0)),
                (Point::new(right, bottom), Point::new(1.0, 1.0)),
                (Point::new(bounds.x, bottom), Point::new(-1.0, 1.0)),
            ] {
                let mut vertex = Vertex::endpoint(point);
                vertex.obstacle = Some(index);
                vertex.direction = direction;
                vertex.offset = workspace.spacing;
                workspace.vertices.push(vertex);
            }
            workspace.obstacles.push(Obstacle {
                bounds,
                corners: [first, first + 1, first + 2, first + 3],
                center: Point::new(
                    bounds.x + (bounds.width / 2.0).floor(),
                    bounds.y + (bounds.height / 2.0).floor(),
                ),
            });
        }
        for metadata in metadata {
            let source = Point::from(metadata.source.site.point);
            let target = Point::from(metadata.target.site.point);
            let start = workspace.vertices.len();
            workspace.vertices.push(Vertex::endpoint(source));
            let end = workspace.vertices.len();
            workspace.vertices.push(Vertex::endpoint(target));
            let excluded = workspace
                .obstacles
                .iter()
                .map(|obs| obs.proper(source) || obs.proper(target))
                .collect::<Vec<_>>();
            let vertices = workspace.shortest(start, end, &excluded)?;
            workspace.paths.push(Path {
                vertices,
                excluded,
                inverted: false,
                marked: false,
                child: None,
                points: Vec::new(),
            });
        }
        for path in &workspace.paths {
            for vertex in path
                .vertices
                .iter()
                .skip(1)
                .take(path.vertices.len().saturating_sub(2))
            {
                workspace.vertices[*vertex].total += 1;
            }
        }
        for vertex in 0..workspace.vertices.len() {
            if workspace.vertices[vertex].total > 0 {
                workspace.check_corner(vertex);
            }
        }
        workspace.grow();
        workspace.label();
        let order = workspace.order();
        workspace.bend(&order);
        let original_count = metadata.len();
        let mut outputs = Vec::with_capacity(original_count);
        for path in 0..original_count {
            let mut chain = Vec::new();
            let mut current = Some(path);
            while let Some(index) = current {
                chain.push(index);
                current = workspace.paths[index].child;
            }
            let mut points = workspace.paths[*chain.last().expect("nonempty chain")]
                .points
                .clone();
            for parent in chain.iter().rev().skip(1) {
                let mut prefix = workspace.paths[*parent].points.clone();
                prefix.pop();
                prefix.extend(points.into_iter().skip(1));
                points = prefix;
            }
            outputs.push(points);
        }
        Ok(outputs)
    }

    fn point(&self, vertex: usize) -> Point {
        self.vertices[vertex].point
    }

    // The complete graph of perimeter and tangent candidates replaces the upstream
    // lazy obstacle discovery/oval pruning. Both search the same Euclidean candidates.
    fn shortest(
        &self,
        start: usize,
        end: usize,
        excluded: &[bool],
    ) -> Result<Vec<usize>, RouteError> {
        let count = self.vertices.len();
        let mut graph = vec![Vec::new(); count];
        let mut link = |a: usize, b: usize, own: &[usize]| {
            if self.obstacles.iter().enumerate().any(|(index, obstacle)| {
                !excluded[index] && !own.contains(&index) && self.blocks(a, b, obstacle)
            }) {
                return;
            }
            if !graph[a].contains(&b) {
                graph[a].push(b);
                graph[b].push(a);
            }
        };
        link(start, end, &[]);
        for (index, obstacle) in self.obstacles.iter().enumerate() {
            if excluded[index] {
                continue;
            }
            for corner in 0..4 {
                link(
                    obstacle.corners[corner],
                    obstacle.corners[(corner + 1) % 4],
                    &[index],
                );
            }
            for endpoint in [start, end] {
                for corner in self.endpoint_corners(self.point(endpoint), obstacle) {
                    link(endpoint, obstacle.corners[corner], &[index]);
                }
            }
            for (other_index, other) in self.obstacles.iter().enumerate().take(index) {
                if excluded[other_index] {
                    continue;
                }
                for (a, b) in self.tangents(obstacle, other) {
                    link(a, b, &[index, other_index]);
                }
            }
        }
        let mut distance = vec![f64::INFINITY; count];
        let mut previous = vec![None; count];
        let mut permanent = vec![false; count];
        distance[start] = 0.0;
        loop {
            let next = (0..count)
                .filter(|index| !permanent[*index] && distance[*index].is_finite())
                .min_by(|a, b| distance[*a].total_cmp(&distance[*b]).then(a.cmp(b)));
            let Some(current) = next else {
                return Err(RouteError::NoObstacleFreePath);
            };
            if current == end {
                break;
            }
            permanent[current] = true;
            for neighbor in &graph[current] {
                let candidate =
                    distance[current] + length(self.point(current), self.point(*neighbor));
                if candidate < distance[*neighbor] {
                    distance[*neighbor] = candidate;
                    previous[*neighbor] = Some(current);
                }
            }
        }
        let mut result = vec![end];
        while *result.last().expect("end") != start {
            let parent =
                previous[*result.last().expect("end")].ok_or(RouteError::NoObstacleFreePath)?;
            result.push(parent);
        }
        result.reverse();
        Ok(result)
    }

    fn blocks(&self, a: usize, b: usize, obstacle: &Obstacle) -> bool {
        let (a, b) = (self.point(a), self.point(b));
        obstacle.proper(a)
            || obstacle.proper(b)
            || intersects(
                a,
                b,
                self.point(obstacle.corners[TL]),
                self.point(obstacle.corners[BR]),
            )
            || intersects(
                a,
                b,
                self.point(obstacle.corners[TR]),
                self.point(obstacle.corners[BL]),
            )
    }

    fn endpoint_corners(&self, point: Point, obstacle: &Obstacle) -> [usize; 2] {
        let bounds = obstacle.bounds;
        let west = point.x < bounds.x;
        let east = point.x >= bounds.x + bounds.width;
        let north = point.y < bounds.y;
        let south = point.y >= bounds.y + bounds.height;
        match (west, east, north, south) {
            (true, _, _, true) | (_, true, true, _) => [TL, BR],
            (true, _, true, _) | (_, true, _, true) => [TR, BL],
            (_, _, true, _) => [TL, TR],
            (_, true, _, _) => [BR, TR],
            (_, _, _, true) => [BR, BL],
            (true, _, _, _) => [TL, BL],
            _ if point.x == bounds.x => [TL, BL],
            _ if point.y == bounds.y => [TL, TR],
            _ if point.y == bounds.y + bounds.height - PIXEL_EDGE => [BL, BR],
            _ => [TR, BR],
        }
    }

    fn tangents(&self, source: &Obstacle, target: &Obstacle) -> Vec<(usize, usize)> {
        let s = source.bounds;
        let t = target.bounds;
        let sr = s.x + s.width;
        let sb = s.y + s.height;
        let tr = t.x + t.width;
        let tb = t.y + t.height;
        if rectangles_intersect(s, t) {
            let mut result = Vec::new();
            let mut connect = |a, b, diagonal_a: bool, diagonal_b: bool| {
                let (a, b) = (source.corners[a], target.corners[b]);
                let diag = |obstacle: &Obstacle, top_right| {
                    if top_right {
                        (obstacle.corners[BL], obstacle.corners[TR])
                    } else {
                        (obstacle.corners[TL], obstacle.corners[BR])
                    }
                };
                let (c, d) = diag(source, diagonal_a);
                let (e, f) = diag(target, diagonal_b);
                if !target.proper(self.point(a))
                    && !source.proper(self.point(b))
                    && !intersects(self.point(a), self.point(b), self.point(c), self.point(d))
                    && !intersects(self.point(a), self.point(b), self.point(e), self.point(f))
                {
                    result.push((a, b));
                }
            };
            connect(BL, BL, false, false);
            connect(BR, BR, true, true);
            connect(TL, TL, true, true);
            connect(TR, TR, false, false);
            if sb == tb {
                connect(BL, BR, false, true);
                connect(BR, BL, true, false);
            }
            if s.y == t.y {
                connect(TL, TR, true, false);
                connect(TR, TL, false, true);
            }
            if s.x == t.x {
                connect(BL, TL, false, true);
                connect(TL, BL, true, false);
            }
            if sr == tr {
                connect(BR, TR, true, false);
                connect(TR, BR, false, true);
            }
            return result;
        }
        if sb - PIXEL_EDGE < t.y || (tb - PIXEL_EDGE >= s.y && sr - PIXEL_EDGE < t.x) {
            return self.tangents(target, source);
        }
        let pairs = if tb - PIXEL_EDGE < s.y {
            let mut pairs = if t.x > s.x {
                vec![
                    (TL, TL),
                    if t.x < sr - PIXEL_EDGE {
                        (TR, BL)
                    } else {
                        (BR, TL)
                    },
                ]
            } else if s.x == t.x {
                vec![(TL, BL), (TR, BL)]
            } else {
                vec![(BL, BL), (TR, BL)]
            };
            pairs.extend(if tr < sr {
                vec![
                    (TR, TR),
                    if tr - PIXEL_EDGE > s.x {
                        (TL, BR)
                    } else {
                        (BL, TR)
                    },
                ]
            } else if sr == tr {
                vec![(TR, BR), (TL, BR)]
            } else {
                vec![(BR, BR), (TL, BR)]
            });
            pairs
        } else {
            let mut pairs = if t.y > s.y {
                vec![
                    (TL, TL),
                    if t.y < sb - PIXEL_EDGE {
                        (BL, TR)
                    } else {
                        (BR, TL)
                    },
                ]
            } else if s.y == t.y {
                vec![(TL, TR), (BL, TR)]
            } else {
                vec![(TR, TR), (BL, TR)]
            };
            pairs.extend(if tb < sb {
                vec![
                    (BL, BL),
                    if tb - PIXEL_EDGE > s.y {
                        (TL, BR)
                    } else {
                        (TR, BL)
                    },
                ]
            } else if sb == tb {
                vec![(BL, BR), (TL, BR)]
            } else {
                vec![(BR, BR), (TL, BR)]
            });
            pairs
        };
        pairs
            .into_iter()
            .map(|(a, b)| (source.corners[a], target.corners[b]))
            .collect()
    }

    fn check_corner(&mut self, index: usize) {
        let vertex = &self.vertices[index];
        if vertex.nearest != 0.0 || vertex.checked {
            return;
        }
        let point = vertex.point;
        let side = 2.0 * vertex.total as f64 * self.spacing + PIXEL_EDGE;
        let region = Rectangle::new(
            if vertex.direction.x > 0.0 {
                point.x
            } else {
                point.x - side
            },
            if vertex.direction.y < 0.0 {
                point.y - side
            } else {
                point.y
            },
            side,
            side,
        );
        let mut nearest = 0.0;
        for (other, obstacle) in self.obstacles.iter().enumerate() {
            if Some(other) == vertex.obstacle || !rectangles_intersect(region, obstacle.bounds) {
                continue;
            }
            let b = obstacle.bounds;
            if point.x >= b.x
                && point.x < b.x + b.width
                && point.y >= b.y
                && point.y < b.y + b.height
            {
                continue;
            }
            let xd = if point.x >= b.x + b.width {
                point.x - b.x - b.width + PIXEL_EDGE
            } else {
                b.x - point.x
            };
            let yd = if point.y < b.y {
                b.y - point.y
            } else {
                point.y - b.y - b.height + PIXEL_EDGE
            };
            let candidate = xd.max(yd);
            if nearest == 0.0 || candidate < nearest {
                nearest = candidate;
            }
        }
        let vertex = &mut self.vertices[index];
        vertex.nearest = nearest;
        vertex.checked = true;
        if nearest != 0.0 {
            // Draw2D performs integer division for spacing squeeze.
            vertex.offset = ((nearest / 2.0).trunc() - PIXEL_EDGE).trunc() / vertex.total as f64;
            vertex.offset = vertex.offset.trunc();
        }
    }

    fn grow_vertex(&mut self, index: usize) {
        let vertex = &mut self.vertices[index];
        if vertex.total == 0 {
            return;
        }
        let modifier = if vertex.nearest == 0.0 {
            vertex.total as f64 * self.spacing
        } else {
            (vertex.nearest / 2.0).trunc() - PIXEL_EDGE
        };
        vertex.point = Point::new(
            vertex.original.x + vertex.direction.x * modifier,
            vertex.original.y + vertex.direction.y * modifier,
        );
    }

    fn grow(&mut self) {
        for pass in 0..GROW_PASSES {
            for vertex in 0..self.vertices.len() {
                self.grow_vertex(vertex);
            }
            let mut changed = false;
            for path in 0..self.paths.len() {
                let old = self.paths[path].vertices.clone();
                let mut replacement = vec![old[0]];
                for segment in old.windows(2) {
                    if let Some(corner) = self.grown_intersection(segment[0], segment[1], path) {
                        self.vertices[corner].total += 1;
                        self.vertices[corner].checked = false;
                        self.vertices[corner].point = self.vertices[corner].original;
                        self.check_corner(corner);
                        self.grow_vertex(corner);
                        replacement.push(corner);
                        changed = true;
                    }
                    replacement.push(segment[1]);
                }
                self.paths[path].vertices = replacement;
            }
            for vertex in &mut self.vertices {
                vertex.point = vertex.original;
            }
            if !changed || pass + 1 == GROW_PASSES {
                break;
            }
        }
    }

    fn grown_intersection(&self, a: usize, b: usize, path: usize) -> Option<usize> {
        let source = self.point(a);
        let target = self.point(b);
        let negative = (target.y - source.y)
            * if target.x - source.x >= 0.0 {
                1.0
            } else {
                -1.0
            }
            < 0.0;
        for (index, obstacle) in self.obstacles.iter().enumerate() {
            if self.paths[path].excluded[index]
                || self.vertices[a].obstacle == Some(index)
                || self.vertices[b].obstacle == Some(index)
            {
                continue;
            }
            let diagonals = if negative {
                [(TL, BR), (BL, TR)]
            } else {
                [(BL, TR), (TL, BR)]
            };
            let mut hit = None;
            for (c, d) in diagonals {
                let (c, d) = (obstacle.corners[c], obstacle.corners[d]);
                let expanded = |corner: usize| {
                    let vertex = &self.vertices[corner];
                    Point::new(
                        vertex.point.x + vertex.direction.x * self.spacing,
                        vertex.point.y + vertex.direction.y * self.spacing,
                    )
                };
                if intersects(source, target, expanded(c), expanded(d)) {
                    hit = Some(
                        if length(source, self.point(c)) + length(target, self.point(c))
                            > length(source, self.point(d)) + length(target, self.point(d))
                        {
                            d
                        } else {
                            c
                        },
                    );
                    break;
                }
            }
            if let Some(corner) = hit {
                let region = self.vertices[corner].deformed(self.spacing);
                if [a, b].iter().any(|endpoint| {
                    self.vertices[*endpoint].obstacle.is_some()
                        && rectangles_intersect(
                            region,
                            self.vertices[*endpoint].deformed(self.spacing),
                        )
                }) {
                    continue;
                }
                return Some(corner);
            }
        }
        None
    }

    fn label(&mut self) {
        let mut stack = (0..self.paths.len()).collect::<Vec<_>>();
        while let Some(path) = stack.pop() {
            if self.paths[path].marked {
                continue;
            }
            self.paths[path].marked = true;
            let sequence = self.paths[path].vertices.clone();
            let mut agree = false;
            for position in 1..sequence.len().saturating_sub(1) {
                let corner = sequence[position];
                let obstacle = self.vertices[corner].obstacle.expect("intermediate corner");
                let cross = cross(
                    self.point(sequence[position - 1]),
                    self.point(corner),
                    self.obstacles[obstacle].center,
                );
                let turn = self.vertices[corner].turn;
                let inverted = self.paths[path].inverted;
                let disagree = (!inverted
                    && ((cross > 0.0 && turn == Turn::Outie)
                        || (cross < 0.0 && turn == Turn::Innie)))
                    || (inverted
                        && ((cross < 0.0 && turn == Turn::Outie)
                            || (cross > 0.0 && turn == Turn::Innie)));
                if turn == Turn::Unset {
                    self.vertices[corner].turn = if cross > 0.0 {
                        if inverted { Turn::Outie } else { Turn::Innie }
                    } else if cross < 0.0 {
                        if inverted { Turn::Innie } else { Turn::Outie }
                    } else {
                        let previous = self.vertices[sequence[position - 1]].turn;
                        if previous == Turn::Unset {
                            Turn::Innie
                        } else {
                            previous
                        }
                    };
                } else if disagree {
                    if inverted || agree {
                        let child = self.paths.len();
                        let next_child = self.paths[path].child.take();
                        self.paths.push(Path {
                            vertices: sequence[position - 1..].to_vec(),
                            excluded: self.paths[path].excluded.clone(),
                            inverted: false,
                            marked: false,
                            child: next_child,
                            points: Vec::new(),
                        });
                        self.paths[path].vertices.truncate(position + 1);
                        self.paths[path].child = Some(child);
                        stack.push(child);
                        break;
                    }
                    self.paths[path].inverted = true;
                    for previous in sequence.iter().take(position).skip(1) {
                        self.vertices[*previous].turn = self.vertices[*previous].turn.invert();
                    }
                } else {
                    agree = true;
                }
                let cosine = angle(
                    self.point(sequence[position - 1]),
                    self.point(corner),
                    self.point(sequence[position + 1]),
                );
                let routes = &mut self.vertices[corner].paths;
                if !routes.iter().any(|(id, _)| *id == path) {
                    routes.push((path, cosine));
                }
            }
        }
        for path in &mut self.paths {
            path.marked = false;
        }
    }

    fn order(&mut self) -> Vec<usize> {
        // Explicit DFS frames avoid unbounded recursion for large connection groups.
        let mut order = Vec::new();
        for root in 0..self.paths.len() {
            let mut stack = vec![(root, false)];
            while let Some((path, finish)) = stack.pop() {
                if finish {
                    order.push(path);
                    continue;
                }
                if self.paths[path].marked {
                    continue;
                }
                self.paths[path].marked = true;
                stack.push((path, true));
                let sequence = &self.paths[path].vertices;
                let mut predecessors = Vec::new();
                for corner in sequence
                    .iter()
                    .skip(1)
                    .take(sequence.len().saturating_sub(2))
                {
                    let routes = &self.vertices[*corner].paths;
                    let Some((_, this)) = routes.iter().find(|(id, _)| *id == path) else {
                        continue;
                    };
                    let this = if self.paths[path].inverted {
                        -*this
                    } else {
                        *this
                    };
                    for (other, cosine) in routes {
                        let other_angle = if self.paths[*other].inverted {
                            -*cosine
                        } else {
                            *cosine
                        };
                        if other_angle < this && !predecessors.contains(other) {
                            predecessors.push(*other);
                        }
                    }
                }
                stack.extend(predecessors.into_iter().rev().map(|path| (path, false)));
            }
        }
        order
    }

    fn bend(&mut self, order: &[usize]) {
        for path in order {
            let sequence = &self.paths[*path].vertices;
            let mut points = vec![self.point(sequence[0])];
            for corner in sequence
                .iter()
                .skip(1)
                .take(sequence.len().saturating_sub(2))
            {
                let vertex = &mut self.vertices[*corner];
                if vertex.turn == Turn::Innie {
                    vertex.count += 1;
                    points.push(vertex.bend(vertex.count));
                } else {
                    points.push(vertex.bend(vertex.total));
                    vertex.total = vertex.total.saturating_sub(1);
                }
            }
            points.push(self.point(*sequence.last().expect("endpoint")));
            self.paths[*path].points = points;
        }
    }
}

fn length(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}
fn cross(a: Point, b: Point, c: Point) -> f64 {
    (a.x - b.x) * (c.y - b.y) - (a.y - b.y) * (c.x - b.x)
}
fn angle(a: Point, b: Point, c: Point) -> f64 {
    let denominator = length(a, b) * length(b, c);
    if denominator == 0.0 {
        return 0.0;
    }
    let cosine = ((a.x - b.x) * (c.x - b.x) + (a.y - b.y) * (c.y - b.y)) / denominator;
    if cross(a, b, c) < 0.0 {
        1.0 + cosine
    } else {
        -(1.0 + cosine)
    }
}
fn rectangles_intersect(a: Rectangle, b: Rectangle) -> bool {
    a.width > 0.0
        && a.height > 0.0
        && b.width > 0.0
        && b.height > 0.0
        && a.x < b.x + b.width
        && b.x < a.x + a.width
        && a.y < b.y + b.height
        && b.y < a.y + a.height
}
fn intersects(a: Point, b: Point, c: Point, d: Point) -> bool {
    if a.x.max(b.x) < c.x.min(d.x)
        || c.x.max(d.x) < a.x.min(b.x)
        || a.y.max(b.y) < c.y.min(d.y)
        || c.y.max(d.y) < a.y.min(b.y)
    {
        return false;
    }
    let opposite =
        |x: f64, y: f64| x == 0.0 || y == 0.0 || x.is_sign_positive() != y.is_sign_positive();
    opposite(cross(a, b, c), cross(a, b, d)) && opposite(cross(c, d, a), cross(c, d, b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::{AnchorSite, RouteEndpoint};

    fn metadata(source: (f64, f64), target: (f64, f64)) -> RouteMetadata {
        let endpoint = |(x, y)| {
            let point = GeometryPoint::new(x, y);
            RouteEndpoint {
                reference: point,
                site: AnchorSite {
                    point,
                    outward_normal: None,
                },
            }
        };
        RouteMetadata {
            source: endpoint(source),
            target: endpoint(target),
        }
    }

    fn checked(bounds: &[Rectangle], metadata: &[RouteMetadata]) -> Vec<Vec<GeometryPoint>> {
        let points = Workspace::solve(bounds, metadata, VISIBILITY_DEFAULT_SPACING).unwrap();
        points
            .into_iter()
            .zip(metadata)
            .map(|(points, metadata)| {
                let points = points
                    .into_iter()
                    .map(GeometryPoint::from)
                    .collect::<Vec<_>>();
                let output = RouteOutput::new(PointList::from_points(points), *metadata).unwrap();
                for bounds in bounds {
                    let obstacle = Obstacle {
                        bounds: *bounds,
                        corners: [0; 4],
                        center: Point::new(0.0, 0.0),
                    };
                    if obstacle.proper(metadata.source.site.point.into())
                        || obstacle.proper(metadata.target.site.point.into())
                    {
                        continue;
                    }
                    for segment in output.points().as_slice().windows(2) {
                        // Independent open-rectangle clipping: boundary tangency is permitted.
                        let mut enter: f64 = 0.0;
                        let mut exit: f64 = 1.0;
                        for (origin, delta, low, high) in [
                            (
                                segment[0].x(),
                                segment[1].x() - segment[0].x(),
                                bounds.x,
                                bounds.x + bounds.width - PIXEL_EDGE,
                            ),
                            (
                                segment[0].y(),
                                segment[1].y() - segment[0].y(),
                                bounds.y,
                                bounds.y + bounds.height - PIXEL_EDGE,
                            ),
                        ] {
                            if delta == 0.0 {
                                if origin <= low || origin >= high {
                                    enter = 1.0;
                                    exit = 0.0;
                                    break;
                                }
                            } else {
                                let a = (low - origin) / delta;
                                let b = (high - origin) / delta;
                                enter = enter.max(a.min(b));
                                exit = exit.min(a.max(b));
                            }
                        }
                        assert!(enter >= exit, "segment {segment:?} enters {bounds:?}");
                    }
                }
                output.points().iter().copied().collect()
            })
            .collect()
    }

    #[test]
    fn unobstructed_diagonal_and_identical_endpoints_are_finite() {
        let routes = checked(
            &[],
            &[
                metadata((0.0, 0.0), (100.0, 70.0)),
                metadata((20.0, 20.0), (20.0, 20.0)),
            ],
        );
        assert_eq!(
            routes[0],
            vec![
                GeometryPoint::new(0.0, 0.0),
                GeometryPoint::new(100.0, 70.0)
            ]
        );
        assert_eq!(routes[1].len(), 2);
    }

    #[test]
    fn single_obstacle_has_diagonal_tangents_and_shared_corner_spacing() {
        let obstacle = Rectangle::new(40.0, 30.0, 30.0, 50.0);
        let pair = metadata((0.0, 50.0), (110.0, 50.0));
        let routes = checked(
            &[obstacle],
            &[pair, pair, metadata((110.0, 50.0), (0.0, 50.0))],
        );
        assert!(routes.iter().all(|route| route.len() > 2));
        assert_ne!(routes[0], routes[1]);
        assert!(routes[0].windows(2).any(|segment| segment[0].x() != segment[1].x()
            && segment[0].y() != segment[1].y()));
        assert_eq!(
            routes,
            checked(
                &[obstacle],
                &[pair, pair, metadata((110.0, 50.0), (0.0, 50.0))]
            )
        );
        for route in routes {
            assert!(
                route
                    .iter()
                    .skip(1)
                    .take(route.len() - 2)
                    .all(|point| point.y() < obstacle.y)
            );
        }
    }

    #[test]
    fn endpoint_strictly_inside_excludes_obstacle_but_boundary_does_not() {
        let obstacle = Rectangle::new(40.0, 30.0, 30.0, 50.0);
        let inside = checked(&[obstacle], &[metadata((50.0, 50.0), (100.0, 50.0))]);
        assert_eq!(inside[0].len(), 2);
        let boundary = checked(&[obstacle], &[metadata((40.0, 50.0), (100.0, 50.0))]);
        assert!(boundary[0].len() > 2);
    }

    #[test]
    fn separated_overlapping_and_squeezed_obstacles_keep_endpoints_and_stable_output() {
        for bounds in [
            vec![
                Rectangle::new(40.0, 30.0, 30.0, 50.0),
                Rectangle::new(100.0, 20.0, 30.0, 70.0),
            ],
            vec![
                Rectangle::new(40.0, 30.0, 50.0, 50.0),
                Rectangle::new(70.0, 20.0, 50.0, 70.0),
            ],
            vec![
                Rectangle::new(40.0, 30.0, 30.0, 50.0),
                Rectangle::new(35.0, 0.0, 40.0, 25.0),
            ],
            vec![
                Rectangle::new(30.0, 80.0, 30.0, 30.0),
                Rectangle::new(80.0, 10.0, 30.0, 30.0),
            ],
        ] {
            let inputs = [
                metadata((0.0, 50.0), (180.0, 50.0)),
                metadata((180.0, 55.0), (0.0, 55.0)),
            ];
            let first = checked(&bounds, &inputs);
            assert_eq!(first, checked(&bounds, &inputs));
        }
    }

    #[test]
    fn grouped_routes_avoid_obstacle_interiors_across_endpoint_directions() {
        let bounds = [
            Rectangle::new(40.0, 30.0, 30.0, 50.0),
            Rectangle::new(90.0, 15.0, 35.0, 90.0),
        ];
        for y in [-20.0, 0.0, 25.0, 50.0, 75.0, 110.0, 140.0] {
            let inputs = [
                metadata((0.0, y), (180.0, 50.0)),
                metadata((180.0, 55.0), (0.0, y)),
                metadata((0.0, y + 5.0), (180.0, 60.0)),
                metadata((20.0, -20.0), (140.0, 130.0)),
            ];
            assert_eq!(checked(&bounds, &inputs), checked(&bounds, &inputs));
        }
    }
}
